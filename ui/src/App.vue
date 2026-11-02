<script setup lang="ts">
// Fensteraufbau: Kopfzeile mit Umschalter, darunter die Ansicht, unten der Stand.
// Der Aufbau kommt aus weppelmann-ui; hier stehen nur Daten und Handlungen.
import { onMounted, ref } from "vue";
import * as UI from "@weppelmann/components";
import { lastScan, rescan, state } from "./store";
import InstalliertView from "./views/InstalliertView.vue";
import KatalogView from "./views/KatalogView.vue";

// Der Katalog ist die Hauptansicht: dort sucht man, installiert und
// aktualisiert. "Installiert" zeigt, was auf der Platte liegt.
const ansicht = ref<"installiert" | "katalog">("katalog");
const reiter = [
  { id: "katalog", label: "Katalog" },
  { id: "installiert", label: "Installiert" },
] as const;

onMounted(rescan);
</script>

<template>
  <div class="app">
    <UI.WpAppBar title="Hexwerk" subtitle="Addon-Verwaltung">
      <template #status>
        <UI.WpBadge v-if="state.loading" tone="warn" label="scannt" />
        <UI.WpBadge v-else-if="state.error" tone="error" label="Fehler" />
        <UI.WpBadge v-else-if="lastScan" tone="ok" :label="`${state.addonCount} Addons`" />
      </template>
      <template #actions>
        <div class="nav">
          <button v-for="r in reiter" :key="r.id" type="button" class="nav__item"
                  :class="{ 'is-active': ansicht === r.id }" @click="ansicht = r.id">
            {{ r.label }}
          </button>
        </div>
        <UI.WpButton label="Neu scannen" size="sm" :state="state.loading ? 'busy' : 'default'" @press="rescan" />
      </template>
    </UI.WpAppBar>

    <InstalliertView v-if="ansicht === 'installiert'" />
    <KatalogView v-else />

    <UI.WpTelemetryBar title="Stand"
                       :stats="lastScan ? `zuletzt ${lastScan.toLocaleTimeString('de-DE')}` : 'noch nicht gescannt'">
      <div v-for="n in state.notes.slice(0, 3)" :key="n.step" class="note">
        <span class="note__step">{{ n.step }}</span>
        <span class="note__detail">{{ n.detail }}</span>
      </div>
    </UI.WpTelemetryBar>
  </div>
</template>

<style scoped>
.app { height: 100%; display: flex; flex-direction: column; }
.nav { display: flex; gap: 2px; }
.nav__item { padding: 4px var(--wp-space-3); background: none; border: 0;
  border-bottom: 2px solid transparent; color: var(--wp-color-text-muted);
  font: inherit; font-size: var(--wp-font-size-sm); }
.nav__item.is-active { color: var(--wp-color-text-default); border-bottom-color: var(--wp-color-accent-default); }
.note { display: flex; gap: var(--wp-space-2); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.note__step { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-muted); }
</style>
