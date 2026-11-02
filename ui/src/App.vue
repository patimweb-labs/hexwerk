<script setup lang="ts">
// Fensteraufbau wie in der GTK-Fassung: Leiste links (Marke, Ansichten,
// Quellen), rechts Kopfbereich mit Titel, darunter der Bildschirm, unten die
// Fusszeile mit Zustand und Zahlen.
//
// Die Detailseite haengt an einer Auswahl, nicht an der Leiste — sie wird aus
// Katalog oder Installiert geoeffnet und ueber die Brotkrumen verlassen.
import { computed, onMounted, ref } from "vue";
import * as UI from "@weppelmann/components";
import { IconBrand, IconFolder, IconMoon, IconPlug, IconSearch, IconSettings, IconSun } from "@weppelmann/icons";
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
  { id: "clients", titel: t("nav.clients"), untertitel: t("page.clients.sub"), icon: IconPlug },
  { id: "installiert", titel: t("nav.installed"), untertitel: t("page.installed.sub"), icon: IconFolder },
  { id: "katalog", titel: t("nav.catalog"), untertitel: t("page.catalog.sub"), icon: IconSearch },
  { id: "einstellungen", titel: t("nav.settings"), untertitel: t("page.settings.sub"), icon: IconSettings },
] as const);

const kopf = computed(() => {
  if (ansicht.value === "detail") {
    return { titel: gewaehlt()?.name ?? t("nav.detail"), untertitel: t("page.detail.sub") };
  }
  return seiten.value.find((s) => s.id === ansicht.value) ?? seiten.value[0];
});

/** Zaehler je Leisten-Eintrag. */
function zaehl(id: string): string {
  if (id === "clients") return String(state.clients.length);
  if (id === "installiert") return String(state.addonCount);
  if (id === "katalog") return String(katalog.stats?.catalogue ?? 0);
  return "";
}

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
const lacuft = computed(() => (installation.laeuft ? t("st.kw.install") : sync.laeuft ? t("st.kw.catalog") : t("st.kw.scan")));

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
        <UI.WpIcon :icon="IconBrand" size="lg" class="marke__zeichen" />
        <span class="marke__wort">HEXWERK</span>
      </div>

      <p class="abschnitt">{{ t("rail.view") }}</p>
      <nav class="nav">
        <button v-for="s in seiten" :key="s.id" type="button" class="nav__eintrag"
                :class="{ 'is-active': ansicht === s.id }" @click="ansicht = s.id">
          <UI.WpIcon :icon="s.icon" size="sm" class="nav__icon" />
          <span class="nav__label">{{ s.titel }}</span>
          <span class="nav__zahl">{{ zaehl(s.id) }}</span>
        </button>
      </nav>

      <p class="abschnitt">{{ t("rail.stock") }}</p>
      <div class="bestand">
        <div class="bestand__zeile"><span>{{ t("rail.addons") }}</span><span class="bestand__zahl">{{ state.addonCount }}</span></div>
        <div class="bestand__zeile"><span>{{ t("rail.origins") }}</span><span class="bestand__zahl">{{ state.notes.length }}</span></div>
      </div>

      <p class="abschnitt">{{ t("nav.sources") }}</p>
      <div class="quellen">
        <div v-for="q in quellen" :key="q.name" class="quelle">
          <span class="quelle__punkt" :class="q.ok ? 'is-ok' : 'is-off'" />
          <span class="quelle__name">{{ quellenName(q.name) }}</span>
          <span class="quelle__zahl">{{ q.anzahl }}</span>
        </div>
        <p v-if="quellen.length === 0" class="quellen__leer">{{ t("st.footer.noScan") }}</p>
      </div>
    </aside>

    <!-- ------------------------------------------------------------ Inhalt -->
    <section class="inhalt">
      <header class="kopf">
        <div class="kopf__text">
          <h1 class="kopf__titel">{{ kopf.titel }}</h1>
          <p class="kopf__unter">{{ kopf.untertitel }}</p>
        </div>

        <div v-if="ansicht === 'detail'" class="krumen">
          <button type="button" class="krumen__link" @click="ansicht = 'katalog'">Katalog</button>
          <span class="krumen__trenner">›</span>
          <span class="krumen__hier">{{ gewaehlt()?.name }}</span>
        </div>

        <button type="button" class="thema" :aria-label="thema"
                :title="thema" @click="schalteThema">
          <UI.WpIcon :icon="themaIcon" size="sm" />
        </button>
      </header>

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

      <footer class="fuss">
        <span class="fuss__marke">HEXWERK</span>
        <UI.WpSpinner v-if="beschaeftigt" size="sm" />
        <span v-if="beschaeftigt" class="fuss__zahl">{{ lacuft }}</span>
        <span class="fuss__zustand">{{ zustand }}</span>
        <span class="fuss__zahl">{{ t("st.kw.counts", katalog.stats?.catalogue ?? 0, state.addonCount, state.clients.length) }}</span>
      </footer>
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
.abschnitt { margin: var(--wp-space-3) 0 var(--wp-space-1); padding: 0 var(--wp-space-3);
  font-family: var(--wp-font-family-mono); font-size: 10px; letter-spacing: 0.12em;
  text-transform: uppercase; color: var(--wp-color-text-faint); }
