// Ein Ort fuer den Zustand. Die Ansichten lesen hier und rufen Aktionen auf —
// keine Ansicht haelt eigene Kopien der Wahrheit.
import { reactive, ref } from "vue";
import { setSprache, sprache } from "./i18n";
import { appInfo, catalogSearch, catalogStats, catalogSync, install, scan, settingsGet, settingsSet, type Client, type Item, type Note, type ScanResult, type Stats, type Settings, type Info } from "./api";

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
  // Deutsche Schreibweise: Komma als Dezimaltrenner, KB/MB/GB gross.
  const zahl = (wert: number, stellen: number) => wert.toFixed(stellen).replace(".", ",");
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${zahl(n / 1024, 1)} KB`;
  if (n < 1024 ** 3) return `${zahl(n / 1024 / 1024, 1)} MB`;
  return `${zahl(n / 1024 ** 3, 2)} GB`;
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

// ------------------------------------------------------------- Einstellungen

export const einstellungen = reactive<{
  werte: Settings | null;
  gespeichert: boolean;
  fehler: string | null;
}>({ werte: null, gespeichert: false, fehler: null });

export const info = reactive<Info>({ version: "", settings_pfad: "", catalog_version: "" });

export async function ladeEinstellungen() {
  try {
    einstellungen.werte = await settingsGet();
    Object.assign(info, await appInfo());
    // Gespeicherte Sprache und gespeichertes Farbschema gelten ab dem ersten Bild.
    wendeSprache(einstellungen.werte.language);
    wendeThema(einstellungen.werte.theme);
  } catch (fehler) {
    einstellungen.fehler = String(fehler);
  }
}

export async function speichereEinstellungen() {
  if (!einstellungen.werte) return;
  try {
    await settingsSet(einstellungen.werte);
    wendeSprache(einstellungen.werte.language);
    wendeThema(einstellungen.werte.theme);
    einstellungen.gespeichert = true;
    einstellungen.fehler = null;
    window.setTimeout(() => (einstellungen.gespeichert = false), 2000);
  } catch (fehler) {
    einstellungen.fehler = String(fehler);
  }
}

// ------------------------------------------------------------ Katalog fuellen

export const sync = reactive<{
  laeuft: boolean;
  bericht: string;
  fehler: string | null;
}>({ laeuft: false, bericht: "", fehler: null });

/** Katalog vom Dienst holen und anzeigen. */
export async function ladeKatalog() {
  sync.laeuft = true;
  sync.fehler = null;
  sync.bericht = "";
  try {
    const e = await catalogSync();
    sync.bericht = e.neu
      ? `Katalog ${e.version} übernommen · ${e.eintraege} Einträge`
      : `Katalog ${e.version} war aktuell · ${e.eintraege} Einträge`;
    katalog.stats = await catalogStats();
    await suchen(katalog.suche);
  } catch (fehler) {
    sync.fehler = String(fehler);
  } finally {
    sync.laeuft = false;
  }
}

// ------------------------------------------------------------------ Detail

/** Katalogeintrag als eigene Seite oeffnen (wie in der GTK-Fassung). */
export function oeffneDetail(item: Item) {
  katalog.gewaehlt = katalog.items.findIndex((i) => i.id === item.id);
  ansicht.value = "detail";
}

/** Zurueck in den Katalog (Brotkrumen). */
export function schliesseDetail() {
  ansicht.value = "katalog";
}

/** Welcher Bildschirm sichtbar ist. Die Detailseite haengt an einer Auswahl,
 *  nicht an der Leiste — sie wird aus Katalog oder Installiert geoeffnet. */
export const ansicht = ref<"clients" | "installiert" | "katalog" | "detail" | "einstellungen">("clients");

/** Zaehler in der Leiste. */
export const zaehler = reactive({ clients: 0, installiert: 0, katalog: 0 });

/** Zahlen fuer Leiste und Fusszeile, ohne den Katalog zu oeffnen. */
export async function ladeStats() {
  try {
    katalog.stats = await catalogStats();
  } catch {
    /* Index noch nicht da: die Leiste zeigt dann eben nichts */
  }
}

/** Anzeigename einer Quelle: der Index speichert Kurzkennungen. */
export function quellenName(kennung: string): string {
  const namen: Record<string, string> = {
    curseforge: "CurseForge",
    wowi: "WoWInterface",
    wago: "Wago",
    tukui: "Tukui",
    github: "GitHub",
  };
  return namen[kennung] ?? kennung;
}

/** Katalogstand als Datum: der Index merkt sich einen Unix-Zeitstempel. */
export function stand(version: string): string {
  const zahl = Number(version);
  if (!Number.isFinite(zahl) || zahl < 1_000_000_000) return version || "noch nicht geholt";
  return new Date(zahl * 1000).toLocaleDateString("de-DE", { day: "2-digit", month: "2-digit", year: "numeric" });
}

// --------------------------------------------------- Sprache und Farbschema

/** Drei Themes, mehr gibt es nicht. `mono` unterscheidet Zustände über Form. */
export type Thema = "dark" | "light" | "mono";
export const thema = ref<Thema>("dark");

/** Sprache sofort anwenden: die Texte hängen an `sprache`, Vue zeichnet neu. */
export function wendeSprache(code: string) {
  setSprache(code);
  document.documentElement.lang = sprache.value;
}

/** Farbschema anwenden: die Tokens hängen an `data-theme` am Wurzelelement. */
export function wendeThema(neu: string) {
  thema.value = (["dark", "light", "mono"] as const).includes(neu as Thema) ? (neu as Thema) : "dark";
  document.documentElement.dataset.theme = thema.value;
}

/**
 * Ein Addon aus der Installiert-Liste in der Detailansicht oeffnen.
 *
 * Die Detailseite arbeitet mit einem Katalogeintrag; ein lokal installiertes
 * Addon hat den aber nicht immer. Deshalb wird im Index nach dem Titel gesucht:
 * ein Treffer oeffnet dessen Detailseite, kein Treffer meldet das ehrlich.
 */
export async function oeffneLokal(titel: string): Promise<boolean> {
  try {
    const treffer = await catalogSearch(titel, 5);
    const passend = treffer.find((i) => i.name.toLowerCase() === titel.toLowerCase()) ?? treffer[0];
    if (!passend) {
      katalog.error = "Kein Katalogeintrag zu diesem Addon.";
      return false;
    }
    katalog.items = [passend, ...katalog.items.filter((i) => i.id !== passend.id)];
    katalog.gewaehlt = 0;
    ansicht.value = "detail";
    return true;
  } catch (fehler) {
    katalog.error = String(fehler);
    return false;
  }
}
