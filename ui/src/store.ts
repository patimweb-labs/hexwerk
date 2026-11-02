// Ein Ort fuer den Zustand. Die Ansichten lesen hier und rufen Aktionen auf —
// keine Ansicht haelt eigene Kopien der Wahrheit.
import { reactive, ref } from "vue";
import { catalogSearch, catalogStats, install, scan, type Client, type Item, type Note, type ScanResult, type Stats } from "./api";

export const state = reactive<{
  clients: Client[];
  notes: Note[];
  addonCount: number;
  loading: boolean;
  error: string | null;
  selectedClient: number;
  selectedFlavor: number;
}>({
  clients: [],
  notes: [],
  addonCount: 0,
  loading: false,
  error: null,
  selectedClient: 0,
  selectedFlavor: 0,
});

/** Zuletzt gescannt (fuer die Fusszeile). */
export const lastScan = ref<Date | null>(null);

export async function rescan() {
  state.loading = true;
  state.error = null;
  try {
    const result: ScanResult = await scan();
    state.clients = result.clients;
    state.notes = result.notes;
    state.addonCount = result.addon_count;
    state.selectedClient = 0;
    state.selectedFlavor = 0;
    lastScan.value = new Date();
  } catch (fehler) {
    state.error = String(fehler);
  } finally {
    state.loading = false;
  }
}

export function client(): Client | undefined {
  return state.clients[state.selectedClient];
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} kB`;
  if (n < 1024 ** 3) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

// ------------------------------------------------------------------ Katalog

export const katalog = reactive<{
  suche: string;
  items: Item[];
  stats: Stats | null;
  loading: boolean;
  error: string | null;
  gewaehlt: number;
}>({
  suche: "",
  items: [],
  stats: null,
  loading: false,
  error: null,
  gewaehlt: -1,
});

/** Suche starten. Leerer Text heisst: die ersten Eintraege zeigen. */
export async function suchen(text = katalog.suche) {
  katalog.loading = true;
  katalog.error = null;
  try {
    katalog.items = await catalogSearch(text);
    if (!katalog.stats) katalog.stats = await catalogStats();
    katalog.gewaehlt = katalog.items.length ? 0 : -1;
  } catch (fehler) {
    katalog.error = String(fehler);
  } finally {
    katalog.loading = false;
  }
}

export function gewaehlt(): Item | undefined {
  return katalog.items[katalog.gewaehlt];
}

// --------------------------------------------------------------- Installieren

/** Ein Ziel, in das installiert werden kann: Client, Spielordner, Pfad. */
export interface Ziel {
  label: string;
  addonsDir: string;
  interfaces: number[];
}

export const ziele = () =>
  state.clients.flatMap((c) =>
    c.flavors.map((f) => ({
      label: `${c.label} · ${f.label}`,
      addonsDir: f.addons_dir,
      interfaces: f.interface,
    })),
  );

export const installation = reactive<{
  laeuft: boolean;
  ziel: number;
  fehler: string | null;
  erfolg: string | null;
}>({ laeuft: false, ziel: 0, fehler: null, erfolg: null });

/** Ein Release installieren und danach neu scannen. */
export async function installiere(item: Item, addonsDir: string) {
  const release = item.latest;
  if (!release) return;
  installation.laeuft = true;
  installation.fehler = null;
  installation.erfolg = null;
  try {
    const ergebnis = await install(addonsDir, release.download_url);
    installation.erfolg = `${ergebnis.folder.split("/").pop()} ${ergebnis.version ?? ""}`.trim();
    // Ein Weg in den Index: der Scan liest den Ordner neu.
    await rescan();
  } catch (fehler) {
    installation.fehler = String(fehler);
  } finally {
    installation.laeuft = false;
  }
}
