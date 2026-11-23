import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { getVersion } from "@tauri-apps/api/app";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { media } from "./formati";
import { i18n, t, type LinguaEffettiva } from "./i18n/i18n.svelte";
import type {
  ControlloSicurezza,
  ErroreSpeedtest,
  EventoFase,
  GruppoVpn,
  Impostazioni,
  MisuraVelocita,
  NuovoControllo,
  ProgressoSpeedtest,
  RisultatoCheck,
  RisultatoSpeedtest,
  SecurityReport,
  ServerSpeedtest,
} from "./tipi";

export type Fase = "inattivo" | "sicurezza" | "velocita";
export type Intervallo = "7" | "30" | "90" | "tutto";
export type FiltroVpn = "tutte" | "accesa" | "spenta";

const GIORNO_MS = 86_400_000;

function descriviErrore(e: unknown): string {
  if (typeof e === "object" && e !== null && "tipo" in e) {
    const err = e as ErroreSpeedtest;
    switch (err.tipo) {
      case "CliMancante":
        return t("err.cliMancante");
      case "Annullato":
        return t("err.annullato");
      case "GiaInEsecuzione":
        return t("err.giaInEsecuzione");
      case "LicenzaNonAccettata":
        return t("err.licenza");
      default:
        return err.dettaglio;
    }
  }
  return String(e);
}

const eCliMancante = (e: unknown) =>
  typeof e === "object" && e !== null && (e as ErroreSpeedtest).tipo === "CliMancante";

export interface MediaSessione {
  timestamp: string;
  server: string | null;
  luogo: string | null;
  vpnAttiva: boolean | null;
  n: number;
  downloadMbps: number;
  uploadMbps: number;
  pingMs: number;
  jitterMs: number | null;
  packetLoss: number | null;
}

class Dashboard {
  // Sicurezza
  report = $state<SecurityReport | null>(null);
  ultimoAggiornamento = $state<Date | null>(null);
  controlli = $state<ControlloSicurezza[]>([]);
  controlloSelezionato = $state<ControlloSicurezza | null>(null);
  erroreSicurezza = $state("");

  // Velocità
  storico = $state<MisuraVelocita[]>([]);
  server = $state<ServerSpeedtest[]>([]);
  serverScelto = $state(""); // "" = automatico
  ripetizioni = $state(1);
  cliPresente = $state(true);
  installazione = $state<"" | "in corso" | "errore">("");
  messaggioInstall = $state("");
  erroreVelocita = $state("");
  progresso = $state<ProgressoSpeedtest | null>(null);

  // Stato generale
  fase = $state<Fase>("inattivo");
  caricamentoIniziale = $state(true);
  messaggio = $state("");

  // Grafico e confronto
  intervallo = $state<Intervallo>("30");
  filtroVpn = $state<FiltroVpn>("tutte");
  confronto = $state<GruppoVpn[]>([]);
  private intervalloScelto = false;

  // Impostazioni (pannello laterale)
  impostazioni = $state<Impostazioni | null>(null);
  pannelloAperto = $state(false);
  erroreImpostazioni = $state("");

  // Dialogo di conferma nella pagina
  dialogo = $state<{ testo: string; risolvi: (v: boolean) => void } | null>(null);
  // Finestra della licenza di Ookla (primo speedtest manuale)
  licenza = $state<{ risolvi: (v: boolean) => void } | null>(null);
  // Esito dell'ultimo import del CSV storico (mostrato nelle impostazioni)
  esitoImport = $state("");

  // Aggiornamenti (GitHub Releases): mai installati senza un clic dell'utente
  versioneApp = $state("");
  aggiornamento = $state<Update | null>(null);
  /** «Più tardi»: l'avviso si nasconde fino al prossimo avvio o alla ricerca manuale. */
  avvisoNascosto = $state(false);
  statoAggiornamento = $state<"" | "controllo" | "download" | "installazione">("");
  /** Percentuale scaricata (0-100), solo se il server dichiara la dimensione. */
  percentualeDownload = $state<number | null>(null);
  /** Esito dell'ultima ricerca manuale (mostrato nelle impostazioni). */
  esitoAggiornamento = $state("");
  erroreAggiornamento = $state(false);

