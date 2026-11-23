# Connessione

**Italiano** | [English](README.md)

[![Licenza: MIT](https://img.shields.io/badge/licenza-MIT-blue.svg)](LICENSE)
[![Ultima release](https://img.shields.io/github/v/release/dilfalmd/connessione?label=release)](https://github.com/dilfalmd/connessione/releases)
![Piattaforma](https://img.shields.io/badge/piattaforma-Windows%2010%2F11-lightgrey)

Un'applicazione per Windows che riunisce in un'unica dashboard il controllo di sicurezza della connessione/VPN e lo speedtest, con storico, grafici e monitoraggio in background. L'interfaccia è in italiano e in inglese.

## Cosa fa

- **Controllo di sicurezza della connessione:** IP pubblico e provider, tunnel VPN attivi, perdite DNS e WebRTC, IPv6, blacklist (DNSBL), con punteggio e verdetto. Il report di un controllo si esporta in un file `.txt`.
- **Speedtest** (download, upload, ping, jitter, packet loss) con la CLI di Ookla Speedtest, con scelta del server e delle ripetizioni.
- **Storico e grafici** di ogni misura e di ogni controllo, con il confronto fra VPN accesa e spenta.
- **Monitoraggio in background:** controlli di sicurezza e speedtest automatici agli intervalli scelti, icona nella tray, notifiche (VPN caduta, perdite, blacklist, soglie di velocità), avvio con Windows.
- **Connessioni a consumo:** l'app segue Windows (oppure lo ignora, oppure considera sempre la rete a consumo) e chiede conferma prima di usare dati su una rete a consumo.
- **Aggiornamenti automatici:** all'avvio l'app cerca su GitHub una versione nuova (si può disattivare) e la installa solo quando premi **Aggiorna ora**.
- Importazione di un vecchio storico `speedtest_log.csv` dello script PowerShell da cui l'app è nata.

## Requisiti

- Windows 10 o 11 (WebView2, già presente nei Windows recenti).
- Per lo speedtest serve la **CLI di Ookla Speedtest** (`speedtest.exe`). L'app **non** la include: se manca, la dashboard propone il pulsante **Installa con winget** (esegue `winget install --id Ookla.Speedtest.CLI -e`), oppure puoi installarla tu e mettere `speedtest.exe` nel `PATH`. Al primo speedtest l'app ti chiede di accettare licenza, termini e informativa sulla privacy di Ookla; la CLI appartiene a Ookla e ha la loro licenza, non quella di questo progetto. Il controllo di sicurezza funziona anche senza la CLI.

## Installazione

1. Scarica `Connessione_<versione>_x64-setup.exe` dalla pagina [Releases](https://github.com/dilfalmd/connessione/releases).
2. Eseguilo. Si installa solo per l'utente corrente, senza privilegi di amministratore, e crea il collegamento nel menu Start.

Il setup **non è firmato** con un certificato a pagamento, quindi Windows SmartScreen può fermarlo la prima volta. Se compare **«Windows ha protetto il PC»** (in inglese «Windows protected your PC»), scegli **Ulteriori informazioni** (More info) e poi **Esegui comunque** (Run anyway).

Chiudendo la finestra l'app resta nella tray; per uscire usa il menu della tray. Per avviarla già nella tray: `connessione.exe --minimizzato` (è ciò che fa «Avvio con Windows»).

## Privacy

Connessione non raccoglie nulla e non invia nulla all'autore né a server dell'autore: non ci sono account, telemetria o statistiche.

Per fare il suo lavoro l'app contatta servizi di terzi, solo dal tuo computer e solo quando parte un controllo o un test:

- **IP e posizione:** ip-api.com, ipwho.is e `am.i.mullvad.net` di Mullvad (il tuo IP pubblico e la posizione approssimativa), api6.ipify.org e ident.me (IPv6) ed `edns.ip-api.com` (quale resolver DNS ti risponde, per il test delle perdite DNS).
- **WebRTC/STUN:** i server STUN di Google e Cloudflare.
- **Blacklist:** i server DNSBL elencati nell'app (per esempio Spamhaus ZEN), tramite il resolver DNS di sistema.
- **Latenza:** una connessione TCP agli indirizzi pubblici di Cloudflare, Google e OpenDNS.
- **Speedtest:** i server di Ookla, tramite la CLI di Ookla (vedi la loro informativa sulla privacy).
- **Aggiornamenti:** GitHub, per cercare una release nuova (solo se il controllo all'avvio è attivo, o quando premi **Cerca aggiornamenti**).

I tuoi dati restano sul tuo computer, in un database SQLite in `%APPDATA%\it.dilettafalcone.connessione` (misure, controlli di sicurezza e impostazioni). Per cancellare tutto basta eliminare quella cartella.

## Compilare dai sorgenti

Requisiti: [Rust](https://www.rust-lang.org/tools/install) (stable), Node.js 24 con [pnpm](https://pnpm.io/), Visual Studio Build Tools 2022 (carico di lavoro C++) e WebView2.

```powershell
pnpm install
pnpm tauri dev        # sviluppo (include i comandi di debug)
pnpm check            # controllo dei tipi del frontend
cargo test            # in src-tauri: test unitari
pnpm tauri build      # release + setup NSIS in src-tauri\target\release\bundle\nsis\
```

Per costruire il setup insieme al pacchetto di aggiornamento serve la chiave di firma dell'updater nella variabile d'ambiente `TAURI_SIGNING_PRIVATE_KEY` (senza, `pnpm tauri build` si ferma alla firma, perché gli artefatti dell'updater sono attivi). La metà pubblica è già in `src-tauri/tauri.conf.json`. Un fork deve generare una propria coppia (`pnpm tauri signer generate`), sostituire la chiave pubblica e l'indirizzo in `endpoints`, e tenere la chiave privata fuori dal repository.

Le icone dell'app e della tray si rigenerano con `python tools/genera_icone_tray.py` (richiede Pillow) e poi `pnpm tauri icon src-tauri/icons/sorgente/app-1024.png`.

Le release le costruisce `.github/workflows/release.yml` quando si pubblica un tag che comincia per `v`.

## Licenza

[MIT](LICENSE) © 2026 Diletta Falcone. Progetto non affiliato a Ookla. Costruito con [Tauri](https://tauri.app/), [Svelte](https://svelte.dev/) e [uPlot](https://github.com/leeoniya/uPlot).
