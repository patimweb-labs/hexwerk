<script setup lang="ts">
// Einstellungen: Sprache, Farbschema, Rangfolge der Quellen. Die Datei ist
// dieselbe wie bei der GTK-Oberflaeche, ein Wechsel setzt nichts zurueck.
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

/** Quelle in der Rangfolge nach oben oder unten schieben. */
function verschiebe(index: number, richtung: -1 | 1) {
  const liste = werte.value?.source_priority;
  if (!liste) return;
  const ziel = index + richtung;
  if (ziel < 0 || ziel >= liste.length) return;
  const [eintrag] = liste.splice(index, 1);
  liste.splice(ziel, 0, eintrag);
}

onMounted(ladeEinstellungen);
</script>

<template>
  <div class="set" v-if="werte">
    <section class="block">
      <h2>Darstellung</h2>
      <div class="reihe">
        <UI.WpSelect v-model="werte.language" :options="sprachen" label="Sprache" />
        <UI.WpSelect v-model="werte.theme" :options="themen" label="Farbschema" />
      </div>
    </section>

    <section class="block">
      <h2>Rangfolge der Quellen</h2>
      <p class="hint">
        Bei mehreren Quellen fuer dasselbe Addon gewinnt die obere. Die Reihenfolge
        entscheidet auch, welche Version als „neueste" angezeigt wird.
      </p>
      <ol class="liste">
        <li v-for="(quelle, i) in werte.source_priority" :key="quelle" class="zeile">
          <span class="nummer">{{ i + 1 }}</span>
          <span class="name">{{ quelle }}</span>
          <button type="button" class="pfeil" :disabled="i === 0" @click="verschiebe(i, -1)">↑</button>
          <button type="button" class="pfeil" :disabled="i === werte.source_priority.length - 1"
                  @click="verschiebe(i, 1)">↓</button>
        </li>
      </ol>
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
.block h2 { margin: 0 0 var(--wp-space-1); font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-md); }
.reihe { display: flex; gap: var(--wp-space-3); }
.reihe > * { flex: 1; max-width: 240px; }
.hint { margin: 0 0 var(--wp-space-2); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); max-width: 60ch; }
.liste { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
.zeile { display: flex; align-items: center; gap: var(--wp-space-2); padding: var(--wp-space-1) var(--wp-space-2);
  background: var(--wp-color-bg-raised); border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.nummer { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); width: 18px; }
.name { flex: 1; font-size: var(--wp-font-size-sm); }
.pfeil { width: 26px; height: 24px; background: var(--wp-color-bg-input); color: var(--wp-color-text-muted);
  border: var(--wp-size-border) solid var(--wp-color-border-subtle); font: inherit; }
.pfeil:disabled { opacity: 0.4; cursor: not-allowed; }
.fuss { display: flex; align-items: center; gap: var(--wp-space-2); }
.ok { font-size: var(--wp-font-size-xs); color: var(--wp-color-status-ok); }
.fehler { font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.laedt { padding: var(--wp-space-4); color: var(--wp-color-text-faint); }
</style>
