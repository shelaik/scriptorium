<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    importFiles,
    listDocuments,
    getThumbnail,
    enrichAll,
    searchDocuments,
    relatedDocuments,
    ragIndexStatus,
    buildRagIndex,
    cancelRagIndex,
    clearRagIndex,
    askLibrary,
    type RagStatus,
    type AskResult,
    recentDocuments,
    documentsByAuthor,
    citeText,
    exportCitations,
    citationLinks,
    type CitationLinks,
    libraryHealth,
    type LibraryHealth,
    libraryFacets,
    type LibraryFacets,
    citationGaps,
    type GapItem,
    listSavedSearches,
    createSavedSearch,
    deleteSavedSearch,
    runSavedSearch,
    type SavedSearch,
    embeddingStatus,
    generateEmbeddings,
    cancelEmbeddings,
    listTags,
    createTag,
    deleteTag,
    setDocumentTags,
    listCollections,
    createCollection,
    deleteCollection,
    addToCollection,
    deleteDocuments,
    restoreDocuments,
    purgeDocuments,
    listTrash,
    addDocumentTag,
    findDuplicates,
    mergeDocuments,
    addByIdentifiers,
    importBibtex,
    findPdf,
    hfResources,
    type HfResources,
    githubRepos,
    githubReadme,
    type GhRepo,
    setRead,
    setFavorite,
    backupLibrary,
    getAiSettings,
    setAiSettings,
    aiListModels,
    aiStatus as fetchAiStatus,
    aiServerStart,
    aiServerStop,
    type AiProvider,
    type AiStatus,
    summarizeDocument,
    autotagDocument,
    getDiscoverySettings,
    setDiscoverySettings,
    setApiKey,
    discoverSearch,
    discoverAdd,
    getObsidianVault,
    setObsidianVault,
    exportToObsidian,
    type SearchResult,
    getWatchedFolder,
    setWatchedFolder,
    type DocumentItem,
    type SearchMode,
    type EmbedStatus,
    type EmbedProgress,
    type Tag,
    type Collection,
  } from "$lib/api";
  import Viewer from "$lib/viewer/Viewer.svelte";
  import MetaEditor from "$lib/MetaEditor.svelte";
  import { printDocument, printDocuments } from "$lib/print";
  import ShareMenu from "$lib/ShareMenu.svelte";
  import { revealDocument, openInBrowser } from "$lib/share";
  import Terminal from "$lib/Terminal.svelte";

  type Filter = {
    kind: "all" | "collection" | "related" | "trash" | "duplicates" | "discover" | "favorite" | "unread" | "terminal" | "author" | "github" | "peerreviewed" | "ask";
    id?: number;
    label?: string;
  };

  const PALETTE = [
    "#ef4444", "#f59e0b", "#10b981", "#3b82f6",
    "#8b5cf6", "#ec4899", "#14b8a6", "#eab308",
  ];

  let docs = $state<DocumentItem[]>([]);
  // Whole-library counts behind the sidebar filters; refreshed by loadDocs().
  let facets = $state<LibraryFacets>({ all: 0, favorite: 0, unread: 0, github: 0, peerreviewed: 0 });
  let recentDocs = $state<DocumentItem[]>([]); // "Continue reading" shelf
  let results = $state<DocumentItem[]>([]);
  let thumbs = $state<Record<number, string>>({});
  let tags = $state<Tag[]>([]);
  let collections = $state<Collection[]>([]);
  let filter = $state<Filter>({ kind: "all" });
  let tagFilter = $state<number[]>([]); // tag ids to filter by (multi-select)
  let tagMode = $state<"all" | "any">("all"); // AND (all) vs OR (any) across selected tags
  let tagsCollapsed = $state(false);
  let tagSort = $state<"asc" | "desc">("asc");
  let displayedTags = $derived(
    [...tags].sort((a, b) => {
      const c = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
      return tagSort === "desc" ? -c : c;
    }),
  );
  // How many documents have no bibliographic metadata yet (bare author/year/venue).
  // Drives the badge + tooltip on the "Metadati" button so it's clear that
  // those fields — and the references/citations/gap features built on them —
  // stay empty until metadata is fetched.
  let needsMeta = $derived(docs.filter(isBare).length);
  let query = $state("");
  let mode = $state<SearchMode>("hybrid");
  let emb = $state<EmbedStatus>({ total: 0, embedded: 0 });
  let embedProgress = $state<EmbedProgress | null>(null);
  let busy = $state(false);
  let enriching = $state(false);
  let generating = $state(false);
  let searching = $state(false);
  let printing = $state(false);
  let dragOver = $state(false);
  let status = $state("");
  let openDoc = $state<DocumentItem | null>(null);
  let cardMenu = $state<{ doc: DocumentItem; x: number; y: number } | null>(null);
  let headerMenu = $state<{ kind: "import" | "export"; x: number; y: number } | null>(null);
  let watchedFolder = $state<string | null>(null);
  let view = $state<"grid" | "list">("grid");
  // Grid thumbnail size (min column width in px): user-resizable via a slider,
  // persisted so the chosen density survives restarts.
  let gridSize = $state(
    (typeof localStorage !== "undefined" && Number(localStorage.getItem("pdfmanage-gridsize"))) || 180,
  );
  $effect(() => {
    try {
      localStorage.setItem("pdfmanage-gridsize", String(gridSize));
    } catch {
      /* ignore */
    }
  });
  let terminalOpened = $state(false); // mount the terminal lazily, then keep it alive across tabs
  // ----- Reusable confirmation dialog (for destructive actions) -----
  let confirmBox = $state<{ msg: string; ok: string; danger: boolean; resolve: (v: boolean) => void } | null>(null);
  function confirmAsk(msg: string, ok = "Elimina", danger = true): Promise<boolean> {
    return new Promise((resolve) => {
      confirmBox = { msg, ok, danger, resolve };
    });
  }
  function answerConfirm(v: boolean) {
    confirmBox?.resolve(v);
    confirmBox = null;
  }
  // Close the integrated terminal (kills the PTY session by unmounting it).
  async function closeTerminal() {
    if (!(await confirmAsk("Chiudere il terminale? La sessione in corso verrà terminata.", "Chiudi"))) return;
    terminalOpened = false;
    setFilter({ kind: "all" });
  }
  let theme = $state(
    (typeof localStorage !== "undefined" && localStorage.getItem("pdfmanage-theme")) || "paper",
  );
  $effect(() => {
    if (typeof document !== "undefined") {
      document.body.dataset.theme = theme;
      try {
        localStorage.setItem("pdfmanage-theme", theme);
      } catch {
        /* ignore */
      }
    }
  });
  let editingId = $state<number | null>(null);
  type SortKey = "favorite" | "author" | "title" | "year" | "added";
  // Multi-criteria sort: criteria apply in the order the user activated them.
  let sortChain = $state<{ key: SortKey; dir: "asc" | "desc" }[]>([]);
  const SORT_KEYS: SortKey[] = ["favorite", "author", "title", "year", "added"];
  const SORT_LABELS: Record<SortKey, string> = {
    favorite: "Preferiti",
    author: "Primo autore",
    title: "Titolo",
    year: "Anno",
    added: "Aggiunto",
  };
  // Direction applied on first activation (the most natural for each field).
  const SORT_NATURAL: Record<SortKey, "asc" | "desc"> = {
    favorite: "desc", // favorites first
    author: "asc",
    title: "asc",
    year: "desc", // newest first
    added: "desc", // most recent first
  };
  let selected = $state<number[]>([]);
  let dupGroups = $state<number[][]>([]);
  let dupMap = $state<Record<number, DocumentItem>>({});
  let idModal = $state(false);
  let idText = $state("");
  let addingIds = $state(false);
  // Online discovery
  let discoverQuery = $state("");
  let discoverAuthor = $state("");
  let discoverSource = $state<"openalex" | "arxiv" | "ads" | "semanticscholar" | "europepmc" | "core" | "doaj" | "huggingface">("openalex");
  // Sources that expose a citation count (for the "Citazioni" sort option).
  const CITES_SOURCES = ["openalex", "ads", "semanticscholar", "europepmc", "core"];
  let discoverYearFrom = $state("");
  let discoverYearTo = $state("");
  let discoverOaOnly = $state(false);
  let discoverSort = $state("relevance");
  let discoverResults = $state<SearchResult[]>([]);
  let discovering = $state(false);
  // Saved searches + "novità" (new since last run) badging.
  let savedSearches = $state<SavedSearch[]>([]);
  let discoverNewIds = $state<Set<string>>(new Set());
  let savingSearch = $state(false);
  // ----- RAG engine ("Chiedi alla libreria") -----
  let askQuestion = $state("");
  let askAnswer = $state("");
  let askSources = $state<AskResult["sources"]>([]);
  let asking = $state(false);
  let ragStatus = $state<RagStatus | null>(null);
  let ragBuilding = $state(false);
  let ragProg = $state<{ done: number; total: number } | null>(null);
  let askScope = $state<{ kind: "doc" | "collection" | "tag"; id: number; label: string } | null>(null);
  // Group the cited passages by document so a repeated source appears once.
  let askGroups = $derived.by(() => {
    const m = new Map<number, { document_id: number; title: string; items: AskResult["sources"] }>();
    for (const s of askSources) {
      let g = m.get(s.document_id);
      if (!g) {
        g = { document_id: s.document_id, title: s.title, items: [] };
        m.set(s.document_id, g);
      }
      g.items.push(s);
    }
    return [...m.values()];
  });
  // External id of the result whose abstract row is expanded (one at a time).
  let expandedAbstract = $state<string | null>(null);
  let discSortKey = $state<"title" | "year" | "venue" | "citations" | null>(null);
  let discSortDir = $state<"asc" | "desc">("asc");
  // Result filters (toggle chips): narrow to papers that ship code, that are
  // peer-reviewed, or that are preprints. They AND together with each other and
  // with the column sort below. Computed against the fields the backend fills in
  // (github_url + pub_status: "published" | "preprint" | "preprint_reviewed").
  let discCodeOnly = $state(false);
  let discPeerOnly = $state(false);
  let discPreprintOnly = $state(false);
  // A peer-reviewed version exists for both a published work and a preprint whose
  // reviewed version is known; a preprint is either a bare or reviewed preprint.
  const isPeer = (r: SearchResult) => r.pub_status === "published" || r.pub_status === "preprint_reviewed";
  const isPreprint = (r: SearchResult) => r.pub_status === "preprint" || r.pub_status === "preprint_reviewed";
  let discDisplayed = $derived.by(() => {
    let rows = discoverResults;
    if (discCodeOnly) rows = rows.filter((r) => !!r.github_url);
    if (discPeerOnly) rows = rows.filter(isPeer);
    if (discPreprintOnly) rows = rows.filter(isPreprint);
    if (!discSortKey) return rows;
    const k = discSortKey;
    const dir = discSortDir === "asc" ? 1 : -1;
    return [...rows].sort((a, b) => {
      let av: string | number = "";
      let bv: string | number = "";
      if (k === "title") {
        av = (a.title ?? "").toLowerCase();
        bv = (b.title ?? "").toLowerCase();
      } else if (k === "venue") {
        av = (a.venue ?? "").toLowerCase();
        bv = (b.venue ?? "").toLowerCase();
      } else if (k === "year") {
        av = a.year ?? 0;
        bv = b.year ?? 0;
      } else {
        av = a.citations;
        bv = b.citations;
      }
      return av < bv ? -dir : av > bv ? dir : 0;
    });
  });
  function toggleDiscSort(k: "title" | "year" | "venue" | "citations") {
    if (discSortKey === k) discSortDir = discSortDir === "asc" ? "desc" : "asc";
    else {
      discSortKey = k;
      discSortDir = "asc";
    }
  }
  function discArrow(k: string): string {
    return discSortKey === k ? (discSortDir === "asc" ? "▲" : "▼") : "";
  }
  let addingExt = $state<string | null>(null);
  let settingsModal = $state(false);
  let helpModal = $state(false);
  let aboutModal = $state(false);
  const APP_VERSION = "0.1.0";
  const APP_YEAR = "2026";
  let settingsTab = $state<"online" | "ai" | "obsidian" | "backup">("online");
  let obsidianVault = $state("");
  let exportingObsidian = $state(false);
  let discEnabled = $state(false);
  let discEmail = $state("");
  // API keys live in the OS vault: the renderer only knows whether each is set.
  let hasKey = $state<Record<string, boolean>>({});
  let keyInput = $state<Record<string, string>>({});
  let keyEditing = $state<Record<string, boolean>>({});
  // AI (Ollama / LM Studio) — optional
  let aiEnabled = $state(false);
  let aiProvider = $state<AiProvider>("ollama");
  let ollamaUrl = $state("http://localhost:11434");
  let lmstudioUrl = $state("http://localhost:1234");
  let aiModel = $state("llama3.2:3b");
  let aiEmbedGpu = $state(false);
  let aiEmbedBatch = $state(0); // 0 = auto
  let ollamaModels = $state<string[] | null>(null);
  let lmstudioModels = $state<string[] | null>(null);
  let ollamaErr = $state("");
  let lmstudioErr = $state("");
  let verifyingOllama = $state(false);
  let verifyingLm = $state(false);
  let aiBusy = $state<number | null>(null);
  // Batch AI (summarize / autotag over the current selection)
  let aiBatch = $state<{ kind: "summary" | "tags"; done: number; total: number } | null>(null);
  let batchCancel = $state(false);
  // True while ANY AI request is in flight (single-doc OR batch) — local models
  // serve one generation at a time, so the two entry points must be mutually exclusive.
  let aiBusyAny = $derived(aiBusy !== null || aiBatch !== null);
  // Live AI status for the header indicator.
  let aiStat = $state<AiStatus | null>(null);
  let aiStatusTimer: ReturnType<typeof setInterval> | undefined;

  // Sidebar creation inputs
  let newTagName = $state("");
  let newCollName = $state("");
  let newCollSmart = $state(false);
  let smartType = $state<"untagged" | "year_gte" | "tag" | "text">("untagged");
  let smartValue = $state("");

  let shown = $derived.by(() => {
    const base = query.trim() ? results : docs;
    if (!tagFilter.length) return base;
    return base.filter((d) =>
      tagMode === "any"
        ? tagFilter.some((tid) => d.tags.some((t) => t.id === tid))
        : tagFilter.every((tid) => d.tags.some((t) => t.id === tid)),
    );
  });
  function cmpSort(a: DocumentItem, b: DocumentItem, key: SortKey): number {
    switch (key) {
      case "favorite":
        return (a.favorite ? 1 : 0) - (b.favorite ? 1 : 0);
      case "author":
        return (a.authors[0] ?? "").toLowerCase().localeCompare((b.authors[0] ?? "").toLowerCase());
      case "title":
        return (a.title ?? "").toLowerCase().localeCompare((b.title ?? "").toLowerCase());
      case "year":
        return (a.year ?? 0) - (b.year ?? 0);
      case "added": {
        const av = a.added_at ?? "";
        const bv = b.added_at ?? "";
        return av < bv ? -1 : av > bv ? 1 : 0;
      }
    }
  }
  let displayed = $derived.by(() => {
    if (!sortChain.length) return shown;
    return [...shown].sort((a, b) => {
      for (const { key, dir } of sortChain) {
        const c = cmpSort(a, b, key) * (dir === "asc" ? 1 : -1);
        if (c) return c;
      }
      return 0;
    });
  });

  // Cycle a criterion: off → natural direction → reversed → off.
  function cycleSort(k: SortKey) {
    const i = sortChain.findIndex((s) => s.key === k);
    if (i === -1) {
      sortChain = [...sortChain, { key: k, dir: SORT_NATURAL[k] }];
    } else if (sortChain[i].dir === SORT_NATURAL[k]) {
      sortChain = sortChain.map((s, j) => (j === i ? { ...s, dir: s.dir === "asc" ? "desc" : "asc" } : s));
    } else {
      sortChain = sortChain.filter((_, j) => j !== i);
    }
  }
  function sortDirOf(k: SortKey): "asc" | "desc" | null {
    return sortChain.find((s) => s.key === k)?.dir ?? null;
  }
  function sortArrow(k: SortKey): string {
    const d = sortDirOf(k);
    return d === "asc" ? "▲" : d === "desc" ? "▼" : "";
  }
  function sortRank(k: SortKey): number {
    const i = sortChain.findIndex((s) => s.key === k);
    return i === -1 ? 0 : i + 1;
  }
  function clearSort() {
    sortChain = [];
  }

  // Select-all (for batch actions): toggles the whole shown list.
  let allSelected = $derived(
    displayed.length > 0 && displayed.every((d) => selected.includes(d.id)),
  );
  function toggleSelectAll() {
    selected = allSelected ? [] : displayed.map((d) => d.id);
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSeq = 0;
  $effect(() => {
    const q = query;
    const m = mode;
    clearTimeout(searchTimer);
    if (!q.trim()) {
      searchSeq++;
      return;
    }
    searchTimer = setTimeout(async () => {
      const myId = ++searchSeq;
      searching = true;
      try {
        const r = await searchDocuments(q, m);
        if (myId !== searchSeq) return;
        results = r;
        ensureThumbs(r);
      } catch (e) {
        if (myId === searchSeq) status = "Errore ricerca: " + e;
      } finally {
        if (myId === searchSeq) searching = false;
      }
    }, 250);
  });

  function ensureThumbs(items: DocumentItem[]) {
    for (const d of items) {
      if (d.has_thumb && !thumbs[d.id]) {
        getThumbnail(d.id)
          .then((t) => {
            if (t) thumbs[d.id] = t;
          })
          .catch(() => {});
      }
    }
  }

  function filterArg() {
    if (filter.kind === "collection") return { collectionId: filter.id };
    if (filter.kind === "favorite") return { flag: "favorite" as const };
    if (filter.kind === "unread") return { flag: "unread" as const };
    if (filter.kind === "github") return { flag: "github" as const };
    if (filter.kind === "peerreviewed") return { flag: "peerreviewed" as const };
    return undefined;
  }

  async function loadDocs() {
    selected = [];
    if (filter.kind === "trash") {
      docs = await listTrash();
    } else if (filter.kind === "duplicates" || filter.kind === "terminal") {
      docs = [];
    } else if (filter.kind === "related" && filter.id != null) {
      docs = await relatedDocuments(filter.id);
    } else if (filter.kind === "author" && filter.label) {
      docs = await documentsByAuthor(filter.label);
    } else {
      docs = await listDocuments(filterArg());
    }
    ensureThumbs(docs);
    // Refresh the "Continue reading" shelf (shown only on the full library view).
    if (filter.kind === "all") {
      try {
        recentDocs = await recentDocuments(8);
        ensureThumbs(recentDocs);
      } catch {
        recentDocs = [];
      }
    }
    // Refresh the sidebar filter counts (whole-library, independent of filter).
    try {
      facets = await libraryFacets();
    } catch {
      /* counts are advisory; leave the previous values on error */
    }
  }

  async function loadDuplicates() {
    dupGroups = await findDuplicates();
    const all = await listDocuments();
    const map: Record<number, DocumentItem> = {};
    for (const d of all) map[d.id] = d;
    dupMap = map;
    ensureThumbs(all);
  }
  async function loadSidebar() {
    tags = await listTags();
    collections = await listCollections();
    await loadSaved();
  }

  function setFilter(f: Filter) {
    filter = f;
    query = "";
    selected = [];
    tagFilter = []; // base-filter change starts a fresh tag selection
    tagMode = "all";
    if (f.kind === "duplicates") loadDuplicates();
    else loadDocs();
  }
  /** Toggle a tag in the multi-tag filter (works on top of the current doc listing). */
  function toggleTagFilter(id: number) {
    const docsView =
      filter.kind === "all" ||
      filter.kind === "favorite" ||
      filter.kind === "unread" ||
      filter.kind === "collection";
    if (!docsView) {
      // From trash/discover/duplicates/related, jump back to the full library first.
      setFilter({ kind: "all" }); // also clears tagFilter + tagMode + selected
      tagFilter = [id];
      return;
    }
    const next = tagFilter.includes(id)
      ? tagFilter.filter((x) => x !== id)
      : [...tagFilter, id];
    tagFilter = next;
    if (next.length <= 1) tagMode = "all"; // mode only matters with 2+ tags
    selected = []; // the working selection resets when the filter changes
  }
  function clearTags() {
    tagFilter = [];
    tagMode = "all";
    selected = [];
  }
  function setTagMode(m: "all" | "any") {
    tagMode = m;
    selected = []; // the visible set changes, so drop any now-hidden selection
  }

  async function toggleRead(doc: DocumentItem) {
    await setRead(doc.id, !doc.is_read);
    cardMenu = null;
    await loadDocs();
  }
  async function toggleFavorite(doc: DocumentItem) {
    const v = !doc.favorite;
    try {
      await setFavorite(doc.id, v);
      doc.favorite = v; // in-place: Svelte 5 deep reactivity updates grid + list
      // If we're viewing the Favorites filter and just un-favorited, drop it.
      if (filter.kind === "favorite" && !v) {
        docs = docs.filter((d) => d.id !== doc.id);
        results = results.filter((d) => d.id !== doc.id);
        selected = selected.filter((x) => x !== doc.id);
      }
    } catch (e) {
      status = "Errore preferiti: " + e;
    } finally {
      cardMenu = null;
    }
  }
  async function doBackup() {
    const dir = await open({ directory: true, multiple: false, title: "Scegli dove salvare il backup" });
    if (!dir || Array.isArray(dir)) return;
    status = "Backup in corso…";
    try {
      const path = await backupLibrary(dir);
      status = "Backup salvato: " + path;
    } catch (e) {
      status = "Errore backup: " + e;
    }
  }

  // ----- Hygiene actions -----
  async function trashSelected(ids: number[]) {
    if (!ids.length) return;
    const n = ids.length;
    if (!(await confirmAsk(`Spostare ${n} ${n > 1 ? "documenti" : "documento"} nel cestino?`, "Sposta nel cestino", false))) return;
    await deleteDocuments(ids);
    cardMenu = null;
    await loadDocs();
    await loadSidebar();
  }
  async function bulkAddTag(tag: Tag) {
    for (const id of selected) await addDocumentTag(id, tag.id);
    await loadDocs();
    await loadSidebar();
  }
  async function bulkAddCollection(coll: Collection) {
    for (const id of selected) await addToCollection(coll.id, id);
    status = `${selected.length} aggiunti a "${coll.name}"`;
    selected = [];
  }

  // ----- Print -----
  async function printSelected() {
    if (!selected.length || printing) return;
    printing = true;
    status = "Preparazione stampa…";
    try {
      const r = await printDocuments(selected);
      if (r.printed === 0) {
        status = "Nessun PDF da stampare (i riferimenti senza file sono stati saltati)";
      } else {
        const noun = r.printed === 1 ? "documento" : "documenti";
        status = r.skipped
          ? `Stampa avviata: ${r.printed} ${noun} · ${r.skipped} senza PDF saltati`
          : `Stampa avviata: ${r.printed} ${noun}`;
      }
    } catch (e) {
      status = "Errore stampa: " + e;
    } finally {
      printing = false;
    }
  }
  async function printOne(doc: DocumentItem) {
    cardMenu = null;
    printing = true;
    status = "Preparazione stampa…";
    try {
      await printDocument(doc.id);
      status = "Stampa avviata";
    } catch {
      status = "Questo elemento non ha un PDF da stampare";
    } finally {
      printing = false;
    }
  }

  function toggleSelect(id: number) {
    selected = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
  }
  async function restoreFromTrash(id: number) {
    await restoreDocuments([id]);
    await loadDocs();
    await loadSidebar();
  }
  async function purgeFromTrash(id: number) {
    if (!(await confirmAsk("Eliminare definitivamente questo documento? L'operazione è irreversibile."))) return;
    await purgeDocuments([id]);
    await loadDocs();
  }
  async function emptyTrash() {
    const n = docs.length;
    if (!n) return;
    if (!(await confirmAsk(`Svuotare il cestino? ${n} ${n > 1 ? "documenti verranno eliminati" : "documento verrà eliminato"} definitivamente.`, "Svuota cestino"))) return;
    await purgeDocuments(docs.map((d) => d.id));
    await loadDocs();
  }
  async function doMerge(group: number[]) {
    if (!(await confirmAsk(`Unire ${group.length} documenti in uno? Gli altri verranno rimossi (note e tag confluiscono nel primo).`, "Unisci", false))) return;
    await mergeDocuments(group[0], group.slice(1));
    status = `Uniti ${group.length} documenti`;
    await loadDuplicates();
  }

  async function handleImport(paths: string[]) {
    if (!paths.length) return;
    busy = true;
    status = `Importazione di ${paths.length} file…`;
    try {
      const res = await importFiles(paths);
      const parts = [`${res.imported.length} importati`];
      if (res.duplicates.length) parts.push(`${res.duplicates.length} già presenti`);
      if (res.warnings.length) parts.push(`${res.warnings.length} senza testo`);
      if (res.errors.length) parts.push(`${res.errors.length} errori`);
      status = parts.join(" · ");
      await loadDocs();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      busy = false;
    }
  }

  async function enrichMeta() {
    enriching = true;
    status = "Recupero metadati da Crossref…";
    try {
      const res = await enrichAll();
      const parts = [`${res.updated} aggiornati`];
      if (res.no_doi) parts.push(`${res.no_doi} senza DOI`);
      if (res.errors.length) parts.push(`${res.errors.length} errori`);
      status = parts.join(" · ");
      await loadDocs();
    } catch (e) {
      status = "Errore metadati: " + e;
    } finally {
      enriching = false;
    }
  }

  async function generateIndex() {
    generating = true;
    status = "Generazione indice semantico… (al primo uso scarica il modello bge-m3 ~2.3GB)";
    try {
      const res = await generateEmbeddings();
      status =
        `Indice semantico: +${res.embedded} documenti` +
        (res.errors.length ? ` · ${res.errors.length} errori` : "");
      await loadStatus();
    } catch (e) {
      status = "Errore indice: " + e;
    } finally {
      generating = false;
    }
  }

  async function stopIndex() {
    try {
      await cancelEmbeddings();
    } catch {
      /* ignore */
    }
  }

  async function loadStatus() {
    try {
      emb = await embeddingStatus();
    } catch {
      /* ignore */
    }
  }

  async function importViaDialog() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!selected) return;
    await handleImport(Array.isArray(selected) ? selected : [selected]);
  }

  async function importBibtexDialog() {
    const file = await open({
      multiple: false,
      filters: [{ name: "BibTeX / RIS", extensions: ["bib", "bibtex", "ris", "txt"] }],
    });
    if (typeof file !== "string") return;
    status = "Importo riferimenti…";
    try {
      const res = await importBibtex(file);
      const parts = [`${res.added} aggiunti`];
      if (res.skipped) parts.push(`${res.skipped} saltati`);
      if (res.errors.length) parts.push(`${res.errors.length} errori`);
      status = "BibTeX: " + parts.join(" · ");
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore import BibTeX: " + e;
    }
  }

  // Code & repos linked to a paper (Hugging Face + GitHub)
  let hfModal = $state(false);
  let hfLoading = $state(false);
  let hfData = $state<HfResources | null>(null);
  let hfTitle = $state("");
  // GitHub
  let ghRepos = $state<GhRepo[] | null>(null);
  let ghReadmeOf = $state<string | null>(null); // "owner/repo" being previewed
  let ghReadmeHtml = $state("");
  let ghReadmeError = $state("");
  let ghReadmeLoading = $state(false);
  async function openHf(d: DocumentItem) {
    cardMenu = null;
    hfModal = true;
    hfLoading = true;
    hfData = null;
    ghRepos = null;
    ghReadmeOf = null;
    ghReadmeHtml = "";
    hfTitle = d.title ?? "documento";
    // Fetch HF resources and GitHub repos in parallel (both best-effort).
    const hfP = hfResources(d.id).then((r) => (hfData = r)).catch((e) => { status = "HF: " + e; });
    const ghP = githubRepos(d.id).then((r) => (ghRepos = r)).catch(() => (ghRepos = []));
    await hfP;
    hfLoading = false;
    await ghP;
  }
  async function openReadme(r: GhRepo) {
    ghReadmeOf = r.full_name;
    ghReadmeLoading = true;
    ghReadmeHtml = "";
    ghReadmeError = "";
    try {
      ghReadmeHtml = await githubReadme(r.owner, r.repo);
    } catch (e) {
      ghReadmeError = "Impossibile caricare il README: " + e;
    } finally {
      ghReadmeLoading = false;
    }
  }
  /** Action: intercept link clicks inside the rendered README → open in browser. */
  function readmeLinks(node: HTMLElement) {
    const handler = (e: MouseEvent) => {
      const a = (e.target as HTMLElement)?.closest("a");
      const href = a?.getAttribute("href");
      if (href && /^https?:\/\//i.test(href)) {
        e.preventDefault();
        openInBrowser(href);
      }
    };
    node.addEventListener("click", handler);
    return { destroy: () => node.removeEventListener("click", handler) };
  }

  // Citations modal (references + cited-by)
  let citModal = $state(false);
  let citLoading = $state(false);
  let citData = $state<CitationLinks | null>(null);
  let citTitle = $state("");
  async function openCitations(d: DocumentItem) {
    cardMenu = null;
    citModal = true;
    citLoading = true;
    citData = null;
    citTitle = d.title ?? "documento";
    try {
      citData = await citationLinks(d.id);
    } catch (e) {
      status = "Errore riferimenti: " + e;
      citModal = false;
    } finally {
      citLoading = false;
    }
  }
  /** Open a library document by id (used by citation links). */
  async function openById(id: number) {
    const found = docs.find((d) => d.id === id) ?? recentDocs.find((d) => d.id === id);
    if (found) {
      citModal = false;
      openDoc = found;
      return;
    }
    // Not in the current view: fetch the full list and locate it.
    try {
      const all = await listDocuments();
      const d = all.find((x) => x.id === id);
      if (d) {
        citModal = false;
        openDoc = d;
      }
    } catch {
      /* ignore */
    }
  }

  async function doFindPdf(d: DocumentItem) {
    cardMenu = null;
    status = `Cerco un PDF per «${d.title ?? "documento"}»…`;
    try {
      const res = await findPdf(d.id);
      status =
        res === "attached"
          ? "PDF trovato e allegato ✓"
          : res === "already"
            ? "Questo documento ha già un PDF"
            : res === "duplicate"
              ? "Quel PDF è già nella libreria (in un altro documento)"
              : "Nessun PDF Open Access trovato";
      if (res === "attached") {
        await loadDocs();
      }
    } catch (e) {
      status = "Errore Trova PDF: " + e;
    }
  }

  async function addIds() {
    const lines = idText.split("\n").map((s) => s.trim()).filter(Boolean);
    if (!lines.length) return;
    addingIds = true;
    status = `Recupero ${lines.length} riferimenti…`;
    try {
      const res = await addByIdentifiers(lines);
      const parts = [`${res.added} aggiunti`];
      if (res.skipped) parts.push(`${res.skipped} già presenti`);
      if (res.errors.length) parts.push(`${res.errors.length} non trovati`);
      status = parts.join(" · ");
      idText = "";
      idModal = false;
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      addingIds = false;
    }
  }

  // ----- Online discovery -----
  async function openSettings() {
    try {
      const s = await getDiscoverySettings();
      discEnabled = s.enabled;
      discEmail = s.email;
      hasKey = {
        openalex_key: s.has_openalex_key,
        ads_token: s.has_ads_token,
        s2_key: s.has_s2_key,
        core_key: s.has_core_key,
        github_token: s.has_github_token,
      };
      keyInput = {};
      keyEditing = {};
    } catch {
      /* ignore */
    }
    try {
      obsidianVault = await getObsidianVault();
    } catch {
      /* ignore */
    }
    try {
      const a = await getAiSettings();
      aiEnabled = a.enabled;
      aiProvider = a.provider;
      ollamaUrl = a.ollama_url;
      lmstudioUrl = a.lmstudio_url;
      aiModel = a.model;
      aiEmbedGpu = a.embed_gpu;
      aiEmbedBatch = a.embed_batch;
    } catch {
      /* ignore */
    }
    ollamaModels = null;
    lmstudioModels = null;
    ollamaErr = "";
    lmstudioErr = "";
    settingsTab = "online";
    settingsModal = true;
    // Auto-detect which providers are up and which models they serve.
    verifyProvider("ollama");
    verifyProvider("lmstudio");
  }
  async function saveSettings() {
    await setDiscoverySettings(discEnabled, discEmail);
    await setAiSettings({
      enabled: aiEnabled,
      provider: aiProvider,
      ollama_url: ollamaUrl,
      lmstudio_url: lmstudioUrl,
      model: aiModel,
      embed_gpu: aiEmbedGpu,
      embed_batch: aiEmbedBatch,
    });
    settingsModal = false;
    status = "Impostazioni salvate";
    refreshAiStatus();
  }
  async function saveKey(name: string) {
    const v = keyInput[name] ?? "";
    try {
      await setApiKey(name, v);
      hasKey[name] = v.trim().length > 0;
      keyInput[name] = "";
      keyEditing[name] = false;
      status = "Chiave salvata nel keychain ✓";
    } catch (e) {
      status = "Errore salvataggio chiave: " + e;
    }
  }
  async function clearKey(name: string) {
    if (!(await confirmAsk("Rimuovere questa chiave API dal keychain?", "Rimuovi"))) return;
    try {
      await setApiKey(name, "");
      hasKey[name] = false;
      keyEditing[name] = false;
      keyInput[name] = "";
      status = "Chiave rimossa dal keychain";
    } catch (e) {
      status = "Errore: " + e;
    }
  }
  async function verifyProvider(provider: AiProvider) {
    const url = provider === "lmstudio" ? lmstudioUrl : ollamaUrl;
    if (provider === "lmstudio") {
      verifyingLm = true;
      lmstudioErr = "";
      lmstudioModels = null;
    } else {
      verifyingOllama = true;
      ollamaErr = "";
      ollamaModels = null;
    }
    try {
      const models = await aiListModels(provider, url);
      if (provider === "lmstudio") lmstudioModels = models;
      else ollamaModels = models;
    } catch (e) {
      if (provider === "lmstudio") lmstudioErr = "" + e;
      else ollamaErr = "" + e;
    } finally {
      if (provider === "lmstudio") verifyingLm = false;
      else verifyingOllama = false;
      // Keep the live header indicator in sync with what the user just checked.
      refreshAiStatus();
    }
  }
  async function refreshAiStatus() {
    try {
      aiStat = await fetchAiStatus();
    } catch {
      aiStat = null;
    }
  }
  /** Apply a "provider::model" value from the combined model dropdown. */
  function chooseModel(value: string) {
    if (!value) return;
    const idx = value.indexOf("::");
    if (idx < 0) return;
    aiProvider = value.slice(0, idx) as AiProvider;
    aiModel = value.slice(idx + 2);
  }
  /** Persist the current AI form (without closing the modal) so the header
   *  indicator — which reads the saved config — reflects what was just started. */
  async function persistAi() {
    try {
      await setAiSettings({
        enabled: aiEnabled,
        provider: aiProvider,
        ollama_url: ollamaUrl,
        lmstudio_url: lmstudioUrl,
        model: aiModel,
        embed_gpu: aiEmbedGpu,
        embed_batch: aiEmbedBatch,
      });
    } catch {
      /* non-fatal */
    }
  }
  async function startServer(provider: AiProvider) {
    const name = provider === "lmstudio" ? "LM Studio" : "Ollama";
    await persistAi(); // so ai_status (and the header chip) reflect this provider
    status = `Avvio del server ${name}…`;
    try {
      await aiServerStart(provider);
      status = `${name}: avvio richiesto`;
      // Poll: a cold start can take a few seconds to bind the port.
      // verifyProvider also refreshes the header indicator (in its finally).
      for (let i = 0; i < 6; i++) {
        await new Promise((r) => setTimeout(r, 1200));
        await verifyProvider(provider);
        const up = provider === "lmstudio" ? lmstudioModels : ollamaModels;
        if (up !== null) {
          status = `${name}: avviato`;
          break;
        }
      }
    } catch (e) {
      status = "Avvio non riuscito: " + e;
    }
  }
  async function stopServer(provider: AiProvider) {
    const name = provider === "lmstudio" ? "LM Studio" : "Ollama";
    await persistAi();
    try {
      await aiServerStop(provider);
      status = `${name}: fermato`;
      setTimeout(() => verifyProvider(provider), 1000);
    } catch (e) {
      status = "Arresto non riuscito: " + e;
    }
  }
  async function summarizeDoc(doc: DocumentItem) {
    cardMenu = null;
    aiBusy = doc.id;
    status = "Riassunto in corso… (può richiedere un momento)";
    try {
      await summarizeDocument(doc.id);
      status = 'Riassunto generato — aprilo da "Modifica metadati"';
    } catch (e) {
      status = "Errore AI: " + e;
    } finally {
      aiBusy = null;
    }
  }
  async function autotagDoc(doc: DocumentItem) {
    cardMenu = null;
    aiBusy = doc.id;
    status = "Tag automatici in corso…";
    try {
      const tags = await autotagDocument(doc.id);
      status = "Tag aggiunti: " + tags.join(", ");
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore AI: " + e;
    } finally {
      aiBusy = null;
    }
  }
  /** Run summarize or autotag over every selected document, sequentially
   *  (local LLMs handle one request at a time), with live progress + cancel. */
  async function runBatchAi(kind: "summary" | "tags") {
    if (!selected.length || aiBusyAny) return; // no concurrent AI requests
    const ids = [...selected];
    batchCancel = false;
    aiBatch = { kind, done: 0, total: ids.length };
    let ok = 0;
    let errs = 0;
    let broke = false;
    let lastErr = "";
    for (const id of ids) {
      if (batchCancel) {
        broke = true;
        break;
      }
      try {
        if (kind === "summary") await summarizeDocument(id);
        else await autotagDocument(id);
        ok++;
      } catch (e) {
        errs++;
        lastErr = "" + e;
      }
      aiBatch = { kind, done: ok + errs, total: ids.length };
    }
    aiBatch = null;
    batchCancel = false;
    if (kind === "tags") {
      // Refresh so the new tag chips + sidebar appear.
      await loadDocs();
      await loadSidebar();
    }
    const label = kind === "summary" ? "Riassunti" : "Tag automatici";
    status =
      `${label}: ${ok} completati` +
      (errs ? ` · ${errs} errori (es. ${lastErr})` : "") +
      (broke ? " · interrotto" : "");
  }
  async function runDiscover() {
    // Allow searching by query OR author alone.
    if (!discoverQuery.trim() && !discoverAuthor.trim()) return;
    discovering = true;
    discoverResults = [];
    expandedAbstract = null;
    discoverNewIds = new Set();
    try {
      discoverResults = await discoverSearch(discoverQuery, discoverSource, {
        author: discoverAuthor.trim() || null,
        yearFrom: discoverYearFrom ? +discoverYearFrom : null,
        yearTo: discoverYearTo ? +discoverYearTo : null,
        oaOnly: discoverOaOnly,
        sort: discoverSort,
      });
      status = `${discoverResults.length} risultati`;
    } catch (e) {
      status = "" + e;
    } finally {
      discovering = false;
    }
  }
  function toggleAbstract(id: string) {
    expandedAbstract = expandedAbstract === id ? null : id;
  }
  // ----- Saved searches -----
  async function loadSaved() {
    try {
      savedSearches = await listSavedSearches();
    } catch {
      /* ignore */
    }
  }
  async function saveCurrentSearch() {
    const name = (discoverQuery.trim() || discoverAuthor.trim() || discoverSource).slice(0, 60);
    if (!name) return;
    savingSearch = true;
    try {
      await createSavedSearch({
        name,
        source: discoverSource,
        query: discoverQuery,
        author: discoverAuthor.trim() || null,
        yearFrom: discoverYearFrom ? +discoverYearFrom : null,
        yearTo: discoverYearTo ? +discoverYearTo : null,
        oaOnly: discoverOaOnly,
        sort: discoverSort,
        seenIds: discoverResults.map((r) => r.external_id),
      });
      await loadSaved();
      status = `Ricerca salvata: «${name}»`;
    } catch (e) {
      status = "Errore salvataggio ricerca: " + e;
    } finally {
      savingSearch = false;
    }
  }
  async function runSaved(s: SavedSearch) {
    if (filter.kind !== "discover") setFilter({ kind: "discover" });
    // Reflect the saved query in the search bar.
    discoverSource = s.source as typeof discoverSource;
    discoverQuery = s.query;
    discoverAuthor = s.author ?? "";
    discoverYearFrom = s.year_from != null ? String(s.year_from) : "";
    discoverYearTo = s.year_to != null ? String(s.year_to) : "";
    discoverOaOnly = s.oa_only;
    discoverSort = s.sort;
    discovering = true;
    discoverResults = [];
    discoverNewIds = new Set();
    expandedAbstract = null;
    try {
      const run = await runSavedSearch(s.id);
      discoverResults = run.results;
      discoverNewIds = new Set(run.new_ids);
      status =
        run.new_ids.length > 0
          ? `${run.new_ids.length} novità su ${run.results.length} risultati`
          : `Nessuna novità (${run.results.length} risultati)`;
      await loadSaved();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      discovering = false;
    }
  }
  async function removeSaved(s: SavedSearch) {
    if (!(await confirmAsk(`Eliminare la ricerca salvata «${s.name}»?`))) return;
    try {
      await deleteSavedSearch(s.id);
      await loadSaved();
    } catch {
      /* ignore */
    }
  }
  // ----- RAG engine -----
  async function loadRagStatus() {
    try {
      ragStatus = await ragIndexStatus();
    } catch {
      /* ignore */
    }
  }
  async function doBuildIndex() {
    if (ragBuilding) return;
    ragBuilding = true;
    ragProg = null;
    try {
      await buildRagIndex();
    } catch (e) {
      status = "Errore indicizzazione: " + e;
      ragBuilding = false;
    }
    // completion + progress handled by the "rag-progress" listener
  }
  async function doCancelIndex() {
    try {
      await cancelRagIndex();
    } catch {
      /* ignore */
    }
  }
  async function doRebuildIndex() {
    if (ragBuilding) return;
    if (!(await confirmAsk("Ricostruire l'indice da zero? I passaggi verranno rigenerati — utile per ottenere le pagine sui documenti già indicizzati.", "Ricostruisci", false))) return;
    try {
      await clearRagIndex();
    } catch (e) {
      status = "Errore: " + e;
      return;
    }
    await loadRagStatus();
    doBuildIndex();
  }
  async function doAsk() {
    if (!askQuestion.trim() || asking) return;
    asking = true;
    askAnswer = "";
    askSources = [];
    try {
      const r = await askLibrary(askQuestion, askScope?.kind, askScope?.id);
      askAnswer = r.answer; // normalize to the final, trimmed text
      askSources = r.sources;
    } catch (e) {
      status = "" + e;
    } finally {
      asking = false;
    }
  }
  /** Open the "ask" view scoped to a single document (from the ⋯ menu). */
  function askAboutDoc(d: DocumentItem) {
    cardMenu = null;
    askScope = { kind: "doc", id: d.id, label: d.title ?? "documento" };
    askAnswer = "";
    askSources = [];
    setFilter({ kind: "ask" });
    loadRagStatus();
  }
  /** Render an answer with [n] turned into clickable citation chips. */
  function answerParts(text: string): { t: string; n: number | null }[] {
    const parts: { t: string; n: number | null }[] = [];
    const re = /\[(\d+)\]/g;
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      if (m.index > last) parts.push({ t: text.slice(last, m.index), n: null });
      parts.push({ t: m[0], n: parseInt(m[1], 10) });
      last = m.index + m[0].length;
    }
    if (last < text.length) parts.push({ t: text.slice(last), n: null });
    return parts;
  }
  function openSourceN(n: number) {
    const s = askSources.find((x) => x.n === n);
    if (s) openById(s.document_id);
  }

  async function addResult(r: SearchResult) {
    addingExt = r.external_id;
    try {
      const res = await discoverAdd(r);
      discoverResults = discoverResults.map((x) =>
        x.external_id === r.external_id ? { ...x, in_library: true } : x,
      );
      status =
        res === "added_pdf"
          ? "Aggiunto con PDF ✓"
          : res === "added_ref"
            ? "Aggiunto (solo metadati) ✓"
            : "Già presente";
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      addingExt = null;
    }
  }

  // ----- Citations / export -----
  async function copyCite(doc: DocumentItem, format: string) {
    try {
      const text = await citeText([doc.id], format);
      await navigator.clipboard.writeText(text);
      cardMenu = null;
      status = format === "bibtex" ? "BibTeX copiato negli appunti" : "Citazione copiata negli appunti";
    } catch (e) {
      status = "Errore copia: " + e;
    }
  }

  /** Copy the stored citekey straight from the loaded document (no round-trip). */
  async function copyCitekey(doc: DocumentItem) {
    if (!doc.citekey) return;
    try {
      await navigator.clipboard.writeText(doc.citekey);
      status = `Citekey copiata: ${doc.citekey}`;
    } catch (e) {
      status = "Errore copia: " + e;
    }
  }

  // ----- Library health (maintenance scan) -----
  let healthModal = $state(false);
  let health = $state<LibraryHealth | null>(null);
  let healthLoading = $state(false);
  async function openHealth() {
    healthModal = true;
    healthLoading = true;
    health = null;
    try {
      health = await libraryHealth();
    } catch (e) {
      status = "Errore salute libreria: " + e;
      healthModal = false;
    } finally {
      healthLoading = false;
    }
  }
  function openHealthRow(id: number) {
    healthModal = false;
    openById(id);
  }

  // ----- Citation gap-finder -----
  let gapModal = $state(false);
  let gaps = $state<GapItem[]>([]);
  let gapsLoading = $state(false);
  async function openGaps() {
    gapModal = true;
    gapsLoading = true;
    gaps = [];
    try {
      gaps = await citationGaps(60);
    } catch (e) {
      status = "Errore gap citazioni: " + e;
      gapModal = false;
    } finally {
      gapsLoading = false;
    }
  }
  function gapSearchOnline(doi: string) {
    gapModal = false;
    discoverQuery = doi;
    setFilter({ kind: "discover" });
    runDiscover();
  }
  async function copyDoi(doi: string) {
    try {
      await navigator.clipboard.writeText(doi);
      status = "DOI copiato negli appunti";
    } catch {
      status = "Impossibile copiare negli appunti";
    }
  }

  async function exportLibrary() {
    const ids = displayed.map((d) => d.id);
    if (!ids.length) return;
    const path = await save({
      defaultPath: "references.bib",
      filters: [
        { name: "BibTeX", extensions: ["bib"] },
        { name: "RIS", extensions: ["ris"] },
        { name: "CSL-JSON", extensions: ["json"] },
      ],
    });
    if (!path) return;
    const ext = path.split(".").pop()?.toLowerCase();
    const format = ext === "ris" ? "ris" : ext === "json" ? "csljson" : "bibtex";
    try {
      await exportCitations(ids, format, path);
      status = `Esportati ${ids.length} riferimenti (${format})`;
    } catch (e) {
      status = "Errore export: " + e;
    }
  }

  // ----- Obsidian / Markdown vault -----
  async function pickObsidianVault() {
    const dir = await open({ directory: true, multiple: false, title: "Scegli la cartella del vault Obsidian" });
    if (typeof dir !== "string") return null;
    obsidianVault = dir;
    await setObsidianVault(dir);
    return dir;
  }
  async function runObsidianExport() {
    let vault = obsidianVault;
    if (!vault) {
      const picked = await pickObsidianVault();
      if (!picked) return;
      vault = picked;
    }
    // Export the current selection if any, otherwise everything shown.
    const ids = selected.length ? selected : displayed.map((d) => d.id);
    if (!ids.length) {
      status = "Niente da esportare";
      return;
    }
    exportingObsidian = true;
    try {
      const n = await exportToObsidian(ids, vault);
      status = `Esportate ${n} note in Obsidian (${baseName(vault)}/Scriptorium)`;
    } catch (e) {
      status = "Errore export Obsidian: " + e;
    } finally {
      exportingObsidian = false;
    }
  }

  // ----- Watched folder -----
  async function pickWatchedFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string") return;
    await setWatchedFolder(dir);
    watchedFolder = dir;
    status = "Cartella sorvegliata attiva";
  }
  async function clearWatchedFolder() {
    await setWatchedFolder(null);
    watchedFolder = null;
  }
  function baseName(p: string): string {
    const parts = p.split(/[\\/]/);
    return parts[parts.length - 1] || p;
  }

  // ----- Tags / collections actions -----
  async function makeTagAndAssign(doc: DocumentItem) {
    const name = newTagName.trim();
    if (!name) return;
    const color = PALETTE[tags.length % PALETTE.length];
    const t = await createTag(name, color);
    newTagName = "";
    await setDocumentTags(doc.id, [...doc.tags.map((x) => x.id), t.id]);
    await refreshAfterTagChange(doc.id);
  }

  async function toggleTag(doc: DocumentItem, tag: Tag) {
    const has = doc.tags.some((t) => t.id === tag.id);
    const ids = has
      ? doc.tags.filter((t) => t.id !== tag.id).map((t) => t.id)
      : [...doc.tags.map((t) => t.id), tag.id];
    await setDocumentTags(doc.id, ids);
    await refreshAfterTagChange(doc.id);
  }

  async function refreshAfterTagChange(docId: number) {
    await loadDocs();
    await loadSidebar();
    if (cardMenu) {
      const d = docs.find((x) => x.id === docId);
      cardMenu = d ? { ...cardMenu, doc: d } : null;
    }
  }

  async function removeTag(tag: Tag) {
    if (!(await confirmAsk(`Eliminare il tag «${tag.name}»? Verrà tolto da tutti i documenti.`))) return;
    await deleteTag(tag.id);
    tagFilter = tagFilter.filter((id) => id !== tag.id);
    await loadDocs();
    await loadSidebar();
  }

  async function addDocToCollection(doc: DocumentItem, coll: Collection) {
    await addToCollection(coll.id, doc.id);
    cardMenu = null;
    status = `Aggiunto a "${coll.name}"`;
  }

  async function makeCollection() {
    const name = newCollName.trim();
    if (!name) return;
    let rule: string | null = null;
    if (newCollSmart) {
      if (smartType === "untagged") rule = JSON.stringify({ type: "untagged" });
      else if (smartType === "year_gte")
        rule = JSON.stringify({ type: "year_gte", value: Number(smartValue) || 0 });
      else if (smartType === "tag")
        rule = JSON.stringify({ type: "tag", tagId: Number(smartValue) || -1 });
      else rule = JSON.stringify({ type: "text", query: smartValue });
    }
    await createCollection(name, newCollSmart, rule);
    newCollName = "";
    newCollSmart = false;
    smartValue = "";
    await loadSidebar();
  }

  async function removeColl(coll: Collection) {
    if (!(await confirmAsk(`Eliminare la collezione «${coll.name}»? I documenti restano in libreria.`))) return;
    await deleteCollection(coll.id);
    if (filter.kind === "collection" && filter.id === coll.id) filter = { kind: "all" };
    await loadDocs();
    await loadSidebar();
  }

  function openCardMenu(e: MouseEvent, doc: DocumentItem) {
    e.stopPropagation();
    cardMenu = { doc, x: e.clientX, y: e.clientY };
  }
  /** Right-click: open the actions menu at the cursor (suppress the native menu). */
  function onContext(e: MouseEvent, doc: DocumentItem) {
    e.preventDefault();
    openCardMenu(e, doc);
  }
  async function revealDoc(doc: DocumentItem) {
    cardMenu = null;
    try {
      await revealDocument(doc.id);
    } catch {
      status = "Questo elemento non ha un file da mostrare";
    }
  }

  /** Keep a fixed-position popover fully inside the window. */
  function clamp(node: HTMLElement, pos: { x: number; y: number }) {
    const apply = (p: { x: number; y: number }) => {
      const r = node.getBoundingClientRect();
      const m = 8;
      let x = p.x;
      let y = p.y;
      if (x + r.width + m > window.innerWidth) x = window.innerWidth - r.width - m;
      if (y + r.height + m > window.innerHeight) y = window.innerHeight - r.height - m;
      node.style.left = Math.max(m, x) + "px";
      node.style.top = Math.max(m, y) + "px";
    };
    apply(pos);
    return { update: apply };
  }

  /** Open the header Importa/Esporta dropdown anchored under its button. */
  function openHeaderMenu(e: MouseEvent, kind: "import" | "export") {
    e.stopPropagation();
    if (headerMenu?.kind === kind) {
      headerMenu = null;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    headerMenu = { kind, x: r.left, y: r.bottom + 6 };
  }

  let dragP: Promise<() => void> | undefined;
  let embP: Promise<() => void> | undefined;
  let watchP: Promise<() => void> | undefined;
  let ragP: Promise<() => void> | undefined;
  let askP: Promise<() => void> | undefined;
  let clearTimer: ReturnType<typeof setTimeout> | undefined;
  onMount(() => {
    loadDocs();
    loadStatus();
    loadSidebar();
    getWatchedFolder()
      .then((w) => (watchedFolder = w))
      .catch(() => {});
    getAiSettings()
      .then((a) => {
        aiEnabled = a.enabled;
        aiProvider = a.provider;
        ollamaUrl = a.ollama_url;
        lmstudioUrl = a.lmstudio_url;
        aiModel = a.model;
        aiEmbedGpu = a.embed_gpu;
        aiEmbedBatch = a.embed_batch;
        refreshAiStatus();
      })
      .catch(() => {});
    // Re-check reachability periodically: the indicator lights up once the user
    // starts Ollama / LM Studio, and dims if the server goes away.
    aiStatusTimer = setInterval(refreshAiStatus, 25000);
    dragP = getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") dragOver = true;
      else if (p.type === "drop") {
        dragOver = false;
        const pdfs = p.paths.filter((x) => x.toLowerCase().endsWith(".pdf"));
        handleImport(pdfs);
      } else dragOver = false;
    });
    embP = listen<EmbedProgress>("embed-progress", (e) => {
      embedProgress = e.payload;
      if (e.payload.phase === "done" || e.payload.phase === "cancelled") {
        loadStatus();
        clearTimeout(clearTimer);
        clearTimer = setTimeout(() => (embedProgress = null), 1800);
      }
    });
    watchP = listen("library-changed", () => {
      loadDocs();
      loadStatus();
    });
    ragP = listen<{ done: number; total: number; phase: string }>("rag-progress", (e) => {
      ragProg = { done: e.payload.done, total: e.payload.total };
      if (e.payload.phase === "done" || e.payload.phase === "cancelled") {
        ragBuilding = false;
        ragProg = null;
        loadRagStatus();
      }
    });
    askP = listen<string>("ask-token", (e) => {
      if (asking) askAnswer += e.payload;
    });
    return () => {
      dragP?.then((f) => f());
      embP?.then((f) => f());
      watchP?.then((f) => f());
      ragP?.then((f) => f());
      askP?.then((f) => f());
      clearTimeout(searchTimer);
      clearTimeout(clearTimer);
      clearInterval(aiStatusTimer);
    };
  });

  function authorLine(d: DocumentItem): string {
    if (!d.authors.length) return "";
    if (d.authors.length <= 3) return d.authors.join(", ");
    return d.authors.slice(0, 3).join(", ") + " et al.";
  }
  /** A document with no bibliographic metadata yet — author, year and venue are
   *  all empty. These are exactly the cards that look bare until "Metadati"
   *  (Crossref enrichment) fills them in, so we flag them in the UI. */
  function isBare(d: DocumentItem): boolean {
    return !d.authors.length && !d.year && !d.venue;
  }
  /** The paper's original link for sharing (DOI, else arXiv/landing), if known. */
  function paperLink(d: DocumentItem | null | undefined): string | undefined {
    return d?.paper_url ?? (d?.doi ? `https://doi.org/${d.doi}` : undefined);
  }
  /** Show all papers by an author (the clickable author chips/lines). */
  function showAuthor(name: string | undefined) {
    if (name && name.trim()) setFilter({ kind: "author", label: name.trim() });
  }

  const MODES: { value: SearchMode; label: string; desc: string }[] = [
    {
      value: "hybrid",
      label: "Tutto",
      desc: "Combina ricerca testuale e semantica e fonde i risultati (consigliato)",
    },
    {
      value: "fulltext",
      label: "Testo",
      desc: "Cerca le parole esatte nel testo dei PDF (veloce, non richiede l'indice)",
    },
    {
      value: "semantic",
      label: "Semantica",
      desc: "Trova per significato, anche con parole diverse (richiede l'indice semantico)",
    },
  ];
