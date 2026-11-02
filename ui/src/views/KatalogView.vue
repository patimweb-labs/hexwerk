<script setup lang="ts">
// Katalog: Suchfeld, Filter zum Mehrfachwählen, darunter ein Kartenraster und
// die Zeile, die sagt, wie viel von wie viel gezeigt wird.
//
// Die Filter sind Kästchen, keine Auswahlfelder: mehrere Quellen und mehrere
// Interface-Nummern gleichzeitig sind der Normalfall ("Retail oder Classic").
import { computed, onMounted, reactive, ref } from "vue";
import * as UI from "@weppelmann/components";
import { t } from "../i18n";
import { katalog, ladeKatalog, oeffneDetail, quellenName, suchen, sync } from "../store";

const eingabe = ref("");
/** Gewählte Quellen und Interface-Nummern; leer heißt: keine Einschränkung. */
const gewaehlteQuellen = reactive(new Set<string>());
const gewaehlteVersionen = reactive(new Set<number>());

const quellen = computed(() => katalog.stats?.per_source ?? []);
const versionen = computed(() => {
  const zaehler = new Map<number, number>();
  for (const item of katalog.items) {
    for (const n of item.latest?.interfaces ?? []) zaehler.set(n, (zaehler.get(n) ?? 0) + 1);
  }
  return [...zaehler.entries()].sort((a, b) => b[1] - a[1]).slice(0, 6);
});

const treffer = computed(() =>
  katalog.items.filter((item) => {
    if (gewaehlteQuellen.size > 0 && !gewaehlteQuellen.has(item.latest?.source ?? "")) return false;
    if (gewaehlteVersionen.size > 0) {
      if (!(item.latest?.interfaces ?? []).some((n) => gewaehlteVersionen.has(n))) return false;
    }
    return true;
  }),
);

function starte() {
  katalog.suche = eingabe.value;
  suchen(eingabe.value);
}

function schalteQuelle(name: string) {
  if (gewaehlteQuellen.has(name)) gewaehlteQuellen.delete(name);
  else gewaehlteQuellen.add(name);
}

function schalteVersion(n: number) {
  if (gewaehlteVersionen.has(n)) gewaehlteVersionen.delete(n);
  else gewaehlteVersionen.add(n);
}

onMounted(() => suchen(""));
</script>

<template>
  <div class="kat">
    <div class="kat__suche">
      <UI.WpInput v-model="eingabe" :placeholder="t('cv.search.hint')" @keyup.enter="starte" />
    </div>

    <div class="kat__filter">
      <div class="gruppe">
        <span class="gruppe__titel">{{ t("cv.sources.hint") }}</span>
        <div class="kasten">
          <UI.WpCheckbox v-for="q in quellen" :key="q[0]" :model-value="gewaehlteQuellen.has(q[0])"
                         :label="`${quellenName(q[0])} (${q[1]})`"
                         @update:model-value="schalteQuelle(q[0])" />
        </div>
      </div>
      <div class="gruppe" v-if="versionen.length">
        <span class="gruppe__titel">{{ t("cv.version.hint") }}</span>
        <div class="kasten">
          <UI.WpCheckbox v-for="v in versionen" :key="v[0]" :model-value="gewaehlteVersionen.has(v[0])"
                         :label="`${v[0]} (${v[1]})`" @update:model-value="schalteVersion(v[0])" />
        </div>
      </div>
      <UI.WpButton :label="t('cv.title')" size="sm" variant="secondary"
                   :state="sync.laeuft ? 'busy' : 'default'" @press="ladeKatalog()" />
    </div>

    <p v-if="sync.fehler" class="fehler">{{ t("ct.sync.fail", sync.fehler) }}</p>
    <p v-if="sync.bericht" class="zeile">{{ sync.bericht }}</p>

    <p class="zeile">
      {{ t("cv.count", treffer.length, katalog.stats?.catalogue ?? 0) }}
      <template v-if="!eingabe"> · {{ t("cv.count.first") }}</template>
    </p>

    <UI.WpEmptyState v-if="katalog.loading" :title="t('cv.building')" text="" />
    <UI.WpEmptyState v-else-if="treffer.length === 0" :title="t('cv.empty.index')"
                     :text="t('cv.empty.filter', katalog.stats?.catalogue ?? 0)" />
    <div v-else class="raster">
      <button v-for="item in treffer" :key="item.id" type="button" class="karte" @click="oeffneDetail(item)">
        <span class="karte__kopf">
          <span class="karte__titel">{{ item.name }}</span>
        </span>
        <span class="karte__meta">
          {{ item.author || t("cv.unspecified") }} · {{ item.latest ? quellenName(item.latest.source) : t("cv.no.source") }}
          <template v-if="item.sources.length > 1"> · {{ item.sources.length }} Quellen</template>
          <template v-else-if="item.sources.length === 1"> · {{ t("cv.sources.one", quellenName(item.sources[0].source)) }}</template>
        </span>
        <span class="karte__text">{{ item.summary || t("dt.hint.no.summary") }}</span>
        <span class="karte__badges">
          <span v-if="item.latest" class="badge">{{ quellenName(item.latest.source) }} {{ item.latest.version }}</span>
          <span v-if="item.installed.length" class="badge badge--ok">{{ t("cv.installed", item.installed[0].version ?? "?") }}</span>
        </span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.kat { flex: 1; min-height: 0; overflow: auto; padding: var(--wp-space-3); display: flex; flex-direction: column; gap: var(--wp-space-2); }
.kat__suche :deep(.wp-input) { width: 100%; }
.kat__filter { display: flex; align-items: flex-start; gap: var(--wp-space-4); flex-wrap: wrap; }
.gruppe { display: flex; flex-direction: column; gap: 2px; }
.gruppe__titel { font-family: var(--wp-font-family-mono); font-size: 10px; letter-spacing: 0.08em;
  text-transform: uppercase; color: var(--wp-color-text-faint); }
.kasten { display: flex; gap: var(--wp-space-3); flex-wrap: wrap; }
.zeile { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.fehler { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.raster { display: grid; grid-template-columns: 1fr 1fr; gap: var(--wp-space-3); }
.karte { display: flex; flex-direction: column; gap: 4px; text-align: left; padding: var(--wp-space-3);
  background: var(--wp-color-bg-panel); border: var(--wp-size-border) solid var(--wp-color-border-subtle);
  color: inherit; font: inherit; }
.karte:hover { border-color: var(--wp-color-border-strong); }
.karte__kopf { display: flex; align-items: center; gap: var(--wp-space-2); }
.karte__titel { font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-md);
  font-weight: var(--wp-font-weight-semibold); }
.karte__meta { font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.karte__text { font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); line-height: 1.45; }
.karte__badges { display: flex; gap: 4px; margin-top: 2px; flex-wrap: wrap; }
.badge { padding: 1px 6px; font-family: var(--wp-font-family-mono); font-size: 10px;
  color: var(--wp-color-text-muted); background: var(--wp-color-bg-raised);
  border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.badge--ok { color: var(--wp-color-status-ok); border-color: var(--wp-color-status-ok); }
</style>
