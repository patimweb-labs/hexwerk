<script setup lang="ts">
// Detailseite: ein Addon in voller Breite — Banner und Galerie, Tabs für
// Beschreibung, Dateien, Screenshots und Quellen, daneben das Ziel zum
// Installieren.
//
// Bilder werden erst hier geholt, nicht beim Kataloglauf: im Index stehen nur
// die Adressen (so macht es auch `spawn_image_fetch` in der GTK-Fassung).
import { computed, onMounted, ref, watch } from "vue";
import * as UI from "@weppelmann/components";
import { IconChevronRight } from "@weppelmann/icons";
import { fetchImages, oeffneUrl } from "../api";
import { t } from "../i18n";
import { bytes, gewaehlt, installiere, installation, lokales, ziele } from "../store";

const item = computed(() => gewaehlt());
const lokal = computed(() => (item.value ? null : lokales.value));

const zielWahl = ref("");
const zielOptionen = computed(() => ziele().map((z) => ({ value: z.addonsDir, label: z.label })));

const reiter = computed(() => [
  { id: "desc", label: t("dt.tab.desc") },
  { id: "files", label: t("dt.tab.files") },
  { id: "shots", label: t("dt.tab.shots") },
  { id: "sources", label: t("dt.tab.sources") },
]);
const tab = ref("desc");

/** Geladene Bilder: Adresse → data-URL. */
const bilder = ref<Record<string, string>>({});
const galerieBild = ref(0);

const adressen = computed(() => item.value?.screenshots ?? []);
const galerie = computed(() => adressen.value.map((a) => bilder.value[a]).filter(Boolean));

async function ladeBilder() {
  const fehlend = adressen.value.filter((a) => !bilder.value[a]);
  if (fehlend.length === 0) return;
  const geladen = await fetchImages(fehlend);
  const neu = { ...bilder.value };
  for (const b of geladen) neu[b.url] = b.data;
  bilder.value = neu;
}

watch(item, () => {
  bilder.value = {};
  galerieBild.value = 0;
  tab.value = "desc";
  ladeBilder();
});
onMounted(ladeBilder);

function weiter(richtung: -1 | 1) {
  const n = galerie.value.length;
  if (n === 0) return;
  galerieBild.value = (galerieBild.value + richtung + n) % n;
}
</script>

