<script lang="ts">
  // Progetti LaTeX: an Overleaf-like local surface. Real folders on disk
  // (app_data/projects/<slug>/), a plain-text editor with autosave, citations
  // pulled from the library ("Cita" -> \cite{citekey}), refs.bib synced from
  // the whole library, and compilation via the system toolchain (Tectonic or
  // latexmk) with an in-app pdf.js preview of main.pdf.
  import { onMount, tick } from "svelte";
  import * as pdfjsLib from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import {
    listProjects,
    createProject,
    projectFiles,
    readProjectFile,
    writeProjectFile,
    readProjectFileB64,
    syncProjectBib,
    compileProject,
    revealProjectDir,
    searchDocuments,
    type ProjectMeta,
    type ProjectFile,
    type CompileResult,
    type DocumentItem,
  } from "$lib/api";

  pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

  /** Extensions we can open in the text editor (the rest is preview-only). */
  const TEXT_EXT = ["tex", "bib", "sty", "cls", "bst", "txt", "md"];

  let projects: ProjectMeta[] = $state([]);
  let current: string | null = $state(null); // slug
  let files: ProjectFile[] = $state([]);
  let openRel: string | null = $state(null);
  let content = $state("");
  let dirty = $state(false);
  let saveState: "idle" | "saving" | "saved" = $state("idle");
  let loadToken = 0; // discards stale file loads after a quick switch

  let newName = $state("");
  let creating = $state(false);
  let errorMsg = $state("");

  let compiling = $state(false);
  let compileRes: CompileResult | null = $state(null);
  let showLog = $state(false);

  let citeOpen = $state(false);
  let citeQuery = $state("");
  let citeHits: DocumentItem[] = $state([]);
  let citeTimer: ReturnType<typeof setTimeout> | null = null;

  let syncMsg = $state("");

  let editorEl: HTMLTextAreaElement | undefined = $state();
  let previewEl: HTMLDivElement | undefined = $state();
  let pdfTask: ReturnType<typeof pdfjsLib.getDocument> | null = null;
  let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null;
  let previewPages = $state(0);
  let rendering = $state(false);

  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    void refresh(true);
    return () => {
      void flushSave();
      void pdfTask?.destroy();
    };
  });

  async function refresh(openFirst = false) {
    try {
      projects = await listProjects();
    } catch (e) {
      errorMsg = String(e);
      return;
    }
    if (openFirst && !current && projects.length) void openProject(projects[0].slug);
  }

  async function openProject(slug: string) {
    await flushSave();
    current = slug;
    openRel = null;
    content = "";
    compileRes = null;
    showLog = false;
    syncMsg = "";
    clearPreview();
    try {
      files = await projectFiles(slug);
    } catch (e) {
      errorMsg = String(e);
      files = [];
      return;
    }
    // Open main.tex right away; show the last compiled PDF if there is one.
    if (files.some((f) => f.rel === "main.tex")) void openFile("main.tex");
    if (files.some((f) => f.rel === "main.pdf")) void showPdf("main.pdf");
  }

  function extOf(rel: string): string {
    const i = rel.lastIndexOf(".");
    return i >= 0 ? rel.slice(i + 1).toLowerCase() : "";
  }

  async function openFile(rel: string) {
    const ext = extOf(rel);
    if (ext === "pdf") {
      void showPdf(rel);
      return;
    }
    if (!TEXT_EXT.includes(ext) || !current) return;
    await flushSave();
    const tok = ++loadToken;
    try {
      const text = await readProjectFile(current, rel);
      if (tok !== loadToken) return; // user switched again meanwhile
      openRel = rel;
      content = text;
      dirty = false;
      saveState = "idle";
    } catch (e) {
      errorMsg = String(e);
    }
  }

  function onEdit() {
    dirty = true;
    saveState = "idle";
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flushSave(), 800);
  }

  /** Write the buffer if dirty. Safe to call anytime (switch, unmount, Ctrl+S). */
  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (!dirty || !current || !openRel) return;
    const slug = current;
    const rel = openRel;
    const text = content;
    saveState = "saving";
    try {
      await writeProjectFile(slug, rel, text);
      // Only mark clean if nothing changed while the write was in flight.
      if (current === slug && openRel === rel && content === text) {
        dirty = false;
        saveState = "saved";
      }
    } catch (e) {
      saveState = "idle";
      errorMsg = String(e);
    }
  }

  function onEditorKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void flushSave();
    }
  }

  async function doCreate() {
    const name = newName.trim();
    if (!name || creating) return;
    creating = true;
    errorMsg = "";
    try {
      const slug = await createProject(name);
      newName = "";
      await refresh();
      await openProject(slug);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      creating = false;
    }
  }

  async function doCompile() {
    if (!current || compiling) return;
    await flushSave();
    compiling = true;
    compileRes = null;
    try {
      const res = await compileProject(current);
      compileRes = res;
      showLog = !res.ok;
      if (res.ok && res.pdf_rel) {
        files = await projectFiles(current);
        void showPdf(res.pdf_rel);
      }
    } catch (e) {
      errorMsg = String(e);
    } finally {
      compiling = false;
    }
  }

  async function doSyncBib() {
    if (!current) return;
    try {
      const n = await syncProjectBib(current);
      syncMsg = `refs.bib aggiornato: ${n} ${n === 1 ? "voce" : "voci"} dalla libreria`;
      files = await projectFiles(current);
      if (openRel === "refs.bib") {
        // Reload the buffer so the editor shows what is on disk now.
        dirty = false;
        openRel = null;
        await openFile("refs.bib");
      }
      setTimeout(() => (syncMsg = ""), 5000);
    } catch (e) {
      errorMsg = String(e);
    }
  }

  // ----- Cita: search the library, insert \cite{citekey} at the cursor -----
  function onCiteInput() {
    if (citeTimer) clearTimeout(citeTimer);
    citeTimer = setTimeout(async () => {
      const q = citeQuery.trim();
      if (!q) {
        citeHits = [];
        return;
      }
      try {
        citeHits = (await searchDocuments(q, "fulltext")).slice(0, 8);
      } catch {
        citeHits = [];
      }
    }, 250);
  }

  function insertCite(d: DocumentItem) {
    if (!d.citekey || !editorEl) return;
    const ins = `\\cite{${d.citekey}}`;
    editorEl.setRangeText(ins, editorEl.selectionStart, editorEl.selectionEnd, "end");
    content = editorEl.value;
    citeOpen = false;
    citeQuery = "";
    citeHits = [];
    editorEl.focus();
    onEdit();
  }

  // ----- PDF preview (pdf.js, same pattern as the reader) -----
  function clearPreview() {
    void pdfTask?.destroy(); // frees the document and its worker resources
    pdfTask = null;
    pdfDoc = null;
    previewPages = 0;
    if (previewEl) previewEl.replaceChildren();
  }

  function b64ToBytes(b64: string): Uint8Array {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  async function showPdf(rel: string) {
    if (!current || rendering) return;
    rendering = true;
    try {
      const b64 = await readProjectFileB64(current, rel);
      clearPreview();
      const task = pdfjsLib.getDocument({ data: b64ToBytes(b64) });
      pdfTask = task;
      const doc = await task.promise;
      pdfDoc = doc;
      previewPages = doc.numPages;
      await tick(); // the preview container appears with previewPages > 0
      if (!previewEl) return;
      const width = Math.max(320, previewEl.clientWidth - 24);
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const max = Math.min(doc.numPages, 60);
      for (let p = 1; p <= max; p++) {
        if (pdfDoc !== doc) return; // a newer preview replaced this one
        const page = await doc.getPage(p);
        const base = page.getViewport({ scale: 1 });
        const scale = width / base.width;
        const vp = page.getViewport({ scale: scale * dpr });
        const canvas = document.createElement("canvas");
        canvas.width = vp.width;
        canvas.height = vp.height;
        canvas.style.width = `${Math.floor(vp.width / dpr)}px`;
        canvas.className = "pvpage";
        await page.render({ canvas, viewport: vp }).promise;
        if (pdfDoc !== doc || !previewEl) return;
        previewEl.appendChild(canvas);
      }
    } catch (e) {
      errorMsg = String(e);
    } finally {
      rendering = false;
    }
  }

  function fmtSize(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  function fmtWhen(ms: number | null): string {
    if (!ms) return "";
    return new Date(ms).toLocaleDateString("it-IT", { day: "numeric", month: "short" });
  }
</script>

<div class="texwrap">
  <!-- Sidebar: projects + files of the open one -->
  <aside class="texside">
    <div class="newproj">
      <input
        placeholder="Nuovo progetto…"
        bind:value={newName}
        onkeydown={(e) => e.key === "Enter" && doCreate()}
      />
      <button class="tbtn" onclick={doCreate} disabled={creating || !newName.trim()}>Crea</button>
    </div>
    <div class="projlist">
      {#each projects as p (p.slug)}
        <button
          class="projitem"
          class:active={p.slug === current}
          onclick={() => openProject(p.slug)}
        >
          <span class="pname">{p.name}</span>
          <span class="pwhen">{fmtWhen(p.updated_at)}</span>
        </button>
      {/each}
      {#if !projects.length}
        <p class="hint">
          Nessun progetto. Creane uno: nasce con un <code>main.tex</code> di partenza e un
          <code>refs.bib</code> con le citazioni della tua libreria.
        </p>
      {/if}
    </div>
    {#if current}
      <div class="filehead">File</div>
      <div class="filelist">
        {#each files as f (f.rel)}
          <button
            class="fitem"
            class:active={f.rel === openRel}
            onclick={() => openFile(f.rel)}
            title={f.rel}
          >
            <span class="frel">{f.rel}</span>
            <span class="fsize">{fmtSize(f.size)}</span>
          </button>
        {/each}
      </div>
      <button class="tbtn ghost" onclick={() => current && revealProjectDir(current)}>
        Apri cartella
      </button>
    {/if}
  </aside>

  <!-- Editor -->
  <section class="texmain">
    {#if current && openRel}
      <div class="edbar">
        <span class="edfile">{openRel}</span>
        <span class="edstate">
          {#if saveState === "saving"}salvataggio…{:else if saveState === "saved"}salvato{:else if dirty}●{/if}
        </span>
        <span class="spacer"></span>
        <div class="citewrap">
          <button class="tbtn" onclick={() => (citeOpen = !citeOpen)}>Cita</button>
          {#if citeOpen}
            <div class="citepop">
              <!-- svelte-ignore a11y_autofocus -->
              <input
                autofocus
                placeholder="Cerca nella libreria…"
                bind:value={citeQuery}
                oninput={onCiteInput}
                onkeydown={(e) => e.key === "Escape" && (citeOpen = false)}
              />
              <div class="citehits">
                {#each citeHits as d (d.id)}
                  <button class="citehit" disabled={!d.citekey} onclick={() => insertCite(d)}>
                    <span class="chtitle">{d.title ?? "(senza titolo)"}</span>
                    <span class="chkey">
                      {d.citekey ?? "senza citekey"}{d.year ? ` · ${d.year}` : ""}
                    </span>
                  </button>
                {/each}
                {#if citeQuery.trim() && !citeHits.length}
                  <p class="hint">Nessun risultato.</p>
                {/if}
              </div>
            </div>
          {/if}
        </div>
        <button class="tbtn" onclick={doSyncBib} title="Riscrive refs.bib con tutta la libreria">
          Sincronizza bibliografia
        </button>
        <button class="tbtn primary" onclick={doCompile} disabled={compiling}>
          {compiling ? "Compilo…" : "Compila"}
        </button>
      </div>
      {#if syncMsg}
        <div class="note ok">{syncMsg}</div>
      {/if}
      {#if compileRes && !compileRes.ok}
        <div class="note err">
          Compilazione fallita{compileRes.tool ? ` (${compileRes.tool})` : ""}.
          <button class="linkbtn" onclick={() => (showLog = !showLog)}>
            {showLog ? "Nascondi log" : "Mostra log"}
          </button>
        </div>
      {:else if compileRes?.ok}
        <div class="note ok">
          Compilato con {compileRes.tool}.
          <button class="linkbtn" onclick={() => (showLog = !showLog)}>
            {showLog ? "Nascondi log" : "Mostra log"}
          </button>
        </div>
      {/if}
      {#if showLog && compileRes}
        <pre class="complog">{compileRes.log}</pre>
      {/if}
      <textarea
        class="texeditor"
        bind:this={editorEl}
        bind:value={content}
        oninput={onEdit}
        onkeydown={onEditorKey}
        spellcheck="false"
      ></textarea>
    {:else if current}
      <div class="placeholder">Scegli un file dal pannello a sinistra.</div>
    {:else}
      <div class="placeholder">
        <h3>Progetti LaTeX</h3>
        <p>
          Un piccolo Overleaf locale: i progetti sono cartelle vere in
          <code>projects/</code> dentro i dati dell'app — file <code>.tex</code> e
          <code>.bib</code> tuoi, per sempre.
        </p>
        <p>
          Per compilare serve un compilatore LaTeX di sistema. Consigliato
          <strong>Tectonic</strong> (un solo eseguibile, scarica i pacchetti da solo):
        </p>
        <pre>winget install Tectonic.Tectonic</pre>
        <p>In alternativa va bene una distribuzione TeX con <code>latexmk</code> (MiKTeX, TeX Live).</p>
        <p>Senza compilatore, editor + citazioni + bibliografia funzionano comunque.</p>
      </div>
    {/if}
    {#if errorMsg}
      <div class="note err">
        {errorMsg}
        <button class="linkbtn" onclick={() => (errorMsg = "")}>×</button>
      </div>
    {/if}
  </section>

  <!-- PDF preview -->
  <section class="texpreview">
    {#if previewPages}
      <div class="pvhead">Anteprima · {previewPages} pagine{rendering ? " (rendering…)" : ""}</div>
      <div class="pvscroll" bind:this={previewEl}></div>
    {:else}
      <div class="placeholder dimmed">
        {#if compiling}
          Compilazione in corso…
        {:else}
          L'anteprima del PDF compilato apparirà qui dopo «Compila».
        {/if}
      </div>
    {/if}
  </section>
</div>

<style>
  .texwrap {
    display: grid;
    grid-template-columns: 230px minmax(320px, 1fr) minmax(280px, 0.9fr);
    gap: 12px;
    /* fills the main pane below the 60px header, like the notes surface */
    height: calc(100vh - 60px);
    min-height: 400px;
    padding: 12px;
    box-sizing: border-box;
  }
  .texside {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: 10px;
  }
  .newproj {
    display: flex;
    gap: 6px;
  }
  .newproj input {
    flex: 1;
    min-width: 0;
    background: var(--field);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 6px 8px;
    font-size: 0.85rem;
  }
  .projlist {
    overflow-y: auto;
    max-height: 30%;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .projitem {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    padding: 6px 8px;
    background: none;
    border: none;
    border-radius: var(--r-sm);
    color: var(--text);
    cursor: pointer;
    text-align: left;
    font-size: 0.88rem;
  }
  .projitem:hover {
    background: var(--hover);
  }
  .projitem.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-weight: 600;
  }
  .pname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pwhen {
    color: var(--faint);
    font-size: 0.72rem;
    flex-shrink: 0;
  }
  .filehead {
    margin-top: 4px;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--faint);
  }
  .filelist {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-height: 0;
  }
  .fitem {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 8px;
    background: none;
    border: none;
    border-radius: var(--r-sm);
    color: var(--dim);
    cursor: pointer;
    text-align: left;
    font-size: 0.82rem;
    font-family: ui-monospace, Consolas, monospace;
  }
  .fitem:hover {
    background: var(--hover);
  }
  .fitem.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .frel {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fsize {
    color: var(--faint);
    font-size: 0.72rem;
    flex-shrink: 0;
  }
  .texmain {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    min-height: 0;
  }
  .edbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .edfile {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 0.85rem;
    color: var(--dim);
  }
  .edstate {
    font-size: 0.75rem;
    color: var(--faint);
    min-width: 70px;
  }
  .spacer {
    flex: 1;
  }
  .tbtn {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    padding: 5px 12px;
    font-size: 0.82rem;
    cursor: pointer;
  }
  .tbtn:hover:not(:disabled) {
    background: var(--hover);
  }
  .tbtn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .tbtn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .tbtn.primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }
  .tbtn.ghost {
    align-self: stretch;
  }
  .citewrap {
    position: relative;
  }
  .citepop {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 30;
    width: 340px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-md);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .citepop input {
    background: var(--field);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 6px 8px;
    font-size: 0.85rem;
  }
  .citehits {
    max-height: 260px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .citehit {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    background: none;
    border: none;
    border-radius: var(--r-sm);
    padding: 6px 8px;
    cursor: pointer;
    text-align: left;
    color: var(--text);
  }
  .citehit:hover:not(:disabled) {
    background: var(--hover);
  }
  .citehit:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .chtitle {
    font-size: 0.84rem;
    line-height: 1.25;
  }
  .chkey {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 0.72rem;
    color: var(--accent);
  }
  .texeditor {
    flex: 1;
    min-height: 0;
    resize: none;
    background: var(--field);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: 12px 14px;
    font-family: ui-monospace, Consolas, "Cascadia Mono", monospace;
    font-size: 0.88rem;
    line-height: 1.55;
    tab-size: 2;
  }
  .note {
    border-radius: var(--r-sm);
    padding: 6px 10px;
    font-size: 0.82rem;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .note.ok {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .note.err {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .linkbtn {
    background: none;
    border: none;
    color: inherit;
    text-decoration: underline;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0;
  }
  .complog {
    max-height: 180px;
    overflow: auto;
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 8px 10px;
    font-size: 0.74rem;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
  }
  .texpreview {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--viewer-bg);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    overflow: hidden;
  }
  .pvhead {
    padding: 6px 12px;
    font-size: 0.78rem;
    color: var(--dim);
    border-bottom: 1px solid var(--border-soft);
    background: var(--panel);
  }
  .pvscroll {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: center;
  }
  .pvscroll :global(.pvpage) {
    box-shadow: var(--shadow-sm);
    border-radius: 3px;
    background: #fff;
  }
  .placeholder {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    text-align: center;
    color: var(--dim);
    padding: 24px;
    gap: 4px;
    font-size: 0.9rem;
  }
  .placeholder.dimmed {
    color: var(--faint);
  }
  .placeholder h3 {
    margin: 0 0 4px;
    color: var(--text);
  }
  .placeholder p {
    margin: 4px 0;
    max-width: 46ch;
  }
  .placeholder pre {
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 6px 12px;
    font-size: 0.82rem;
  }
  .hint {
    color: var(--faint);
    font-size: 0.8rem;
    padding: 4px 8px;
    margin: 0;
  }
  code {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 0.85em;
  }
</style>
