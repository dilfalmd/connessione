<script lang="ts">
  import uPlot from "uplot";
  import "uplot/dist/uPlot.min.css";
  import { dashboard as d, type FiltroVpn, type Intervallo } from "$lib/stato.svelte";
  import { dataOra, numero } from "$lib/formati";
  import { t } from "$lib/i18n/i18n.svelte";
  import type { MisuraVelocita } from "$lib/tipi";

  const intervalli: { v: Intervallo; t: "graf.g7" | "graf.g30" | "graf.g90" | "graf.tutto" }[] = [
    { v: "7", t: "graf.g7" },
    { v: "30", t: "graf.g30" },
    { v: "90", t: "graf.g90" },
    { v: "tutto", t: "graf.tutto" },
  ];
  const filtri: { v: FiltroVpn; t: "graf.tutte" | "graf.vpnAccesa" | "graf.vpnSpenta" }[] = [
    { v: "tutte", t: "graf.tutte" },
    { v: "accesa", t: "graf.vpnAccesa" },
    { v: "spenta", t: "graf.vpnSpenta" },
  ];

  /** Oltre questo intervallo fra due misure la linea si interrompe (3 giorni). */
  const SALTO_MAX_SEC = 3 * 86400;

  /** Percentile (interpolazione lineare) di un elenco ordinato. */
  function percentile(ordinati: number[], p: number): number {
    const pos = (ordinati.length - 1) * p;
    const i = Math.floor(pos);
    const f = pos - i;
    return i + 1 < ordinati.length ? ordinati[i] + f * (ordinati[i + 1] - ordinati[i]) : ordinati[i];
  }

  /** Massimo dell'asse del ping se ci sono valori anomali, altrimenti `null` (scala automatica). */
  function massimoPing(valori: number[]): number | null {
    if (valori.length === 0) return null;
    const ord = [...valori].sort((a, b) => a - b);
    const mediana = percentile(ord, 0.5);
    if (mediana <= 0 || ord[ord.length - 1] <= 3 * mediana) return null;
    return percentile(ord, 0.95) * 1.2;
  }

  let host: HTMLDivElement;
  let plot: uPlot | undefined;
  let tema = $state(0);
  let misure = $derived(d.misureGrafico);

  function colori() {
    const cs = getComputedStyle(document.documentElement);
    const v = (n: string) => cs.getPropertyValue(n).trim();
    return {
      testo: v("--testo-tenue"),
      griglia: v("--bordo"),
      sfondo: v("--superficie"),
      neutro: v("--neutro"),
      download: v("--serie-download"),
      upload: v("--serie-upload"),
      ping: v("--serie-ping"),
    };
  }

  function costruisci(dati: MisuraVelocita[]) {
    plot?.destroy();
    plot = undefined;
    if (!host || dati.length === 0) return;
    const c = colori();
    // Due misure distanti più di 3 giorni non si collegano: si inserisce un punto vuoto (null)
    // fra le due e si lascia spanGaps a false. I punti restano tutti.
    const x: number[] = [];
    const righe: (MisuraVelocita | null)[] = [];
    dati.forEach((m, i) => {
      const t = new Date(m.timestamp).getTime() / 1000;
      if (i > 0) {
        const prec = x[x.length - 1];
        if (t - prec > SALTO_MAX_SEC) {
          x.push((t + prec) / 2);
          righe.push(null);
        }
      }
      x.push(t);
      righe.push(m);
    });
    const down = righe.map((m) => (m ? m.downloadMbps : null));
    const up = righe.map((m) => (m ? m.uploadMbps : null));

    // Asse del ping: se un valore supera 3 volte la mediana, il massimo diventa il 95° percentile
    // per 1,2 e i punti fuori scala si disegnano sul bordo come triangoli (valore vero nel tooltip)
    const pingMax = massimoPing(dati.map((m) => m.pingMs));
    const fuoriScala = (m: MisuraVelocita | null) => m !== null && pingMax !== null && m.pingMs > pingMax;
    const ping = righe.map((m) => (m ? (fuoriScala(m) ? (pingMax as number) : m.pingMs) : null));
    const xMin = Math.min(...x);
    const xMax = Math.max(...x);
    const margine = Math.max(12 * 3600, (xMax - xMin) * 0.03);

    const tip = document.createElement("div");
    tip.className = "tip-grafico";

    function punti(u: uPlot) {
      const ctx = u.ctx;
      const dpr = window.devicePixelRatio || 1;
      ctx.save();
      const serie: [number, string][] = [
        [1, c.download],
        [2, c.upload],
        [3, c.ping],
      ];
      for (const [i, colore] of serie) {
        const ys = u.data[i] as (number | null)[];
        const scala = u.series[i].scale as string;
        for (let k = 0; k < x.length; k++) {
          const m = righe[k];
          const y = ys[k];
          if (m === null || y === null) continue;
          const cx = u.valToPos(x[k], "x", true);
          const cy = u.valToPos(y, scala, true);
          const vpn = m.vpnAttiva;
          ctx.beginPath();
          if (i === 3 && fuoriScala(m)) {
            // Triangolo sul bordo superiore: il valore è oltre la scala
            const r = 6 * dpr;
            ctx.moveTo(cx, cy - r * 0.2);
            ctx.lineTo(cx - r, cy + r * 1.1);
            ctx.lineTo(cx + r, cy + r * 1.1);
            ctx.closePath();
          } else {
            ctx.arc(cx, cy, 4 * dpr, 0, Math.PI * 2);
          }
          ctx.lineWidth = 1.8 * dpr;
          if (vpn === true) {
            ctx.fillStyle = colore;
            ctx.strokeStyle = colore;
            ctx.fill();
          } else if (vpn === false) {
            ctx.fillStyle = c.sfondo;
            ctx.strokeStyle = colore;
            ctx.fill();
            ctx.stroke();
          } else {
            ctx.fillStyle = c.neutro;
            ctx.strokeStyle = c.neutro;
            ctx.fill();
          }
        }
      }
      ctx.restore();
    }

    function suggerimento(u: uPlot) {
      const k = u.cursor.idx;
      if (k === null || k === undefined || u.cursor.left === undefined || u.cursor.left < 0) {
        tip.style.display = "none";
        return;
      }
      const m = righe[k];
      if (!m) {
        tip.style.display = "none";
        return;
      }
      tip.replaceChildren();
      const riga = (testo: string, forte = false) => {
        const e = document.createElement(forte ? "strong" : "div");
        e.textContent = testo;
        tip.appendChild(e);
        if (forte) tip.appendChild(document.createElement("br"));
      };
      riga(dataOra(m.timestamp), true);
      riga(
        t("graf.tipServer", {
          v: `${m.serverNome ?? t("comune.nd")}${m.serverLuogo ? ` (${m.serverLuogo})` : ""}`,
        }),
      );
      riga(t("graf.tipDownload", { v: numero(m.downloadMbps) }));
      riga(t("graf.tipUpload", { v: numero(m.uploadMbps) }));
      riga(t("graf.tipPing", { v: numero(m.pingMs) }) + (fuoriScala(m) ? t("graf.tipOltreScala") : ""));
      riga(
        t("graf.tipVpn", {
          v:
            m.vpnAttiva === true
              ? t("graf.tipVpnAccesa")
              : m.vpnAttiva === false
                ? t("graf.tipVpnSpenta")
                : t("graf.tipVpnSconosciuta"),
        }),
      );
      tip.style.display = "block";
      const larg = u.over.clientWidth;
      const sinistra = u.cursor.left + 14;
      tip.style.left = `${sinistra + tip.offsetWidth > larg ? u.cursor.left - tip.offsetWidth - 14 : sinistra}px`;
      tip.style.top = `${Math.max(0, Math.min((u.cursor.top ?? 0) - 10, u.over.clientHeight - tip.offsetHeight))}px`;
    }

    const asse = (scala: string, etichetta: string, lato: 1 | 3, griglia: boolean): uPlot.Axis => ({
      scale: scala,
      side: lato,
      label: etichetta,
      stroke: c.testo,
      labelFont: "12px system-ui",
      font: "12px system-ui",
      grid: { show: griglia, stroke: c.griglia, width: 1 },
      ticks: { stroke: c.griglia, width: 1 },
      size: 56,
    });

    const opzioni: uPlot.Options = {
      width: Math.max(300, host.clientWidth),
      height: 280,
      padding: [10, 8, 0, 0],
      legend: { show: false },
      cursor: { drag: { x: false, y: false }, points: { size: 9 } },
      scales: {
        // Intervallo fisso calcolato sui dati: con una sola misura uPlot allargherebbe l'asse a anni
        x: { time: true, auto: false, range: [xMin - margine, xMax + margine] },
        mbps: { range: (_u, _min, max) => [0, Math.max(10, max * 1.12)] },
        ms: {
          range: (_u, _min, max) => [0, pingMax !== null ? pingMax * 1.06 : Math.max(10, max * 1.12)],
        },
      },
      axes: [
        {
          stroke: c.testo,
          font: "12px system-ui",
          grid: { stroke: c.griglia, width: 1 },
          ticks: { stroke: c.griglia, width: 1 },
        },
        asse("mbps", "Mbps", 3, true),
        asse("ms", "ms", 1, false),
      ],
      series: [
        {},
        { label: t("graf.serie.download"), scale: "mbps", stroke: c.download, width: 2, points: { show: false }, spanGaps: false },
        { label: t("graf.serie.upload"), scale: "mbps", stroke: c.upload, width: 2, points: { show: false }, spanGaps: false },
        { label: t("graf.serie.ping"), scale: "ms", stroke: c.ping, width: 1.5, dash: [5, 4], points: { show: false }, spanGaps: false },
      ],
      hooks: {
        ready: [(u) => u.over.appendChild(tip)],
        draw: [punti],
        setCursor: [suggerimento],
      },
    };
    plot = new uPlot(opzioni, [x, down, up, ping], host);
  }

  $effect(() => {
    // Dipendenze: misure filtrate e tema di sistema
    tema;
    costruisci(misure);
    return () => {
      plot?.destroy();
      plot = undefined;
    };
  });

  $effect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const cambia = () => (tema += 1);
    mq.addEventListener("change", cambia);
    const ro = new ResizeObserver(() => {
      if (plot && host) plot.setSize({ width: Math.max(300, host.clientWidth), height: 280 });
    });
    ro.observe(host);
    return () => {
      mq.removeEventListener("change", cambia);
      ro.disconnect();
    };
  });