.nav { display: flex; flex-direction: column; }
.nav__eintrag { display: flex; align-items: center; gap: var(--wp-space-2); padding: 7px var(--wp-space-3);
  background: none; border: 0; border-left: 2px solid transparent; color: var(--wp-color-text-muted);
  font: inherit; font-size: var(--wp-font-size-sm); text-align: left; }
.nav__eintrag.is-active { color: var(--wp-color-text-default); border-left-color: var(--wp-color-accent-default); background: var(--wp-color-bg-hover); }
.nav__icon { color: var(--wp-color-text-faint); }
.nav__label { flex: 1; }
.nav__zahl { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.bestand { display: flex; flex-direction: column; gap: 1px; padding: 0 var(--wp-space-3); }
.bestand__zeile { display: flex; justify-content: space-between; gap: var(--wp-space-2); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); }
.bestand__zahl { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-faint); }
.quellen { display: flex; flex-direction: column; gap: 1px; padding: 0 var(--wp-space-3); }
.quelle { display: flex; align-items: center; gap: var(--wp-space-2); font-size: var(--wp-font-size-xs); }
.quelle__punkt { width: 6px; height: 6px; flex: none; }
.quelle__punkt.is-ok { background: var(--wp-color-status-ok); }
.quelle__punkt.is-off { background: var(--wp-color-border-strong); }
.quelle__name { flex: 1; color: var(--wp-color-text-muted); }
.quelle__zahl { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-faint); }
.quellen__leer { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }

/* ---- Inhalt ---------------------------------------------------------- */
.inhalt { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.kopf { display: flex; align-items: center; gap: var(--wp-space-3); padding: var(--wp-space-2) var(--wp-space-4);
  border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); background: var(--wp-color-bg-panel); }
.kopf__text { flex: 1; min-width: 0; }
.kopf__titel { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-lg); font-weight: var(--wp-font-weight-semibold); }
.kopf__unter { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.krumen { display: flex; align-items: center; gap: 6px; font-size: var(--wp-font-size-sm); }
.krumen__link { background: none; border: 0; padding: 0; color: var(--wp-color-accent-default); font: inherit; }
.krumen__trenner { color: var(--wp-color-text-faint); }
.krumen__hier { color: var(--wp-color-text-default); }
.thema { width: 30px; height: 28px; display: inline-flex; align-items: center; justify-content: center;
  background: var(--wp-color-bg-raised); color: var(--wp-color-text-muted);
  border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.seite { flex: 1; min-height: 0; display: flex; }
.fuss { display: flex; align-items: center; gap: var(--wp-space-3); padding: 5px var(--wp-space-4);
  border-top: var(--wp-size-border) solid var(--wp-color-border-subtle); background: var(--wp-color-bg-panel);
  font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.fuss__marke { font-family: var(--wp-font-family-display); letter-spacing: 0.1em; color: var(--wp-color-text-muted); }
.fuss__zustand { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fuss__zahl { font-family: var(--wp-font-family-mono); }
/* Meldung ueber der Ansicht, nicht im Weg: unten rechts, ueber der Fusszeile. */
.toast { position: absolute; right: var(--wp-space-4); bottom: 44px; min-width: 280px; max-width: 460px; z-index: 20; }
.inhalt { position: relative; }

/* Kompakte Leiste: nur Symbole, Breite wie ein Symbol plus Rand. */
.leiste.is-compact { width: 58px; }
.leiste.is-compact .marke__wort,
.leiste.is-compact .nav__label,
.leiste.is-compact .nav__zahl,
.leiste.is-compact .bestand,
.leiste.is-compact .quellen,
.leiste.is-compact .abschnitt { display: none; }
.leiste.is-compact .nav__eintrag { justify-content: center; padding: 8px 0; }
.leiste.is-compact .marke { justify-content: center; padding: var(--wp-space-3) 0; }
</style>
