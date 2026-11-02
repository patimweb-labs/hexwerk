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
