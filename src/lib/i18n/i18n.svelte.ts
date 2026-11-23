// Traduzioni dell'interfaccia, senza librerie: `t(chiave, parametri?)` è reattiva perché legge
// `i18n.lingua`, uno stato Svelte 5. La lingua «effettiva» (it o en) la decide il backend Rust
// (la scelta «Automatica» segue la lingua di Windows) e arriva con `lingua_effettiva`.
import { en } from "./en";
import { it } from "./it";

export type Chiave = keyof typeof it;
export type LinguaEffettiva = "it" | "en";
export type Parametri = Record<string, string | number>;

const dizionari: Record<LinguaEffettiva, Record<Chiave, string>> = { it, en };

function linguaIniziale(): LinguaEffettiva {
  // Solo finché il backend non risponde: evita un lampo nella lingua sbagliata
  try {
    return navigator.language.toLowerCase().startsWith("it") ? "it" : "en";
  } catch {
    return "it";
  }
}

class StatoLingua {
  lingua = $state<LinguaEffettiva>(linguaIniziale());

  /** Codice BCP 47 per `Intl`: virgola decimale in italiano, punto in inglese. */
  locale = $derived(this.lingua === "it" ? "it-IT" : "en-GB");

  imposta(l: LinguaEffettiva) {
    this.lingua = l;
    try {
      document.documentElement.lang = l;
    } catch {
      /* nessun documento (test) */
    }
  }
}

export const i18n = new StatoLingua();

/** Testo nella lingua corrente; `{nome}` nel testo si sostituisce con `parametri.nome`. */
export function t(chiave: Chiave, parametri?: Parametri): string {
  const testo = dizionari[i18n.lingua][chiave] ?? dizionari.it[chiave];
  if (!parametri) return testo;
  return testo.replace(/\{(\w+)\}/g, (m, nome: string) => (nome in parametri ? String(parametri[nome]) : m));
}
