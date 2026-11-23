<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { ora } from "$lib/formati";
  import { t } from "$lib/i18n/i18n.svelte";
</script>

<header class="barra">
  <div class="titolo">
    <h1>Connessione</h1>
    <span class="tenue aggiornamento">
      {#if d.fase !== "inattivo"}
        {t("barra.aggiornamentoInCorso")}
      {:else}
        {t("barra.ultimoAggiornamento", { ora: ora(d.ultimoAggiornamento) })}
      {/if}
    </span>
  </div>

  <div class="comandi">
    <div class="opzioni" aria-label={t("barra.opzioniTest")}>
      <label>
        <span class="tenue">{t("barra.server")}</span>
        <select bind:value={d.serverScelto} onchange={() => d.salvaSceltaBarra()} disabled={d.occupato || !d.cliPresente}>
          <option value="">{t("barra.automatico")}</option>
          {#each d.server as s (s.id)}
            <option value={String(s.id)}>{s.nome} - {s.luogo}</option>
          {/each}
        </select>
      </label>
      <label>
        <span class="tenue">{t("barra.ripetizioni")}</span>
        <select bind:value={d.ripetizioni} onchange={() => d.salvaSceltaBarra()} disabled={d.occupato || !d.cliPresente} class="stretta">
          {#each [1, 2, 3, 4, 5] as n (n)}<option value={n}>{n}</option>{/each}
        </select>
      </label>
    </div>

    <button class="secondario" onclick={() => d.soloSicurezza()} disabled={d.occupato}>
      {t("barra.soloSicurezza")}
    </button>
    <button class="secondario" onclick={() => d.soloVelocita()} disabled={d.occupato || !d.cliPresente}>
      {t("barra.soloVelocita")}
    </button>
    <button class="principale" onclick={() => d.checkCompleto()} disabled={d.occupato}>
      {t("barra.checkCompleto")}
    </button>
    <button
      class="ingranaggio"
      onclick={() => (d.pannelloAperto = true)}
      aria-label={t("barra.impostazioni")}
      title={t("barra.impostazioni")}
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <circle cx="12" cy="12" r="3.2" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.56-1.11 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34h0a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87v0a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.56 1.03z" />
      </svg>
    </button>
  </div>
</header>

<style>
  .barra {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 24px;
    padding: 12px 24px;
    background: color-mix(in srgb, var(--superficie) 92%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--bordo);
  }
  .titolo {
    display: flex;
    align-items: baseline;
    gap: 14px;
  }
  h1 {
    margin: 0;
    font-size: 20px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .aggiornamento {
    font-size: 12.5px;
  }
  .comandi {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 10px;
  }
  .opzioni {
    display: flex;
    gap: 10px;
    margin-right: 6px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 11.5px;
  }
  select {
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    border-radius: 8px;
    padding: 6px 8px;
    min-width: 150px;
    max-width: 220px;
  }
  select.stretta {
    min-width: 56px;
  }
  button {
    border-radius: 8px;
    padding: 8px 14px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font-weight: 550;
  }
  button.secondario:hover:not(:disabled) {
    border-color: var(--accento);
  }
  button.ingranaggio {
    padding: 8px 10px;
    display: grid;
    place-items: center;
    color: var(--testo-tenue);
  }
  button.ingranaggio:hover {
    color: var(--accento);
    border-color: var(--accento);
  }
  button.principale {
    background: var(--accento);
    color: var(--accento-testo);
    border-color: var(--accento);
    padding: 8px 20px;
  }
  button.principale:hover:not(:disabled) {
    filter: brightness(1.08);
  }
</style>
