<script lang="ts">
  import { untrack } from "svelte";
  import { dashboard as d } from "$lib/stato.svelte";
  import type { Impostazioni } from "$lib/tipi";
  import { t } from "$lib/i18n/i18n.svelte";

  const intervalliSicurezza = [5, 15, 30, 60];
  const intervalliSpeedtest = [1, 3, 6, 12, 24];

  let bozza = $state<Impostazioni | null>(null);
  // Soglie: interruttore + valore (le soglie spente sono null nel backend)
  let usaDownload = $state(false);
  let valoreDownload = $state(50);
  let usaPing = $state(false);
  let valorePing = $state(100);
  let salvando = $state(false);

  // Alla apertura si parte da una copia delle impostazioni correnti
  $effect(() => {
    const aperto = d.pannelloAperto;
    const imp = d.impostazioni;
    if (!aperto || !imp) return;
    untrack(() => {
      bozza = { ...imp };
      usaDownload = imp.sogliaDownloadMinMbps !== null;
      valoreDownload = imp.sogliaDownloadMinMbps ?? 50;
      usaPing = imp.sogliaPingMaxMs !== null;
      valorePing = imp.sogliaPingMaxMs ?? 100;
      d.erroreImpostazioni = "";
    });
  });

  function chiudi() {
    d.pannelloAperto = false;
  }

  async function salva() {
    if (!bozza) return;
    salvando = true;
    const ok = await d.salvaImpostazioni({
      ...bozza,
      sogliaDownloadMinMbps: usaDownload && valoreDownload > 0 ? Number(valoreDownload) : null,
      sogliaPingMaxMs: usaPing && valorePing > 0 ? Number(valorePing) : null,
    });
    salvando = false;
    if (ok) chiudi();
  }

  function tasto(e: KeyboardEvent) {
    if (d.pannelloAperto && e.key === "Escape" && !d.dialogo) chiudi();
  }

  let sceltaBarra = $derived(
    `${
      d.serverScelto === ""
        ? t("imp.serverAutomatico")
        : t("imp.serverNome", {
            nome: d.server.find((s) => String(s.id) === d.serverScelto)?.nome ?? d.serverScelto,
          })
    }, ${t(d.ripetizioni === 1 ? "imp.ripUna" : "imp.ripPiu", { n: d.ripetizioni })}`,
  );
</script>

<svelte:window onkeydown={tasto} />