  private inizializzato = false;

  occupato = $derived(this.fase !== "inattivo");

  /** Media dell'ultima sessione di misure (tutte le ripetizioni dello stesso avvio). */
  ultimaMedia = $derived.by<MediaSessione | null>(() => {
    if (this.storico.length === 0) return null;
    const ultima = this.storico[this.storico.length - 1];
    const misure = this.storico.filter((m) => m.sessione === ultima.sessione);
    return {
      timestamp: ultima.timestamp,
      server: ultima.serverNome,
      luogo: ultima.serverLuogo,
      vpnAttiva: ultima.vpnAttiva,
      n: misure.length,
      downloadMbps: media(misure.map((m) => m.downloadMbps)) ?? 0,
      uploadMbps: media(misure.map((m) => m.uploadMbps)) ?? 0,
      pingMs: media(misure.map((m) => m.pingMs)) ?? 0,
      jitterMs: media(misure.flatMap((m) => (m.jitterMs === null ? [] : [m.jitterMs]))),
      packetLoss: media(misure.flatMap((m) => (m.packetLoss === null ? [] : [m.packetLoss]))),
    };
  });

  /** Misure dell'intervallo scelto (prima del filtro VPN). */
  misureIntervallo = $derived.by<MisuraVelocita[]>(() => {
    const da = this.inizioIntervallo();
    if (da === null) return this.storico;
    return this.storico.filter((m) => new Date(m.timestamp).getTime() >= da);
  });

  misureGrafico = $derived.by<MisuraVelocita[]>(() => {
    const f = this.filtroVpn;
    if (f === "tutte") return this.misureIntervallo;
    return this.misureIntervallo.filter((m) => m.vpnAttiva === (f === "accesa"));
  });

  private inizioIntervallo(): number | null {
    if (this.intervallo === "tutto") return null;
    return Date.now() - Number(this.intervallo) * GIORNO_MS;
  }

  // --- Avvio ---------------------------------------------------------------

  async inizializza() {
    if (this.inizializzato) return;
    this.inizializzato = true;
    await this.caricaLingua();

    await listen<ProgressoSpeedtest>("speedtest-progresso", (ev) => {
      this.progresso = ev.payload;
    });
    await listen<EventoFase>("check-fase", (ev) => {
      // Vale anche per i turni automatici (controllo o speedtest in background): la fase risulta
      // attiva e lo speedtest automatico si può annullare dalla dashboard
      if (ev.payload.fase === "sicurezza") this.fase = "sicurezza";
      else if (ev.payload.fase === "velocita") this.fase = "velocita";
      else {
        this.fase = "inattivo";
        this.progresso = null;
      }
    });

    // Eventi del monitoraggio in background (controlli e misure automatici, menu della tray)
    await listen<NuovoControllo>("nuovo-controllo", (ev) => {
      this.aggiornaReport(ev.payload.report);
      void this.caricaControlli();
    });
    await listen("nuova-misura", () => {
      void this.caricaStorico();
    });
    await listen<string>("tray-azione", (ev) => {
      if (ev.payload === "check") void this.checkCompleto();
      else void this.soloSicurezza();
    });
    await listen<Impostazioni>("impostazioni-cambiate", (ev) => {
      this.impostazioni = ev.payload;
    });

    await Promise.all([this.caricaImpostazioni(), this.caricaStorico(), this.caricaControlli(true)]);
    this.caricamentoIniziale = false;
    // Ricerca di aggiornamenti all'avvio: in silenzio, senza rallentare il primo controllo
    void this.caricaVersione();
    if (this.impostazioni?.controllaAggiornamenti) void this.cercaAggiornamenti(false);
    // Il server si legge in parallelo: non deve ritardare il primo controllo
    void this.caricaServer();
    // Controllo leggero all'avvio: solo sicurezza
    await this.soloSicurezza();
  }

  // --- Caricamento dati ------------------------------------------------------

  async caricaStorico() {
    try {
      this.storico = await invoke<MisuraVelocita[]>("storico_velocita", {});
      if (!this.intervalloScelto) this.intervallo = this.storico.length < 10 ? "tutto" : "30";
    } catch (e) {
      this.erroreVelocita = t("msg.storicoNonLeggibile", { err: descriviErrore(e) });
    }
    await this.caricaConfronto();
  }

