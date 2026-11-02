<script setup lang="ts">
// Fensteraufbau wie in der GTK-Fassung: Leiste links (Marke, Ansichten,
// Quellen), rechts Kopfbereich mit Titel, darunter der Bildschirm, unten die
// Fusszeile mit Zustand und Zahlen.
//
// Alles, was ein Baustein sein kann, kommt aus der Bibliothek: WpNavList fuer die
// Ansichten, WpFactList fuer Bestand und Quellen, WpStatusBar fuer die Fusszeile,
// WpToolbar fuer den Kopf. In dieser Datei steht nur noch, was Hexwerk eigen ist.
//
// Die Detailseite haengt an einer Auswahl, nicht an der Leiste — sie wird aus
// Katalog oder Installiert geoeffnet und ueber die Brotkrumen verlassen.
import { computed, onMounted } from "vue";
import * as UI from "@weppelmann/components";
import { IconFolder, IconMoon, IconPlug, IconSearch, IconSettings, IconSun } from "@weppelmann/icons";
import { t } from "./i18n";
import { ansicht, einstellungen, gewaehlt, installation, katalog, ladeStats, lastScan, meldung, quellenName, rescan, state, sync, thema, wendeThema } from "./store";
import ClientsView from "./views/ClientsView.vue";
import InstalliertView from "./views/InstalliertView.vue";
import KatalogView from "./views/KatalogView.vue";
import DetailView from "./views/DetailView.vue";
import EinstellungenView from "./views/EinstellungenView.vue";

// Titel und Untertitel kommen aus lang.rs; die Sprache schaltet sofort um,
// weil `t()` die reaktive Sprache liest.
const seiten = computed(() => [
  { id: "clients", label: t("nav.clients"), untertitel: t("page.clients.sub"), icon: IconPlug },
  { id: "installiert", label: t("nav.installed"), untertitel: t("page.installed.sub"), icon: IconFolder },
  { id: "katalog", label: t("nav.catalog"), untertitel: t("page.catalog.sub"), icon: IconSearch },
  { id: "einstellungen", label: t("nav.settings"), untertitel: t("page.settings.sub"), icon: IconSettings },
]);

/** Zaehler je Leisten-Eintrag. */
function zaehl(id: string): string {
  if (id === "clients") return String(state.clients.length);
  if (id === "installiert") return String(state.addonCount);
  if (id === "katalog") return String(katalog.stats?.catalogue ?? 0);
  return "";
}

/** Eintraege fuer die Leiste, in der Form der Bibliothek. */
const eintraege = computed(() =>
  seiten.value.map((s) => ({ id: s.id, label: s.label, icon: s.icon, count: zaehl(s.id) })),
);

const kopf = computed(() => {
  if (ansicht.value === "detail") {
    return { label: gewaehlt()?.name ?? t("nav.detail"), untertitel: t("page.detail.sub") };
  }
  return seiten.value.find((s) => s.id === ansicht.value) ?? seiten.value[0];
});

/** Bestand als zwei Fakten — dieselbe Form wie die Fakten der Detailseite. */
const bestand = computed(() => [
  { key: t("rail.addons"), value: String(state.addonCount), mono: true },
  { key: t("rail.origins"), value: String(state.notes.length), mono: true },
]);

// Quellen mit Anzahl aus dem Index; der Zustand kommt aus dem letzten Lauf.
const quellen = computed(() =>
  (katalog.stats?.per_source ?? []).map(([name, anzahl]) => ({ name, anzahl, ok: anzahl > 0 })),
);

/** Drei Themes im Wechsel: dunkel, hell, mono. */
function schalteThema() {
  const reihe = ["dark", "light", "mono"] as const;
  const jetzt = reihe.indexOf(thema.value);
  wendeThema(reihe[(jetzt + 1) % reihe.length]);
}
const themaIcon = computed(() => (thema.value === "dark" ? IconSun : IconMoon));

/** "Kompakte Leiste, nur Symbole" aus den Einstellungen. */
const kompakt = computed(() => einstellungen.werte?.nav === "compact");

/** Laeuft gerade etwas im Hintergrund? */
const beschaeftigt = computed(() => state.loading || installation.laeuft || sync.laeuft);
const laeuft = computed(() => (installation.laeuft ? t("st.kw.install") : sync.laeuft ? t("st.kw.catalog") : t("st.kw.scan")));

const zustand = computed(() => {
  if (state.loading) return t("cl.running");
  if (state.error) return state.error;
  if (lastScan.value) return `${t("footer.ready")} · ${t("st.ready", katalog.stats?.catalogue ?? 0)}`;
  return t("st.footer.idle");
});

onMounted(() => {
  rescan();
  // Die Leiste zeigt Katalogzahl und Quellen, bevor jemand den Katalog oeffnet.
  ladeStats();
});
</script>

