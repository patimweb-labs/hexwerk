<script setup lang="ts">
// Katalog: suchen und die Trefferliste. Ein Klick auf eine Zeile öffnet die
// Detailseite — sie ist ein eigener Bildschirm, keine Spalte.
import { computed, onMounted, ref } from "vue";
import * as UI from "@weppelmann/components";
import { IconSearch } from "@weppelmann/icons";
import { katalog, ladeKatalog, oeffneDetail, suchen, sync } from "../store";

const eingabe = ref("");
const spalten = [
  { key: "name", label: "Addon", sortable: true },
  { key: "author", label: "Autor" },
  { key: "version", label: "Version", align: "end" as const },
  { key: "stand", label: "Quelle", align: "end" as const },
];

const zeilen = computed(() =>
  katalog.items.map((i) => ({
    name: i.name,
    author: i.author,
    version: i.latest?.version ?? "—",
    stand: i.latest?.source ?? "—",
  })),
);
const tabelle = computed(() =>
  katalog.loading
    ? ({ kind: "laedt" } as const)
    : katalog.error
      ? ({ kind: "fehler", message: katalog.error } as const)
      : zeilen.value.length
        ? ({ kind: "fertig", rows: zeilen.value } as const)
        : ({ kind: "leer", text: "Kein Eintrag passt. Anderer Suchbegriff?" } as const),
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
      <UI.WpInput v-model="eingabe" placeholder="Addon suchen …" @keyup.enter="starte" />
      <UI.WpButton label="Suchen" :icon="IconSearch" size="sm" @press="starte" />
      <UI.WpButton label="Katalog holen" size="sm" variant="secondary"
                   :state="sync.laeuft ? 'busy' : 'default'" @press="ladeKatalog()" />
      <span class="kat__stats" v-if="katalog.stats">
        {{ katalog.stats.catalogue }} im Katalog · {{ katalog.stats.installed }} installiert ·
        {{ katalog.stats.updates }} Updates
      </span>
    </div>

    <p v-if="sync.fehler" class="fehler">{{ sync.fehler }}</p>
    <p v-for="q in sync.bericht" :key="q[0]" class="quelle-zeile">{{ q[0] }}: {{ q[1] }}</p>

    <div class="kat__liste">
      <UI.WpTable :columns="spalten" :state="tabelle" caption="Katalog"
                  @row-click="(i: number) => oeffneDetail(katalog.items[i])" />
      <div class="chips">
        <button v-for="s in katalog.stats?.per_source ?? []" :key="s[0]" type="button" class="chip"
                @click="eingabe = ''; suchen('')">{{ s[0] }} {{ s[1] }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.kat { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--wp-space-2); padding: var(--wp-space-3); }
.kat__suche { display: flex; align-items: center; gap: var(--wp-space-2); }
.kat__suche :deep(.wp-input) { flex: 1; }
.kat__stats { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.kat__liste { flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: var(--wp-space-2); }
.chips { display: flex; gap: 4px; flex-wrap: wrap; }
.chip { padding: 2px var(--wp-space-2); font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs);
  background: var(--wp-color-bg-raised); border: var(--wp-size-border) solid var(--wp-color-border-subtle);
  color: var(--wp-color-text-muted); }
.fehler { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.quelle-zeile { margin: 0; font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
</style>
