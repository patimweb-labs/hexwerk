<script setup lang="ts">
// Detailseite: ein Addon in voller Breite. Erreichbar aus Katalog und
// Installiert; der Rueckweg steht in den Brotkrumen im Kopfbereich.
import { computed, ref } from "vue";
import * as UI from "@weppelmann/components";
import { bytes, gewaehlt, installiere, installation, ziele } from "../store";

const item = computed(() => gewaehlt());
const zielWahl = ref("");
const zielOptionen = computed(() => ziele().map((z) => ({ value: z.addonsDir, label: z.label })));
</script>

<template>
  <div class="dt" v-if="item">
    <div class="dt__kopf">
      <div>
        <h2 class="dt__name">{{ item.name }}</h2>
        <p class="dt__autor">{{ item.author }}<span v-if="item.category"> · {{ item.category }}</span></p>
      </div>
      <div class="dt__ziel">
        <UI.WpSelect v-model="zielWahl" :options="zielOptionen" label="Ziel" />
        <UI.WpButton label="Installieren" variant="primary"
                     :state="installation.laeuft ? 'busy' : item.latest ? 'default' : 'disabled'"
                     @press="installiere(item, zielWahl)" />
      </div>
    </div>

    <p v-if="installation.fehler" class="dt__fehler">{{ installation.fehler }}</p>
    <p v-if="installation.erfolg" class="dt__erfolg">installiert: {{ installation.erfolg }}</p>

    <p class="dt__text">{{ item.summary }}</p>

    <div class="dt__spalten">
      <section>
        <h3 class="dt__unter">Quellen</h3>
        <table class="dt__tabelle">
          <thead><tr><th>Quelle</th><th class="r">Version</th><th class="r">Groesse</th><th class="r">Interfaces</th></tr></thead>
          <tbody>
            <tr v-for="s in item.sources" :key="s.source">
              <td class="mono">{{ s.source }}</td>
              <td class="r mono">{{ s.version }}</td>
              <td class="r mono">{{ s.size_bytes ? bytes(s.size_bytes) : "—" }}</td>
              <td class="r mono">{{ s.interfaces.join(", ") || "—" }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section>
        <h3 class="dt__unter">Installiert</h3>
        <table class="dt__tabelle" v-if="item.installed.length">
          <thead><tr><th>Client</th><th>Ordner</th><th class="r">Version</th></tr></thead>
          <tbody>
            <tr v-for="i in item.installed" :key="i.folder">
              <td>{{ i.client_label }} · {{ i.flavor }}</td>
              <td class="mono">{{ i.folder }}</td>
              <td class="r mono">{{ i.version ?? "?" }}<template v-if="i.update"> → {{ i.update }}</template></td>
            </tr>
          </tbody>
        </table>
        <p v-else class="dt__leer">Nirgends installiert.</p>
      </section>
    </div>

    <p v-if="item.homepage" class="dt__home">
      <a :href="item.homepage" target="_blank" rel="noreferrer">{{ item.homepage }}</a>
    </p>
  </div>
  <p v-else class="dt__leer">Kein Eintrag gewählt.</p>
</template>

<style scoped>
.dt { flex: 1; overflow: auto; padding: var(--wp-space-4); display: flex; flex-direction: column; gap: var(--wp-space-3); }
.dt__kopf { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--wp-space-4); }
.dt__name { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-xl); }
.dt__autor { margin: 2px 0 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.dt__ziel { display: flex; align-items: flex-end; gap: var(--wp-space-2); min-width: 320px; }
.dt__ziel :deep(.wp-formfield) { flex: 1; }
.dt__text { margin: 0; max-width: 80ch; font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.dt__spalten { display: grid; grid-template-columns: 1fr 1fr; gap: var(--wp-space-4); }
.dt__unter { margin: 0 0 var(--wp-space-1); font-size: var(--wp-font-size-xs); text-transform: uppercase;
  letter-spacing: 0.06em; color: var(--wp-color-text-faint); }
.dt__tabelle { width: 100%; border-collapse: collapse; font-size: var(--wp-font-size-xs); }
.dt__tabelle th { text-align: left; padding: 4px var(--wp-space-2); color: var(--wp-color-text-faint);
  font-weight: normal; border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.dt__tabelle td { padding: 4px var(--wp-space-2); border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.dt__tabelle .r { text-align: right; }
.dt__tabelle .mono { font-family: var(--wp-font-family-mono); }
.dt__leer { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.dt__fehler { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-error); }
.dt__erfolg { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-status-ok); }
.dt__home { margin: 0; font-size: var(--wp-font-size-xs); }
.dt__home a { color: var(--wp-color-accent-default); }
</style>