  async caricaConfronto() {
    const da = this.inizioIntervallo();
    try {
      this.confronto = await invoke<GruppoVpn[]>("confronto_vpn", {
        da: da === null ? null : new Date(da).toISOString(),
        a: null,
      });
    } catch {
      this.confronto = [];
    }
  }

  /** Legge gli ultimi 10 controlli; al primo caricamento mostra l'ultimo come stato corrente. */
  async caricaControlli(mostraUltimo = false) {
    try {
      this.controlli = await invoke<ControlloSicurezza[]>("ultimi_controlli", { limite: 10 });
      const ultimo = this.controlli[0];
      if (mostraUltimo && ultimo && ultimo.report && typeof ultimo.report === "object") {
        const r = ultimo.report as SecurityReport;
        if (r.blacklistResults && r.activeTunnels) {
          this.report = r;
          this.ultimoAggiornamento = new Date(ultimo.timestamp);
        }
      }
    } catch {
      this.controlli = [];
    }
  }

  async caricaServer() {
    try {
      this.server = await invoke<ServerSpeedtest[]>("elenca_server");
      this.cliPresente = true;
    } catch (e) {
      if (eCliMancante(e)) this.cliPresente = false;
      // Altri errori (rete assente): l'elenco resta vuoto, resta valido «Automatico»
    }
  }

  // --- Impostazioni -----------------------------------------------------------

  /** La lingua effettiva (la scelta «automatica» è già risolta dal backend) pilota tutti i testi. */
  async caricaLingua() {
    try {
      i18n.imposta(await invoke<LinguaEffettiva>("lingua_effettiva"));
    } catch {
      /* resta la lingua di partenza */
    }
  }

  async caricaImpostazioni() {
    try {
      const i = await invoke<Impostazioni>("leggi_impostazioni");
      this.impostazioni = i;
      // Server e ripetizioni della barra ripartono dall'ultima scelta salvata
      this.serverScelto = i.serverSpeedtest === null ? "" : String(i.serverSpeedtest);
      this.ripetizioni = i.ripetizioniSpeedtest;
    } catch {
      this.impostazioni = null;
    }
  }

  async salvaImpostazioni(bozza: Impostazioni): Promise<boolean> {
    this.erroreImpostazioni = "";
    try {
      this.impostazioni = await invoke<Impostazioni>("salva_impostazioni", {
        impostazioni: {
          ...bozza,
          serverSpeedtest: this.serverScelto === "" ? null : Number(this.serverScelto),
          ripetizioniSpeedtest: this.ripetizioni,
        },
      });
      await this.caricaLingua();
      this.messaggio = t("msg.impostazioniSalvate");
      return true;
    } catch (e) {
      this.erroreImpostazioni = String(e);
      return false;
    }
  }

  /** Server e ripetizioni scelti nella barra servono anche allo speedtest automatico. */
  async salvaSceltaBarra() {
    try {
      await invoke("imposta_scelta_speedtest", {
        serverId: this.serverScelto === "" ? null : Number(this.serverScelto),
        ripetizioni: this.ripetizioni,
      });
    } catch {
      /* non critico */
    }
  }

  // --- Aggiornamenti ----------------------------------------------------------

  async caricaVersione() {
    try {
      this.versioneApp = await getVersion();
    } catch {
      this.versioneApp = "";
    }
  }

  /**
   * Cerca una versione nuova. All'avvio (`manuale` falso) ogni errore è silenzioso e non si
   * dice nulla se non c'è niente; dal pulsante delle impostazioni si mostra sempre l'esito.
   */
  async cercaAggiornamenti(manuale: boolean) {
    if (this.statoAggiornamento !== "") return;
    this.statoAggiornamento = "controllo";
    if (manuale) {
      this.esitoAggiornamento = "";
      this.erroreAggiornamento = false;
    }
    try {
      const nuovo = await check();
      if (nuovo) {
        this.aggiornamento = nuovo;
        this.avvisoNascosto = false;
        if (manuale) this.esitoAggiornamento = t("agg.disponibile", { versione: nuovo.version });
      } else if (manuale) {
        this.esitoAggiornamento = t("agg.aggiornata", { versione: this.versioneApp });
      }
    } catch (e) {
      if (manuale) {
        this.esitoAggiornamento = t("agg.errore", { err: String(e) });
        this.erroreAggiornamento = true;
      }
    } finally {
      this.statoAggiornamento = "";
    }
  }

