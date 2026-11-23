// Tipi corrispondenti a `SecurityReport` (src-tauri/src/scanner/report.rs, serde camelCase).

export type OverallStatus = "PROTECTED" | "WARNING" | "UNPROTECTED" | "ERROR";

export interface TunnelInterface {
  name: string;
  ipv4: string;
  ipv6: string;
  mtu: number;
  isUp: boolean;
  speedMbps: number;
  isTunnel: boolean;
}

export interface DnsServerEntry {
  ip: string;
  geoOrg: string;
  ednsIp: string | null;
  ednsGeoOrg: string | null;
  isLeakSuspect: boolean;
}

export type StatoDnsbl = "in_lista" | "pulito" | "non_verificabile";

export interface VoceDnsbl {
  stato: StatoDnsbl;
  nota: string | null;
}

export interface SecurityReport {
  /** ISO 8601 con fuso orario locale */
  timestamp: string;
  durationSec: number;

  overallStatus: OverallStatus;
  overallTitle: string;
  overallSummary: string;
  /** 0 - 100 */
  securityScore: number;

  ipv4: string | null;
  ipv6: string | null;
  isp: string;
  org: string;
  asn: string;
  country: string;
  countryCode: string;
  region: string;
  city: string;
  timezone: string;
  coordinates: string;
  reverseDns: string;

  isVpnOrProxyIp: boolean;
  /** Il servizio IP non dice se è una VPN e nient'altro lo decide: stato sconosciuto. */
  vpnSconosciuta?: boolean;
  isHostingDatacenter: boolean;
  isMullvadServer: boolean;
  mullvadServerName: string;

  activeTunnels: TunnelInterface[];
  allUpInterfaces: TunnelInterface[];
  localDnsServers: string[];
  defaultRouteViaTunnel: boolean;
  killswitchOrStrictRouteDetected: boolean;

  dnsResolvers: DnsServerEntry[];
  dnsLeakDetected: boolean;
  ednsLeakDetected: boolean;
  dnsStatusText: string;

  stunIpv4: string | null;
  webrtcLeakDetected: boolean;
  webrtcStatusText: string;
  ipv6LeakDetected: boolean;
  latencyMs: number | null;

  /** Stato per lista DNSBL; solo `in_lista` conta per punteggio e verdetto. */
  blacklistResults: Record<string, VoceDnsbl>;
  blacklistedCount: number;
  mullvadBlacklisted: boolean | null;

  warnings: string[];
  positives: string[];
}

// ---------------------------------------------------------------------------
// Velocità (src-tauri/src/speedtest e archivio)
// ---------------------------------------------------------------------------

export interface ServerSpeedtest {
  id: number;
  nome: string;
  luogo: string;
  paese: string;
  host: string;
}

export type FaseSpeedtest = "ping" | "download" | "upload";

/** Payload dell'evento Tauri `speedtest-progresso`. */
export interface ProgressoSpeedtest {
  /** Da 1 */
  ripetizione: number;
  totale: number;
  fase: FaseSpeedtest;
  /** 0 - 1 all'interno della fase */
  avanzamento: number;
  /** ms per il ping, Mbps per download e upload */
  valore: number;
}

export interface MisuraVelocita {
  id: number | null;
  timestamp: string;
  sessione: string;
  isp: string | null;
  ipEsterno: string | null;
  serverId: number | null;
  serverNome: string | null;
  serverLuogo: string | null;
  pingMs: number;
  jitterMs: number | null;
  downloadMbps: number;
  uploadMbps: number;
  packetLoss: number | null;
  url: string | null;
  vpnAttiva: boolean | null;
  origine: "app" | "import";
}

export interface MediaSpeedtest {
  pingMs: number;
  jitterMs: number | null;
  downloadMbps: number;
  uploadMbps: number;
  packetLoss: number | null;
}

export interface RisultatoSpeedtest {
  sessione: string;
  misure: MisuraVelocita[];
  media: MediaSpeedtest;
}

/** Errore tipizzato dei comandi speedtest. */
export type ErroreSpeedtest =
  | { tipo: "CliMancante" }
  | { tipo: "Annullato" }
  | { tipo: "GiaInEsecuzione" }
  | { tipo: "LicenzaNonAccettata" }
  | { tipo: "ArgomentoNonValido"; dettaglio: string }
  | { tipo: "Fallito"; dettaglio: string };

export interface ControlloSicurezza {
  id: number;
  timestamp: string;
  verdetto: OverallStatus;
  punteggio: number;
  ipv4: string | null;
  vpnAttiva: boolean | null;
  /** Report completo com'era al momento del controllo */
  report: SecurityReport;
}


// ---------------------------------------------------------------------------
// Lotto 3: confronto VPN, check completo
// ---------------------------------------------------------------------------

export interface StatMisura {
  media: number | null;
  mediana: number | null;
}

/** Gruppo di misure con lo stesso stato VPN (true, false oppure null = sconosciuto). */
export interface GruppoVpn {
  vpnAttiva: boolean | null;
  misure: number;
  download: StatMisura;
  upload: StatMisura;
  ping: StatMisura;
}

export type FaseCheck = "sicurezza" | "velocita" | "fatto";

/** Payload dell'evento Tauri `check-fase`. */
export interface EventoFase {
  fase: FaseCheck;
}

export interface RisultatoCheck {
  controllo: SecurityReport;
  controlloId: number | null;
  velocita: RisultatoSpeedtest | null;
  erroreVelocita: ErroreSpeedtest | null;
}

// ---------------------------------------------------------------------------
// Lotto 4: impostazioni, monitoraggio
// ---------------------------------------------------------------------------

/** Come trattare le connessioni a consumo: segue Windows, sempre a consumo, oppure ignora. */
export type ModalitaConsumo = "seguiWindows" | "sempre" | "ignora";

/** Lingua scelta nelle impostazioni: «auto» segue la lingua di Windows. */
export type LinguaScelta = "auto" | "it" | "en";

export interface Impostazioni {
  monitoraggioAttivo: boolean;
  /** 5, 15, 30 o 60 */
  intervalloSicurezzaMin: number;
  /** null = speedtest automatico spento; altrimenti 1, 3, 6, 12 o 24 */
  speedtestOgniOre: number | null;
  serverSpeedtest: number | null;
  ripetizioniSpeedtest: number;
  sogliaDownloadMinMbps: number | null;
  sogliaPingMaxMs: number | null;
  chiudiInTray: boolean;
  avvioConWindows: boolean;
  notificheAttive: boolean;
  modalitaConsumo: ModalitaConsumo;
  /** L'utente ha accettato licenza, termini e informativa privacy di Ookla (solo di sola lettura: si accetta con `accetta_licenza_ookla`). */
  licenzaOoklaAccettata: boolean;
  lingua: LinguaScelta;
  /** Cerca aggiornamenti su GitHub all'avvio (in silenzio); il pulsante manuale funziona comunque. */
  controllaAggiornamenti: boolean;
}

/** Evento `nuovo-controllo` (controllo automatico a finestra aperta o nascosta). */
export interface NuovoControllo {
  controlloId: number | null;
  report: SecurityReport;
}

/** Evento `nuova-misura` (speedtest automatico). */
export interface NuovaMisura {
  misure: MisuraVelocita[];
}
