# Connessione

[Italiano](README.it.md) | **English**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Latest release](https://img.shields.io/github/v/release/dilfalmd/connessione?label=release)](https://github.com/dilfalmd/connessione/releases)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-lightgrey)

A Windows app that puts a connection/VPN security check and a speed test in one dashboard, with history, charts and background monitoring. The interface is available in Italian and English.

## What it does

- **Connection security check:** public IP and provider, active VPN tunnels, DNS and WebRTC leaks, IPv6, blacklists (DNSBL), with a score and a verdict. A report can be exported to a `.txt` file.
- **Speed test** (download, upload, ping, jitter, packet loss) using the Ookla Speedtest CLI, with server and repetition choice.
- **History and charts** of every measurement and check, with a comparison between VPN on and VPN off.
- **Background monitoring:** automatic security checks and speed tests at the intervals you choose, system tray icon, notifications (VPN dropped, leaks, blacklists, speed thresholds), start with Windows.
- **Metered connections:** the app follows Windows (or ignores it, or always treats the connection as metered) and asks before using data on a metered network.
- **Automatic updates:** the app checks GitHub for a new version at startup (you can turn this off) and installs it only when you click **Update now**.
- Import of an old `speedtest_log.csv` history from the PowerShell script this app grew out of.

## Requirements

- Windows 10 or 11 (WebView2, already present on current Windows).
- For the speed test, the **Ookla Speedtest CLI** (`speedtest.exe`). The app does **not** include it: if it is missing, the dashboard offers an **Install with winget** button (it runs `winget install --id Ookla.Speedtest.CLI -e`), or you can install it yourself and put `speedtest.exe` in your `PATH`. The first time you run a speed test the app asks you to accept Ookla's license, terms and privacy policy; the CLI belongs to Ookla and is covered by their license, not by this project's. The security check works without the CLI.

## Installation

1. Download `Connessione_<version>_x64-setup.exe` from the [Releases](https://github.com/dilfalmd/connessione/releases) page.
2. Run it. It installs for the current user only, without administrator rights, and adds a Start menu shortcut.

The setup is **not signed** with a paid code-signing certificate, so Windows SmartScreen may stop it the first time. If you see **"Windows protected your PC"**, choose **More info** and then **Run anyway**.

Closing the window keeps the app running in the tray; use the tray menu to quit. To start it directly in the tray run `connessione.exe --minimizzato` (this is what "Start with Windows" does).

## Privacy

Connessione does not collect anything and sends nothing to its author or to any server of the author: there is no account, no telemetry and no analytics.

To do its job the app contacts third-party services, only from your computer and only when a check or test runs:

- **IP and location lookups:** ip-api.com, ipwho.is and Mullvad's `am.i.mullvad.net` (your public IP and approximate location), api6.ipify.org and ident.me (IPv6), and `edns.ip-api.com` (which DNS resolver answers you, for the leak test).
- **WebRTC/STUN:** Google and Cloudflare STUN servers.
- **Blacklists:** the DNSBL servers listed in the app (for example Spamhaus ZEN), through your system DNS resolver.
- **Latency:** a TCP connection to Cloudflare, Google and OpenDNS public addresses.
- **Speed test:** Ookla's servers, through the Ookla CLI (see Ookla's privacy policy).
- **Updates:** GitHub, to look for a new release (only if the startup check is on, or when you press **Check for updates**).

All your data stays on your computer, in an SQLite database in `%APPDATA%\it.dilettafalcone.connessione` (measurements, security checks and settings). Delete that folder to erase everything.

## Build from source

Requirements: [Rust](https://www.rust-lang.org/tools/install) (stable), Node.js 24 with [pnpm](https://pnpm.io/), Visual Studio Build Tools 2022 (C++ workload) and WebView2.

```powershell
pnpm install
pnpm tauri dev        # development (includes debug commands)
pnpm check            # frontend type check
cargo test            # in src-tauri: unit tests
pnpm tauri build      # release + NSIS setup in src-tauri\target\release\bundle\nsis\
```

To build the setup together with the update package you need the updater signing key in the `TAURI_SIGNING_PRIVATE_KEY` environment variable (without it `pnpm tauri build` stops at the signing step, because updater artifacts are enabled). The public half is already in `src-tauri/tauri.conf.json`. A fork must generate its own pair (`pnpm tauri signer generate`), replace the public key and the `endpoints` address, and keep the private key out of the repository.

App and tray icons are regenerated with `python tools/genera_icone_tray.py` (needs Pillow), then `pnpm tauri icon src-tauri/icons/sorgente/app-1024.png`.

Releases are built by `.github/workflows/release.yml` when a tag starting with `v` is pushed.

## License

[MIT](LICENSE) © 2026 Diletta Falcone. Not affiliated with Ookla. Built with [Tauri](https://tauri.app/), [Svelte](https://svelte.dev/) and [uPlot](https://github.com/leeoniya/uPlot).