<template>
  <div class="dt" v-if="item">
    <!-- Banner: erstes Bild, sonst nur der Kopf. -->
    <header class="dt__kopf">
      <div class="dt__titel">
        <h2 class="dt__name">{{ item.name }}</h2>
        <p class="dt__autor">
          {{ item.author || t("cv.unspecified") }} · {{ item.category || t("dt.no.category") }}
          <template v-if="item.latest"> · {{ t("dt.published", item.latest.released_at || t("dt.no.date")) }}</template>
        </p>
        <p class="dt__stand">
          <UI.WpBadge v-if="item.latest" tone="neutral" :label="`${item.latest.source} ${item.latest.version}`" />
          <UI.WpBadge v-if="item.installed.length" tone="ok" :label="t('cv.installed', item.installed[0].version ?? '?')" />
          <UI.WpBadge v-else tone="neutral" :label="t('dt.not.installed')" />
        </p>
      </div>

      <div class="dt__ziel">
        <UI.WpSelect v-model="zielWahl" :options="zielOptionen" :label="t('dt.install.target')" />
        <UI.WpButton :label="t('dt.install')" variant="primary"
                     :state="installation.laeuft ? 'busy' : item.latest ? 'default' : 'disabled'"
                     @press="installiere(item, zielWahl)" />
      </div>
    </header>

    <p v-if="installation.fehler" class="dt__fehler">{{ installation.fehler }}</p>
    <p v-if="installation.erfolg" class="dt__erfolg">{{ t("dt.installed") }}: {{ installation.erfolg }}</p>

    <!-- Galerie -->
    <section class="galerie" v-if="galerie.length">
      <UI.WpImageFrame class="galerie__rahmen"
                       :state="{ kind: 'bild', src: galerie[galerieBild] }"
                       :alt="t('dt.preview', item.name)"
                       :max-height="'400px'" />
      <div class="galerie__leiste" v-if="galerie.length > 1">
        <UI.WpButton :label="'‹'" size="sm" variant="secondary" :aria-label="t('dt.prev')" @press="weiter(-1)" />
        <span class="galerie__zaehler">{{ galerieBild + 1 }} / {{ galerie.length }}</span>
        <UI.WpButton :label="'›'" size="sm" variant="secondary" :aria-label="t('dt.next')" @press="weiter(1)" />
      </div>
    </section>
    <p v-else-if="item.screenshots.length" class="dt__leer">{{ t("dt.hint.loading.image") }}</p>
    <p v-else class="dt__leer">{{ t("dt.hint.no.images") }}</p>

    <!-- Reiter -->
    <div class="dt__reiter">
      <button v-for="r in reiter" :key="r.id" type="button" class="dt__reiter-knopf"
              :class="{ 'is-active': tab === r.id }" @click="tab = r.id">{{ r.label }}</button>
    </div>

    <section v-if="tab === 'desc'" class="dt__bereich">
      <p class="dt__text">{{ item.summary || t("dt.summary.fallback") }}</p>
      <dl class="dt__fakten">
        <dt>{{ t("dt.row.author") }}</dt><dd>{{ item.author || t("cv.unspecified") }}</dd>
        <dt>{{ t("dt.row.category") }}</dt><dd>{{ item.category || t("dt.no.category") }}</dd>
        <dt>{{ t("dt.row.homepage") }}</dt>
        <dd><UI.WpLink v-if="item.homepage" :href="item.homepage" external block :label="item.homepage" @open="oeffneUrl" /><span v-else>—</span></dd>
        <dt>{{ t("dt.row.interface") }}</dt><dd>{{ item.latest?.interfaces.join(", ") || "—" }}</dd>
      </dl>
    </section>

    <section v-else-if="tab === 'files'" class="dt__bereich">
      <table class="dt__tabelle" v-if="item.releases.length || item.sources.length">
        <thead>
          <tr><th>{{ t("set.col.source") }}</th><th class="r">{{ t("dt.cap.version") }}</th><th class="r">{{ t("dt.cap.size") }}</th><th class="r">{{ t("dt.cap.interface") }}</th></tr>
        </thead>
        <tbody>
          <tr v-for="r in (item.releases.length ? item.releases : item.sources)" :key="r.source + r.version">
            <td class="mono">{{ r.source }}</td>
            <td class="r mono">{{ r.version }}</td>
            <td class="r mono">{{ r.size_bytes ? bytes(r.size_bytes) : t("dt.no.size") }}</td>
            <td class="r mono">{{ r.interfaces.join(", ") || "—" }}</td>
          </tr>
        </tbody>
      </table>
      <p v-else class="dt__leer">{{ t("dt.index.no.release") }}</p>
    </section>

    <section v-else-if="tab === 'shots'" class="dt__bereich">
      <div class="bilder" v-if="galerie.length">
        <UI.WpImageFrame v-for="(bild, i) in galerie" :key="i" class="bilder__rahmen"
                         :state="{ kind: 'bild', src: bild }" :alt="t('dt.preview', item.name)"
                         :ratio="'16 / 9'" :max-height="'110px'" clickable
                         @click="galerieBild = i; tab = 'desc'" />
      </div>
      <p v-else class="dt__leer">{{ t("dt.hint.no.images") }}</p>
    </section>

    <section v-else class="dt__bereich">
      <table class="dt__tabelle" v-if="item.sources.length">
        <thead><tr><th>{{ t("set.col.source") }}</th><th class="r">{{ t("dt.cap.version") }}</th><th class="r">{{ t("dt.cap.size") }}</th></tr></thead>
        <tbody>
          <tr v-for="s in item.sources" :key="s.source">
            <td class="mono">{{ s.source }}</td>
            <td class="r mono">{{ s.version }}</td>
            <td class="r mono">{{ s.size_bytes ? bytes(s.size_bytes) : "—" }}</td>
          </tr>
        </tbody>
      </table>
      <p v-else class="dt__leer">{{ t("dt.no.source.leads") }}</p>

      <h3 class="dt__unter">{{ t("dt.installed") }}</h3>
      <table class="dt__tabelle" v-if="item.installed.length">
        <thead><tr><th>{{ t("dt.row.client") }}</th><th>{{ t("dt.row.folder") }}</th><th class="r">{{ t("dt.cap.version") }}</th></tr></thead>
        <tbody>
          <tr v-for="i in item.installed" :key="i.folder">
            <td>{{ i.client_label }} · {{ i.flavor }}</td>
            <td class="mono">{{ i.folder }}</td>
            <td class="r mono">{{ i.version ?? "?" }}<template v-if="i.update"> → {{ i.update }}</template></td>
          </tr>
        </tbody>
      </table>
      <p v-else class="dt__leer">{{ t("dt.not.installed") }}</p>
    </section>
  </div>

  <!-- Lokal installiert, aber in keinem Markt: zeigen, was da ist. -->
  <div class="dt" v-else-if="lokal">
    <header class="dt__kopf">
      <div class="dt__titel">
        <h2 class="dt__name">{{ lokal.title }}</h2>
        <p class="dt__autor">{{ t("dt.no.catalogue") }}</p>
        <p class="dt__stand">
          <UI.WpBadge tone="ok" :label="t('dt.local.installed')" />
        </p>
      </div>
    </header>
    <dl class="dt__fakten">
      <dt>{{ t("dt.cap.folder") }}</dt><dd>{{ lokal.folder }}</dd>
      <dt>{{ t("dt.cap.version") }}</dt><dd>{{ lokal.version ?? "—" }}</dd>
      <dt>{{ t("dt.cap.interface") }}</dt><dd>{{ lokal.interface.join(", ") || "—" }}</dd>
      <dt>{{ t("dt.cap.size") }}</dt><dd>{{ bytes(lokal.size_bytes) }}</dd>
    </dl>
    <p class="dt__leer">{{ t("dt.empty.text") }}</p>
  </div>

  <p v-else class="dt__leer">{{ t("dt.empty") }}</p>
