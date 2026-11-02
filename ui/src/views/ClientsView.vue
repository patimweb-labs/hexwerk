<script setup lang="ts">
// Clients: die Übersicht, welches Spiel wo liegt. Je Client die Herkunft
// (Steam, Faugus, manuell …), das Präfix und die Spielordner mit Interface,
// Addon-Zahl und belegtem Platz.
//
// Das ist die Ansicht zum Nachsehen und Aufräumen; die Arbeit (Addons
// durchsuchen) passiert im Katalog und unter "Installiert".
import { computed } from "vue";
import * as UI from "@weppelmann/components";
import { t } from "../i18n";
import { bytes, lastScan, rescan, state } from "../store";

const gesamt = computed(() => state.clients.reduce(
  (summe, c) => summe + c.flavors.reduce((s, f) => s + f.size_bytes, 0), 0));
</script>

<template>
  <div class="cl">
    <!-- Titel und Untertitel stehen im Kopfbereich des Fensters (App.vue). -->
    <header class="cl__kopf">
      <p class="cl__sub">{{ t("cl.sub") }}</p>
    </header>

    <UI.WpEmptyState v-if="state.clients.length === 0 && !state.loading"
                     :title="t('cl.none.title')" :text="t('cl.none.text')" />

    <div class="cl__liste">
      <UI.WpCard v-for="c in state.clients" :key="c.id" :title="c.label" class="karte" flush>
        <template #actions>
          <UI.WpBadge tone="neutral" :label="c.origin.toUpperCase()" />
        </template>
        <p class="karte__pfad" :title="c.prefix">~{{ c.prefix.replace(/^\/home\/[^/]+/, "") }}</p>
        <div v-for="f in c.flavors" :key="f.dir_name" class="ordner">
          <span class="ordner__name">{{ f.dir_name }}</span>
          <span class="ordner__iface">{{ f.interface.join(" · ") || "—" }}</span>
          <span class="ordner__raum" />
          <UI.WpBadge tone="ok" :label="f.addons.length === 1 ? t('cl.addon.one') : t('cl.addons', f.addons.length)" />
          <span class="ordner__size">{{ bytes(f.size_bytes) }}</span>
        </div>
      </UI.WpCard>
    </div>

    <!-- Was sonst noch gesucht wird; findet eine Herkunft nichts, fehlt sie hier. -->
    <section class="hinweis">
      <h3 class="hinweis__titel">{{ t("cl.more.title") }}</h3>
      <p class="hinweis__text">{{ t("cl.more.text") }}</p>
    </section>

    <footer class="cl__fuss">
      <span class="cl__zaehler">
        {{ t("st.intros", state.notes.length, state.clients.length, state.addonCount) }}
      </span>
      <UI.WpButton :label="t('cl.rescan')" size="sm" :state="state.loading ? 'busy' : 'default'" @press="rescan" />
    </footer>
  </div>
</template>

<style scoped>
.cl { flex: 1; overflow: auto; padding: var(--wp-space-4); display: flex; flex-direction: column; gap: var(--wp-space-3); }
.cl__kopf { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); }
.cl__titel { margin: 0; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-xl); }
.cl__sub { margin: 2px 0 0; font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.cl__liste { display: flex; flex-direction: column; gap: var(--wp-space-3); }
.karte__pfad { margin: 0 0 var(--wp-space-2); font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint); }
.hinweis { padding: var(--wp-space-3); border: var(--wp-size-border) dashed var(--wp-color-border-subtle); }
.hinweis__titel { margin: 0 0 4px; font-family: var(--wp-font-family-display); font-size: var(--wp-font-size-sm); }
.hinweis__text { margin: 0; font-size: var(--wp-font-size-xs); color: var(--wp-color-text-muted); }
.hinweis code { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-default); }
.cl__fuss { display: flex; align-items: center; justify-content: space-between; gap: var(--wp-space-3); margin-top: auto; padding-top: var(--wp-space-3); }
.cl__zaehler { font-family: var(--wp-font-family-mono); font-size: var(--wp-font-size-sm); color: var(--wp-color-text-muted); }
.karte__pfad { margin: 0 0 var(--wp-space-2); font-family: var(--wp-font-family-mono);
  font-size: var(--wp-font-size-xs); color: var(--wp-color-text-faint);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
/* Eine Zeile je Spielordner: Name, Interface-Nummern, Anzahl, Groesse. */
.ordner { display: flex; align-items: center; gap: var(--wp-space-2);
  padding: 3px var(--wp-space-2); font-size: var(--wp-font-size-xs);
  border-top: var(--wp-size-border) solid var(--wp-color-border-subtle); }
.ordner__name { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-muted); }
.ordner__iface { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-faint); }
.ordner__raum { flex: 1; }
.ordner__size { font-family: var(--wp-font-family-mono); color: var(--wp-color-text-faint); }
</style>
