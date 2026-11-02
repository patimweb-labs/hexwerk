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
        Gefundene WoW-Installationen. Der Pfad gilt für alle Addon-Aktionen;
        gelesen wird nur, solange du nichts änderst.
      </p>
    </header>

    <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading"
                     title="Kein Client gefunden"
                     text="Hexwerk sucht in Steam-Bibliotheken, Wine-Prefixes (Faugus, Heroic, Lutris, Bottles) und nach Ordnern mit World of Warcraft." />

    <div class="cl__liste">
      <article v-for="c in state.clients" :key="c.id" class="karte">
        <div class="karte__kopf">
          <span class="karte__zeichen" aria-hidden="true">{{ c.label.slice(0, 1).toUpperCase() }}</span>
          <h3 class="karte__name">{{ c.label }}</h3>
          <span class="karte__spacer" />
          <UI.WpBadge tone="neutral" :label="c.origin.toUpperCase()" />
        </div>

        <p class="karte__pfad" :title="c.prefix">~{{ c.prefix.replace(/^\/home\/[^/]+/, "") }}</p>

        <div v-for="f in c.flavors" :key="f.dir_name" class="ordner">
          <span class="ordner__name">{{ f.dir_name }}</span>
          <span class="ordner__iface">{{ f.interface.join(" · ") || "—" }}</span>
          <span class="karte__spacer" />
          <UI.WpBadge tone="ok" :label="`${f.addons.length} Addon${f.addons.length === 1 ? '' : 's'}`" />
          <span class="ordner__size">{{ bytes(f.size_bytes) }}</span>
        </div>
      </article>
    </div>

    <!-- Was sonst noch gesucht wird; findet eine Herkunft nichts, fehlt sie hier. -->
    <section class="hinweis">
      <h3 class="hinweis__titel">Weitere Herkünfte</h3>
      <p class="hinweis__text">
        Lutris, Bottles, PortProton und ein beliebiger Ordner mit <code>_retail_</code> werden
        mitgesucht. Findet eine Herkunft nichts, fehlt sie hier; die Scan-Seite nennt den Grund.
      </p>
    </section>

    <footer class="cl__fuss">
      <span class="cl__zaehler">
        Herkünfte {{ state.notes.length }} · Clients {{ state.clients.length }} · Addons {{ state.addonCount }}
      </span>
      <UI.WpButton label="Erneut scannen" size="sm" :state="state.loading ? 'busy' : 'default'" @press="rescan" />
    </footer>
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
.karte__zeichen { width: 22px; height: 22px; flex: none; display: inline-flex; align-items: center; justify-content: center;
  background: var(--wp-color-bg-input); border: var(--wp-size-border) solid var(--wp-color-border-strong);
  font-family: var(--wp-font-family-mono); font-size: 11px; color: var(--wp-color-text-muted); }
.karte__spacer { flex: 1; }
.karte__pfad { margin: 0 0 var(--wp-space-2); font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.ordner { display: flex; align-items: center; gap: var(--wp-space-2); padding: 3px 0; border-top: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.ordner__iface { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.ordner__size { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); }
.hinweis { padding: var(--wp-space-3); border: var(--wp-size-border) dashed var(--wp-color-border-subtle); }
.hinweis__titel { margin: 0 0 4px; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-sm); }
.hinweis__text { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); }
.hinweis code { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-default); }
.cl__fuss { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); margin-top: auto; padding-top: var(--wp-space-3); }
.cl__zaehler { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.ordner__name { display: block; color: var(--wp-color-text-default); }
</style>
