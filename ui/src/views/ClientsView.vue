<script setup lang="ts">
// Clients: die Übersicht, welches Spiel wo liegt. Je Client die Herkunft
// (Steam, Faugus, manuell …), das Präfix und die Spielordner mit Interface,
// Addon-Zahl und belegtem Platz.
//
// Das ist die Ansicht zum Nachsehen und Aufräumen; die Arbeit (Addons
// durchsuchen) passiert im Katalog und unter "Installiert".
import { computed, ref } from "vue";
import * as UI from "@weppelmann/components";
import { t } from "../i18n";
import { bytes, lastScan, rescan, state } from "../store";
import { ordnerEntfernen, ordnerHinzufuegen, type Client } from "../api";

/** Pfad, den der Nutzer eintippt oder einfuegt. */
const pfad = ref("");
const fehler = ref<string | null>(null);
const laeuft = ref(false);

/**
 * Ordner aufnehmen: pruefen, speichern, neu scannen. Die Pruefung selbst
 * passiert im Rust-Teil; hier steht nur, was mit dem Ergebnis geschieht.
 */
async function hinzufuegen() {
  const wert = pfad.value.trim();
  if (!wert || laeuft.value) return;
  laeuft.value = true;
  fehler.value = null;
  try {
    await ordnerHinzufuegen(wert);
    pfad.value = "";
    await rescan();
  } catch (e) {
    // Der Rust-Teil liefert die Begruendung als String.
    fehler.value = typeof e === "string" ? e : String(e);
  } finally {
    laeuft.value = false;
  }
}

/** Einen selbst angegebenen Ordner wieder austragen. */
async function entfernen(client: Client) {
  if (laeuft.value) return;
  laeuft.value = true;
  fehler.value = null;
  try {
    await ordnerEntfernen(client.prefix);
    await rescan();
  } catch (e) {
    fehler.value = typeof e === "string" ? e : String(e);
  } finally {
    laeuft.value = false;
  }
}

const gesamt = computed(() => state.clients.reduce(
  (summe, c) => summe + c.flavors.reduce((s, f) => s + f.size_bytes, 0), 0));

// Die Spielordner als Tabelle: Ordner, Interface-Nummern, Anzahl, Groesse.
// Als Tabelle stehen die Werte untereinander — in einer Zeile mit Abstaenden
// schwimmt jede Zeile anders, je nachdem wie lang die Interface-Liste ist.
const ordnerSpalten = [
  { key: "ordner", label: t("cl.col.folder") },
  { key: "interface", label: t("iv.col.interface"), align: "end" as const },
  { key: "addons", label: t("cl.col.addons"), align: "end" as const },
  { key: "size", label: t("dt.cap.size"), align: "end" as const },
];

function ordnerTabelle(flavors: Client["flavors"]) {
  return {
    kind: "fertig" as const,
    rows: flavors.map((f) => ({
      ordner: f.dir_name,
      interface: f.interface.join(" · ") || "—",
      addons: f.addons.length,
      size: bytes(f.size_bytes),
    })),
  };
}

</script>

