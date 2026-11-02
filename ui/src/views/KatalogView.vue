<script setup lang="ts">
// Katalog: Suchfeld, zwei Filter, darunter ein Kartenraster mit zwei Spalten und
// die Zeile, die sagt, wie viel von wie viel gezeigt wird. Ein Klick öffnet die
// Detailseite.
//
// Karten statt Tabelle: eine Tabellenzeile trägt Titel und Version, aber nicht
// die Kurzbeschreibung — und die entscheidet, ob man ein Addon öffnen will.
import { computed, onMounted, ref } from "vue";
import * as UI from "@weppelmann/components";
import { katalog, ladeKatalog, oeffneDetail, quellenName, suchen, sync } from "../store";

const eingabe = ref("");
const quelle = ref("");
const version = ref("");

const quellenFilter = computed(() => [
  { value: "", label: "Alle Quellen" },
  ...(katalog.stats?.per_source ?? []).map(([name, n]) => ({ value: name, label: `${quellenName(name)} (${n})` })),
]);
const versionsFilter = [
  { value: "", label: "Alle Versionen" },
  { value: "retail", label: "Retail" },
  { value: "classic", label: "Classic" },
];

/** Treffer, nach Quelle gefiltert. */
const treffer = computed(() =>
  quelle.value ? katalog.items.filter((i) => i.latest?.source === quelle.value) : katalog.items,
);

function starte() {
  katalog.suche = eingabe.value;
  suchen(eingabe.value);
}

onMounted(() => suchen(""));
</script>

<template>
  <div class="kat">
    <div class="kat__suche">
      <UI.WpInput v-model="eingabe" placeholder="Suche: Name, Autor, Beschreibung …" @keyup.enter="starte" />
    </div>

    <div class="kat__filter">
      <UI.WpSelect v-model="quelle" :options="quellenFilter" />
      <UI.WpSelect v-model="version" :options="versionsFilter" />
      <UI.WpButton label="Katalog holen" size="sm" variant="secondary"
                   :state="sync.laeuft ? 'busy' : 'default'" @press="ladeKatalog()" />
    </div>

    <p v-if="sync.fehler" class="fehler">{{ sync.fehler }}</p>
    <p v-if="sync.bericht" class="zeile">{{ sync.bericht }}</p>

    <p class="zeile">
      {{ treffer.length }} von {{ katalog.stats?.catalogue ?? 0 }} Einträgen
      <template v-if="!eingabe"> · erste Seite, Suche verfeinern</template>
    </p>

    <UI.WpEmptyState v-if="katalog.loading" title="Wird geladen" text="Der Index wird befragt." />
    <UI.WpEmptyState v-else-if="treffer.length === 0" title="Kein Eintrag"
                     text="Kein Addon passt zu diesem Suchbegriff. Anderer Begriff, weniger Filter?" />
    <div v-else class="raster">
      <button v-for="item in treffer" :key="item.id" type="button" class="karte" @click="oeffneDetail(item)">
        <span class="karte__kopf">
          <span class="karte__box" aria-hidden="true" />
          <span class="karte__titel">{{ item.name }}</span>
        </span>
        <span class="karte__meta">
          {{ item.author }} · {{ item.latest ? quellenName(item.latest.source) : "—" }}
          <template v-if="item.sources.length"> · {{ item.sources.length }} Quelle{{ item.sources.length === 1 ? "" : "n" }}</template>
        </span>
        <span class="karte__text">{{ item.summary || "Keine Angabe" }}</span>
        <span class="karte__badges">
          <span v-if="item.latest" class="badge">{{ quellenName(item.latest.source) }} {{ item.latest.version }}</span>
        </span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.kat { flex: 1; min-height: 0; overflow: auto; padding: var(--wp-space-3); display: flex; flex-direction: column; gap: var(--wp-space-2); }
.kat__suche :deep(.wp-input) { width: 100%; }
.kat__filter { display: flex; gap: var(--wp-space-2); align-items: center; }
.kat__filter > :deep(.wp-select) { min-width: 190px; }
.zeile { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.fehler { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }

.raster { display: grid; grid-template-columns: 1fr 1fr; gap: var(--wp-space-3); }
.karte { display: flex; flex-direction: column; gap: 4px; text-align: left; padding: var(--wp-space-3);
  background: var(--wp-color-bg-panel); border: var(--wp-size-border) solid var(--wp-color-border-subtle);
  color: inherit; font: inherit; }
.karte:hover { border-color: var(--wp-color-border-strong); }
.karte__kopf { display: flex; align-items: center; gap: var(--wp-space-2); }
.karte__box { width: 16px; height: 16px; flex: none; border: var(--wp-size-border) solid var(--wp-color-border-strong);
  background: var(--wp-color-bg-input); }
.karte__titel { font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-md);
  font-weight: var(--wp-font-weight-semibold); }
.karte__meta { font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.karte__text { font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); line-height: 1.45; }
.karte__badges { display: flex; gap: 4px; margin-top: 2px; }
.badge { padding: 1px 6px; font-family: var(--wp-font-family-mono); font-size: 10px;
  color: var(--wp-color-text-muted); background: var(--wp-color-bg-raised);
  border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
</style>
