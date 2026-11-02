<script setup lang="ts">
// Fensteraufbau: Kopfzeile, Client-Spalte, Addon-Liste, Fusszeile.
// Der Aufbau kommt aus weppelmann-ui; hier stehen nur Daten und Handlungen.
import { computed, onMounted } from "vue";
import * as UI from "@weppelmann/components";
import { bytes, client, lastScan, rescan, state } from "./store";

const aktuell = computed(() => client());
const flavor = computed(() => aktuell.value?.flavors[state.selectedFlavor]);
const zeilen = computed(() =>
  (flavor.value?.addons ?? []).map((a) => ({
    name: a.title,
    version: a.version ?? "—",
    interface: a.interface.length ? a.interface[a.interface.length - 1] : "—",
    size: bytes(a.size_bytes),
  })),
);
const columns = [
  { key: "name", label: "Addon", sortable: true },
  { key: "version", label: "Version", align: "end" as const, sortable: true },
  { key: "interface", label: "Interface", align: "end" as const },
  { key: "size", label: "Groesse", align: "end" as const },
];
const tabelle = computed(() =>
  zeilen.value.length
    ? ({ kind: "fertig", rows: zeilen.value } as const)
    : ({ kind: "leer", text: "Keine Addons in diesem Ordner." } as const),
);

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
        <UI.WpButton label="Neu scannen" size="sm" :state="state.loading ? 'busy' : 'default'" @press="rescan" />
      </template>
    </UI.WpAppBar>

    <div class="app__body">
      <UI.WpSidePanel title="Clients" :count="state.clients.length">
        <button
          v-for="(c, i) in state.clients"
          :key="c.id"
          type="button"
          class="client"
          :class="{ 'is-active': i === state.selectedClient }"
          @click="state.selectedClient = i; state.selectedFlavor = 0"
        >
          <span class="client__label">{{ c.label }}</span>
          <span class="client__meta">{{ c.origin }} · {{ c.flavors.length }} Ordner</span>
        </button>
        <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading" title="Nichts gefunden"
                         text="Kein World-of-Warcraft-Ordner gefunden. Laeuft das Spiel ueber Steam, Faugus oder Lutris?" />
      </UI.WpSidePanel>

      <main class="app__main">
        <div class="tabs">
          <button
            v-for="(f, i) in aktuell?.flavors ?? []"
            :key="f.dir_name"
            type="button"
            class="tab"
            :class="{ 'is-active': i === state.selectedFlavor }"
            @click="state.selectedFlavor = i"
          >
            {{ f.label }}
            <span class="tab__count">{{ f.addons.length }}</span>
          </button>
        </div>

        <p v-if="flavor" class="pfad">{{ flavor.addons_dir }}</p>

        <UI.WpTable :columns="columns" :state="tabelle" caption="Installierte Addons" />

        <p v-if="state.error" class="fehler">{{ state.error }}</p>
      </main>
    </div>

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
.app__body { flex: 1; min-height: 0; display: flex; }
.app__main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--wp-space-2); padding: var(--wp-space-3); overflow: auto; }
.client { display: flex; flex-direction: column; gap: 2px; text-align: left; padding: var(--wp-space-2);
  background: var(--wp-color-bg-raised); border: var(--wp-size-border) solid var(--wp-color-border-subtle);
  color: inherit; font: inherit; }
.client.is-active { border-color: var(--wp-color-accent-default); }
.client__label { font-size: var(--wp-font-size-sm); }
.client__meta { font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.tabs { display: flex; gap: 2px; }
.tab { display: inline-flex; align-items: center; gap: 6px; padding: 6px var(--wp-space-3);
  background: var(--wp-color-bg-raised); border: var(--wp-size-border) solid var(--wp-color-border-subtle);
  color: var(--wp-color-text-muted); font: inherit; font-size: var(--wp-font-size-sm); }
.tab.is-active { color: var(--wp-color-text-default); border-color: var(--wp-color-accent-default); }
.tab__count { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.pfad { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.fehler { margin: 0; color: var(--wp-color-status-error); font-size: var(--wp-font-size-sm); }
.note { display: flex; gap: var(--wp-space-2); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.note__step { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-muted); }
</style>
