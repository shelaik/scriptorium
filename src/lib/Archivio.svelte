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
  } from "$lib/api";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  let {
    onOpenGrid,
    onChanged,
  }: {
    onOpenGrid: (id: number, label: string) => void;
    /** Chiamata dopo ogni mutazione delle raccolte, così la sidebar resta fresca. */
    onChanged?: () => void;
  } = $props();

  type SelKey = number | "unfiled";

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
  let docDrag = $state<{ ids: number[]; from: number | null; label: string; x: number; y: number; target: SelKey | null; valid: boolean } | null>(null);
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
    const t = tree;
    if (!t) return { nodes, edges: [] as { from: LNode; to: LNode }[], w: 900, h: 400 };
    let row = 0;
    nodes.push({
      key: "unfiled", id: null, name: "SENZA RACCOLTA", count: t.unfiled,
      smart: false, watch: false, depth: 0, x: PADX, y: PADY + row++ * ROWH, parent: null,
    });
    const byParent = new Map<number | null, typeof t.collections>();
    for (const c of t.collections) {
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
      msg = "Errore albero: " + e;
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
        msg = "Errore documenti: " + e;
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
  });
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
        msg = "Specchio spento (la cartella resta com'è; i tuoi file veri non sono mai stati toccati)";
      } else {
        const dir = await openDialog({
          directory: true,
          title: "Scegli la cartella dello specchio (vuota o dedicata)",
          defaultPath: mirror?.dir || undefined,
        });
        if (typeof dir !== "string" || !dir) return;
        mirror = await setMirror(true, dir);
        msg = "Specchio attivo: genero l'albero in " + dir;
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
      msg = `Specchio rigenerato: ${s.linked} hardlink, ${s.copied} copie, ${s.folders} cartelle${s.missing ? `, ${s.missing} mancanti` : ""}`;
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
      if (out.length === 0) msg = suggUnfiled ? "Nessun candidato (prova a togliere «solo senza raccolta»)" : "Nessun candidato con un vettore semantico";
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
      msg = `${ids.length} paper aggiunti a «${targetName}»`;
      await refresh();
      notifyChanged();
    } catch (e) {
      msg = "" + e;
    } finally {
      suggAdding = false;
    }
  }
  function fmtSugg(s: CollectionSuggestion): string {
    const a = s.lead_author ? s.lead_author + " " : "";
    const y = s.year ? `${s.year} — ` : "";
    return a + y + (s.title ?? "Senza titolo");
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
      msg = `Raccolta «${name}» creata`;
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
      msg = "Raccolta eliminata (sotto-raccolte risalite, nessun paper perso)";
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
  function nodeTargetValid(dragId: number, t: SelKey | "root" | null): boolean {
    if (t === "root") return true;
    if (t == null || t === "unfiled") return false;
    if (t === dragId) return false;
    const n = layout.nodes.find((x) => x.key === t);
    if (!n || n.smart) return false;
    return !isDescendantOf(t, dragId);
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
    const t: SelKey | "root" | null = n ? n.key : hitBackground(e) ? "root" : null;
    nodeDrag.x = e.clientX;
    nodeDrag.y = e.clientY;
    nodeDrag.target = t;
    nodeDrag.valid = nodeTargetValid(nodeDrag.id, t);
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
    const t: SelKey | "root" | null = n ? n.key : hitBackground(e) ? "root" : null;
    if (!nodeTargetValid(start.id, t)) return;
    try {
      if (t === "root") {
        await moveCollection(start.id, null);
        msg = `«${start.name}» portata alla radice`;
      } else {
        await moveCollection(start.id, t as number);
        msg = `«${start.name}» annidata`;
      }
      await loadTree();
      notifyChanged();
    } catch (err) {
      msg = "" + err;
    }
  }

  // -- trascinamento PAPER (assegnazione) --
  let ddown: { id: number; label: string; x: number; y: number } | null = null;
  function docTargetValid(from: number | null, t: SelKey | null): boolean {
    if (t == null) return false;
    if (t === "unfiled") return from != null; // togli dalla raccolta di provenienza
    const n = layout.nodes.find((x) => x.key === t);
    if (!n || n.smart) return false;
    return t !== from;
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
    docDrag.target = n ? n.key : null;
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
    const t: SelKey | null = n ? n.key : null;
    if (!docTargetValid(drag.from, t)) return;
    try {
      if (t === "unfiled") {
        await removeDocumentsFromCollection(drag.from as number, drag.ids);
        msg = "Tolto dalla raccolta";
      } else if (drag.from == null || e.ctrlKey) {
        await addDocumentsToCollection(t as number, drag.ids);
        msg = e.ctrlKey && drag.from != null ? "Aggiunto anche qui (appartenenza multipla)" : "Aggiunto alla raccolta";
      } else {
        await moveDocumentsToCollection(drag.from, t as number, drag.ids);
        msg = "Spostato";
      }
      await refresh();
      notifyChanged();
    } catch (err) {
      msg = "" + err;
    }
  }

  function fmtDoc(d: DocumentItem): string {
    const a = d.authors?.[0] ? d.authors[0] + (d.authors.length > 1 ? " et al." : "") : "";
    const y = d.year ? ` · ${d.year}` : "";
    return (a ? a + y + " — " : "") + (d.title ?? "Senza titolo");
  }
</script>

<div class="arch">
  <header>
    <div class="brand"><span class="t1">ARCHIVIO</span><span class="t2">// RACCOLTE</span></div>
    <div class="chips">
      {#if tree}
        <span class="chip">{tree.total} PAPER</span>
        <span class="chip">{tree.collections.length} RACCOLTE</span>
        <span class="chip">{tree.unfiled} SENZA RACCOLTA</span>
      {/if}
      {#if creating === "root"}
        <input
          class="mkinput"
          placeholder="nome della raccolta…"
          bind:value={createVal}
          onkeydown={(e) => { if (e.key === "Enter") void doCreate(); if (e.key === "Escape") creating = null; }}
        />
        <button class="chip act" onclick={doCreate}>CREA</button>
        <button class="chip" onclick={() => (creating = null)}>ANNULLA</button>
      {:else}
        <button class="chip act" onclick={() => { creating = "root"; createVal = ""; }}>+ NUOVA RACCOLTA</button>
      {/if}
      <button
        class="chip"
        class:act={mirror?.enabled}
        onclick={toggleMirror}
        disabled={mirrorBusy}
        title={mirror?.enabled ? `Specchio attivo in ${mirror.dir} — clic per spegnerlo` : "Proietta le raccolte in una cartella leggibile (hardlink: zero spazio extra, i file veri non si toccano)"}
      >
        SPECCHIO {mirror?.enabled ? "●" : "○"}
      </button>
      {#if mirror?.enabled}
        <button class="chip" onclick={regenMirror} disabled={mirrorBusy} title="Ricostruisci ora l'albero su disco">{mirrorBusy ? "…" : "RIGENERA"}</button>
        <button class="chip" onclick={() => void mirrorReveal()} title="Apri la cartella dello specchio">APRI</button>
      {/if}
    </div>
  </header>
  <div class="hint">Trascina un <b>paper</b> (dall'elenco a destra) su una raccolta per spostarlo — <b>Ctrl</b> = aggiungi anche lì, l'appartenenza è multipla · trascina una <b>raccolta</b> su un'altra per annidarla, sullo sfondo per riportarla alla radice</div>
  {#if msg}<div class="msg">{msg}</div>{/if}

  <div class="body">
    <div class="treewrap">
      <svg class="archsvg" width={layout.w} height={layout.h}>
        <text class="zone" x={PADX} y="34">GERARCHIA</text>
        {#each layout.edges as e (e.from.key + ">" + e.to.key)}
          <path class="trace" d={edgePath(e)} />
        {/each}
        {#each layout.nodes as n (n.key)}
          {@const isTarget =
            (docDrag != null && docDrag.target === n.key && docDrag.valid) ||
            (nodeDrag != null && nodeDrag.target === n.key && nodeDrag.valid)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <g
            class="node"
            class:sel={sel === n.key}
            class:smart={n.smart}
            class:unfiled={n.key === "unfiled"}
            class:drophover={isTarget}
            data-cid={n.key}
            onpointerdown={(e) => nodePointerDown(e, n)}
          >
            <rect x={n.x} y={n.y} width={W} height={H} rx="6" />
            <rect class="pin" x={n.x - 4} y={n.y + H / 2 - 7} width="4" height="14" rx="1" />
            <text class="nlabel" x={n.x + 12} y={n.y + 20}>{n.name.length > 22 ? n.name.slice(0, 21) + "…" : n.name}</text>
            <text class="nsub" x={n.x + 12} y={n.y + 36}>
              {n.count} paper{n.smart ? " · SMART (si popola da sola)" : ""}
            </text>
            <text class="ncount" x={n.x + W - 12} y={n.y + 20}>{n.count}</text>
            {#if n.watch}
              <text class="nbell" x={n.x + W - 12} y={n.y + 38}>◉</text>
            {/if}
          </g>
        {/each}
      </svg>
    </div>

    <aside class="panel">
      {#if selNode}
        <div class="phead">
          <span class="ptitle">{selNode.key === "unfiled" ? "SENZA RACCOLTA" : selNode.name}</span>
        </div>
        {#if breadcrumb.length > 1}
          <div class="crumb">{breadcrumb.join(" / ")}</div>
        {/if}
        <div class="pstats">
          <span>diretti <b>{selNode.count}</b></span>
          {#if selNode.key !== "unfiled" && selTotal !== selNode.count}<span>appartenenze nel ramo <b>{selTotal}</b></span>{/if}
          {#if selNode.smart}<span class="smartbadge">SMART — si popola da sola</span>{/if}
        </div>

        {#if selNode.key !== "unfiled"}
          <div class="pacts">
            <button class="pbtn" onclick={() => onOpenGrid(selNode.id as number, selNode.name)}>APRI NELLA GRIGLIA</button>
            {#if renaming}
              <input
                class="mkinput"
                bind:value={renameVal}
                onkeydown={(e) => { if (e.key === "Enter") void doRename(); if (e.key === "Escape") renaming = false; }}
              />
              <button class="pbtn" onclick={doRename}>SALVA</button>
            {:else}
              <button class="pbtn" onclick={() => { renaming = true; renameVal = selNode.name; }}>RINOMINA</button>
            {/if}
            {#if creating === "child"}
              <input
                class="mkinput"
                placeholder="nome sotto-raccolta…"
                bind:value={createVal}
                onkeydown={(e) => { if (e.key === "Enter") void doCreate(); if (e.key === "Escape") creating = null; }}
              />
              <button class="pbtn" onclick={doCreate}>CREA</button>
            {:else if !selNode.smart}
              <button class="pbtn" onclick={() => { creating = "child"; createVal = ""; }}>+ SOTTO-RACCOLTA</button>
            {/if}
            {#if confirmDel}
              <button class="pbtn danger" onclick={doDelete}>CONFERMI L'ELIMINAZIONE?</button>
              <button class="pbtn" onclick={() => (confirmDel = false)}>NO</button>
            {:else}
              <button class="pbtn danger" onclick={() => (confirmDel = true)}>ELIMINA</button>
            {/if}
          </div>
          <div class="pnote">Eliminare una raccolta non tocca i paper: le sotto-raccolte risalgono di un livello.</div>

          {#if !selNode.smart}
            <div class="watchrow">
              <span class="wlbl">RICERCA «NOVITÀ»</span>
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
                {selNode.watch ? "ATTIVA ●" : "SPENTA ○"}
              </button>
            </div>
            <div class="pnote">
              Con la ricerca attiva, a ogni avvio Scriptorium cerca online nuovi paper per questa
              raccolta (query = il nome; puoi raffinarla fra le ricerche salvate di «Novità»). Le novità
              accettate dal feed <b>entrano già nella raccolta</b>; con ≥3 paper indicizzati i risultati
              sono filtrati per somiglianza semantica. Spegnendola, la ricerca (e il suo feed)
              <b>viene rimossa</b> dalle ricerche salvate.
            </div>
          {/if}

          {#if !selNode.smart}
            <div class="suggbox">
              <div class="sugghead">
                <span>✦ SUGGERIMENTI {suggLoading ? "· CALCOLO…" : sugg != null ? `(${suggVisible.length} sopra soglia)` : ""}</span>
                {#if sugg != null}
                  <button class="pclosemini" onclick={() => (sugg = null)}>✕</button>
                {/if}
              </div>
              <!-- La SORGENTE si sceglie PRIMA di calcolare: sempre visibile. -->
              <div class="suggctl">
                <span class="modechips">
                  <button class="lfm" class:active={suggMode === "name"} disabled={suggLoading} onclick={() => setSuggMode("name")} title="Somiglianza col solo NOME della raccolta (utile su raccolte nuove dal titolo parlante)">NOME</button>
                  <button class="lfm" class:active={suggMode === "content"} disabled={suggLoading || selNode.count === 0} onclick={() => setSuggMode("content")} title={selNode.count === 0 ? "Serve almeno un paper nella raccolta" : "Somiglianza col solo CONTENUTO (centroide dei paper già dentro) — ignora il nome"}>CONTENUTO</button>
                  <button class="lfm" class:active={suggMode === "both"} disabled={suggLoading || selNode.count === 0} onclick={() => setSuggMode("both")} title={selNode.count === 0 ? "Serve almeno un paper nella raccolta" : "Miscela nome+contenuto, col peso qui sotto"}>ENTRAMBI</button>
                </span>
                {#if suggMode === "both" && selNode.count > 0}
                  <label class="sldlbl" title="Quanto pesa il CONTENUTO nella miscela (il resto è il nome)">
                    contenuto <b>{suggWeight}%</b> · nome <b>{100 - suggWeight}%</b>
                    <input type="range" min="0" max="100" step="5" bind:value={suggWeight} disabled={suggLoading} onchange={saveSuggWeight} />
                  </label>
                {/if}
                <label class="chklbl">
                  <input type="checkbox" bind:checked={suggUnfiled} disabled={suggLoading} onchange={() => { if (sugg != null) void loadSuggestions(); }} />
                  solo senza raccolta
                </label>
              </div>
              {#if sugg == null}
                <button class="pbtn wide" onclick={loadSuggestions} disabled={suggLoading}>
                  {suggLoading ? "CALCOLO…" : "CALCOLA I SUGGERIMENTI"}
                </button>
              {:else}
                <div class="suggctl">
                  <label class="sldlbl">
                    confidenza ≥ <b>{suggThreshold}%</b>
                    <input type="range" min="30" max="90" step="1" bind:value={suggThreshold} />
                  </label>
                  <button class="pbtn" onclick={addAllSuggestions} disabled={suggAdding || suggVisible.length === 0}>
                    {suggAdding ? "AGGIUNGO…" : `AGGIUNGI TUTTI (${suggVisible.length})`}
                  </button>
                </div>
                <div class="sugglist">
                  {#each suggVisible as s (s.id)}
                    <div class="srow" title={fmtSugg(s)}>
                      <button class="saddbtn" onclick={() => addSuggestion(s)} title="Aggiungi alla raccolta">+</button>
                      <span class="sbar"><span class="sfill" style="width:{Math.round(s.score * 100)}%"></span></span>
                      <span class="spct">{Math.round(s.score * 100)}</span>
                      <span class="stxt">{fmtSugg(s)}</span>
                    </div>
                  {:else}
                    <div class="srow empty">Niente sopra il {suggThreshold}% — abbassa la soglia.</div>
                  {/each}
                </div>
                <div class="pnote">Punteggio = somiglianza semantica (bge-m3, in locale) con la sorgente scelta sopra: il <b>nome</b> della raccolta, il suo <b>contenuto</b> (i paper già dentro — più ne aggiungi, meglio va; niente rete né Ollama), o la miscela col peso che preferisci. Nulla viene aggiunto senza il tuo clic.</div>
              {/if}
            </div>
          {/if}
        {:else}
          <div class="pnote">I paper non ancora archiviati. Trascinali su una raccolta a sinistra; trascinare qui un paper (da una raccolta) lo toglie da quella raccolta.</div>
        {/if}

        <div class="dochead">PAPER {loadingDocs ? "…" : `(${docs.length})`}</div>
        <div class="doclist">
          {#each docs as d (d.id)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="drow"
              class:dragging={docDrag != null && docDrag.ids[0] === d.id}
              onpointerdown={(e) => docPointerDown(e, d)}
              title={fmtDoc(d) + " — trascinami su una raccolta"}
            >
              <span class="grip">⣿</span>
              <span class="dtxt">{fmtDoc(d)}</span>
            </div>
          {:else}
            <div class="drow empty">{loadingDocs ? "Carico…" : "Nessun paper qui."}</div>
          {/each}
        </div>
      {/if}
    </aside>
  </div>

  {#if nodeDrag}
    <div class="ghost" style="left:{nodeDrag.x + 12}px; top:{nodeDrag.y + 10}px;">
      {nodeDrag.name}
      <span class="gsub">{nodeDrag.valid ? (nodeDrag.target === "root" ? "→ radice" : "→ sotto-raccolta") : "…"}</span>
    </div>
  {/if}
  {#if docDrag}
    <div class="ghost" style="left:{docDrag.x + 12}px; top:{docDrag.y + 10}px;">
      {docDrag.label.length > 48 ? docDrag.label.slice(0, 47) + "…" : docDrag.label}
      <span class="gsub">{docDrag.valid ? (docDrag.target === "unfiled" ? "→ togli dalla raccolta" : "→ qui") : "…"}</span>
    </div>
  {/if}
</div>

<style>
  .arch {
    height: calc(100vh - 60px);
    min-height: 420px;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(1000px 500px at 70% -10%, rgba(0, 80, 110, 0.18), transparent 60%),
      #04080d;
    color: #9fdcec;
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
    border-bottom: 1px solid rgba(0, 200, 240, 0.14);
    flex: none;
    flex-wrap: wrap;
  }
  .brand .t1 { font-size: 15px; letter-spacing: 0.32em; color: #4fe3ff; text-shadow: 0 0 12px rgba(79, 227, 255, 0.5); }
  .brand .t2 { margin-left: 8px; font-size: 10px; letter-spacing: 0.22em; color: rgba(159, 220, 236, 0.45); }
  .chips { display: flex; gap: 7px; align-items: center; flex-wrap: wrap; }
  .chip {
    font-size: 10px;
    letter-spacing: 0.12em;
    padding: 3px 8px;
    border-radius: 3px;
    border: 1px solid rgba(0, 200, 240, 0.25);
    color: rgba(159, 220, 236, 0.6);
    background: none;
    font-family: inherit;
    white-space: nowrap;
  }
  button.chip { cursor: pointer; }
  .chip.act { color: #4fe3ff; }
  .chip.act:hover { background: rgba(0, 200, 240, 0.12); }
  .mkinput {
    background: rgba(6, 22, 32, 0.9);
    border: 1px solid rgba(0, 200, 240, 0.4);
    color: #d9f6ff;
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
    color: rgba(159, 220, 236, 0.42);
  }
  .hint b { color: rgba(159, 220, 236, 0.7); }
  .msg { flex: none; padding: 4px 14px 0; font-size: 11px; color: #ffd166; }

  .body { flex: 1; display: flex; min-height: 0; }
  .treewrap { flex: 1; overflow: auto; min-width: 0; }
  .archsvg { display: block; }
  .zone { font-size: 10px; letter-spacing: 0.4em; fill: rgba(0, 200, 240, 0.35); }
  .trace { fill: none; stroke: rgba(0, 190, 230, 0.22); stroke-width: 1.5; }

  .node { cursor: pointer; }
  .node rect:not(.pin) {
    fill: rgba(6, 22, 32, 0.88);
    stroke: rgba(0, 190, 230, 0.35);
    stroke-width: 1.3;
  }
  .node .pin { fill: rgba(0, 190, 230, 0.4); }
  .node .nlabel { font-size: 12px; font-weight: 700; letter-spacing: 0.08em; fill: #bfeefb; pointer-events: none; }
  .node .nsub { font-size: 9px; letter-spacing: 0.06em; fill: rgba(159, 220, 236, 0.5); pointer-events: none; }
  .node .ncount { font-size: 11px; text-anchor: end; fill: #7be9ff; pointer-events: none; }
  .node.sel rect:not(.pin) { stroke: #37e0ff; stroke-width: 2.2; filter: drop-shadow(0 0 8px rgba(55, 224, 255, 0.4)); }
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
    border-left: 1px solid rgba(0, 200, 240, 0.14);
    background: rgba(4, 12, 18, 0.92);
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    min-height: 0;
  }
  .phead .ptitle { font-size: 14px; font-weight: 700; letter-spacing: 0.18em; color: #4fe3ff; overflow-wrap: anywhere; }
  .crumb { font-size: 9px; letter-spacing: 0.08em; color: rgba(159, 220, 236, 0.4); margin-top: 3px; }
  .pstats {
    display: flex; gap: 12px; flex-wrap: wrap;
    font-size: 10px; color: rgba(159, 220, 236, 0.6);
    border-top: 1px solid rgba(0, 200, 240, 0.12);
    border-bottom: 1px solid rgba(0, 200, 240, 0.12);
    padding: 7px 0; margin: 8px 0;
  }
  .pstats b { color: #bfeefb; }
  .smartbadge { color: #ffd166; }
  .pacts { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 6px; }
  .pbtn {
    background: none;
    border: 1px solid rgba(0, 200, 240, 0.3);
    color: #9fdcec;
    font-family: inherit;
    font-size: 9px;
    letter-spacing: 0.16em;
    padding: 4px 9px;
    border-radius: 3px;
    cursor: pointer;
  }
  .pbtn:hover { color: #4fe3ff; border-color: rgba(79, 227, 255, 0.6); }
  .pbtn.danger { color: #ff8b94; border-color: rgba(255, 89, 100, 0.4); }
  .pbtn.danger:hover { color: #ff5964; border-color: rgba(255, 89, 100, 0.7); }
  .pnote { font-size: 10px; line-height: 1.5; color: rgba(159, 220, 236, 0.45); margin-bottom: 8px; }

  .nbell { font-size: 10px; text-anchor: end; fill: #ffd166; pointer-events: none; }
  .watchrow { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
  .wlbl { font-size: 9px; letter-spacing: 0.24em; color: rgba(0, 200, 240, 0.5); }
  .pbtn.won { color: #ffd166; border-color: rgba(255, 209, 102, 0.5); }

  /* ---- suggerimenti ---- */
  .suggbox {
    border-top: 1px solid rgba(0, 200, 240, 0.12);
    padding-top: 8px;
    margin-bottom: 8px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .pbtn.wide { width: 100%; text-align: center; color: #7be9ff; }
  .sugghead {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 9px;
    letter-spacing: 0.3em;
    color: rgba(0, 200, 240, 0.5);
    margin-bottom: 5px;
  }
  .pclosemini { background: none; border: none; color: rgba(159, 220, 236, 0.5); cursor: pointer; font-family: inherit; }
  .pclosemini:hover { color: #4fe3ff; }
  .suggctl { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-bottom: 6px; }
  .modechips { display: flex; gap: 4px; }
  .lfm {
    background: none;
    border: 1px solid rgba(0, 200, 240, 0.2);
    color: rgba(159, 220, 236, 0.5);
    font-family: inherit;
    font-size: 9px;
    letter-spacing: 0.14em;
    padding: 2px 7px;
    border-radius: 3px;
    cursor: pointer;
  }
  .lfm.active { color: #4fe3ff; border-color: rgba(79, 227, 255, 0.6); }
  .lfm:disabled { opacity: 0.4; cursor: default; }
  .sldlbl { font-size: 9px; letter-spacing: 0.1em; color: rgba(159, 220, 236, 0.6); display: flex; align-items: center; gap: 6px; }
  .sldlbl b { color: #7be9ff; min-width: 30px; }
  .sldlbl input[type="range"] { width: 110px; accent-color: #37e0ff; }
  .chklbl { font-size: 9px; letter-spacing: 0.1em; color: rgba(159, 220, 236, 0.6); display: flex; align-items: center; gap: 4px; }
  .sugglist { overflow-y: auto; max-height: 220px; min-height: 0; }
  .srow {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    line-height: 1.6;
    padding: 2px 2px;
  }
  .srow:hover { background: rgba(0, 200, 240, 0.07); }
  .srow.empty { color: rgba(159, 220, 236, 0.35); font-style: italic; }
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
  .sbar { width: 46px; height: 5px; background: rgba(0, 200, 240, 0.12); border-radius: 2px; flex: none; overflow: hidden; }
  .sfill { display: block; height: 100%; background: linear-gradient(90deg, #1b8aa8, #37e0ff); }
  .spct { color: #7be9ff; min-width: 20px; text-align: right; flex: none; }
  .stxt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: rgba(191, 238, 251, 0.85); }
  .dochead { font-size: 9px; letter-spacing: 0.3em; color: rgba(0, 200, 240, 0.4); margin: 4px 0; }
  .doclist { overflow-y: auto; min-height: 0; flex: 1; }
  .drow {
    display: flex;
    gap: 8px;
    align-items: baseline;
    font-size: 11px;
    line-height: 1.5;
    padding: 3px 4px;
    border-radius: 3px;
    color: rgba(191, 238, 251, 0.85);
    cursor: grab;
    touch-action: none;
  }
  .drow:hover { background: rgba(0, 200, 240, 0.08); }
  .drow.dragging { opacity: 0.45; }
  .drow .grip { color: rgba(0, 200, 240, 0.35); font-size: 9px; flex: none; }
  .drow .dtxt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .drow.empty { color: rgba(159, 220, 236, 0.35); font-style: italic; cursor: default; }

  .ghost {
    position: fixed;
    z-index: 300;
    pointer-events: none;
    background: rgba(9, 45, 62, 0.95);
    border: 1px solid #37e0ff;
    color: #eaffff;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 4px;
    box-shadow: 0 0 14px rgba(55, 224, 255, 0.4);
  }
  .ghost .gsub { margin-left: 8px; font-size: 9px; color: #7be9ff; }
</style>
