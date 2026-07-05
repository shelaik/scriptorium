<script lang="ts">
  import {
    getDocumentMeta,
    citationLinks,
    setDocumentTags,
    createTag,
    type DocumentItem,
    type EditableMeta,
    type CitationLinks,
    type Tag,
  } from "$lib/api";

  // Pannello di dettaglio del documento: un click sulla card lo apre, il
  // doppio click (o Invio, o «Apri») lancia il lettore. Si auto-carica
  // abstract/riassunto/note e i numeri delle citazioni; i tag si modificano
  // qui, al volo. Tutto con i token dell'app.
  let {
    doc,
    tags,
    aiEnabled,
    aiBusy = false,
    thumb = null,
    tagColors,
    onOpen,
    onClose,
    onRadial,
    onAuthor,
    onFavorite,
    onRead,
    onCitations,
    onAttach,
    onSummarize,
    onChanged,
  }: {
    doc: DocumentItem;
    tags: Tag[];
    aiEnabled: boolean;
    aiBusy?: boolean;
    thumb?: string | null;
    tagColors: string[];
    onOpen: () => void;
    onClose: () => void;
    onRadial: (e: MouseEvent) => void;
    onAuthor: (name: string) => void;
    onFavorite: () => void;
    onRead: () => void;
    onCitations: () => void;
    onAttach: () => void;
    onSummarize: () => void;
    onChanged: () => void;
  } = $props();

  let meta = $state<EditableMeta | null>(null);
  let cit = $state<CitationLinks | null>(null);
  let abstractOpen = $state(false);
  let newTag = $state("");
  let copied = $state(false);
  let req = 0; // scarta le risposte di un documento ormai cambiato

  $effect(() => {
    const id = doc.id;
    void doc.has_summary; // il riassunto appena generato deve ricomparire qui
    const my = ++req;
    meta = null;
    cit = null;
    abstractOpen = false;
    getDocumentMeta(id)
      .then((m) => {
        if (my === req) meta = m;
      })
      .catch(() => {});
    citationLinks(id)
      .then((c) => {
        if (my === req) cit = c;
      })
      .catch(() => {});
  });

  const refsInLib = $derived(cit?.references.filter((r) => r.in_library != null).length ?? 0);

  async function toggleTag(t: Tag) {
    const has = doc.tags.some((x) => x.id === t.id);
    const ids = has ? doc.tags.filter((x) => x.id !== t.id).map((x) => x.id) : [...doc.tags.map((x) => x.id), t.id];
    try {
      await setDocumentTags(doc.id, ids);
      onChanged();
    } catch {
      /* il genitore mostra già gli errori di rete altrove */
    }
  }
  async function addTag() {
    const name = newTag.trim();
    if (!name) return;
    try {
      const existing = tags.find((t) => t.name.toLowerCase() === name.toLowerCase());
      const t = existing ?? (await createTag(name, tagColors[tags.length % tagColors.length]));
      if (!doc.tags.some((x) => x.id === t.id)) {
        await setDocumentTags(doc.id, [...doc.tags.map((x) => x.id), t.id]);
      }
      newTag = "";
      onChanged();
    } catch {
      /* ignore */
    }
  }
  async function copyCitekey() {
    if (!doc.citekey) return;
    try {
      await navigator.clipboard.writeText(doc.citekey);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* ignore */
    }
  }
  const authorLine = $derived(
    doc.authors.length <= 3 ? doc.authors : [...doc.authors.slice(0, 3), `+${doc.authors.length - 3}`],
  );
  const pct = $derived(
    doc.is_read
      ? 100
      : doc.page_count && doc.last_page && doc.last_page > 0
        ? Math.min(100, Math.round((doc.last_page / doc.page_count) * 100))
        : null,
  );
</script>

