<script setup lang="ts">
// Was auf der Platte liegt: Clients links, Spielordner als Reiter, Addons als
// Tabelle. Die Daten kommen aus dem Scan, nicht aus dem Katalog.
//
// Deinstallieren geht über den Papierkorb, nie über Löschen: der Ordner wandert
// nach `~/.local/share/Trash` mit `.trashinfo`, damit er auch außerhalb von
// Hexwerk zurückholbar ist. Vorher wird gefragt — ein Klick soll nichts
// unwiderruflich machen (so macht es die GTK-Fassung, app.rs:1785).
import { computed, ref } from "vue";
import * as UI from "@weppelmann/components";
import { IconTrash } from "@weppelmann/icons";
import { deinstalliere, zurueckholen } from "../api";
import { t } from "../i18n";
import { bytes, client, oeffneLokal, rescan, state, zeigeMeldung } from "../store";

const aktuell = computed(() => client());
const flavor = computed(() => aktuell.value?.flavors[state.selectedFlavor]);
const spalten = computed(() => [
  { key: "name", label: t("iv.col.addon"), sortable: true },
  { key: "version", label: t("dt.cap.version"), align: "end" as const, sortable: true },
  { key: "interface", label: t("iv.col.interface"), align: "end" as const },
  { key: "size", label: t("dt.cap.size"), align: "end" as const },
]);
/** Spielordner als Reiter; der Schluessel ist der Index. */
const flavorTabs = computed(() =>
  (aktuell.value?.flavors ?? []).map((f, i) => ({ id: String(i), label: f.label, badge: String(f.addons.length) })),
);

const tabelle = computed(() =>
  (flavor.value?.addons ?? []).length
    ? ({ kind: "fertig", rows: (flavor.value?.addons ?? []).map((a) => ({
        name: a.title,
        version: a.version ?? "—",
        interface: a.interface.length ? a.interface[a.interface.length - 1] : "—",
        size: bytes(a.size_bytes),
      })) } as const)
    : ({ kind: "leer", text: t("iv.empty.title") } as const),
);

/** Markierte Zeile: alles, was sich auf ein Addon bezieht, hängt daran. */
const zeile = ref<number | null>(null);
const gewaehlt = computed(() => (zeile.value === null ? null : flavor.value?.addons[zeile.value] ?? null));

const frage = ref(false);
const laeuft = ref(false);

async function inDenPapierkorb() {
  const addon = gewaehlt.value;
  const ordner = flavor.value?.addons_dir;
  if (!addon || !ordner) return;
  laeuft.value = true;
  try {
    const imKorb = await deinstalliere(ordner, addon.folder);
    zeigeMeldung(t("iv.trash.done", addon.title), "ok");
    zeile.value = null;
    // Zurückholbar: der Name im Papierkorb bleibt gemerkt, bis etwas anderes passiert.
    zuletztGeloescht.value = imKorb;
    await rescan();
  } catch (fehler) {
    zeigeMeldung(String(fehler), "error");
  } finally {
    laeuft.value = false;
  }
}

const zuletztGeloescht = ref<string | null>(null);

async function rueckgaengig() {
  if (!zuletztGeloescht.value) return;
  try {
    const pfad = await zurueckholen(zuletztGeloescht.value);
    zeigeMeldung(pfad, "ok");
    zuletztGeloescht.value = null;
    await rescan();
  } catch (fehler) {
    zeigeMeldung(String(fehler), "error");
  }
}
</script>

<template>
  <div class="inst">
    <UI.WpSidePanel :title="t('nav.clients')" :count="state.clients.length">
      <UI.WpChoiceCard v-for="(c, i) in state.clients" :key="c.id" :title="c.label"
                       :meta="`${c.origin} · ${c.flavors.length} Ordner`" :active="i === state.selectedClient"
                       @press="state.selectedClient = i; state.selectedFlavor = 0; zeile = null" />
      <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading" :title="t('cl.none.title')"
                       :text="t('cl.none.text')" />
    </UI.WpSidePanel>

    <main class="inst__main">
      <div class="kopf">
        <UI.WpTabs :model-value="String(state.selectedFlavor)" :tabs="flavorTabs" :label="t('rail.view')"
                   @update:model-value="(v: string) => { state.selectedFlavor = Number(v); zeile = null; }" />

        <div class="aktionen">
          <UI.WpButton :label="t('iv.trash.undo')" size="sm" variant="secondary"
                       :state="zuletztGeloescht ? 'default' : 'disabled'" @press="rueckgaengig" />
          <UI.WpButton :label="t('iv.remove')" size="sm" variant="secondary" :icon="IconTrash"
                       :state="!gewaehlt || laeuft ? 'disabled' : 'default'"
                       @press="frage = true" />
        </div>
      </div>

      <p v-if="flavor" class="pfad">{{ flavor.addons_dir }}</p>
      <UI.WpTable :columns="spalten" :state="tabelle" :caption="t('iv.title')"
                  @row-click="(i: number) => (zeile = i)"
                  @row-activate="(i: number) => flavor?.addons[i] && oeffneLokal(flavor.addons[i])" />
      <p v-if="gewaehlt" class="wahl">{{ t('iv.col.addon') }}: {{ gewaehlt.title }}</p>
    </main>

    <UI.WpDialog :open="frage" :title="t('iv.remove')" tone="warn"
                 :description="gewaehlt ? t('iv.trash.text', gewaehlt.title) : ''"
                 :confirm-label="t('iv.trash.confirm')" :cancel-label="t('iv.install.cancel')"
                 @update:open="frage = $event" @confirm="inDenPapierkorb" />
  </div>
</template>

<style scoped>
.inst { flex: 1; min-height: 0; display: flex; }
.inst__main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--wp-space-2); padding: var(--wp-space-3); overflow: auto; }
.kopf { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); }
.aktionen { display: flex; gap: var(--wp-space-2); }
.pfad { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.wahl { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
</style>