  /** Scarica e installa la versione trovata (solo dopo il clic su «Aggiorna ora»). */
  async installaAggiornamento() {
    const agg = this.aggiornamento;
    if (!agg || this.statoAggiornamento !== "") return;
    this.statoAggiornamento = "download";
    this.percentualeDownload = null;
    let totale = 0;
    let scaricato = 0;
    try {
      await agg.downloadAndInstall((ev: DownloadEvent) => {
        if (ev.event === "Started") totale = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          scaricato += ev.data.chunkLength;
          if (totale > 0) this.percentualeDownload = Math.min(100, Math.round((scaricato / totale) * 100));
        } else this.statoAggiornamento = "installazione";
      });
      // Su Windows l'installer chiude l'app e la riavvia da sé: qui non si arriva quasi mai
      this.statoAggiornamento = "installazione";
    } catch (e) {
      this.statoAggiornamento = "";
      this.percentualeDownload = null;
      this.messaggio = t("agg.erroreInstallazione", { err: String(e) });
    }
  }

  rimandaAggiornamento() {
    this.avvisoNascosto = true;
  }

  // --- Intervallo e filtro ---------------------------------------------------

  async impostaIntervallo(v: Intervallo) {
    this.intervalloScelto = true;
    this.intervallo = v;
    await this.caricaConfronto();
  }

  impostaFiltro(v: FiltroVpn) {
    this.filtroVpn = v;
  }

  // --- Conferma a consumo ----------------------------------------------------

  chiedi(testo: string): Promise<boolean> {
    return new Promise((risolvi) => {
      this.dialogo = {
        testo,
        risolvi: (v) => {
          this.dialogo = null;
          risolvi(v);
        },
      };
    });
  }

  /** Vero se si può procedere con lo speedtest (rete non a consumo o conferma dell'utente). */
  private async consentiVelocita(): Promise<boolean> {
    let aConsumo = false;
    try {
      aConsumo = await invoke<boolean>("connessione_a_consumo");
    } catch {
      aConsumo = false;
    }
    return aConsumo ? this.chiedi(t("msg.consumo")) : true;
  }

  // --- Licenza di Ookla --------------------------------------------------------

  /**
   * Vero se la licenza di Ookla è accettata: se non lo è, mostra la finestra e, su «Accetto»,
   * salva il consenso nel backend. Il backend rifiuta comunque lo speedtest senza consenso.
   */
  private async assicuraLicenza(): Promise<boolean> {
    if (this.impostazioni?.licenzaOoklaAccettata) return true;
    const accettata = await new Promise<boolean>((risolvi) => {
      this.licenza = {
        risolvi: (v) => {
          this.licenza = null;
          risolvi(v);
        },
      };
    });
    if (!accettata) return false;
    try {
      this.impostazioni = await invoke<Impostazioni>("accetta_licenza_ookla");
    } catch (e) {
      this.erroreVelocita = t("msg.licenzaNonSalvata", { err: String(e) });
      return false;
    }
    // L'elenco dei server non era stato letto senza licenza
    void this.caricaServer();
    return true;
  }

  // --- Azioni ------------------------------------------------------------------

  private aggiornaReport(r: SecurityReport) {
    this.report = r;
    this.ultimoAggiornamento = new Date();
  }

  async soloSicurezza() {
    if (this.occupato) return;
    this.fase = "sicurezza";
    this.erroreSicurezza = "";
    try {
      this.aggiornaReport(await invoke<SecurityReport>("scan_sicurezza"));
    } catch (e) {
      this.erroreSicurezza = String(e);
    } finally {
      this.fase = "inattivo";
      await this.caricaControlli();
    }
  }

  async soloVelocita() {
    if (this.occupato) return;
    if (!(await this.assicuraLicenza())) return;
    if (!(await this.consentiVelocita())) return;
    this.fase = "velocita";
    this.erroreVelocita = "";
    this.progresso = null;
    try {
      const r = await invoke<RisultatoSpeedtest>("esegui_speedtest", {
        serverId: this.serverScelto === "" ? null : Number(this.serverScelto),
        ripetizioni: this.ripetizioni,
      });
      this.messaggio =
        r.misure.length === 1 ? t("msg.testCompletatoUna") : t("msg.testCompletato", { n: r.misure.length });
    } catch (e) {
      if (eCliMancante(e)) this.cliPresente = false;
      else this.erroreVelocita = descriviErrore(e);
    } finally {
      this.fase = "inattivo";
      this.progresso = null;
      await this.caricaStorico();
    }
  }

  async checkCompleto() {
    if (this.occupato) return;
    if (!(await this.assicuraLicenza()) || !(await this.consentiVelocita())) {
      // Senza licenza di Ookla o su connessione a consumo senza conferma: solo la parte di sicurezza
      await this.soloSicurezza();
      return;
    }
    this.fase = "sicurezza";
    this.erroreSicurezza = "";
    this.erroreVelocita = "";
    this.progresso = null;
    try {
      const r = await invoke<RisultatoCheck>("check_completo", {
        serverId: this.serverScelto === "" ? null : Number(this.serverScelto),
        ripetizioni: this.ripetizioni,
      });
      this.aggiornaReport(r.controllo);
      if (r.erroreVelocita) {
        if (r.erroreVelocita.tipo === "CliMancante") this.cliPresente = false;
        else this.erroreVelocita = descriviErrore(r.erroreVelocita);
      }
    } catch (e) {
      this.erroreSicurezza = String(e);
    } finally {
      this.fase = "inattivo";
      this.progresso = null;
      await Promise.all([this.caricaStorico(), this.caricaControlli()]);
    }
  }

  async annulla() {
    try {
      await invoke("annulla_speedtest");
    } catch {
      /* niente da annullare */
    }
  }

  async installaCli() {
    this.installazione = "in corso";
    this.messaggioInstall = "";
    try {
      await invoke<string>("installa_speedtest_cli");
      this.cliPresente = await invoke<boolean>("cli_speedtest_presente");
      this.installazione = "";
      if (this.cliPresente) await this.caricaServer();
    } catch (e) {
      this.installazione = "errore";
      this.messaggioInstall = String(e);
    }
  }

  // --- Copia ed esporta --------------------------------------------------------

  async copiaIp(ip: string): Promise<boolean> {
    try {
      await writeText(ip);
      this.messaggio = t("msg.ipCopiato", { ip });
      return true;
    } catch {
      this.messaggio = t("msg.copiaFallita");
      return false;
    }
  }

  async esportaReport(controlloId: number | null) {
    const data = new Date().toISOString().slice(0, 10);
    try {
      const percorso = await save({
        defaultPath: `report_connessione_${data}.txt`,
        filters: [{ name: t("file.testo"), extensions: ["txt"] }],
      });
      if (!percorso) return;
      await invoke("esporta_report", { percorso, controlloId });
      this.messaggio = t("msg.reportSalvato", { percorso });
    } catch (e) {
      this.messaggio = t("msg.esportazioneFallita", { err: String(e) });
    }
  }

  /** Sceglie un CSV di storico (formato dello script PowerShell) e lo importa. */
  async importaCsv() {
    this.esitoImport = "";
    try {
      const percorso = await open({
        multiple: false,
        directory: false,
        filters: [{ name: t("file.csv"), extensions: ["csv"] }],
      });
      if (!percorso || Array.isArray(percorso)) return;
      this.esitoImport = await invoke<string>("importa_storico_csv", { percorso });
      await this.caricaStorico();
    } catch (e) {
      this.esitoImport = t("msg.importFallito", { err: String(e) });
    }
  }

  selezionaControllo(c: ControlloSicurezza | null) {
    this.controlloSelezionato = this.controlloSelezionato?.id === c?.id ? null : c;
  }
}

export const dashboard = new Dashboard();
