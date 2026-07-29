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
  import { t, tp } from "$lib/i18n/index.svelte";

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
    onSendToNote,
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
    /** Send the abstract or AI summary to a note (with a citation to this paper). */
    onSendToNote?: (
      p: { content: string; label: string; collapse: boolean },
      ev: MouseEvent,
    ) => void;
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

  // NB: le variabili locali non possono chiamarsi `t`: ombreggerebbero la
  // funzione di traduzione importata qui sopra.
  async function toggleTag(tg: Tag) {
    const has = doc.tags.some((x) => x.id === tg.id);
    const ids = has ? doc.tags.filter((x) => x.id !== tg.id).map((x) => x.id) : [...doc.tags.map((x) => x.id), tg.id];
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
      const existing = tags.find((x) => x.name.toLowerCase() === name.toLowerCase());
      const tg = existing ?? (await createTag(name, tagColors[tags.length % tagColors.length]));
      if (!doc.tags.some((x) => x.id === tg.id)) {
        await setDocumentTags(doc.id, [...doc.tags.map((x) => x.id), tg.id]);
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

<aside class="panel" aria-label={t("Dettaglio documento")}>
  <div class="phead">
    <button class="pico" onclick={(e) => { e.stopPropagation(); onRadial(e); }} title={t("Tutte le azioni (come il tasto destro)")} aria-label={t("Azioni")}>⋯</button>
    <span class="phint">{t("doppio click o Invio per leggere")}</span>
    <button class="pico" onclick={onClose} title={t("Chiudi il pannello (Esc)")} aria-label={t("Chiudi")}>✕</button>
  </div>

  <div class="pcover" class:noimg={!thumb}>
    {#if thumb}
      <img src={thumb} alt="" />
    {:else}
      <!-- i18n-exempt: «PDF» e' una sigla identica in inglese -->
      <div class="pph">{doc.has_file ? "PDF" : t("Riferimento — senza PDF")}</div>
    {/if}
    {#if pct !== null}
      <div class="pprog" title={doc.is_read ? t("Letto") : t("Letto al {pct}%", { pct })}><div class="pfill" style="width:{pct}%"></div></div>
    {/if}
  </div>

  <h2 class="ptitle">{doc.title ?? t("Senza titolo")}</h2>
  {#if doc.authors.length}
    <p class="pauthors">
      {#each authorLine as a, i (i)}
        {#if a.startsWith("+")}<span class="pmore">{a}</span>{:else}<button class="plink" onclick={() => onAuthor(a)} title={t("Tutti i lavori di {autore}", { autore: a })}>{a}</button>{/if}{#if i < authorLine.length - 1}<span class="psep">·</span>{/if}
      {/each}
    </p>
  {/if}
  <!-- i18n-exempt: unisce dati dell'utente (sede, anno) con un separatore neutro -->
  {#if doc.venue || doc.year}<p class="pvenue">{[doc.venue, doc.year].filter(Boolean).join(" · ")}</p>{/if}
  {#if doc.citekey}
    <button class="pcitekey" onclick={copyCitekey} title={t("Copia la citekey")}>{copied ? t("copiata ✓") : doc.citekey}</button>
  {/if}

  <div class="pactions">
    <button class="primary popen" onclick={onOpen} disabled={!doc.has_file} title={doc.has_file ? t("Apri nel lettore") : t("Nessun PDF allegato")}>{t("Apri")}</button>
    <button class="pico big" class:on={doc.favorite} onclick={onFavorite} title={doc.favorite ? t("Togli dai preferiti") : t("Aggiungi ai preferiti")} aria-label={t("Preferito")}>{doc.favorite ? "★" : "☆"}</button>
    <button class="pico big" class:on={doc.is_read} onclick={onRead} title={doc.is_read ? t("Segna come da leggere") : t("Segna come letto")} aria-label={t("Letto")}>✓</button>
  </div>

  {#if !doc.has_file}
    <button class="pattach" onclick={onAttach}>{t("Allega PDF…")}</button>
  {/if}

  <div class="psec">
    <h3>{t("Tag")}</h3>
    <div class="ptags">
      {#each doc.tags as tg (tg.id)}
        <span class="ptag" style="background:{(tg.color ?? '#888')}2b; border-color:{tg.color ?? '#888'}">
          {tg.name}<button class="ptagx" onclick={() => toggleTag(tg)} title={t("Togli questo tag")} aria-label={t("Togli il tag {nome}", { nome: tg.name })}>×</button>
        </span>
      {/each}
    </div>
    <div class="ptagadd">
      <input list="paneltags" placeholder={t("aggiungi tag…")} bind:value={newTag} onkeydown={(e) => e.key === "Enter" && addTag()} />
      <datalist id="paneltags">
        {#each tags.filter((x) => !doc.tags.some((y) => y.id === x.id)) as tg (tg.id)}<option value={tg.name}></option>{/each}
      </datalist>
      <button class="pico" onclick={addTag} disabled={!newTag.trim()} title={t("Aggiungi il tag")} aria-label={t("Aggiungi tag")}>+</button>
    </div>
  </div>

  {#if meta?.summary}
    <div class="psec">
      <div class="psechead">
        <h3>{t("Riassunto AI")}</h3>
        {#if onSendToNote}<button class="tonote" title={t("Manda il riassunto agli Appunti (con citazione a questo paper)")} onclick={(e) => onSendToNote?.({ content: meta?.summary ?? "", label: /* i18n-exempt: etichetta di attribuzione incastonata negli appunti dell'utente */ "Riassunto AI di", collapse: false }, e)}>{t("→ Appunti")}</button>{/if}
      </div>
      <p class="pbody">{meta.summary}</p>
    </div>
  {:else if aiEnabled}
    <div class="psec">
      <h3>{t("Riassunto AI")}</h3>
      <button class="pghost" onclick={onSummarize} disabled={aiBusy}>{aiBusy ? t("genero…") : t("Genera riassunto")}</button>
    </div>
  {/if}

  {#if meta?.abstract_text}
    <div class="psec">
      <div class="psechead">
        <!-- i18n-exempt: «Abstract» e' identico in inglese -->
        <h3>Abstract</h3>
        {#if onSendToNote}<button class="tonote" title={t("Manda l'abstract agli Appunti (con citazione a questo paper)")} onclick={(e) => onSendToNote?.({ content: meta?.abstract_text ?? "", label: /* i18n-exempt: etichetta di attribuzione incastonata negli appunti dell'utente */ "Abstract di", collapse: true }, e)}>{t("→ Appunti")}</button>{/if}
      </div>
      <p class="pbody" class:clamp={!abstractOpen}>{meta.abstract_text}</p>
      {#if meta.abstract_text.length > 260}
        <button class="pmore plink" onclick={() => (abstractOpen = !abstractOpen)}>{abstractOpen ? t("meno") : t("tutto")}</button>
      {/if}
    </div>
  {/if}

  {#if meta?.notes}
    <div class="psec">
      <h3>{t("Nota del documento")}</h3>
      <p class="pbody pnotes">{meta.notes.length > 220 ? meta.notes.slice(0, 220) + "…" : meta.notes}</p>
    </div>
  {/if}

  <div class="psec">
    <h3>{t("Citazioni")}</h3>
    {#if cit}
      {@const rif = refsInLib
        ? t("{n} riferimenti ({inLib} in libreria)", { n: cit.references.length, inLib: refsInLib })
        : tp(cit.references.length, "1 riferimento", "{n} riferimenti")}
      <p class="pbody">
        {t("{riferimenti} · citato da {citanti} tuoi documenti", { riferimenti: rif, citanti: cit.cited_by.length })}
      </p>
      <button class="pghost" onclick={onCitations}>{t("Riferimenti e citazioni…")}</button>
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
  .psechead { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  .psechead h3 { margin-bottom: 6px; }
  .tonote {
    background: transparent; border: 1px solid var(--border-soft); border-radius: 6px;
    color: var(--accent); font-size: 11px; padding: 2px 8px; cursor: pointer; white-space: nowrap;
  }
  .tonote:hover { border-color: var(--accent); background: var(--accent-soft); }
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