<template>
  <div class="cl">
    <!-- Titel und Untertitel stehen im Kopfbereich des Fensters (App.vue). -->
    <header class="cl__kopf">
      <p class="cl__sub">{{ t("cl.sub") }}</p>
    </header>

    <!-- Eigener Ordner: fuer Installationen ausserhalb der Programmordner
         (anderes Laufwerk, Netzlaufwerk, eigener Pfad). -->
    <section class="eigen">
      <UI.WpFormField :label="t('cl.add.title')" :hint="t('cl.add.hint')"
                      :error="fehler ? `${t('cl.add.error')}: ${fehler}` : undefined">
        <template #default="{ id, describedby, invalid }">
          <div class="eigen__zeile">
            <UI.WpInput :id="id" v-model="pfad" :aria-describedby="describedby"
                        :aria-invalid="invalid || undefined"
                        :state="invalid ? 'invalid' : 'default'"
                        :placeholder="t('cl.add.placeholder')" @keyup.enter="hinzufuegen" />
            <UI.WpButton :label="t('cl.add.button')" size="sm"
                         :state="laeuft ? 'busy' : 'default'" @press="hinzufuegen" />
          </div>
        </template>
      </UI.WpFormField>
    </section>

    <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading"
                     :title="t('cl.none.title')" :text="t('cl.none.text')" />

    <div class="cl__liste">
      <UI.WpCard v-for="c in state.clients" :key="c.id" :title="c.label" class="karte" flush>
        <template #actions>
          <UI.WpBadge tone="neutral" :label="c.origin.toUpperCase()" />
          <UI.WpButton v-if="c.origin_slug === 'manual'" :label="t('cl.remove')" size="sm"
                       :state="laeuft ? 'busy' : 'default'" @press="entfernen(c)" />
        </template>
        <UI.WpTable class="ordner-tabelle" :columns="ordnerSpalten" :state="ordnerTabelle(c.flavors)" />
      </UI.WpCard>
    </div>

    <!-- Was sonst noch gesucht wird; findet eine Herkunft nichts, fehlt sie hier. -->
    <section class="hinweis">
      <h3 class="hinweis__titel">{{ t("cl.more.title") }}</h3>
      <p class="hinweis__text">{{ t("cl.more.text") }}</p>
    </section>

    <footer class="cl__fuss">
      <span class="cl__zaehler">
        {{ t("st.intros", state.notes.length, state.clients.length, state.addonCount) }}
      </span>
      <UI.WpButton :label="t('cl.rescan')" size="sm" :state="state.loading ? 'busy' : 'default'" @press="rescan" />
    </footer>
  </div>
</template>

<style scoped>
.cl { flex: 1; overflow: auto; padding: var(--wp-space-4); display: flex; flex-direction: column; gap: var(--wp-space-3); }
.cl__kopf { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); }
.cl__titel { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-xl); }
.cl__sub { margin: 2px 0 0; font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.cl__liste { display: flex; flex-direction: column; gap: var(--wp-space-3); }
/* Eigener Ordner: ruhiger Kasten, eine Zeile mit Feld und Knopf. */
.eigen { padding: var(--wp-space-3); border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.eigen__zeile { display: flex; gap: var(--wp-space-2); align-items: center; }
.karte__pfad { margin: 0 0 var(--wp-space-2); font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.hinweis { padding: var(--wp-space-3); border: var(--wp-size-border) dashed var(--wp-color-border-subtle); }
.hinweis__titel { margin: 0 0 4px; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-sm); }
.hinweis__text { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); }
.hinweis code { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-default); }
.cl__fuss { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); margin-top: auto; padding-top: var(--wp-space-3); }
.cl__zaehler { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
/* Eine Zeile je Spielordner: Name, Interface-Nummern, Anzahl, Groesse. */
/* Die Tabelle sitzt in der Karte: kein zweiter Rahmen, kein Aussenabstand. */
.ordner-tabelle :deep(.wp-table) { border: 0; }
/* Die Tabelle liegt buendig in der Karte — kein Kasten im Kasten. Die Zellen
   tragen ihren eigenen Abstand, damit der Text nicht am Rand klebt. */
.karte :deep(.wp-card__body) { padding: 0; }
.ordner-tabelle :deep(table) { font-size: var(--wp-font-size-xs); }
.ordner-tabelle :deep(th) { padding: 2px var(--wp-space-2); background: transparent;
  border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.ordner-tabelle :deep(td) { padding: 3px var(--wp-space-2); }
.ordner-tabelle :deep(th:first-child),
.ordner-tabelle :deep(td:first-child) { padding-left: var(--wp-space-3); }
.ordner-tabelle :deep(th:last-child),
.ordner-tabelle :deep(td:last-child) { padding-right: var(--wp-space-3); }
.ordner-tabelle :deep(td:first-child) { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-muted); }
.ordner-tabelle :deep(td:not(:first-child)) { font-family: var(--wp-font-family-mono);
  color: var(--wp-color-text-faint); }
</style>