</script>

<section class="card" aria-labelledby="titolo-grafico">
  <div class="testata">
    <h2 id="titolo-grafico">{t("graf.titolo")}</h2>
    <div class="controlli">
      <div class="gruppo" role="group" aria-label={t("graf.ariaIntervallo")}>
        {#each intervalli as i (i.v)}
          <button class:attivo={d.intervallo === i.v} onclick={() => d.impostaIntervallo(i.v)}>{t(i.t)}</button>
        {/each}
      </div>
      <div class="gruppo" role="group" aria-label={t("graf.ariaFiltro")}>
        {#each filtri as f (f.v)}
          <button class:attivo={d.filtroVpn === f.v} onclick={() => d.impostaFiltro(f.v)}>{t(f.t)}</button>
        {/each}
      </div>
    </div>
  </div>

  <div class="legenda tenue" aria-hidden="true">
    <span><i class="linea" style="background: var(--serie-download)"></i>{t("graf.legDownload")}</span>
    <span><i class="linea" style="background: var(--serie-upload)"></i>{t("graf.legUpload")}</span>
    <span><i class="linea tratt" style="border-color: var(--serie-ping)"></i>{t("graf.legPing")}</span>
    <span class="sep"></span>
    <span><i class="punto pieno"></i>{t("graf.vpnAccesa")}</span>
    <span><i class="punto vuoto"></i>{t("graf.vpnSpenta")}</span>
    <span><i class="punto grigio"></i>{t("graf.legSconosciuta")}</span>
  </div>

  <div class="area">
    <div bind:this={host} class="grafico" class:nascosto={misure.length === 0}></div>
    {#if misure.length === 0}
      <p class="vuoto">
        {#if d.caricamentoIniziale}
          {t("graf.caricamento")}
        {:else if d.storico.length === 0}
          {t("graf.nessunaMisuraStorico")}
        {:else}
          {t("graf.nessunaMisuraFiltro")}
        {/if}
      </p>
    {/if}
  </div>
</section>

<style>
  .card {
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio);
    box-shadow: var(--ombra);
    padding: 18px 20px;
    min-width: 0;
  }
  .testata {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  .controlli {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .gruppo {
    display: inline-flex;
    border: 1px solid var(--bordo);
    border-radius: 8px;
    overflow: hidden;
  }
  .gruppo button {
    border: 0;
    background: var(--superficie-2);
    padding: 5px 11px;
    font-size: 12.5px;
  }
  .gruppo button + button {
    border-left: 1px solid var(--bordo);
  }
  .gruppo button.attivo {
    background: var(--accento);
    color: var(--accento-testo);
  }
  .legenda {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 10px 0 4px;
    font-size: 12px;
    align-items: center;
  }
  .legenda span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .sep {
    width: 1px;
    height: 12px;
    background: var(--bordo);
  }
  .linea {
    display: inline-block;
    width: 18px;
    height: 3px;
    border-radius: 2px;
  }
  .linea.tratt {
    height: 0;
    border-top: 2px dashed;
    background: none;
  }
  .punto {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    box-sizing: border-box;
  }
  .punto.pieno { background: var(--testo-tenue); }
  .punto.vuoto { border: 2px solid var(--testo-tenue); background: var(--superficie); }
  .punto.grigio { background: var(--neutro); opacity: 0.55; }
  .area {
    position: relative;
    min-height: 280px;
  }
  .grafico {
    width: 100%;
  }
  .grafico.nascosto {
    display: none;
  }
  .vuoto {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    color: var(--testo-tenue);
  }
  :global(.tip-grafico) {
    position: absolute;
    display: none;
    z-index: 10;
    pointer-events: none;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: 12.5px;
    line-height: 1.4;
    white-space: nowrap;
    background: var(--superficie-2);
    color: var(--testo);
    border: 1px solid var(--bordo);
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.18);
  }
  :global(.u-legend) {
    display: none;
  }
</style>
