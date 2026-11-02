<script setup lang="ts">
// Clients: die Übersicht, welches Spiel wo liegt. Je Client die Herkunft
// (Steam, Faugus, manuell …), das Präfix und die Spielordner mit Interface,
// Addon-Zahl und belegtem Platz.
//
// Das ist die Ansicht zum Nachsehen und Aufräumen; die Arbeit (Addons
// durchsuchen) passiert im Katalog und unter "Installiert".
import { computed } from "vue";
import * as UI from "@weppelmann/components";
import { bytes, lastScan, rescan, state } from "../store";

const gesamt = computed(() => state.clients.reduce(
  (summe, c) => summe + c.flavors.reduce((s, f) => s + f.size_bytes, 0), 0));
</script>

<template>
  <div class="cl">
    <!-- Titel und Untertitel stehen im Kopfbereich des Fensters (App.vue). -->
    <header class="cl__kopf">
      <p class="cl__sub">
        {{ state.clients.length }} gefunden · {{ state.addonCount }} Addons · {{ bytes(gesamt) }} belegt
        <template v-if="lastScan"> · zuletzt {{ lastScan.toLocaleTimeString("de-DE") }}</template>
      </p>
      <UI.WpButton label="Neu scannen" size="sm" :state="state.loading ? 'busy' : 'default'" @press="rescan" />
    </header>

    <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading"
                     title="Kein Client gefunden"
                     text="Hexwerk sucht in Steam-Bibliotheken, Wine-Prefixes (Faugus, Heroic, Lutris, Bottles) und nach Ordnern mit World of Warcraft." />

    <div class="cl__liste">
      <article v-for="c in state.clients" :key="c.id" class="karte">
        <div class="karte__kopf">
          <h3 class="karte__name">{{ c.label }}</h3>
          <UI.WpBadge tone="neutral" :label="c.origin" />
        </div>

        <dl class="karte__fakten">
          <dt>Präfix</dt><dd :title="c.prefix">{{ c.prefix }}</dd>
          <dt>Spielordner</dt><dd :title="c.game_dir">{{ c.game_dir }}</dd>
        </dl>

        <table class="ordner">
          <thead>
            <tr><th>Ordner</th><th class="r">Interface</th><th class="r">Addons</th><th class="r">Platz</th></tr>
          </thead>
          <tbody>
            <tr v-for="f in c.flavors" :key="f.dir_name">
              <td>
                <span class="ordner__name">{{ f.label }}</span>
                <span class="ordner__pfad" :title="f.addons_dir">{{ f.addons_dir }}</span>
              </td>
              <td class="r mono">{{ f.interface.join(", ") || "—" }}</td>
              <td class="r mono">{{ f.addons.length }}</td>
              <td class="r mono">{{ bytes(f.size_bytes) }}</td>
            </tr>
          </tbody>
        </table>
      </article>
    </div>
  </div>
</template>

<style scoped>
.cl { flex: 1; overflow: auto; padding: var(--wp-space-4); display: flex; flex-direction: column; gap: var(--wp-space-3); }
.cl__kopf { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); }
.cl__titel { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-xl); }
.cl__sub { margin: 2px 0 0; font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.cl__liste { display: flex; flex-direction: column; gap: var(--wp-space-3); }
.karte { padding: var(--wp-space-3); background: var(--wp-color-bg-panel); border: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.karte__kopf { display: flex; align-items: center; gap: var(--wp-space-2); margin-bottom: var(--wp-space-2); }
.karte__name { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-lg); }
.karte__fakten { display: grid; grid-template-columns: 110px 1fr; gap: 2px var(--wp-space-2); margin: 0 0 var(--wp-space-2); font-size: var(--wp-font-size-xs); }
.karte__fakten dt { color: var(--wp-color-text-faint); }
.karte__fakten dd { margin: 0; font-family: var(--wp-font-family-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ordner { width: 100%; border-collapse: collapse; font-size: var(--wp-font-size-xs); }
.ordner th { text-align: left; padding: 4px var(--wp-space-2); color: var(--wp-color-text-faint); font-weight: normal; border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.ordner td { padding: 4px var(--wp-space-2); border-bottom: var(--wp-size-border) solid var(--wp-color-border-subtle); vertical-align: top; }
.ordner .r { text-align: right; }
.ordner .mono { font-family: var(--wp-font-family-mono); }
.ordner__name { display: block; color: var(--wp-color-text-default); }
.ordner__pfad { display: block; font-family: var(--wp-font-family-mono); color: var(--wp-color-text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 46ch; }
</style>
