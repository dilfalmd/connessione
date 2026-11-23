<script lang="ts">
  import { onMount } from "svelte";
  import { dashboard as d } from "$lib/stato.svelte";
  import BarraSuperiore from "$lib/componenti/BarraSuperiore.svelte";
  import CardSicurezza from "$lib/componenti/CardSicurezza.svelte";
  import CardVelocita from "$lib/componenti/CardVelocita.svelte";
  import GraficoStorico from "$lib/componenti/GraficoStorico.svelte";
  import CardConfrontoVpn from "$lib/componenti/CardConfrontoVpn.svelte";
  import CardControlli from "$lib/componenti/CardControlli.svelte";
  import DialogoConferma from "$lib/componenti/DialogoConferma.svelte";
  import DialogoLicenzaOokla from "$lib/componenti/DialogoLicenzaOokla.svelte";
  import PannelloImpostazioni from "$lib/componenti/PannelloImpostazioni.svelte";
  import AvvisoAggiornamento from "$lib/componenti/AvvisoAggiornamento.svelte";

  onMount(() => {
    void d.inizializza();
  });

  // Il messaggio informativo scompare da solo
  $effect(() => {
    if (!d.messaggio) return;
    const t = setTimeout(() => (d.messaggio = ""), 6000);
    return () => clearTimeout(t);
  });
</script>

<svelte:head>
  <title>Connessione</title>
</svelte:head>

<BarraSuperiore />

<main>
  <div class="sicurezza"><CardSicurezza /></div>
  <div class="velocita"><CardVelocita /></div>
  <div class="grafico"><GraficoStorico /></div>
  <div class="confronto"><CardConfrontoVpn /></div>
  <div class="controlli"><CardControlli /></div>
</main>

{#if d.messaggio}
  <div class="avviso" role="status">{d.messaggio}</div>
{/if}

<AvvisoAggiornamento />
<PannelloImpostazioni />
<DialogoConferma />
<DialogoLicenzaOokla />

<style>
  main {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 16px;
    max-width: 1400px;
    margin: 0 auto;
    padding: 20px 24px 40px;
    align-items: start;
  }
  .sicurezza { grid-column: span 6; }
  .velocita { grid-column: span 6; }
  .grafico { grid-column: span 12; }
  .confronto { grid-column: span 5; }
  .controlli { grid-column: span 7; }

  @media (max-width: 900px) {
    main {
      grid-template-columns: minmax(0, 1fr);
      padding: 14px 14px 32px;
    }
    .sicurezza,
    .velocita,
    .grafico,
    .confronto,
    .controlli {
      grid-column: 1;
    }
  }

  .avviso {
    position: fixed;
    left: 50%;
    bottom: 20px;
    transform: translateX(-50%);
    max-width: min(640px, calc(100vw - 32px));
    padding: 10px 16px;
    border-radius: 10px;
    background: var(--testo);
    color: var(--sfondo);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.3);
    overflow-wrap: anywhere;
    z-index: 40;
  }
</style>
