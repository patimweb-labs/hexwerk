// Ein Ort fuer den Zustand. Die Ansichten lesen hier und rufen Aktionen auf —
// keine Ansicht haelt eigene Kopien der Wahrheit.
import { reactive, ref } from "vue";
import { scan, type Client, type Note, type ScanResult } from "./api";

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
