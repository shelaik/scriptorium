<script lang="ts">
  // ============================================================================
  // ARCHIVIO — le raccolte come sinottico navigabile (stessa lingua visiva
  // della Plancia). Gerarchia vera (parent_id), trascinamenti:
  //   · trascina un PAPER su un nodo   → sposta (Ctrl = aggiungi anche lì)
  //   · trascina un NODO su un nodo    → diventa sotto-raccolta
  //   · trascina un NODO sullo sfondo  → torna alla radice
  //
  // NB: NIENTE drag&drop HTML5 — su Windows il drag-drop nativo di Tauri
  // intercetta gli eventi DOM (ondrop non arriva mai al webview) e spegnerlo
  // romperebbe l'import dei PDF trascinati dal sistema. Tutto il trascinamento
  // interno usa pointer events (pointerdown/move/up/cancel + elementFromPoint).
  //
  // L'appartenenza è multipla per natura (un paper può stare in più raccolte);
  // le raccolte smart si popolano da sole: niente drop dentro, e trascinare
  // VIA da una smart equivale ad «aggiungi altrove» (non c'è nulla da togliere).
  // ============================================================================
  import {
    archiveTree,
    type ArchiveTree,
    type DocumentItem,
    listDocuments,
    listUnfiledDocuments,
    createCollection,
    renameCollection,
    moveCollection,
    deleteCollectionRehome,
    addDocumentsToCollection,
    moveDocumentsToCollection,
    removeDocumentsFromCollection,
    suggestForCollection,
    type CollectionSuggestion,
    type SuggestMode,
    setCollectionWatch,
    mirrorStatus,
    setMirror,
    mirrorRegenerate,
    mirrorReveal,
    type MirrorStatus,
    collectionNotes,
    setNoteCollection,
    createNote,
    saveNote,
    type NoteLink,
  } from "$lib/api";
  import { refToken } from "$lib/notecite";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { t, tp } from "$lib/i18n/index.svelte";

  /** Frammenti autonomi cuciti da un segno, non da una parola: il separatore
   *  resta identico in ogni lingua. */
  const SEP = " · "; /* i18n-exempt: separatore, non testo d'interfaccia */

  let {
    onOpenGrid,
    onOpenGraph,
    onChanged,
    onOpenNote,
  }: {
    onOpenGrid: (id: number, label: string) => void;
    /** Apre la Costellazione ristretta a questa raccolta. */
    onOpenGraph?: (id: number, label: string) => void;
    /** Chiamata dopo ogni mutazione delle raccolte, così la sidebar resta fresca. */
    onChanged?: () => void;
    /** Apre un appunto .md nella vista Appunti (gestita dalla pagina). */
    onOpenNote?: (slug: string) => void;
  } = $props();

  type SelKey = number | "unfiled"; /* i18n-exempt: "unfiled" è un valore interno, non un'etichetta */

  let tree = $state<ArchiveTree | null>(null);
  let sel = $state<SelKey>("unfiled");
  let docs = $state<DocumentItem[]>([]);
  let loadingDocs = $state(false);
  let msg = $state("");
  let renaming = $state(false);
  let renameVal = $state("");
  let creating = $state<"root" | "child" | null>(null);
  let createVal = $state("");
  let confirmDel = $state(false);
  // Trascinamento in corso (nodo o paper), con bersaglio corrente e validità.
  let nodeDrag = $state<{ id: number; name: string; x: number; y: number; target: SelKey | "root" | null; valid: boolean } | null>(null);
  let docDrag = $state<{ ids: number[]; from: number | null; label: string; x: number; y: number; target: SelKey | "root" | null; valid: boolean } | null>(null);
  // Suggerimenti semantici per la raccolta selezionata (motore bge-m3 locale).
  let mirror = $state<MirrorStatus | null>(null);
  let mirrorBusy = $state(false);
  let sugg = $state<CollectionSuggestion[] | null>(null);
  let suggLoading = $state(false);
  let suggThreshold = $state(55); // slider di confidenza (0..100, sul coseno)
  let suggUnfiled = $state(true); // proponi solo paper senza raccolta
  let suggAdding = $state(false);
  // Sorgente della somiglianza: NOME della raccolta, CONTENUTO (centroide dei
  // membri), o ENTRAMBI con peso regolabile (quota del contenuto, default 50%).
  let suggMode = $state<SuggestMode>("both");
  let suggWeight = $state(50);
  try {
    const m = localStorage.getItem("scriptorium-suggmode");
    /* i18n-exempt: "name"/"content"/"both" sono valori persistiti e protocollo col Rust, non etichette */
    if (m === "name" || m === "content" || m === "both") suggMode = m;
    const w = Number(localStorage.getItem("scriptorium-suggweight"));
    if (Number.isFinite(w) && w >= 0 && w <= 100) suggWeight = w;
  } catch {
    /* localStorage assente: si parte coi default */
  }
  function setSuggMode(m: SuggestMode) {
    if (suggMode === m || suggLoading) return;
    suggMode = m;
    try {
      localStorage.setItem("scriptorium-suggmode", m);
    } catch { /* ignore */ }
    // Prima del primo calcolo è solo una scelta; a lista aperta, ricalcola.
    if (sugg != null) void loadSuggestions();
  }
  function saveSuggWeight() {
    try {
      localStorage.setItem("scriptorium-suggweight", String(suggWeight));
    } catch { /* ignore */ }
    if (sugg != null) void loadSuggestions();
  }

  // ---- layout dell'albero -----------------------------------------------------
  const W = 216, H = 46, COLW = 252, ROWH = 62, PADX = 26, PADY = 64;

  interface LNode {
    key: SelKey;
    id: number | null;
    name: string;
    count: number;
    smart: boolean;
    watch: boolean;
    depth: number;
    x: number;
    y: number;
    parent: number | null;
  }

  const layout = $derived.by(() => {
    const nodes: LNode[] = [];
    // NB: la variabile dell'albero si chiama `tr`, non `t`: `t` è la funzione di
    // traduzione importata e ombreggiarla farebbe fallire la traduzione qui dentro.
    const tr = tree;
    if (!tr) return { nodes, edges: [] as { from: LNode; to: LNode }[], w: 900, h: 400 };
    let row = 0;
    nodes.push({
      key: "unfiled", id: null, name: t("SENZA RACCOLTA"), count: tr.unfiled,
      smart: false, watch: false, depth: 0, x: PADX, y: PADY + row++ * ROWH, parent: null,
    });
    const byParent = new Map<number | null, typeof tr.collections>();
    for (const c of tr.collections) {
      const k = c.parent_id ?? null;
      if (!byParent.has(k)) byParent.set(k, []);
      byParent.get(k)!.push(c);
    }
    let maxDepth = 0;
    const walk = (parent: number | null, depth: number) => {
      for (const c of byParent.get(parent) ?? []) {
        maxDepth = Math.max(maxDepth, depth);
        nodes.push({
          key: c.id, id: c.id, name: c.name, count: c.count, smart: c.is_smart,
          watch: c.watch, depth, x: PADX + depth * COLW, y: PADY + row++ * ROWH, parent,
        });
        walk(c.id, depth + 1);
      }
    };
    walk(null, 0);
    const byId = new Map(nodes.filter((n) => n.id != null).map((n) => [n.id as number, n]));
    const edges: { from: LNode; to: LNode }[] = [];
    for (const n of nodes) {
      if (n.parent != null) {
        const p = byId.get(n.parent);
        if (p) edges.push({ from: p, to: n });
      }
    }
    return {
      nodes,
      edges,
      w: PADX * 2 + (maxDepth + 1) * COLW + 40,
      h: PADY + row * ROWH + 30,
    };
  });

  // ---- tastiera sull'albero ---------------------------------------------------
  // Lo schema è un SVG: senza questo non era raggiungibile in nessun modo da
  // tastiera. `layout.nodes` è già in ordine di visita (dall'alto in basso), quindi
  // su/giù è ±1 nell'elenco; destra/sinistra seguono la gerarchia vera.
  let treeEl = $state<SVGSVGElement | undefined>();

  function focusNode(n: LNode | undefined) {
    if (!n) return;
    sel = n.key;
    // Tieni il nodo scelto dentro la finestra visibile del riquadro.
    treeEl?.parentElement?.querySelector(`[data-cid="${CSS.escape(String(n.key))}"]`)
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }

  function treeKey(e: KeyboardEvent) {
    const ns = layout.nodes;
    if (!ns.length) return;
    const at = ns.findIndex((n) => n.key === sel);
    const cur = at >= 0 ? ns[at] : undefined;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusNode(ns[at < 0 ? 0 : Math.min(at + 1, ns.length - 1)]);
        break;
      case "ArrowUp":
        e.preventDefault();
        focusNode(ns[at <= 0 ? 0 : at - 1]);
        break;
      case "ArrowRight":
        e.preventDefault();
        if (cur?.id != null) focusNode(ns.find((n) => n.parent === cur.id));
        break;
      case "ArrowLeft":
        e.preventDefault();
        if (cur?.parent != null) focusNode(ns.find((n) => n.id === cur.parent));
        break;
      case "Home": e.preventDefault(); focusNode(ns[0]); break;
      case "End": e.preventDefault(); focusNode(ns[ns.length - 1]); break;
      case "Enter":
        if (cur && cur.key !== "unfiled" && cur.id != null) {
          e.preventDefault();
          onOpenGrid(cur.id, cur.name);
        }
        break;
    }
  }

  function edgePath(e: { from: LNode; to: LNode }): string {
    const ax = e.from.x + W, ay = e.from.y + H / 2;
    const bx = e.to.x, by = e.to.y + H / 2;
    const mid = ax + (bx - ax) / 2;
    if (Math.abs(ay - by) < 3) return `M ${ax} ${ay} L ${bx} ${by}`;
    return `M ${ax} ${ay} L ${mid} ${ay} L ${mid} ${by} L ${bx} ${by}`;
  }

  const selNode = $derived(layout.nodes.find((n) => n.key === sel) ?? null);
  /** Somma delle APPARTENENZE nel ramo (un paper in più raccolte conta più volte). */
  const selTotal = $derived.by(() => {
    if (!tree || !selNode) return 0;
    if (selNode.key === "unfiled") return tree.unfiled;
    const kids = new Map<number | null, number[]>();
    const cnt = new Map<number, number>();
    for (const c of tree.collections) {
      const k = c.parent_id ?? null;
      if (!kids.has(k)) kids.set(k, []);
      kids.get(k)!.push(c.id);
      cnt.set(c.id, c.count);
    }
    let sum = 0;
    const stack = [selNode.id as number];
    while (stack.length) {
      const id = stack.pop()!;
      sum += cnt.get(id) ?? 0;
      for (const k of kids.get(id) ?? []) stack.push(k);
    }
    return sum;
  });
  const breadcrumb = $derived.by(() => {
    if (!tree || !selNode || selNode.id == null) return [] as string[];
    const byId = new Map(tree.collections.map((c) => [c.id, c]));
    const parts: string[] = [];
    let cur = byId.get(selNode.id);
    while (cur) {
      parts.unshift(cur.name);
      cur = cur.parent_id != null ? byId.get(cur.parent_id) : undefined;
    }
    return parts;
  });

  /** true se `maybeDesc` è dentro il sottoalbero di `rootId` (per la validità del drop). */
  function isDescendantOf(maybeDesc: number, rootId: number): boolean {
    if (!tree) return false;
    const byId = new Map(tree.collections.map((c) => [c.id, c]));
    let cur = byId.get(maybeDesc);
    for (let i = 0; cur && i < 64; i++) {
      if (cur.id === rootId) return true;
      cur = cur.parent_id != null ? byId.get(cur.parent_id) : undefined;
    }
    return false;
  }

  // ---- dati -------------------------------------------------------------------
  let docsEpoch = 0; // anti-race: due clic rapidi non devono mostrare la lista sbagliata
  async function loadTree() {
    try {
      tree = await archiveTree();
      if (sel !== "unfiled" && !tree.collections.some((c) => c.id === sel)) sel = "unfiled";
    } catch (e) {
      msg = t("Errore albero: {err}", { err: String(e) });
    }
  }
  async function loadDocs() {
    const epoch = ++docsEpoch;
    loadingDocs = true;
    try {
      const out = sel === "unfiled" ? await listUnfiledDocuments() : await listDocuments({ collectionId: sel });
      if (epoch === docsEpoch) docs = out;
    } catch (e) {
      if (epoch === docsEpoch) {
        msg = t("Errore documenti: {err}", { err: String(e) });
        docs = [];
      }
    } finally {
      if (epoch === docsEpoch) loadingDocs = false;
    }
  }
  async function refresh() {
    await loadTree();
    await loadDocs();
  }
  $effect(() => {
    void sel;
    void loadDocs();
    void loadCollNotes();
  });

  // ---- appunti .md agganciati alla raccolta -----------------------------------
  // Il caso d'uso: un appunto che discute un gruppo di paper si trova DENTRO quel
  // gruppo, invece che in fondo a un elenco piatto che cresce a ogni sintesi.
  let collNotes = $state<NoteLink[]>([]);
  let creatingNote = $state(false);
  let noteTitleVal = $state("");
  let noteBusy = $state(false);
  let notesEpoch = 0;

  async function loadCollNotes() {
    const epoch = ++notesEpoch;
    if (typeof sel !== "number") {
      collNotes = [];
      return;
    }
    try {
      const out = await collectionNotes(sel);
      if (epoch === notesEpoch) collNotes = out;
    } catch {
      if (epoch === notesEpoch) collNotes = [];
    }
  }

  async function unlinkNote(slug: string) {
    if (typeof sel !== "number") return;
    try {
      await setNoteCollection(slug, sel, false);
      await loadCollNotes();
      msg = t("Appunto sganciato da questa raccolta (il file .md resta dov'era)");
    } catch (e) {
      msg = String(e);
    }
  }

  /** Lo scheletro dell'appunto nuovo: un INVENTARIO, non un'impalcatura retorica.
   *  Titolo, una riga che nomina la raccolta, l'elenco dei paper come citazioni.
   *  Niente «## Tesi / ## Antitesi»: l'elenco è il dato che l'utente avrebbe
   *  copiato comunque a mano (le citekey non sono a schermo, stanno nella scheda
   *  di ogni paper), mentre delle intestazioni imporrebbero una struttura al suo
   *  pensiero — e una lista si cancella in un gesto, un'impalcatura sbagliata la
   *  si combatte. */
  function seedForCollection(title: string, name: string, items: DocumentItem[]): string {
    // Stesso neutralizzatore di buildQuoteBlock: un titolo che contiene «[[…]]»
    // non deve generare un collegamento fantasma nel vault.
    const safe = (s: string) => s.replace(/\[\[/g, "[\\[").replace(/\]\]/g, "]\\]");
    const head = `# ${safe(title)}\n\n`;
    if (!items.length) return head;
    const rows = items.map((d) => {
      const meta = [d.authors[0], d.year].filter(Boolean).join(", ");
      return `- ${refToken(d.citekey, d.title ?? "")}${meta ? ` — ${safe(meta)}` : ""}`;
    });
    return `${head}${t("Paper della raccolta «{raccolta}»:", { raccolta: safe(name) })}\n\n${rows.join("\n")}\n`;
  }

  async function doCreateNote() {
    if (typeof sel !== "number" || noteBusy) return;
    const target = sel;
    const name = selNode?.name ?? "";
    // Titolo vuoto → nome della raccolta: senza questo si cadrebbe nel ripiego di
    // `create_note`, che scrive un titolo italiano dentro un FILE, dove nessuna
    // traduzione arriva.
    const title = (noteTitleVal.trim() || name).trim();
    if (!title) return;
    const items = docs.slice(); // istantanea: la lista può cambiare sotto
    noteBusy = true;
    try {
      const slug = await createNote(title);
      // Prima l'aggancio (un INSERT su una tabella minuscola), poi il corpo (I/O
      // su disco): se cade il corpo, l'appunto esiste, è intitolato ed è già
      // visibile qui. Nessun rollback che lo cancelli: `create_note` ha già
      // scritto un file dell'utente, e distruggerlo perché una seconda scrittura
      // è fallita sarebbe peggio del difetto.
      await setNoteCollection(slug, target, true);
      await saveNote(slug, seedForCollection(title, name, items));
      creatingNote = false;
      noteTitleVal = "";
      await loadCollNotes();
      onChanged?.();
      msg = t("Appunto «{titolo}» creato in questa raccolta", { titolo: title });
      onOpenNote?.(slug);
    } catch (e) {
      msg = String(e);
      await loadCollNotes();
    } finally {
      noteBusy = false;
    }
  }
  $effect(() => {
    void loadTree();
    void mirrorStatus().then((m) => (mirror = m)).catch(() => {});
  });

  // ---- specchio su disco ------------------------------------------------------
  async function toggleMirror() {
    if (mirrorBusy) return;
    mirrorBusy = true;
    try {
      if (mirror?.enabled) {
        mirror = await setMirror(false, null);
        msg = t("Specchio spento (la cartella resta com'è; i tuoi file veri non sono mai stati toccati)");
      } else {
        const dir = await openDialog({
          directory: true,
          title: t("Scegli la cartella dello specchio (vuota o dedicata)"),
          defaultPath: mirror?.dir || undefined,
        });
        if (typeof dir !== "string" || !dir) return;
        mirror = await setMirror(true, dir);
        msg = t("Specchio attivo: genero l'albero in {cartella}", { cartella: dir });
      }
    } catch (e) {
      msg = "" + e;
    } finally {
      mirrorBusy = false;
    }
  }
  async function regenMirror() {
    if (mirrorBusy) return;
    mirrorBusy = true;
    try {
      const s = await mirrorRegenerate();
      const parti = [
        tp(s.linked, "1 hardlink", "{n} hardlink"),
        tp(s.copied, "1 copia", "{n} copie"),
        tp(s.folders, "1 cartella", "{n} cartelle"),
      ];
      if (s.missing) parti.push(tp(s.missing, "1 mancante", "{n} mancanti"));
      msg = t("Specchio rigenerato: {dettaglio}", { dettaglio: parti.join(", ") });
    } catch (e) {
      msg = "" + e;
    } finally {
      mirrorBusy = false;
    }
  }

  function select(key: SelKey) {
    if (sel === key) return;
    sel = key;
    renaming = false;
    creating = null;
    confirmDel = false;
    sugg = null;
    msg = "";
  }

  // ---- suggerimenti -----------------------------------------------------------
  const suggVisible = $derived((sugg ?? []).filter((s) => s.score * 100 >= suggThreshold));
  let suggEpoch = 0; // anti-race: vince l'ULTIMA richiesta, non l'ultima risposta
  async function loadSuggestions() {
    if (typeof sel !== "number") return;
    // Su una raccolta vuota il CONTENUTO non può funzionare: coercizione onesta a NOME.
    if (selNode && selNode.count === 0 && suggMode !== "name") suggMode = "name";
    const target = sel;
    const epoch = ++suggEpoch;
    suggLoading = true;
    msg = "";
    try {
      const out = await suggestForCollection(target, suggUnfiled, suggMode, suggWeight / 100);
      if (epoch !== suggEpoch || sel !== target) return; // superata: scarta
      sugg = out;
      if (out.length === 0)
        msg = suggUnfiled
          ? t("Nessun candidato (prova a togliere «solo senza raccolta»)")
          : t("Nessun candidato con un vettore semantico");
    } catch (e) {
      if (epoch === suggEpoch && sel === target) {
        msg = "" + e;
        sugg = null;
      }
    } finally {
      if (epoch === suggEpoch) suggLoading = false;
    }
  }
  async function addSuggestion(s: CollectionSuggestion) {
    if (typeof sel !== "number") return;
    const target = sel;
    try {
      await addDocumentsToCollection(target, [s.id]);
      // Se nel frattempo hai cambiato raccolta, non riaprire il pannello altrove.
      if (sel === target && sugg != null) sugg = sugg.filter((x) => x.id !== s.id);
      await refresh();
      notifyChanged();
    } catch (e) {
      msg = "" + e;
    }
  }
  async function addAllSuggestions() {
    if (typeof sel !== "number" || suggVisible.length === 0) return;
    const target = sel;
    const targetName = selNode?.name ?? "";
    suggAdding = true;
    try {
      const ids = suggVisible.map((s) => s.id);
      await addDocumentsToCollection(target, ids);
      if (sel === target && sugg != null) sugg = sugg.filter((x) => !ids.includes(x.id));
      msg = tp(ids.length, "1 paper aggiunto a «{raccolta}»", "{n} paper aggiunti a «{raccolta}»", { raccolta: targetName });
      await refresh();
      notifyChanged();
    } catch (e) {
      msg = "" + e;
    } finally {
      suggAdding = false;
    }
  }
  function fmtSugg(s: CollectionSuggestion): string {
    const testa = [s.lead_author, s.year].filter(Boolean).join(" ");
    const titolo = s.title ?? t("Senza titolo");
    return testa ? t("{testa} — {titolo}", { testa, titolo }) : titolo;
  }

  function notifyChanged() {
    try {
      onChanged?.();
    } catch {
      /* la sidebar non deve mai rompere l'Archivio */
    }
  }

  // ---- operazioni raccolte ----------------------------------------------------
  async function doCreate() {
    const name = createVal.trim();
    if (!name) return;
    try {
      const parent = creating === "child" && typeof sel === "number" ? sel : null;
      await createCollection(name, false, null, parent);
      creating = null;
      createVal = "";
      await loadTree();
      notifyChanged();
      msg = t("Raccolta «{nome}» creata", { nome: name });
    } catch (e) {
      msg = "" + e;
    }
  }
  async function doRename() {
    if (typeof sel !== "number") return;
    const name = renameVal.trim();
    if (!name) return;
    try {
      await renameCollection(sel, name);
      renaming = false;
      await loadTree();
      notifyChanged();
    } catch (e) {
      msg = "" + e;
    }
  }
  async function doDelete() {
    if (typeof sel !== "number") return;
    try {
      await deleteCollectionRehome(sel);
      confirmDel = false;
      sel = "unfiled";
      await refresh();
      notifyChanged();
      msg = t("Raccolta eliminata (sotto-raccolte risalite, nessun paper perso)");
    } catch (e) {
      msg = "" + e;
    }
  }

  // ---- macchina dei trascinamenti (pointer events, condivisa) -----------------
  function hitNode(e: PointerEvent): LNode | null {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    const g = el?.closest?.("[data-cid]") as HTMLElement | null;
    if (!g) return null;
    const v = g.dataset.cid!;
    const key: SelKey = v === "unfiled" ? "unfiled" : Number(v);
    return layout.nodes.find((n) => n.key === key) ?? null;
  }
  function hitBackground(e: PointerEvent): boolean {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    return !!(el?.closest?.(".treewrap"));
  }
  function cleanupWindowListeners(move: (e: PointerEvent) => void, up: (e: PointerEvent) => void, cancel: (e: PointerEvent) => void) {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", cancel);
  }

  // -- trascinamento NODI (ri-annidamento) --
  let pdown: { id: number; name: string; x: number; y: number } | null = null;
  // `tgt` e non `t`: `t` è la funzione di traduzione importata.
  function nodeTargetValid(dragId: number, tgt: SelKey | "root" | null): boolean {
    if (tgt === "root") return true;
    if (tgt == null || tgt === "unfiled") return false;
    if (tgt === dragId) return false;
    const n = layout.nodes.find((x) => x.key === tgt);
    if (!n || n.smart) return false;
    return !isDescendantOf(tgt, dragId);
  }
  function nodePointerDown(e: PointerEvent, n: LNode) {
    if (e.button !== 0 || n.id == null) {
      if (n.id == null && e.button === 0) select("unfiled");
      return;
    }
    pdown = { id: n.id, name: n.name, x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", nodePointerMove);
    window.addEventListener("pointerup", nodePointerUp);
    window.addEventListener("pointercancel", nodePointerCancel);
  }
  function nodePointerMove(e: PointerEvent) {
    if (!pdown) return;
    if (!nodeDrag) {
      if (Math.hypot(e.clientX - pdown.x, e.clientY - pdown.y) < 7) return;
      nodeDrag = { id: pdown.id, name: pdown.name, x: e.clientX, y: e.clientY, target: null, valid: false };
    }
    const n = hitNode(e);
    const tgt: SelKey | "root" | null = n ? n.key : hitBackground(e) ? "root" : null;
    nodeDrag.x = e.clientX;
    nodeDrag.y = e.clientY;
    nodeDrag.target = tgt;
    nodeDrag.valid = nodeTargetValid(nodeDrag.id, tgt);
  }
  function nodePointerCancel() {
    cleanupWindowListeners(nodePointerMove, nodePointerUp, nodePointerCancel);
    pdown = null;
    nodeDrag = null;
  }
  async function nodePointerUp(e: PointerEvent) {
    cleanupWindowListeners(nodePointerMove, nodePointerUp, nodePointerCancel);
    const start = pdown;
    const wasDragging = nodeDrag != null;
    pdown = null;
    nodeDrag = null;
    if (!start) return;
    if (!wasDragging) {
      select(start.id);
      return;
    }
    const n = hitNode(e);
    const tgt: SelKey | "root" | null = n ? n.key : hitBackground(e) ? "root" : null;
    if (!nodeTargetValid(start.id, tgt)) return;
    try {
      if (tgt === "root") {
        await moveCollection(start.id, null);
        msg = t("«{nome}» portata alla radice", { nome: start.name });
      } else {
        await moveCollection(start.id, tgt as number);
        msg = t("«{nome}» annidata", { nome: start.name });
      }
      await loadTree();
      notifyChanged();
    } catch (err) {
      msg = "" + err;
    }
  }

  // -- trascinamento PAPER (assegnazione) --
  let ddown: { id: number; label: string; x: number; y: number } | null = null;
  function docTargetValid(from: number | null, tgt: SelKey | "root" | null): boolean {
    if (tgt == null) return false;
    // Sfondo vuoto o nodo «SENZA RACCOLTA»: stesso gesto — togli il paper
    // dalla raccolta di provenienza (torna fra i senza-raccolta se non è altrove).
    if (tgt === "unfiled" || tgt === "root") return from != null;
    const n = layout.nodes.find((x) => x.key === tgt);
    if (!n || n.smart) return false;
    return tgt !== from;
  }
  function docPointerDown(e: PointerEvent, d: DocumentItem) {
    if (e.button !== 0) return;
    ddown = { id: d.id, label: fmtDoc(d), x: e.clientX, y: e.clientY };
    window.addEventListener("pointermove", docPointerMove);
    window.addEventListener("pointerup", docPointerUp);
    window.addEventListener("pointercancel", docPointerCancel);
  }
  function docPointerMove(e: PointerEvent) {
    if (!ddown) return;
    if (!docDrag) {
      if (Math.hypot(e.clientX - ddown.x, e.clientY - ddown.y) < 7) return;
      // Da una raccolta smart non c'è appartenenza da togliere: sorgente = null
      // (il trascinamento diventa un puro «aggiungi altrove»).
      const from = sel === "unfiled" || selNode?.smart ? null : (sel as number);
      docDrag = { ids: [ddown.id], from, label: ddown.label, x: e.clientX, y: e.clientY, target: null, valid: false };
    }
    const n = hitNode(e);
    docDrag.x = e.clientX;
    docDrag.y = e.clientY;
    docDrag.target = n ? n.key : hitBackground(e) ? "root" : null;
    docDrag.valid = docTargetValid(docDrag.from, docDrag.target);
  }
  function docPointerCancel() {
    cleanupWindowListeners(docPointerMove, docPointerUp, docPointerCancel);
    ddown = null;
    docDrag = null;
  }
  async function docPointerUp(e: PointerEvent) {
    cleanupWindowListeners(docPointerMove, docPointerUp, docPointerCancel);
    const drag = docDrag;
    ddown = null;
    docDrag = null;
    if (!drag) return; // click semplice sulla riga: nessuna azione
    const n = hitNode(e);
    const tgt: SelKey | "root" | null = n ? n.key : hitBackground(e) ? "root" : null;
    if (!docTargetValid(drag.from, tgt)) return;
    try {
      if (tgt === "unfiled" || tgt === "root") {
        await removeDocumentsFromCollection(drag.from as number, drag.ids);
        msg = t("Tolto dalla raccolta");
      } else if (drag.from == null || e.ctrlKey) {
        await addDocumentsToCollection(tgt as number, drag.ids);
        msg = e.ctrlKey && drag.from != null ? t("Aggiunto anche qui (appartenenza multipla)") : t("Aggiunto alla raccolta");
      } else {
        await moveDocumentsToCollection(drag.from, tgt as number, drag.ids);
        msg = t("Spostato");
      }
      await refresh();
      notifyChanged();
    } catch (err) {
      msg = "" + err;
    }
  }

  function fmtDoc(d: DocumentItem): string {
    const primo = d.authors?.[0];
    const testa = primo
      ? [d.authors.length > 1 ? t("{autori} et al.", { autori: primo }) : primo, d.year].filter(Boolean).join(SEP)
      : "";
    const titolo = d.title ?? t("Senza titolo");
    return testa ? t("{testa} — {titolo}", { testa, titolo }) : titolo;
  }
</script>

<div class="arch">
  <header>
    <div class="brand"><span class="t1">{t("ARCHIVIO")}</span><span class="t2">{t("// RACCOLTE")}</span></div>
    <div class="chips">
      {#if tree}
        <span class="chip">{t("{n} PAPER", { n: tree.total })}</span>
        <span class="chip">{t("{n} RACCOLTE", { n: tree.collections.length })}</span>
        <span class="chip">{t("{n} SENZA RACCOLTA", { n: tree.unfiled })}</span>
      {/if}
      {#if creating === "root"}
        <input
          class="mkinput"
          placeholder={t("nome della raccolta…")}
          bind:value={createVal}
          onkeydown={(e) => { if (e.key === "Enter") void doCreate(); if (e.key === "Escape") creating = null; }}
        />
        <button class="chip act" onclick={doCreate}>{t("CREA")}</button>
        <button class="chip" onclick={() => (creating = null)}>{t("ANNULLA")}</button>
      {:else}
        <button class="chip act" onclick={() => { creating = "root"; createVal = ""; }}>{t("+ NUOVA RACCOLTA")}</button>
      {/if}
      <button
        class="chip"
        class:act={mirror?.enabled}
        onclick={toggleMirror}
        disabled={mirrorBusy}
        title={mirror?.enabled
          ? t("Specchio attivo in {cartella} — clic per spegnerlo", { cartella: mirror.dir })
          : t("Proietta le raccolte in una cartella leggibile (hardlink: zero spazio extra, i file veri non si toccano)")}
      >
        {t("SPECCHIO")} {mirror?.enabled ? "●" : "○"}
      </button>
      {#if mirror?.enabled}
        <button class="chip" onclick={regenMirror} disabled={mirrorBusy} title={t("Ricostruisci ora l'albero su disco")}>{mirrorBusy ? "…" : t("RIGENERA")}</button>
        <button class="chip" onclick={() => void mirrorReveal()} title={t("Apri la cartella dello specchio")}>{t("APRI")}</button>
      {/if}
    </div>
  </header>
  <!-- I <b> spezzano la frase in più nodi di testo: ogni tratto è una chiave a sé,
       tagliata dove anche l'inglese può reggere il pezzo da solo.
       i18n-exempt: «Ctrl» è il nome di un tasto, non una parola. -->
  <div class="hint">
    {t("Trascina un")} <b>{t("paper|trascinamento")}</b>
    {t("(dall'elenco a destra) su una raccolta per spostarlo —")} <b>Ctrl</b>
    {t("= aggiungi anche lì, l'appartenenza è multipla;")} <b>{t("sullo sfondo vuoto")}</b>
    {t("= toglilo dalla raccolta · trascina una")} <b>{t("raccolta|trascinamento")}</b>
    {t("su un'altra per annidarla, sullo sfondo per riportarla alla radice")}
  </div>
  {#if msg}<div class="msg">{msg}</div>{/if}

  <div class="body">
    <div class="treewrap">
      <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
      <svg
        class="archsvg"
        bind:this={treeEl}
        width={layout.w}
        height={layout.h}
        role="tree"
        tabindex="0"
        aria-label={t("Raccolte — frecce per spostarti, Invio per aprire nella griglia")}
        aria-activedescendant={sel != null ? "arcn-" + sel : undefined}
        onkeydown={treeKey}
      >
        <text class="zone" x={PADX} y="34">{t("GERARCHIA")}</text>
        {#each layout.edges as e (e.from.key + ">" + e.to.key)}
          <path class="trace" d={edgePath(e)} />
        {/each}
        <!-- Il nodo del ciclo si chiama `nd`, non `n`: il segnaposto iniettato da
             `tp` si chiama proprio n, e la collisione sarebbe silenziosa. -->
        {#each layout.nodes as nd (nd.key)}
          {@const isTarget =
            (docDrag != null && docDrag.target === nd.key && docDrag.valid) ||
            (nodeDrag != null && nodeDrag.target === nd.key && nodeDrag.valid)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <g
            class="node"
            class:sel={sel === nd.key}
            class:smart={nd.smart}
            class:unfiled={nd.key === "unfiled"}
            class:drophover={isTarget}
            data-cid={nd.key}
            id={"arcn-" + nd.key}
            role="treeitem"
            tabindex={-1}
            aria-level={nd.depth + 1}
            aria-selected={sel === nd.key}
            aria-label={tp(nd.count, "{nome} — 1 paper", "{nome} — {n} paper", { nome: nd.name })}
            onpointerdown={(e) => nodePointerDown(e, nd)}
          >
            <rect x={nd.x} y={nd.y} width={W} height={H} rx="6" />
            <rect class="pin" x={nd.x - 4} y={nd.y + H / 2 - 7} width="4" height="14" rx="1" />
            <text class="nlabel" x={nd.x + 12} y={nd.y + 20}>{nd.name.length > 22 ? nd.name.slice(0, 21) + "…" : nd.name}</text>
            <text class="nsub" x={nd.x + 12} y={nd.y + 36}>
              {nd.smart
                ? tp(nd.count, "1 paper · SMART (si popola da sola)", "{n} paper · SMART (si popola da sola)")
                : tp(nd.count, "1 paper", "{n} paper")}
            </text>
            <text class="ncount" x={nd.x + W - 12} y={nd.y + 20}>{nd.count}</text>
            {#if nd.watch}
              <text class="nbell" x={nd.x + W - 12} y={nd.y + 38}>◉</text>
            {/if}
          </g>
        {/each}
      </svg>
    </div>

    <aside class="panel">
      {#if selNode}
        <div class="phead">
          <span class="ptitle">{selNode.key === "unfiled" ? t("SENZA RACCOLTA") : selNode.name}</span>
        </div>
        {#if breadcrumb.length > 1}
          <!-- i18n-exempt: nomi di raccolte dell'utente uniti da un separatore neutro -->
          <div class="crumb">{breadcrumb.join(" / ")}</div>
        {/if}
        <div class="pstats">
          <span>{t("diretti {n}", { n: selNode.count })}</span>
          {#if selNode.key !== "unfiled" && selTotal !== selNode.count}<span>{t("appartenenze nel ramo {n}", { n: selTotal })}</span>{/if}
          {#if selNode.smart}<span class="smartbadge">{t("SMART — si popola da sola")}</span>{/if}
        </div>

        {#if selNode.key !== "unfiled"}
          <div class="pacts">
            <button class="pbtn" onclick={() => onOpenGrid(selNode.id as number, selNode.name)}>{t("APRI NELLA GRIGLIA")}</button>
            {#if onOpenGraph}
              <button class="pbtn" onclick={() => onOpenGraph?.(selNode.id as number, selNode.name)} title={t("La Costellazione ristretta a questa raccolta (vicinanze ricalcolate al suo interno)")}>{t("COSTELLAZIONE")}</button>
            {/if}
            {#if renaming}
              <input
                class="mkinput"
                bind:value={renameVal}
                onkeydown={(e) => { if (e.key === "Enter") void doRename(); if (e.key === "Escape") renaming = false; }}
              />
              <button class="pbtn" onclick={doRename}>{t("SALVA")}</button>
            {:else}
              <button class="pbtn" onclick={() => { renaming = true; renameVal = selNode.name; }}>{t("RINOMINA")}</button>
            {/if}
            {#if creating === "child"}
              <input
                class="mkinput"
                placeholder={t("nome sotto-raccolta…")}
                bind:value={createVal}
                onkeydown={(e) => { if (e.key === "Enter") void doCreate(); if (e.key === "Escape") creating = null; }}
              />
              <button class="pbtn" onclick={doCreate}>{t("CREA")}</button>
            {:else if !selNode.smart}
              <button class="pbtn" onclick={() => { creating = "child"; createVal = ""; }}>{t("+ SOTTO-RACCOLTA")}</button>
            {/if}
            {#if confirmDel}
              <button class="pbtn danger" onclick={doDelete}>{t("CONFERMI L'ELIMINAZIONE?")}</button>
              <button class="pbtn" onclick={() => (confirmDel = false)}>{t("NO")}</button>
            {:else}
              <button class="pbtn danger" onclick={() => (confirmDel = true)}>{t("ELIMINA")}</button>
            {/if}
          </div>
          <div class="pnote">{t("Eliminare una raccolta non tocca i paper: le sotto-raccolte risalgono di un livello.")}</div>

          <div class="notesec">
            <span class="wlbl">{t("APPUNTI")}</span>
            {#if collNotes.length}
              <ul class="notelist">
                {#each collNotes as nt (nt.slug)}
                  <li>
                    <button class="notelink" onclick={() => onOpenNote?.(nt.slug)} title={t("Apri questo appunto")}>
                      {nt.title}
                    </button>
                    <button
                      class="noteoff"
                      onclick={() => void unlinkNote(nt.slug)}
                      title={t("Togli l'appunto da questa raccolta (il file .md non viene toccato)")}
                      aria-label={t("Togli da questa raccolta")}
                    >×</button>
                  </li>
                {/each}
              </ul>
            {:else}
              <div class="pnote">{t("Nessun appunto agganciato. Un appunto che discute questi paper si trova qui, invece che in fondo a un elenco.")}</div>
            {/if}
            {#if creatingNote}
              <input
                class="mkinput"
                placeholder={t("titolo dell'appunto…")}
                bind:value={noteTitleVal}
                onkeydown={(e) => { if (e.key === "Enter") void doCreateNote(); if (e.key === "Escape") creatingNote = false; }}
              />
              <button class="pbtn" onclick={doCreateNote} disabled={noteBusy}>{noteBusy ? t("CREO…") : t("CREA")}</button>
            {:else}
              <!-- L'etichetta È la divulgazione: dice quanti paper finiranno
                   nell'appunto, così non c'è nessuna soglia invisibile da indovinare. -->
              <button
                class="pbtn"
                disabled={loadingDocs || noteBusy}
                onclick={() => { creatingNote = true; noteTitleVal = selNode?.name ?? ""; }}
                title={t("Crea un appunto .md già agganciato a questa raccolta, con i suoi paper elencati come citazioni")}
              >
                {docs.length
                  ? tp(docs.length, "+ APPUNTO CON 1 PAPER", "+ APPUNTO CON I {n} PAPER")
                  : t("+ NUOVO APPUNTO")}
              </button>
            {/if}
          </div>

          {#if !selNode.smart}
            <div class="watchrow">
              <span class="wlbl">{t("RICERCA «NOVITÀ»")}</span>
              <button
                class="pbtn"
                class:won={selNode.watch}
                onclick={async () => {
                  if (typeof sel !== "number") return;
                  try {
                    await setCollectionWatch(sel, !selNode.watch);
                    await loadTree();
                    notifyChanged();
                  } catch (e) {
                    msg = "" + e;
                  }
                }}
              >
                {selNode.watch ? t("ATTIVA ●") : t("SPENTA ○")}
              </button>
            </div>
            <!-- Anche qui i <b> spezzano la frase: un tratto per ogni pezzo, tagliato
                 su confini di proposizione. -->
            <div class="pnote">
              {t("Con la ricerca attiva, a ogni avvio Scriptorium cerca online nuovi paper per questa raccolta (query = il nome; puoi raffinarla fra le ricerche salvate di «Novità»). Le novità accettate dal feed")}
              <b>{t("entrano già nella raccolta")}</b>{t("; con ≥3 paper indicizzati i risultati sono filtrati per somiglianza semantica. Spegnendola, la ricerca (e il suo feed)")}
              <b>{t("viene rimossa")}</b>
              {t("dalle ricerche salvate.")}
            </div>
          {/if}

          {#if !selNode.smart}
            <div class="suggbox">
              <div class="sugghead">
                <span>{suggLoading
                    ? t("✦ SUGGERIMENTI · CALCOLO…")
                    : sugg != null
                      ? t("✦ SUGGERIMENTI ({n} sopra soglia)", { n: suggVisible.length })
                      : t("✦ SUGGERIMENTI")}</span>
                {#if sugg != null}
                  <button class="pclosemini" onclick={() => (sugg = null)}>✕</button>
                {/if}
              </div>
              <!-- La SORGENTE si sceglie PRIMA di calcolare: sempre visibile. -->
              <div class="suggctl">
                <!-- i18n-exempt: "name"/"content"/"both" sono valori persistiti in
                     localStorage e attesi dal backend; si traducono solo le etichette. -->
                <span class="modechips">
                  <button class="lfm" class:active={suggMode === "name"} disabled={suggLoading} onclick={() => setSuggMode("name")} title={t("Somiglianza col solo NOME della raccolta (utile su raccolte nuove dal titolo parlante)")}>{t("NOME")}</button>
                  <button class="lfm" class:active={suggMode === "content"} disabled={suggLoading || selNode.count === 0} onclick={() => setSuggMode("content")} title={selNode.count === 0 ? t("Serve almeno un paper nella raccolta") : t("Somiglianza col solo CONTENUTO (centroide dei paper già dentro) — ignora il nome")}>{t("CONTENUTO")}</button>
                  <button class="lfm" class:active={suggMode === "both"} disabled={suggLoading || selNode.count === 0} onclick={() => setSuggMode("both")} title={selNode.count === 0 ? t("Serve almeno un paper nella raccolta") : t("Miscela nome+contenuto, col peso qui sotto")}>{t("ENTRAMBI")}</button>
                </span>
                {#if suggMode === "both" && selNode.count > 0}
                  <label class="sldlbl" title={t("Quanto pesa il CONTENUTO nella miscela (il resto è il nome)")}>
                    {t("contenuto {a}% · nome {b}%", { a: suggWeight, b: 100 - suggWeight })}
                    <input type="range" min="0" max="100" step="5" bind:value={suggWeight} disabled={suggLoading} onchange={saveSuggWeight} />
                  </label>
                {/if}
                <label class="chklbl">
                  <input type="checkbox" bind:checked={suggUnfiled} disabled={suggLoading} onchange={() => { if (sugg != null) void loadSuggestions(); }} />
                  {t("solo senza raccolta")}
                </label>
              </div>
              {#if sugg == null}
                <button class="pbtn wide" onclick={loadSuggestions} disabled={suggLoading}>
                  {suggLoading ? t("CALCOLO…") : t("CALCOLA I SUGGERIMENTI")}
                </button>
              {:else}
                <div class="suggctl">
                  <label class="sldlbl">
                    {t("confidenza ≥ {soglia}%", { soglia: suggThreshold })}
                    <input type="range" min="30" max="90" step="1" bind:value={suggThreshold} />
                  </label>
                  <button class="pbtn" onclick={addAllSuggestions} disabled={suggAdding || suggVisible.length === 0}>
                    {suggAdding ? t("AGGIUNGO…") : t("AGGIUNGI TUTTI ({n})", { n: suggVisible.length })}
                  </button>
                </div>
                <div class="sugglist">
                  {#each suggVisible as s (s.id)}
                    <div class="srow" title={fmtSugg(s)}>
                      <button class="saddbtn" onclick={() => addSuggestion(s)} title={t("Aggiungi alla raccolta")}>+</button>
                      <span class="sbar"><span class="sfill" style="width:{Math.round(s.score * 100)}%"></span></span>
                      <span class="spct">{Math.round(s.score * 100)}</span>
                      <span class="stxt">{fmtSugg(s)}</span>
                    </div>
                  {:else}
                    <div class="srow empty">{t("Niente sopra il {soglia}% — abbassa la soglia.", { soglia: suggThreshold })}</div>
                  {/each}
                </div>
                <div class="pnote">
                  {t("Punteggio = somiglianza semantica (bge-m3, in locale) con la sorgente scelta sopra: il")}
                  <b>{t("nome|sorgente dei suggerimenti")}</b>
                  {t("della raccolta, il suo")}
                  <b>{t("contenuto|sorgente dei suggerimenti")}</b>
                  {t("(i paper già dentro — più ne aggiungi, meglio va; niente rete né Ollama), o la miscela col peso che preferisci. Nulla viene aggiunto senza il tuo clic.")}
                </div>
              {/if}
            </div>
          {/if}
        {:else}
          <div class="pnote">{t("I paper non ancora archiviati. Trascinali su una raccolta a sinistra; trascinare qui un paper (da una raccolta) lo toglie da quella raccolta.")}</div>
        {/if}

        <div class="dochead">{loadingDocs ? t("PAPER …") : t("PAPER ({n})", { n: docs.length })}</div>
        <div class="doclist">
          {#each docs as d (d.id)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="drow"
              class:dragging={docDrag != null && docDrag.ids[0] === d.id}
              onpointerdown={(e) => docPointerDown(e, d)}
              title={t("{documento} — trascinami su una raccolta", { documento: fmtDoc(d) })}
            >
              <span class="grip">⣿</span>
              <span class="dtxt">{fmtDoc(d)}</span>
            </div>
          {:else}
            <div class="drow empty">{loadingDocs ? t("Carico…") : t("Nessun paper qui.")}</div>
          {/each}
        </div>
      {/if}
    </aside>
  </div>

  {#if nodeDrag}
    <div class="ghost" style="left:{nodeDrag.x + 12}px; top:{nodeDrag.y + 10}px;">
      {nodeDrag.name}
      <span class="gsub">{nodeDrag.valid ? (nodeDrag.target === "root" ? t("→ radice") : t("→ sotto-raccolta")) : "…"}</span>
    </div>
  {/if}
  {#if docDrag}
    <div class="ghost" style="left:{docDrag.x + 12}px; top:{docDrag.y + 10}px;">
      {docDrag.label.length > 48 ? docDrag.label.slice(0, 47) + "…" : docDrag.label}
      <span class="gsub">{docDrag.valid ? (docDrag.target === "unfiled" || docDrag.target === "root" ? t("→ togli dalla raccolta") : t("→ qui")) : "…"}</span>
    </div>
  {/if}
</div>

<style>
  .arch {
    height: calc(100vh - 148px);
    min-height: 420px;
    display: flex;
    flex-direction: column;
    background:
      var(--surface);
    color: var(--text);
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    border-radius: 8px;
    overflow: hidden;
    user-select: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 14px 6px;
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }
  .brand .t1 { font-size: 15px; letter-spacing: 0.32em; color: var(--accent); text-shadow: 0 0 12px rgba(79, 227, 255, 0.5); }
  .brand .t2 { margin-left: 8px; font-size: 10px; letter-spacing: 0.22em; color: var(--faint); }
  .chips { display: flex; gap: 7px; align-items: center; flex-wrap: wrap; }
  .chip {
    font-size: 10px;
    letter-spacing: 0.12em;
    padding: 3px 8px;
    border-radius: 3px;
    border: 1px solid var(--border);
    color: var(--dim);
    background: none;
    font-family: inherit;
    white-space: nowrap;
  }
  button.chip { cursor: pointer; }
  .chip.act { color: var(--accent); }
  .chip.act:hover { background: var(--border); }
  .mkinput {
    background: var(--field);
    border: 1px solid var(--border);
    color: var(--text);
    font-family: inherit;
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 3px;
    min-width: 170px;
  }
  .hint {
    flex: none;
    padding: 5px 14px 0;
    font-size: 10px;
    color: var(--faint);
  }
  .hint b { color: var(--dim); }
  .msg { flex: none; padding: 4px 14px 0; font-size: 11px; color: #ffd166; }

  .body { flex: 1; display: flex; min-height: 0; }
  .treewrap { flex: 1; overflow: auto; min-width: 0; }
  .archsvg { display: block; }
  .archsvg:focus { outline: none; }
  /* Con la tastiera il nodo scelto deve saltare all'occhio: il solo stile .sel
     nasce per il clic (dove sai già dove hai cliccato). */
  .archsvg:focus-visible .node.sel rect:first-of-type { stroke: var(--accent); stroke-width: 2.5; }
  .zone { font-size: 10px; letter-spacing: 0.4em; fill: var(--border); }
  .trace { fill: none; stroke: var(--border); stroke-width: 1.5; }

  .node { cursor: pointer; }
  .node rect:not(.pin) {
    fill: var(--field);
    stroke: var(--border);
    stroke-width: 1.3;
  }
  .node .pin { fill: var(--border); }
  .node .nlabel { font-size: 12px; font-weight: 700; letter-spacing: 0.08em; fill: var(--text); pointer-events: none; }
  .node .nsub { font-size: 9px; letter-spacing: 0.06em; fill: var(--dim); pointer-events: none; }
  .node .ncount { font-size: 11px; text-anchor: end; fill: var(--accent); pointer-events: none; }
  .node.sel rect:not(.pin) { stroke: var(--accent); stroke-width: 2.2; filter: drop-shadow(0 0 8px rgba(55, 224, 255, 0.4)); }
  .node.unfiled rect:not(.pin) { stroke-dasharray: 5 4; }
  .node.smart rect:not(.pin) { stroke: rgba(255, 209, 102, 0.5); }
  .node.smart .nsub { fill: rgba(255, 209, 102, 0.7); }
  .node.drophover rect:not(.pin) {
    stroke: #6ef0c0;
    stroke-width: 2.4;
    fill: rgba(9, 50, 40, 0.9);
    filter: drop-shadow(0 0 10px rgba(110, 240, 192, 0.5));
  }

  .panel {
    width: 380px;
    flex: none;
    border-left: 1px solid var(--border);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    min-height: 0;
  }
  .phead .ptitle { font-size: 14px; font-weight: 700; letter-spacing: 0.18em; color: var(--accent); overflow-wrap: anywhere; }
  .crumb { font-size: 9px; letter-spacing: 0.08em; color: var(--faint); margin-top: 3px; }
  .pstats {
    display: flex; gap: 12px; flex-wrap: wrap;
    font-size: 10px; color: var(--dim);
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    padding: 7px 0; margin: 8px 0;
  }
  /* Il <b> attorno al numero è stato tolto: la frase «diretti {n}» è una chiave
     sola, perché in inglese il numero precede la parola. */
  .smartbadge { color: #ffd166; }
  .pacts { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 6px; }
  .pbtn {
    background: none;
    border: 1px solid var(--border);
    color: var(--text);
    font-family: inherit;
    font-size: 9px;
    letter-spacing: 0.16em;
    padding: 4px 9px;
    border-radius: 3px;
    cursor: pointer;
  }
  .pbtn:hover { color: var(--accent); border-color: rgba(79, 227, 255, 0.6); }
  .pbtn.danger { color: #ff8b94; border-color: rgba(255, 89, 100, 0.4); }
  .pbtn.danger:hover { color: #ff5964; border-color: rgba(255, 89, 100, 0.7); }
  .pnote { font-size: 10px; line-height: 1.5; color: var(--faint); margin-bottom: 8px; }
  /* Appunti agganciati: stessa grammatica visiva del blocco «Novità» qui sotto
     (etichetta a tutto-maiuscolo + contenuto), così non sembra un innesto. */
  .notesec { display: flex; flex-direction: column; gap: 6px; margin: 10px 0 12px; }
  .notelist { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  .notelist li { display: flex; align-items: center; gap: 4px; }
  .notelink {
    flex: 1; min-width: 0; text-align: left;
    background: none; border: none; padding: 3px 0; cursor: pointer;
    color: var(--text); font-size: 11.5px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .notelink:hover { color: var(--accent); text-decoration: underline; }
  .noteoff {
    background: none; border: none; cursor: pointer; padding: 0 4px;
    color: var(--faint); font-size: 13px; line-height: 1; flex: 0 0 auto;
  }
  .noteoff:hover { color: var(--danger); }

  .nbell { font-size: 10px; text-anchor: end; fill: #ffd166; pointer-events: none; }
  .watchrow { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
  .wlbl { font-size: 9px; letter-spacing: 0.24em; color: var(--border); }
  .pbtn.won { color: #ffd166; border-color: rgba(255, 209, 102, 0.5); }

  /* ---- suggerimenti ---- */
  .suggbox {
    border-top: 1px solid var(--border);
    padding-top: 8px;
    margin-bottom: 8px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .pbtn.wide { width: 100%; text-align: center; color: var(--accent); }
  .sugghead {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 9px;
    letter-spacing: 0.3em;
    color: var(--border);
    margin-bottom: 5px;
  }
  .pclosemini { background: none; border: none; color: var(--dim); cursor: pointer; font-family: inherit; }
  .pclosemini:hover { color: var(--accent); }
  .suggctl { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-bottom: 6px; }
  .modechips { display: flex; gap: 4px; }
  .lfm {
    background: none;
    border: 1px solid var(--border);
    color: var(--dim);
    font-family: inherit;
    font-size: 9px;
    letter-spacing: 0.14em;
    padding: 2px 7px;
    border-radius: 3px;
    cursor: pointer;
  }
  .lfm.active { color: var(--accent); border-color: rgba(79, 227, 255, 0.6); }
  .lfm:disabled { opacity: 0.4; cursor: default; }
  .sldlbl { font-size: 9px; letter-spacing: 0.1em; color: var(--dim); display: flex; align-items: center; gap: 6px; }
  /* Idem per le percentuali: chiave unica, niente <b> interno. */
  .sldlbl input[type="range"] { width: 110px; accent-color: var(--accent); }
  .chklbl { font-size: 9px; letter-spacing: 0.1em; color: var(--dim); display: flex; align-items: center; gap: 4px; }
  .sugglist { overflow-y: auto; max-height: 220px; min-height: 0; }
  .srow {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    line-height: 1.6;
    padding: 2px 2px;
  }
  .srow:hover { background: var(--accent-soft); }
  .srow.empty { color: var(--faint); font-style: italic; }
  .saddbtn {
    background: none;
    border: 1px solid rgba(110, 240, 192, 0.4);
    color: #6ef0c0;
    font-family: inherit;
    font-size: 11px;
    line-height: 1;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    cursor: pointer;
    flex: none;
  }
  .saddbtn:hover { background: rgba(110, 240, 192, 0.15); }
  .sbar { width: 46px; height: 5px; background: var(--border); border-radius: 2px; flex: none; overflow: hidden; }
  .sfill { display: block; height: 100%; background: linear-gradient(90deg, #1b8aa8, var(--accent)); }
  .spct { color: var(--accent); min-width: 20px; text-align: right; flex: none; }
  .stxt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text); }
  .dochead { font-size: 9px; letter-spacing: 0.3em; color: var(--border); margin: 4px 0; }
  .doclist { overflow-y: auto; min-height: 0; flex: 1; }
  .drow {
    display: flex;
    gap: 8px;
    align-items: baseline;
    font-size: 11px;
    line-height: 1.5;
    padding: 3px 4px;
    border-radius: 3px;
    color: var(--text);
    cursor: grab;
    touch-action: none;
  }
  .drow:hover { background: var(--accent-soft); }
  .drow.dragging { opacity: 0.45; }
  .drow .grip { color: var(--border); font-size: 9px; flex: none; }
  .drow .dtxt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .drow.empty { color: var(--faint); font-style: italic; cursor: default; }

  .ghost {
    position: fixed;
    z-index: 300;
    pointer-events: none;
    background: rgba(9, 45, 62, 0.95);
    border: 1px solid var(--accent);
    color: var(--text);
    font-family: ui-monospace, Consolas, monospace;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 4px;
    box-shadow: 0 0 14px rgba(55, 224, 255, 0.4);
  }
  .ghost .gsub { margin-left: 8px; font-size: 9px; color: var(--accent); }
</style>