<template>
  <div class="fenster">
    <!-- ------------------------------------------------------------ Leiste -->
    <aside class="leiste" :class="{ 'is-compact': kompakt }">
      <div class="marke">
        <img class="marke__zeichen" src="/icon.svg" alt="" width="22" height="22" />
        <span class="marke__wort">HEXWERK</span>
      </div>

      <UI.WpNavList :entries="eintraege" :model-value="ansicht" :title="t('rail.view')"
                    :compact="kompakt" :label="t('rail.view')"
                    @update:model-value="(id: string) => (ansicht = id as typeof ansicht.value)" />

      <template v-if="!kompakt">
        <UI.WpSectionLabel :label="t('rail.stock')" />
        <div class="bestand"><UI.WpFactList :facts="bestand" label-width="1fr" /></div>

        <UI.WpSectionLabel :label="t('nav.sources')" />
        <div class="quellen">
          <div v-for="q in quellen" :key="q.name" class="quelle">
            <span class="quelle__punkt" :class="q.ok ? 'is-ok' : 'is-off'" />
            <span class="quelle__name">{{ quellenName(q.name) }}</span>
            <span class="quelle__zahl">{{ q.anzahl }}</span>
          </div>
          <p v-if="quellen.length === 0" class="quellen__leer">{{ t("st.footer.noScan") }}</p>
        </div>
      </template>
    </aside>

    <!-- ------------------------------------------------------------ Inhalt -->
    <section class="inhalt">
      <UI.WpToolbar class="kopf" :title="kopf.label" :subtitle="kopf.untertitel">
        <template #actions>
          <div v-if="ansicht === 'detail'" class="krumen">
            <button type="button" class="krumen__link" @click="ansicht = 'katalog'">Katalog</button>
            <span class="krumen__trenner">›</span>
            <span class="krumen__hier">{{ gewaehlt()?.name }}</span>
          </div>
          <button type="button" class="thema" :aria-label="thema" :title="thema" @click="schalteThema">
            <UI.WpIcon :icon="themaIcon" size="sm" />
          </button>
        </template>
      </UI.WpToolbar>

      <main class="seite">
        <ClientsView v-if="ansicht === 'clients'" />
        <InstalliertView v-else-if="ansicht === 'installiert'" />
        <KatalogView v-else-if="ansicht === 'katalog'" />
        <DetailView v-else-if="ansicht === 'detail'" />
        <EinstellungenView v-else />
      </main>

      <div v-if="meldung" class="toast">
        <UI.WpToast :tone="meldung.ton" :title="meldung.titel" dismissible @close="meldung = null" />
      </div>

      <UI.WpStatusBar brand="HEXWERK" :state="zustand"
                      :counts="t('st.kw.counts', katalog.stats?.catalogue ?? 0, state.addonCount, state.clients.length)">
        <template #lead>
          <UI.WpSpinner v-if="beschaeftigt" size="sm" />
          <span v-if="beschaeftigt" class="fuss__zahl">{{ laeuft }}</span>
        </template>
      </UI.WpStatusBar>
    </section>
  </div>
</template>

<style scoped>
.fenster { height: 100%; display: flex; background: var(--wp-color-bg-canvas); }

/* ---- Leiste ---------------------------------------------------------- */
.leiste { width: 232px; flex: none; display: flex; flex-direction: column; gap: var(--wp-space-1);
  background: var(--wp-color-bg-panel); border-right: var(--wp-size-border) solid var(--wp-color-border-subtle);
  padding-bottom: var(--wp-space-3); overflow-y: auto; }
.marke { display: flex; align-items: center; gap: var(--wp-space-2); padding: var(--wp-space-3) var(--wp-space-3) var(--wp-space-2); }
.marke__zeichen { color: var(--wp-color-accent-default); }
.marke__wort { font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-lg); letter-spacing: 0.08em; }
.bestand { padding: 0 var(--wp-space-3); }
.quellen { display: flex; flex-direction: column; gap: 1px; padding: 0 var(--wp-space-3); }
.quelle { display: flex; align-items: center; gap: var(--wp-space-2); font-size: var(--wp-font-size-xs); }
.quelle__punkt { width: 6px; height: 6px; flex: none; }
.quelle__punkt.is-ok { background: var(--wp-color-status-ok); }
.quelle__punkt.is-off { background: var(--wp-color-border-strong); }
.quelle__name { flex: 1; color: var(--wp-color-text-muted); }
.quelle__zahl { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-faint); }
.quellen__leer { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }

/* ---- Inhalt ---------------------------------------------------------- */
.inhalt { flex: 1; min-width: 0; display: flex; flex-direction: column; position: relative; }
.kopf { padding: var(--wp-space-2) var(--wp-space-4); border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle);
  background: var(--wp-color-bg-panel); }
.krumen { display: flex; align-items: center; gap: 6px; font-size: var(--wp-font-size-sm); }
.krumen__link { background: none; border: 0; padding: 0; color: var(--wp-color-accent-default); font: inherit; cursor: pointer; }
.krumen__trenner { color: var(--wp-color-text-faint); }
.krumen__hier { color: var(--wp-color-text-default); }
.thema { width: 30px; height: 28px; display: inline-flex; align-items: center; justify-content: center;
  background: var(--wp-color-bg-raised); color: var(--wp-color-text-muted);
  border: var(--wp-size-border) solid var(--wp-color-border-subtle); cursor: pointer; }
.seite { flex: 1; min-height: 0; display: flex; }
.fuss__zahl { font-family: var(--wp-font-family-mono); }
/* Meldung ueber der Ansicht, nicht im Weg: unten rechts, ueber der Fusszeile. */
.toast { position: absolute; right: var(--wp-space-4); bottom: 44px; min-width: 280px; max-width: 460px; z-index: 20; }

/* Kompakte Leiste: nur Symbole, Breite wie ein Symbol plus Rand. */
.leiste.is-compact { width: 58px; }
.leiste.is-compact .marke__wort { display: none; }
.leiste.is-compact .marke { justify-content: center; padding: var(--wp-space-3) 0; }
</style>