{#if d.pannelloAperto}
  <div class="velo" role="presentation" onclick={chiudi}></div>
  <div class="pannello" aria-labelledby="titolo-impostazioni" aria-modal="true" role="dialog">
    <header>
      <h2 id="titolo-impostazioni">{t("imp.titolo")}</h2>
      <button class="chiudi" onclick={chiudi} aria-label={t("comune.chiudi")}>×</button>
    </header>

    {#if bozza}
      <div class="corpo">
        <section>
          <h3>{t("imp.monitoraggio")}</h3>
          <label class="riga-switch">
            <span>{t("imp.monitoraggioAttivo")}<small>{t("imp.monitoraggioDesc")}</small></span>
            <input type="checkbox" role="switch" bind:checked={bozza.monitoraggioAttivo} />
          </label>
          <label class="riga">
            <span>{t("imp.controlloOgni")}</span>
            <select bind:value={bozza.intervalloSicurezzaMin} disabled={!bozza.monitoraggioAttivo}>
              {#each intervalliSicurezza as m (m)}<option value={m}>{t("imp.minuti", { n: m })}</option>{/each}
            </select>
          </label>
          <label class="riga">
            <span>
              {t("imp.speedtestAuto")}
              <small>{t("imp.speedtestUsa", { scelta: sceltaBarra })}</small>
            </span>
            <select bind:value={bozza.speedtestOgniOre} disabled={!bozza.monitoraggioAttivo}>
              <option value={null}>{t("imp.spento")}</option>
              {#each intervalliSpeedtest as h (h)}<option value={h}>{h === 1 ? t("imp.ogniOra") : t("imp.ogniOre", { n: h })}</option>{/each}
            </select>
          </label>
        </section>

        <section>
          <h3>{t("imp.consumoTitolo")}</h3>
          <label class="riga">
            <span>{t("imp.consumoTitolo")}</span>
            <select bind:value={bozza.modalitaConsumo}>
              <option value="seguiWindows">{t("imp.consumoSegui")}</option>
              <option value="sempre">{t("imp.consumoSempre")}</option>
              <option value="ignora">{t("imp.consumoIgnora")}</option>
            </select>
          </label>
          <small class="spiegazione">
            {t("imp.consumoSpiega")}
          </small>
        </section>

        <section>
          <h3>{t("imp.soglie")}</h3>
          <div class="riga-soglia">
            <label class="riga-switch compatta">
              <span>{t("imp.downloadMin")}</span>
              <input type="checkbox" role="switch" bind:checked={usaDownload} />
            </label>
            <span class="valore">
              <input type="number" min="1" step="1" bind:value={valoreDownload} disabled={!usaDownload} />
              <span class="tenue">Mbps</span>
            </span>
          </div>
          <div class="riga-soglia">
            <label class="riga-switch compatta">
              <span>{t("imp.pingMax")}</span>
              <input type="checkbox" role="switch" bind:checked={usaPing} />
            </label>
            <span class="valore">
              <input type="number" min="1" step="1" bind:value={valorePing} disabled={!usaPing} />
              <span class="tenue">ms</span>
            </span>
          </div>
        </section>

        <section>
          <h3>{t("imp.applicazione")}</h3>
          <label class="riga-switch">
            <span>{t("imp.chiudiInTray")}<small>{t("imp.chiudiInTrayDesc")}</small></span>
            <input type="checkbox" role="switch" bind:checked={bozza.chiudiInTray} />
          </label>
          <label class="riga-switch">
            <span>{t("imp.avvioWindows")}<small>{t("imp.avvioWindowsDesc")}</small></span>
            <input type="checkbox" role="switch" bind:checked={bozza.avvioConWindows} />
          </label>
          <label class="riga">
            <span>{t("imp.lingua")}</span>
            <select bind:value={bozza.lingua}>
              <option value="auto">{t("imp.linguaAuto")}</option>
              <option value="it">{t("imp.linguaIt")}</option>
              <option value="en">{t("imp.linguaEn")}</option>
            </select>
          </label>
          <label class="riga-switch">
            <span>{t("imp.notifiche")}<small>{t("imp.notificheDesc")}</small></span>
            <input type="checkbox" role="switch" bind:checked={bozza.notificheAttive} />
          </label>
        </section>

        <section>
          <h3>{t("imp.aggiornamenti")}</h3>
          <label class="riga-switch">
            <span>{t("imp.controllaAvvio")}<small>{t("imp.controllaAvvioDesc")}</small></span>
            <input type="checkbox" role="switch" bind:checked={bozza.controllaAggiornamenti} />
          </label>
          <div class="riga">
            <span>{t("imp.versione", { versione: d.versioneApp || "?" })}</span>
            <button class="importa" onclick={() => d.cercaAggiornamenti(true)} disabled={d.statoAggiornamento !== ""}>
              {d.statoAggiornamento === "controllo" ? t("imp.cercando") : t("imp.cerca")}
            </button>
          </div>
          {#if d.esitoAggiornamento}
            <p class="esito" class:errore-agg={d.erroreAggiornamento} role="status">{d.esitoAggiornamento}</p>
          {/if}
        </section>

        <section>
          <h3>{t("imp.dati")}</h3>
          <div class="riga">
            <span>{t("imp.storico")}<small>{t("imp.storicoDesc")}</small></span>
            <button class="importa" onclick={() => d.importaCsv()}>{t("imp.importa")}</button>
          </div>
          {#if d.esitoImport}<p class="esito" role="status">{d.esitoImport}</p>{/if}
        </section>

        {#if d.erroreImpostazioni}<p class="errore">{d.erroreImpostazioni}</p>{/if}
      </div>

      <footer>
        <button onclick={chiudi}>{t("comune.annulla")}</button>
        <button class="salva" onclick={salva} disabled={salvando}>{salvando ? t("imp.salvando") : t("imp.salva")}</button>
      </footer>
    {:else}
      <p class="vuoto">{t("imp.nonDisponibili")}</p>
    {/if}
  </div>
{/if}

<style>
  .velo {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: rgb(8 12 20 / 0.45);
  }
  .pannello {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 31;
    width: min(420px, 100vw);
    display: flex;
    flex-direction: column;
    background: var(--superficie);
    border-left: 1px solid var(--bordo);
    box-shadow: -10px 0 40px rgb(0 0 0 / 0.25);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--bordo);
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  .chiudi {
    border: 0;
    background: none;
    font-size: 26px;
    line-height: 1;
    padding: 0 6px;
    color: var(--testo-tenue);
  }
  .corpo {
    flex: 1;
    overflow-y: auto;
    padding: 8px 20px 20px;
  }
  section {
    padding: 10px 0;
    border-bottom: 1px solid var(--bordo);
  }
  section:last-of-type {
    border-bottom: 0;
  }
  h3 {
    margin: 4px 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--testo-tenue);
  }
  small {
    display: block;
    color: var(--testo-tenue);
    font-size: 12px;
    margin-top: 1px;
  }
  .riga,
  .riga-switch {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 7px 0;
  }
  .riga-switch.compatta {
    padding: 0;
  }
  .spiegazione {
    margin-top: 2px;
  }
  .importa {
    flex: none;
    border-radius: 8px;
    padding: 6px 12px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font: inherit;
    font-weight: 550;
  }
  .esito {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--testo-tenue);
  }
  .errore-agg {
    color: var(--pericolo);
  }
  .importa:disabled {
    opacity: 0.55;
  }
  .riga-soglia {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 0;
  }
  .valore {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  select,
  input[type="number"] {
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    border-radius: 8px;
    padding: 6px 8px;
    font: inherit;
    color: inherit;
  }
  input[type="number"] {
    width: 84px;
    text-align: right;
  }
  /* Interruttore */
  input[type="checkbox"][role="switch"] {
    appearance: none;
    flex: none;
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: var(--neutro-sfondo);
    border: 1px solid var(--bordo);
    position: relative;
    cursor: pointer;
    transition: background 0.15s;
  }
  input[type="checkbox"][role="switch"]::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--testo-tenue);
    transition: transform 0.15s, background 0.15s;
  }
  input[type="checkbox"][role="switch"]:checked {
    background: var(--accento);
    border-color: var(--accento);
  }
  input[type="checkbox"][role="switch"]:checked::after {
    transform: translateX(16px);
    background: var(--accento-testo);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 14px 20px;
    border-top: 1px solid var(--bordo);
  }
  footer button {
    border-radius: 8px;
    padding: 8px 18px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font-weight: 550;
  }
  footer .salva {
    background: var(--accento);
    color: var(--accento-testo);
    border-color: var(--accento);
  }
  .errore {
    margin: 10px 0 0;
    padding: 8px 12px;
    border-radius: 8px;
    background: var(--pericolo-sfondo);
    color: var(--pericolo);
  }
  .vuoto {
    padding: 24px;
    color: var(--testo-tenue);
  }
</style>
