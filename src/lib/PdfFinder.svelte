<script lang="ts">
  import { onMount } from "svelte";
  import { pdfCandidates, attachPdfCandidate, attachFromUrl, type PdfProbe, type PdfCandidate } from "$lib/api";
  import { openInBrowser } from "$lib/share";

  let { id, onClose, onApplied }: { id: number; onClose: () => void; onApplied: () => void } = $props();

  let probe = $state<PdfProbe | null>(null);
  let loading = $state(true);
  let busyIdx = $state<number | null>(null);
  let rowErr = $state<{ idx: number; msg: string } | null>(null);
  let error = $state("");
  let manualUrl = $state("");
  let manualBusy = $state(false);

  onMount(async () => {
    try {
      probe = await pdfCandidates(id);
    } catch (e) {
      error = "Errore ricerca: " + e;
    } finally {
      loading = false;
    }
  });

  async function attach(c: PdfCandidate, idx: number) {
    if (busyIdx !== null) return;
    busyIdx = idx;
    rowErr = null;
    error = "";
    try {
      const r = await attachPdfCandidate(id, c);
      if (r === "attached") {
        onApplied();
        return;
      }
      rowErr = {
        idx,
        msg:
          r === "duplicate"
            ? "Questo PDF è già in libreria su un'altra voce — meglio unire i duplicati"
            : r === "already"
              ? "La voce ha già un PDF"
              : "Nessun link scaricabile per questo candidato — prova il prossimo, o apri la pagina e allega il link qui sotto",
      };
    } catch (e) {
      rowErr = { idx, msg: "" + e };
    } finally {
      busyIdx = null;
    }
  }

  async function attachManual() {
    const u = manualUrl.trim();
    if (!u || manualBusy) return;
    manualBusy = true;
    error = "";
    try {
      const r = await attachFromUrl(id, u);
      if (r === "attached") {
        onApplied();
        return;
      }
      error =
        r === "duplicate"
          ? "Quel PDF è già in libreria (su un'altra voce)"
          : r === "already"
            ? "La voce ha già un PDF"
            : r === "not_pdf"
              ? "Il link non è un PDF diretto (deve scaricare un .pdf)"
              : "Non allegato: " + r;
    } catch (e) {
      error = "" + e;
    } finally {
      manualBusy = false;
    }
  }

  function landing(c: PdfCandidate): string | null {
    return c.landing_url ?? (c.doi ? `https://doi.org/${c.doi}` : null) ?? c.pdf_url;
  }

  function authorsLine(c: PdfCandidate): string {
    if (!c.authors.length) return "";
    if (c.authors.length <= 4) return c.authors.join(", ");
    return c.authors.slice(0, 4).join(", ") + " et al.";
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="back" onmousedown={(e) => { if (e.target === e.currentTarget) onClose(); }} role="presentation">
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div class="modal" role="dialog" tabindex="-1" aria-label="Trova PDF" onclick={(e) => e.stopPropagation()}>
    <h2>Trova PDF — candidati</h2>
    {#if loading}
      <p class="dim">Cerco su arXiv, OpenAlex, Semantic Scholar e Crossref — per identificativo e per titolo…</p>
    {:else}
      {#if probe?.title}
        <p class="ctx">Voce: <strong>{probe.title}</strong></p>
      {/if}

      {#if probe && probe.candidates.length}
        <ul class="cands">
          {#each probe.candidates as c, i (i)}
            <li class="cand" class:sure={c.sure}>
              <div class="candbody">
                <p class="candtitle">{c.title ?? "Senza titolo"}</p>
                {#if authorsLine(c)}<p class="candauth">{authorsLine(c)}</p>{/if}
                <p class="candmeta">
                  {[c.year, c.venue].filter(Boolean).join(" · ")}
                  {#if c.pdf_url}<span class="idchip" title={c.pdf_url}>PDF</span>{/if}
                  {#if c.doi}<span class="idchip" title="DOI">{c.doi}</span>{/if}
                  {#if c.arxiv_id}<span class="idchip" title="arXiv">arXiv:{c.arxiv_id}</span>{/if}
                </p>
                <p class="candorigin">{c.origin}</p>
                {#if c.sure || c.signals.length}
                  <p class="chips">
                    {#if c.sure}<span class="chip surechip">corrispondenza sicura</span>{/if}
                    {#each c.signals as s (s)}<span class="chip">{s}</span>{/each}
                  </p>
                {/if}
                {#if rowErr && rowErr.idx === i}
                  <p class="dup">{rowErr.msg}</p>
                {/if}
              </div>
              <div class="candact">
                <button
                  class={c.sure || i === 0 ? "primary small" : "ghost small"}
                  disabled={busyIdx !== null}
                  onclick={() => attach(c, i)}
                  title="Scarica il PDF da questa fonte e allegalo a questa voce (senza duplicati)"
                >{busyIdx === i ? "scarico…" : "Scarica e allega"}</button>
                {#if landing(c)}
                  <button class="ghost small" onclick={() => openInBrowser(landing(c)!)} title="Apri la pagina del paper nel browser per controllare">Apri pagina</button>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="dim">Nessun candidato trovato online. Se conosci la pagina del paper, incolla qui sotto il link diretto al PDF.</p>
      {/if}

      <div class="manual">
        <label for="pfurl">Link diretto al PDF:</label>
        <input
          id="pfurl"
          bind:value={manualUrl}
          placeholder="https://…/file.pdf (vanno bene anche le pagine GitHub /blob/)"
          onkeydown={(e) => e.key === "Enter" && attachManual()}
        />
        <button class="ghost small" disabled={manualBusy || !manualUrl.trim()} onclick={attachManual}>{manualBusy ? "…" : "Allega"}</button>
      </div>

      {#if error}<p class="err">{error}</p>{/if}
      <div class="actions">
        <button class="ghost" onclick={onClose}>Chiudi</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .back {
    position: fixed; inset: 0; z-index: 82; /* sopra il pannello Riferimento (80) */
    background: rgba(44, 46, 53, 0.4);
    display: flex; align-items: center; justify-content: center; padding: 24px;
  }
  .modal {
    width: 680px; max-width: 100%; max-height: 92vh; overflow: auto;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-lg, 14px); padding: 22px 24px;
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(44, 46, 53, 0.22));
    resize: both; min-width: 440px; min-height: 260px;
  }
  h2 { margin: 0 0 12px; font-size: 18px; font-family: var(--serif); font-weight: 600; color: var(--text); }
  .dim { color: var(--dim); }
  .ctx { margin: 0 0 12px; font-size: 13px; color: var(--text); }
  .cands { list-style: none; margin: 0 0 14px; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .cand {
    display: flex; gap: 12px; align-items: flex-start;
    border: 1px solid var(--border); border-radius: 10px; padding: 10px 12px;
    background: var(--field);
  }
  .cand.sure { border-color: var(--accent); }
  .candbody { flex: 1; min-width: 0; }
  .candtitle { margin: 0; font-size: 14px; font-weight: 600; color: var(--text); }
  .candauth { margin: 2px 0 0; font-size: 12.5px; color: var(--text); opacity: 0.9; }
  .candmeta { margin: 3px 0 0; font-size: 12px; color: var(--dim); display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .idchip {
    font-family: var(--mono, monospace); font-size: 11px; color: var(--dim);
    border: 1px solid var(--border); border-radius: 6px; padding: 1px 6px;
    max-width: 250px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .candorigin { margin: 4px 0 0; font-size: 11.5px; color: var(--dim); font-style: italic; }
  .chips { margin: 6px 0 0; display: flex; flex-wrap: wrap; gap: 5px; }
  .chip {
    font-size: 11px; color: var(--dim); border: 1px solid var(--border);
    border-radius: 999px; padding: 2px 8px; background: var(--surface);
  }
  .surechip { color: var(--on-accent); background: var(--accent); border-color: var(--accent); font-weight: 600; }
  .dup { margin: 6px 0 0; font-size: 12px; color: var(--danger); }
  .candact { flex-shrink: 0; display: flex; flex-direction: column; gap: 6px; align-items: stretch; }
  .manual { display: flex; align-items: center; gap: 8px; margin: 4px 0 6px; }
  .manual label { font-size: 12px; color: var(--dim); white-space: nowrap; }
  .manual input {
    flex: 1; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: 7px; padding: 7px 10px; font-size: 13px; font-family: inherit; outline: none;
  }
  .manual input:focus { border-color: var(--accent); }
  .err { color: var(--danger); font-size: 13px; margin: 6px 0 0; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 10px; }
  button {
    border-radius: 8px; padding: 9px 18px; font-size: 14px; font-weight: 600; cursor: pointer;
    border: 1px solid var(--border); background: transparent; color: var(--accent);
  }
  button.small { padding: 6px 12px; font-size: 13px; }
  button.primary { background: var(--accent); color: var(--on-accent); border: none; }
  button:disabled { opacity: 0.55; cursor: default; }
</style>
