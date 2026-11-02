<script setup lang="ts">
// Einstellungen: Sprache, Farbschema und der Katalogdienst.
//
// Die Quellen werden zentral über den Dienst gesteuert — die App fragt nur
// Manifest und Katalog ab. Deshalb steht hier die Adresse des Dienstes und der
// Schlüssel, keine Rangfolge einzelner Märkte.
import { computed, onMounted } from "vue";
import * as UI from "@weppelmann/components";
import { SPRACHEN, t } from "../i18n";
import { einstellungen, info, ladeEinstellungen, speichereEinstellungen, stand } from "../store";

const sprachen = SPRACHEN;
const themen = [
  { value: "dark", label: "Dunkel" },
  { value: "light", label: "Hell" },
  { value: "mono", label: "Mono" },
];

const werte = computed(() => einstellungen.werte);

onMounted(ladeEinstellungen);
</script>

<template>
  <div class="set" v-if="werte">
    <section class="block">
      <p class="feld__label">{{ t("set.language") }}</p>
      <div class="knopfreihe">
        <button v-for="s in sprachen" :key="s.value" type="button" class="knopf"
                :class="{ 'is-active': werte.language === s.value }" @click="werte.language = s.value">
          {{ s.label }}
        </button>
      </div>

      <p class="feld__label">{{ t("set.appearance") }}</p>
      <div class="knopfreihe">
        <button v-for="t in themen" :key="t.value" type="button" class="knopf"
                :class="{ 'is-active': werte.theme === t.value }" @click="werte.theme = t.value">
          {{ t.label }}
        </button>
      </div>

      <label class="schalter">
        <input type="checkbox" :checked="werte.nav === 'compact'"
               @change="werte.nav = ($event.target as HTMLInputElement).checked ? 'compact' : 'full'" />
        <span>{{ t("set.nav.compact") }}</span>
      </label>
    </section>

    <section class="block">
      <h3>{{ t("set.file") }}</h3>
      <p class="pfad">{{ info.settings_pfad || "wird beim Speichern angelegt" }}</p>
    </section>

    <section class="block">
      <h3>{{ t("set.about") }}</h3>
      <dl class="fakten">
        <dt>{{ t("set.version") }}</dt><dd>{{ info.version }}</dd>
        <dt>{{ t("set.version.catalog") }}</dt><dd>{{ stand(info.catalog_version) }}</dd>
      </dl>
    </section>

    <div class="fuss">
      <UI.WpButton :label="t('set.saved')" variant="primary" @press="speichereEinstellungen" />
      <span v-if="einstellungen.gespeichert" class="ok">{{ t("set.saved") }}</span>
      <span v-if="einstellungen.fehler" class="fehler">{{ einstellungen.fehler }}</span>
    </div>
  </div>
  <p v-else class="laedt">Einstellungen werden geladen …</p>
</template>

<style scoped>
.set { flex: 1; overflow: auto; padding: var(--wp-space-4); display: flex; flex-direction: column; gap: var(--wp-space-5); max-width: 720px; }
.block h3 { margin: 0 0 var(--wp-space-1); font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-md); font-weight: var(--wp-font-weight-semibold); }
.knopfreihe { display: flex; gap: 2px; margin-bottom: var(--wp-space-2); }
.knopf { padding: 5px var(--wp-space-3); font: inherit; font-size: var(--wp-font-size-sm);
  background: var(--wp-color-bg-raised); color: var(--wp-color-text-muted);
  border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.knopf.is-active { color: var(--wp-color-bg-canvas); background: var(--wp-color-status-ok); border-color: var(--wp-color-status-ok); }
.hint { margin: 0 0 var(--wp-space-2); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); max-width: 64ch; }
.feld { display: flex; flex-direction: column; gap: 2px; margin-bottom: var(--wp-space-2); }
.feld__label { font-family: var(--wp-font-family-mono); font-size: 10px; letter-spacing: 0.08em;
  text-transform: uppercase; color: var(--wp-color-text-faint); }
.schalter { display: flex; align-items: center; gap: var(--wp-space-2); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.pfad { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); word-break: break-all; }
.fakten { display: grid; grid-template-columns: 120px 1fr; gap: 2px var(--wp-space-2); margin: 0; font-size: var(--wp-font-size-xs); }
.fakten dt { color: var(--wp-color-text-faint); }
.fakten dd { margin: 0; font-family: var(--wp-font-family-mono); }
.fuss { display: flex; align-items: center; gap: var(--wp-space-2); }
.ok { font-size: var(--wp-font-size-xs); color: var(--wp-color-status-ok); }
.fehler { font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.laedt { padding: var(--wp-space-4); color: var(--wp-color-text-faint); }
</style>
