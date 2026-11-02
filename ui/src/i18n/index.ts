// Sprache der Oberfläche: Deutsch, Englisch, Spanisch, Französisch.
//
// Die Texte stehen in `texte.json` und stammen aus `crates/ui-gtk/src/lang.rs` —
// nicht abgeschrieben, sondern aus der Tabelle dort erzeugt. Eine Sprache im
// Code und eine in einer Datei wären zwei Wahrheiten.
//
// Platzhalter sind `{}` und werden der Reihe nach gefüllt, wie in der
// GTK-Fassung (`t("cv.no.hit").replacen("{}", …)`).
import { ref } from "vue";
import texte from "./texte.json";

export type Sprache = "de" | "en" | "es" | "fr";

export const SPRACHEN: { value: Sprache; label: string }[] = [
  { value: "de", label: "Deutsch" },
  { value: "en", label: "English" },
  { value: "es", label: "Español" },
  { value: "fr", label: "Français" },
];

/** Die gerade gewählte Sprache. Reaktiv, damit die Oberfläche sofort umschaltet. */
export const sprache = ref<Sprache>("de");

type Tabelle = Record<string, Partial<Record<Sprache, string>>>;
const tabelle = texte as Tabelle;

export function setSprache(neu: string) {
  sprache.value = (["de", "en", "es", "fr"] as const).includes(neu as Sprache)
    ? (neu as Sprache)
    : "de";
}

/**
 * Text zu einem Schlüssel. Fehlt der Schlüssel in der gewählten Sprache, gilt
 * Deutsch; fehlt er ganz, steht der Schlüssel da — sichtbar, nicht leer.
 */
export function t(key: string, ...werte: (string | number)[]): string {
  const eintrag = tabelle[key];
  if (!eintrag) return key;
  let text = eintrag[sprache.value] ?? eintrag.de ?? key;
  for (const wert of werte) {
    text = text.replace("{}", String(wert));
  }
  return text;
}

/** Für Vorlagen: alle Schlüssel, die im Code vorkommen, ohne Umweg. */
export function hat(key: string): boolean {
  return key in tabelle;
}
