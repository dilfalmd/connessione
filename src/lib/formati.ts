import { i18n, t } from "./i18n/i18n.svelte";
import type { OverallStatus } from "./tipi";

const formattatori = new Map<string, Intl.NumberFormat>();

/** Numero con i separatori della lingua corrente (virgola in italiano, punto in inglese); `null`/`undefined` diventa «N/D». */
export function numero(v: number | null | undefined, cifre = 1): string {
  if (v === null || v === undefined || Number.isNaN(v)) return t("comune.nd");
  const chiave = `${i18n.locale}|${cifre}`;
  let f = formattatori.get(chiave);
  if (!f) {
    f = new Intl.NumberFormat(i18n.locale, { minimumFractionDigits: cifre, maximumFractionDigits: cifre });
    formattatori.set(chiave, f);
  }
  return f.format(v);
}

export function dataOra(iso: string | Date | null | undefined): string {
  if (!iso) return t("comune.nd");
  const d = typeof iso === "string" ? new Date(iso) : iso;
  if (Number.isNaN(d.getTime())) return t("comune.nd");
  return d.toLocaleString(i18n.locale, {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function ora(d: Date | null | undefined): string {
  if (!d) return t("comune.mai");
  return d.toLocaleTimeString(i18n.locale, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function etichettaVerdetto(v: OverallStatus): string {
  return t(`verdetto.${v}`);
}

/** Classe CSS di colore per un verdetto. */
export const classeVerdetto: Record<OverallStatus, string> = {
  PROTECTED: "ok",
  WARNING: "avviso",
  UNPROTECTED: "pericolo",
  ERROR: "neutro",
};

export function media(valori: number[]): number | null {
  if (valori.length === 0) return null;
  return valori.reduce((a, b) => a + b, 0) / valori.length;
}
