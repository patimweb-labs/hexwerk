<script setup lang="ts">
// Was auf der Platte liegt: Clients links, Spielordner als Reiter, Addons als
// Tabelle. Die Daten kommen aus dem Scan, nicht aus dem Katalog.
import { computed } from "vue";
import * as UI from "@weppelmann/components";
import { bytes, client, state } from "../store";

const aktuell = computed(() => client());
const flavor = computed(() => aktuell.value?.flavors[state.selectedFlavor]);
const spalten = [
  { key: "name", label: "Addon", sortable: true },
  { key: "version", label: "Version", align: "end" as const, sortable: true },
  { key: "interface", label: "Interface", align: "end" as const },
  { key: "size", label: "Groesse", align: "end" as const },
];
const tabelle = computed(() =>
  (flavor.value?.addons ?? []).length
    ? ({ kind: "fertig", rows: (flavor.value?.addons ?? []).map((a) => ({
        name: a.title,
        version: a.version ?? "—",
        interface: a.interface.length ? a.interface[a.interface.length - 1] : "—",
        size: bytes(a.size_bytes),
      })) } as const)
    : ({ kind: "leer", text: "Keine Addons in diesem Ordner." } as const),
);
</script>

<template>
  <div class="inst">
    <UI.WpSidePanel title="Clients" :count="state.clients.length">
      <button v-for="(c, i) in state.clients" :key="c.id" type="button" class="client"
              :class="{ 'is-active': i === state.selectedClient }"
              @click="state.selectedClient = i; state.selectedFlavor = 0">
        <span class="client__label">{{ c.label }}</span>
        <span class="client__meta">{{ c.origin }} · {{ c.flavors.length }} Ordner</span>
      </button>
      <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading" title="Nichts gefunden"
                       text="Kein World-of-Warcraft-Ordner gefunden. Laeuft das Spiel ueber Steam, Faugus oder Lutris?" />
    </UI.WpSidePanel>

    <main class="inst__main">
      <div class="tabs">
        <button v-for="(f, i) in aktuell?.flavors ?? []" :key="f.dir_name" type="button" class="tab"
                :class="{ 'is-active': i === state.selectedFlavor }" @click="state.selectedFlavor = i">
          {{ f.label }} <span class="tab__count">{{ f.addons.length }}</span>
        </button>
      </div>
      <p v-if="flavor" class="pfad">{{ flavor.addons_dir }}</p>
      <UI.WpTable :columns="spalten" :state="tabelle" caption="Installierte Addons" />
    </main>
  </div>
</template>

<style scoped>
.inst { flex: 1; min-height: 0; display: flex; }
.inst__main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--wp-space-2); padding: var(--wp-space-3); overflow: auto; }
.client { display: flex; flex-direction: column; gap: 2px; text-align: left; padding: var(--wp-space-2);
  background: var(--wp-color-bg-raised); border: var(--wp-size-border) solid var(--wp-color-border-subtle); color: inherit; font: inherit; }
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
</style>