</template>

<style scoped>
.dt { flex: 1; overflow: auto; padding: var(--wp-space-4); display: flex; flex-direction: column; gap: var(--wp-space-3); }
.dt__kopf { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--wp-space-4); }
.dt__titel { min-width: 0; }
.dt__name { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-xl); }
.dt__autor { margin: 2px 0 var(--wp-space-1); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.dt__stand { display: flex; gap: var(--wp-space-1); flex-wrap: wrap; margin: 0; }
.dt__ziel { display: flex; align-items: flex-end; gap: var(--wp-space-2); min-width: 320px; }
.dt__ziel :deep(.wp-formfield) { flex: 1; }

.galerie { display: flex; flex-direction: column; gap: var(--wp-space-1); }
.galerie__rahmen { max-width: 620px; }
.galerie__leiste { display: flex; align-items: center; gap: var(--wp-space-2); }
.galerie__zaehler { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }

.dt__reiter { display: flex; gap: 2px; border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.dt__reiter-knopf { padding: 6px var(--wp-space-3); background: none; border: 0; border-bottom: 2px solid transparent;
  color: var(--wp-color-text-muted); font: inherit; font-size: var(--wp-font-size-sm); }
.dt__reiter-knopf.is-active { color: var(--wp-color-text-default); border-bottom-color: var(--wp-color-accent-default); }

.dt__bereich { display: flex; flex-direction: column; gap: var(--wp-space-2); }
.dt__text { margin: 0; max-width: 85ch; font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); line-height: 1.5; }
.dt__fakten { display: grid; grid-template-columns: 130px 1fr; gap: 2px var(--wp-space-3); margin: 0; font-size: var(--wp-font-size-xs); }
.dt__fakten dt { color: var(--wp-color-text-faint); }
.dt__fakten dd { margin: 0; font-family: var(--wp-font-family-mono); overflow: hidden; text-overflow: ellipsis; }
.dt__fakten a { color: var(--wp-color-accent-default); }
.dt__tabelle { width: 100%; border-collapse: collapse; font-size: var(--wp-font-size-xs); }
.dt__tabelle th { text-align: left; padding: 4px var(--wp-space-2); color: var(--wp-color-text-faint);
  font-weight: normal; border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.dt__tabelle td { padding: 4px var(--wp-space-2); border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.dt__tabelle .r { text-align: right; }
.dt__tabelle .mono { font-family: var(--wp-font-family-mono); }
.dt__unter { margin: var(--wp-space-3) 0 var(--wp-space-1); font-size: var(--wp-font-size-xs); text-transform: uppercase; letter-spacing: 0.06em; color: var(--wp-color-text-faint); }
.bilder { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: var(--wp-space-2); }
.dt__leer { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.dt__fehler { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.dt__erfolg { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-ok); }
</style>