<aside class="panel" aria-label="Dettaglio documento">
  <div class="phead">
    <button class="pico" onclick={(e) => { e.stopPropagation(); onRadial(e); }} title="Tutte le azioni (come il tasto destro)" aria-label="Azioni">⋯</button>
    <span class="phint">doppio click o Invio per leggere</span>
    <button class="pico" onclick={onClose} title="Chiudi il pannello (Esc)" aria-label="Chiudi">✕</button>
  </div>

  <div class="pcover" class:noimg={!thumb}>
    {#if thumb}
      <img src={thumb} alt="" />
    {:else}
      <div class="pph">{doc.has_file ? "PDF" : "Riferimento — senza PDF"}</div>
    {/if}
    {#if pct !== null}
      <div class="pprog" title={doc.is_read ? "Letto" : `Letto al ${pct}%`}><div class="pfill" style="width:{pct}%"></div></div>
    {/if}
  </div>

  <h2 class="ptitle">{doc.title ?? "Senza titolo"}</h2>
  {#if doc.authors.length}
    <p class="pauthors">
      {#each authorLine as a, i (i)}
        {#if a.startsWith("+")}<span class="pmore">{a}</span>{:else}<button class="plink" onclick={() => onAuthor(a)} title={`Tutti i lavori di ${a}`}>{a}</button>{/if}{#if i < authorLine.length - 1}<span class="psep">·</span>{/if}
      {/each}
    </p>
  {/if}
  {#if doc.venue || doc.year}<p class="pvenue">{[doc.venue, doc.year].filter(Boolean).join(" · ")}</p>{/if}
  {#if doc.citekey}
    <button class="pcitekey" onclick={copyCitekey} title="Copia la citekey">{copied ? "copiata ✓" : doc.citekey}</button>
  {/if}

  <div class="pactions">
    <button class="primary popen" onclick={onOpen} disabled={!doc.has_file} title={doc.has_file ? "Apri nel lettore" : "Nessun PDF allegato"}>Apri</button>
    <button class="pico big" class:on={doc.favorite} onclick={onFavorite} title={doc.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti"} aria-label="Preferito">{doc.favorite ? "★" : "☆"}</button>
    <button class="pico big" class:on={doc.is_read} onclick={onRead} title={doc.is_read ? "Segna come da leggere" : "Segna come letto"} aria-label="Letto">✓</button>
  </div>

  {#if !doc.has_file}
    <button class="pattach" onclick={onAttach}>Allega PDF…</button>
  {/if}

  <div class="psec">
    <h3>Tag</h3>
    <div class="ptags">
      {#each doc.tags as t (t.id)}
        <span class="ptag" style="background:{(t.color ?? '#888')}2b; border-color:{t.color ?? '#888'}">
          {t.name}<button class="ptagx" onclick={() => toggleTag(t)} title="Togli questo tag" aria-label={`Togli ${t.name}`}>×</button>
        </span>
      {/each}
    </div>
    <div class="ptagadd">
      <input list="paneltags" placeholder="aggiungi tag…" bind:value={newTag} onkeydown={(e) => e.key === "Enter" && addTag()} />
      <datalist id="paneltags">
        {#each tags.filter((t) => !doc.tags.some((x) => x.id === t.id)) as t (t.id)}<option value={t.name}></option>{/each}
      </datalist>
      <button class="pico" onclick={addTag} disabled={!newTag.trim()} title="Aggiungi il tag" aria-label="Aggiungi tag">+</button>
    </div>
  </div>

  {#if meta?.summary}
    <div class="psec">
      <h3>Riassunto AI</h3>
      <p class="pbody">{meta.summary}</p>
    </div>
  {:else if aiEnabled}
    <div class="psec">
      <h3>Riassunto AI</h3>
      <button class="pghost" onclick={onSummarize} disabled={aiBusy}>{aiBusy ? "genero…" : "Genera riassunto"}</button>
    </div>
  {/if}

  {#if meta?.abstract_text}
    <div class="psec">
      <h3>Abstract</h3>
      <p class="pbody" class:clamp={!abstractOpen}>{meta.abstract_text}</p>
      {#if meta.abstract_text.length > 260}
        <button class="pmore plink" onclick={() => (abstractOpen = !abstractOpen)}>{abstractOpen ? "meno" : "tutto"}</button>
      {/if}
    </div>
  {/if}

  {#if meta?.notes}
    <div class="psec">
      <h3>Le tue note</h3>
      <p class="pbody pnotes">{meta.notes.length > 220 ? meta.notes.slice(0, 220) + "…" : meta.notes}</p>
    </div>
  {/if}

  <div class="psec">
    <h3>Citazioni</h3>
    {#if cit}
      <p class="pbody">
        {cit.references.length} riferimenti{refsInLib ? ` (${refsInLib} in libreria)` : ""} · citato da {cit.cited_by.length} tuoi documenti
      </p>
      <button class="pghost" onclick={onCitations}>Riferimenti e citazioni…</button>
    {:else}
      <p class="pbody dim">…</p>
    {/if}
  </div>
</aside>

<style>
  .panel {
    width: 320px; flex: 0 0 320px; overflow-y: auto; overflow-x: hidden;
    background: var(--surface); border-left: 1px solid var(--border);
    padding: 10px 16px 26px;
    animation: pslide 0.18s cubic-bezier(0.2, 0.9, 0.3, 1);
  }
  @keyframes pslide { from { transform: translateX(24px); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .panel { animation: none; } }

  .phead { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 8px; }
  .phint { font-size: 10px; color: var(--faint); }
  .pico {
    background: transparent; border: 1px solid transparent; border-radius: var(--r-sm);
    color: var(--dim); cursor: pointer; font-size: 14px; line-height: 1;
    width: 26px; height: 26px; display: inline-flex; align-items: center; justify-content: center; flex: 0 0 auto;
  }
  .pico:hover { background: var(--hover); color: var(--accent); }
  .pico.big { width: 34px; height: 34px; font-size: 17px; border-color: var(--border); }
  .pico.big.on { color: var(--accent); background: var(--accent-soft); border-color: var(--accent-soft2); }
  .pico:disabled { opacity: 0.45; cursor: default; }

  .pcover {
    position: relative; width: 100%; aspect-ratio: 4 / 3; overflow: hidden;
    border-radius: var(--r-md); border: 1px solid var(--border); background: var(--thumb-bg);
    display: flex; align-items: center; justify-content: center; margin-bottom: 12px;
  }
  .pcover img { width: 100%; height: 100%; object-fit: cover; object-position: top; display: block; }
  .pph { font-size: 13px; color: var(--dim); text-align: center; border: 1.5px dashed var(--border); border-radius: var(--r-sm); margin: 14px; padding: 10px 14px; }
  .pprog { position: absolute; left: 0; right: 0; bottom: 0; height: 4px; background: color-mix(in srgb, var(--border) 60%, transparent); }
  .pfill { height: 100%; background: var(--accent); }

  .ptitle { margin: 0 0 4px; font-family: var(--serif); font-size: 17px; line-height: 1.35; font-weight: 600; color: var(--text); }
  .pauthors { margin: 0 0 2px; font-size: 12.5px; color: var(--dim); }
  .plink { background: none; border: none; padding: 0; color: var(--dim); cursor: pointer; font-size: inherit; }
  .plink:hover { color: var(--accent); text-decoration: underline; }
  .psep { margin: 0 4px; color: var(--faint); }
  .pmore { color: var(--faint); font-size: 11.5px; }
  .pvenue { margin: 0 0 6px; font-size: 11.5px; color: var(--faint); }
  .pcitekey {
    background: var(--field); border: 1px solid var(--border); border-radius: var(--r-pill);
    color: var(--dim); font-size: 10.5px; padding: 1px 9px; cursor: pointer; font-family: ui-monospace, Consolas, monospace;
  }
  .pcitekey:hover { border-color: var(--accent); color: var(--accent); }

  .pactions { display: flex; align-items: center; gap: 8px; margin: 14px 0 4px; }
  .popen { flex: 1; }
  button.primary {
    background: var(--accent); color: var(--on-accent); border: none;
    border-radius: var(--r-sm); padding: 8px 14px; font-size: 13.5px; font-weight: 600; cursor: pointer;
    box-shadow: var(--shadow-sm);
  }
  button.primary:hover:not(:disabled) { background: var(--accent-strong); }
  button.primary:disabled { opacity: 0.5; cursor: default; }
  .pattach {
    width: 100%; margin-top: 8px;
    background: var(--accent-soft); color: var(--accent); border: 1px solid var(--accent-soft2);
    border-radius: var(--r-sm); padding: 7px; font-size: 12.5px; font-weight: 600; cursor: pointer;
  }

  .psec { margin-top: 16px; border-top: 1px solid var(--border-soft); padding-top: 10px; }
  .psec h3 { margin: 0 0 6px; font-size: 10.5px; font-weight: 700; letter-spacing: 0.6px; text-transform: uppercase; color: var(--faint); }
  .pbody { margin: 0 0 6px; font-size: 12.5px; line-height: 1.55; color: var(--text); white-space: pre-line; }
  .pbody.dim { color: var(--faint); }
  .pbody.clamp { display: -webkit-box; -webkit-line-clamp: 5; line-clamp: 5; -webkit-box-orient: vertical; overflow: hidden; }
  .pnotes { color: var(--dim); font-style: italic; }
  .pghost {
    background: transparent; color: var(--accent); border: 1px solid var(--border);
    border-radius: var(--r-sm); padding: 5px 11px; font-size: 12px; font-weight: 600; cursor: pointer;
  }
  .pghost:hover:not(:disabled) { border-color: var(--accent); background: var(--accent-soft); }
  .pghost:disabled { opacity: 0.5; cursor: default; }

  .ptags { display: flex; flex-wrap: wrap; gap: 5px; margin-bottom: 8px; }
  .ptag {
    display: inline-flex; align-items: center; gap: 3px;
    border: 1px solid; border-radius: var(--r-pill); padding: 1px 4px 1px 9px;
    font-size: 11.5px; color: var(--text);
  }
  .ptagx { background: none; border: none; color: var(--dim); cursor: pointer; font-size: 12px; padding: 0 3px; }
  .ptagx:hover { color: var(--danger); }
  .ptagadd { display: flex; gap: 6px; }
  .ptagadd input {
    flex: 1; min-width: 0; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: var(--r-sm); padding: 5px 9px; font-size: 12px; outline: none;
  }
  .ptagadd input:focus { border-color: var(--accent); }
</style>
