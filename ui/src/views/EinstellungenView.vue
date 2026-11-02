<script setup lang="ts">
// Einstellungen: Sprache, Farbschema und der Katalogdienst.
//
// Die Quellen werden zentral über den Dienst gesteuert — die App fragt nur
// Manifest und Katalog ab. Deshalb steht hier die Adresse des Dienstes und der
// Schlüssel, keine Rangfolge einzelner Märkte.
import { computed, onMounted } from "vue";
import * as UI from "@weppelmann/components";
import { einstellungen, ladeEinstellungen, speichereEinstellungen } from "../store";

const sprachen = [
  { value: "de", label: "Deutsch" },
  { value: "en", label: "English" },
  { value: "es", label: "Español" },
  { value: "fr", label: "Français" },
];
const themen = [
  { value: "dark", label: "Dunkel" },
  { value: "light", label: "Hell" },
  { value: "mono", label: "Mono" },
];

const werte = computed(() => einstellungen.werte);

const adresse = computed({
  get: () => werte.value?.urls?.catalog ?? "",
  set: (v: string) => { if (werte.value) werte.value.urls.catalog = v; },
});
const schluessel = computed({
  get: () => werte.value?.keys?.catalog ?? "",
  set: (v: string) => { if (werte.value) werte.value.keys.catalog = v; },
});

onMounted(ladeEinstellungen);
</script>

<template>
  <div class="set" v-if="werte">
    <section class="block">
      <p class="feld__label">Sprache</p>
      <div class="knopfreihe">
        <button v-for="s in sprachen" :key="s.value" type="button" class="knopf"
                :class="{ 'is-active': werte.language === s.value }" @click="werte.language = s.value">
          {{ s.label }}
        </button>
      </div>

      <p class="feld__label">Aussehen</p>
      <div class="knopfreihe">
        <button v-for="t in themen" :key="t.value" type="button" class="knopf"
                :class="{ 'is-active': werte.theme === t.value }" @click="werte.theme = t.value">
          {{ t.label }}
        </button>
      </div>
    </section>

    <section class="block">
      <h3>Katalogdienst</h3>
      <p class="hint">
        Der Katalog kommt als Ganzes von diesem Dienst; die App hält keinen
        Schlüssel für die einzelnen Märkte. Leer lassen heißt: die eingebaute
        Adresse verwenden.
      </p>
      <div class="feld">
        <span class="feld__label">Adresse</span>
        <UI.WpInput v-model="adresse" placeholder="https://hexwerk-api.weppelmann.eu" />
      </div>
      <div class="feld">
        <span class="feld__label">Schlüssel</span>
        <UI.WpInput v-model="schluessel" placeholder="Schlüssel" />
      </div>
    </section>

    <section class="block">
      <h3>Ablage</h3>
      <dl class="fakten">
        <dt>Index</dt><dd>~/.local/share/hexwerk/index.db</dd>
        <dt>Einstellungen</dt><dd>~/.config/hexwerk/settings.json</dd>
      </dl>
      <p class="hint">Dieselbe Datei wie bei der alten Oberfläche — ein Wechsel setzt nichts zurück.</p>
    </section>

    <div class="fuss">
      <UI.WpButton label="Speichern" variant="primary" @press="speichereEinstellungen" />
      <span v-if="einstellungen.gespeichert" class="ok">gespeichert</span>
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
.fakten { display: grid; grid-template-columns: 120px 1fr; gap: 2px var(--wp-space-2); margin: 0; font-size: var(--wp-font-size-xs); }
.fakten dt { color: var(--wp-color-text-faint); }
.fakten dd { margin: 0; font-family: var(--wp-font-family-mono); }
.fuss { display: flex; align-items: center; gap: var(--wp-space-2); }
.ok { font-size: var(--wp-font-size-xs); color: var(--wp-color-status-ok); }
.fehler { font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.laedt { padding: var(--wp-space-4); color: var(--wp-color-text-faint); }
</style>
