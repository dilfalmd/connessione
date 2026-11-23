<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { t } from "$lib/i18n/i18n.svelte";

  let dialogo = $derived(d.dialogo);

  function tasto(e: KeyboardEvent) {
    if (!dialogo) return;
    if (e.key === "Escape") dialogo.risolvi(false);
  }
</script>

<svelte:window onkeydown={tasto} />

{#if dialogo}
  <div class="velo" role="presentation">
    <div class="finestra" role="alertdialog" aria-modal="true" aria-labelledby="testo-dialogo">
      <p id="testo-dialogo">{dialogo.testo}</p>
      <div class="azioni">
        <button onclick={() => dialogo?.risolvi(false)}>{t("dlg.no")}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="si" autofocus onclick={() => dialogo?.risolvi(true)}>{t("dlg.continua")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .velo {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgb(8 12 20 / 0.5);
  }
  .finestra {
    width: min(440px, calc(100vw - 32px));
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio);
    padding: 22px;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.35);
  }
  p {
    margin: 0 0 18px;
    font-size: 15px;
    line-height: 1.5;
  }
  .azioni {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  button {
    border-radius: 8px;
    padding: 8px 18px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font-weight: 550;
  }
  .si {
    background: var(--accento);
    color: var(--accento-testo);
    border-color: var(--accento);
  }
</style>
