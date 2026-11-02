<script setup lang="ts">
// Katalog: Suchfeld, Filter zum Mehrfachwählen, darunter ein Kartenraster und
// die Zeile, die sagt, wie viel von wie viel gezeigt wird.
//
// Die Filter sind Kästchen, keine Auswahlfelder: mehrere Quellen und mehrere
// Interface-Nummern gleichzeitig sind der Normalfall ("Retail oder Classic").
import { computed, onMounted, reactive, ref } from "vue";
import * as UI from "@weppelmann/components";
import { t } from "../i18n";
import { katalog, oeffneDetail, quellenName, suchen, sync } from "../store";

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
      <UI.WpFieldset :title="t('cv.sources.hint')">
        <div class="kasten">
          <UI.WpCheckbox v-for="q in quellen" :key="q[0]" :model-value="gewaehlteQuellen.has(q[0])"
                         :label="`${quellenName(q[0])} (${q[1]})`"
                         @update:model-value="schalteQuelle(q[0])" />
        </div>
      </UI.WpFieldset>
      <UI.WpFieldset v-if="versionen.length" :title="t('cv.version.hint')">
        <div class="kasten">
          <UI.WpCheckbox v-for="v in versionen" :key="v[0]" :model-value="gewaehlteVersionen.has(v[0])"
                        :label="`${v[0]} (${v[1]})`" @update:model-value="schalteVersion(v[0])" />
        </div>
      </UI.WpFieldset>
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
      <UI.WpChoiceCard v-for="item in treffer" :key="item.id" :title="item.name"
                       :meta="`${item.author || t('cv.unspecified')} · ${item.latest ? quellenName(item.latest.source) : t('cv.no.source')}`"
                       :note="item.summary || t('dt.hint.no.summary')"
                       @press="oeffneDetail(item)">
        <template #trailing>
          <span class="badges">
            <UI.WpBadge v-if="item.latest" tone="neutral" :label="`${quellenName(item.latest.source)} ${item.latest.version}`" />
            <UI.WpBadge v-if="item.installed.length" tone="ok" :label="t('cv.installed', item.installed[0].version ?? '?')" />
          </span>
        </template>
      </UI.WpChoiceCard>
    </div>
  </div>
</template>

<style scoped>
.kat { flex: 1; min-height: 0; overflow: auto; padding: var(--wp-space-3); display: flex; flex-direction: column; gap: var(--wp-space-2); }
.kat__suche :deep(.wp-input) { width: 100%; }
.kat__filter { display: flex; align-items: flex-start; gap: var(--wp-space-4); flex-wrap: wrap; }
.kasten { display: flex; gap: var(--wp-space-3); flex-wrap: wrap; }
.zeile { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.fehler { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.raster { display: grid; grid-template-columns: 1fr 1fr; gap: var(--wp-space-3); }
.karte:hover { border-color: var(--wp-color-border-strong); }
.badges { display: flex; flex-direction: column; gap: 2px; align-items: flex-end; max-width: 100%; }
.badges :deep(.wp-badge) { max-width: 100%; overflow: hidden; text-overflow: ellipsis; display: block; }
</style>