</script>

<svelte:window onclick={() => { cardMenu = null; headerMenu = null; }} />

{#snippet githubMark()}
  <svg class="ghmark" viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z" /></svg>
{/snippet}

{#snippet secretField(name: string, label: string, hint: string)}
  <div class="setlbl">
    {label}
    {#if hasKey[name] && !keyEditing[name]}
      <div class="airow">
        <span class="keyset">✓ impostata</span>
        <button class="ghost small" onclick={() => { keyEditing[name] = true; keyInput[name] = ""; }}>Sostituisci</button>
        <button class="ghost small" onclick={() => clearKey(name)}>Rimuovi</button>
      </div>
    {:else}
      <div class="airow">
        <input type="text" bind:value={keyInput[name]} placeholder="incolla la chiave e premi Salva…" onkeydown={(e) => e.key === "Enter" && saveKey(name)} />
        <button class="ghost small" onclick={() => saveKey(name)}>Salva</button>
        {#if hasKey[name]}<button class="ghost small" onclick={() => (keyEditing[name] = false)}>Annulla</button>{/if}
      </div>
    {/if}
    <span class="sethint">{hint}</span>
  </div>
{/snippet}

{#snippet pubBadge(status: string | null, link: string | null)}
  {#if status === "published"}
    <span class="pubbadge pub" title="Articolo peer-reviewed (pubblicato)">peer-reviewed</span>
  {:else if status === "preprint"}
    <span class="pubbadge pre" title="Preprint — nessuna versione peer-reviewed nota">preprint</span>
  {:else if status === "preprint_reviewed"}
    <span class="pubbadge pre" title="Preprint">preprint</span>
    {#if link}
      <button class="pubbadge pub link" title="La versione peer-reviewed esiste — apri (DOI)" onclick={(e) => { e.stopPropagation(); openInBrowser(link); }}>peer-reviewed ↗</button>
    {:else}
      <span class="pubbadge pub" title="La versione peer-reviewed esiste">peer-reviewed</span>
    {/if}
  {/if}
{/snippet}

<div class="app" class:drag={dragOver}>
  <header>
    <div class="brand">
      <h1>Scriptorium</h1>
      <span class="count" title="Documenti mostrati (o nel filtro attivo)">{tagFilter.length || query.trim() ? displayed.length : docs.length}</span>
      {#if aiStat?.enabled}
        <button
          class="aichip"
          class:active={aiStat.reachable && aiStat.model_available}
          class:warn={aiStat.reachable && !aiStat.model_available}
          title={aiStat.detail}
          onclick={openSettings}
          aria-label={aiStat.detail}
        >
          <span class="aidot"></span>AI
        </button>
      {/if}
    </div>
    <div class="searchgroup">
      <input
        class="search"
        type="search"
        placeholder="Cerca per testo o significato…"
        title="Cerca nei tuoi PDF"
        bind:value={query}
      />
      <select
        class="searchmode"
        bind:value={mode}
        title="Come cercare: Tutto (testo + significato), Testo (parole esatte) o Semantica (per significato)"
      >
        {#each MODES as m (m.value)}
          <option value={m.value}>{m.label}</option>
        {/each}
      </select>
      {#if searching}<span class="searchspin">cerco…</span>{/if}
    </div>
    <button
      class="ghost"
      class:attn={needsMeta > 0 && !enriching}
      onclick={enrichMeta}
      disabled={enriching || docs.length === 0}
      title={"Recupera da Crossref (tramite il DOI trovato nel PDF): titoli, autori, anno, rivista, abstract e l'elenco dei riferimenti.\nMolte funzioni si popolano solo dopo: «Riferimenti e citazioni», «Gap di citazioni» e i campi autore/anno/rivista delle schede." + (needsMeta > 0 ? `\n\n${needsMeta} ${needsMeta === 1 ? "documento è" : "documenti sono"} ancora senza metadati.` : "")}
    >
      {enriching ? "…" : "Metadati"}{#if needsMeta > 0 && !enriching}<span class="metabadge">{needsMeta}</span>{/if}
    </button>
    <button class="ghost" class:menuopen={headerMenu?.kind === "export"} onclick={(e) => openHeaderMenu(e, "export")} disabled={displayed.length === 0} title="Esporta: citazioni (BibTeX/RIS/CSL) o note Markdown per Obsidian">
      Esporta ▾
    </button>
    <button class="primary" class:menuopen={headerMenu?.kind === "import"} onclick={(e) => openHeaderMenu(e, "import")} title="Importa: PDF dal disco o una libreria BibTeX (.bib)">
      {busy || exportingObsidian ? "…" : "Importa ▾"}
    </button>
  </header>

  <div class="toolbar">
    <div class="modes">
      <button class="mode" class:active={view === "grid"} onclick={() => (view = "grid")} title="Vista a griglia (copertine)">Griglia</button>
      <button class="mode" class:active={view === "list"} onclick={() => (view = "list")} title="Vista a lista (dettagli, colonne ordinabili)">Lista</button>
      {#if filter.kind !== "trash" && filter.kind !== "discover" && filter.kind !== "duplicates" && filter.kind !== "ask" && displayed.length}
        <button class="mode" onclick={toggleSelectAll} title="Seleziona o deseleziona tutti i documenti mostrati (per le azioni multiple)">{allSelected ? "Deseleziona tutti" : "Seleziona tutti"}</button>
      {/if}
      {#if view === "grid" && filter.kind !== "discover" && filter.kind !== "ask"}
        <div class="gridzoom" title="Dimensione delle copertine nella griglia">
          <button class="zbtn" onclick={() => (gridSize = Math.max(120, gridSize - 30))} aria-label="Copertine più piccole" title="Più piccole">−</button>
          <input class="zrange" type="range" min="120" max="360" step="10" bind:value={gridSize} aria-label="Dimensione copertine" />
          <button class="zbtn" onclick={() => (gridSize = Math.min(360, gridSize + 30))} aria-label="Copertine più grandi" title="Più grandi">+</button>
        </div>
      {/if}
    </div>
    <div class="index">
      <select class="themesel" bind:value={theme} title="Tema colori dell'interfaccia">
        <optgroup label="Chiare">
          <option value="paper">Carta</option>
          <option value="sepia">Seppia</option>
          <option value="solarized">Solarized</option>
          <option value="sage">Salvia</option>
          <option value="pastel">Pastello</option>
          <option value="medieval">Medievale</option>
        </optgroup>
        <optgroup label="Scure">
          <option value="dark">Scuro</option>
          <option value="nord">Nord</option>
          <option value="graphite">Grafite</option>
          <option value="forest">Foresta</option>
          <option value="synthwave">Synthwave</option>
        </optgroup>
      </select>
      {#if embedProgress && embedProgress.phase !== "done" && embedProgress.phase !== "cancelled"}
        <span class="hint">
          {embedProgress.phase === "model" ? "Carico modello bge-m3…" : `Indicizzo ${embedProgress.done}/${embedProgress.total}`}
        </span>
        <div class="bar"><div class="fill" style="width:{embedProgress.total ? (embedProgress.done / embedProgress.total) * 100 : 8}%"></div></div>
        <button class="ghost small" onclick={stopIndex} title="Ferma l'indicizzazione (i documenti già indicizzati restano salvati)">Stop</button>
      {:else}
        <span class="hint" title="Quanti documenti hanno l'embedding per la ricerca semantica, sul totale">Indice semantico: {emb.embedded}/{emb.total}</span>
        <button
          class="ghost small"
          onclick={generateIndex}
          disabled={generating || emb.embedded >= emb.total || emb.total === 0}
          title="Calcola gli embedding dei documenti mancanti per abilitare la ricerca semantica (la prima volta scarica il modello ~2.3GB)"
        >
          {generating ? "…" : "Genera"}
        </button>
      {/if}
    </div>
  </div>

  {#if filter.kind !== "trash" && filter.kind !== "discover" && filter.kind !== "duplicates" && filter.kind !== "terminal" && filter.kind !== "ask" && displayed.length}
    <div class="sortbar">
      <span class="sortlabel" title="Clicca un criterio per attivarlo, di nuovo per invertire (▲/▼), ancora per toglierlo. Più criteri si combinano nell'ordine in cui li attivi (il numero indica la priorità).">Ordina:</span>
      {#each SORT_KEYS as k (k)}
        <button class="sortchip" class:on={sortDirOf(k)} onclick={() => cycleSort(k)} title={`Ordina per ${SORT_LABELS[k].toLowerCase()}`}>
          {SORT_LABELS[k]}{#if sortDirOf(k)}<span class="sar">{sortArrow(k)}</span>{#if sortChain.length > 1}<span class="srank">{sortRank(k)}</span>{/if}{/if}
        </button>
      {/each}
      {#if sortChain.length}<button class="sortclear" onclick={clearSort} title="Azzera l'ordinamento">azzera</button>{/if}
    </div>
  {/if}

  {#if status}<div class="status">{status}</div>{/if}

  {#if aiBatch}
    <div class="batchbar">
      <span>{aiBatch.kind === "summary" ? "Riassunto AI" : "Tag automatici AI"} in corso: {aiBatch.done}/{aiBatch.total}</span>
      <div class="bar"><div class="fill" style="width:{aiBatch.total ? (aiBatch.done / aiBatch.total) * 100 : 0}%"></div></div>
      <button class="ghost small" onclick={() => (batchCancel = true)} title="Interrompi l'operazione AI in corso">Stop</button>
    </div>
  {/if}

  <div class="body">
    <aside class="sidebar">
      <button class="navitem" class:active={filter.kind === "all"} onclick={() => setFilter({ kind: "all" })} title="Mostra tutti i documenti (rimuovi i filtri)">
        Tutti{#if facets.all}<span class="navcount">{facets.all}</span>{/if}
      </button>
      <button class="navitem" class:active={filter.kind === "favorite"} onclick={() => setFilter({ kind: "favorite" })} title="Solo i documenti contrassegnati come preferiti">
        Preferiti{#if facets.favorite}<span class="navcount">{facets.favorite}</span>{/if}
      </button>
      <button class="navitem" class:active={filter.kind === "unread"} onclick={() => setFilter({ kind: "unread" })} title="Solo i documenti non ancora segnati come letti">
        Da leggere{#if facets.unread}<span class="navcount">{facets.unread}</span>{/if}
      </button>
      <button class="navitem" class:active={filter.kind === "github"} onclick={() => setFilter({ kind: "github" })} title="Solo i documenti che citano un repository GitHub (codice disponibile)">
        Con codice (GitHub){#if facets.github}<span class="navcount">{facets.github}</span>{/if}
      </button>
      <button class="navitem" class:active={filter.kind === "peerreviewed"} onclick={() => setFilter({ kind: "peerreviewed" })} title="Solo gli articoli peer-reviewed (pubblicati), esclusi i preprint">
        Peer-reviewed{#if facets.peerreviewed}<span class="navcount">{facets.peerreviewed}</span>{/if}
      </button>

      <div class="sec tagsec">
        <button class="seclabel" onclick={() => (tagsCollapsed = !tagsCollapsed)} title="Comprimi o espandi i tag">
          <span class="chev" class:open={!tagsCollapsed}>▸</span>
          Tag
          <span class="seccount">{tags.length}</span>
        </button>
        <span class="secbtns">
          {#if tags.length > 1}
            <button class="secaction" onclick={() => (tagSort = tagSort === "asc" ? "desc" : "asc")} title="Ordina i tag in ordine alfabetico (A→Z / Z→A)">{tagSort === "asc" ? "A→Z" : "Z→A"}</button>
          {/if}
          {#if tagFilter.length}<button class="secaction" onclick={clearTags} title="Azzera il filtro per tag">azzera</button>{/if}
        </span>
      </div>
      {#if !tagsCollapsed}
        <div class="taglist">
          {#each displayedTags as t (t.id)}
            <div class="navrow">
              <button class="navitem" class:active={tagFilter.includes(t.id)} onclick={() => toggleTagFilter(t.id)} title={`Filtra per il tag "${t.name}" — puoi selezionarne più di uno`}>
                <span class="dot" style="background:{t.color ?? '#888'}"></span>{t.name}
                {#if tagFilter.includes(t.id)}<span class="navcheck">✓</span>{/if}
              </button>
              <button class="x" title="Elimina questo tag (lo rimuove da tutti i documenti)" onclick={() => removeTag(t)}>×</button>
            </div>
          {/each}
        </div>
      {/if}

      <div class="sec">Collezioni</div>
      {#each collections as c (c.id)}
        <div class="navrow">
          <button class="navitem" class:active={filter.kind === "collection" && filter.id === c.id} onclick={() => setFilter({ kind: "collection", id: c.id, label: c.name })} title={`Filtra per la collezione "${c.name}"${c.is_smart ? " (smart: si aggiorna da sola)" : ""}`}>
            {c.name}
          </button>
          <button class="x" title="Elimina la collezione (i documenti restano in libreria)" onclick={() => removeColl(c)}>×</button>
        </div>
      {/each}

      <div class="newcoll">
        <input
          placeholder="Nuova collezione…"
          title="Nome della nuova collezione. Premi Invio o «Crea»"
          bind:value={newCollName}
          onkeydown={(e) => e.key === "Enter" && makeCollection()}
        />
        <label class="smart" title="Collezione automatica: si popola da sola in base a una regola, invece di aggiungere i documenti a mano">
          <input type="checkbox" bind:checked={newCollSmart} /> smart
        </label>
        {#if newCollSmart}
          <select bind:value={smartType} title="Regola di appartenenza della collezione smart">
            <option value="untagged">Senza tag</option>
            <option value="year_gte">Anno ≥</option>
            <option value="tag">Per tag (id)</option>
            <option value="text">Testo</option>
          </select>
          {#if smartType !== "untagged"}
            <input class="sval" placeholder="valore" title="Valore della regola: anno minimo, id del tag, o testo da cercare" bind:value={smartValue} />
          {/if}
        {/if}
        <button class="ghost small" onclick={makeCollection} title="Crea la collezione">Crea</button>
      </div>

      {#if savedSearches.length}
        <div class="sec">Ricerche salvate</div>
        {#each savedSearches as s (s.id)}
          <div class="navrow">
            <button class="navitem" onclick={() => runSaved(s)} title={`Rilancia «${s.name}» e mostra le novità — fonte: ${s.source}`}>
              <span class="dot saveddot"></span>{s.name}
            </button>
            <button class="x" title="Elimina questa ricerca salvata" onclick={() => removeSaved(s)}>×</button>
          </div>
        {/each}
      {/if}

      <div class="sec">Strumenti</div>
      <button class="navitem" class:active={filter.kind === "ask"} onclick={() => { setFilter({ kind: "ask" }); loadRagStatus(); }} title="Fai domande alla tua libreria: risposte con citazioni dai tuoi documenti (AI locale)">Chiedi alla libreria</button>
      <button class="navitem" class:active={filter.kind === "discover"} onclick={() => setFilter({ kind: "discover" })} title="Cerca paper online (arXiv / OpenAlex / ADS) e aggiungili alla libreria">Scopri online</button>
      <button class="navitem" onclick={() => (idModal = true)} title="Aggiungi riferimenti incollando DOI / arXiv / ISBN / PMID (crea voci senza PDF allegato)">Aggiungi per ID</button>
      <button class="navitem" class:active={filter.kind === "duplicates"} onclick={() => setFilter({ kind: "duplicates" })} title="Trova e unisci documenti duplicati (per DOI o titolo+anno)">Duplicati</button>
      <button class="navitem" onclick={openHealth} title="Salute della libreria: file mancanti, PDF senza testo, metadati/incorporamenti/copertine mancanti, duplicati">Salute libreria</button>
      <button class="navitem" onclick={openGaps} title="I DOI che la tua libreria cita di più ma che non possiedi ancora">Gap di citazioni</button>
      <button class="navitem" class:active={filter.kind === "trash"} onclick={() => setFilter({ kind: "trash" })} title="Documenti eliminati: ripristina o elimina definitivamente">Cestino</button>
      <button class="navitem" class:active={filter.kind === "terminal"} onclick={() => { terminalOpened = true; setFilter({ kind: "terminal" }); }} title="Terminale integrato: usa claude code o altri strumenti a riga di comando sui tuoi PDF">Terminale</button>

      <div class="sec">Cartella sorvegliata</div>
      <div class="watched">
        {#if watchedFolder}
          <span class="wpath" title={watchedFolder}>{baseName(watchedFolder)}</span>
          <button class="x" title="Smetti di sorvegliare" onclick={clearWatchedFolder}>×</button>
        {:else}
          <button class="ghost small" onclick={pickWatchedFolder} title="Scegli una cartella: importa subito i PDF già presenti e poi quelli che aggiungerai automaticamente">Scegli cartella…</button>
        {/if}
      </div>

      <div class="appfoot">
        <button class="navitem foot" onclick={openSettings} title="Impostazioni: abilita la ricerca online, email, chiave OpenAlex e token ADS">Impostazioni</button>
        <button class="navitem foot" onclick={() => (helpModal = true)} title="Guida all'uso e scorciatoie da tastiera">Aiuto</button>
        <button class="navitem foot" onclick={() => (aboutModal = true)} title="Informazioni su Scriptorium">Informazioni</button>
      </div>
    </aside>

    <main class="main">
      {#if terminalOpened}
        <div class="termview" class:hidden={filter.kind !== "terminal"}>
          <Terminal onClose={closeTerminal} />
        </div>
      {/if}
      {#if filter.kind === "terminal"}
        <!-- terminal is rendered in the persistent host above -->
      {:else if filter.kind === "trash"}
        <div class="fbanner">
          <span>Cestino — {docs.length} {docs.length === 1 ? "elemento" : "elementi"}</span>
          {#if docs.length}<button onclick={emptyTrash} title="Elimina definitivamente tutto il cestino">Svuota cestino</button>{/if}
        </div>
        {#if docs.length === 0}
          <div class="empty"><p class="big">Cestino vuoto</p></div>
        {:else}
          <div class="listwrap">
            <table class="list">
              <tbody>
                {#each docs as d (d.id)}
                  <tr>
                    <td class="ttl">{d.title ?? "Senza titolo"}</td>
                    <td class="dim">{authorLine(d) || "—"}</td>
                    <td class="rowact">
                      <button class="rowbtn" title="Ripristina nella libreria" onclick={() => restoreFromTrash(d.id)}>Ripristina</button>
                      <button class="rowbtn del" title="Elimina definitivamente (irreversibile)" onclick={() => purgeFromTrash(d.id)}>✕</button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {:else if filter.kind === "duplicates"}
        <div class="fbanner"><span>Duplicati — {dupGroups.length} {dupGroups.length === 1 ? "gruppo" : "gruppi"}</span></div>
        {#if dupGroups.length === 0}
          <div class="empty"><p class="big">Nessun duplicato</p><p>Nessun doppione per DOI o titolo+anno.</p></div>
        {:else}
          <div class="dupwrap">
            {#each dupGroups as g, gi (gi)}
              <div class="dupgroup">
                <div class="duphead">
                  <span>{g.length} copie</span>
                  <button class="ghost small" onclick={() => doMerge(g)} title="Unisci nel primo: sposta tag/collezioni/annotazioni, gli altri finiscono nel cestino">Unisci</button>
                </div>
                {#each g as id, i (id)}
                  <div class="duprow">
                    <span class="badge">{i === 0 ? "master" : "↳"}</span>
                    <span class="dt" title={dupMap[id]?.title ?? ""}>{dupMap[id]?.title ?? "#" + id}</span>
                    <span class="dim">{dupMap[id] ? [dupMap[id].venue, dupMap[id].year].filter(Boolean).join(" · ") : ""}</span>
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        {/if}
      {:else if filter.kind === "ask"}
        <div class="askwrap">
          <div class="askhead">
            <h2 class="askh">Chiedi alla libreria</h2>
            <p class="askintro">Domande in linguaggio naturale → risposta con citazioni dai tuoi documenti. Tutto in locale (recupero dei passaggi + espansione su citazioni e documenti simili + AI locale).</p>
            <div class="askindex">
              {#if ragStatus}
                <span class="askstat">Indice: <strong>{ragStatus.indexed_docs}/{ragStatus.total_docs}</strong> documenti · {ragStatus.chunks} passaggi</span>
              {/if}
              {#if ragBuilding}
                <span class="askstat">Indicizzazione… {ragProg ? `${ragProg.done}/${ragProg.total}` : ""}</span>
                <button class="ghost small" onclick={doCancelIndex}>Stop</button>
              {:else}
                <button class="ghost small" onclick={doBuildIndex} title="Crea/aggiorna l'indice dei passaggi (necessario per le risposte)">
                  {ragStatus && ragStatus.indexed_docs < ragStatus.total_docs ? "Costruisci/aggiorna indice" : "Aggiorna indice"}
                </button>
                {#if ragStatus && ragStatus.chunks > 0}
                  <button class="ghost small" onclick={doRebuildIndex} title="Rigenera tutti i passaggi da zero (per ottenere le pagine sui documenti già indicizzati)">Ricostruisci</button>
                {/if}
              {/if}
            </div>
          </div>
          {#if !aiStat?.enabled}
            <p class="askwarn">Le funzioni AI sono disattivate. Abilitale in <strong>Impostazioni → AI locale</strong> (serve Ollama o LM Studio).</p>
          {:else if !aiStat?.reachable}
            <p class="askwarn">Provider AI non raggiungibile. Avvia Ollama/LM Studio o controlla le Impostazioni.</p>
          {/if}
          {#if askScope}
            <div class="askscope">
              Ambito: <strong>{askScope.label}</strong>
              <button class="scopex" title="Cerca in tutta la libreria" onclick={() => (askScope = null)}>✕</button>
            </div>
          {/if}
          <div class="askbar">
            <input
              class="askq"
              placeholder={askScope ? "Domanda su questo documento…" : "Es. cosa dicono i miei paper su…"}
              bind:value={askQuestion}
              onkeydown={(e) => e.key === "Enter" && doAsk()}
            />
            <button class="primary" onclick={doAsk} disabled={asking || !askQuestion.trim()}>{asking ? "…" : "Chiedi"}</button>
          </div>
          {#if asking && !askAnswer}
            <p class="askstat">Sto cercando nei tuoi documenti…</p>
          {/if}
          {#if askAnswer}
            <div class="askanswer">{#each answerParts(askAnswer) as p}{#if p.n !== null}<button class="citechip" title="Apri la fonte" onclick={() => openSourceN(p.n!)}>{p.t}</button>{:else}{p.t}{/if}{/each}{#if asking}<span class="caret">▍</span>{/if}</div>
            {#if askGroups.length}
              <div class="asksrc">
                <h3>Fonti</h3>
                <ul class="srclist">
                  {#each askGroups as g (g.document_id)}
                    <li>
                      <button class="hflink" onclick={() => openById(g.document_id)}>{g.title}</button>
                      {#if g.items[0].relation !== "match"}<span class="srcrel">{g.items[0].relation}</span>{/if}
                      <div class="passages">
                        {#each g.items as s (s.n)}
                          <span class="passchip" title={`Passaggio usato${s.page != null ? ` (p. ${s.page})` : ""}:\n\n${s.excerpt}`}>
                            [{s.n}]{#if s.page != null}<span class="ppage"> p. {s.page}</span>{/if}
                          </span>
                        {/each}
                      </div>
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          {/if}
        </div>
      {:else if filter.kind === "discover"}
        <div class="discbar">
          <select
            bind:value={discoverSource}
            title="Fonte di ricerca"
            onchange={() => { if (!CITES_SOURCES.includes(discoverSource) && discoverSort === "citations") discoverSort = "relevance"; }}>
            <option value="openalex">OpenAlex</option>
            <option value="arxiv">arXiv</option>
            <option value="ads">ADS</option>
            <option value="semanticscholar">Semantic Scholar</option>
            <option value="europepmc">Europe PMC</option>
            <option value="core">CORE</option>
            <option value="doaj">DOAJ</option>
            <option value="huggingface">Hugging Face (trending ML)</option>
          </select>
          <input
            class="discq"
            placeholder="Cerca paper online…"
            bind:value={discoverQuery}
            onkeydown={(e) => e.key === "Enter" && runDiscover()}
          />
          <input
            class="discau"
            placeholder="autore"
            title="Filtra per autore (opzionale)"
            bind:value={discoverAuthor}
            onkeydown={(e) => e.key === "Enter" && runDiscover()}
          />
          <input class="discy" type="number" placeholder="dal" title="Anno minimo" bind:value={discoverYearFrom} />
          <input class="discy" type="number" placeholder="al" title="Anno massimo" bind:value={discoverYearTo} />
          <label class="discoa" title="Mostra solo lavori Open Access"><input type="checkbox" bind:checked={discoverOaOnly} /> Solo OA</label>
          <select bind:value={discoverSort} title="Ordina i risultati">
            <option value="relevance">Rilevanza</option>
            <option value="date">Data</option>
            {#if CITES_SOURCES.includes(discoverSource)}<option value="citations">Citazioni</option>{/if}
          </select>
          <button class="primary" onclick={runDiscover} disabled={discovering}>{discovering ? "…" : "Cerca"}</button>
          <button class="ghost" onclick={saveCurrentSearch} disabled={savingSearch || (!discoverQuery.trim() && !discoverAuthor.trim())} title="Salva questa ricerca per monitorare le novità sul tema">{savingSearch ? "…" : "★ Salva"}</button>
        </div>
        {#if discoverResults.length === 0}
          <div class="empty">
            <p class="big">Cerca paper online</p>
            <p>Fonti: arXiv (preprint STEM), OpenAlex (tutto), ADS (astrofisica), Semantic Scholar (citazioni), Europe PMC (biomedicina), CORE (full-text OA), DOAJ (riviste OA). I PDF si scaricano solo se Open Access; gli altri si aggiungono come riferimento.</p>
          </div>
        {:else}
          <div class="discfilters">
            <span class="dflabel">Filtra:</span>
            <button
              class="dfchip"
              class:on={discCodeOnly}
              onclick={() => (discCodeOnly = !discCodeOnly)}
              title="Mostra solo i paper che pubblicano anche il codice (repository GitHub rilevato)">
              {@render githubMark()} Con codice
              <span class="dfcount">{discoverResults.filter((r) => !!r.github_url).length}</span>
            </button>
            <button
              class="dfchip"
              class:on={discPeerOnly}
              onclick={() => { discPeerOnly = !discPeerOnly; if (discPeerOnly) discPreprintOnly = false; }}
              title="Mostra solo i paper peer-reviewed (esiste una versione pubblicata)">
              Peer-reviewed
              <span class="dfcount">{discoverResults.filter(isPeer).length}</span>
            </button>
            <button
              class="dfchip"
              class:on={discPreprintOnly}
              onclick={() => { discPreprintOnly = !discPreprintOnly; if (discPreprintOnly) discPeerOnly = false; }}
              title="Mostra solo i preprint">
              Preprint
              <span class="dfcount">{discoverResults.filter(isPreprint).length}</span>
            </button>
            {#if discCodeOnly || discPeerOnly || discPreprintOnly}
              <button class="dfclear" onclick={() => { discCodeOnly = false; discPeerOnly = false; discPreprintOnly = false; }} title="Rimuovi tutti i filtri">✕ azzera</button>
              <span class="dfshown">{discDisplayed.length} / {discoverResults.length} mostrati</span>
            {/if}
          </div>
          <div class="listwrap">
            <table class="list">
              <colgroup>
                <col />
                <col class="c-auth" />
                <col class="c-year" />
                <col class="c-venue" />
                <col class="c-cit" />
                <col class="c-act2" />
              </colgroup>
              <thead>
                <tr>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => toggleDiscSort("title")} title="Ordina per titolo">Titolo<span class="ar">{discArrow("title")}</span></th>
                  <th>Autori</th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable num" onclick={() => toggleDiscSort("year")} title="Ordina per anno">Anno<span class="ar">{discArrow("year")}</span></th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => toggleDiscSort("venue")} title="Ordina per rivista">Rivista<span class="ar">{discArrow("venue")}</span></th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable num" onclick={() => toggleDiscSort("citations")} title="Ordina per citazioni">Cit.<span class="ar">{discArrow("citations")}</span></th>
                  <th aria-label="azioni"></th>
                </tr>
              </thead>
              <tbody>
                {#each discDisplayed as r (r.source + r.external_id)}
                  {@const rid = r.source + r.external_id}
                  <tr>
                    <td class="ttl">
                      {#if r.in_library}
                        <span class="addbtn inlib" title="Già nella libreria">✓</span>
                      {:else}
                        <button class="addbtn" onclick={() => addResult(r)} disabled={addingExt === r.external_id} title="Aggiungi alla libreria (scarica il PDF se Open Access)" aria-label="Aggiungi alla libreria">{addingExt === r.external_id ? "…" : "+"}</button>
                      {/if}
                      {#if r.abstract_text}
                        <button
                          class="abstoggle"
                          class:open={expandedAbstract === rid}
                          onclick={() => toggleAbstract(rid)}
                          title={expandedAbstract === rid ? "Nascondi abstract" : "Mostra abstract"}
                          aria-label="Mostra/nascondi abstract">▸</button>
                      {/if}
                      {#if discoverNewIds.has(r.external_id)}<span class="nuovo" title="Nuovo dall'ultima volta che hai eseguito questa ricerca salvata">novità</span>{/if}
                      <span title={r.title ?? ""}>{r.title ?? "Senza titolo"}</span>
                      {#if r.pub_status}<span class="badgeinline">{@render pubBadge(r.pub_status, r.url)}</span>{/if}
                    </td>
                    <td class="dim" title={r.authors.join(", ")}>{r.authors.slice(0, 3).join(", ")}{r.authors.length > 3 ? " et al." : ""}</td>
                    <td class="num dim">{r.year ?? "—"}</td>
                    <td class="dim" title={r.venue ?? ""}>{r.venue || "—"}</td>
                    <td class="num dim">{r.citations || "—"}</td>
                    <td class="rowact">
                      {#if r.github_url}<button class="ghicon" title="Codice su GitHub: {r.github_url}" aria-label="Repository GitHub" onclick={() => openInBrowser(r.github_url!)}>{@render githubMark()}</button>{/if}
                      {#if r.oa_pdf_url}<span class="badge2 oa">OA</span>{/if}
                      {#if r.url}
                        <button class="rowbtn ghost" onclick={() => openInBrowser(r.url!)} title="Apri la pagina del paper nel browser">Apri</button>
                      {/if}
                    </td>
                  </tr>
                  {#if expandedAbstract === rid && r.abstract_text}
                    <tr class="absrow">
                      <td colspan="6"><div class="abswrap">{r.abstract_text}</div></td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
            {#if discDisplayed.length === 0}
              <p class="dfempty">Nessun risultato con i filtri attivi. <button class="linklike" onclick={() => { discCodeOnly = false; discPeerOnly = false; discPreprintOnly = false; }}>Azzera i filtri</button></p>
            {/if}
          </div>
        {/if}
      {:else}
        <div class="topbars">
        {#if filter.kind !== "all"}
          <div class="fbanner">
            <span>
              {filter.kind === "related"
                ? "Correlati a"
                : filter.kind === "collection"
                  ? "Collezione"
                  : filter.kind === "favorite"
                    ? "Preferiti"
                    : filter.kind === "author"
                      ? "Autore"
                      : filter.kind === "github"
                        ? "Con codice (GitHub)"
                        : filter.kind === "peerreviewed"
                          ? "Peer-reviewed"
                          : "Da leggere"}{filter.label ? ": " : ""}<strong>{filter.label ?? ""}</strong>
            </span>
            <button onclick={() => setFilter({ kind: "all" })} title="Rimuovi il filtro e mostra tutto">Mostra tutti</button>
          </div>
        {/if}
        {#if tagFilter.length}
          <div class="fbanner tagbanner">
            <span class="tagfilter">
              <span class="tflabel">Filtro tag:</span>
              {#each tagFilter as tid (tid)}
                {@const tg = tags.find((x) => x.id === tid)}
                {#if tg}
                  <span class="tfchip" style="background:{(tg.color ?? '#888888')}22; border-color:{tg.color ?? '#888888'}">
                    {tg.name}
                    <button class="tfx" title="Rimuovi questo tag dal filtro" onclick={() => toggleTagFilter(tid)}>×</button>
                  </span>
                {/if}
              {/each}
              {#if tagFilter.length > 1}
                <span class="tfmode">
                  <button class:active={tagMode === "all"} onclick={() => setTagMode("all")} title="Mostra i documenti che hanno TUTTI i tag selezionati">tutti</button>
                  <button class:active={tagMode === "any"} onclick={() => setTagMode("any")} title="Mostra i documenti che hanno ALMENO UNO dei tag selezionati">qualsiasi</button>
                </span>
              {/if}
            </span>
            <button onclick={clearTags} title="Rimuovi il filtro per tag">Azzera</button>
          </div>
        {/if}
        {#if selected.length}
          <div class="bulkbar">
            <span>{selected.length} selezionati</span>
            <button onclick={printSelected} disabled={printing} title="Stampa i documenti selezionati come un unico lavoro di stampa">{printing ? "…" : "Stampa"}</button>
            <ShareMenu
              ids={selected}
              label={selected.length === 1
                ? (displayed.find((d) => d.id === selected[0])?.title ?? "Documento PDF")
                : `${selected.length} documenti PDF`}
              link={selected.length === 1 ? paperLink(displayed.find((d) => d.id === selected[0])) : null}
              compact
              onstatus={(s) => (status = s)}
            />
            {#if aiEnabled}
              <button onclick={() => runBatchAi("summary")} disabled={aiBusyAny} title="Genera un riassunto AI per ogni documento selezionato">{aiBatch?.kind === "summary" ? `Riassunto ${aiBatch.done}/${aiBatch.total}…` : "Riassumi (AI)"}</button>
              <button onclick={() => runBatchAi("tags")} disabled={aiBusyAny} title="Genera tag automatici AI per ogni documento selezionato">{aiBatch?.kind === "tags" ? `Tag ${aiBatch.done}/${aiBatch.total}…` : "Tag automatici (AI)"}</button>
            {/if}
            <button class="del" onclick={() => trashSelected(selected)} title="Sposta i selezionati nel cestino">Elimina</button>
            <select title="Aggiungi un tag ai selezionati" onchange={(e) => { const t = tags.find((x) => x.id === +e.currentTarget.value); if (t) bulkAddTag(t); e.currentTarget.value = ""; }}>
              <option value="">+ Tag…</option>
              {#each tags as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
            </select>
            <select title="Aggiungi i selezionati a una collezione" onchange={(e) => { const c = collections.find((x) => x.id === +e.currentTarget.value); if (c) bulkAddCollection(c); e.currentTarget.value = ""; }}>
              <option value="">+ Collezione…</option>
              {#each collections.filter((c) => !c.is_smart) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
            </select>
            <button onclick={() => (selected = [])} title="Annulla la selezione">Deseleziona</button>
          </div>
        {/if}
        </div>
        {#if filter.kind === "all" && !query.trim() && !tagFilter.length && recentDocs.length}
          <section class="recentshelf">
            <h2 class="shelfh">Continua a leggere</h2>
            <div class="shelf">
              {#each recentDocs as d (d.id)}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div class="shelfcard" role="button" tabindex="0" title={d.title ?? "Senza titolo"} onclick={() => (openDoc = d)} onkeydown={(e) => { if (e.key === "Enter") openDoc = d; }}>
                  <div class="shelfthumb">
                    {#if thumbs[d.id]}<img src={thumbs[d.id]} alt="" />{:else}<div class="thumb-placeholder">PDF</div>{/if}
                  </div>
                  <span class="shelftitle">{d.title ?? "Senza titolo"}</span>
                </div>
              {/each}
            </div>
          </section>
        {/if}
        {#if displayed.length === 0}
          <div class="empty">
            {#if query.trim()}
              <p class="big">Nessun risultato</p><p>Prova un'altra ricerca o cambia modalità.</p>
            {:else if tagFilter.length}
              <p class="big">Nessun documento con questi tag</p><p>Prova a togliere un tag{tagFilter.length > 1 ? " o passa a «qualsiasi»" : ""}, oppure premi «Azzera».</p>
            {:else if filter.kind !== "all"}
              <p class="big">Vuoto</p><p>Nessun documento in «{filter.label}».</p>
            {:else}
              <p class="big">Nessun documento</p><p>Trascina qui dei PDF, oppure usa <strong>+ Importa PDF</strong>.</p>
            {/if}
          </div>
        {:else if view === "grid"}
          <div class="grid" style="--grid-min: {gridSize}px">
            {#each displayed as d (d.id)}
              <article class="card" class:selcard={selected.includes(d.id)} role="button" tabindex="0" onclick={() => (openDoc = d)} oncontextmenu={(e) => onContext(e, d)} onkeydown={(e) => { if (e.key === "Enter") openDoc = d; }}>
                <button class="dots" title="Altre azioni (anche col tasto destro)" onclick={(e) => openCardMenu(e, d)}>⋯</button>
                <button class="cardsel" class:on={selected.includes(d.id)} title="Seleziona per azioni multiple" aria-label="Seleziona" onclick={(e) => { e.stopPropagation(); toggleSelect(d.id); }}>{selected.includes(d.id) ? "✓" : ""}</button>
                <button class="starbtn" class:on={d.favorite} title={d.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti"} aria-label="Preferito" onclick={(e) => { e.stopPropagation(); toggleFavorite(d); }}>{d.favorite ? "★" : "☆"}</button>
                <div class="thumb">
                  {#if thumbs[d.id]}<img src={thumbs[d.id]} alt="" />{:else}<div class="thumb-placeholder">PDF</div>{/if}
                </div>
                <div class="meta">
                  <h3 title={d.title ?? ""}>{d.title ?? "Senza titolo"}</h3>
                  {#if authorLine(d)}<p class="authors"><button type="button" class="authorlink" title={`Mostra tutti i lavori di ${d.authors[0]}`} onclick={(e) => { e.stopPropagation(); showAuthor(d.authors[0]); }}>{authorLine(d)}</button></p>{/if}
                  {#if d.year || d.venue}<p class="venue">{[d.venue, d.year].filter(Boolean).join(" · ")}</p>{/if}
                  {#if d.citekey && !isBare(d)}<button type="button" class="ckey" title={`Citekey: ${d.citekey} — clic per copiare`} aria-label={`Copia citekey ${d.citekey}`} onclick={(e) => { e.stopPropagation(); copyCitekey(d); }}>{d.citekey}</button>{/if}
                  {#if isBare(d)}<p class="metamiss" title="Autori, anno e rivista non ancora recuperati. Premi «Metadati» (in alto) per recuperarli da Crossref.">ⓘ metadati non recuperati</p>{/if}
                  {#if d.pub_status}<div class="badgerow">{@render pubBadge(d.pub_status, d.paper_url)}</div>{/if}
                  {#if d.github_url}
                    <button class="ghchip" title={`Apri il repository GitHub: ${d.github_url}`} aria-label="Apri repository GitHub" onclick={(e) => { e.stopPropagation(); openInBrowser(d.github_url!); }}>{@render githubMark()} codice</button>
                  {/if}
                  {#if d.tags.length}
                    <div class="chips">
                      {#each d.tags as t (t.id)}<span class="chip" style="background:{(t.color ?? '#888')}33; border-color:{t.color ?? '#888'}">{t.name}</span>{/each}
                    </div>
                  {/if}
                </div>
              </article>
            {/each}
          </div>
        {:else}
          <div class="listwrap">
            <table class="list">
              <colgroup>
                <col class="c-sel" />
                <col class="c-title" />
                <col class="c-auth" />
                <col class="c-year" />
                <col class="c-venue" />
                <col class="c-tags" />
                <col class="c-doi" />
                <col class="c-date" />
                <col class="c-act" />
              </colgroup>
              <thead>
                <tr>
                  <th></th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => cycleSort("title")} title="Ordina per titolo (clicca di nuovo per invertire, ancora per togliere)">Titolo<span class="ar">{sortArrow("title")}</span>{#if sortChain.length > 1 && sortRank("title")}<span class="ar rnk">{sortRank("title")}</span>{/if}</th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => cycleSort("author")} title="Ordina per primo autore (clicca di nuovo per invertire, ancora per togliere)">Autori<span class="ar">{sortArrow("author")}</span>{#if sortChain.length > 1 && sortRank("author")}<span class="ar rnk">{sortRank("author")}</span>{/if}</th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable num" onclick={() => cycleSort("year")} title="Ordina per anno (clicca di nuovo per invertire, ancora per togliere)">Anno<span class="ar">{sortArrow("year")}</span>{#if sortChain.length > 1 && sortRank("year")}<span class="ar rnk">{sortRank("year")}</span>{/if}</th>
                  <th>Rivista</th>
                  <th>Tag</th>
                  <th>DOI</th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => cycleSort("added")} title="Ordina per data di aggiunta (clicca di nuovo per invertire, ancora per togliere)">Aggiunto<span class="ar">{sortArrow("added")}</span>{#if sortChain.length > 1 && sortRank("added")}<span class="ar rnk">{sortRank("added")}</span>{/if}</th>
                  <th aria-label="azioni"></th>
                </tr>
              </thead>
              <tbody>
                {#each displayed as d (d.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <tr onclick={() => (openDoc = d)} oncontextmenu={(e) => onContext(e, d)} class:selrow={selected.includes(d.id)}>
                    <td class="sel"><input type="checkbox" checked={selected.includes(d.id)} onclick={(e) => e.stopPropagation()} onchange={() => toggleSelect(d.id)} title="Seleziona" /></td>
                    <td class="ttl" title={d.title ?? ""}><button class="starinline" class:on={d.favorite} title={d.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti"} aria-label="Preferito" onclick={(e) => { e.stopPropagation(); toggleFavorite(d); }}>{d.favorite ? "★" : "☆"}</button>{d.title ?? "Senza titolo"}{#if d.github_url}<button class="ghicon" title={`Apri il repository GitHub: ${d.github_url}`} aria-label="Apri repository GitHub" onclick={(e) => { e.stopPropagation(); openInBrowser(d.github_url!); }}>{@render githubMark()}</button>{/if}{#if d.citekey && !isBare(d)}<button type="button" class="ckey-inline" title={`Citekey: ${d.citekey} — clic per copiare`} aria-label={`Copia citekey ${d.citekey}`} onclick={(e) => { e.stopPropagation(); copyCitekey(d); }}>{d.citekey}</button>{/if}{#if isBare(d)}<span class="metamiss-inline" title="Autori, anno e rivista non ancora recuperati. Premi «Metadati» (in alto) per recuperarli da Crossref.">ⓘ</span>{/if}</td>
                    <td class="dim" title={authorLine(d)}>{#if authorLine(d)}<button type="button" class="authorlink" title={`Mostra tutti i lavori di ${d.authors[0]}`} onclick={(e) => { e.stopPropagation(); showAuthor(d.authors[0]); }}>{authorLine(d)}</button>{:else}—{/if}</td>
                    <td class="num dim">{d.year ?? "—"}</td>
                    <td class="dim" title={d.venue ?? ""}>{d.venue || "—"}{#if d.pub_status}<span class="badgeinline">{@render pubBadge(d.pub_status, d.paper_url)}</span>{/if}</td>
                    <td>
                      <div class="tagcell">
                        {#each d.tags.slice(0, 2) as t (t.id)}<span class="chip" style="background:{(t.color ?? '#888')}33; border-color:{t.color ?? '#888'}">{t.name}</span>{/each}
                        {#if d.tags.length > 2}<span class="more">+{d.tags.length - 2}</span>{/if}
                      </div>
                    </td>
                    <td class="doi dim" title={d.doi ?? ""}>{d.doi || "—"}</td>
                    <td class="num dim">{(d.added_at ?? "").slice(0, 10)}</td>
                    <td class="rowact">
                      <button class="rowbtn" title="Modifica metadati" onclick={(e) => { e.stopPropagation(); editingId = d.id; }}>Modifica</button>
                      <button class="rowbtn" title="Tag, collezioni, cita, elimina" onclick={(e) => openCardMenu(e, d)}>⋯</button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/if}
    </main>
  </div>

  {#if dragOver}<div class="dropmask"><span>Rilascia i PDF per importarli</span></div>{/if}

  {#if openDoc}
    <Viewer id={openDoc.id} title={openDoc.title ?? "PDF"} link={paperLink(openDoc)} onClose={() => (openDoc = null)} />
  {/if}

  {#if headerMenu}
    <div class="menu" use:clamp={{ x: headerMenu.x, y: headerMenu.y }}>
      {#if headerMenu.kind === "import"}
        <button class="medit" onclick={() => { headerMenu = null; importViaDialog(); }}>PDF dal disco…</button>
        <button class="medit" onclick={() => { headerMenu = null; importBibtexDialog(); }}>Da BibTeX (.bib) — Zotero/Mendeley…</button>
      {:else}
        <button class="medit" onclick={() => { headerMenu = null; exportLibrary(); }} disabled={displayed.length === 0}>Citazioni (BibTeX / RIS / CSL)…</button>
        <button class="medit" onclick={() => { headerMenu = null; runObsidianExport(); }} disabled={exportingObsidian || displayed.length === 0}>In Obsidian (note Markdown)</button>
      {/if}
    </div>
  {/if}

  {#if cardMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="menu" use:clamp={{ x: cardMenu.x, y: cardMenu.y }} onclick={(e) => e.stopPropagation()}>
      <button class="medit" onclick={() => { openDoc = cardMenu!.doc; cardMenu = null; }}>Apri</button>
      <button class="medit" onclick={() => { editingId = cardMenu!.doc.id; cardMenu = null; }}>Modifica metadati</button>
      <button class="medit" onclick={() => toggleFavorite(cardMenu!.doc)}>{cardMenu.doc.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti"}</button>
      <button class="medit" onclick={() => toggleRead(cardMenu!.doc)}>{cardMenu.doc.is_read ? "Segna come da leggere" : "Segna come letto"}</button>
      <button class="medit" onclick={() => printOne(cardMenu!.doc)} disabled={printing} title="Stampa questo documento">Stampa</button>
      <ShareMenu ids={[cardMenu.doc.id]} label={cardMenu.doc.title ?? "Documento PDF"} link={paperLink(cardMenu.doc)} variant="menuitem" onstatus={(s) => (status = s)} onclose={() => (cardMenu = null)} />
      <button class="medit" onclick={() => revealDoc(cardMenu!.doc)} title="Apri la cartella che contiene il PDF">Mostra nella cartella</button>
      {#if !cardMenu.doc.has_thumb}
        <button class="medit" onclick={() => doFindPdf(cardMenu!.doc)} title="Cerca un PDF Open Access (Unpaywall / arXiv) e allegalo a questo riferimento">Trova PDF</button>
      {/if}
      {#if aiEnabled}
        <button class="medit" onclick={() => summarizeDoc(cardMenu!.doc)} disabled={aiBusyAny} title="Genera un riassunto in italiano con Ollama (locale)">{aiBusy === cardMenu.doc.id ? "AI…" : "Riassumi (AI)"}</button>
        <button class="medit" onclick={() => autotagDoc(cardMenu!.doc)} disabled={aiBusyAny} title="Suggerisci e assegna tag tematici con Ollama (locale)">{aiBusy === cardMenu.doc.id ? "AI…" : "Tag automatici (AI)"}</button>
      {/if}
      <button class="medit" title="Mostra i documenti più simili per significato (richiede l'indice semantico)" onclick={() => { setFilter({ kind: "related", id: cardMenu!.doc.id, label: cardMenu!.doc.title ?? "documento" }); cardMenu = null; }}>Correlati</button>
      <button class="medit" title="Repository GitHub citati dal paper + modelli/dataset su Hugging Face" onclick={() => openHf(cardMenu!.doc)}>Codice & repository (GitHub + HF)</button>
      <button class="medit" title="Bibliografia del paper e documenti della tua libreria che lo citano" onclick={() => openCitations(cardMenu!.doc)}>Riferimenti e citazioni</button>
      <button class="medit" title="Fai una domanda solo su questo documento (AI locale)" onclick={() => askAboutDoc(cardMenu!.doc)}>Chiedi su questo documento</button>
      <button class="medit" onclick={() => copyCite(cardMenu!.doc, "apa")} title="Copia la citazione formattata (APA) negli appunti">Copia citazione (APA)</button>
      <button class="medit" onclick={() => copyCite(cardMenu!.doc, "bibtex")} title="Copia la voce BibTeX negli appunti">Copia BibTeX</button>
      <button class="medit" onclick={() => copyCite(cardMenu!.doc, "citekey")} title="Copia la chiave di citazione (es. vaswani2017attention)">Copia citekey</button>
      <button class="medit" onclick={() => copyCite(cardMenu!.doc, "latex")} title={"Copia \\cite{key} pronto per LaTeX"}>Copia \cite&#123;…&#125;</button>
      <button class="medit del" onclick={() => trashSelected([cardMenu!.doc.id])} title="Sposta nel cestino (recuperabile)">Elimina</button>
      <div class="mtitle">Tag</div>
      {#each tags as t (t.id)}
        <label class="mtag">
          <input type="checkbox" checked={cardMenu.doc.tags.some((x) => x.id === t.id)} onchange={() => toggleTag(cardMenu!.doc, t)} />
          <span class="dot" style="background:{t.color ?? '#888'}"></span>{t.name}
        </label>
      {/each}
      <div class="mnew">
        <input placeholder="nuovo tag…" bind:value={newTagName} onkeydown={(e) => e.key === "Enter" && makeTagAndAssign(cardMenu!.doc)} />
        <button class="ghost small" onclick={() => makeTagAndAssign(cardMenu!.doc)}>+</button>
      </div>
      {#if collections.filter((c) => !c.is_smart).length}
        <div class="mtitle">Aggiungi a</div>
        {#each collections.filter((c) => !c.is_smart) as c (c.id)}
          <button class="mcoll" onclick={() => addDocToCollection(cardMenu!.doc, c)}>{c.name}</button>
        {/each}
      {/if}
    </div>
  {/if}

  {#if editingId !== null}
    <MetaEditor
      id={editingId}
      onClose={() => (editingId = null)}
      onSaved={async () => {
        editingId = null;
        await loadDocs();
        await loadSidebar();
      }}
    />
  {/if}

  {#if idModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) idModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div class="idmodal" onclick={(e) => e.stopPropagation()}>
        <h2>Aggiungi per identificatore</h2>
        <p class="dimtext">Incolla DOI, ID arXiv, ISBN o PMID — uno per riga. Recupero i metadati e creo le voci (senza PDF allegato).</p>
        <textarea
          rows="6"
          bind:value={idText}
          placeholder={"10.1038/nature14539\n2301.12345\n9780262033848\npmid:31452104"}
        ></textarea>
        <div class="modactions">
          <button class="ghost" onclick={() => (idModal = false)}>Annulla</button>
          <button class="primary" onclick={addIds} disabled={addingIds}>{addingIds ? "…" : "Aggiungi"}</button>
        </div>
      </div>
    </div>
  {/if}

  {#if aboutModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) aboutModal = false; }} role="presentation">
      <div class="idmodal aboutbox" role="dialog" tabindex="-1">
        <div class="abouthead">
          <span class="aboutmark">S</span>
          <h2>Scriptorium</h2>
          <p class="aboutver">versione {APP_VERSION} · {APP_YEAR}</p>
        </div>
        <p class="abouttag">Gestore di PDF e riferimenti, locale e veloce. Tutto resta sul tuo computer; le funzioni di rete e AI sono opzionali e disattivabili.</p>
        <p class="aboutmeta">Costruito con Tauri · Rust · SvelteKit · SQLite.</p>
        <p class="aboutcopy">© {APP_YEAR} Scriptorium</p>
        <div class="modactions"><button class="primary" onclick={() => (aboutModal = false)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if confirmBox}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback confirmback" onmousedown={(e) => { if (e.target === e.currentTarget) answerConfirm(false); }} role="presentation">
      <div class="confirmbox" role="dialog" tabindex="-1">
        <p class="confirmmsg">{confirmBox.msg}</p>
        <div class="modactions">
          <button class="ghost" onclick={() => answerConfirm(false)}>Annulla</button>
          <button class="primary" class:danger={confirmBox.danger} onclick={() => answerConfirm(true)}>{confirmBox.ok}</button>
        </div>
      </div>
    </div>
  {/if}

  {#if citModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) citModal = false; }} role="presentation">
      <div class="idmodal hfwide" role="dialog" tabindex="-1">
        <h2>Riferimenti e citazioni</h2>
        <p class="dimtext" title={citTitle}>{citTitle}</p>
        {#if citLoading}
          <p class="dimtext">Carico i riferimenti…</p>
        {:else if citData}
          <div class="hfsec ghsec">
            <h3>Citato nella tua libreria ({citData.cited_by.length})</h3>
            {#if citData.cited_by.length}
              <ul class="hflist">
                {#each citData.cited_by as c (c.id)}
                  <li><button class="hflink" onclick={() => openById(c.id)} title="Apri questo documento">{c.title ?? "Senza titolo"}{c.year ? ` (${c.year})` : ""}</button></li>
                {/each}
              </ul>
            {:else}<p class="dimtext">Nessun documento della libreria cita questo paper (servono i metadati/DOI dei tuoi documenti).</p>{/if}
          </div>
          <div class="hfsec">
            <h3>Bibliografia ({citData.references.length})</h3>
            {#if citData.references.length}
              <ul class="hflist reflist">
                {#each citData.references as r, i (i)}
                  <li class="refrow">
                    {#if r.in_library}
                      <button class="hflink" onclick={() => openById(r.in_library!)} title="Nella tua libreria — apri">{r.title ?? r.raw ?? r.ref_doi}</button>
                      <span class="badge2 inlibref">in libreria</span>
                    {:else}
                      <span class="reftext">{r.raw ?? r.ref_doi ?? "—"}</span>
                      {#if r.ref_doi}<button class="hflink small" onclick={() => openInBrowser(`https://doi.org/${r.ref_doi}`)} title="Apri il DOI">DOI ↗</button>{/if}
                    {/if}
                  </li>
                {/each}
              </ul>
            {:else}<p class="dimtext">Nessun riferimento estratto. Usa <strong>Metadati</strong> (in alto) per recuperarli da Crossref tramite il DOI.</p>{/if}
          </div>
        {/if}
        <div class="modactions"><button class="ghost" onclick={() => (citModal = false)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if healthModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) healthModal = false; }} role="presentation">
      <div class="idmodal hfwide" role="dialog" tabindex="-1">
        <h2>Salute della libreria</h2>
        {#if healthLoading}
          <p class="dimtext">Analisi in corso…</p>
        {:else if health}
          {@const cats = [
            { label: "File mancanti sul disco", rows: health.missing_file, hint: "Il PDF non è più al percorso salvato." },
            { label: "PDF senza testo estratto", rows: health.no_text, hint: "Probabili scansioni (immagine): non cercabili né indicizzabili." },
            { label: "Metadati incompleti", rows: health.no_metadata, hint: "Manca titolo, anno o autori." },
            { label: "Senza incorporamento semantico", rows: health.no_embedding, hint: "Esclusi dalla ricerca semantica e da «Correlati»." },
            { label: "Senza copertina", rows: health.no_thumbnail, hint: "Nessuna anteprima generata." },
          ]}
          <p class="dimtext">{health.total} documenti analizzati.</p>
          {#each cats as cat (cat.label)}
            <div class="hfsec">
              <h3>{cat.label} ({cat.rows.length})</h3>
              {#if cat.rows.length}
                <p class="dimtext">{cat.hint}</p>
                <ul class="hflist">
                  {#each cat.rows.slice(0, 50) as r (r.id)}
                    <li><button class="hflink" onclick={() => openHealthRow(r.id)} title={r.path}>{r.title ?? r.path.split(/[\\/]/).pop()}</button></li>
                  {/each}
                </ul>
                {#if cat.rows.length > 50}<p class="dimtext">…e altri {cat.rows.length - 50}.</p>{/if}
              {:else}<p class="dimtext">Tutto a posto ✓</p>{/if}
            </div>
          {/each}
          <div class="hfsec">
            <h3>Duplicati — stesso file ({health.duplicates.length})</h3>
            {#if health.duplicates.length}
              <ul class="hflist">
                {#each health.duplicates as g (g.file_hash)}
                  <li class="refrow">
                    <span class="reftext">{g.titles.find((t) => t) ?? "(senza titolo)"} — {g.ids.length} copie</span>
                    {#each g.ids as did (did)}<button class="hflink small" onclick={() => openHealthRow(did)}>#{did}</button>{/each}
                  </li>
                {/each}
              </ul>
              <p class="dimtext">Unisci i duplicati dallo strumento <strong>Duplicati</strong>.</p>
            {:else}<p class="dimtext">Nessun duplicato ✓</p>{/if}
          </div>
        {/if}
        <div class="modactions"><button class="ghost" onclick={() => (healthModal = false)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if gapModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) gapModal = false; }} role="presentation">
      <div class="idmodal hfwide" role="dialog" tabindex="-1">
        <h2>Gap di citazioni</h2>
        <p class="dimtext">I DOI che la tua libreria cita di più ma che non possiedi ancora. Si basa sui riferimenti estratti — recupera i <strong>Metadati</strong> dei tuoi paper (Crossref) per arricchirli.</p>
        {#if gapsLoading}
          <p class="dimtext">Calcolo in corso…</p>
        {:else if gaps.length}
          <ul class="hflist reflist">
            {#each gaps as g (g.doi)}
              <li class="refrow">
                <span class="badge2" title="Citato da {g.count} tuoi documenti">×{g.count}</span>
                <span class="reftext">{g.sample ?? g.doi}</span>
                <button class="hflink small" onclick={() => gapSearchOnline(g.doi)} title="Cerca questo DOI online per aggiungerlo">Cerca</button>
                <button class="hflink small" onclick={() => openInBrowser(`https://doi.org/${g.doi}`)} title="Apri il DOI nel browser">DOI ↗</button>
                <button class="hflink small" onclick={() => copyDoi(g.doi)} title="Copia il DOI">Copia</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="dimtext">Nessun gap rilevato. Servono riferimenti con DOI: apri un documento → <strong>Riferimenti e citazioni</strong>, oppure recupera i <strong>Metadati</strong> da Crossref.</p>
        {/if}
        <div class="modactions"><button class="ghost" onclick={() => (gapModal = false)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if hfModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) hfModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="idmodal hfwide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
        <h2>Codice & repository</h2>
        <p class="dimtext" title={hfTitle}>{hfTitle}</p>

        <div class="hfsec ghsec">
          <h3>Repository GitHub {#if ghRepos}({ghRepos.length}){/if}</h3>
          {#if ghRepos === null}
            <p class="dimtext">Cerco su GitHub…</p>
          {:else if !ghRepos.length}
            <p class="dimtext">Nessun repository GitHub trovato nel testo di questo documento.</p>
          {:else}
            <ul class="hflist">
              {#each ghRepos as r (r.full_name)}
                <li class="ghrow">
                  <button class="hflink" onclick={() => openInBrowser(r.url)} title={r.description ?? "Apri su GitHub"}>{r.full_name}</button>
                  <span class="hfmeta">★ {r.stars}{#if r.language} · {r.language}{/if}{#if r.license} · {r.license}{/if}</span>
                  <button class="ghost small readmebtn" onclick={() => openReadme(r)}>README</button>
                </li>
              {/each}
            </ul>
            {#if ghReadmeOf}
              <div class="readmebox">
                <div class="readmehd">README · {ghReadmeOf}</div>
                {#if ghReadmeLoading}
                  <p class="dimtext">Carico il README…</p>
                {:else if ghReadmeError}
                  <p class="dimtext">{ghReadmeError}</p>
                {:else}
                  <div class="readme" use:readmeLinks>{@html ghReadmeHtml}</div>
                {/if}
              </div>
            {/if}
          {/if}
        </div>

        <div class="hfsec">
          <h3>Hugging Face</h3>
          {#if hfLoading}
            <p class="dimtext">Cerco su Hugging Face…</p>
          {:else if !hfData?.arxiv_id}
            <p class="dimtext">Nessun identificatore arXiv per questo documento: non posso collegarlo a modelli/dataset su Hugging Face.</p>
          {:else}
            {#if hfData.paper_url}
              <button class="hflink paper" onclick={() => openInBrowser(hfData!.paper_url!)}>Apri la pagina del paper su Hugging Face ↗</button>
            {/if}
            <div class="hfsub">Modelli ({hfData.models.length})</div>
            {#if hfData.models.length}
              <ul class="hflist">
                {#each hfData.models as m (m.id)}
                  <li><button class="hflink" onclick={() => openInBrowser(m.url)} title="Apri su Hugging Face">{m.id}</button><span class="hfmeta">♥ {m.likes} · ↓ {m.downloads}</span></li>
                {/each}
              </ul>
            {:else}<p class="dimtext">Nessun modello collegato.</p>{/if}
            <div class="hfsub">Dataset ({hfData.datasets.length})</div>
            {#if hfData.datasets.length}
              <ul class="hflist">
                {#each hfData.datasets as m (m.id)}
                  <li><button class="hflink" onclick={() => openInBrowser(m.url)} title="Apri su Hugging Face">{m.id}</button><span class="hfmeta">♥ {m.likes}</span></li>
                {/each}
              </ul>
            {:else}<p class="dimtext">Nessun dataset collegato.</p>{/if}
          {/if}
        </div>
        <div class="modactions"><button class="ghost" onclick={() => (hfModal = false)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if helpModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) helpModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="idmodal helpmodal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
        <h2>Guida a Scriptorium</h2>
        <p class="dimtext">Gestore di PDF e riferimenti, locale e veloce. Tutto resta sul tuo computer; le funzioni di rete e AI sono opzionali e disattivabili.</p>

        <div class="helpsec">
          <h3>Libreria e organizzazione</h3>
          <ul>
            <li><strong>Importa ▾</strong> (in alto): PDF dal disco (anche trascinandoli) o una libreria <strong>Zotero/Mendeley</strong> via <em>.bib</em>; oppure riferimenti da DOI/arXiv/ISBN/PMID con <em>Aggiungi per ID</em> (sidebar).</li>
            <li><strong>Tag</strong> colorati e <strong>Collezioni</strong> (anche “smart”, che si popolano da sole).</li>
            <li><strong>Filtri</strong> rapidi nella sidebar: Tutti, Preferiti, Da leggere, <strong>Con codice (GitHub)</strong>, <strong>Peer-reviewed</strong>.</li>
            <li><strong>Badge</strong> su card/lista/risultati: <em>preprint</em> / <em>peer-reviewed</em> (e se per un preprint esiste la versione pubblicata, link diretto al DOI).</li>
            <li><strong>Ordinamento</strong> combinabile (barra “Ordina”): preferiti, autore, titolo, anno, data — clic per attivare/invertire/togliere, i numeri indicano la priorità.</li>
            <li><strong>Griglia</strong>: le copertine si <strong>ridimensionano</strong> con il cursore <em>− ▭ +</em> nella barra in alto (la dimensione scelta viene ricordata).</li>
            <li><strong>Continua a leggere</strong>: in “Tutti” trovi in alto gli ultimi PDF aperti. Clic su un <strong>autore</strong> → tutti i suoi lavori.</li>
            <li><strong>Duplicati</strong> (unione) e <strong>Cestino</strong> (ripristino/eliminazione definitiva) tra gli Strumenti.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Ricerca</h3>
          <ul>
            <li><strong>Locale</strong>: barra in alto, modalità <em>Testo</em>, <em>Semantica</em> (per significato) o <em>Ibrida</em>. Cerca anche nelle tue <strong>note e annotazioni</strong>.</li>
            <li><strong>Online</strong> (<em>Scopri online</em>): arXiv, OpenAlex, ADS, Semantic Scholar, Europe PMC, CORE, DOAJ, Hugging Face. Filtri anno/autore/solo-OA e, sui risultati, chip <strong>Con codice</strong> / <strong>Peer-reviewed</strong> / <strong>Preprint</strong> (con conteggi) oltre alle colonne ordinabili. I PDF Open Access si scaricano, gli altri si aggiungono come riferimento.</li>
            <li><strong>Ricerche salvate</strong>: dopo una ricerca premi <em>★ Salva</em> → compare nella sidebar; cliccandola la rilancia e marca con <strong>“novità”</strong> i risultati nuovi dall'ultima volta.</li>
            <li><strong>Trova PDF</strong> (menu ⋯): per un riferimento senza file, cerca un PDF Open Access (Unpaywall/arXiv) e lo allega.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Codice & repository</h3>
          <ul>
            <li>I paper che citano un repo mostrano l'icona <strong>GitHub</strong> (card, lista, risultati online): cliccala per aprire il repository.</li>
            <li>Menu ⋯ → <strong>Codice & repository</strong>: anteprima del <strong>README</strong> nell'app, più i modelli/dataset collegati su <strong>Hugging Face</strong>.</li>
            <li>Filtro <strong>“Con codice (GitHub)”</strong> nella sidebar per vedere solo i paper con codice disponibile.</li>
            <li>Menu ⋯ → <strong>Riferimenti e citazioni</strong>: la bibliografia del paper (con i riferimenti già nella tua libreria cliccabili) e i documenti che lo <strong>citano</strong>. I riferimenti arrivano dall'arricchimento <em>Metadati</em>.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Lettura (visualizzatore PDF)</h3>
          <ul>
            <li><strong>Evidenzia</strong> selezionando il testo, poi aggiungi una nota; oppure modalità <strong>Nota</strong> per appunti “a spillo” in un punto qualsiasi.</li>
            <li><strong>Cerca nel documento</strong>, <strong>indice</strong>, zoom/adatta, rotazione, due pagine, modalità notte; riprende dall'<strong>ultima pagina</strong> letta.</li>
            <li><strong>Estrai tabella</strong> (icona griglia): trascina un rettangolo su una tabella → anteprima → esporta in CSV / Markdown / Excel (+ “migliora con AI”).</li>
            <li><strong>Estrai testo</strong> (icona testo): trascina un'area → copia il testo o salvalo in .txt/.md.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>F</kbd></td><td>Cerca nel documento</td></tr>
              <tr><td><kbd>+</kbd> / <kbd>−</kbd> / <kbd>0</kbd></td><td>Ingrandisci / riduci / zoom 100%</td></tr>
              <tr><td><kbd>W</kbd> / <kbd>H</kbd></td><td>Adatta alla larghezza / alla pagina</td></tr>
              <tr><td><kbd>2</kbd></td><td>Vista a due pagine</td></tr>
              <tr><td><kbd>N</kbd></td><td>Aggiungi una nota</td></tr>
              <tr><td><kbd>I</kbd></td><td>Modalità notte (inverti colori)</td></tr>
              <tr><td><kbd>[</kbd> / <kbd>]</kbd></td><td>Ruota a sinistra / destra</td></tr>
              <tr><td><kbd>Ctrl</kbd>+rotella</td><td>Zoom continuo</td></tr>
              <tr><td><kbd>Esc</kbd></td><td>Chiudi / annulla</td></tr>
              <tr><td><kbd>?</kbd></td><td>Scorciatoie (dentro il lettore)</td></tr>
            </tbody>
          </table>
        </div>

        <div class="helpsec">
          <h3>Funzioni opzionali</h3>
          <ul>
            <li><strong>Esporta ▾</strong> (in alto): <em>Citazioni</em> (BibTeX / RIS / CSL) dei documenti mostrati, oppure <em>In Obsidian</em> (note Markdown nel tuo vault).</li>
            <li><strong>AI locale</strong> (Ollama / LM Studio): riassunti e tag automatici — opzionali, mai automatici, disattivabili. Impostazioni → AI locale.</li>
            <li><strong>Terminale</strong> integrato (es. per <code>claude code</code>), <strong>Backup</strong> completo, e <strong>temi</strong> dell'interfaccia (in alto a destra).</li>
            <li>Suggerimento: le finestre si <strong>ridimensionano</strong> trascinando l'angolo in basso a destra.</li>
          </ul>
        </div>

        <div class="modactions"><button class="primary" onclick={() => (helpModal = false)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if settingsModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) settingsModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div class="setmodal" onclick={(e) => e.stopPropagation()}>
        <h2>Impostazioni</h2>
        <div class="setbody">
          <nav class="setnav">
            <button class="setnavitem" class:active={settingsTab === "online"} onclick={() => (settingsTab = "online")}>Ricerca online</button>
            <button class="setnavitem" class:active={settingsTab === "ai"} onclick={() => (settingsTab = "ai")}>AI locale</button>
            <button class="setnavitem" class:active={settingsTab === "obsidian"} onclick={() => (settingsTab = "obsidian")}>Obsidian</button>
            <button class="setnavitem" class:active={settingsTab === "backup"} onclick={() => (settingsTab = "backup")}>Backup</button>
          </nav>
          <div class="setpane">
            {#if settingsTab === "online"}
              <p class="dimtext">La ricerca online è una funzione di rete: finché è disattivata, l'app resta 100% offline. I PDF vengono scaricati solo per i lavori Open Access.</p>
              <label class="setrow"><input type="checkbox" bind:checked={discEnabled} /> Abilita ricerca online (arXiv + OpenAlex + ADS)</label>
              <label class="setlbl">
                Email di contatto — opzionale ma consigliata
                <input bind:value={discEmail} placeholder="tua@email.it" />
                <span class="sethint">Inviata agli archivi (Crossref, OpenAlex, Unpaywall) per identificarti gentilmente: dà limiti di richiesta più alti e meno blocchi. È <strong>richiesta</strong> per “Trova PDF” (Unpaywall). Non viene usata per altro.</span>
              </label>
              <p class="sethint" style="margin: 4px 0 -2px;">🔒 Le chiavi sono salvate nel <strong>Credential Manager di Windows</strong> (cifrate), non nel database. Dopo il salvataggio non sono più visibili: puoi solo sostituirle o rimuoverle.</p>
              {@render secretField("openalex_key", "Chiave API OpenAlex", "Gratuita su openalex.org/settings/api (opzionale: il free tier funziona anche senza).")}
              {@render secretField("ads_token", "Token API ADS", "Gratuito su ui.adsabs.harvard.edu (Account → API Token). Richiesto per la fonte ADS.")}
              {@render secretField("s2_key", "Chiave API Semantic Scholar", "Opzionale (alza i limiti), su semanticscholar.org/product/api.")}
              {@render secretField("core_key", "Chiave API CORE", "Gratuita su core.ac.uk/services/api. Richiesta per la fonte CORE.")}
              {@render secretField("github_token", "Token GitHub", "Opzionale (alza il limite di richieste per README/repo), su github.com/settings/tokens.")}
            {:else if settingsTab === "ai"}
              <p class="dimtext">
                Locali e disattivate di default. Richiedono <strong>Ollama</strong> oppure <strong>LM Studio</strong> installato, con almeno un modello caricato.
                Puoi <em>Avviare</em>/<em>Fermare</em> il server direttamente da qui (richiede Ollama nel PATH, o la CLI <code>lms</code> di LM Studio), premere <em>Verifica</em> per vedere i modelli, poi scegliere quale usare nel menu in fondo. Quando un provider è raggiungibile compare l'indicatore <strong>AI</strong> in alto.
              </p>
              <label class="setrow"><input type="checkbox" bind:checked={aiEnabled} /> Abilita le funzioni AI</label>

              <div class="setlbl">
                Ollama — URL del server
                <div class="airow">
                  <input bind:value={ollamaUrl} placeholder="http://localhost:11434" />
                  <button class="ghost small" onclick={() => verifyProvider("ollama")} disabled={verifyingOllama}>{verifyingOllama ? "…" : "Verifica"}</button>
                  <button class="ghost small" onclick={() => startServer("ollama")} title="Avvia il server Ollama (ollama serve)">Avvia</button>
                  <button class="ghost small" onclick={() => stopServer("ollama")} title="Ferma il server Ollama">Ferma</button>
                </div>
                {#if ollamaModels}
                  <span class="aifeedback ok">{ollamaModels.length ? `✓ raggiungibile — ${ollamaModels.length} ${ollamaModels.length === 1 ? "modello" : "modelli"}` : "✓ raggiungibile, ma nessun modello (scaricane uno: ollama pull …)"}</span>
                {:else if ollamaErr}
                  <span class="aifeedback bad">✗ non raggiungibile — avvia Ollama</span>
                {/if}
              </div>

              <div class="setlbl">
                LM Studio — URL del server
                <div class="airow">
                  <input bind:value={lmstudioUrl} placeholder="http://localhost:1234" />
                  <button class="ghost small" onclick={() => verifyProvider("lmstudio")} disabled={verifyingLm}>{verifyingLm ? "…" : "Verifica"}</button>
                  <button class="ghost small" onclick={() => startServer("lmstudio")} title="Avvia il server di LM Studio (lms server start)">Avvia</button>
                  <button class="ghost small" onclick={() => stopServer("lmstudio")} title="Ferma il server di LM Studio (lms server stop)">Ferma</button>
                </div>
                {#if lmstudioModels}
                  <span class="aifeedback ok">{lmstudioModels.length ? `✓ raggiungibile — ${lmstudioModels.length} ${lmstudioModels.length === 1 ? "modello" : "modelli"}` : "✓ raggiungibile, ma nessun modello caricato in LM Studio"}</span>
                {:else if lmstudioErr}
                  <span class="aifeedback bad">✗ non raggiungibile — avvia il server di LM Studio</span>
                {/if}
              </div>

              <label class="setlbl">
                Modello da usare (scelto tra quelli trovati)
                <select value={`${aiProvider}::${aiModel}`} onchange={(e) => chooseModel(e.currentTarget.value)}>
                  {#if aiModel && !(aiProvider === "lmstudio" ? (lmstudioModels ?? []) : (ollamaModels ?? [])).includes(aiModel)}
                    <option value={`${aiProvider}::${aiModel}`}>{aiModel} — attuale ({aiProvider === "lmstudio" ? "LM Studio" : "Ollama"})</option>
                  {/if}
                  {#if ollamaModels?.length}
                    <optgroup label="Ollama">
                      {#each ollamaModels as m (m)}<option value={`ollama::${m}`}>{m}</option>{/each}
                    </optgroup>
                  {/if}
                  {#if lmstudioModels?.length}
                    <optgroup label="LM Studio">
                      {#each lmstudioModels as m (m)}<option value={`lmstudio::${m}`}>{m}</option>{/each}
                    </optgroup>
                  {/if}
                  {#if !ollamaModels?.length && !lmstudioModels?.length}
                    <option value="" disabled>Avvia un provider e premi «Verifica» per vedere i modelli</option>
                  {/if}
                </select>
              </label>
              <label class="setrow"><input type="checkbox" bind:checked={aiEmbedGpu} /> Indicizzazione su GPU (via Ollama)</label>
              <p class="sethint" style="margin-top: -8px;">
                Calcola gli embeddings dell'indice con la GPU tramite Ollama (modello <code>bge-m3</code>, 1024-dim, compatibile con l'indice esistente) invece del modello CPU integrato. Richiede Ollama avviato e <code>ollama pull bge-m3</code>.
                Su 6 GB di VRAM condivide la memoria con l'LLM: conviene indicizzare con l'LLM scarico. Se cambi metodo, <strong>Ricostruisci</strong> l'indice in «Chiedi alla libreria» per coerenza.
              </p>
              <label class="setlbl">
                Dimensione batch embeddings — 0 = automatico (64 su GPU, 16 su CPU)
                <input type="number" min="0" max="512" bind:value={aiEmbedBatch} placeholder="0 (auto)" />
                <span class="sethint">Su GPU potenti (es. RTX 4090/5090) alza a <strong>128–256</strong> per saturare la GPU e velocizzare l'indicizzazione. Su CPU lascia basso (8–16).</span>
              </label>
            {:else if settingsTab === "obsidian"}
              <p class="dimtext">
                Esporta i tuoi documenti come note <strong>Markdown</strong> in un vault <strong>Obsidian</strong> (funziona anche con Logseq, Zettlr, Foam…).
                Ogni documento diventa una nota <code>.md</code> in <code>&lt;vault&gt;/Scriptorium/</code> con metadati, abstract, note, annotazioni e tag/autori come <code>[[wikilink]]</code> per il grafo.
                L'esportazione è a senso unico (Scriptorium → vault) e sovrascrive le note esistenti con lo stesso titolo.
              </p>
              <label class="setlbl">
                Cartella del vault
                <div class="airow">
                  <input bind:value={obsidianVault} placeholder="(nessuna cartella scelta)" readonly />
                  <button class="ghost small" onclick={pickObsidianVault}>Scegli…</button>
                </div>
              </label>
              <p class="dimtext">Per esportare usa il pulsante <strong>→ Obsidian</strong> in alto: invia i documenti mostrati (o quelli selezionati).</p>
            {:else}
              <p class="dimtext">Salva una copia completa (database + PDF + miniature) in una cartella a tua scelta.</p>
              <button class="ghost" onclick={doBackup}>Scegli cartella e salva backup…</button>
            {/if}
          </div>
        </div>
        <div class="modactions">
          <button class="ghost" onclick={() => (settingsModal = false)}>Annulla</button>
          <button class="primary" onclick={saveSettings}>Salva</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(body) {
    margin: 0;
    --bg: #f6f2e9;
    --surface: #fffdf8;
    --panel: #efe9dc;
    --field: #fffdf8;
    --text: #2c2e35;
    --dim: #63666e;
    --faint: #8c8f97;
    --border: #e2dccd;
    --border-soft: #ebe5d7;
    --accent: #2b4a78;
    --accent-strong: #1f3a63;
    --accent-soft: #e7edf6;
    --accent-soft2: #d6e0ef;
    --hover: #efe9da;
    --danger: #b0322a;
    --danger-soft: #f2ddda;
    --on-accent: #fff;
    --zebra: #faf6ec;
    --thumb-bg: #ece6d7;
    --thumb-fg: #c2b9a3;
    --viewer-bg: #e8e2d5;
    --serif: "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, "Times New Roman", serif;
    --sans: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    /* shared design tokens (consistent radii / depth / focus ring across all themes) */
    --r-sm: 8px;
    --r-md: 11px;
    --r-lg: 16px;
    --r-pill: 999px;
    --shadow-sm: 0 1px 2px rgba(20, 22, 28, 0.05), 0 1px 3px rgba(20, 22, 28, 0.06);
    --shadow-md: 0 4px 16px rgba(20, 22, 28, 0.09);
    --shadow-lg: 0 22px 60px rgba(20, 22, 28, 0.20);
    --ring: rgba(43, 74, 120, 0.22);
    --ease: 0.18s cubic-bezier(0.4, 0, 0.2, 1);
    background: var(--bg);
    color: var(--text);
    font-family: var(--sans);
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
  }
  /* a calm, modern focus ring on every interactive control */
  :global(input:focus-visible),
  :global(select:focus-visible),
  :global(textarea:focus-visible),
  :global(button:focus-visible) {
    outline: none;
    box-shadow: 0 0 0 3px var(--ring);
  }
  :global(body[data-theme="dark"]) {
    --bg: #14171d; --surface: #1b1f27; --panel: #161a21; --field: #11141a;
    --text: #e6e8ec; --dim: #9aa1ad; --faint: #6b7280;
    --border: #2a2f3a; --border-soft: #21262f;
    --accent: #6f9bf0; --accent-strong: #8fb4f5; --accent-soft: #1e2a40; --accent-soft2: #294063;
    --on-accent: #0f1722; --hover: #20252e;
    --danger: #ff8585; --danger-soft: #3a1f1f;
    --zebra: #181c23; --thumb-bg: #0e1116; --thumb-fg: #3a4150; --viewer-bg: #0c0e12;
    --ring: rgba(111, 155, 240, 0.32); --shadow-lg: 0 22px 60px rgba(0, 0, 0, 0.5);
  }
  :global(body[data-theme="synthwave"]) {
    --bg: #1a1033; --surface: #241546; --panel: #1f1140; --field: #160c2e;
    --text: #f3e8ff; --dim: #b69fd9; --faint: #8a72b5;
    --border: #3a2566; --border-soft: #2c1c4f;
    --accent: #ff5fc4; --accent-strong: #ff86d4; --accent-soft: #34194f; --accent-soft2: #4a2570;
    --on-accent: #1a0a26; --hover: #2a1850;
    --danger: #ff6b8b; --danger-soft: #3a1530;
    --zebra: #1f1240; --thumb-bg: #160c2e; --thumb-fg: #6a4f9c; --viewer-bg: #120a26;
    --ring: rgba(255, 95, 196, 0.32); --shadow-lg: 0 22px 60px rgba(0, 0, 0, 0.55);
  }
  :global(body[data-theme="pastel"]) {
    --bg: #f7f5fb; --surface: #ffffff; --panel: #f0ecf8; --field: #ffffff;
    --text: #4a4458; --dim: #7c768e; --faint: #a7a1b8;
    --border: #e6e0f0; --border-soft: #efeaf7;
    --accent: #8e7cc3; --accent-strong: #7a66b5; --accent-soft: #efeafa; --accent-soft2: #e1d7f3;
    --on-accent: #fff; --hover: #f1edf9;
    --danger: #cf7a89; --danger-soft: #f6e4e8;
    --zebra: #faf8fd; --thumb-bg: #ece9f4; --thumb-fg: #c5bdda; --viewer-bg: #ece8f4;
    --ring: rgba(142, 124, 195, 0.30);
  }
  :global(body[data-theme="medieval"]) {
    --bg: #ece0c2; --surface: #f5ecd4; --panel: #e2d2a9; --field: #f5ecd4;
    --text: #3a2c18; --dim: #6b5836; --faint: #94815c;
    --border: #cbb585; --border-soft: #d9c699;
    --accent: #7c2118; --accent-strong: #611812; --accent-soft: #e6d2a8; --accent-soft2: #d8bf8a;
    --on-accent: #f5ecd4; --hover: #e4d3a6;
    --danger: #8a2418; --danger-soft: #e7cba0;
    --zebra: #e7dab8; --thumb-bg: #ddc99c; --thumb-fg: #b09a6c; --viewer-bg: #ddc99c;
    --ring: rgba(124, 33, 24, 0.26);
  }
  /* ===== New palettes ===== */
  :global(body[data-theme="sepia"]) {
    --bg: #f4ecdf; --surface: #fbf5ea; --panel: #ece0cc; --field: #fbf5ea;
    --text: #3b3024; --dim: #6f5f4a; --faint: #9a8a72;
    --border: #ddcbb0; --border-soft: #e7d9c2;
    --accent: #9a5b24; --accent-strong: #7e4818; --accent-soft: #f0e2cc; --accent-soft2: #e3cda8;
    --on-accent: #fbf5ea; --hover: #ece0cc;
    --danger: #a3402c; --danger-soft: #ecd6c8;
    --zebra: #f0e7d6; --thumb-bg: #e7d9c0; --thumb-fg: #c2ad8a; --viewer-bg: #e7dcc8;
    --ring: rgba(154, 91, 36, 0.26);
  }
  :global(body[data-theme="solarized"]) {
    --bg: #fdf6e3; --surface: #fffbf0; --panel: #eee8d5; --field: #fffbf0;
    --text: #3a4d52; --dim: #657b83; --faint: #93a1a1;
    --border: #e3dcc4; --border-soft: #ece5d0;
    --accent: #268bd2; --accent-strong: #1a6fae; --accent-soft: #e3eef2; --accent-soft2: #cfe2ec;
    --on-accent: #fffbf0; --hover: #eee8d5;
    --danger: #dc322f; --danger-soft: #f4dcd6;
    --zebra: #f7f0dd; --thumb-bg: #ece5d0; --thumb-fg: #c7bfa3; --viewer-bg: #ece5d0;
    --ring: rgba(38, 139, 210, 0.26);
  }
  :global(body[data-theme="sage"]) {
    --bg: #eef2ec; --surface: #f8fbf6; --panel: #e2e9dd; --field: #f8fbf6;
    --text: #2f3a30; --dim: #5d6b5c; --faint: #8a9789;
    --border: #d3ddcd; --border-soft: #e0e8da;
    --accent: #4f7a52; --accent-strong: #3d6440; --accent-soft: #e3eede; --accent-soft2: #cfe0c9;
    --on-accent: #f8fbf6; --hover: #e2e9dd;
    --danger: #b0463a; --danger-soft: #ecdcd6;
    --zebra: #f2f5ef; --thumb-bg: #e0e8da; --thumb-fg: #b4c3ad; --viewer-bg: #e2e9dd;
    --ring: rgba(79, 122, 82, 0.26);
  }
  :global(body[data-theme="nord"]) {
    --bg: #2e3440; --surface: #3b4252; --panel: #353c4a; --field: #2b303b;
    --text: #eceff4; --dim: #b4bcca; --faint: #7b8494;
    --border: #434c5e; --border-soft: #3a4252;
    --accent: #88c0d0; --accent-strong: #a3d0dd; --accent-soft: #3a4654; --accent-soft2: #4a5a6a;
    --on-accent: #2e3440; --hover: #3f4859;
    --danger: #d8868f; --danger-soft: #43333a;
    --zebra: #353c4a; --thumb-bg: #2b303b; --thumb-fg: #4c566a; --viewer-bg: #272c36;
    --ring: rgba(136, 192, 208, 0.34); --shadow-lg: 0 22px 60px rgba(0, 0, 0, 0.5);
  }
  :global(body[data-theme="graphite"]) {
    --bg: #1c1c1f; --surface: #252528; --panel: #202023; --field: #19191c;
    --text: #e8e8ea; --dim: #a0a0a6; --faint: #6e6e75;
    --border: #34343a; --border-soft: #2a2a2f;
    --accent: #c2a878; --accent-strong: #d4bd92; --accent-soft: #2e2a22; --accent-soft2: #433d30;
    --on-accent: #1c1c1f; --hover: #2b2b2f;
    --danger: #e0908a; --danger-soft: #3a2422;
    --zebra: #212124; --thumb-bg: #161618; --thumb-fg: #3c3c42; --viewer-bg: #141416;
    --ring: rgba(194, 168, 120, 0.32); --shadow-lg: 0 22px 60px rgba(0, 0, 0, 0.55);
  }
  :global(body[data-theme="forest"]) {
    --bg: #12201a; --surface: #1a2c23; --panel: #16271f; --field: #0f1c16;
    --text: #e3efe7; --dim: #9bb6a6; --faint: #6c8676;
    --border: #284339; --border-soft: #1f352c;
    --accent: #6fc28d; --accent-strong: #8fd2a6; --accent-soft: #1c3328; --accent-soft2: #28493a;
    --on-accent: #0f1c16; --hover: #213a2e;
    --danger: #e59389; --danger-soft: #36241f;
    --zebra: #16271f; --thumb-bg: #0f1c16; --thumb-fg: #345644; --viewer-bg: #0d1813;
    --ring: rgba(111, 194, 141, 0.32); --shadow-lg: 0 22px 60px rgba(0, 0, 0, 0.5);
  }
  .app { min-height: 100vh; display: flex; flex-direction: column; position: relative; }
  header {
    display: flex; align-items: center; gap: 16px;
    padding: 14px 22px; background: var(--surface); border-bottom: 1px solid var(--border);
  }
  .brand { display: flex; align-items: center; gap: 10px; }
  h1 { font-size: 19px; margin: 0; font-weight: 600; font-family: var(--serif); letter-spacing: 0.2px; }
  .count { background: var(--accent-soft); color: var(--accent); border-radius: 20px; padding: 1px 9px; font-size: 12px; font-weight: 600; }
  .aichip {
    display: inline-flex; align-items: center; gap: 6px;
    background: var(--field); color: var(--faint);
    border: 1px solid var(--border); border-radius: 20px;
    padding: 2px 10px; font-size: 11px; font-weight: 700; letter-spacing: 0.5px;
    cursor: pointer;
  }
  .aichip:hover { border-color: var(--accent); }
  .aichip .aidot { width: 7px; height: 7px; border-radius: 50%; background: var(--faint); }
  .aichip.active { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); }
  .aichip.active .aidot { background: #1f9d57; box-shadow: 0 0 0 2px rgba(31, 157, 87, 0.18); }
  .aichip.warn { color: #b5821a; border-color: #e3c485; }
  .aichip.warn .aidot { background: #e0a93b; box-shadow: 0 0 0 2px rgba(224, 169, 59, 0.18); }
  .search {
    flex: 1; max-width: 520px; background: var(--field); border: 1px solid var(--border);
    color: var(--text); border-radius: 8px; padding: 9px 12px; font-size: 14px; outline: none;
  }
  .search:focus { border-color: var(--accent); }
  .searchgroup { flex: 1; max-width: 560px; display: flex; align-items: stretch; }
  .searchgroup .search { flex: 1; max-width: none; border-top-right-radius: 0; border-bottom-right-radius: 0; border-right: none; }
  .searchmode {
    background: var(--panel); color: var(--text); border: 1px solid var(--border); border-left: none;
    border-top-right-radius: 8px; border-bottom-right-radius: 8px;
    padding: 0 8px; font-size: 13px; cursor: pointer; outline: none;
  }
  .searchmode:focus { border-color: var(--accent); }
  .searchspin { align-self: center; margin-left: 8px; font-size: 11px; color: var(--faint); white-space: nowrap; }
  .themesel {
    background: var(--field); color: var(--text); border: 1px solid var(--border);
    border-radius: 7px; padding: 4px 8px; font-size: 12px; cursor: pointer; outline: none;
  }
  .themesel:focus { border-color: var(--accent); }
  button.primary {
    margin-left: auto; background: var(--accent); color: var(--on-accent); border: none;
    border-radius: var(--r-sm); padding: 9px 16px; font-size: 14px; font-weight: 600; cursor: pointer;
    box-shadow: var(--shadow-sm); transition: background var(--ease), box-shadow var(--ease), transform var(--ease);
  }
  button.primary:hover:not(:disabled) { background: var(--accent-strong); box-shadow: var(--shadow-md); }
  button.primary:active:not(:disabled) { transform: translateY(1px); }
  button.primary:disabled { opacity: 0.55; cursor: default; }
  button.ghost {
    background: transparent; color: var(--accent); border: 1px solid var(--border);
    border-radius: var(--r-sm); padding: 9px 14px; font-size: 14px; font-weight: 600; cursor: pointer;
    transition: background var(--ease), border-color var(--ease);
  }
  button.ghost.small { padding: 5px 10px; font-size: 12px; }
  button.ghost:hover:not(:disabled) { border-color: var(--accent); background: var(--accent-soft); }
  button.ghost:disabled { opacity: 0.5; cursor: default; }
  button.menuopen { border-color: var(--accent); background: var(--accent-soft); }
  /* Count of documents still missing metadata, on the "Metadati" button. */
  button.ghost.attn { border-color: var(--accent-soft2); background: var(--accent-soft); }
  .metabadge {
    display: inline-block; min-width: 16px; margin-left: 6px; padding: 0 5px;
    border-radius: 9px; background: var(--accent); color: var(--on-accent);
    font-size: 10.5px; font-weight: 700; line-height: 16px; text-align: center; vertical-align: 1px;
  }
  .toolbar {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 8px 22px; background: var(--panel); border-bottom: 1px solid var(--border);
  }
  .modes { display: flex; align-items: center; gap: 6px; }
  button.mode {
    background: transparent; color: var(--dim); border: 1px solid transparent;
    border-radius: 7px; padding: 5px 12px; font-size: 13px; cursor: pointer;
  }
  button.mode.active { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); }
  /* grid thumbnail zoom (− slider +) */
  .gridzoom { display: flex; align-items: center; gap: 4px; margin-left: 4px; padding-left: 8px; border-left: 1px solid var(--border); }
  .zbtn {
    width: 24px; height: 24px; border-radius: 6px; border: 1px solid var(--border);
    background: transparent; color: var(--dim); cursor: pointer; font-size: 15px; line-height: 1;
    display: flex; align-items: center; justify-content: center;
  }
  .zbtn:hover { border-color: var(--accent-soft2); color: var(--accent); }
  .zrange { width: 96px; accent-color: var(--accent); cursor: pointer; }
  .index { display: flex; align-items: center; gap: 10px; }
  .hint { font-size: 12px; color: var(--faint); white-space: nowrap; }
  .bar { width: 140px; height: 6px; background: var(--border); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 0.2s; }
  .status { padding: 8px 22px; color: var(--dim); font-size: 13px; border-bottom: 1px solid var(--border-soft); }
  /* multi-criteria sort bar (grid + list) */
  .sortbar {
    display: flex; align-items: center; flex-wrap: wrap; gap: 6px;
    padding: 7px 22px; background: var(--bg); border-bottom: 1px solid var(--border-soft);
  }
  .sortlabel { font-size: 12px; color: var(--dim); margin-right: 2px; cursor: help; }
  .sortchip {
    display: inline-flex; align-items: center; gap: 4px;
    background: transparent; color: var(--dim); border: 1px solid var(--border);
    border-radius: 999px; padding: 3px 11px; font-size: 12.5px; cursor: pointer; transition: background 0.12s, color 0.12s;
  }
  .sortchip:hover { color: var(--accent); border-color: var(--accent-soft2); }
  .sortchip.on { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); font-weight: 600; }
  .sortchip .sar { font-size: 9px; }
  .sortchip .srank {
    font-size: 9px; font-weight: 700; background: var(--accent); color: #fff;
    border-radius: 50%; width: 14px; height: 14px; display: inline-flex; align-items: center; justify-content: center;
  }
  .sortclear { background: transparent; border: none; color: var(--faint); font-size: 12px; cursor: pointer; padding: 3px 6px; text-decoration: underline; }
  .sortclear:hover { color: var(--danger); }
  .list th .ar.rnk {
    font-size: 8px; font-weight: 700; background: var(--accent); color: #fff;
    border-radius: 50%; padding: 0 3px; margin-left: 2px;
  }
  .batchbar {
    display: flex; align-items: center; gap: 12px;
    padding: 8px 22px; background: var(--accent-soft);
    border-bottom: 1px solid var(--accent-soft2); font-size: 13px; color: var(--accent);
  }

  .body { flex: 1; display: flex; min-height: 0; }
  .sidebar {
    width: 222px; flex: 0 0 222px; background: var(--panel); border-right: 1px solid var(--border);
    padding: 12px 10px; overflow: auto;
  }
  .sec { font-size: 11px; text-transform: uppercase; letter-spacing: 0.6px; color: var(--faint); font-weight: 600; margin: 16px 6px 6px; }
  .navrow { display: flex; align-items: center; }
  .navitem {
    flex: 1; text-align: left; background: transparent; border: none; color: var(--text);
    border-radius: 7px; padding: 7px 9px; font-size: 13px; cursor: pointer;
    display: flex; align-items: center; gap: 8px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
  }
  .navitem:hover { background: var(--hover); }
  .navitem.active { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .navcheck { margin-left: auto; color: var(--accent); font-size: 12px; }
  /* Whole-library count on a sidebar filter, right-aligned and muted. */
  .navcount { margin-left: auto; color: var(--faint); font-size: 11px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .navitem.active .navcount { color: var(--accent); }
  .secaction { background: none; border: none; color: var(--accent); cursor: pointer; font-size: 10px; text-transform: none; letter-spacing: 0; text-decoration: underline; }
  .tagsec { display: flex; align-items: center; justify-content: space-between; gap: 6px; }
  .seclabel {
    background: none; border: none; cursor: pointer; padding: 0;
    display: flex; align-items: center; gap: 5px;
    color: var(--faint); font-size: 11px; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.6px;
  }
  .seclabel:hover { color: var(--dim); }
  .chev { display: inline-block; font-size: 9px; transition: transform 0.12s; }
  .chev.open { transform: rotate(90deg); }
  .seccount {
    background: var(--accent-soft); color: var(--accent);
    border-radius: 9px; padding: 0 6px; font-size: 10px; font-weight: 600;
    letter-spacing: 0; text-transform: none;
  }
  .secbtns { display: flex; align-items: center; gap: 4px; }
  /* ~7 tags visible, the rest scroll with the wheel; discreet scrollbar (shows on hover) */
  .taglist {
    max-height: 224px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
  }
  .taglist:hover { scrollbar-color: var(--border) transparent; }
  .taglist::-webkit-scrollbar { width: 8px; }
  .taglist::-webkit-scrollbar-track { background: transparent; }
  .taglist::-webkit-scrollbar-thumb { background: transparent; border-radius: 8px; border: 2px solid transparent; background-clip: padding-box; }
  .taglist:hover::-webkit-scrollbar-thumb { background: var(--border); background-clip: padding-box; }
  .taglist::-webkit-scrollbar-thumb:hover { background: var(--faint); background-clip: padding-box; }
  .dot { width: 9px; height: 9px; border-radius: 50%; flex: 0 0 auto; }
  .x { background: transparent; border: none; color: var(--faint); cursor: pointer; font-size: 15px; padding: 2px 6px; }
  .navrow:hover .x { color: var(--dim); }
  .newcoll { margin: 10px 6px; display: flex; flex-direction: column; gap: 6px; }
  .newcoll input, .newcoll select {
    background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: 6px; padding: 6px 8px; font-size: 12px; outline: none;
  }
  .newcoll .smart { font-size: 12px; color: var(--dim); display: flex; align-items: center; gap: 6px; }
  /* App-level chrome, separated from the working tools above. */
  .appfoot { margin: 18px 0 4px; padding-top: 8px; border-top: 1px solid var(--border); }
  .navitem.foot { color: var(--dim); font-size: 12px; padding: 6px 9px; }
  .navitem.foot:hover { color: var(--text); }
  .watched { display: flex; align-items: center; gap: 4px; margin: 6px; }
  .wpath {
    flex: 1; font-size: 12px; color: var(--dim); overflow: hidden;
    white-space: nowrap; text-overflow: ellipsis;
  }

  .main { flex: 1; overflow: auto; min-width: 0; background: var(--bg); position: relative; }
  .termview { position: absolute; inset: 0; }
  .termview.hidden { display: none; }
  .fbanner {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 10px 20px; background: var(--accent-soft); border-bottom: 1px solid var(--accent-soft2);
    font-size: 13px; color: var(--text); position: sticky; top: 0; z-index: 2;
  }
  .fbanner strong { color: var(--accent); }
  .fbanner button {
    background: var(--surface); border: 1px solid var(--border); color: var(--accent);
    border-radius: 7px; padding: 5px 11px; font-size: 12px; cursor: pointer; white-space: nowrap;
  }
  .fbanner button:hover { border-color: var(--accent); }
  .topbars { position: sticky; top: 0; z-index: 3; }
  .topbars > .fbanner, .topbars > .bulkbar { position: static; }
  .tagbanner { flex-wrap: wrap; }
  .tagfilter { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .tflabel { color: var(--dim); }
  .tfchip { display: inline-flex; align-items: center; gap: 3px; font-size: 11.5px; padding: 1px 4px 1px 8px; border-radius: 11px; border: 1px solid; color: var(--text); }
  .tfx { background: none; border: none; color: var(--faint); cursor: pointer; font-size: 13px; line-height: 1; padding: 0 1px; }
  .tfx:hover { color: var(--danger); }
  .tfmode { display: inline-flex; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; margin-left: 2px; }
  .tfmode button { background: var(--surface); border: none; color: var(--dim); font-size: 11px; padding: 2px 8px; cursor: pointer; }
  .tfmode button:hover { color: var(--accent); }
  .tfmode button.active { background: var(--accent); color: var(--on-accent); }
  .empty { text-align: center; color: var(--faint); padding: 100px 20px; }
  .empty .big { font-size: 20px; color: var(--dim); margin-bottom: 6px; font-family: var(--serif); }
  .grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--grid-min, 180px), 1fr));
    gap: 18px; padding: 22px;
  }
  .card {
    position: relative; background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-md);
    overflow: hidden; transition: border-color var(--ease), box-shadow var(--ease), transform var(--ease); cursor: pointer; text-align: left;
    box-shadow: var(--shadow-sm);
  }
  .card:hover { border-color: var(--accent-soft2); box-shadow: var(--shadow-md); transform: translateY(-3px); }
  .dots {
    position: absolute; top: 6px; right: 6px; z-index: 2;
    width: 26px; height: 26px; border-radius: 6px; border: none;
    background: rgba(255, 253, 248, 0.85); color: var(--accent); cursor: pointer; font-size: 15px; line-height: 1;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12); opacity: 0; transition: opacity 0.12s;
  }
  .card:hover .dots { opacity: 1; }
  .dots:hover { background: var(--accent); color: var(--on-accent); }
  /* grid selection checkbox (left of the ⋯ menu) */
  .cardsel {
    position: absolute; top: 6px; right: 38px; z-index: 2;
    width: 26px; height: 26px; border-radius: 6px;
    border: 1px solid var(--border);
    background: rgba(255, 253, 248, 0.9); color: var(--accent);
    cursor: pointer; font-size: 14px; line-height: 1; font-weight: 700;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
    opacity: 0; transition: opacity 0.12s;
  }
  .card:hover .cardsel { opacity: 1; }
  .cardsel.on { opacity: 1; background: var(--accent); color: var(--on-accent); border-color: var(--accent); }
  .card.selcard { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-soft2); }
  .thumb { aspect-ratio: 3 / 4; background: var(--thumb-bg); display: flex; align-items: center; justify-content: center; overflow: hidden; border-bottom: 1px solid var(--border); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .thumb-placeholder { color: var(--thumb-fg); font-size: 28px; font-weight: 700; font-family: var(--serif); }
  .meta { padding: 11px 13px 14px; }
  .meta h3 {
    font-size: 14px; margin: 0 0 4px; line-height: 1.34; font-family: var(--serif); font-weight: 600; color: var(--text);
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .authors { font-size: 12px; color: var(--dim); margin: 0 0 2px; }
  /* Shown on bare cards/rows that have no author/year/venue until "Metadati" runs. */
  .metamiss { font-size: 11px; color: var(--faint); margin: 2px 0 0; font-style: italic; cursor: help; }
  .metamiss-inline { color: var(--faint); margin-left: 6px; cursor: help; font-size: 11px; }
  /* Persistent citekey: monospace chip on cards, inline badge in the list. Click to copy. */
  .ckey {
    display: inline-block; margin: 2px 0 0; padding: 1px 7px; border-radius: 9px; max-width: 100%;
    background: var(--field); border: 1px solid var(--border); color: var(--dim);
    font-family: ui-monospace, "Cascadia Code", Consolas, monospace; font-size: 10.5px;
    cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; vertical-align: middle;
  }
  .ckey:hover { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
  .ckey-inline {
    display: inline-block; max-width: 22ch; margin-left: 8px; padding: 0 5px;
    border: 1px solid var(--border); border-radius: 7px;
    background: transparent; color: var(--faint); cursor: pointer;
    font-family: ui-monospace, "Cascadia Code", Consolas, monospace; font-size: 10.5px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; vertical-align: -3px;
  }
  .ckey-inline:hover { border-color: var(--accent); color: var(--accent); }
  .authorlink {
    background: none; border: none; padding: 0; margin: 0; font: inherit; color: inherit;
    cursor: pointer; text-align: left; line-height: inherit;
  }
  .authorlink:hover { color: var(--accent); text-decoration: underline; }
  /* GitHub "has code" affordances */
  .ghmark { vertical-align: -2px; }
  .ghchip {
    display: inline-flex; align-items: center; gap: 4px;
    margin-top: 6px; padding: 1px 8px; border-radius: 10px;
    background: var(--accent-soft); color: var(--accent); border: 1px solid var(--accent-soft2);
    font-size: 10.5px; cursor: pointer;
  }
  .ghchip:hover { background: var(--accent); color: var(--on-accent); }
  .ghicon { background: none; border: none; color: var(--dim); cursor: pointer; padding: 0 2px; margin-left: 6px; vertical-align: -2px; }
  .ghicon:hover { color: var(--accent); }
  /* preprint / peer-reviewed status badges (academic, understated) */
  .badgerow { margin-top: 6px; display: flex; gap: 5px; flex-wrap: wrap; }
  .badgeinline { margin-left: 8px; white-space: nowrap; }
  .pubbadge {
    display: inline-flex; align-items: center; gap: 3px;
    font-size: 10px; padding: 1px 7px; border-radius: 4px; border: 1px solid;
    line-height: 1.5; vertical-align: 1px; white-space: nowrap;
  }
  button.pubbadge { cursor: pointer; }
  .pubbadge.pub { color: #1b6b4a; border-color: #b7ddc8; background: #e6f4ec; }
  .pubbadge.pre { color: #8a5a00; border-color: #e8d3a0; background: #faf0d7; }
  button.pubbadge.link:hover { filter: brightness(0.96); text-decoration: underline; }
  .saveddot { background: var(--accent); }
  .nuovo {
    display: inline-flex; align-items: center; vertical-align: 1px;
    font-size: 9.5px; font-weight: 700; letter-spacing: 0.03em;
    padding: 1px 6px; border-radius: 4px; margin-right: 6px;
    background: var(--accent); color: var(--on-accent);
  }
  /* Help modal */
  .helpmodal { width: 700px; max-height: 86vh; overflow-y: auto; }
  .helpsec { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border); }
  .helpsec h3 { font-size: 14px; font-family: var(--serif); margin: 0 0 8px; color: var(--text); }
  .helpsec ul { margin: 0; padding-left: 18px; }
  .helpsec li { font-size: 13px; line-height: 1.55; color: var(--text); margin: 4px 0; }
  .kbdtable { width: 100%; border-collapse: collapse; margin-top: 10px; }
  .kbdtable td { padding: 4px 8px; font-size: 12.5px; border-bottom: 1px solid var(--border-soft); color: var(--dim); }
  .kbdtable td:first-child { white-space: nowrap; width: 1%; }
  .helpmodal kbd {
    background: var(--panel); border: 1px solid var(--border); border-radius: 5px;
    padding: 1px 6px; font-family: ui-monospace, monospace; font-size: 12px; color: var(--text);
  }
  /* "Continue reading" shelf */
  .recentshelf { padding: 16px 22px 4px; }
  .shelfh { font-size: 13px; font-weight: 600; color: var(--dim); margin: 0 0 10px; text-transform: uppercase; letter-spacing: 0.04em; }
  .shelf { display: flex; gap: 14px; overflow-x: auto; padding-bottom: 6px; }
  .shelfcard {
    flex: 0 0 auto; width: 108px; cursor: pointer; text-align: left;
    border-radius: 8px; transition: transform 0.12s;
  }
  .shelfcard:hover { transform: translateY(-2px); }
  .shelfthumb {
    aspect-ratio: 3 / 4; background: var(--thumb-bg); border: 1px solid var(--border);
    border-radius: 7px; overflow: hidden; display: flex; align-items: center; justify-content: center;
  }
  .shelfthumb img { width: 100%; height: 100%; object-fit: cover; }
  .shelftitle {
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    font-size: 11.5px; color: var(--text); margin-top: 5px; line-height: 1.3;
  }
  .venue { font-size: 11.5px; color: var(--faint); margin: 0; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 8px; }
  .chip { font-size: 10.5px; padding: 1px 7px; border-radius: 10px; border: 1px solid; color: var(--text); }
  /* favorite star — grid overlay (mirrors .dots) */
  .starbtn {
    position: absolute; top: 6px; left: 6px; z-index: 2;
    width: 26px; height: 26px; border-radius: 6px; border: none;
    background: rgba(255, 253, 248, 0.85); color: var(--faint);
    cursor: pointer; font-size: 15px; line-height: 1;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
    opacity: 0; transition: opacity 0.12s;
  }
  .card:hover .starbtn { opacity: 1; }
  .starbtn:hover { background: var(--surface); color: #e0a93b; }
  .starbtn.on { opacity: 1; color: #e0a93b; }
  /* favorite star — inline in the list title cell */
  .starinline {
    background: transparent; border: none; cursor: pointer;
    color: var(--faint); font-size: 14px; line-height: 1;
    padding: 0 6px 0 0; vertical-align: middle;
  }
  .starinline:hover, .starinline.on { color: #e0a93b; }


  /* list view */
  .listwrap { padding: 16px 20px; }
  .list {
    width: 100%; border-collapse: separate; border-spacing: 0;
    table-layout: fixed; font-size: 13px;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-md); overflow: hidden;
    box-shadow: var(--shadow-sm);
  }
  col.c-year { width: 64px; }
  col.c-date { width: 96px; }
  col.c-act { width: 78px; }
  col.c-auth { width: 18%; }
  col.c-venue { width: 16%; }
  col.c-tags { width: 13%; }
  col.c-doi { width: 13%; }
  .list thead th {
    position: sticky; top: 0; z-index: 1; background: var(--panel); text-align: left;
    color: var(--dim); font-weight: 600; font-size: 11px; text-transform: uppercase; letter-spacing: 0.4px;
    padding: 11px 12px; border-bottom: 1px solid var(--border); white-space: nowrap;
  }
  .list th.num, .list td.num { text-align: right; }
  .list th .ar { margin-left: 4px; color: var(--accent); font-size: 9px; }
  .list th.sortable { cursor: pointer; user-select: none; }
  .list th.sortable:hover { color: var(--accent); }
  .list tbody tr { cursor: pointer; }
  .list tbody tr:nth-child(even) { background: var(--zebra); }
  .list tbody tr:hover { background: var(--accent-soft); }
  .list td {
    padding: 9px 12px; vertical-align: middle; border-bottom: 1px solid var(--border-soft);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .list td.dim { color: var(--dim); }
  .list td.ttl { color: var(--text); font-weight: 600; font-family: var(--serif); }
  .list td.doi { font-family: ui-monospace, monospace; font-size: 11.5px; }
  .list td.rowact { text-align: right; white-space: nowrap; overflow: visible; }
  .tagcell { display: flex; gap: 5px; align-items: center; overflow: hidden; }
  .tagcell .chip { flex: 0 0 auto; max-width: 96px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more { font-size: 10.5px; color: var(--faint); flex: 0 0 auto; }
  col.c-sel { width: 34px; }
  .list td.sel { text-align: center; overflow: visible; }
  .list tbody tr.selrow { background: var(--accent-soft2) !important; }
  .rowbtn.del:hover { border-color: var(--danger); color: var(--danger); }
  .medit.del { color: var(--danger); }
  .medit.del:hover { background: var(--danger-soft); }

  /* bulk selection bar */
  .bulkbar {
    display: flex; align-items: center; gap: 10px; flex-wrap: wrap;
    padding: 9px 20px; background: var(--accent-soft); border-bottom: 1px solid var(--accent-soft2);
    font-size: 13px; color: var(--accent); position: sticky; top: 0; z-index: 2;
  }
  .bulkbar button, .bulkbar select {
    background: var(--surface); color: var(--accent); border: 1px solid var(--border);
    border-radius: 7px; padding: 5px 11px; font-size: 12px; cursor: pointer; outline: none;
  }
  .bulkbar button:hover { border-color: var(--accent); }
  .bulkbar button.del:hover { border-color: var(--danger); color: var(--danger); }

  /* duplicates view */
  .dupwrap { padding: 20px; display: flex; flex-direction: column; gap: 14px; }
  .dupgroup { background: var(--surface); border: 1px solid var(--border); border-radius: 10px; padding: 12px 16px; }
  .duphead { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; color: var(--dim); font-size: 12px; }
  .duprow { display: flex; align-items: center; gap: 10px; padding: 4px 0; font-size: 13px; }
  .duprow .badge { font-size: 10px; color: var(--faint); width: 56px; flex: 0 0 auto; text-transform: uppercase; }
  .duprow .dt { color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }

  /* add-by-identifier modal */
  .modalback {
    position: fixed; inset: 0; z-index: 80; background: rgba(44, 46, 53, 0.4);
    display: flex; align-items: center; justify-content: center; padding: 24px;
  }
  .idmodal {
    width: 520px; max-width: 100%; background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--r-lg); padding: 22px 24px; box-shadow: var(--shadow-lg);
    /* resizable dialog: drag the bottom-right corner; content scrolls. */
    overflow: auto; resize: both; min-width: 340px; min-height: 200px; max-height: 92vh;
  }
  .idmodal h2 { margin: 0 0 8px; font-size: 18px; font-family: var(--serif); font-weight: 600; }
  .idmodal .dimtext { color: var(--dim); font-size: 13px; margin: 0 0 12px; }
  .idmodal textarea {
    width: 100%; box-sizing: border-box; background: var(--field); border: 1px solid var(--border);
    color: var(--text); border-radius: 7px; padding: 9px; font-size: 13px;
    font-family: ui-monospace, monospace; resize: vertical; outline: none;
  }
  .idmodal textarea:focus { border-color: var(--accent); }
  .modactions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 12px; }
  .modactions .primary { margin-left: 0; }
  /* confirmation dialog (destructive actions) — sits above every other overlay */
  .confirmback { z-index: 95; background: rgba(20, 22, 28, 0.5); }
  .confirmbox {
    width: 400px; max-width: 100%; background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--r-lg); padding: 22px 24px; box-shadow: var(--shadow-lg);
  }
  .confirmmsg { margin: 0 0 18px; font-size: 14.5px; line-height: 1.5; color: var(--text); }
  /* About dialog */
  .aboutbox { width: 380px; min-width: 320px; min-height: 0; text-align: center; resize: none; }
  .abouthead { display: flex; flex-direction: column; align-items: center; gap: 3px; margin-bottom: 10px; }
  .aboutmark {
    width: 54px; height: 54px; border-radius: 14px;
    background: var(--accent); color: var(--on-accent);
    font-family: var(--serif); font-size: 30px; font-weight: 600;
    display: flex; align-items: center; justify-content: center; margin-bottom: 6px;
    box-shadow: var(--shadow-md);
  }
  .aboutbox h2 { margin: 0; }
  .aboutver { margin: 0; font-size: 12.5px; color: var(--dim); letter-spacing: 0.02em; }
  .abouttag { font-size: 13px; color: var(--text); line-height: 1.55; margin: 8px 0 10px; }
  .aboutmeta { font-size: 12px; color: var(--dim); margin: 0 0 14px; }
  .aboutcopy { font-size: 11.5px; color: var(--faint); margin: 0 0 4px; }
  .aboutbox .modactions { justify-content: center; }
  button.primary.danger { background: var(--danger); }
  button.primary.danger:hover:not(:disabled) { background: var(--danger); filter: brightness(0.92); }
  /* Hugging Face modal */
  .hfsec { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border); }
  .hfsec h3 { font-size: 13.5px; margin: 0 0 8px; color: var(--text); font-weight: 600; }
  .hfsub { font-size: 12px; color: var(--dim); font-weight: 600; margin: 10px 0 5px; }
  .hflist { list-style: none; margin: 0; padding: 0; max-height: 170px; overflow-y: auto; }
  .hflist li { display: flex; align-items: center; gap: 8px; padding: 3px 0; }
  .hflink {
    background: none; border: none; padding: 0; color: var(--accent); cursor: pointer;
    font-size: 13px; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .hflink:hover { text-decoration: underline; }
  .hflink.paper { font-size: 13px; margin: 2px 0 4px; }
  .hflink.small { font-size: 11px; flex-shrink: 0; }
  .hfmeta { font-size: 11.5px; color: var(--faint); margin-left: auto; white-space: nowrap; }
  /* references / citations modal */
  .reflist { max-height: 260px; }
  .reflist .refrow { display: flex; align-items: baseline; gap: 8px; padding: 5px 0; border-bottom: 1px solid var(--border-soft); white-space: normal; }
  .reftext { font-size: 12px; color: var(--dim); line-height: 1.45; flex: 1; }
  .badge2.inlibref { color: var(--accent); border-color: var(--accent-soft2); background: var(--accent-soft); flex-shrink: 0; }
  .reflist .hflink { white-space: normal; }
  /* GitHub section + rendered README */
  .hfwide { width: 640px; max-height: 85vh; overflow-y: auto; }
  .ghsec { border-top: none; padding-top: 0; margin-top: 8px; }
  .ghrow { gap: 8px; }
  .readmebtn { white-space: nowrap; flex-shrink: 0; padding: 2px 9px; }
  .readmebox { margin-top: 10px; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
  .readmehd { background: var(--panel); padding: 6px 10px; font-size: 12px; color: var(--dim); }
  .readme { max-height: 340px; overflow-y: auto; padding: 12px 14px; font-size: 13px; line-height: 1.55; color: var(--text); }
  .readme :global(h1), .readme :global(h2) { font-size: 15px; font-family: var(--serif); border-bottom: 1px solid var(--border); padding-bottom: 4px; margin: 12px 0 8px; }
  .readme :global(h3) { font-size: 13.5px; margin: 10px 0 6px; }
  .readme :global(p) { margin: 6px 0; }
  .readme :global(pre) { background: var(--panel); padding: 8px 10px; border-radius: 6px; overflow-x: auto; }
  .readme :global(code) { font-family: ui-monospace, monospace; font-size: 12px; }
  .readme :global(img) { max-width: 100%; height: auto; }
  .readme :global(a) { color: var(--accent); }
  .readme :global(table) { border-collapse: collapse; margin: 8px 0; }
  .readme :global(td), .readme :global(th) { border: 1px solid var(--border); padding: 3px 7px; }
  .readme :global(ul), .readme :global(ol) { padding-left: 20px; }

  /* settings modal — categories on the left, options on the right */
  .setmodal {
    width: 680px; max-width: 100%; height: 480px; max-height: 92vh;
    display: flex; flex-direction: column;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--r-lg); padding: 22px 24px; box-shadow: var(--shadow-lg);
    overflow: auto; resize: both; min-width: 420px; min-height: 300px;
  }
  .setmodal h2 { margin: 0 0 14px; font-size: 18px; font-family: var(--serif); font-weight: 600; }
  .setbody { display: flex; gap: 18px; flex: 1; min-height: 0; }
  .setnav { display: flex; flex-direction: column; gap: 3px; width: 150px; flex-shrink: 0; }
  .setnavitem {
    text-align: left; background: transparent; border: 1px solid transparent; color: var(--dim);
    border-radius: 8px; padding: 8px 11px; font-size: 13.5px; cursor: pointer; transition: background 0.12s, color 0.12s;
  }
  .setnavitem:hover { background: var(--panel); color: var(--text); }
  .setnavitem.active { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); font-weight: 600; }
  .setpane {
    flex: 1; min-width: 0; overflow-y: auto; padding: 2px 10px 2px 2px;
    border-left: 1px solid var(--border); padding-left: 18px;
  }
  .setpane .dimtext { color: var(--dim); font-size: 13px; margin: 0 0 12px; }
  .setpane .setlbl input, .setpane .setlbl select { width: 100%; box-sizing: border-box; }

  /* discovery panel */
  .discbar {
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 12px 20px; border-bottom: 1px solid var(--border); background: var(--panel);
    position: sticky; top: 0; z-index: 2;
  }
  .discbar select, .discbar input {
    background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: 7px; padding: 7px 9px; font-size: 13px; outline: none;
  }
  .discbar select:focus, .discbar input:focus { border-color: var(--accent); }
  .discbar .discq { flex: 1; min-width: 200px; }
  .discbar .discau { width: 130px; }
  .discbar .discy { width: 64px; }
  .discbar .discoa { font-size: 12px; color: var(--dim); display: flex; align-items: center; gap: 5px; white-space: nowrap; }
  .discbar .primary { margin-left: 0; }
  /* discovery result filters (toggle chips) */
  .discfilters { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 10px 16px 0; }
  .dflabel { font-size: 12px; color: var(--faint); }
  .dfchip {
    display: inline-flex; align-items: center; gap: 6px;
    background: transparent; border: 1px solid var(--border); color: var(--dim);
    border-radius: 999px; padding: 4px 11px; font-size: 12.5px; cursor: pointer; white-space: nowrap;
    transition: border-color var(--ease), background var(--ease), color var(--ease);
  }
  .dfchip:hover { border-color: var(--accent-soft2); color: var(--accent); }
  .dfchip.on { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); }
  .dfchip :global(svg) { width: 13px; height: 13px; }
  .dfcount {
    font-size: 11px; font-variant-numeric: tabular-nums; color: var(--faint);
    background: color-mix(in srgb, var(--dim) 14%, transparent); border-radius: 999px; padding: 0 6px; min-width: 16px; text-align: center;
  }
  .dfchip.on .dfcount { color: var(--accent); background: color-mix(in srgb, var(--accent) 16%, transparent); }
  .dfclear { background: transparent; border: none; color: var(--faint); cursor: pointer; font-size: 12px; padding: 4px 6px; }
  .dfclear:hover { color: var(--accent); }
  .dfshown { font-size: 12px; color: var(--faint); margin-left: auto; font-variant-numeric: tabular-nums; }
  .dfempty { padding: 22px 16px; color: var(--dim); font-size: 13px; }
  .linklike { background: none; border: none; color: var(--accent); cursor: pointer; font: inherit; padding: 0; text-decoration: underline; }
  col.c-cit { width: 56px; }
  col.c-act2 { width: 132px; }
  /* discovery: compact add (+) / in-library (✓) at the left of the title */
  .addbtn {
    background: transparent; border: 1px solid var(--border); color: var(--accent);
    border-radius: 6px; padding: 0 7px; font-size: 15px; line-height: 1.5; font-weight: 600;
    cursor: pointer; margin-right: 7px; vertical-align: middle;
  }
  .addbtn:hover:not(:disabled) { background: var(--accent-soft); border-color: var(--accent-soft2); }
  .addbtn:disabled { opacity: 0.55; cursor: default; }
  .addbtn.inlib { color: #1b6b4a; border-color: #b7ddc8; background: #e6f4ec; cursor: default; }
  .badge2 { font-size: 10.5px; padding: 2px 8px; border-radius: 10px; border: 1px solid; white-space: nowrap; }
  .badge2.oa { color: #1b6b4a; border-color: #b7ddc8; background: #e6f4ec; }
  .badge2.meta { color: var(--dim); border-color: var(--border); }
  /* discovery: abstract toggle + expandable row */
  .abstoggle {
    background: transparent; border: none; color: var(--dim); cursor: pointer;
    font-size: 11px; padding: 0 6px 0 0; line-height: 1;
    transition: transform 0.12s ease; display: inline-block;
  }
  .abstoggle:hover { color: var(--accent); }
  .abstoggle.open { transform: rotate(90deg); }
  tr.absrow td { padding: 0 12px 10px; border-top: none; }
  .abswrap {
    font-size: 12.5px; line-height: 1.55; color: var(--dim);
    background: var(--field); border: 1px solid var(--border); border-radius: 7px;
    padding: 9px 12px; max-height: 220px; overflow-y: auto; white-space: pre-wrap;
  }

  /* "Chiedi alla libreria" (RAG) view */
  .askwrap { padding: 24px 28px; max-width: 900px; }
  .askh { font-family: var(--serif); font-size: 22px; margin: 0 0 4px; font-weight: 600; }
  .askintro { font-size: 13px; color: var(--dim); line-height: 1.55; margin: 0 0 12px; max-width: 72ch; }
  .askindex { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 16px; }
  .askstat { font-size: 12.5px; color: var(--dim); }
  .askwarn {
    font-size: 12.5px; color: #8a5a00; background: #faf0d7; border: 1px solid #e8d3a0;
    border-radius: var(--r-sm); padding: 8px 12px; margin: 0 0 14px;
  }
  .askbar { display: flex; gap: 8px; margin-bottom: 14px; }
  .askq {
    flex: 1; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: var(--r-sm); padding: 11px 14px; font-size: 14px; outline: none;
  }
  .askq:focus { border-color: var(--accent); }
  .askbar .primary { margin-left: 0; }
  .askanswer {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-md);
    padding: 16px 18px; font-size: 14px; line-height: 1.7; color: var(--text);
    white-space: pre-wrap; box-shadow: var(--shadow-sm);
  }
  .citechip {
    background: var(--accent-soft); color: var(--accent); border: 1px solid var(--accent-soft2);
    border-radius: 5px; padding: 0 5px; font-size: 11px; font-weight: 700; cursor: pointer;
    vertical-align: 1px; margin: 0 1px;
  }
  .citechip:hover { background: var(--accent); color: var(--on-accent); }
  .asksrc { margin-top: 16px; }
  .asksrc h3 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--faint); margin: 0 0 8px; }
  .srclist { list-style: none; margin: 0; padding: 0; }
  .srclist > li { font-size: 13px; margin: 10px 0; color: var(--text); }
  .passages { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 5px; }
  .passchip {
    font-size: 11px; background: var(--panel); border: 1px solid var(--border);
    border-radius: 6px; padding: 1px 7px; color: var(--dim); cursor: help; white-space: nowrap;
  }
  .passchip:hover { border-color: var(--accent); color: var(--accent); }
  .ppage { color: var(--faint); }
  .srcrel { font-size: 10.5px; color: var(--faint); border: 1px solid var(--border); border-radius: 8px; padding: 0 6px; margin-left: 6px; }
  .askscope {
    display: inline-flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--accent);
    background: var(--accent-soft); border: 1px solid var(--accent-soft2);
    border-radius: var(--r-pill); padding: 3px 6px 3px 12px; margin-bottom: 10px;
  }
  .scopex { background: none; border: none; color: var(--accent); cursor: pointer; font-size: 13px; line-height: 1; padding: 0 2px; }
  .scopex:hover { color: var(--danger); }
  .caret { color: var(--accent); animation: blink 1s steps(2) infinite; }
  @keyframes blink { 50% { opacity: 0; } }

  /* settings modal rows */
  .setrow { display: flex; align-items: center; gap: 8px; font-size: 14px; color: var(--text); margin: 6px 0 14px; }
  .setlbl { display: flex; flex-direction: column; gap: 5px; font-size: 12px; color: var(--dim); margin-bottom: 13px; }
  .setlbl input, .setlbl select { background: var(--field); border: 1px solid var(--border); color: var(--text); border-radius: 7px; padding: 8px 10px; font-size: 14px; outline: none; }
  .sethint { font-size: 11.5px; color: var(--faint); line-height: 1.45; margin-top: 5px; }
  .keyset { color: #1b6b4a; font-size: 13px; font-weight: 600; display: inline-flex; align-items: center; }
  .setlbl input:focus, .setlbl select:focus { border-color: var(--accent); }
  .airow { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .airow input { flex: 1; min-width: 150px; }
  .airow .ghost.small { white-space: nowrap; }
  .aifeedback { font-size: 12px; margin-top: 5px; }
  .aifeedback.ok { color: #1b6b4a; }
  .aifeedback.bad { color: var(--danger); }
  .rowbtn {
    background: transparent; border: 1px solid transparent; color: var(--dim);
    border-radius: 6px; padding: 3px 8px; font-size: 14px; cursor: pointer;
  }
  .rowbtn:hover { border-color: var(--accent); color: var(--accent); }
  .rowbtn.ghost { border-color: var(--border); }
  .medit {
    display: block; width: 100%; text-align: left; background: transparent; border: none;
    color: var(--text); padding: 6px 4px; font-size: 13px; cursor: pointer; border-radius: 6px;
    border-bottom: 1px solid var(--border-soft); margin-bottom: 6px;
  }
  .medit:hover { background: var(--accent-soft); }
  .dropmask {
    position: fixed; inset: 0; background: rgba(43, 74, 120, 0.1); border: 3px dashed var(--accent);
    display: flex; align-items: center; justify-content: center; font-size: 22px; color: var(--accent); z-index: 20; pointer-events: none;
  }
  .menu {
    position: fixed; z-index: 70; width: 220px; max-height: 60vh; overflow: auto;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-md); padding: 8px;
    box-shadow: var(--shadow-lg);
  }
  .mtitle { font-size: 11px; text-transform: uppercase; color: var(--faint); margin: 4px 4px 6px; }
  .mtag { display: flex; align-items: center; gap: 7px; padding: 4px 4px; font-size: 13px; color: var(--text); cursor: pointer; }
  .mnew { display: flex; gap: 6px; margin: 6px 2px; }
  .mnew input {
    flex: 1; min-width: 0; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: 6px; padding: 5px 8px; font-size: 12px; outline: none;
  }
  .mcoll {
    display: block; width: 100%; text-align: left; background: transparent; border: none;
    color: var(--text); padding: 5px 4px; font-size: 13px; cursor: pointer; border-radius: 6px;
  }
  .mcoll:hover { background: var(--accent-soft); }
</style>
