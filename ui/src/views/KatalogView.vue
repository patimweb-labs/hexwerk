<script setup lang="ts">
// Katalog: suchen, Ergebnisliste links, Detail rechts. Die Daten kommen aus dem
// Index (SQLite mit Volltextsuche); hier wird nur gesucht und angezeigt.
import { computed, onMounted, ref } from "vue";
import * as UI from "@weppelmann/components";
import { IconSearch } from "@weppelmann/icons";
import { bytes, gewaehlt, katalog, suchen } from "../store";

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

const detail = computed(() => gewaehlt());

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
      <span class="kat__stats" v-if="katalog.stats">
        {{ katalog.stats.catalogue }} im Katalog · {{ katalog.stats.installed }} installiert ·
        {{ katalog.stats.updates }} Updates
      </span>
    </div>

    <div class="kat__body">
      <div class="kat__liste">
        <UI.WpTable :columns="spalten" :state="tabelle" caption="Katalog"
                    @row-click="(i: number) => (katalog.gewaehlt = i)" />
        <div class="chips">
          <button v-for="(s, i) in katalog.stats?.per_source ?? []" :key="s[0]" type="button"
                  class="chip" @click="eingabe = ''; suchen('')">{{ s[0] }} {{ s[1] }}</button>
        </div>
      </div>

      <aside class="kat__detail">
        <template v-if="detail">
          <h2 class="titel">{{ detail.name }}</h2>
          <p class="autor">{{ detail.author }}<span v-if="detail.category"> · {{ detail.category }}</span></p>
          <p class="text">{{ detail.summary }}</p>

          <dl class="fakten">
            <dt>Neueste</dt><dd>{{ detail.latest?.version ?? "—" }}</dd>
            <dt>Quelle</dt><dd>{{ detail.latest?.source ?? "—" }}</dd>
            <dt>Groesse</dt><dd>{{ detail.latest?.size_bytes ? bytes(detail.latest.size_bytes) : "—" }}</dd>
            <dt>Interfaces</dt><dd>{{ detail.latest?.interfaces.join(", ") || "—" }}</dd>
          </dl>

          <p class="unter">Quellen</p>
          <ul class="liste">
            <li v-for="s in detail.sources" :key="s.source">
              <span class="quelle">{{ s.source }}</span>
              <span class="version">{{ s.version }}</span>
            </li>
          </ul>

          <p class="unter">Installiert</p>
          <ul class="liste" v-if="detail.installed.length">
            <li v-for="i in detail.installed" :key="i.folder">
              <span class="quelle">{{ i.client_label }} · {{ i.flavor }}</span>
              <span class="version">{{ i.version ?? "?" }}<template v-if="i.update"> → {{ i.update }}</template></span>
            </li>
          </ul>
          <p v-else class="leer">Nirgends installiert.</p>
        </template>
        <p v-else class="leer">Eintrag links waehlen.</p>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.kat { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--wp-space-2); padding: var(--wp-space-3); }
.kat__suche { display: flex; align-items: center; gap: var(--wp-space-2); }
.kat__suche :deep(.wp-input) { flex: 1; }
.kat__stats { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.kat__body { flex: 1; min-height: 0; display: flex; gap: var(--wp-space-3); }
.kat__liste { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--wp-space-2); overflow: auto; }
.kat__detail { width: 340px; flex: none; overflow: auto; padding: var(--wp-space-3);
  background: var(--wp-color-bg-panel); border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.chips { display: flex; gap: 4px; flex-wrap: wrap; }
.chip { padding: 2px var(--wp-space-2); font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs);
  background: var(--wp-color-bg-raised); border: var(--wp-size-border) solid var(--wp-color-border-subtle);
  color: var(--wp-color-text-muted); }
.titel { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-lg); }
.autor { margin: 2px 0 var(--wp-space-2); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.text { margin: 0 0 var(--wp-space-3); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.fakten { display: grid; grid-template-columns: 90px 1fr; gap: 2px var(--wp-space-2); margin: 0 0 var(--wp-space-3); font-size: var(--wp-font-size-xs); }
.fakten dt { color: var(--wp-color-text-faint); }
.fakten dd { margin: 0; font-family: var(--wp-font-family-mono); }
.unter { margin: var(--wp-space-3) 0 var(--wp-space-1); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); text-transform: uppercase; letter-spacing: 0.06em; }
.liste { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
.liste li { display: flex; justify-content: space-between; gap: var(--wp-space-2); font-size: var(--wp-font-size-xs); }
.quelle { color: var(--wp-color-text-muted); }
.version { font-family: var(--wp-font-family-mono); }
.leer { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
</style>
