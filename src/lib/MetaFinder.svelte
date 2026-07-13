<script lang="ts">
  import { onMount } from "svelte";
  import { metadataCandidates, applyMetaCandidate, type MetaProbe, type MetaCandidate } from "$lib/api";

  let {
    id,
    onClose,
    onApplied,
    onEditManual,
  }: { id: number; onClose: () => void; onApplied: () => void; onEditManual?: () => void } = $props();

  let probe = $state<MetaProbe | null>(null);
  let loading = $state(true);
  let applying = $state(false);
  let error = $state("");
  let manualId = $state("");

  onMount(async () => {
    try {
      probe = await metadataCandidates(id);
    } catch (e) {
      error = "Errore ricerca: " + e;
    } finally {
      loading = false;
    }
  });

  async function apply(c: MetaCandidate) {
    applying = true;
    error = "";
    try {
      await applyMetaCandidate(id, c);
      onApplied();
    } catch (e) {
      error = "" + e;
      applying = false;
    }
  }

  /** Parse the manual input into a DOI or arXiv candidate (null if neither). */
  function manualCandidate(): MetaCandidate | null {
    const t = manualId.trim();
    if (!t) return null;
    const base: MetaCandidate = {
      source: "crossref",
      origin: "identificativo inserito",
      doi: null,
      arxiv_id: null,
      title: null,
      authors: [],
      year: null,
      venue: null,
      score: 0,
      sure: false,
      signals: [],
      duplicate_of: null,
    };
    const doi = t.match(/10\.\d{4,9}\/\S+/)?.[0] ?? null;
    if (doi) return { ...base, doi };
    const arx = t.replace(/^arxiv[:\s/]*/i, "").match(/\d{4}\.\d{4,5}(v\d+)?/)?.[0] ?? null;
    if (arx) return { ...base, source: "arxiv", arxiv_id: arx };
    return null;
  }

  async function applyManual() {
    const c = manualCandidate();
    if (c) await apply(c);
  }

  function authorsLine(c: MetaCandidate): string {
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
  <div class="modal" role="dialog" tabindex="-1" aria-label="Recupera metadati" onclick={(e) => e.stopPropagation()}>
    <h2>Recupera metadati</h2>
    {#if loading}
      <p class="dim">Cerco su Crossref, arXiv e OpenAlex — identificativi stampati nel PDF, nome del file, titolo…</p>
    {:else}
      {#if probe?.pdf_title}
        <p class="ctx">Titolo rilevato nel PDF: <strong>{probe.pdf_title}</strong></p>
      {/if}
      {#if probe?.filename}
        <p class="dimfile" title={probe.filename}>File: {probe.filename}</p>
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
                {#if c.duplicate_of}
                  <p class="dup">Già in libreria come «{c.duplicate_of}» — probabilmente è un duplicato: meglio unirli (Cura della libreria → Duplicati).</p>
                {/if}
              </div>
              <div class="candact">
                <button
                  class={c.sure || i === 0 ? "primary small" : "ghost small"}
                  disabled={applying || !!c.duplicate_of}
                  onclick={() => apply(c)}
                  title="Applica titolo, autori, anno, rivista (e riferimenti se c'è il DOI) a questo documento"
                >{applying ? "…" : "Usa questi"}</button>
              </div>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="dim">Nessun candidato trovato online (documento non indicizzato, o prima pagina illeggibile). Prova con un identificativo qui sotto, o correggi a mano.</p>
      {/if}

      <div class="manual">
        <label for="mfid">Ho il DOI / ID arXiv:</label>
        <input id="mfid" bind:value={manualId} placeholder="10.1038/nature14539 oppure 2301.12345" onkeydown={(e) => e.key === "Enter" && applyManual()} />
        <button class="ghost small" disabled={applying || !manualCandidate()} onclick={applyManual} title="Scarico il record completo e lo applico">Applica</button>
      </div>

      {#if error}<p class="err">{error}</p>{/if}
      <div class="actions">
        {#if onEditManual}<button class="ghost" onclick={onEditManual}>Modifica a mano…</button>{/if}
        <button class="ghost" onclick={onClose}>Chiudi</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .back {
    position: fixed; inset: 0; z-index: 82; /* sopra Cura della libreria (80) */
    background: rgba(44, 46, 53, 0.4);
    display: flex; align-items: center; justify-content: center; padding: 24px;
  }
  .modal {
    width: 660px; max-width: 100%; max-height: 92vh; overflow: auto;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-lg, 14px); padding: 22px 24px;
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(44, 46, 53, 0.22));
    resize: both; min-width: 420px; min-height: 260px;
  }
  h2 { margin: 0 0 12px; font-size: 18px; font-family: var(--serif); font-weight: 600; color: var(--text); }
  .dim { color: var(--dim); }
  .ctx { margin: 0 0 2px; font-size: 13px; color: var(--text); }
  .dimfile { margin: 0 0 12px; font-size: 12px; color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
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
    max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .candorigin { margin: 4px 0 0; font-size: 11.5px; color: var(--dim); font-style: italic; }
  .chips { margin: 6px 0 0; display: flex; flex-wrap: wrap; gap: 5px; }
  .chip {
    font-size: 11px; color: var(--dim); border: 1px solid var(--border);
    border-radius: 999px; padding: 2px 8px; background: var(--surface);
  }
  .surechip { color: var(--on-accent); background: var(--accent); border-color: var(--accent); font-weight: 600; }
  .dup { margin: 6px 0 0; font-size: 12px; color: var(--danger); }
  .candact { flex-shrink: 0; }
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
