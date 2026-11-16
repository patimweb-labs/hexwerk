// Fortschritt einer Installation als Anzeige: Wert fuer den Balken und Text.
// Rein, ohne Vue: die Ansicht rendert nur, was hier herauskommt.
import type { Fortschritt } from "./api";
import { t } from "./i18n";
import { bytes } from "./store";

export interface FortschrittAnzeige {
  /** `null` heisst unbestimmt: es laeuft, aber das Ende ist unbekannt. */
  wert: number | null;
  max: number;
  text: string;
}

export function fortschrittAnzeige(f: Fortschritt): FortschrittAnzeige {
  if (f.kind === "entpackt") return { wert: null, max: 100, text: t("dt.install.unpack") };
  // Ohne Groesse oder mit einer, die nicht stimmt (der Katalog kennt nur die
  // Groesse der Quelle), bleibt der Balken unbestimmt; gezaehlt wird trotzdem.
  if (!f.total || f.done > f.total) return { wert: null, max: 100, text: bytes(f.done) };
  return { wert: f.done, max: f.total, text: t("dt.install.progress", bytes(f.done), bytes(f.total)) };
}
