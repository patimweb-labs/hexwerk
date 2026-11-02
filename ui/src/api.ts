// Der Vertrag zum Rust-Teil. Jeder Befehl hier hat ein Gegenstueck in
// crates/desktop/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";

export interface Addon {
  folder: string;
  title: string;
  version: string | null;
  interface: number[];
  size_bytes: number;
}

export interface Flavor {
  dir_name: string;
  label: string;
  addons_dir: string;
  interface: number[];
  game_version: string;
  size_bytes: number;
  addons: Addon[];
}

export interface Client {
  id: string;
  label: string;
  origin: string;
  prefix: string;
  game_dir: string;
  flavors: Flavor[];
}

export interface Note {
  step: string;
  level: "ok" | "info" | "error";
  detail: string;
}

export interface ScanResult {
  clients: Client[];
  notes: Note[];
  addon_count: number;
}

export function scan(): Promise<ScanResult> {
  return invoke<ScanResult>("scan");
}

export interface Release {
  source: string;
  version: string;
  released_at: string | null;
  download_url: string;
  size_bytes: number | null;
  interfaces: number[];
}

export interface InstalledInfo {
  client_label: string;
  flavor: string;
  folder: string;
  name: string;
  version: string | null;
  size_bytes: number;
  update: string | null;
}

export interface Item {
  id: number;
  key: string;
  name: string;
  author: string;
  summary: string;
  category: string | null;
  homepage: string | null;
  screenshots: string[];
  latest: Release | null;
  sources: Release[];
  installed: InstalledInfo[];
  releases: Release[];
}

export interface Stats {
  catalogue: number;
  clients: number;
  installed: number;
  updates: number;
  per_source: [string, number][];
}

export function catalogStats(): Promise<Stats> {
  return invoke<Stats>("catalog_stats");
}

export function catalogSearch(query: string, limit = 120): Promise<Item[]> {
  return invoke<Item[]>("catalog_search", { query, limit });
}

export interface InstallErgebnis {
  folder: string;
  version: string | null;
}

/** Ein Release laden und in ein Addon-Verzeichnis entpacken. */
export function install(addonsDir: string, url: string): Promise<InstallErgebnis> {
  return invoke<InstallErgebnis>("install", { addonsDir, url });
}

export interface Settings {
  source_priority: string[];
  language: string;
  nav: string;
  keys: Record<string, string>;
  urls: Record<string, string>;
  theme: string;
}

export function settingsGet(): Promise<Settings> {
  return invoke<Settings>("settings_get");
}

export function settingsSet(settings: Settings): Promise<void> {
  return invoke<void>("settings_set", { settings });
}

export interface SyncErgebnis {
  version: string;
  eintraege: number;
  neu: boolean;
}

/** Katalog vom Dienst holen: Manifest prüfen, bei neuem Stand übernehmen. */
export function catalogSync(): Promise<SyncErgebnis> {
  return invoke<SyncErgebnis>("catalog_sync");
}

export interface Info {
  version: string;
  settings_pfad: string;
  catalog_version: string;
}

export function appInfo(): Promise<Info> {
  return invoke<Info>("app_info");
}

export interface Bild {
  url: string;
  data: string;
}

/** Bilder einer Detailseite laden (data-URLs, erst beim Öffnen geholt). */
export function fetchImages(urls: string[]): Promise<Bild[]> {
  return invoke<Bild[]>("fetch_images", { urls });
}

/** Einen Link im Systembrowser öffnen (target="_blank" tut es hier nicht). */
export function oeffneUrl(url: string): Promise<void> {
  return invoke<void>("oeffne_url", { url });
}
