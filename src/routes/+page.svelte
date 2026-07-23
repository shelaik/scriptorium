<script lang="ts">
  import { onMount, untrack, tick } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    importFiles,
    listDocuments,
    getThumbnail,
    rebuildThumbnails,
    recoverMissingMetadata,
    cancelRecoverMetadata,
    type MetaRecoverProgress,
    repairMetadata,
    searchDocuments,
    searchNotes,
    type NoteHit,
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
    ocrDocument,
    libraryFacets,
    type LibraryFacets,
    citationGaps,
    resolveReferenceDois,
    cancelReferenceDois,
    type GapItem,
    listSavedSearches,
    createSavedSearch,
    deleteSavedSearch,
    runSavedSearch,
    setWatchAutoRun,
    type SavedSearch,
    novitaCount,
    listNovita,
    dismissHit,
    dismissWatchHits,
    acceptHit,
    sweepWatchesNow,
    type NovitaGroup,
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
    addFromUrl,
    getConnectorInfo,
    setConnectorEnabled,
    type ConnectorInfo,
    importReferenceManager,
    importLatexZip,
    findPdf,
    hfResources,
    type HfResources,
    githubRepos,
    githubReadme,
    type GhRepo,
    setRead,
    setFavorite,
    backupLibrary,
    inspectBackup,
    stageRestore,
    restartApp,
    getAiSettings,
    setAiSettings,
    aiListModels,
    aiUnloadModels,
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
    exploreCitations,
    writeTextFile,
    saveNoteAsset,
    importNoteAsset,
    type CitationNeighbors,
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
  import {
    similarityGraph,
    saveGraphPositions,
    documentPath,
    removeFromCollection,
    getDocumentMeta,
    attachFromUrl,
    updateTag,
    wikiList,
    wikiGet,
    wikiGenerate,
    wikiDelete,
    wikiCancel,
    listNotes,
    getNote,
    exportNote,
    previewMarkdown,
    noteExportHtml,
    createNote,
    appendToNote,
    saveNote,
    renameNote,
    deleteNote,
    revealNotesDir,
    type NoteMeta,
    type NoteView,
    compareDocuments,
    generateReview,
    harvestResults,
    readingPath,
    exportTable,
    type SimilarityGraph,
    type WikiPageMeta,
    type WikiPage,
    type AiDocResult,
    type PathStep,
    listProjects,
    type ProjectMeta,
    type ExploreSeed,
    type CompanionPaths,
    companionPaths,
    checkUpdate,
    openPlancia,
    pulseLogStatus,
    setPulseLog,
    pulseRevealLogs,
  } from "$lib/api";
  import Viewer from "$lib/viewer/Viewer.svelte";
  import MetaEditor from "$lib/MetaEditor.svelte";
  import MetaFinder from "$lib/MetaFinder.svelte";
  import PdfFinder from "$lib/PdfFinder.svelte";
  import { mathRender } from "$lib/math";
  import { printDocument, printDocuments, printHtml } from "$lib/print";
  import ShareMenu from "$lib/ShareMenu.svelte";
  import { revealDocument, openInBrowser, shareTo, type ShareTarget } from "$lib/share";
  import Terminal from "$lib/Terminal.svelte";
  import RadialMenu from "$lib/RadialMenu.svelte";
  import type { RadialItem } from "$lib/radial";
  import CommandPalette from "$lib/CommandPalette.svelte";
  import type { PaletteEntry } from "$lib/palette";
  import Constellation from "$lib/Constellation.svelte";
  import CitationMap from "$lib/CitationMap.svelte";
  import DetailPanel from "$lib/DetailPanel.svelte";
  import SendToNotePicker from "$lib/SendToNotePicker.svelte";
  import TexProjects from "$lib/TexProjects.svelte";
  import Archivio from "$lib/Archivio.svelte";
  import { refToken, type NotePayload } from "$lib/notecite";

  type Filter = {
    kind: "all" | "collection" | "related" | "trash" | "duplicates" | "discover" | "favorite" | "unread" | "terminal" | "author" | "github" | "peerreviewed" | "ask" | "wiki" | "novita" | "mywork" | "notes" | "projects" | "archivio";
    id?: number;
    label?: string;
  };

  const PALETTE = [
    "#ef4444", "#f59e0b", "#10b981", "#3b82f6",
    "#8b5cf6", "#ec4899", "#14b8a6", "#eab308",
  ];

  let docs = $state<DocumentItem[]>([]);
  // Whole-library counts behind the sidebar filters; refreshed by loadDocs().
  let facets = $state<LibraryFacets>({ all: 0, favorite: 0, unread: 0, github: 0, peerreviewed: 0, own: 0 });
  let recentDocs = $state<DocumentItem[]>([]); // "Continue reading" shelf

  // ----- Coach mark una-tantum: al primo avvio spiega destro + Ctrl+K -----
  let showCoach = $state(false);
  try {
    showCoach = !localStorage.getItem("scriptorium-coach-seen");
  } catch {
    /* localStorage assente: non insistere col suggerimento */
  }
  function dismissCoach() {
    showCoach = false;
    try {
      localStorage.setItem("scriptorium-coach-seen", "1");
    } catch {
      /* ignora */
    }
  }

  // ----- Home leggera (vista «Tutti»): contatori + riscopri del giorno -----
  let homeCollapsed = $state(false);
  let rediscoverTick = $state(0);
  try {
    homeCollapsed = localStorage.getItem("scriptorium-home-collapsed") === "1";
  } catch {
    /* localStorage non disponibile: mostra la home */
  }
  function toggleHome() {
    homeCollapsed = !homeCollapsed;
    try {
      localStorage.setItem("scriptorium-home-collapsed", homeCollapsed ? "1" : "0");
    } catch {
      /* ignora */
    }
  }
  const unreadCount = $derived(docs.filter((d) => d.has_file && !d.is_read).length);
  const readingCount = $derived(docs.filter((d) => d.has_file && !d.is_read && (d.last_page ?? 0) > 1).length);
  const addedThisMonth = $derived.by(() => {
    const now = new Date();
    const y = now.getFullYear();
    const m = now.getMonth();
    return docs.filter((d) => {
      if (!d.added_at) return false;
      const t = new Date(d.added_at);
      return t.getFullYear() === y && t.getMonth() === m;
    }).length;
  });
  // Un paper da riscoprire, stabile nell'arco della giornata (rinfrescabile con ↻):
  // preferisce i non letti, altrimenti pesca da tutta la libreria con PDF.
  const rediscoverPick = $derived.by(() => {
    const unread = docs.filter((d) => d.has_file && !d.is_read);
    const pool = unread.length ? unread : docs.filter((d) => d.has_file);
    if (!pool.length) return null;
    const day = Math.floor(Date.now() / 86400000);
    return pool[(day + rediscoverTick) % pool.length];
  });
  let results = $state<DocumentItem[]>([]);
  let noteResults = $state<NoteHit[]>([]); // standalone-note hits for the current search
  let thumbs = $state<Record<number, string>>({});
  let rebuildingThumbs = $state(false);
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
  // Bulk metadata recovery: live progress from the `meta-progress` event.
  let metaScan = $state<MetaRecoverProgress | null>(null);
  // Which document the "Recupera metadati" candidate finder is open on.
  let metaFindId = $state<number | null>(null);
  // Which reference-only document the "Trova PDF" candidate finder is open on.
  let pdfFindId = $state<number | null>(null);
  let generating = $state(false);
  let searching = $state(false);
  let printing = $state(false);
  let dragOver = $state(false);
  let status = $state("");
  let openDoc = $state<DocumentItem | null>(null);
  // The last PDF opened in the reader — powers the "Riprendi lettura" quick action
  // (toolbar + radial). Seeded from the recent shelf at load so it survives a restart.
  let lastReadDoc = $state<DocumentItem | null>(null);
  // Compact tool bar: which global-radial group's dropdown is open (items are
  // derived live from buildGlobalRadial() at render, so disabled/badge stay fresh).
  let toolMenu = $state<{ id: string; x: number; y: number } | null>(null);
  // Which tool-bar group owns the current view, so its icon shows a "you are here"
  // mark (replaces the active highlight the moved sidebar buttons used to give).
  let activeToolGroup = $derived.by(() => {
    if (helpModal) return "g-help";
    if (careModal) return "g-tools";
    switch (filter.kind) {
      case "novita": return "g-novita";
      case "notes": return "g-notes";
      case "projects": return "g-projects";
      case "archivio": return "g-archivio";
      case "ask": return "g-ask";
      case "wiki": return "g-wiki";
      case "discover": return "g-disc";
      case "terminal": return "g-term";
      case "trash": return "g-trash";
      case "duplicates": return "g-tools";
      default: return null;
    }
  });
  let watchedFolder = $state<string | null>(null);
  let view = $state<"grid" | "list" | "map">("grid");
  // ----- Orbita layer: radial menu, command palette, popovers, map, spotlight -----
  let radial = $state<{
    x: number;
    y: number;
    items: RadialItem[];
    title: string;
    subtitle?: string;
    thumb?: string | null;
  } | null>(null);
  let paletteOpen = $state(false);
  let sortPop = $state(false); // "Ordina" popover in the strip
  let indexPop = $state(false); // semantic-index popover in the header
  let sidebarHidden = $state(
    typeof localStorage !== "undefined" && localStorage.getItem("scriptorium-sidebar") === "hidden",
  );
  $effect(() => {
    try {
      localStorage.setItem("scriptorium-sidebar", sidebarHidden ? "hidden" : "shown");
    } catch {
      /* ignore */
    }
  });
  // Anchored flyout panels opened from the radial menu (tag / collection pickers).
  let tagPanel = $state<{ doc: DocumentItem; x: number; y: number } | null>(null);
  let collPanel = $state<{ doc: DocumentItem; x: number; y: number } | null>(null);
  // Semantic constellation (map view) data, loaded lazily and invalidated on library changes.
  let graph = $state<SimilarityGraph | null>(null);
  let graphLoading = $state(false);
  // "Riscopri" spotlight: a serendipitous pick from the library.
  let spotlight = $state<{ doc: DocumentItem; blurb: string } | null>(null);
  // Page requested for the viewer by "ask" source chips (deep link into the PDF).
  let openDocPage = $state<number | null>(null);
  let searchEl = $state<HTMLInputElement | undefined>();
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
  type SortKey = "favorite" | "author" | "title" | "year" | "venue" | "added";
  // Multi-criteria sort: criteria apply in the order the user activated them.
  let sortChain = $state<{ key: SortKey; dir: "asc" | "desc" }[]>([]);
  const SORT_KEYS: SortKey[] = ["favorite", "author", "title", "year", "venue", "added"];
  const SORT_LABELS: Record<SortKey, string> = {
    favorite: "Preferiti",
    author: "Primo autore",
    title: "Titolo",
    year: "Anno",
    venue: "Rivista",
    added: "Aggiunto",
  };
  // Direction applied on first activation (the most natural for each field).
  const SORT_NATURAL: Record<SortKey, "asc" | "desc"> = {
    favorite: "desc", // favorites first
    author: "asc",
    title: "asc",
    year: "desc", // newest first
    venue: "asc",
    added: "desc", // most recent first
  };
  let selected = $state<number[]>([]);
  // O(1) membership for the grid/list hot paths: every card checks it per render, and a
  // "Seleziona tutti" toggle otherwise re-runs O(selected) array scans for every row.
  let selectedSet = $derived(new Set(selected));
  let dupGroups = $state<number[][]>([]);
  let dupMap = $state<Record<number, DocumentItem>>({});
  let idModal = $state(false);
  let idText = $state("");
  let addingIds = $state(false);
  // "Aggancia da URL" + browser connector
  let urlModal = $state(false);
  let urlInput = $state("");
  let urlBusy = $state(false);
  let connectorInfo = $state<ConnectorInfo | null>(null);
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
  // "Novità" feed: papers surfaced by the on-launch sweep of saved searches.
  let novitaN = $state(0); // unread count → nav badge
  let novitaGroups = $state<NovitaGroup[]>([]);
  let novitaLoading = $state(false);
  let novitaSweeping = $state(false);
  let acceptingHit = $state<number | null>(null);
  let novitaP: Promise<() => void> | null = null;
  // In-flight accept/ignore mutations: while > 0 the event-driven feed reload is
  // skipped so it can't revert an optimistic removal (a card wouldn't reappear).
  let novitaMutating = 0;
  // ----- RAG engine ("Chiedi alla libreria") -----
  let askQuestion = $state("");
  let askAnswer = $state("");
  let askSources = $state<AskResult["sources"]>([]);
  let asking = $state(false);
  let ragStatus = $state<RagStatus | null>(null);
  let ragBuilding = $state(false);
  let ragProg = $state<{ done: number; total: number } | null>(null);
  // Reference-DOI backfill (Cura → Gap di citazioni)
  let refdoiRunning = $state(false);
  let refdoiProg = $state<{ done: number; total: number; resolved: number } | null>(null);
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
  // ----- Controllo aggiornamenti: solo un avviso (nessun download automatico) -----
  let updateLatest = $state<string | null>(null); // versione più nuova trovata
  let updateUrl = $state("https://github.com/shelaik/scriptorium");
  /** Confronta la versione con GitHub. `manual` = lanciato dall'utente (parla
   *  sempre); all'avvio è silenzioso, gira solo con la scoperta online attiva
   *  e al più una volta al giorno. */
  async function checkUpdatesNow(manual: boolean) {
    if (manual) status = "Controllo aggiornamenti…";
    try {
      const u = await checkUpdate();
      updateUrl = u.url;
      if (u.newer && u.latest) {
        updateLatest = u.latest;
        status = `È disponibile Scriptorium ${u.latest} (hai la ${u.current}) — clic sul segnalino in alto per aprire GitHub`;
      } else if (manual) {
        status = u.latest
          ? `Sei aggiornato: ${u.current} è l'ultima versione`
          : "Controllo non riuscito: repository non raggiungibile (privato oppure offline)";
      }
    } catch (e) {
      if (manual) status = "Controllo aggiornamenti: " + e;
    }
  }

  let helpModal = $state(false);
  // Guida a schede: si riapre sempre da «Inizia qui». È una finestra flottante
  // NON modale: si trascina, resta aperta mentre si lavora e — a scelta — resta
  // in primo piano sopra ogni vista (lettore incluso).
  type HelpTab = "inizia" | "libreria" | "lettura" | "scrittura" | "scoperta" | "ai" | "faq";
  let helpTab = $state<HelpTab>("inizia");
  let helpPos = $state({ x: 80, y: 80 });
  let helpPin = $state(false);
  try {
    helpPin = localStorage.getItem("scriptorium-help-pin") === "1";
  } catch {
    /* localStorage assente */
  }
  $effect(() => {
    try {
      localStorage.setItem("scriptorium-help-pin", helpPin ? "1" : "0");
    } catch {
      /* ignora */
    }
  });
  function openHelp() {
    helpTab = "inizia";
    if (!helpModal) {
      // Riapri dov'era l'ultima volta; al primo uso, in alto a destra.
      let saved: { x: number; y: number } | null = null;
      try {
        saved = JSON.parse(localStorage.getItem("scriptorium-help-pos") ?? "null");
      } catch {
        /* posizione corrotta: usa il default */
      }
      const x = saved?.x ?? window.innerWidth - 660 - 28;
      const y = saved?.y ?? 64;
      helpPos = {
        x: Math.min(Math.max(12, x), Math.max(12, window.innerWidth - 200)),
        y: Math.min(Math.max(0, y), Math.max(0, window.innerHeight - 120)),
      };
    }
    helpModal = true;
  }
  /** Trascina la guida dalla barra del titolo (i controlli restano cliccabili). */
  function startHelpDrag(e: MouseEvent) {
    if ((e.target as HTMLElement | null)?.closest("button, input, label")) return;
    e.preventDefault();
    const sx = e.clientX - helpPos.x;
    const sy = e.clientY - helpPos.y;
    const move = (ev: MouseEvent) => {
      if (!(ev.buttons & 1)) return up(); // mouseup perso (Alt+Tab): sgancia
      helpPos = {
        x: Math.min(Math.max(ev.clientX - sx, -460), window.innerWidth - 160),
        y: Math.min(Math.max(ev.clientY - sy, 0), window.innerHeight - 44),
      };
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      try {
        localStorage.setItem("scriptorium-help-pos", JSON.stringify(helpPos));
      } catch {
        /* ignora */
      }
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  }
  let aboutModal = $state(false);
  const APP_VERSION = "0.9.34";
  const APP_YEAR = "2026";
  let settingsTab = $state<"online" | "ai" | "obsidian" | "connector" | "mcp" | "backup" | "maint">("online");
  // Percorsi dei binari compagni (CLI + server MCP), per la scheda «CLI e MCP».
  let companions = $state<CompanionPaths | null>(null);
  async function loadCompanions() {
    try {
      companions = await companionPaths();
    } catch (e) {
      status = "Errore percorsi: " + e;
    }
  }
  const mcpAddCmd = $derived(companions ? `claude mcp add scriptorium -- "${companions.mcp}"` : "");
  const mcpJsonSnippet = $derived(
    companions ? `"scriptorium": { "command": ${JSON.stringify(companions.mcp)} }` : "",
  );
  async function copyPlain(text: string, okMsg: string) {
    try {
      await navigator.clipboard.writeText(text);
      status = okMsg;
    } catch {
      status = "Copia non riuscita";
    }
  }
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
      case "venue":
        return (a.venue ?? "").toLowerCase().localeCompare((b.venue ?? "").toLowerCase());
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
  // ---- Pannello dettaglio + cursore tastiera sulla libreria ----
  // Un click sulla card mette a fuoco il documento (pannello a destra);
  // doppio click / Invio aprono il lettore. Le frecce muovono il fuoco.
  let focusId = $state<number | null>(null);
  const panelDoc = $derived(
    focusId == null
      ? null
      : (displayed.find((d) => d.id === focusId) ?? docs.find((d) => d.id === focusId) ?? null),
  );
  function focusCard(d: DocumentItem) {
    focusId = focusId === d.id ? focusId : d.id;
  }
  /** Colonne correnti della griglia (per ↑/↓), misurate dal layout reale. */
  function gridColumns(): number {
    const el = document.querySelector(".grid");
    if (!el) return 1;
    return Math.max(1, getComputedStyle(el).gridTemplateColumns.split(" ").length);
  }
  function moveFocus(delta: number) {
    if (!displayed.length) return;
    const i = focusId == null ? -1 : displayed.findIndex((d) => d.id === focusId);
    const next = i === -1 ? 0 : Math.min(displayed.length - 1, Math.max(0, i + delta));
    focusId = displayed[next].id;
    ensureThumbs([displayed[next]]);
    tick().then(() => document.querySelector(".kfocus")?.scrollIntoView({ block: "nearest" }));
  }

  function clearSort() {
    sortChain = [];
  }

  // Select-all (for batch actions): toggles the whole shown list.
  let allSelected = $derived(
    displayed.length > 0 && displayed.every((d) => selectedSet.has(d.id)),
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
      // Bump the epoch so any in-flight search's `finally` is a no-op, and reset the
      // spinner + stale hits here — otherwise clearing the box mid-search leaves
      // "cerco…" stuck forever and retyping flashes the previous query's results.
      searchSeq++;
      searching = false;
      results = [];
      noteResults = [];
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
      // Notes are a best-effort, separate source — never block the doc results.
      try {
        const nr = await searchNotes(q);
        if (myId === searchSeq) noteResults = nr;
      } catch {
        /* ignore */
      }
    }, 250);
  });

  // Bounded thumbnail loader: queue ids and keep only a few getThumbnail() calls in
  // flight at once. Opening a large library used to fire one IPC per document all at
  // once — thousands serializing on the single DB mutex, each returning a base64 PNG
  // — which janked first paint. The queue caps that burst without dropping any cover.
  const THUMB_CONCURRENCY = 6;
  let thumbQueue: number[] = [];
  const thumbQueued = new Set<number>();
  let thumbActive = 0;
  function pumpThumbs() {
    while (thumbActive < THUMB_CONCURRENCY && thumbQueue.length) {
      const id = thumbQueue.shift()!;
      thumbQueued.delete(id);
      if (thumbs[id]) continue; // filled in the meantime
      thumbActive++;
      getThumbnail(id)
        .then((t) => {
          if (t) thumbs[id] = t;
        })
        .catch(() => {})
        .finally(() => {
          thumbActive--;
          pumpThumbs();
        });
    }
  }
  function ensureThumbs(items: DocumentItem[]) {
    for (const d of items) {
      if (d.has_thumb && !thumbs[d.id] && !thumbQueued.has(d.id)) {
        thumbQueued.add(d.id);
        thumbQueue.push(d.id);
      }
    }
    pumpThumbs();
  }
  /** Drop cached covers (and any pending fetches) for documents that no longer exist. */
  function forgetThumbs(ids: number[]) {
    const gone = new Set(ids);
    for (const id of ids) {
      delete thumbs[id];
      thumbQueued.delete(id);
    }
    thumbQueue = thumbQueue.filter((id) => !gone.has(id));
  }

  /** Re-render every cover at high resolution so zoomed-in grid thumbnails are crisp. */
  async function rebuildThumbs() {
    if (rebuildingThumbs) return;
    rebuildingThumbs = true;
    status = "Rigenerazione anteprime in corso…";
    try {
      const n = await rebuildThumbnails();
      // Drop the cached data URLs so the freshly-rendered, higher-res covers reload.
      thumbs = {};
      thumbQueue = [];
      thumbQueued.clear();
      ensureThumbs(displayed);
      status = `✓ ${n} anteprime rigenerate ad alta risoluzione`;
    } catch (e) {
      status = `Errore nella rigenerazione delle anteprime: ${e}`;
    } finally {
      rebuildingThumbs = false;
    }
  }

  function filterArg() {
    if (filter.kind === "collection") return { collectionId: filter.id };
    if (filter.kind === "favorite") return { flag: "favorite" as const };
    if (filter.kind === "unread") return { flag: "unread" as const };
    if (filter.kind === "github") return { flag: "github" as const };
    if (filter.kind === "peerreviewed") return { flag: "peerreviewed" as const };
    if (filter.kind === "mywork") return { flag: "mywork" as const };
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
        // Seed "Riprendi lettura" so the action works right after a restart, before
        // anything is opened this session. A live open() overrides this.
        if (!lastReadDoc) lastReadDoc = recentDocs.find((d) => d.has_file) ?? null;
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
    // Lasciando la vista Progetti, la richiesta "apri questo progetto" decade:
    // al prossimo ingresso il componente riparte dal progetto più recente.
    if (f.kind !== "projects") projectsOpenSlug = null;
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
      filter.kind === "github" ||
      filter.kind === "peerreviewed" ||
      filter.kind === "mywork" ||
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
  async function doRestoreFolder() {
    const dir = await open({ directory: true, multiple: false, title: "Scegli la cartella di backup (contiene pdfmanage.db)" });
    if (!dir || Array.isArray(dir)) return;
    await restoreFrom(dir);
  }
  async function doRestoreDbFile() {
    const f = await open({
      directory: false,
      multiple: false,
      title: "Scegli un file .db di backup",
      filters: [{ name: "Database", extensions: ["db"] }],
    });
    if (!f || Array.isArray(f)) return;
    await restoreFrom(f);
  }
  async function restoreFrom(source: string) {
    let info;
    try {
      info = await inspectBackup(source);
    } catch (e) {
      status = "Backup non valido: " + e;
      return;
    }
    const kind = info.full
      ? "libreria completa (database + PDF + note + progetti)"
      : "solo il database (catalogo, tag, grafo, annotazioni)";
    const ok = await confirmAsk(
      `Ripristinare da questo backup?\n\n• ${info.doc_count} documenti\n• ${kind}\n\nLa libreria ATTUALE verrà sostituita (ne viene salvata prima una copia di sicurezza in backups/). L'app si riavvierà per applicare il ripristino.`,
      "Ripristina e riavvia",
      true,
    );
    if (!ok) return;
    try {
      await stageRestore(source);
    } catch (e) {
      status = "Non riesco a preparare il ripristino: " + e;
      return;
    }
    try {
      await restartApp();
    } catch {
      status = "Ripristino pronto: verrà applicato al prossimo avvio di Scriptorium.";
    }
  }

  // ----- Hygiene actions -----
  async function trashSelected(ids: number[]) {
    if (!ids.length) return;
    const n = ids.length;
    if (!(await confirmAsk(`Spostare ${n} ${n > 1 ? "documenti" : "documento"} nel cestino?`, "Sposta nel cestino", false))) return;
    await deleteDocuments(ids);
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
    forgetThumbs([id]);
    await loadDocs();
  }
  async function emptyTrash() {
    const n = docs.length;
    if (!n) return;
    if (!(await confirmAsk(`Svuotare il cestino? ${n} ${n > 1 ? "documenti verranno eliminati" : "documento verrà eliminato"} definitivamente.`, "Svuota cestino"))) return;
    const ids = docs.map((d) => d.id);
    await purgeDocuments(ids);
    forgetThumbs(ids);
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
    if (enriching) return; // callable from header/radial/palette: no concurrent runs
    enriching = true;
    status = "Recupero metadati dei documenti incompleti (arXiv dal nome file, DOI e titolo dal PDF)…";
    try {
      const res = await recoverMissingMetadata();
      const parts = [`${res.updated} aggiornati`];
      if (res.from_arxiv) parts.push(`${res.from_arxiv} da arXiv (nome file)`);
      if (res.unresolved)
        parts.push(`${res.unresolved} da confermare a mano (tasto destro → Organizza → Recupera metadati)`);
      if (res.errors.length) parts.push(`${res.errors.length} errori`);
      status = "Metadati: " + parts.join(" · ");
      await loadDocs();
    } catch (e) {
      status = "Errore metadati: " + e;
    } finally {
      enriching = false;
      metaScan = null;
    }
  }

  let repairing = $state(false);
  // Plancia: log attività su file (Impostazioni → Manutenzione).
  let pulseLog = $state(false);
  let pulseLogDir = $state("");
  async function togglePulseLog() {
    try {
      const s = await setPulseLog(!pulseLog);
      pulseLog = s.enabled;
      pulseLogDir = s.dir;
    } catch (e) {
      status = "Log Plancia: " + e;
      // La checkbox non deve mentire: riallineala allo stato reale del backend.
      try {
        const s = await pulseLogStatus();
        pulseLog = s.enabled;
        pulseLogDir = s.dir;
      } catch {
        /* ignore */
      }
    }
  }
  let repairMsg = $state("");
  async function repairMeta() {
    if (!(await confirmAsk(
      "Verifico ogni documento e correggo quelli il cui titolo non corrisponde al PDF " +
      "(di solito causati dal DOI di un lavoro citato). Per ognuno cerco il record giusto " +
      "online per titolo (Crossref/arXiv); i paper arXiv si recuperano dall'id nel nome file; " +
      "se non è indicizzato da nessuna parte, ricavo almeno il titolo dalla prima riga del PDF. " +
      "I documenti già corretti non vengono toccati. Può richiedere fino a un minuto. Procedo?",
      "Ripara", false
    ))) return;
    repairing = true;
    repairMsg = "Verifica e riparazione in corso… (può richiedere fino a un minuto)";
    status = repairMsg;
    try {
      const res = await repairMetadata();
      if (res.checked === 0) {
        repairMsg = "Nessun metadato errato trovato — tutto a posto ✓";
      } else {
        const parts = [`${res.checked} corretti`];
        if (res.repaired_arxiv) parts.push(`${res.repaired_arxiv} da arXiv`);
        if (res.resolved_online) parts.push(`${res.resolved_online} risolti online per titolo`);
        if (res.retitled) parts.push(`${res.retitled} dal testo del PDF`);
        repairMsg = "Riparazione completata: " + parts.join(" · ");
      }
      status = repairMsg;
      await loadDocs();
    } catch (e) {
      repairMsg = "Errore riparazione: " + e;
      status = repairMsg;
    } finally {
      repairing = false;
    }
  }

  async function generateIndex() {
    if (generating) return; // callable from radial/palette/map too: no concurrent jobs
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

  async function importRefManagerDialog() {
    const file = await open({
      multiple: false,
      filters: [
        { name: "Bibliografia (BibTeX / RIS / CSL-JSON)", extensions: ["bib", "bibtex", "ris", "json", "csljson", "csl", "txt"] },
      ],
    });
    if (typeof file !== "string") return;
    // Optionally point at the export's PDF folder (Zotero "Export Files", etc.) so
    // attachments get pulled in. Skippable — if the file itself carries PDF paths,
    // those are used automatically.
    let pdfDir: string | undefined;
    if (await confirmAsk(
      "Indicare una cartella con i PDF esportati?\n\nServe solo se la tua esportazione tiene i PDF in una cartella a parte (es. Zotero «Esporta file»). Se i percorsi dei PDF sono già dentro il file, puoi saltare.",
      "Scegli cartella…",
      false,
    )) {
      const dir = await open({ directory: true });
      if (typeof dir === "string") pdfDir = dir;
    }
    status = "Importo la bibliografia…";
    try {
      const res = await importReferenceManager(file, pdfDir);
      const parts = [`${res.added} aggiunti`];
      if (res.pdfs_attached) parts.push(`${res.pdfs_attached} con PDF`);
      if (res.tags_applied) parts.push(`${res.tags_applied} tag`);
      if (res.duplicates) parts.push(`${res.duplicates} già presenti`);
      if (res.dois_resolved) parts.push(`${res.dois_resolved} DOI recuperati`);
      if (res.errors.length) parts.push(`${res.errors.length} errori`);
      status = `${res.format}: ` + (res.entries ? parts.join(" · ") : "nessuna voce trovata nel file");
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore import bibliografia: " + e;
    }
  }

  async function importLatexDialog() {
    const file = await open({
      multiple: false,
      filters: [{ name: "Progetto LaTeX (.zip)", extensions: ["zip"] }],
    });
    if (typeof file !== "string") return;
    status = "Importo il progetto LaTeX…";
    try {
      const res = await importLatexZip(file);
      const parts: string[] = [];
      if (res.imported) parts.push(`${res.imported} tuoi paper aggiunti`);
      if (res.duplicates) parts.push(`${res.duplicates} già presenti`);
      if (res.references_linked) parts.push(`${res.references_linked} riferimenti collegati`);
      if (res.dois_resolved) parts.push(`${res.dois_resolved} DOI recuperati`);
      if (!res.pdfs_found) parts.push("nessun PDF nel .zip");
      if (res.errors.length) parts.push(`${res.errors.length} errori`);
      status = "LaTeX: " + (parts.length ? parts.join(" · ") : "niente da importare");
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore import LaTeX: " + e;
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

  // Snowball / online citation explorer (OpenAlex).
  let exploreModal = $state(false);
  // Esploratore: vista Mappa (default) o Lista; popover azioni sul nodo cliccato.
  let exploreView = $state<"map" | "list">("map");
  let mapPop = $state<{ r: SearchResult; x: number; y: number } | null>(null);
  /** Open the in-library document matching this DOI (map node click). */
  async function openByDoi(doi: string) {
    try {
      const all = await listDocuments();
      const d = all.find((x) => x.doi?.toLowerCase() === doi.toLowerCase());
      if (d) {
        exploreModal = false;
        openDocument(d);
      } else {
        status = "Non trovo questo DOI in libreria (rigenera l'esplorazione?)";
      }
    } catch (e) {
      status = "" + e;
    }
  }
  let exploreLoading = $state(false);
  let exploreData = $state<CitationNeighbors | null>(null);
  let exploreTitle = $state("");
  // Seed of the paper currently centred in the explorer (DOI / OpenAlex id / gated title).
  let exploreSeed: ExploreSeed | null = null;
  // Breadcrumb of papers visited via «Esplora ↗», so snowballing is non-destructive.
  let exploreStack = $state<{ seed: ExploreSeed; title: string }[]>([]);
  // «+ PDF»: which neighbour's paste-the-PDF-URL field is open, and its value.
  let pdfInputFor = $state<string | null>(null);
  let pdfUrlInput = $state("");
  /** Focus the PDF-URL field as soon as it appears (so the user can paste right away). */
  function pdfFocus(node: HTMLInputElement) {
    node.focus();
  }
  /** Re-seed the explorer (DOI, OpenAlex id, or gated title — the seed doc or any neighbour). */
  async function runExplore(seed: ExploreSeed, title: string) {
    exploreLoading = true;
    exploreData = null;
    exploreTitle = title;
    exploreSeed = seed;
    try {
      exploreData = await exploreCitations(seed);
    } catch (e) {
      status = "Errore esplora citazioni: " + e;
      exploreModal = false;
    } finally {
      exploreLoading = false;
    }
  }
  /** Snowball to a neighbour, remembering the current node so «← Indietro» can return. */
  async function navExplore(seed: ExploreSeed, title: string) {
    mapPop = null;
    if (exploreSeed) exploreStack = [...exploreStack, { seed: exploreSeed, title: exploreTitle }];
    await runExplore(seed, title);
  }
  /** Go back to the previously explored paper. */
  async function backExplore() {
    mapPop = null;
    const prev = exploreStack[exploreStack.length - 1];
    if (!prev) return;
    exploreStack = exploreStack.slice(0, -1);
    await runExplore(prev.seed, prev.title);
  }
  /** Save one neighbour list (references or citations) as a Markdown file with paper links. */
  async function saveNeighborList(kind: "references" | "citations") {
    if (!exploreData) return;
    const list = kind === "references" ? exploreData.references : exploreData.citations;
    if (!list.length) {
      status = "La lista è vuota — niente da salvare";
      return;
    }
    const heading = kind === "references" ? "Riferimenti (questo paper cita)" : "Citato da";
    const lines = [`# ${heading}`, "", `Paper: ${exploreTitle}`, `Fonte: OpenAlex · ${list.length} paper`, ""];
    for (const r of list) {
      const link = r.url ?? (r.doi ? `https://doi.org/${r.doi}` : "");
      const meta = [r.authors?.[0], r.year, r.venue].filter(Boolean).join(", ");
      lines.push(
        `- **${r.title ?? "Senza titolo"}**${meta ? ` — ${meta}` : ""}${r.citations ? ` · ${r.citations} cit.` : ""}${link ? `\n  <${link}>` : ""}`,
      );
    }
    try {
      const base = kind === "references" ? "riferimenti" : "citato-da";
      // Suggest a filename that also carries the paper's title, sanitized for the
      // filesystem (strip Windows-illegal chars, collapse spaces, cap length).
      const safeTitle = (exploreTitle || "")
        .replace(/[\\/:*?"<>|]/g, "")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 70)
        .replace(/[.\s]+$/, "")
        .trim();
      const fname = safeTitle ? `${safeTitle} — ${base}.md` : `${base}.md`;
      const path = await save({
        defaultPath: fname,
        filters: [
          { name: "Markdown", extensions: ["md"] },
          { name: "Testo", extensions: ["txt"] },
        ],
      });
      if (!path) return;
      await writeTextFile(path, lines.join("\n") + "\n");
      status = `Lista salvata in ${path}`;
    } catch (e) {
      status = "Errore nel salvataggio della lista: " + e;
    }
  }
  async function openExplore(d: DocumentItem) {
    if (!d.doi && !(d.title ?? "").trim()) {
      status = "Serve un DOI o almeno un titolo per esplorare le citazioni";
      return;
    }
    exploreStack = [];
    exploreView = "map";
    mapPop = null;
    exploreModal = true;
    await runExplore({ doi: d.doi, title: d.title }, d.title ?? "documento");
  }
  /** Add one neighbour to the library (reuses discover_add) and mark it in place. */
  async function addNeighbor(r: SearchResult) {
    addingExt = r.external_id;
    try {
      const res = await discoverAdd(r);
      const mark = (list: SearchResult[]) =>
        list.map((x) => (x.external_id === r.external_id ? { ...x, in_library: true } : x));
      if (exploreData)
        exploreData = { ...exploreData, references: mark(exploreData.references), citations: mark(exploreData.citations) };
      if (mapPop && mapPop.r.external_id === r.external_id)
        mapPop = { ...mapPop, r: { ...mapPop.r, in_library: true } };
      status =
        res === "added_pdf" ? "Aggiunto con PDF ✓" : res === "added_ref" ? "Aggiunto (solo metadati) ✓" : "Già presente";
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      addingExt = null;
    }
  }
  /** Add a neighbour using a PDF URL the user pasted (e.g. one they opened in the browser).
   *  Reuses discover_add: the pasted link is passed as oa_pdf_url, so it goes through the
   *  same SSRF-guarded, size-capped download as any Open-Access PDF. */
  async function addNeighborWithPdf(r: SearchResult) {
    const url = pdfUrlInput.trim();
    if (!url) return;
    if (!/^https:\/\//i.test(url)) {
      status = "Serve un link diretto al PDF che inizia con https://";
      return;
    }
    addingExt = r.external_id;
    try {
      const res = await discoverAdd({ ...r, oa_pdf_url: url });
      const mark = (list: SearchResult[]) =>
        list.map((x) => (x.external_id === r.external_id ? { ...x, in_library: true } : x));
      if (exploreData)
        exploreData = { ...exploreData, references: mark(exploreData.references), citations: mark(exploreData.citations) };
      status =
        res === "added_pdf"
          ? "Aggiunto col PDF ✓"
          : res === "added_ref"
            ? "Il link non era un PDF diretto: salvato come riferimento (controlla l'URL)"
            : "Già presente";
      if (res !== "added_ref") {
        pdfInputFor = null;
        pdfUrlInput = "";
      }
      await loadDocs();
      await loadSidebar();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      addingExt = null;
    }
  }
  /** Best-effort: fill the PDF-URL field from the clipboard (falls back to manual paste). */
  async function pastePdfUrlFromClipboard() {
    try {
      const t = await navigator.clipboard.readText();
      if (t && t.trim()) pdfUrlInput = t.trim();
      else status = "Appunti vuoti — copia prima il link del PDF";
    } catch {
      status = "Non riesco a leggere gli appunti: incolla con Ctrl+V";
    }
  }

  // ---- "Aggancia da URL" + browser connector (bookmarklet) ----
  async function loadConnector() {
    try {
      connectorInfo = await getConnectorInfo();
    } catch {
      /* connector optional; ignore */
    }
  }
  function openUrlModal() {
    urlModal = true;
    urlInput = "";
    loadConnector();
  }
  async function pasteUrlFromClipboard() {
    try {
      const t = await navigator.clipboard.readText();
      if (t && t.trim()) urlInput = t.trim();
      else status = "Appunti vuoti — copia prima il link del PDF";
    } catch {
      status = "Non riesco a leggere gli appunti: incolla con Ctrl+V";
    }
  }
  async function doAddFromUrl() {
    const u = urlInput.trim();
    if (!u) return;
    urlBusy = true;
    try {
      const r = await addFromUrl(u);
      status =
        r === "added"
          ? "PDF agganciato alla libreria ✓"
          : r === "duplicate"
            ? "Già presente in libreria"
            : "Non sembra un PDF diretto — usa il link che termina in .pdf";
      if (r === "added") {
        urlInput = "";
        urlModal = false;
        await loadDocs();
        await loadStatus();
        await loadSidebar();
      }
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      urlBusy = false;
    }
  }
  async function toggleConnector(enabled: boolean) {
    try {
      connectorInfo = await setConnectorEnabled(enabled);
    } catch (e) {
      status = "Errore connettore: " + e;
    }
  }

  // ---- Appunti intelligenti: quando la finestra torna in primo piano e negli
  // appunti c'è un link che sembra un PDF, proponi l'aggancio con un toast.
  // Tutto locale: la lettura avviene solo al focus e non parte nulla finché
  // l'utente non clicca. Interruttore in Impostazioni → Connettore.
  let clipAssist = $state(
    typeof localStorage === "undefined" || localStorage.getItem("scriptorium-clipassist") !== "off",
  );
  $effect(() => {
    try {
      localStorage.setItem("scriptorium-clipassist", clipAssist ? "on" : "off");
    } catch {
      /* ignore */
    }
  });
  let clipOffer = $state<string | null>(null); // URL proposto nel toast
  let clipBusy = $state(false);
  let clipSeen = ""; // ultimo testo già proposto/gestito (una proposta per copia)

  /** Il testo copiato sembra un link a un PDF agganciabile? */
  function looksLikePdfUrl(t: string): boolean {
    if (!/^https?:\/\/\S+$/i.test(t)) return false;
    return /\.pdf(\?|#|$)/i.test(t) || /arxiv\.org\/(abs|pdf)\//i.test(t) || /openreview\.net\/pdf/i.test(t);
  }

  async function checkClipboard() {
    if (!clipAssist || clipBusy || urlModal || openDoc) return;
    let t = "";
    try {
      t = (await navigator.clipboard.readText()).trim();
    } catch {
      return; // appunti vuoti o non testuali: mai disturbare
    }
    if (!t || t === clipSeen || !looksLikePdfUrl(t)) return;
    clipSeen = t;
    // Le pagine arXiv /abs/ non sono il file: passa direttamente al PDF.
    clipOffer = t.replace(/arxiv\.org\/abs\//i, "arxiv.org/pdf/");
  }

  async function clipGrab() {
    if (!clipOffer || clipBusy) return;
    const u = clipOffer;
    clipBusy = true;
    status = "Aggancio dagli appunti…";
    try {
      const r = await addFromUrl(u);
      status =
        r === "added"
          ? "PDF agganciato dagli appunti ✓"
          : r === "duplicate"
            ? "Già presente in libreria"
            : "Quel link non è un PDF diretto";
      if (r === "added") {
        await loadDocs();
        await loadStatus();
        await loadSidebar();
      }
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      clipBusy = false;
      clipOffer = null;
    }
  }
  /** Build the one-line `javascript:` bookmarklet (ASCII-only) for the current port+token. */
  function buildBookmarklet(port: number, token: string): string {
    const js =
      "(function(){" +
      "var u=location.href;" +
      "var m=document.querySelector('meta[name=\"citation_pdf_url\"]');" +
      "var a=document.querySelector('a[href$=\".pdf\"]');" +
      "var p=/\\.pdf(\\?|#|$)/i.test(u)?u:(m&&m.content)||(a&&a.href)||u;" +
      "var t=document.createElement('div');t.textContent='Scriptorium: invio...';" +
      "t.style.cssText='position:fixed;z-index:2147483647;right:16px;bottom:16px;padding:10px 14px;background:#2b4a78;color:#fff;font:14px sans-serif;border-radius:8px;box-shadow:0 4px 16px rgba(0,0,0,.3)';" +
      "document.body.appendChild(t);" +
      "var L={added:'aggiunto \\u2713',duplicate:'gi\\u00e0 in libreria',not_pdf:'non \\u00e8 un PDF diretto',error:'errore'};" +
      "fetch('http://127.0.0.1:" +
      port +
      "/add?url='+encodeURIComponent(p),{headers:{'X-Scriptorium-Token':'" +
      token +
      "'}})" +
      ".then(function(r){return r.json()})" +
      ".then(function(j){t.textContent='Scriptorium: '+(L[j.status]||j.status)})" +
      // Strict-CSP sites (e.g. GitHub) block the fetch to loopback: fall back to a
      // top-level navigation to the connector's /grab page (not subject to
      // connect-src). URL + token travel in the #fragment, never on the wire.
      ".catch(function(){t.textContent='Scriptorium: il sito blocca il connettore, continuo in una scheda\\u2026';" +
      "window.open('http://127.0.0.1:" +
      port +
      "/grab#u='+encodeURIComponent(p)+'&t=" +
      token +
      "')})" +
      ".finally(function(){setTimeout(function(){t.remove()},4000)});" +
      "})();";
    return "javascript:" + js;
  }
  const bookmarklet = $derived(
    connectorInfo ? buildBookmarklet(connectorInfo.port, connectorInfo.token) : "",
  );
  async function copyBookmarklet() {
    if (!bookmarklet) return;
    try {
      await navigator.clipboard.writeText(bookmarklet);
      status = "Bookmarklet copiato — crea un preferito e incolla questo come indirizzo";
    } catch {
      status = "Copia non riuscita";
    }
  }
  async function openCitations(d: DocumentItem) {
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
  /** Open a library document by id (used by citation links), optionally at a page. */
  async function openById(id: number, page: number | null = null) {
    const found = docs.find((d) => d.id === id) ?? recentDocs.find((d) => d.id === id);
    if (found) {
      citModal = false;
      openDocument(found, page);
      return;
    }
    // Not in the current view: fetch the full list and locate it.
    try {
      const all = await listDocuments();
      const d = all.find((x) => x.id === id);
      if (d) {
        citModal = false;
        openDocument(d, page);
      }
    } catch {
      /* ignore */
    }
  }

  // Per-doc "Trova PDF" is now the CANDIDATES dialog (PdfFinder): the strictly
  // gated automatic path lives on in the batch sweep (findPdf) only.

  // ---- «Trova PDF» in blocco: allega copie OA a più riferimenti in sequenza ----
  let pdfBatch = $state<{ done: number; total: number; found: number } | null>(null);
  let pdfBatchCancel = false;
  async function batchFindPdf(targets: DocumentItem[]) {
    const list = targets.filter((d) => !d.has_file);
    if (!list.length) {
      status = "Nessun riferimento senza PDF qui";
      return;
    }
    if (pdfBatch) return; // one sweep at a time
    pdfBatchCancel = false;
    let done = 0;
    let found = 0;
    let dup = 0;
    let miss = 0;
    let err = 0;
    pdfBatch = { done, total: list.length, found };
    for (const d of list) {
      if (pdfBatchCancel) break;
      try {
        const r = await findPdf(d.id);
        if (r === "attached") found++;
        else if (r === "duplicate") dup++;
        else miss++;
      } catch {
        err++;
      }
      done++;
      pdfBatch = { done, total: list.length, found };
    }
    const parts = [`${found} PDF allegati su ${list.length}`];
    if (dup) parts.push(`${dup} già in libreria altrove`);
    if (miss) parts.push(`${miss} senza copia Open Access`);
    if (err) parts.push(`${err} errori`);
    status = "Trova PDF: " + parts.join(" · ") + (pdfBatchCancel ? " · interrotto" : "");
    pdfBatch = null;
    await loadDocs();
    await loadSidebar();
  }
  /** Sweep EVERY reference-only entry in the library (not just the current view). */
  async function findPdfAllRefs() {
    try {
      const all = await listDocuments();
      await batchFindPdf(all.filter((d) => !d.has_file));
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
    try {
      const pl = await pulseLogStatus();
      pulseLog = pl.enabled;
      pulseLogDir = pl.dir;
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
    // Save independently: a discovery hiccup must not silently block the AI save
    // (or vice versa) — that would leave the user thinking they enabled the AI.
    let err = "";
    try {
      await setDiscoverySettings(discEnabled, discEmail);
    } catch (e) {
      err = "ricerca online: " + e;
    }
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
    } catch (e) {
      err = err ? `${err}; AI: ${e}` : "AI: " + e;
    }
    settingsModal = false;
    status = err ? "Impostazioni salvate con errori — " + err : "Impostazioni salvate";
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
      // Unload models FIRST (frees VRAM via the API while the server is still up),
      // then stop the server. Killing the server alone can orphan the model runner
      // and leave the GPU memory allocated.
      try { await aiUnloadModels(); } catch { /* best-effort */ }
      await aiServerStop(provider);
      status = `${name}: fermato`;
      setTimeout(() => verifyProvider(provider), 1000);
    } catch (e) {
      status = "Arresto non riuscito: " + e;
    }
  }
  /** Toolbar/radial: free the GPU by unloading the active provider's models from
   *  VRAM, leaving the server and the AI feature running (reloads on next use). */
  async function freeGpuMemory() {
    try {
      const n = await aiUnloadModels();
      status = n > 0 ? `GPU liberata — ${n} modell${n === 1 ? "o" : "i"} scaricat${n === 1 ? "o" : "i"} dalla VRAM ✓` : "GPU liberata (nessun modello era caricato) ✓";
      setTimeout(() => refreshAiStatus(), 800);
    } catch (e) {
      status = "Impossibile liberare la GPU: " + e;
    }
  }
  /** Toolbar/radial: fully stop the AI — unload models, stop the local server, and
   *  turn the AI feature off (the switch in Impostazioni). */
  async function stopAiFully() {
    const provider: AiProvider = aiProvider;
    try { await aiUnloadModels(); } catch { /* best-effort */ }
    try { await aiServerStop(provider); } catch { /* best-effort — may be external */ }
    aiEnabled = false;
    try {
      await persistAi();
    } catch { /* ignore */ }
    status = "AI fermata: modelli scaricati, server arrestato, AI disattivata — il chip «AI off» in alto la riattiva";
    setTimeout(() => refreshAiStatus(), 800);
  }
  /** One-click re-enable from the header chip / radial: flip the saved switch back
   *  on (provider/model/URLs untouched) and refresh the gates that hide the AI
   *  features. If the server is down the chip turns gray and its tooltip guides to
   *  Impostazioni to start it. */
  async function quickEnableAi() {
    aiEnabled = true;
    try {
      await persistAi();
      await refreshAiStatus();
      // persistAi swallows its own errors — trust the LIVE state, not the absence
      // of an exception, before claiming success.
      if (!aiStat?.enabled) {
        status = "Impossibile attivare l'AI: il salvataggio non è riuscito — riprova dalle Impostazioni";
        return;
      }
      status = aiStat.reachable
        ? "AI attivata ✓"
        : "AI attivata — il server non risponde: avvialo dalle Impostazioni (clic sul chip AI)";
    } catch (e) {
      status = "Impossibile attivare l'AI: " + e;
    }
  }
  async function summarizeDoc(doc: DocumentItem) {
    aiBusy = doc.id;
    status = doc.has_summary
      ? "Rigenero il riassunto… (quello esistente verrà sostituito)"
      : "Riassunto in corso… (può richiedere un momento)";
    try {
      await summarizeDocument(doc.id);
      status = 'Riassunto generato — aprilo da "Modifica metadati"';
      await loadDocs(); // refresh the ✦ indicator
    } catch (e) {
      status = "Errore AI: " + e;
    } finally {
      aiBusy = null;
    }
  }
  async function autotagDoc(doc: DocumentItem) {
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
    // Skip documents that are already done: a summary is regenerated only on
    // explicit single-document request, and autotag only runs on untagged docs.
    const all = [...selected];
    const docOf = (id: number) => displayed.find((x) => x.id === id) ?? docs.find((x) => x.id === id);
    const ids = all.filter((id) => {
      const d = docOf(id);
      if (!d) return true;
      return kind === "summary" ? !d.has_summary : d.tags.length === 0;
    });
    const skipped = all.length - ids.length;
    if (!ids.length) {
      status =
        kind === "summary"
          ? "Tutti i selezionati hanno già un riassunto AI (per rifarne uno: tasto destro sul singolo documento)"
          : "Tutti i selezionati hanno già dei tag";
      return;
    }
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
    // Refresh so tag chips / ✦ summary indicators appear.
    await loadDocs();
    if (kind === "tags") await loadSidebar();
    const label = kind === "summary" ? "Riassunti" : "Tag automatici";
    status =
      `${label}: ${ok} completati` +
      (skipped ? ` · ${skipped} saltati (già presenti)` : "") +
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
    // Command-palette "Ricerca salvata: …" entries aren't disabled during a run
    // (unlike the Cerca button), so guard here: without this, launching a second
    // saved search while the first is in flight lets the slower one overwrite the
    // newer one's results under the wrong query, and its finally re-enables the UI.
    if (discovering) return;
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
      await refreshNovitaCount();
    } catch {
      /* ignore */
    }
  }

  // ----- "Novità": feed from the saved-search sweep -----
  async function refreshNovitaCount() {
    try {
      novitaN = await novitaCount();
    } catch {
      /* ignore — badge just stays put */
    }
  }
  async function openNovita() {
    setFilter({ kind: "novita" });
    novitaLoading = true;
    try {
      novitaGroups = await listNovita();
    } catch (e) {
      status = "Errore nel caricare le novità: " + e;
    } finally {
      novitaLoading = false;
    }
  }
  /** Add a feed item to the library, then drop it from the open feed + badge. */
  async function acceptNovita(watchId: number, hitId: number) {
    acceptingHit = hitId;
    novitaMutating++;
    try {
      const res = await acceptHit(hitId);
      status =
        res === "added_pdf"
          ? "Aggiunto alla libreria (PDF scaricato) ✓"
          : res === "added_ref"
            ? "Aggiunto come riferimento ✓"
            : "Era già in libreria";
      dropHitFromFeed(watchId, hitId);
      await refreshNovitaCount();
      loadDocs();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      acceptingHit = null;
      novitaMutating--;
    }
  }
  async function ignoreNovita(watchId: number, hitId: number) {
    novitaMutating++;
    try {
      await dismissHit(hitId);
      dropHitFromFeed(watchId, hitId);
      await refreshNovitaCount();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      novitaMutating--;
    }
  }
  async function ignoreAllNovita(watchId: number) {
    novitaMutating++;
    try {
      await dismissWatchHits(watchId);
      novitaGroups = novitaGroups.filter((g) => g.watch_id !== watchId);
      await refreshNovitaCount();
    } catch (e) {
      status = "Errore: " + e;
    } finally {
      novitaMutating--;
    }
  }
  /** Remove one hit from the in-memory feed, dropping now-empty groups. */
  function dropHitFromFeed(watchId: number, hitId: number) {
    novitaGroups = novitaGroups
      .map((g) => (g.watch_id === watchId ? { ...g, hits: g.hits.filter((h) => h.hit_id !== hitId) } : g))
      .filter((g) => g.hits.length > 0);
  }
  async function sweepNovitaNow() {
    if (novitaSweeping) return;
    novitaSweeping = true;
    status = "Cerco novità negli archivi…";
    try {
      const fresh = await sweepWatchesNow();
      status = fresh > 0 ? `${fresh} nuovi paper trovati` : "Nessuna novità al momento";
      novitaGroups = await listNovita();
      await refreshNovitaCount();
    } catch (e) {
      status = "Errore ricerca novità: " + e;
    } finally {
      novitaSweeping = false;
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
    if (s) openById(s.document_id, s.page ?? null);
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
      status = format === "bibtex" ? "BibTeX copiato negli appunti" : "Citazione copiata negli appunti";
    } catch (e) {
      status = "Errore copia: " + e;
    }
  }

  /** Copy a citation for a whole selection: one \cite{k1,k2,…} or all BibTeX entries. */
  async function copyCiteMulti(ids: number[], format: string) {
    if (!ids.length) return;
    try {
      const text = await citeText(ids, format);
      if (!text.trim()) {
        status = "Niente da citare nella selezione";
        return;
      }
      await navigator.clipboard.writeText(text);
      status =
        format === "bibtex"
          ? `BibTeX copiato (${ids.length} voci)`
          : format === "latex"
            ? `\\cite copiato (${ids.length} chiavi)`
            : `Citazioni copiate (${ids.length})`;
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

  /** Copy the paper's title to the clipboard (quick action from grid/list menus). */
  async function copyTitle(doc: DocumentItem) {
    const t = (doc.title ?? "").trim();
    if (!t) {
      status = "Questo documento non ha un titolo da copiare";
      return;
    }
    try {
      await navigator.clipboard.writeText(t);
      status = "Titolo copiato negli appunti";
    } catch (e) {
      status = "Errore copia: " + e;
    }
  }

  // ----- "Cura della libreria": salute + gap di citazioni + duplicati, a schede -----
  let careModal = $state(false);
  let careTab = $state<"salute" | "gap" | "duplicati">("salute");
  let health = $state<LibraryHealth | null>(null);
  let healthLoading = $state(false);
  /** Open the care surface on a tab, loading its data. */
  function openCare(tab: "salute" | "gap" | "duplicati") {
    careModal = true;
    careTab = tab;
    if (tab === "salute") openHealth();
    else if (tab === "gap") openGaps();
    else loadDuplicates();
  }
  async function openHealth() {
    if (healthLoading) return; // re-entry guard: don't run two scans at once
    careModal = true;
    careTab = "salute";
    healthLoading = true;
    health = null;
    try {
      health = await libraryHealth();
    } catch (e) {
      status = "Errore salute libreria: " + e;
      careModal = false;
    } finally {
      healthLoading = false;
    }
  }
  function openHealthRow(id: number) {
    careModal = false;
    openById(id);
  }
  // OCR a scanned PDF (Windows OCR engine), then refresh the health scan so the
  // now-searchable document drops out of "PDF senza testo".
  let ocrBusy = $state<number | null>(null);
  async function runOcr(id: number) {
    ocrBusy = id;
    status = "OCR in corso… (può richiedere qualche secondo per documenti lunghi)";
    try {
      const res = await ocrDocument(id);
      status = res.truncated
        ? `OCR: ${res.chars.toLocaleString()} caratteri dalle prime ${res.pages} di ${res.total_pages} pagine (limite) ✓`
        : `OCR completato: ${res.chars.toLocaleString()} caratteri da ${res.pages} pagine ✓`;
      await loadDocs();
      if (careModal && careTab === "salute") await openHealth();
    } catch (e) {
      status = "Errore OCR: " + e;
    } finally {
      ocrBusy = null;
    }
  }

  // ----- Citation gap-finder -----
  let gaps = $state<GapItem[]>([]);
  let gapsLoading = $state(false);
  async function openGaps() {
    careModal = true;
    careTab = "gap";
    gapsLoading = true;
    gaps = [];
    try {
      gaps = await citationGaps(60);
    } catch (e) {
      status = "Errore gap citazioni: " + e;
      careModal = false;
    } finally {
      gapsLoading = false;
    }
  }
  async function runResolveRefDois() {
    if (refdoiRunning) return;
    refdoiRunning = true;
    refdoiProg = null;
    try {
      const s = await resolveReferenceDois();
      status =
        s.resolved > 0
          ? `DOI riferimenti: ${s.resolved} risolti su ${s.scanned} citazioni (${s.updated_rows} righe aggiornate) — ${s.remaining} citazioni ancora senza DOI.`
          : `Nessun DOI recuperato su ${s.scanned} citazioni — ${s.remaining} ancora senza DOI.`;
      // Newly-resolved DOIs can surface fresh gaps: refresh the list in place.
      try {
        gaps = await citationGaps(60);
      } catch {
        /* ignore */
      }
    } catch (e) {
      status = "Errore risoluzione DOI: " + e;
    } finally {
      refdoiRunning = false;
      refdoiProg = null;
    }
  }
  function cancelRefDois() {
    cancelReferenceDois().catch(() => {});
  }
  function gapSearchOnline(doi: string) {
    careModal = false;
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

  async function exportLibrary(onlyIds?: number[]) {
    const ids = onlyIds?.length ? onlyIds : displayed.map((d) => d.id);
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
    // Keep the tag flyout in sync with the reloaded document.
    if (tagPanel) {
      const d = docs.find((x) => x.id === docId);
      tagPanel = d ? { ...tagPanel, doc: d } : null;
    }
  }

  // ----- Tag: rinomina / ricolora (matitina nella barra laterale) -----
  let tagEdit = $state<{ id: number; name: string; color: string | null; x: number; y: number } | null>(null);
  async function saveTagEdit() {
    if (!tagEdit || !tagEdit.name.trim()) return;
    try {
      await updateTag(tagEdit.id, tagEdit.name.trim(), tagEdit.color);
      tagEdit = null;
      await loadSidebar();
      await loadDocs(); // i chip sulle card cambiano nome/colore
    } catch (e) {
      status = "" + e;
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
    if (!(await confirmAsk(`Eliminare la collezione «${coll.name}»? I documenti restano in libreria (le eventuali sotto-raccolte risalgono di un livello).`))) return;
    await deleteCollection(coll.id);
    if (filter.kind === "collection" && filter.id === coll.id) filter = { kind: "all" };
    await loadDocs();
    await loadSidebar();
  }

  function openCardMenu(e: MouseEvent, doc: DocumentItem) {
    openRadialDoc(e, doc);
  }
  /** Right-click: open the radial menu at the cursor (suppress the native menu). */
  function onContext(e: MouseEvent, doc: DocumentItem) {
    openRadialDoc(e, doc);
  }
  async function revealDoc(doc: DocumentItem) {
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

  /** Tool-bar icon click: open the group's dropdown, or run it if it's a leaf. */
  function openTool(e: MouseEvent, g: RadialItem) {
    e.stopPropagation();
    if (g.disabled) return; // defensive: a disabled top-level icon does nothing
    sortPop = false;
    indexPop = false;
    if (g.children && g.children.length) {
      if (toolMenu?.id === g.id) {
        toolMenu = null;
        return;
      }
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      toolMenu = { id: g.id, x: r.left, y: r.bottom + 6 };
    } else {
      toolMenu = null;
      g.action?.();
    }
  }

  /** Run a tool-menu entry (ignoring disabled ones) and close the menu. */
  function runTool(it: RadialItem) {
    if (it.disabled) return;
    toolMenu = null;
    it.action?.();
  }

  let dragP: Promise<() => void> | undefined;
  let embP: Promise<() => void> | undefined;
  let watchP: Promise<() => void> | undefined;
  let ragP: Promise<() => void> | undefined;
  let refdoiP: Promise<() => void> | undefined;
  let askP: Promise<() => void> | undefined;
  let connP: Promise<() => void> | undefined;
  let wikiP: Promise<() => void> | undefined;
  let metaP: Promise<() => void> | undefined;
  let clearTimer: ReturnType<typeof setTimeout> | undefined;
  onMount(() => {
    loadDocs();
    loadStatus();
    loadSidebar();
    loadConnector();
    checkClipboard(); // magari l'app è stata aperta subito dopo aver copiato un link
    // Avviso-versione all'avvio: silenzioso, opt-in (solo con la scoperta online
    // attiva) e al più una volta al giorno.
    void (async () => {
      try {
        const last = Number(localStorage.getItem("scriptorium-update-check") || 0);
        if (Date.now() - last < 24 * 3600 * 1000) return;
        const s = await getDiscoverySettings();
        if (!s.enabled) return;
        localStorage.setItem("scriptorium-update-check", String(Date.now()));
        await checkUpdatesNow(false);
      } catch {
        /* offline o impostazioni non leggibili: nessun avviso */
      }
    })();
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
        // While a note editor is open, embed dropped images into the note (Tauri's
        // native drag-drop intercepts OS file drops, so the HTML5 ondrop never sees
        // them — this is the real image-drop path). PDFs still go to import.
        if (noteEditorActive()) {
          const images = p.paths.filter((x) => /\.(png|jpe?g|gif|webp|bmp|svg|avif)$/i.test(x));
          if (images.length) void insertNoteImagePaths(images);
          const pdfs = p.paths.filter((x) => x.toLowerCase().endsWith(".pdf"));
          if (pdfs.length) handleImport(pdfs);
          return;
        }
        const pdfs = p.paths.filter((x) => x.toLowerCase().endsWith(".pdf"));
        handleImport(pdfs);
      } else dragOver = false;
    });
    embP = listen<EmbedProgress>("embed-progress", (e) => {
      embedProgress = e.payload;
      if (e.payload.phase === "done" || e.payload.phase === "cancelled") {
        loadStatus();
        // New embeddings change the constellation: rebuild it (or drop the cache).
        graph = null;
        graphError = false;
        if (view === "map") loadGraph(true);
        clearTimeout(clearTimer);
        clearTimer = setTimeout(() => (embedProgress = null), 1800);
      }
    });
    watchP = listen("library-changed", () => {
      loadDocs();
      loadStatus();
      graph = null; // imported/removed docs invalidate the semantic map
      graphError = false;
      if (view === "map") loadGraph(true);
    });
    // A PDF grabbed from the browser via the bookmarklet connector.
    connP = listen<string>("connector-added", (e) => {
      const s = e.payload;
      status =
        s === "added"
          ? "PDF agganciato dal browser ✓"
          : s === "duplicate"
            ? "Dal browser: già presente in libreria"
            : s === "not_pdf"
              ? "Dal browser: il link non è un PDF diretto"
              : "Dal browser: errore nel download";
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
    refdoiP = listen<{ done: number; total: number; resolved: number }>("refdoi-progress", (e) => {
      if (refdoiRunning) refdoiProg = e.payload;
    });
    wikiP = listen<{ phase: string; done: number; total: number; concept: string }>("wiki-progress", (e) => {
      wikiProg = e.payload.phase === "done" ? null : e.payload;
    });
    metaP = listen<MetaRecoverProgress>("meta-progress", (e) => {
      metaScan = e.payload.phase === "running" ? e.payload : null;
    });
    // "Novità": the on-launch sweep finished — refresh the badge (and the feed if open).
    novitaP = listen<number>("novita-changed", () => {
      refreshNovitaCount();
      // Don't reload the feed mid accept/ignore or we'd revert the optimistic removal.
      if (filter.kind === "novita" && novitaMutating === 0)
        listNovita().then((g) => (novitaGroups = g)).catch(() => {});
    });
    refreshNovitaCount();
    return () => {
      dragP?.then((f) => f());
      embP?.then((f) => f());
      watchP?.then((f) => f());
      ragP?.then((f) => f());
      refdoiP?.then((f) => f());
      askP?.then((f) => f());
      connP?.then((f) => f());
      wikiP?.then((f) => f());
      metaP?.then((f) => f());
      novitaP?.then((f) => f());
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
  /** Reading progress as a 0–100 percentage, or null when there's nothing to show
   *  (document never opened and not marked read, or page count unknown). */
  function readPct(d: DocumentItem): number | null {
    if (d.is_read) return 100;
    if (!d.page_count || !d.last_page || d.last_page <= 0) return null;
    return Math.min(100, Math.round((d.last_page / d.page_count) * 100));
  }
  /** The paper's original link for sharing (DOI, else arXiv/landing), if known. */
  function paperLink(d: DocumentItem | null | undefined): string | undefined {
    return d?.paper_url ?? (d?.doi ? `https://doi.org/${d.doi}` : undefined);
  }
  /** Show all papers by an author (the clickable author chips/lines). */
  function showAuthor(name: string | undefined) {
    if (name && name.trim()) setFilter({ kind: "author", label: name.trim() });
  }

  // ===================== Orbita: radial menu + command palette =====================
  // A single action layer feeds both surfaces, so every feature stays reachable
  // even though the chrome shows almost no buttons.

  /** Stroke-icon paths (24×24, feather-style) for radial petals. */
  const I = {
    pulse: "M5.64 18.36A9 9 0 1 1 18.36 18.36M12 12l3.5-3.5",
    open: "M2 4h6a4 4 0 0 1 4 4v12a3 3 0 0 0-3-3H2zM22 4h-6a4 4 0 0 0-4 4v12a3 3 0 0 1 3-3h7z",
    star: "M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01z",
    check: "M22 11.08V12a10 10 0 1 1-5.93-9.14M22 4L12 14.01l-3-3",
    quote: "M10 11H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v8a4 4 0 0 1-4 4M20 11h-4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v8a4 4 0 0 1-4 4",
    ai: "M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9zM19 15l.9 2.1L22 18l-2.1.9L19 21l-.9-2.1L16 18l2.1-.9z",
    folder: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z",
    share: "M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8M16 6l-4-4-4 4M12 2v13",
    trash: "M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M10 11v6M14 11v6",
    code: "M16 18l6-6-6-6M8 6l-6 6 6 6",
    edit: "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7M18.5 2.5a2.12 2.12 0 0 1 3 3L12 15l-4 1 1-4z",
    imp: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3",
    exp: "M15 3h6v6M10 14L21 3M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5",
    grid: "M3 3h7v7H3zM14 3h7v7h-7zM14 14h7v7h-7zM3 14h7v7H3z",
    list: "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01",
    map: "M12 4a2 2 0 1 0 .01 0M5 16a2 2 0 1 0 .01 0M19 16a2 2 0 1 0 .01 0M10.8 9.6L6.6 14.6M13.2 9.6l4.2 5M7 18h10",
    search: "M11 3a8 8 0 1 0 .01 0M21 21l-4.35-4.35",
    eye: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8zM12 9a3 3 0 1 0 .01 0",
    theme: "M12 2a10 10 0 0 0 0 20 2 2 0 0 0 2-2v-1a2 2 0 0 1 2-2h1a5 5 0 0 0 5-5A10 10 0 0 0 12 2zM7 10h.01M12 6h.01M17 10h.01",
    tools: "M14.7 6.3a4.5 4.5 0 0 0-6.4 5.6L3 17.2V21h3.8l5.3-5.3a4.5 4.5 0 0 0 5.6-6.4l-2.9 2.9-2.1-2.1z",
    compass: "M12 2a10 10 0 1 0 .01 0M16 8l-2.5 5.5L8 16l2.5-5.5z",
    gear: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z",
    tag: "M20.6 13.4l-8-8A2 2 0 0 0 11.2 5H5a2 2 0 0 0-2 2v6.2a2 2 0 0 0 .6 1.4l8 8a2 2 0 0 0 2.8 0l6.2-6.2a2 2 0 0 0 0-2.8zM7.5 9.5h.01",
    print: "M6 9V2h12v7M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2M6 14h12v8H6z",
    reveal: "M3 7a2 2 0 0 1 2-2h4l2 3h8a2 2 0 0 1 2 2v1H5zM3 7v11a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7",
    copy: "M9 9h11v11H9zM5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1",
    ask: "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3M12 17h.01M12 2a10 10 0 1 0 .01 0",
    near: "M12 8a4 4 0 1 0 .01 0M12 2v2M12 20v2M2 12h2M20 12h2",
    heal: "M22 12h-4l-3 9L9 3l-3 9H2",
    bell: "M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9M13.7 21a2 2 0 0 1-3.4 0",
    note: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6M16 13H8M16 17H8M10 9H8",
    globe: "M12 2a10 10 0 1 0 .01 0M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10",
    term: "M4 17l6-6-6-6M12 19h8",
    backup: "M22 12H2M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11zM6 16h.01M10 16h.01",
    layers: "M12 2l9 5-9 5-9-5zM3 12l9 5 9-5M3 17l9 5 9-5",
    bookmark: "M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z",
    // lente con "+": cerca e AGGIUNGE i metadati (distinta dalla lente semplice)
    metafind: "M11 3a8 8 0 1 0 .01 0M21 21l-4.35-4.35M11 8v6M8 11h6",
    // salvagente (life-buoy): inconfondibile col «?» di Chiedi alla libreria
    help: "M12 2a10 10 0 1 0 .01 0M12 8a4 4 0 1 0 .01 0M4.93 4.93l4.24 4.24M14.83 14.83l4.24 4.24M14.83 9.17l4.24-4.24M4.93 19.07l4.24-4.24",
    x: "M18 6L6 18M6 6l12 12",
  };

  /** Open a document in the viewer, optionally at a specific page. Reference-only
   *  entries have no file: offer to attach one instead of a failing viewer. */
  function openDocument(d: DocumentItem, page: number | null = null) {
    if (!d.has_file) {
      refPanel = { doc: d, url: "", busy: false };
      return;
    }
    lastReadDoc = d; // remember it for "Riprendi lettura"
    openDocPage = page;
    openDoc = d;
  }

  /** Reopen the last-read PDF at the page where we left off (the reader restores the
   *  stored last page when opened with no explicit page). The cached lastReadDoc snapshot
   *  may be stale (its target since trashed/purged/merged), so re-validate against the
   *  fresh recent list before opening — and fall back to the next most recent PDF (or a
   *  notice) instead of mounting the reader on a dead id. */
  async function resumeLastRead() {
    const wantId = lastReadDoc?.id ?? null;
    let d: DocumentItem | null = null;
    try {
      const recent = await recentDocuments(8);
      d =
        (wantId != null ? recent.find((x) => x.id === wantId && x.has_file) : undefined) ??
        recent.find((x) => x.has_file) ??
        null;
    } catch {
      d = lastReadDoc?.has_file ? lastReadDoc : null; // DB unreachable: best effort
    }
    if (d) {
      lastReadDoc = d;
      openDocument(d);
    } else {
      lastReadDoc = null;
      status = "Nessun PDF letto di recente.";
    }
  }

  // ---- "Riferimento senza PDF": attach a file to an existing entry ----
  let refPanel = $state<{ doc: DocumentItem; url: string; busy: boolean } | null>(null);

  /** "Trova PDF" from the reference panel: open the candidates dialog. */
  function refFindPdf() {
    if (!refPanel) return;
    pdfFindId = refPanel.doc.id;
  }

  async function refAttach() {
    if (!refPanel || refPanel.busy) return;
    const u = refPanel.url.trim();
    if (!u) return;
    refPanel.busy = true;
    try {
      const r = await attachFromUrl(refPanel.doc.id, u);
      status =
        r === "attached"
          ? "PDF allegato al riferimento ✓"
          : r === "already"
            ? "Questo documento ha già un PDF"
            : r === "duplicate"
              ? "Quel PDF è già in libreria (in un altro documento): usa Strumenti → Duplicati per unirli"
              : "Quel link non è un PDF diretto";
      if (r === "attached") {
        await loadDocs();
        await loadSidebar();
        const fresh = docs.find((x) => x.id === refPanel!.doc.id);
        refPanel = null;
        if (fresh) openDocument(fresh);
        return;
      }
    } catch (e) {
      status = "Errore: " + e;
    }
    if (refPanel) refPanel.busy = false;
  }

  async function refPaste() {
    if (!refPanel) return;
    try {
      const t = await navigator.clipboard.readText();
      if (t && t.trim()) refPanel.url = t.trim();
    } catch {
      status = "Non riesco a leggere gli appunti: incolla con Ctrl+V";
    }
  }

  async function shareDoc(target: ShareTarget, ids: number[], label: string, link?: string | null) {
    const r = await shareTo(target, ids, label, link);
    status = r.note;
  }

  /** Copy the on-disk PDF path (reference-only entries have none). */
  async function copyPath(doc: DocumentItem) {
    try {
      const p = await documentPath(doc.id);
      if (!p) {
        status = "Questo riferimento non ha un file PDF";
        return;
      }
      await navigator.clipboard.writeText(p);
      status = "Percorso copiato negli appunti";
    } catch (e) {
      status = "Errore: " + e;
    }
  }

  /** Remove the document from the collection currently being viewed. */
  async function removeDocFromCurrentCollection(doc: DocumentItem) {
    if (filter.kind !== "collection" || filter.id == null) return;
    try {
      await removeFromCollection(filter.id, doc.id);
      status = `Rimosso da «${filter.label}»`;
      await loadDocs();
    } catch (e) {
      status = "Errore: " + e;
    }
  }

  const THEMES: { value: string; label: string; dark: boolean }[] = [
    { value: "paper", label: "Carta", dark: false },
    { value: "sepia", label: "Seppia", dark: false },
    { value: "solarized", label: "Solarized", dark: false },
    { value: "sage", label: "Salvia", dark: false },
    { value: "pastel", label: "Pastello", dark: false },
    { value: "medieval", label: "Medievale", dark: false },
    { value: "dark", label: "Scuro", dark: true },
    { value: "nord", label: "Nord", dark: true },
    { value: "graphite", label: "Grafite", dark: true },
    { value: "forest", label: "Foresta", dark: true },
    { value: "synthwave", label: "Synthwave", dark: true },
  ];

  /** Doc radial: everything the old flat context menu offered, organized in orbits. */
  function buildDocRadial(d: DocumentItem): RadialItem[] {
    const shareKids: RadialItem[] = [
      { id: "sh-wa", label: "WhatsApp", hint: "Copia il PDF e apre WhatsApp: incolla con Ctrl+V", action: () => shareDoc("whatsapp", [d.id], d.title ?? "Documento PDF", paperLink(d)) },
      { id: "sh-tm", label: "Teams", hint: "Copia il PDF e apre Teams", action: () => shareDoc("teams", [d.id], d.title ?? "Documento PDF", paperLink(d)) },
      { id: "sh-gm", label: "Gmail", hint: "Copia il PDF e apre Gmail", action: () => shareDoc("gmail", [d.id], d.title ?? "Documento PDF", paperLink(d)) },
      { id: "sh-ol", label: "Outlook", hint: "Outlook desktop: PDF allegato direttamente", action: () => shareDoc("outlook", [d.id], d.title ?? "Documento PDF", paperLink(d)) },
      { id: "sh-pr", label: "Stampa", icon: I.print, disabled: printing, action: () => printOne(d) },
      { id: "sh-rv", label: "Mostra nella cartella", icon: I.reveal, action: () => revealDoc(d) },
      { id: "sh-cp", label: "Copia percorso", icon: I.copy, hint: "Copia il percorso del file PDF", action: () => copyPath(d) },
    ];
    const citeKids: RadialItem[] = [
      { id: "ci-apa", label: "Copia APA", action: () => copyCite(d, "apa") },
      { id: "ci-ieee", label: "Copia IEEE", action: () => copyCite(d, "ieee") },
      { id: "ci-bib", label: "Copia BibTeX", action: () => copyCite(d, "bibtex") },
      { id: "ci-key", label: "Copia citekey", hint: d.citekey ?? undefined, action: () => copyCite(d, "citekey") },
      { id: "ci-tex", label: "Copia \\cite{…}", hint: "Pronto per LaTeX", action: () => copyCite(d, "latex") },
      { id: "ci-pan", label: "Copia [@…]", hint: "Pronto per Pandoc/Quarto", action: () => copyCite(d, "pandoc") },
      { id: "ci-ref", label: "Riferimenti e citazioni", hint: "Bibliografia del paper e chi lo cita nella tua libreria", action: () => openCitations(d) },
    ];
    citeKids.push({
      id: "ci-exp",
      label: "Esplora citazioni (online)",
      hint: d.doi
        ? "Snowball su OpenAlex: citazioni da e verso questo paper, aggiungile alla libreria"
        : "Snowball su OpenAlex — senza DOI il paper si aggancia per titolo (corrispondenza rigorosa)",
      disabled: !d.doi && !(d.title ?? "").trim(),
      action: () => openExplore(d),
    });
    const aiKids: RadialItem[] = [
      {
        id: "ai-sum",
        label: d.has_summary ? "Riassunto ✓ (rigenera)" : "Riassumi",
        checked: d.has_summary,
        hint: d.has_summary ? "Già presente: cliccando lo rigeneri e sovrascrivi" : "Riassunto in italiano con l'AI locale",
        disabled: !aiStat?.enabled || aiBusyAny,
        action: () => summarizeDoc(d),
      },
      {
        id: "ai-tag",
        label: d.tags.length ? "Tag automatici (ha già tag)" : "Tag automatici",
        checked: d.tags.length > 0,
        hint: d.tags.length ? `Ha già ${d.tags.length} tag: la chiamata può aggiungerne altri` : "Suggerisce e assegna tag tematici",
        disabled: !aiStat?.enabled || aiBusyAny,
        action: () => autotagDoc(d),
      },
      { id: "ai-ask", label: "Chiedi al documento", hint: "Domande in linguaggio naturale su questo PDF", disabled: !aiStat?.enabled, action: () => askAboutDoc(d) },
      { id: "ai-rel", label: "Correlati", icon: I.near, hint: "I documenti più vicini per significato (indice semantico)", action: () => setFilter({ kind: "related", id: d.id, label: d.title ?? "documento" }) },
      { id: "ai-path", label: "Percorso di lettura", icon: I.map, hint: "Cosa leggere prima per capirlo: fondamenti citati + vicini precedenti (senza LLM)", action: () => openReadingPath(d) },
    ];
    const orgKids: RadialItem[] = [
      { id: "or-find", label: "Recupera metadati…", icon: I.metafind, hint: "Cerca online la scheda giusta (Crossref, arXiv, OpenAlex) e scegli tu quale applicare", action: () => (metaFindId = d.id) },
      { id: "or-meta", label: "Modifica metadati", icon: I.edit, action: () => (editingId = d.id) },
      { id: "or-tag", label: "Tag…", icon: I.tag, hint: "Assegna o togli tag", action: () => (tagPanel = { doc: d, x: radial?.x ?? 300, y: radial?.y ?? 200 }) },
      { id: "or-coll", label: "Collezioni…", icon: I.folder, hint: "Aggiungi a una collezione", action: () => (collPanel = { doc: d, x: radial?.x ?? 300, y: radial?.y ?? 200 }) },
    ];
    if (filter.kind === "collection")
      orgKids.push({ id: "or-rm", label: `Togli da «${filter.label ?? "collezione"}»`, danger: true, action: () => removeDocFromCurrentCollection(d) });
    if (!d.has_file)
      orgKids.push({ id: "or-pdf", label: "Allega PDF…", hint: "Questa voce è solo un riferimento: trova un PDF Open Access o allegane uno da un link", action: () => (refPanel = { doc: d, url: "", busy: false }) });
    const items: RadialItem[] = [
      { id: "d-open", label: "Apri", icon: I.open, hint: "Leggi nel visore integrato", action: () => openDocument(d) },
      { id: "d-copyt", label: "Copia titolo", icon: I.copy, hint: "Copia il titolo del paper negli appunti", disabled: !(d.title ?? "").trim(), action: () => copyTitle(d) },
      { id: "d-fav", label: "Preferito", icon: I.star, checked: d.favorite, hint: d.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti", action: () => toggleFavorite(d) },
      { id: "d-read", label: "Letto", icon: I.check, checked: d.is_read, hint: d.is_read ? "Segna come da leggere" : "Segna come letto", action: () => toggleRead(d) },
      { id: "d-cite", label: "Cita", icon: I.quote, hint: "Copia citazioni, riferimenti, esplora", children: citeKids },
      { id: "d-ai", label: "AI", icon: I.ai, hint: aiStat?.enabled ? "Riassunto, tag, domande, correlati" : "Correlati (per il resto attiva l'AI locale)", children: aiKids },
      { id: "d-org", label: "Organizza", icon: I.folder, hint: "Metadati, tag, collezioni", children: orgKids },
      { id: "d-code", label: "Codice & repo", icon: I.code, hint: "Repository GitHub citati + modelli e dataset Hugging Face", action: () => openHf(d) },
      { id: "d-share", label: "Condividi", icon: I.share, hint: "Invia, stampa, mostra file", children: shareKids },
      { id: "d-del", label: "Elimina", icon: I.trash, danger: true, hint: "Sposta nel cestino (recuperabile)", action: () => trashSelected([d.id]) },
    ];
    // Reference-only entry: finding its PDF is THE next step — a first-class petal.
    if (!d.has_file)
      items.splice(1, 0, {
        id: "d-findpdf",
        label: "Trova PDF…",
        icon: I.imp,
        hint: "Mostra i candidati trovati online (arXiv, OpenAlex, Semantic Scholar, Crossref, per identificativo e per titolo): scegli tu quale scaricare e allegare",
        disabled: !!pdfBatch,
        action: () => (pdfFindId = d.id),
      });
    return items;
  }

  /** Selection radial: batch actions on the current multi-selection. */
  function buildSelectionRadial(): RadialItem[] {
    const ids = [...selected];
    const one = ids.length === 1 ? displayed.find((d) => d.id === ids[0]) : undefined;
    const label = one?.title ?? `${ids.length} documenti PDF`;
    const link = one ? paperLink(one) : null;
    const shareKids: RadialItem[] = (["whatsapp", "teams", "gmail", "outlook"] as ShareTarget[]).map((t) => ({
      id: "ssh-" + t,
      label: t === "whatsapp" ? "WhatsApp" : t === "teams" ? "Teams" : t === "gmail" ? "Gmail" : "Outlook",
      action: () => shareDoc(t, ids, label, link),
    }));
    const tagKids: RadialItem[] = tags.map((t) => ({ id: "st-" + t.id, label: t.name, action: () => bulkAddTag(t) }));
    const collKids: RadialItem[] = collections
      .filter((c) => !c.is_smart)
      .map((c) => ({ id: "sc-" + c.id, label: c.name, action: () => bulkAddCollection(c) }));
    const items: RadialItem[] = [
      { id: "s-print", label: "Stampa", icon: I.print, disabled: printing, hint: "Un unico lavoro di stampa", action: () => printSelected() },
      { id: "s-share", label: "Condividi", icon: I.share, children: shareKids },
    ];
    // Batch OA-PDF finder over the selected reference-only entries.
    const noPdf = ids
      .map((x) => displayed.find((d) => d.id === x))
      .filter((d): d is DocumentItem => !!d && !d.has_file);
    if (noPdf.length)
      items.push({
        id: "s-findpdf",
        label: `Trova PDF (${noPdf.length} riferiment${noPdf.length === 1 ? "o" : "i"})`,
        icon: I.imp,
        hint: "Cerca e allega la copia Open Access per le voci selezionate senza file (arXiv, Unpaywall, OpenAlex, Semantic Scholar)",
        disabled: !!pdfBatch,
        action: () => batchFindPdf(noPdf),
      });
    // Gate on the LIVE saved state (aiStat), not the Settings-form variable: the
    // form state can lag what's persisted and silently hide these items.
    if (aiStat?.enabled) {
      items.push({ id: "s-sum", label: "Riassumi (AI)", icon: I.ai, disabled: aiBusyAny, hint: "Un riassunto per ogni selezionato", action: () => runBatchAi("summary") });
      items.push({ id: "s-tags", label: "Tag automatici (AI)", icon: I.tag, disabled: aiBusyAny, action: () => runBatchAi("tags") });
      if (ids.length >= 2 && ids.length <= 3)
        items.push({ id: "s-cmp", label: "Confronta (AI)", icon: I.near, disabled: aiBusyAny || wikiBusy, hint: "Tabella: obiettivo, metodo, dati, risultati, limiti — e cosa aggiunge ciascuno", action: () => runCompare() });
      if (ids.length >= 2)
        items.push({ id: "s-rev", label: "Rassegna (AI)", icon: I.quote, disabled: aiBusyAny || wikiBusy, hint: "Mini related-work per temi (2-10 paper); salvabile come appunto con backlink [[@citekey]]", action: () => runReview() });
      items.push({ id: "s-res", label: "Tabella risultati (AI)", icon: I.grid, disabled: aiBusyAny || wikiBusy, hint: "Raccogli metriche e numeri dei paper in un'unica tabella (CSV/Excel)", action: () => runHarvest() });
      items.push({ id: "s-wiki", label: "Pagina wiki (AI)", icon: I.open, disabled: aiBusyAny || wikiBusy, hint: "Una pagina della Wiki con esattamente questi documenti come fonti (max 10)", action: () => (wikiFromSel = { ids: [...selected].slice(0, 10), concept: "" }) });
    }
    if (tagKids.length) items.push({ id: "s-tag", label: "Aggiungi tag", icon: I.tag, children: tagKids });
    if (collKids.length) items.push({ id: "s-coll", label: "In collezione", icon: I.folder, children: collKids });
    items.push({
      id: "s-cite",
      label: "Cita",
      icon: I.quote,
      hint: "Copia le citazioni della selezione per LaTeX/Pandoc",
      children: [
        { id: "sci-tex", label: "Copia \\cite{…}", hint: "Un solo \\cite con tutte le chiavi", action: () => copyCiteMulti(ids, "latex") },
        { id: "sci-bib", label: "Copia BibTeX", hint: "Le voci .bib di tutti i selezionati", action: () => copyCiteMulti(ids, "bibtex") },
        { id: "sci-pan", label: "Copia [@…] (Pandoc)", action: () => copyCiteMulti(ids, "pandoc") },
        { id: "sci-key", label: "Copia citekey", action: () => copyCiteMulti(ids, "citekey") },
      ],
    });
    items.push({ id: "s-exp", label: "Esporta citazioni", icon: I.exp, hint: "Salva un file .bib/.ris/.json", action: () => exportLibrary(ids) });
    items.push({ id: "s-none", label: "Deseleziona", icon: I.x, action: () => (selected = []) });
    items.push({ id: "s-del", label: "Elimina", icon: I.trash, danger: true, action: () => trashSelected(ids) });
    return items;
  }

  /** Global radial: the whole app, one flick away. */
  function buildGlobalRadial(): RadialItem[] {
    return [
      {
        id: "g-home",
        label: "I miei paper",
        icon: I.grid,
        hint: "Torna alla griglia dei paper (tutta la libreria)",
        action: () => {
          setFilter({ kind: "all" });
          view = "grid";
        },
      },
      {
        id: "g-imp",
        label: "Importa",
        icon: I.imp,
        hint: "PDF, BibTeX, identificatori, URL",
        children: [
          { id: "gi-pdf", label: "PDF dal disco…", action: () => importViaDialog() },
          { id: "gi-bib", label: "Da gestore bibliografico…", hint: "Zotero, Mendeley, EndNote… (.bib/.ris/CSL-JSON) + PDF + tag", action: () => importRefManagerDialog() },
          { id: "gi-tex", label: "Progetto LaTeX (.zip)…", hint: "I tuoi paper: PDF + bibliografia", action: () => importLatexDialog() },
          { id: "gi-id", label: "Per identificatore…", hint: "DOI / arXiv / ISBN / PMID", action: () => (idModal = true) },
          { id: "gi-url", label: "Da URL…", hint: "Scarica un PDF da un link", action: () => openUrlModal() },
          { id: "gi-watch", label: "Cartella sorvegliata…", hint: "Importa automaticamente i PDF che aggiungi", action: () => pickWatchedFolder() },
        ],
      },
      {
        id: "g-view",
        label: "Vista",
        icon: I.eye,
        hint: "Griglia, lista, costellazione, ordinamento",
        children: [
          { id: "gv-grid", label: "Griglia", icon: I.grid, checked: view === "grid", action: () => (view = "grid") },
          { id: "gv-list", label: "Lista", icon: I.list, checked: view === "list", action: () => (view = "list") },
          { id: "gv-map", label: "Costellazione", icon: I.map, checked: view === "map", hint: "Mappa semantica della libreria", action: () => (view = "map") },
          { id: "gv-side", label: "Barra laterale", checked: !sidebarHidden, hint: "Mostra/nascondi (Ctrl+B)", action: () => (sidebarHidden = !sidebarHidden) },
          ...SORT_KEYS.map((k) => ({
            id: "gs-" + k,
            label: "Ordina: " + SORT_LABELS[k],
            checked: !!sortDirOf(k),
            badge: sortDirOf(k) ? (sortDirOf(k) === "asc" ? "↑" : "↓") : undefined,
            hint: "Un tocco attiva, un altro inverte, un terzo toglie",
            action: () => cycleSort(k),
          })),
        ],
      },
      {
        id: "g-resume",
        label: "Riprendi lettura",
        icon: I.bookmark,
        hint: "Torna all'ultimo PDF, al punto in cui eri",
        disabled: !lastReadDoc,
        action: () => resumeLastRead(),
      },
      { id: "g-ask", label: "Chiedi alla libreria", icon: I.ask, hint: "Risposte con citazioni dai tuoi PDF (AI locale)", action: () => { setFilter({ kind: "ask" }); loadRagStatus(); } },
      { id: "g-wiki", label: "Wiki della libreria", icon: I.open, hint: "La tua enciclopedia privata, generata dai tuoi paper", action: () => openWikiView() },
      { id: "g-disc", label: "Cerca online", icon: I.globe, hint: "arXiv, OpenAlex, ADS e altre fonti", action: () => setFilter({ kind: "discover" }) },
      {
        id: "g-notes",
        label: "Appunti",
        icon: I.note,
        hint: "I tuoi appunti in Markdown (file .md) con [[collegamenti]]",
        action: () => openNotesView(),
      },
      {
        id: "g-projects",
        label: "Progetti (LaTeX)",
        icon: I.code,
        hint: "Scrivi in LaTeX con citazioni dalla libreria; compila con Tectonic/latexmk",
        action: () => setFilter({ kind: "projects" }),
      },
      {
        id: "g-archivio",
        label: "Archivio",
        icon: I.folder,
        hint: "Raccolte e sotto-raccolte in vista sinottica: organizza i paper trascinandoli",
        action: () => setFilter({ kind: "archivio" }),
      },
      { id: "g-redis", label: "Riscopri", icon: I.compass, hint: "Un documento dimenticato, pescato per te", action: () => rediscover() },
      {
        id: "g-novita",
        label: "Novità",
        icon: I.bell,
        hint: "Nuovi paper sui temi che segui (ricerche salvate), raccolti a ogni avvio",
        badge: novitaN > 0 ? (novitaN > 99 ? "99+" : String(novitaN)) : undefined,
        action: () => openNovita(),
      },
      {
        id: "g-exp",
        label: "Esporta",
        icon: I.exp,
        children: [
          { id: "ge-cit", label: "Citazioni (BibTeX/RIS/CSL)…", disabled: displayed.length === 0, action: () => exportLibrary() },
          { id: "ge-obs", label: "In Obsidian (Markdown)", disabled: exportingObsidian || displayed.length === 0, action: () => runObsidianExport() },
        ],
      },
      {
        id: "g-tools",
        label: "Cura della libreria",
        icon: I.heal,
        hint: "Salute, gap di citazioni, duplicati e manutenzione",
        children: [
          { id: "gc-health", label: "Salute libreria", hint: "File mancanti, PDF senza testo, metadati incompleti…", action: () => openCare("salute") },
          { id: "gc-meta", label: "Recupera metadati mancanti", hint: needsMeta ? `${needsMeta} documenti incompleti — arXiv dal nome file, DOI e titolo dal PDF` : "Nessun documento incompleto al momento", disabled: enriching || needsMeta === 0, action: () => enrichMeta() },
          { id: "gc-findpdf", label: "Trova PDF dei riferimenti", hint: "Cerca copie Open Access (arXiv, Unpaywall, OpenAlex, Semantic Scholar) per TUTTE le voci senza file e le allega", disabled: !!pdfBatch, action: () => findPdfAllRefs() },
          { id: "gc-gaps", label: "Gap di citazioni", hint: "I DOI più citati dai tuoi paper che ancora non possiedi", action: () => openCare("gap") },
          { id: "gc-dup", label: "Duplicati", hint: "Trova e unisci le copie dello stesso lavoro", action: () => openCare("duplicati") },
          { id: "gt-thumb", label: "Rigenera anteprime", hint: "Ricrea le copertine dal PDF ad alta risoluzione", disabled: rebuildingThumbs, action: () => rebuildThumbs() },
        ],
      },
      { id: "g-emb", label: "Indice semantico", icon: I.layers, hint: `${emb.embedded}/${emb.total} indicizzati — abilita ricerca per significato, Correlati e Costellazione`, disabled: generating || emb.embedded >= emb.total || emb.total === 0, action: () => generateIndex() },
      ...(aiStat?.enabled
        ? [{
            id: "g-aimem",
            label: "Memoria AI",
            icon: I.ai,
            hint: "Libera la GPU (scarica i modelli) o ferma del tutto l'AI locale",
            children: [
              { id: "am-free", label: "Libera GPU — scarica i modelli", hint: "Scarica i modelli dalla VRAM; il server e l'AI restano attivi (si ricaricano al bisogno)", disabled: !aiStat?.reachable, action: () => freeGpuMemory() },
              { id: "am-stop", label: "Ferma AI — server e spegni", hint: "Scarica i modelli, ferma il server locale e disattiva l'AI", action: () => stopAiFully() },
            ],
          }]
        : [{
            id: "g-aion",
            label: "Attiva AI",
            icon: I.ai,
            hint: "Riaccendi l'AI locale (riassunti, tag automatici, domande, wiki…)",
            action: () => quickEnableAi(),
          }]),
      { id: "g-backup", label: "Backup libreria", icon: I.backup, hint: "Copia completa della libreria (PDF + database) in una cartella", action: () => doBackup() },
      { id: "g-trash", label: "Cestino", icon: I.trash, hint: "I documenti eliminati (ripristinabili)", action: () => setFilter({ kind: "trash" }) },
      { id: "g-term", label: "Terminale", icon: I.term, hint: "PowerShell integrato nella cartella dei PDF", action: () => { terminalOpened = true; setFilter({ kind: "terminal" }); } },
      { id: "g-plancia", label: "Plancia", icon: I.pulse, hint: "La sala macchine: cosa sta lavorando adesso, in tempo reale (finestra separata)", action: () => void openPlancia() },
      {
        id: "g-help",
        label: "Guida",
        icon: I.help,
        hint: "La guida di Scriptorium: si sposta e resta aperta (anche in primo piano) mentre lavori",
        action: () => openHelp(),
      },
      {
        id: "g-theme",
        label: "Aspetto",
        icon: I.theme,
        hint: "11 temi, chiari e scuri",
        children: THEMES.map((t) => ({
          id: "th-" + t.value,
          label: t.label,
          badge: t.dark ? "●" : "○",
          checked: theme === t.value,
          action: () => (theme = t.value),
        })),
      },
      {
        id: "g-sys",
        label: "Sistema",
        icon: I.gear,
        children: [
          { id: "gy-set", label: "Impostazioni", icon: I.gear, action: () => openSettings() },
          { id: "gy-upd", label: "Controlla aggiornamenti", hint: "Confronta la tua versione con GitHub — solo un avviso, niente installazioni automatiche", action: () => void checkUpdatesNow(true) },
          { id: "gy-about", label: "Informazioni", action: () => (aboutModal = true) },
        ],
      },
    ];
  }

  function openRadialDoc(e: MouseEvent, d: DocumentItem) {
    e.preventDefault();
    e.stopPropagation();
    ensureThumbs([d]);
    if (selected.length > 1 && selected.includes(d.id)) {
      radial = { x: e.clientX, y: e.clientY, items: buildSelectionRadial(), title: `${selected.length} selezionati`, subtitle: "Azioni sulla selezione", thumb: thumbs[d.id] ?? null };
    } else {
      radial = { x: e.clientX, y: e.clientY, items: buildDocRadial(d), title: d.title ?? "Senza titolo", subtitle: authorLine(d) || undefined, thumb: thumbs[d.id] ?? null };
    }
  }

  /** Right-click on empty chrome → the global radial. Native menu stays available
   *  inside text fields and the terminal. */
  function onGlobalContext(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, [contenteditable], .termview, .xterm, .card, .list tbody tr, .shelfcard, .modalback, .back, .menu, .pop, .floatpill, .toast")) return;
    if (openDoc || radial || paletteOpen || editingId !== null) return;
    e.preventDefault();
    radial = { x: e.clientX, y: e.clientY, items: buildGlobalRadial(), title: "Scriptorium", subtitle: "Menu rapido", thumb: null };
  }

  // ----- Command palette entries: actions + navigation + themes + documents -----
  // ----- Palette: contenuti raggiungibili per nome (oltre ai documenti) -----
  // I progetti LaTeX vivono nel componente TexProjects; per la palette teniamo
  // una copia leggera dell'elenco, aggiornata a ogni apertura della palette.
  let palProjects = $state<ProjectMeta[]>([]);
  // Slug che TexProjects deve aprire quando viene montato/quando cambia
  // (impostato dalla palette: «Progetto LaTeX: …»).
  let projectsOpenSlug = $state<string | null>(null);
  $effect(() => {
    if (!paletteOpen) return;
    // Richieste locali economiche: ad ogni apertura la palette vede appunti,
    // pagine wiki e progetti freschi anche se le loro viste non sono mai state aperte.
    void (async () => {
      try {
        notesList = await listNotes();
      } catch {
        /* vault non leggibile: la palette resta senza appunti */
      }
      try {
        wikiPages = await wikiList();
      } catch {
        /* ignora */
      }
      try {
        palProjects = await listProjects();
      } catch {
        /* ignora */
      }
    })();
  });

  function paletteEntries(): PaletteEntry[] {
    const out: PaletteEntry[] = [];
    // Le voci di navigazione chiudono il lettore se aperto (la palette ora
    // funziona anche lì): senza questo, la vista cambierebbe dietro il PDF.
    const leaveReader = () => {
      openDoc = null;
      openDocPage = null;
    };
    const walk = (items: RadialItem[], trail: string) => {
      for (const it of items) {
        if (it.disabled) continue;
        if (it.children) walk(it.children, trail ? `${trail} · ${it.label}` : it.label);
        else if (it.action)
          out.push({
            id: "act-" + it.id,
            title: trail ? `${trail} · ${it.label}` : it.label,
            hint: it.hint,
            section: "Azioni",
            keywords: it.hint,
            run: it.action,
          });
      }
    };
    walk(buildGlobalRadial(), "");
    if (selected.length > 1) walk(buildSelectionRadial(), `Selezione (${selected.length})`);
    // Navigation
    const nav: [string, string, () => void][] = [
      ["Tutti", "tutta la libreria", () => setFilter({ kind: "all" })],
      ["Preferiti", "", () => setFilter({ kind: "favorite" })],
      ["Da leggere", "", () => setFilter({ kind: "unread" })],
      ["Con codice (GitHub)", "", () => setFilter({ kind: "github" })],
      ["Peer-reviewed", "", () => setFilter({ kind: "peerreviewed" })],
      ...(facets.own ? [["Il mio lavoro", "", () => setFilter({ kind: "mywork" })] as [string, string, () => void]] : []),
      ["Cestino", "", () => setFilter({ kind: "trash" })],
    ];
    for (const [label, hint, run] of nav) out.push({ id: "nav-" + label, title: label, hint, section: "Vai a", run: () => { leaveReader(); run(); } });
    for (const c of collections)
      out.push({ id: "nav-c" + c.id, title: `Collezione: ${c.name}`, section: "Vai a", run: () => { leaveReader(); setFilter({ kind: "collection", id: c.id, label: c.name }); } });
    for (const t of tags)
      out.push({ id: "nav-t" + t.id, title: `Tag: ${t.name}`, hint: `${t.count} documenti`, section: "Vai a", run: () => { leaveReader(); toggleTagFilter(t.id); } });
    for (const s of savedSearches)
      out.push({ id: "nav-s" + s.id, title: `Ricerca salvata: ${s.name}`, hint: "rilancia e mostra le novità", section: "Vai a", run: () => { leaveReader(); runSaved(s); } });
    // Appunti, pagine wiki e progetti LaTeX: raggiungibili per nome, come i documenti.
    for (const n of notesList.slice(0, 300))
      out.push({ id: "note-" + n.slug, title: `Appunto: ${n.title}`, hint: n.excerpt || undefined, section: "Appunti", keywords: "appunto nota md", run: () => { leaveReader(); void openNoteHit(n.slug); } });
    for (const w of wikiPages.slice(0, 300))
      out.push({ id: "wiki-" + w.slug, title: `Wiki: ${w.title}`, section: "Wiki", keywords: "wiki pagina concetto", run: () => { leaveReader(); openWikiView(); void openWikiPage(w.slug); } });
    for (const p of palProjects.slice(0, 100))
      out.push({
        id: "proj-" + p.slug,
        title: `Progetto LaTeX: ${p.name}`,
        section: "Progetti",
        keywords: "latex progetto tex overleaf",
        // null → tick → slug: forza il cambiamento della prop anche quando si
        // richiede lo stesso progetto due volte (nel frattempo l'utente può
        // essere passato a mano su un altro).
        run: async () => {
          leaveReader();
          setFilter({ kind: "projects" });
          projectsOpenSlug = null;
          await tick();
          projectsOpenSlug = p.slug;
        },
      });
    // La guida, scheda per scheda.
    const helpTabs: [HelpTab, string][] = [
      ["inizia", "Inizia qui"],
      ["libreria", "Libreria"],
      ["lettura", "Lettura"],
      ["scrittura", "Scrittura"],
      ["scoperta", "Scoperta"],
      ["ai", "AI & dati"],
      ["faq", "FAQ — Come faccio a…"],
    ];
    for (const [tab, label] of helpTabs)
      out.push({ id: "help-" + tab, title: `Guida: ${label}`, section: "Guida", keywords: "aiuto help manuale documentazione faq", run: () => { openHelp(); helpTab = tab; } });
    for (const t of THEMES)
      out.push({ id: "th-" + t.value, title: `Tema: ${t.label}`, hint: t.dark ? "scuro" : "chiaro", section: "Aspetto", keywords: "tema aspetto colori", run: () => (theme = t.value) });
    // Documents (open directly) — not in the trash view, where `docs` holds deleted items
    const pool = filter.kind === "trash" ? [] : displayed.length ? displayed : docs;
    for (const d of pool.slice(0, 400))
      out.push({
        id: "doc-" + d.id,
        title: d.title ?? "Senza titolo",
        hint: [d.authors[0], d.year].filter(Boolean).join(" · "),
        section: "Documenti",
        keywords: d.authors.join(" ") + " " + (d.citekey ?? ""),
        run: () => openDocument(d),
      });
    return out;
  }

  // ----- Global keyboard shortcuts (main window only; the viewer has its own) -----
  function onGlobalKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      // Disponibile anche nel lettore: la palette (z-95) sta sopra e gestisce
      // i suoi tasti; le voci di navigazione chiudono il lettore da sole.
      paletteOpen = !paletteOpen;
      return;
    }
    if (openDoc) return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
      e.preventDefault();
      sidebarHidden = !sidebarHidden;
      return;
    }
    const t = e.target as HTMLElement | null;
    const typing = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
    if (e.key === "/" && !typing && !paletteOpen && !radial && editingId === null && !document.querySelector(".modalback, .back")) {
      e.preventDefault();
      searchEl?.focus();
      return;
    }
    // ---- Navigazione della libreria da tastiera (griglia e lista) ----
    const docsView =
      filter.kind === "all" || filter.kind === "favorite" || filter.kind === "unread" ||
      filter.kind === "collection" || filter.kind === "related" || filter.kind === "author" ||
      filter.kind === "github" || filter.kind === "peerreviewed" || filter.kind === "mywork";
    const uiBusy =
      paletteOpen || !!radial || editingId !== null || !!document.querySelector(".modalback, .back");
    if (!typing && !uiBusy && docsView && view !== "map" && displayed.length) {
      const k = e.key;
      if (k === "ArrowRight" || k === "ArrowLeft" || k === "ArrowDown" || k === "ArrowUp") {
        e.preventDefault();
        const cols = view === "grid" ? gridColumns() : 1;
        moveFocus(k === "ArrowRight" ? 1 : k === "ArrowLeft" ? -1 : k === "ArrowDown" ? cols : -cols);
        return;
      }
      // Invio apre il lettore (le card a fuoco DOM hanno già il proprio handler).
      if (k === "Enter" && panelDoc && !t?.closest(".card, tr, button, a")) {
        e.preventDefault();
        openDocument(panelDoc);
        return;
      }
      if (k === " " && !t?.closest("button, a, input, select, textarea")) {
        e.preventDefault();
        focusId = focusId != null ? null : displayed[0].id;
        return;
      }
      if ((k === "x" || k === "X") && focusId != null) {
        e.preventDefault();
        toggleSelect(focusId);
        return;
      }
      if ((k === "f" || k === "F") && panelDoc) {
        e.preventDefault();
        toggleFavorite(panelDoc);
        return;
      }
    }
    if (e.key === "Escape") {
      if (toolMenu || sortPop || indexPop) {
        toolMenu = null;
        sortPop = false;
        indexPop = false;
      } else if (focusId != null) {
        focusId = null; // chiude il pannello dettaglio
      }
    }
  }

  // ----- Costellazione (semantic map) -----
  let graphError = $state(false); // latched on failure so the effect can't retry-loop
  // Graph density controls (Costellazione 2.0 fase 1): k = neighbours per node,
  // minSim = similarity floor. Persisted so the user's tuning survives restarts.
  let graphK = $state(Number(localStorage.getItem("scriptorium-graph-k")) || 4);
  let graphMinSim = $state(Number(localStorage.getItem("scriptorium-graph-minsim")) || 0.55);
  async function loadGraph(force = false) {
    if (graphLoading || (graph && !force) || (graphError && !force)) return;
    graphLoading = true;
    graphError = false;
    try {
      graph = await similarityGraph(graphK, graphMinSim);
    } catch (e) {
      graphError = true;
      status = "Mappa semantica: " + e;
    } finally {
      graphLoading = false;
    }
  }
  // ----- Stelle fantasma: online discoveries drawn around a seed node -----
  interface MapGhost {
    key: string;
    seedId: number;
    title: string;
    year: number | null;
    venue: string | null;
    inLibrary: boolean;
    added: boolean;
    result: SearchResult;
    /** Ghost this one was discovered from (exploration chain); null = from the seed node. */
    parentKey: string | null;
    doi: string | null;
    author: string | null;
  }
  let mapGhosts = $state<MapGhost[]>([]);
  let ghostBusy = $state(false);
  /** Fetch papers related to a node (citations / topic / author) and show them
   *  as dashed ghost stars anchored to it. Results append (dedup by key). */
  async function exploreFromNode(id: number, relation: "citations" | "similar" | "author") {
    if (ghostBusy) return;
    let d = displayed.find((x) => x.id === id) ?? docs.find((x) => x.id === id) ?? recentDocs.find((x) => x.id === id);
    if (!d) {
      try {
        d = (await listDocuments()).find((x) => x.id === id);
      } catch {
        /* ignore */
      }
    }
    if (!d) return;
    ghostBusy = true;
    status =
      relation === "citations"
        ? "Cerco citazioni collegate…"
        : relation === "similar"
          ? "Cerco paper simili…"
          : "Cerco altri lavori dell'autore…";
    try {
      let results: SearchResult[] = [];
      if (relation === "citations") {
        if (!d.doi && !(d.title ?? "").trim()) throw "Serve un DOI o almeno un titolo per le citazioni";
        const cn = await exploreCitations({ doi: d.doi, title: d.title });
        if (cn.seed_unresolved) throw "OpenAlex non riconosce questo paper (né per DOI né per titolo)";
        results = [...cn.references, ...cn.citations];
      } else if (relation === "similar") {
        if (!(d.title ?? "").trim()) throw "Questo paper non ha un titolo da cercare";
        results = await discoverSearch(d.title!, "openalex", { author: null, yearFrom: null, yearTo: null, oaOnly: false, sort: "relevance" });
        results = results.filter((r) => !d!.doi || (r.doi ?? "") !== d!.doi); // not the seed itself
      } else {
        const author = d.authors[0];
        if (!author) throw "Questo paper non ha autori registrati";
        results = await discoverSearch(author, "openalex", { author, yearFrom: null, yearTo: null, oaOnly: false, sort: "relevance" });
      }
      const existing = new Set(mapGhosts.map((g) => g.key));
      const fresh: MapGhost[] = [];
      for (const r of results) {
        const key = r.external_id || r.doi || r.title || "";
        if (!key || existing.has(key)) continue;
        existing.add(key);
        fresh.push({
          key,
          seedId: id,
          title: r.title ?? "Senza titolo",
          year: r.year,
          venue: r.venue,
          inLibrary: r.in_library,
          added: false,
          result: r,
          parentKey: null,
          doi: r.doi ?? null,
          author: r.authors?.[0] ?? null,
        });
        if (fresh.length >= 12) break; // keep the fan readable
      }
      mapGhosts = [...mapGhosts, ...fresh];
      status = fresh.length
        ? `${fresh.length} ${fresh.length === 1 ? "stella fantasma trovata" : "stelle fantasma trovate"} — tratteggiate attorno al paper`
        : "Nessun nuovo risultato per questa relazione";
    } catch (e) {
      status = "Esplorazione non riuscita: " + e;
    } finally {
      ghostBusy = false;
    }
  }
  /** Explore onward FROM a ghost (snowball chain): the new discoveries anchor
   *  to that ghost, so you can keep digging without adding anything first. */
  async function exploreFromGhost(key: string, relation: "citations" | "similar" | "author") {
    if (ghostBusy) return;
    const g = mapGhosts.find((x) => x.key === key);
    if (!g) return;
    const r0 = g.result;
    ghostBusy = true;
    status =
      relation === "citations"
        ? "Cerco citazioni collegate…"
        : relation === "similar"
          ? "Cerco paper simili…"
          : "Cerco altri lavori dell'autore…";
    try {
      let results: SearchResult[] = [];
      if (relation === "citations") {
        // Ghosts come from OpenAlex: their external_id names the work exactly,
        // so citations work even without a DOI.
        const cn = await exploreCitations({
          openalexId: r0.source === "openalex" ? r0.external_id : null,
          doi: r0.doi,
          title: r0.title,
        });
        if (cn.seed_unresolved) throw "OpenAlex non riconosce questa scoperta";
        results = [...cn.references, ...cn.citations];
      } else if (relation === "similar") {
        if (!(r0.title ?? "").trim()) throw "Questa scoperta non ha un titolo da cercare";
        results = await discoverSearch(r0.title!, "openalex", { author: null, yearFrom: null, yearTo: null, oaOnly: false, sort: "relevance" });
        results = results.filter((x) => !r0.doi || (x.doi ?? "") !== r0.doi); // not the ghost itself
      } else {
        const author = r0.authors?.[0];
        if (!author) throw "Questa scoperta non ha autori registrati";
        results = await discoverSearch(author, "openalex", { author, yearFrom: null, yearTo: null, oaOnly: false, sort: "relevance" });
      }
      const existing = new Set(mapGhosts.map((x) => x.key));
      const fresh: MapGhost[] = [];
      for (const r of results) {
        const k = r.external_id || r.doi || r.title || "";
        if (!k || existing.has(k)) continue;
        existing.add(k);
        fresh.push({
          key: k,
          seedId: g.seedId,
          title: r.title ?? "Senza titolo",
          year: r.year,
          venue: r.venue,
          inLibrary: r.in_library,
          added: false,
          result: r,
          parentKey: key,
          doi: r.doi ?? null,
          author: r.authors?.[0] ?? null,
        });
        if (fresh.length >= 10) break; // keep the chain readable
      }
      mapGhosts = [...mapGhosts, ...fresh];
      status = fresh.length
        ? `${fresh.length} nuove scoperte, in catena da «${g.title.slice(0, 48)}»`
        : "Nessun nuovo risultato da questa scoperta";
    } catch (e) {
      status = "Esplorazione non riuscita: " + e;
    } finally {
      ghostBusy = false;
    }
  }
  /** Add a ghost's paper to the library (downloads the PDF when Open Access). */
  async function addGhostToLibrary(key: string) {
    const g = mapGhosts.find((x) => x.key === key);
    if (!g || g.added || g.inLibrary) return;
    status = "Aggiungo alla libreria…";
    try {
      await discoverAdd(g.result);
      mapGhosts = mapGhosts.map((x) => (x.key === key ? { ...x, added: true } : x));
      status = "Aggiunto alla libreria ✓ — entrerà nel grafo al prossimo aggiornamento dell'indice";
      loadDocs();
    } catch (e) {
      status = "Aggiunta non riuscita: " + e;
    }
  }

  /** Apply new density parameters from the map's HUD and rebuild the graph. */
  function setGraphParams(k: number, minSim: number) {
    graphK = Math.min(8, Math.max(1, Math.round(k)));
    graphMinSim = Math.min(0.95, Math.max(0.3, minSim));
    localStorage.setItem("scriptorium-graph-k", String(graphK));
    localStorage.setItem("scriptorium-graph-minsim", String(graphMinSim));
    loadGraph(true);
  }
  // Depend on `view` only: loadGraph's internal reads must not re-trigger this.
  $effect(() => {
    if (view === "map") untrack(() => loadGraph());
  });

  // ----- Riscopri: weighted serendipity over the library -----
  async function rediscover() {
    // Always draw from the full live library: `docs` may hold the trash or a
    // filtered slice depending on the current view.
    let pool: DocumentItem[] = [];
    try {
      pool = await listDocuments();
    } catch {
      pool = filter.kind === "trash" ? recentDocs : docs;
    }
    if (!pool.length) {
      status = "La libreria è vuota: importa qualche PDF prima";
      return;
    }
    const weights = pool.map((d) => {
      let w = 1;
      if (!d.is_read) w += 2; // prefer unread
      if (!d.last_page) w += 1.5; // never opened
      if (d.favorite) w += 0.5; // favorites deserve a comeback
      return w;
    });
    let r = Math.random() * weights.reduce((a, b) => a + b, 0);
    let pick = pool[0];
    for (let i = 0; i < pool.length; i++) {
      r -= weights[i];
      if (r <= 0) {
        pick = pool[i];
        break;
      }
    }
    ensureThumbs([pick]);
    let blurb = "";
    try {
      const m = await getDocumentMeta(pick.id);
      blurb = (m.summary || m.abstract_text || "").trim();
    } catch {
      /* ignore */
    }
    spotlight = { doc: pick, blurb: blurb.length > 420 ? blurb.slice(0, 420) + "…" : blurb };
  }

  // ---- Wiki della libreria: pagine concettuali generate dall'LLM locale ----
  let wikiPages = $state<WikiPageMeta[]>([]);
  let wikiPage = $state<WikiPage | null>(null);
  let wikiNewConcept = $state("");
  let wikiBusy = $state(false);
  let wikiProg = $state<{ phase: string; done: number; total: number; concept: string } | null>(null);

  // ----- Note (.md vault) -----
  let notesList = $state<NoteMeta[]>([]);
  // How the notes sidebar is ordered; remembered across sessions.
  type NoteSortKey = "updated" | "created" | "title";
  const noteSortStored =
    typeof localStorage !== "undefined" ? localStorage.getItem("scriptorium-notesort") : null;
  let noteSort = $state<NoteSortKey>(
    noteSortStored === "created" || noteSortStored === "title" ? noteSortStored : "updated",
  );
  $effect(() => {
    try {
      localStorage.setItem("scriptorium-notesort", noteSort);
    } catch {
      /* ignore */
    }
  });
  // Compact Italian date for the notes list ("10 lug 2026"); null-safe.
  const noteDateFmt = new Intl.DateTimeFormat("it-IT", { day: "numeric", month: "short", year: "numeric" });
  const fmtNoteDateShort = (ms: number | null) => (ms ? noteDateFmt.format(new Date(ms)) : "—");
  // The rendered order. Derived (not a mutation of notesList) so any refresh —
  // list reload, autosave prepend, append — re-sorts consistently.
  const notesSorted = $derived.by(() => {
    const byTitle = (a: NoteMeta, b: NoteMeta) =>
      a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
    const arr = [...notesList];
    if (noteSort === "title") {
      arr.sort(byTitle);
    } else if (noteSort === "created") {
      arr.sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0) || byTitle(a, b));
    } else {
      arr.sort((a, b) => (b.updated_at ?? 0) - (a.updated_at ?? 0) || byTitle(a, b));
    }
    return arr;
  });
  let noteView = $state<NoteView | null>(null);
  let noteDraft = $state(""); // raw markdown in the editor
  /** Normalize newlines when loading a note from disk: the textarea's value/API
   *  is LF-only (the DOM normalizes CRLF), so a CRLF file edited in Notepad would
   *  desync every selection offset from noteDraft and corrupt toolbar edits. */
  const normNl = (s: string) => s.replace(/\r\n?/g, "\n");
  let noteMode = $state<"edit" | "preview" | "split">("preview");
  let livePreviewHtml = $state(""); // rendered draft for the side-by-side "Affiancato" view
  let livePreviewTimer: ReturnType<typeof setTimeout> | undefined;
  let livePreviewSeq = 0; // guards against a slow render landing after a newer one
  let noteNewTitle = $state("");
  let noteSaved = $state(true); // false while an autosave is pending/failed
  let noteSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let noteFlush: Promise<void> | null = null; // in-flight save (single-flight guard)
  let noteRenaming = $state(false);
  let noteRenameValue = $state("");

  // ----- "Manda a nota": send a selection / abstract / summary into a .md note -----
  let sendNote = $state<{ payload: NotePayload; pos: { x: number; y: number } } | null>(null);
  const sendNoteCurrent = $derived(
    noteView ? { slug: noteView.slug, title: noteView.title } : null,
  );
  /** Open the destination picker for a piece of text originating from `d`. */
  async function openSendToNote(
    d: DocumentItem | null,
    part: { content: string; label?: string; page?: number | null; collapse?: boolean; code?: string | null; raw?: boolean },
    pos: { x: number; y: number },
  ) {
    if (!d || !part.content.trim()) {
      status = "Niente da mandare agli Appunti";
      return;
    }
    // If the open note has an unsaved edit, get it onto disk BEFORE any append, so
    // the append reads the flushed copy (not a stale one) and a later autosave
    // can't overwrite the block. If the flush fails, abort rather than risk the
    // draft — same discipline as openNote/newNote/commitRename.
    if (noteView && !noteSaved) {
      await flushNote();
      if (!noteSaved) {
        status = "Salvataggio dell'appunto aperto non riuscito — riprova prima di mandarci del testo";
        return;
      }
    }
    sendNote = {
      payload: {
        content: part.content,
        citekey: d.citekey ?? null,
        title: d.title ?? "senza titolo",
        page: part.page ?? null,
        label: part.label ?? null,
        collapse: part.collapse ?? false,
        code: part.code ?? null,
        raw: part.raw ?? false,
      },
      pos,
    };
  }
  /** After an append: refresh the notes list, and reload the open note if it was the target. */
  async function afterSendToNote(info: { slug: string; title: string }) {
    try {
      notesList = await listNotes();
    } catch {
      /* ignore */
    }
    // If we appended to the note currently open in the editor, reload it from disk
    // (which now holds the flushed draft + the new block) so the editor shows it.
    // Guard on noteSaved: if the user started a fresh edit while the picker was up,
    // reloading would discard that unsaved draft — keep it, exactly like
    // openNote/newNote/commitRename/refreshNotePreview.
    if (noteView?.slug === info.slug && noteSaved) {
      try {
        noteView = await getNote(info.slug);
        noteDraft = normNl(noteView.content_md);
        noteSaved = true;
        // The side-by-side preview renders `livePreviewHtml`, which only refreshes on
        // keystrokes (onNoteInput). A programmatic append doesn't fire that, so re-render
        // it here or the new block (e.g. a formula) stays invisible until the user
        // toggles modes. The plain "Anteprima" mode reads noteView.html (already reloaded).
        if (noteMode === "split") renderLivePreview();
      } catch {
        /* ignore */
      }
    }
  }

  async function openNotesView() {
    setFilter({ kind: "notes" });
    await loadNotes();
  }
  /** Open a note from a search hit: switch to the Note view and load it. */
  async function openNoteHit(slug: string) {
    await openNotesView();
    await openNote(slug);
  }
  async function loadNotes() {
    try {
      notesList = await listNotes();
    } catch (e) {
      status = "Errore nel caricare gli appunti: " + e;
    }
  }
  async function openNote(slug: string) {
    await flushNote(); // persist the note we're leaving
    if (noteView && !noteSaved) {
      status = "Salvataggio non riuscito: riprova prima di cambiare appunto";
      return; // don't overwrite the still-unsaved edits with another note
    }
    try {
      noteView = await getNote(slug);
      noteDraft = normNl(noteView.content_md);
      noteMode = "preview";
      noteSaved = true;
    } catch (e) {
      status = "Errore nell'aprire l'appunto: " + e;
    }
  }
  async function newNote() {
    const title = noteNewTitle.trim();
    await flushNote();
    if (noteView && !noteSaved) {
      status = "Salvataggio non riuscito: riprova prima di creare un appunto";
      return;
    }
    try {
      const slug = await createNote(title);
      noteNewTitle = "";
      await loadNotes();
      await openNote(slug);
      noteMode = "edit"; // jump straight into writing
    } catch (e) {
      status = "Errore nel creare l'appunto: " + e;
    }
  }
  function onNoteInput() {
    noteSaved = false;
    clearTimeout(noteSaveTimer);
    noteSaveTimer = setTimeout(flushNote, 700);
    if (noteMode === "split") scheduleLivePreview();
  }
  /** Debounced live render of the draft for the side-by-side preview. Renders the
   *  raw draft directly (no disk round-trip) so it tracks keystrokes; a sequence
   *  guard drops a slow render that resolves after a newer edit. */
  function scheduleLivePreview() {
    clearTimeout(livePreviewTimer);
    livePreviewTimer = setTimeout(renderLivePreview, 250);
  }
  async function renderLivePreview() {
    const seq = ++livePreviewSeq;
    const src = noteDraft;
    try {
      const html = await previewMarkdown(src);
      if (seq === livePreviewSeq) livePreviewHtml = html;
    } catch {
      /* keep the last good preview */
    }
  }
  // ----- Lightweight Markdown formatting toolbar over the note editor -----
  let noteEditorEl = $state<HTMLTextAreaElement | null>(null);
  /** Toolbar buttons call preventDefault on mousedown so the textarea never loses
   *  focus (and its selection) while clicking them. */
  const keepEditorFocus = (e: MouseEvent) => e.preventDefault();
  /** Apply a text edit to the editor IN PLACE: unlike reassigning noteDraft (which
   *  replaces the whole value, throws the caret to the end and scrolls the view
   *  there), this preserves the scroll position and the caret. execCommand keeps
   *  the edit in the browser's undo stack (Ctrl+Z works); setRangeText is the
   *  fallback. State is synced from the DOM afterwards. */
  function applyNoteEdit(start: number, end: number, text: string, selA: number, selB: number) {
    const el = noteEditorEl;
    if (!el) return;
    const st = el.scrollTop;
    el.focus();
    el.setSelectionRange(start, end);
    let done = false;
    try {
      done = text.length > 0 && document.execCommand("insertText", false, text);
    } catch {
      done = false;
    }
    if (!done) el.setRangeText(text, start, end, "preserve");
    el.setSelectionRange(selA, selB);
    el.scrollTop = st;
    noteDraft = el.value;
    onNoteInput();
  }
  /** Wrap the current selection in `pre`/`post` (e.g. **bold**, *italic*, `code`).
   *  All offsets/slices use el.value — the same LF-normalized space as the DOM
   *  selection — never noteDraft, which could lag or differ in newline flavor. */
  function mdWrap(pre: string, post: string, placeholder = "testo") {
    const el = noteEditorEl;
    if (!el) return;
    const s = el.selectionStart, e = el.selectionEnd;
    const sel = el.value.slice(s, e) || placeholder;
    applyNoteEdit(s, e, pre + sel + post, s + pre.length, s + pre.length + sel.length);
  }
  /** Apply a line prefix to every selected line: headings (replace level), lists,
   *  quotes (toggle). */
  function mdLinePrefix(prefix: string, heading = false) {
    const el = noteEditorEl;
    if (!el) return;
    const text = el.value;
    const s = el.selectionStart, e = el.selectionEnd;
    const lineStart = text.lastIndexOf("\n", s - 1) + 1;
    let lineEnd = text.indexOf("\n", e);
    if (lineEnd === -1) lineEnd = text.length;
    const block = text.slice(lineStart, lineEnd);
    const newBlock = block
      .split("\n")
      .map((ln) => {
        if (heading) return prefix + ln.replace(/^#{1,6}\s+/, "");
        return ln.startsWith(prefix) ? ln.slice(prefix.length) : prefix + ln;
      })
      .join("\n");
    applyNoteEdit(lineStart, lineEnd, newBlock, lineStart, lineStart + newBlock.length);
  }
  /** Insert text at the cursor (formula block, horizontal rule…). */
  function mdInsert(text: string) {
    const el = noteEditorEl;
    if (!el) return;
    const s = el.selectionStart, e = el.selectionEnd;
    applyNoteEdit(s, e, text, s + text.length, s + text.length);
  }
  // ----- Drop or paste images straight into the note (stored in assets/) -----
  const NOTE_IMG_MAX_BYTES = 20 * 1024 * 1024; // same cap as the backend store
  function readAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const r = new FileReader();
      r.onload = () => resolve(String(r.result));
      r.onerror = () => reject(r.error ?? new Error("lettura non riuscita"));
      r.readAsDataURL(file);
    });
  }
  /** Sanitize a filename for use as Markdown image alt text (single line, no `]`). */
  function imgAlt(name: string): string {
    return (name || "immagine").replace(/\.[^.]+$/, "").replace(/[\[\]\r\n]/g, " ").trim() || "immagine";
  }
  /** True when an open note editor can receive a dropped/pasted image. */
  function noteEditorActive(): boolean {
    return filter.kind === "notes" && !!noteView && (noteMode === "edit" || noteMode === "split");
  }
  /** Splice already-rendered `![alt](…)` blocks into the draft at the caret, but
   *  only if we're still on the same note we started from — the async round-trips
   *  can outlast a note switch, and writing into the new note would silently
   *  corrupt it (same discipline as the flushNote slug guard). If the text changed
   *  while the images were being stored, the drop-time offsets are stale: insert
   *  at the CURRENT caret instead of overwriting freshly typed text. */
  function commitNoteImages(
    targetSlug: string, s: number, e: number, valueSnapshot: string,
    blocks: string[], skipped: number, total: number,
  ) {
    if (noteView?.slug !== targetSlug) {
      status = "Immagine non inserita: hai cambiato appunto durante la lettura";
      return;
    }
    if (!blocks.length) {
      status = skipped ? "Nessuna immagine inserita (troppo grande o illeggibile)" : "Niente da inserire";
      return;
    }
    const snippet = "\n\n" + blocks.join("\n\n") + "\n\n";
    const el = noteEditorEl;
    // Don't yank focus from another field (rename box, search…) mid-typing.
    const active = document.activeElement;
    const focusElsewhere =
      active !== el && (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement);
    if (el && !focusElsewhere) {
      let a = s, b = e;
      if (el.value !== valueSnapshot) a = b = el.selectionEnd; // stale offsets → current caret
      applyNoteEdit(a, b, snippet, a + snippet.length, a + snippet.length);
    } else {
      // Editor not insertable right now: splice into state at (clamped) offsets.
      const a = Math.min(s, noteDraft.length), b = Math.min(e, noteDraft.length);
      noteDraft = noteDraft.slice(0, a) + snippet + noteDraft.slice(b);
      onNoteInput();
    }
    const done = blocks.length === 1 && total === 1 ? "Immagine inserita ✓" : `${blocks.length} immagini inserite ✓`;
    status = skipped ? `${done} · ${skipped} saltate (troppo grandi o illeggibili)` : done;
  }
  /** Insert one or more image *files* (paste, or HTML5 drop) at the cursor. The
   *  bytes go to the vault's assets/ folder; the note gets a short reference. */
  async function insertNoteImages(files: File[]) {
    const imgs = files.filter((f) => f.type.startsWith("image/"));
    if (!imgs.length || !noteView) return;
    const target = noteView.slug;
    const el = noteEditorEl;
    // Snapshot caret AND text BEFORE the async round-trips (see commitNoteImages).
    const snap = el ? el.value : noteDraft;
    const s = el ? el.selectionStart : noteDraft.length;
    const e = el ? el.selectionEnd : noteDraft.length;
    const blocks: string[] = [];
    let skipped = 0;
    for (const f of imgs) {
      if (f.size > NOTE_IMG_MAX_BYTES) { skipped++; continue; }
      try {
        const rel = await saveNoteAsset(await readAsDataUrl(f));
        blocks.push(`![${imgAlt(f.name)}](${rel})`);
      } catch {
        skipped++;
      }
    }
    commitNoteImages(target, s, e, snap, blocks, skipped, imgs.length);
  }
  /** Insert images given by filesystem *path* (OS drag&drop): the backend copies
   *  them into assets/ and returns the short reference. */
  async function insertNoteImagePaths(paths: string[]) {
    if (!paths.length || !noteView) return;
    const target = noteView.slug;
    const el = noteEditorEl;
    const snap = el ? el.value : noteDraft;
    const s = el ? el.selectionStart : noteDraft.length;
    const e = el ? el.selectionEnd : noteDraft.length;
    const blocks: string[] = [];
    let skipped = 0;
    for (const p of paths) {
      try {
        const name = p.split(/[\\/]/).pop() || "immagine";
        blocks.push(`![${imgAlt(name)}](${await importNoteAsset(p)})`);
      } catch {
        skipped++;
      }
    }
    commitNoteImages(target, s, e, snap, blocks, skipped, paths.length);
  }
  // NOTE: on Windows/WebView2 Tauri's native drag-drop (dragDropEnabled, default on)
  // intercepts OS file drops, so these HTML5 handlers never receive files — the real
  // image-drop path is the global onDragDropEvent handler routing to
  // insertNoteImagePaths. These remain as a harmless fallback if native DnD is off.
  function onNoteDrop(ev: DragEvent) {
    const files = ev.dataTransfer?.files;
    if (!files || !files.length) return;
    const imgs = Array.from(files).filter((f) => f.type.startsWith("image/"));
    if (!imgs.length) return; // let non-image drops (e.g. text) behave normally
    ev.preventDefault();
    void insertNoteImages(imgs);
  }
  function onNoteDragOver(ev: DragEvent) {
    // Allow the drop only when it carries files (images), so text DnD still works.
    if (ev.dataTransfer?.types?.includes("Files")) ev.preventDefault();
  }
  function onNotePaste(ev: ClipboardEvent) {
    const items = ev.clipboardData?.items;
    if (!items) return;
    const files: File[] = [];
    for (let i = 0; i < items.length; i++) {
      const it = items[i];
      if (it.kind === "file" && it.type.startsWith("image/")) {
        const f = it.getAsFile();
        if (f) files.push(f);
      }
    }
    if (!files.length) return; // plain-text paste falls through untouched
    ev.preventDefault();
    void insertNoteImages(files);
  }
  /** Move the paragraph/block containing the cursor up or down (reorder images/text).
   *  Blocks are separated by runs of 2+ newlines; boundaries are computed exactly so
   *  the right block moves regardless of how many blank lines separate them. */
  function mdMoveBlock(dir: -1 | 1) {
    const el = noteEditorEl;
    if (!el) return;
    const caret = el.selectionStart;
    const text = el.value;
    const bounds: [number, number][] = [];
    let start = 0;
    for (const m of text.matchAll(/\n{2,}/g)) {
      bounds.push([start, m.index!]);
      start = m.index! + m[0].length;
    }
    bounds.push([start, text.length]);
    let idx = bounds.findIndex(([, b]) => caret <= b);
    if (idx === -1) idx = bounds.length - 1;
    const j = idx + dir;
    if (j < 0 || j >= bounds.length) return;
    const [ia, ib] = bounds[idx];
    const [ja, jb] = bounds[j];
    const blockI = text.slice(ia, ib);
    const blockJ = text.slice(ja, jb);
    // Don't shuffle an empty leading/trailing block (from 2+ trailing newlines): that
    // would just inject blank lines instead of being a no-op.
    if (!blockI.trim() || !blockJ.trim()) return;
    // Swap ONLY the two neighbouring blocks, keeping the separator between them
    // verbatim — no whole-text rewrite, so other blank runs and the view survive.
    if (dir === 1) {
      const repl = blockJ + text.slice(ib, ja) + blockI;
      const selA = ia + repl.length - blockI.length;
      applyNoteEdit(ia, jb, repl, selA, selA + blockI.length);
    } else {
      const repl = blockI + text.slice(jb, ia) + blockJ;
      applyNoteEdit(ja, ib, repl, ja, ja + blockI.length);
    }
  }
  /** Export the open note to a self-contained HTML page or a LaTeX document. */
  async function exportNoteAs(fmt: "html" | "latex") {
    if (!noteView) return;
    await flushNote(); // export the latest on-disk copy
    if (!noteSaved) {
      status = "Salvataggio non riuscito: riprova prima di esportare";
      return;
    }
    const ext = fmt === "html" ? "html" : "tex";
    const path = await save({
      defaultPath: `${noteView.slug}.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    try {
      await exportNote(noteView.slug, fmt, path);
      status = fmt === "html" ? "Appunto esportato in HTML ✓" : "Appunto esportato in LaTeX ✓ (con le figure)";
    } catch (e) {
      status = "Esportazione non riuscita: " + e;
    }
  }
  /** Export the note as a plain .md copy (the note IS Markdown on disk: this just
   *  saves the freshly-flushed content wherever the user chooses). */
  async function exportNoteMd() {
    if (!noteView) return;
    await flushNote(); // export the latest content
    if (!noteSaved) {
      status = "Salvataggio non riuscito: riprova prima di esportare";
      return;
    }
    const path = await save({
      defaultPath: `${noteView.slug}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    try {
      await writeTextFile(path, noteDraft);
      status = "Appunto esportato in Markdown ✓";
    } catch (e) {
      status = "Esportazione non riuscita: " + e;
    }
  }
  /** Export the note to PDF: render it to a self-contained HTML page, then open the
   *  print dialog on that page alone (the user chooses "Salva come PDF"). */
  async function exportNotePdf() {
    if (!noteView) return;
    await flushNote();
    if (!noteSaved) {
      status = "Salvataggio non riuscito: riprova prima di esportare";
      return;
    }
    try {
      const html = await noteExportHtml(noteView.slug);
      await printHtml(html);
      status = "Apri la stampa: scegli «Salva come PDF» per esportare";
    } catch (e) {
      status = "Esportazione in PDF non riuscita: " + e;
    }
  }
  /** Persist the current note. Single-flight (concurrent callers await the same
   *  save) and loops until the on-disk copy matches the draft, so keystrokes
   *  typed during an in-flight save are never lost. Leaves noteSaved=false on
   *  error so callers can avoid discarding the dirty draft. */
  function flushNote(): Promise<void> {
    clearTimeout(noteSaveTimer);
    if (noteFlush) return noteFlush;
    if (noteSaved || !noteView) return Promise.resolve();
    noteFlush = doFlushNote().finally(() => (noteFlush = null));
    return noteFlush;
  }
  async function doFlushNote() {
    while (noteView && !noteSaved) {
      const slug = noteView.slug;
      const snapshot = noteDraft;
      let meta: NoteMeta;
      try {
        meta = await saveNote(slug, snapshot);
      } catch (e) {
        status = "Errore nel salvare l'appunto: " + e;
        return; // leave noteSaved=false → callers know the save failed
      }
      if (noteView?.slug !== slug) return; // switched away mid-save; the new note handles itself
      notesList = [meta, ...notesList.filter((n) => n.slug !== slug)];
      if (noteDraft === snapshot) {
        noteSaved = true; // fully flushed
      }
      // else: the draft changed during the save → loop and persist the newer text
    }
  }
  /** Re-render the preview (re-weave links) from the current draft. */
  async function refreshNotePreview() {
    await flushNote();
    if (!noteSaved) return; // save failed — keep the dirty draft, don't overwrite it
    if (noteView) {
      try {
        noteView = await getNote(noteView.slug);
        noteDraft = normNl(noteView.content_md);
      } catch {
        /* keep the current view */
      }
    }
  }
  function startRename() {
    if (!noteView) return;
    noteRenameValue = noteView.title;
    noteRenaming = true;
  }
  /** Abandon an in-progress rename (Escape or click-away), keeping the title. */
  function cancelRename() {
    noteRenaming = false;
  }
  /** Commit a title rename: rewrites the title line AND renames the .md file.
   *  Only reached from Enter, so no click is racing us for the focus. */
  async function commitRename() {
    if (!noteRenaming || !noteView) return;
    const target = noteView.slug; // the note we're renaming
    const t = noteRenameValue.trim();
    noteRenaming = false;
    if (!t || t === noteView.title) return;
    await flushNote(); // persist body edits so the rename reads the latest content
    if (!noteSaved) {
      status = "Salvataggio non riuscito: riprova prima di rinominare";
      return;
    }
    try {
      const newSlug = await renameNote(target, t);
      await loadNotes();
      // Follow the rename only if we're still viewing that same note.
      if (noteView?.slug === target) await openNote(newSlug);
      status = "Appunto rinominato ✓";
    } catch (e) {
      status = "Errore rinomina: " + e;
    }
  }
  /** Format a note's epoch-ms timestamp for the info line. */
  function fmtNoteDate(ms: number | null): string {
    if (!ms) return "—";
    try {
      return new Date(ms).toLocaleString("it-IT", { dateStyle: "medium", timeStyle: "short" });
    } catch {
      return "—";
    }
  }
  async function removeNote(slug: string) {
    if (noteView && noteView.slug !== slug) await flushNote(); // persist a different open note first
    const n = notesList.find((x) => x.slug === slug);
    if (!(await confirmAsk(`Eliminare l'appunto «${n?.title ?? slug}»?`))) return;
    try {
      await deleteNote(slug);
      if (noteView?.slug === slug) {
        noteView = null;
        noteDraft = "";
      }
      await loadNotes();
    } catch (e) {
      status = "Errore: " + e;
    }
  }
  /** Focus (and select) an input as soon as it mounts — for the rename field. */
  function focusOnMount(node: HTMLElement) {
    node.focus();
    if (node instanceof HTMLInputElement) node.select();
  }
  /** Intercept clicks on woven note links: #note-<slug>, #doc-<id>, external. */
  function noteLinksAction(node: HTMLElement) {
    const onClick = (e: MouseEvent) => {
      const a = (e.target as HTMLElement)?.closest("a");
      if (!a) return;
      const href = a.getAttribute("href") ?? "";
      if (href.startsWith("#note-")) {
        e.preventDefault();
        openNote(href.slice("#note-".length));
      } else if (href.startsWith("#doc-")) {
        e.preventDefault();
        const id = parseInt(href.slice("#doc-".length), 10);
        if (!Number.isNaN(id)) openById(id, null);
      } else if (/^https?:/.test(href)) {
        e.preventDefault();
        openInBrowser(href);
      }
    };
    node.addEventListener("click", onClick);
    return { destroy: () => node.removeEventListener("click", onClick) };
  }

  async function loadWikiList() {
    try {
      wikiPages = await wikiList();
    } catch (e) {
      status = "Wiki: " + e;
    }
  }
  function openWikiView() {
    setFilter({ kind: "wiki" });
    loadWikiList();
  }
  async function openWikiPage(slug: string) {
    try {
      wikiPage = await wikiGet(slug);
    } catch (e) {
      status = "Wiki: " + e;
    }
  }
  async function runWikiGenerate(concept: string, tagId: number | null = null) {
    const c = concept.trim();
    if (!c || wikiBusy) return;
    wikiBusy = true;
    try {
      const slug = await wikiGenerate(c, tagId);
      wikiNewConcept = "";
      await loadWikiList();
      await openWikiPage(slug);
      status = `Pagina wiki «${c}» generata ✓`;
    } catch (e) {
      status = "Wiki: " + e;
    } finally {
      wikiBusy = false;
      wikiProg = null;
    }
  }
  /** Generate/refresh one page per tag (≥2 documents make a meaningful page). */
  async function wikiGenerateFromTags() {
    if (wikiBusy) return;
    const worth = tags.filter((t) => t.count >= 2);
    if (!worth.length) {
      status = "Nessun tag con almeno 2 documenti: assegna qualche tag prima";
      return;
    }
    wikiBusy = true;
    try {
      for (const t of worth) {
        const slug = await wikiGenerate(t.name, t.id);
        await loadWikiList();
        if (!wikiPage) await openWikiPage(slug);
      }
      status = `Wiki aggiornata: ${worth.length} pagine ✓`;
    } catch (e) {
      status = "Wiki: " + e;
    } finally {
      wikiBusy = false;
      wikiProg = null;
    }
  }
  // "Pagina wiki dalla selezione": the user picks the sources explicitly.
  let wikiFromSel = $state<{ ids: number[]; concept: string } | null>(null);
  async function runWikiFromSelection() {
    if (!wikiFromSel || wikiBusy) return;
    const c = wikiFromSel.concept.trim();
    if (!c) return;
    const ids = wikiFromSel.ids;
    wikiFromSel = null;
    wikiBusy = true;
    try {
      const slug = await wikiGenerate(c, null, ids);
      openWikiView();
      await loadWikiList();
      await openWikiPage(slug);
      status = `Pagina wiki «${c}» generata dalle ${ids.length} fonti scelte ✓`;
    } catch (e) {
      status = "Wiki: " + e;
    } finally {
      wikiBusy = false;
      wikiProg = null;
    }
  }

  async function removeWikiPage(slug: string) {
    if (!(await confirmAsk("Eliminare questa pagina wiki? I documenti non vengono toccati.", "Elimina"))) return;
    try {
      await wikiDelete(slug);
      if (wikiPage?.slug === slug) wikiPage = null;
      await loadWikiList();
    } catch (e) {
      status = "" + e;
    }
  }
  async function stopWiki() {
    try {
      await wikiCancel();
    } catch {
      /* ignore */
    }
  }
  /** First cited page of source [n], for the deep link into the PDF. */
  function wikiSourceTarget(n: number): { docId: number; page: number | null } | null {
    const s = wikiPage?.sources.find((x) => x.n === n);
    if (!s) return null;
    return { docId: s.document_id, page: s.claims.find((c) => c.page != null)?.page ?? null };
  }
  /** Intercept clicks inside the rendered page: [n] → source PDF at page; [[…]] → other page. */
  function wikiLinksAction(node: HTMLElement) {
    const handler = (e: MouseEvent) => {
      const a = (e.target as HTMLElement).closest("a");
      if (!a) return;
      const href = a.getAttribute("href") ?? "";
      if (href.startsWith("#src-")) {
        e.preventDefault();
        const t = wikiSourceTarget(parseInt(href.slice(5), 10));
        if (t) openById(t.docId, t.page);
      } else if (href.startsWith("#wiki-")) {
        e.preventDefault();
        const slug = href.slice(6);
        if (wikiPages.some((p) => p.slug === slug)) openWikiPage(slug);
      } else if (/^https?:/i.test(href)) {
        e.preventDefault();
        openInBrowser(href);
      }
    };
    node.addEventListener("click", handler);
    return {
      destroy() {
        node.removeEventListener("click", handler);
      },
    };
  }

  // ---- Sintesi sulla selezione (confronto / rassegna / tabella risultati) ----
  let aiDoc = $state<(AiDocResult & { kind: string; title: string }) | null>(null);
  let resultsGrid = $state<string[][] | null>(null);
  let pathModal = $state<{ doc: DocumentItem; steps: PathStep[] } | null>(null);

  async function runCompare() {
    const ids = selected.slice(0, 3);
    if (ids.length < 2) {
      status = "Seleziona 2 o 3 documenti da confrontare";
      return;
    }
    if (wikiBusy) return;
    wikiBusy = true;
    try {
      const r = await compareDocuments(ids);
      aiDoc = { ...r, kind: "Confronto", title: `Confronto di ${ids.length} paper` };
    } catch (e) {
      status = "Confronto: " + e;
    } finally {
      wikiBusy = false;
      wikiProg = null;
    }
  }
  async function runReview() {
    const picked = selected.length ? selected : [];
    if (picked.length < 2) {
      status = "Seleziona da 2 a 10 paper per una rassegna";
      return;
    }
    const ids = picked.slice(0, 10);
    if (picked.length > 10) {
      status = `Rassegna sui primi 10 di ${picked.length} paper selezionati (il massimo)`;
    }
    if (wikiBusy) return;
    wikiBusy = true;
    try {
      const r = await generateReview(ids);
      aiDoc = { ...r, kind: "Rassegna", title: `Rassegna di ${ids.length} paper` };
    } catch (e) {
      status = "Rassegna: " + e;
    } finally {
      wikiBusy = false;
      wikiProg = null;
    }
  }
  /** Turn the open AI doc (rassegna/confronto) into a real .md note in the vault:
   *  rewrite each [n] to a live [[@citekey]] backlink, then create + open it. */
  async function aiDocToNote() {
    if (!aiDoc) return;
    let body = aiDoc.md;
    for (const s of aiDoc.sources) {
      body = body.split(`[${s.n}]`).join(refToken(s.citekey, s.title));
    }
    const today = new Date().toISOString().slice(0, 10);
    let slug: string | null = null;
    try {
      slug = await createNote(`${aiDoc.title} — ${today}`);
      await appendToNote(slug, body); // seeded "# title" + the synthesis with backlinks
    } catch (e) {
      // Roll back the just-created (content-less) note so nothing dangles.
      if (slug) {
        try {
          await deleteNote(slug);
        } catch {
          /* best-effort */
        }
      }
      status = "Errore nel salvare l'appunto: " + e;
      return;
    }
    aiDoc = null;
    try {
      await openNotesView(); // switch to the Notes surface…
      await openNote(slug); // …and open the fresh note
    } catch {
      /* the note is saved; just couldn't auto-open it */
    }
    status = "Salvato negli Appunti ✓";
  }
  async function runHarvest() {
    const ids = selected.slice(0, 8);
    if (!ids.length) {
      status = "Seleziona i documenti da cui raccogliere i risultati";
      return;
    }
    if (wikiBusy) return;
    wikiBusy = true;
    try {
      resultsGrid = await harvestResults(ids);
    } catch (e) {
      status = "Risultati: " + e;
    } finally {
      wikiBusy = false;
      wikiProg = null;
    }
  }
  /** Copy the synthesis as markdown, or with [n] rewritten to \cite{} / [@key]. */
  async function copyAiDoc(fmt: "md" | "latex" | "pandoc") {
    if (!aiDoc) return;
    let text = aiDoc.md;
    if (fmt !== "md") {
      for (const s of aiDoc.sources) {
        const key = s.citekey ?? `doc${s.document_id}`;
        const rep = fmt === "latex" ? `\\cite{${key}}` : `[@${key}]`;
        text = text.split(`[${s.n}]`).join(rep);
      }
    }
    try {
      await navigator.clipboard.writeText(text);
      status = fmt === "md" ? "Markdown copiato" : fmt === "latex" ? "Copiato con \\cite{…}" : "Copiato con [@citekey]";
    } catch {
      status = "Impossibile copiare negli appunti";
    }
  }
  async function saveAiDoc() {
    if (!aiDoc) return;
    const path = await save({
      defaultPath: aiDoc.kind === "Rassegna" ? "rassegna.md" : "confronto.md",
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    try {
      const sources = aiDoc.sources
        .map((s) => `[${s.n}] ${s.title}${s.year ? ` (${s.year})` : ""}${s.citekey ? ` — @${s.citekey}` : ""}`)
        .join("\n");
      await writeTextFile(path, `${aiDoc.md}\n\n---\n### Fonti\n${sources}\n`);
      status = "Salvato ✓";
    } catch (e) {
      status = "Errore salvataggio: " + e;
    }
  }
  /** [n] links inside the synthesis open the source document. */
  function aiDocLinksAction(node: HTMLElement) {
    const handler = (e: MouseEvent) => {
      const a = (e.target as HTMLElement).closest("a");
      if (!a) return;
      const href = a.getAttribute("href") ?? "";
      if (href.startsWith("#src-")) {
        e.preventDefault();
        const s = aiDoc?.sources.find((x) => x.n === parseInt(href.slice(5), 10));
        if (s) openById(s.document_id);
      } else if (/^https?:/i.test(href)) {
        e.preventDefault();
        openInBrowser(href);
      }
    };
    node.addEventListener("click", handler);
    return {
      destroy() {
        node.removeEventListener("click", handler);
      },
    };
  }
  async function exportResults(fmt: "csv" | "md" | "xlsx") {
    if (!resultsGrid) return;
    const path = await save({
      defaultPath: `risultati.${fmt}`,
      filters: [{ name: fmt.toUpperCase(), extensions: [fmt] }],
    });
    if (!path) return;
    try {
      await exportTable(resultsGrid, fmt, path);
      status = "Tabella esportata ✓";
    } catch (e) {
      status = "Errore export: " + e;
    }
  }

  // ---- Percorso di lettura (grafo citazioni + embedding, senza LLM) ----
  async function openReadingPath(d: DocumentItem) {
    try {
      pathModal = { doc: d, steps: await readingPath(d.id) };
    } catch (e) {
      status = "Percorso di lettura: " + e;
    }
  }
  async function addPathStep(step: PathStep) {
    if (!step.doi) return;
    try {
      await addByIdentifiers([step.doi]);
      status = "Riferimento aggiunto alla libreria ✓ (usa «Allega PDF…» o Trova PDF per il file)";
      await loadDocs();
      await loadSidebar();
      if (pathModal) pathModal = { doc: pathModal.doc, steps: await readingPath(pathModal.doc.id) };
    } catch (e) {
      status = "Errore: " + e;
    }
  }

  // Status messages surface as a quiet toast that fades on its own.
  let statusTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!status) return;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (status = ""), 7000);
  });

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

<svelte:window
  onclick={() => { toolMenu = null; sortPop = false; indexPop = false; tagPanel = null; collPanel = null; mapPop = null; tagEdit = null; }}
  onkeydown={onGlobalKey}
  oncontextmenu={onGlobalContext}
  onfocus={checkClipboard}
/>

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
      <button class="railtoggle" title="Mostra/nascondi la barra laterale (Ctrl+B)" aria-label="Barra laterale" onclick={(e) => { e.stopPropagation(); sidebarHidden = !sidebarHidden; }}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 4h18a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM9 4v16" /></svg>
      </button>
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
      {:else}
        <!-- AI off: the chip stays visible so one click brings everything back
             (after «Ferma AI» the features would otherwise vanish with no way back
             in sight — the #1 "the AI won't activate" confusion). -->
        <button
          class="aichip off"
          title="AI locale disattivata — clic per riattivarla (riassunti, tag automatici, domande…)"
          onclick={quickEnableAi}
          aria-label="AI locale disattivata — clic per riattivarla"
        >
          <span class="aidot"></span>AI off
        </button>
      {/if}
      <button
        class="aichip idxchip"
        class:active={emb.total > 0 && emb.embedded >= emb.total}
        class:busy={!!embedProgress && embedProgress.phase !== "done" && embedProgress.phase !== "cancelled"}
        title="Indice semantico: abilita ricerca per significato, correlati e costellazione"
        aria-label="Indice semantico"
        onclick={(e) => { e.stopPropagation(); indexPop = !indexPop; sortPop = false; toolMenu = null; }}
      >
        <span class="aidot"></span>◈ {emb.embedded}/{emb.total}
      </button>
      {#if updateLatest}
        <button
          class="aichip active"
          title={`È disponibile Scriptorium ${updateLatest} — apri GitHub`}
          aria-label={`Nuova versione ${updateLatest} disponibile`}
          onclick={() => openInBrowser(updateUrl)}
        >
          <span class="aidot"></span>↑ {updateLatest}
        </button>
      {/if}
    </div>
    <div class="searchgroup">
      <input
        class="search"
        type="search"
        placeholder="Cerca per testo o significato…  ( / )"
        title="Cerca nei tuoi PDF — scorciatoia: /"
        bind:value={query}
        bind:this={searchEl}
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
    {#if needsMeta > 0}
      <button
        class="ambient"
        onclick={enrichMeta}
        disabled={enriching}
        title={"Recupera titolo, autori, anno, rivista, abstract e riferimenti dei documenti incompleti:\narXiv dall'id nel nome del file (anche scansioni), poi DOI e titolo dal PDF (Crossref/arXiv).\nSolo abbinamenti sicuri; i restanti si confermano a mano (tasto destro → Organizza → Recupera metadati)."}
      >
        {enriching ? (metaScan ? `recupero… ${metaScan.done}/${metaScan.total}` : "recupero…") : `✦ ${needsMeta} senza metadati`}
      </button>
    {/if}
    <button class="iconbtn" title="Palette comandi — ogni azione, digitando (Ctrl+K)" aria-label="Palette comandi" onclick={(e) => { e.stopPropagation(); paletteOpen = true; }}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M18 3a3 3 0 0 0-3 3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" /></svg>
    </button>
  </header>

  <nav class="toolbar" aria-label="Strumenti">
    {#each buildGlobalRadial() as g (g.id)}
      <button
        class="iconbtn"
        class:menuopen={toolMenu?.id === g.id}
        class:active={activeToolGroup === g.id}
        title={g.hint ? g.label + " — " + g.hint : g.label}
        aria-label={g.label}
        disabled={g.disabled}
        onclick={(e) => openTool(e, g)}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d={g.icon} /></svg>
        {#if g.badge}<span class="toolbadge">{g.badge}</span>{/if}
      </button>
    {/each}
  </nav>

  {#if embedProgress && embedProgress.phase !== "done" && embedProgress.phase !== "cancelled"}
    <div class="headprog" title={embedProgress.phase === "model" ? "Carico il modello bge-m3…" : `Indicizzo ${embedProgress.done}/${embedProgress.total}`}>
      <div class="fill" style="width:{embedProgress.total ? (embedProgress.done / embedProgress.total) * 100 : 8}%"></div>
    </div>
  {/if}

  {#if filter.kind !== "trash" && filter.kind !== "discover" && filter.kind !== "duplicates" && filter.kind !== "terminal" && filter.kind !== "ask" && filter.kind !== "wiki"}
    <div class="strip">
      <div class="stripleft">
        <div class="seg" role="group" aria-label="Vista">
          <button class="segbtn" class:active={view === "grid"} onclick={() => (view = "grid")} title="Griglia (copertine)" aria-label="Vista a griglia">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 3h7v7H3zM14 3h7v7h-7zM14 14h7v7h-7zM3 14h7v7H3z" /></svg>
          </button>
          <button class="segbtn" class:active={view === "list"} onclick={() => (view = "list")} title="Lista (colonne ordinabili)" aria-label="Vista a lista">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" /></svg>
          </button>
          <button class="segbtn" class:active={view === "map"} onclick={() => (view = "map")} title="Costellazione: la libreria come mappa semantica" aria-label="Vista a costellazione">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M12 4a2 2 0 1 0 .01 0M5 16a2 2 0 1 0 .01 0M19 16a2 2 0 1 0 .01 0M10.8 9.6L6.6 14.6M13.2 9.6l4.2 5" /></svg>
          </button>
        </div>
        {#if view === "grid"}
          <div class="gridzoom" title="Dimensione delle copertine nella griglia">
            <button class="zbtn" onclick={() => (gridSize = Math.max(120, gridSize - 30))} aria-label="Copertine più piccole" title="Più piccole">−</button>
            <input class="zrange" type="range" min="120" max="360" step="10" bind:value={gridSize} aria-label="Dimensione copertine" />
            <button class="zbtn" onclick={() => (gridSize = Math.min(360, gridSize + 30))} aria-label="Copertine più grandi" title="Più grandi">+</button>
          </div>
        {/if}
      </div>
      <div class="stripright">
        {#if displayed.length && view !== "map"}
          <button class="chipbtn" onclick={toggleSelectAll} title="Seleziona o deseleziona tutti i documenti mostrati (per le azioni multiple)">{allSelected ? "Deseleziona tutti" : "Seleziona tutti"}</button>
          <button class="chipbtn" class:on={sortChain.length > 0} onclick={(e) => { e.stopPropagation(); sortPop = !sortPop; indexPop = false; toolMenu = null; }} title="Ordina i documenti (più criteri combinabili)">
            Ordina{#if sortChain.length}: {SORT_LABELS[sortChain[0].key]} {sortArrow(sortChain[0].key)}{#if sortChain.length > 1} +{sortChain.length - 1}{/if}{/if} ▾
          </button>
        {/if}
      </div>
      {#if sortPop}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="pop sortpop" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") sortPop = false; e.stopPropagation(); }} role="menu" tabindex="-1">
          <div class="poptitle">Ordina per <span class="popnote">un tocco attiva · un altro inverte · un terzo toglie</span></div>
          <div class="popchips">
            {#each SORT_KEYS as k (k)}
              <button class="sortchip" class:on={sortDirOf(k)} onclick={() => cycleSort(k)} title={`Ordina per ${SORT_LABELS[k].toLowerCase()}`}>
                {SORT_LABELS[k]}{#if sortDirOf(k)}<span class="sar">{sortArrow(k)}</span>{#if sortChain.length > 1}<span class="srank">{sortRank(k)}</span>{/if}{/if}
              </button>
            {/each}
          </div>
          {#if sortChain.length}<button class="sortclear" onclick={clearSort} title="Azzera l'ordinamento">azzera</button>{/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if indexPop}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="pop indexpop" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") indexPop = false; e.stopPropagation(); }} role="dialog" tabindex="-1">
      <div class="poptitle">Indice semantico</div>
      <p class="popbody">Abilita la ricerca per significato, i «Correlati» e la Costellazione. {emb.embedded}/{emb.total} documenti indicizzati.</p>
      {#if embedProgress && embedProgress.phase !== "done" && embedProgress.phase !== "cancelled"}
        <div class="poprow">
          <span class="hint">{embedProgress.phase === "model" ? "Carico modello bge-m3…" : `Indicizzo ${embedProgress.done}/${embedProgress.total}`}</span>
          <div class="bar"><div class="fill" style="width:{embedProgress.total ? (embedProgress.done / embedProgress.total) * 100 : 8}%"></div></div>
          <button class="ghost small" onclick={stopIndex} title="Ferma l'indicizzazione (i documenti già indicizzati restano salvati)">Stop</button>
        </div>
      {:else}
        <div class="poprow">
          <button
            class="ghost small"
            onclick={generateIndex}
            disabled={generating || emb.embedded >= emb.total || emb.total === 0}
            title="Calcola gli embedding mancanti (la prima volta scarica il modello ~2.3GB)"
          >
            {generating ? "…" : emb.embedded >= emb.total && emb.total > 0 ? "Aggiornato ✓" : "Genera"}
          </button>
        </div>
      {/if}
    </div>
  {/if}

  <div class="body">
    <aside class="sidebar" class:collapsed={sidebarHidden} inert={sidebarHidden}>
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
      {#if facets.own}
        <button class="navitem" class:active={filter.kind === "mywork"} onclick={() => setFilter({ kind: "mywork" })} title="I tuoi lavori, importati da progetti LaTeX (.zip)">
          Il mio lavoro<span class="navcount">{facets.own}</span>
        </button>
      {/if}

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
              <button class="navitem" class:active={tagFilter.includes(t.id)} onclick={() => toggleTagFilter(t.id)} title={`Filtra per il tag "${t.name}" (${t.count} paper) — puoi selezionarne più di uno`}>
                <span class="dot" style="background:{t.color ?? '#888'}"></span>{t.name}
                <span class="navcount">{t.count}</span>
                {#if tagFilter.includes(t.id)}<span class="navcheck">✓</span>{/if}
              </button>
              <button class="x edit" title="Rinomina o cambia colore" aria-label={`Modifica il tag ${t.name}`} onclick={(e) => { e.stopPropagation(); tagEdit = { id: t.id, name: t.name, color: t.color ?? PALETTE[0], x: (e.currentTarget as HTMLElement).getBoundingClientRect().left, y: (e.currentTarget as HTMLElement).getBoundingClientRect().bottom + 4 }; }}>✎</button>
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

      <div class="sec">Cartella sorvegliata</div>
      <div class="watched">
        {#if watchedFolder}
          <span class="wpath" title={watchedFolder}>{baseName(watchedFolder)}</span>
          <button class="x" title="Smetti di sorvegliare" onclick={clearWatchedFolder}>×</button>
        {:else}
          <button class="ghost small" onclick={pickWatchedFolder} title="Scegli una cartella: importa subito i PDF già presenti e poi quelli che aggiungerai automaticamente">Scegli cartella…</button>
        {/if}
      </div>

      <p class="sidehint" title="Chiedi alla libreria, Wiki, Appunti, Cerca online, Novità, Cura della libreria, Cestino, Terminale, Guida, Impostazioni e Informazioni sono ora nella barra strumenti in alto ↑ — tasto destro: menu radiale · Ctrl+K: palette">Gli strumenti sono nella barra in alto ↑</p>
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
      {:else if filter.kind === "wiki"}
        <div class="wikiwrap">
          <aside class="wikinav">
            <div class="wikinew">
              <input
                placeholder="Nuova pagina: concetto o tag…"
                bind:value={wikiNewConcept}
                onkeydown={(e) => e.key === "Enter" && runWikiGenerate(wikiNewConcept)}
                title="Scrivi un concetto (o il nome di un tag): la pagina viene sintetizzata dai documenti pertinenti"
              />
              <button class="ghost small" onclick={() => runWikiGenerate(wikiNewConcept)} disabled={wikiBusy || !wikiNewConcept.trim()}>{wikiBusy ? "…" : "Genera"}</button>
            </div>
            <button class="ghost small wikiall" onclick={wikiGenerateFromTags} disabled={wikiBusy} title="Una pagina per ogni tag con almeno 2 documenti (le esistenti vengono rigenerate)">
              {wikiBusy ? "Genero…" : "Genera/aggiorna dai tag"}
            </button>
            {#if wikiProg}
              <div class="wikiprog">
                <span class="hint">{wikiProg.phase === "estrazione" ? `Leggo le fonti ${wikiProg.done + 1}/${wikiProg.total - 1}` : wikiProg.phase === "sintesi" ? "Scrivo la pagina…" : "Controllo le fonti…"} — {wikiProg.concept}</span>
                <div class="bar"><div class="fill" style="width:{wikiProg.total ? (wikiProg.done / wikiProg.total) * 100 : 5}%"></div></div>
                <button class="ghost small" onclick={stopWiki} title="Ferma al prossimo passaggio">Stop</button>
              </div>
            {/if}
            {#if !aiStat?.enabled}
              <p class="askwarn">Le funzioni AI sono disattivate: abilitale in <strong>Impostazioni → AI locale</strong>.</p>
            {/if}
            <div class="wikilist">
              {#each wikiPages as p (p.slug)}
                <div class="navrow">
                  <button class="navitem" class:active={wikiPage?.slug === p.slug} onclick={() => openWikiPage(p.slug)} title={`${p.n_sources} fonti · generata ${p.generated_at ?? ""}${p.stale ? " · la libreria è cambiata: rigenera" : ""}`}>
                    {p.title}
                    {#if p.stale}<span class="wikistale" title="La libreria è cambiata da quando è stata generata">●</span>{/if}
                  </button>
                  <button class="x" title="Elimina questa pagina" onclick={() => removeWikiPage(p.slug)}>×</button>
                </div>
              {/each}
              {#if !wikiPages.length}
                <p class="wikiempty">Nessuna pagina ancora. Scrivi un concetto qui sopra, o parti da «Genera dai tag».</p>
              {/if}
            </div>
          </aside>
          <section class="wikibody">
            {#if wikiPage}
              <header class="wikihead">
                <h2 class="wikititle">{wikiPage.title}</h2>
                <span class="wikimeta">{wikiPage.sources.length} fonti · {wikiPage.model ?? ""}</span>
                <button class="ghost small" onclick={() => runWikiGenerate(wikiPage!.concept)} disabled={wikiBusy} title="Rigenera la pagina con lo stato attuale della libreria">Rigenera</button>
              </header>
              <article class="wikihtml" use:wikiLinksAction use:mathRender={wikiPage.html}>
                <!-- eslint-disable-next-line svelte/no-at-html-tags -- HTML sanificato dal backend (ammonia) -->
                {@html wikiPage.html}
              </article>
              <div class="wikisources">
                <h3>Fonti</h3>
                {#each wikiPage.sources as s (s.n)}
                  <div class="wikisrc" class:unused={!s.used}>
                    <button class="hflink" onclick={() => openById(s.document_id, s.claims.find((c) => c.page != null)?.page ?? null)} title={s.used ? "Apri il PDF" : "Fonte non utilizzata dalla sintesi — apri comunque"}>
                      [{s.n}] {s.title}{s.year ? ` (${s.year})` : ""}
                    </button>
                    <span class="wikipages">
                      {#each s.claims.filter((c) => c.page != null) as c, ci (ci)}
                        <button class="passchip" onclick={() => openById(s.document_id, c.page)} title={c.text}>p. {c.page}</button>
                      {/each}
                    </span>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="empty wikiintro">
                <p class="big">La tua enciclopedia privata</p>
                <p>Ogni pagina è scritta dall'AI locale leggendo <strong>solo i tuoi documenti</strong>: le citazioni [n] aprono il PDF alla pagina giusta, i concetti si collegano tra loro, e nulla esce dal tuo computer.</p>
                <p class="dimtext">Suggerimento: parti da «Genera/aggiorna dai tag» — una pagina per ciascun tema della tua libreria.</p>
              </div>
            {/if}
          </section>
        </div>
      {:else if filter.kind === "notes"}
        <div class="wikiwrap">
          <aside class="wikinav">
            <div class="wikinew">
              <input
                placeholder="Nuovo appunto: titolo…"
                bind:value={noteNewTitle}
                onkeydown={(e) => e.key === "Enter" && newNote()}
                title="Crea un nuovo appunto .md"
              />
              <button class="ghost small" onclick={newNote} title="Crea l'appunto">Nuovo</button>
            </div>
            <button class="ghost small wikiall" onclick={revealNotesDir} title="Apri la cartella degli appunti (.md) nel file explorer">
              Apri cartella appunti
            </button>
            {#if notesList.length > 1}
              <div class="notesort">
                <label class="notesortlbl" for="notesort-sel">Ordina</label>
                <select id="notesort-sel" class="notesortsel" bind:value={noteSort} title="Ordina l'elenco degli appunti">
                  <option value="updated">Ultima modifica</option>
                  <option value="created">Data creazione</option>
                  <option value="title">Titolo (A→Z)</option>
                </select>
              </div>
            {/if}
            <div class="wikilist">
              {#each notesSorted as n (n.slug)}
                <div class="navrow">
                  <button class="navitem noteitem" class:active={noteView?.slug === n.slug} onclick={() => openNote(n.slug)} title={n.excerpt || n.title}>
                    <span class="notetitle">{n.title}</span>
                    {#if n.excerpt}<span class="noteexc">{n.excerpt}</span>{/if}
                    <span class="notedates" title="Ultima modifica · creazione">mod. {fmtNoteDateShort(n.updated_at)} · creato {fmtNoteDateShort(n.created_at)}</span>
                  </button>
                  <button class="x" title="Elimina questo appunto" onclick={() => removeNote(n.slug)}>×</button>
                </div>
              {/each}
              {#if !notesList.length}
                <p class="wikiempty">Nessun appunto. Scrivi un titolo qui sopra e premi «Nuovo». Gli appunti sono file .md su disco: collega con [[Titolo appunto]] oppure [[@citekey]] per un paper.</p>
              {/if}
            </div>
          </aside>
          <section class="wikibody">
            {#if noteView}
              <header class="wikihead notehead">
                {#if noteRenaming}
                  <input
                    class="noterename"
                    bind:value={noteRenameValue}
                    onkeydown={(e) => { if (e.key === "Enter") commitRename(); else if (e.key === "Escape") cancelRename(); }}
                    onblur={cancelRename}
                    title="Nuovo titolo — Invio per confermare, Esc o clic fuori per annullare (rinomina anche il file .md)"
                    use:focusOnMount
                  />
                {:else}
                  <h2 class="wikititle notetitleh" ondblclick={startRename} title="Doppio clic per rinominare">{noteView.title}</h2>
                {/if}
                <span class="notesaved" class:pending={!noteSaved}>{noteSaved ? "Salvato ✓" : "Salvo…"}</span>
              </header>
              <div class="noteactions">
                <button class="ghost small" disabled={noteRenaming} onclick={startRename} title="Rinomina l'appunto: cambia il titolo e il nome del file">Rinomina</button>
                <div class="notemodes">
                  <button class="ghost small" class:on={noteMode === "edit"} onclick={() => (noteMode = "edit")} title="Modifica il Markdown">Modifica</button>
                  <button class="ghost small" class:on={noteMode === "split"} onclick={() => { noteMode = "split"; renderLivePreview(); }} title="Modifica con anteprima affiancata in tempo reale">Affiancato</button>
                  <button class="ghost small" class:on={noteMode === "preview"} onclick={() => { noteMode = "preview"; refreshNotePreview(); }} title="Anteprima resa (formule, collegamenti, immagini)">Anteprima</button>
                </div>
                <div class="noteexport">
                  <span class="noteexplbl">Esporta</span>
                  <button class="ghost small" onclick={exportNoteMd} title="Salva una copia .md dell'appunto (Markdown puro)">MD</button>
                  <button class="ghost small" onclick={() => exportNoteAs("html")} title="Esporta come pagina HTML autonoma (formule in MathML e immagini incluse)">HTML</button>
                  <button class="ghost small" onclick={() => exportNoteAs("latex")} title="Esporta come documento LaTeX (.tex con le figure estratte in una cartella)">LaTeX</button>
                  <button class="ghost small" onclick={exportNotePdf} title="Apre la stampa dell'appunto reso: scegli «Salva come PDF»">PDF</button>
                </div>
              </div>
              <div class="noteinfo">
                <button class="noteinfopath" onclick={revealNotesDir} title={"Apri la cartella — " + noteView.path}>📁 {noteView.path}</button>
                <span class="noteinfodate" title="Data di creazione del file">creata {fmtNoteDate(noteView.created_at)}</span>
                <span class="noteinfodate" title="Ultima modifica">· modificata {fmtNoteDate(noteView.updated_at)}</span>
              </div>
              {#if noteMode === "edit" || noteMode === "split"}
                <div class="noteedtoolbar" role="toolbar" aria-label="Formattazione">
                  <button onmousedown={keepEditorFocus} onclick={() => mdLinePrefix("# ", true)} title="Titolo (H1)"><b>T</b></button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdLinePrefix("## ", true)} title="Sottotitolo (H2)">T₂</button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdLinePrefix("### ", true)} title="Titolo minore (H3)">T₃</button>
                  <span class="edsep"></span>
                  <button onmousedown={keepEditorFocus} onclick={() => mdWrap("**", "**", "grassetto")} title="Grassetto"><b>B</b></button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdWrap("*", "*", "corsivo")} title="Corsivo"><i>I</i></button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdWrap("`", "`", "codice")} title="Codice inline"><code>‹›</code></button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdWrap("[", "](https://)", "testo")} title="Collegamento">🔗</button>
                  <span class="edsep"></span>
                  <button onmousedown={keepEditorFocus} onclick={() => mdLinePrefix("- ")} title="Elenco puntato">•</button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdLinePrefix("1. ")} title="Elenco numerato">1.</button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdLinePrefix("> ")} title="Citazione">❝</button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdInsert("\n\n$$\n\n$$\n\n")} title="Blocco formula LaTeX ($$…$$)">∑</button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdInsert("\n\n---\n\n")} title="Separatore orizzontale">―</button>
                  <span class="edsep"></span>
                  <button onmousedown={keepEditorFocus} onclick={() => mdMoveBlock(-1)} title="Sposta su il blocco (paragrafo/immagine)">↑</button>
                  <button onmousedown={keepEditorFocus} onclick={() => mdMoveBlock(1)} title="Sposta giù il blocco (paragrafo/immagine)">↓</button>
                </div>
                <div class="noteedwrap" class:split={noteMode === "split"}>
                  <textarea
                    class="noteeditor"
                    bind:this={noteEditorEl}
                    bind:value={noteDraft}
                    oninput={onNoteInput}
                    onblur={flushNote}
                    ondrop={onNoteDrop}
                    ondragover={onNoteDragOver}
                    onpaste={onNotePaste}
                    placeholder={"Scrivi in Markdown…\n\nTrascina o incolla un'immagine per inserirla. Usa [[Titolo di un altro appunto]] per collegare un appunto, [[@citekey]] o [[Titolo del paper]] per un documento. $$ … $$ per una formula."}
                    spellcheck="false"
                  ></textarea>
                  {#if noteMode === "split"}
                    <article class="wikihtml notehtml livepreview" use:mathRender={livePreviewHtml}>
                      <!-- eslint-disable-next-line svelte/no-at-html-tags -- HTML sanificato dal backend (ammonia) -->
                      {@html livePreviewHtml}
                    </article>
                  {/if}
                </div>
              {:else}
                <article class="wikihtml notehtml" use:noteLinksAction use:mathRender={noteView.html}>
                  <!-- eslint-disable-next-line svelte/no-at-html-tags -- HTML sanificato dal backend (ammonia) -->
                  {@html noteView.html}
                </article>
                {#if noteView.backlinks.length}
                  <div class="notebacklinks">
                    <h3>Collegato da</h3>
                    {#each noteView.backlinks as b (b.slug)}
                      <button class="passchip" onclick={() => openNote(b.slug)} title={`Apri «${b.title}»`}>{b.title}</button>
                    {/each}
                  </div>
                {/if}
              {/if}
            {:else}
              <div class="empty wikiintro">
                <p class="big">I tuoi appunti</p>
                <p>Appunti in <strong>Markdown</strong>, salvati come <strong>file .md veri</strong> nella cartella degli appunti — restano tuoi, leggibili e modificabili anche da terminale o da qualsiasi editor.</p>
                <p class="dimtext">Collega con <code>[[Titolo appunto]]</code> o un paper con <code>[[@citekey]]</code> / <code>[[Titolo del paper]]</code>. I backlink compaiono in fondo a ogni appunto.</p>
                <p class="dimtext">Scrivi <code>$$ … $$</code> per una <strong>formula</strong> (resa in anteprima), usa la barra di formattazione (grassetto, titoli, liste, sposta blocchi), <strong>trascina o incolla immagini</strong> (salvate come file in <code>assets/</code>, nell'appunto resta solo un riferimento breve), ed <strong>esporta</strong> in <strong>HTML</strong>, <strong>LaTeX</strong> o <strong>PDF</strong> con formule e figure incluse.</p>
              </div>
            {/if}
          </section>
        </div>
      {:else if filter.kind === "projects"}
        <TexProjects openSlug={projectsOpenSlug} />
      {:else if filter.kind === "archivio"}
        <Archivio
          onOpenGrid={(id, label) => setFilter({ kind: "collection", id, label })}
          onChanged={() => void loadSidebar()}
        />
      {:else if filter.kind === "novita"}
        <div class="novhead">
          <div class="novtitle">
            <h2>Novità</h2>
            <span class="novsub">Nuovi paper sui temi che segui — raccolti a ogni avvio dalle tue ricerche salvate</span>
          </div>
          <button class="ghost small" onclick={sweepNovitaNow} disabled={novitaSweeping} title="Cerca subito nuovi paper per tutte le ricerche salvate">
            {novitaSweeping ? "Cerco…" : "↻ Cerca ora"}
          </button>
        </div>
        {#if novitaLoading}
          <div class="empty"><p>Carico le novità…</p></div>
        {:else if !novitaGroups.length}
          <div class="empty novintro">
            <p class="big">Nessuna novità al momento</p>
            {#if savedSearches.length}
              <p>Le tue {savedSearches.length} ricerche salvate vengono ricontrollate a ogni avvio. Quando esce qualcosa di nuovo lo trovi qui, pronto da aggiungere con un click.</p>
              <p class="dimtext">Puoi forzare il controllo adesso con «↻ Cerca ora».</p>
            {:else}
              <p>Salva una ricerca da <strong>Cerca online</strong> (★ Salva) per iniziare a monitorare un tema: le novità compariranno qui.</p>
            {/if}
          </div>
        {:else}
          <div class="novfeed">
            {#each novitaGroups as g (g.watch_id)}
              <section class="novgroup">
                <header class="novgh">
                  <span class="novgname">{g.watch_name}</span>
                  <span class="novgcount">{g.hits.length}</span>
                  <button class="hflink small" onclick={() => ignoreAllNovita(g.watch_id)} title="Segna tutte come lette (le rimuove dalle novità)">segna tutte lette</button>
                </header>
                {#each g.hits as h (h.hit_id)}
                  {@const r = h.result}
                  {@const hid = "nov" + h.hit_id}
                  <article class="novcard">
                    <div class="novmain">
                      <div class="novtl">
                        {#if r.abstract_text}
                          <button class="abstoggle" class:open={expandedAbstract === hid} onclick={() => toggleAbstract(hid)} title={expandedAbstract === hid ? "Nascondi abstract" : "Mostra abstract"} aria-label="Mostra/nascondi abstract">▸</button>
                        {/if}
                        <span class="novt" title={r.title ?? ""}>{r.title ?? "Senza titolo"}</span>
                        {#if r.pub_status}<span class="badgeinline">{@render pubBadge(r.pub_status, r.url)}</span>{/if}
                        {#if r.in_library}<span class="nuovo inlibtag" title="Già presente in libreria">in libreria</span>{/if}
                      </div>
                      <div class="novmeta">
                        {r.authors.slice(0, 4).join(", ")}{r.authors.length > 4 ? " et al." : ""}{r.year ? ` · ${r.year}` : ""}{r.venue ? ` · ${r.venue}` : ""}
                      </div>
                      {#if expandedAbstract === hid && r.abstract_text}
                        <p class="novabs">{r.abstract_text}</p>
                      {/if}
                    </div>
                    <div class="novact">
                      {#if r.in_library}
                        <button class="ghost small" onclick={() => ignoreNovita(g.watch_id, h.hit_id)} title="Rimuovi dalle novità">Ignora</button>
                      {:else}
                        <button class="ghost small primary" onclick={() => acceptNovita(g.watch_id, h.hit_id)} disabled={acceptingHit === h.hit_id} title="Aggiungi alla libreria (scarica il PDF se Open Access)">{acceptingHit === h.hit_id ? "…" : "Aggiungi"}</button>
                        <button class="hflink small" onclick={() => ignoreNovita(g.watch_id, h.hit_id)} title="Rimuovi dalle novità">Ignora</button>
                      {/if}
                    </div>
                  </article>
                {/each}
              </section>
            {/each}
          </div>
        {/if}
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
            <option value="huggingface">HF Papers (ex Papers with Code)</option>
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
            <p>Fonti: arXiv (preprint STEM), OpenAlex (tutto), ADS (astrofisica), Semantic Scholar (citazioni), Europe PMC (biomedicina), CORE (full-text OA), DOAJ (riviste OA), HF Papers (paper con codice — il successore di Papers with Code). I PDF si scaricano solo se Open Access; gli altri si aggiungono come riferimento.</p>
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
                          : filter.kind === "mywork"
                            ? "Il mio lavoro"
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
          <div class="floatpill" role="toolbar" aria-label="Azioni sulla selezione">
            <span class="pillcount">{selected.length} selezionati</span>
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
            {#if aiStat?.enabled}
              <button onclick={() => runBatchAi("summary")} disabled={aiBusyAny} title="Genera un riassunto AI per ogni documento selezionato">{aiBatch?.kind === "summary" ? `Riassunto ${aiBatch.done}/${aiBatch.total}…` : "Riassumi (AI)"}</button>
              <button onclick={() => runBatchAi("tags")} disabled={aiBusyAny} title="Genera tag automatici AI per ogni documento selezionato">{aiBatch?.kind === "tags" ? `Tag ${aiBatch.done}/${aiBatch.total}…` : "Tag automatici (AI)"}</button>
            {/if}
            <select title="Aggiungi un tag ai selezionati" onchange={(e) => { const t = tags.find((x) => x.id === +e.currentTarget.value); if (t) bulkAddTag(t); e.currentTarget.value = ""; }}>
              <option value="">+ Tag…</option>
              {#each tags as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
            </select>
            <select title="Aggiungi i selezionati a una collezione" onchange={(e) => { const c = collections.find((x) => x.id === +e.currentTarget.value); if (c) bulkAddCollection(c); e.currentTarget.value = ""; }}>
              <option value="">+ Collezione…</option>
              {#each collections.filter((c) => !c.is_smart) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
            </select>
            <button class="del" onclick={() => trashSelected(selected)} title="Sposta i selezionati nel cestino">Elimina</button>
            <button class="pillx" onclick={() => (selected = [])} title="Annulla la selezione" aria-label="Deseleziona">✕</button>
          </div>
        {/if}
        </div>
        {#if filter.kind === "all" && view !== "map" && !query.trim() && !tagFilter.length && docs.length}
          <section class="home" class:collapsed={homeCollapsed}>
            <div class="homehead">
              <button class="homefold" onclick={toggleHome} title={homeCollapsed ? "Mostra la home" : "Comprimi la home"} aria-expanded={!homeCollapsed}>
                <span class="homechev">{homeCollapsed ? "▸" : "▾"}</span> Panoramica
              </button>
              {#if !homeCollapsed}
                <div class="homestats">
                  <button class="hstat" onclick={() => setFilter({ kind: "unread" })} title="Vai ai documenti da leggere">
                    <span class="hnum">{unreadCount}</span><span class="hlab">da leggere</span>
                  </button>
                  <span class="hstat" title="Documenti aperti ma non ancora finiti">
                    <span class="hnum">{readingCount}</span><span class="hlab">in lettura</span>
                  </span>
                  <span class="hstat" title="Aggiunti alla libreria questo mese">
                    <span class="hnum">{addedThisMonth}</span><span class="hlab">questo mese</span>
                  </span>
                </div>
              {/if}
            </div>
            {#if !homeCollapsed && rediscoverPick}
              {@const rd = rediscoverPick}
              <div class="rediscover">
                <div class="rdlabel">
                  Riscopri
                  <button class="rdshuffle" onclick={() => (rediscoverTick += 1)} title="Un altro paper a caso" aria-label="Un altro">↻</button>
                </div>
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div class="rdcard" role="button" tabindex="0" title={rd.title ?? "Senza titolo"} onclick={() => openDocument(rd)} oncontextmenu={(e) => onContext(e, rd)} onkeydown={(e) => { if (e.key === "Enter") openDocument(rd); }}>
                  <div class="rdthumb">
                    {#if thumbs[rd.id]}<img src={thumbs[rd.id]} alt="" />{:else}<div class="thumb-placeholder">PDF</div>{/if}
                  </div>
                  <div class="rdmeta">
                    <span class="rdtitle">{rd.title ?? "Senza titolo"}</span>
                    <span class="rdsub">{[authorLine(rd), rd.year].filter(Boolean).join(" · ")}</span>
                  </div>
                </div>
              </div>
            {/if}
          </section>
        {/if}
        {#if filter.kind === "all" && view !== "map" && !query.trim() && !tagFilter.length && recentDocs.length}
          <section class="recentshelf">
            <h2 class="shelfh">Continua a leggere</h2>
            <div class="shelf">
              {#each recentDocs as d (d.id)}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div class="shelfcard" role="button" tabindex="0" title={d.title ?? "Senza titolo"} onclick={() => openDocument(d)} oncontextmenu={(e) => onContext(e, d)} onkeydown={(e) => { if (e.key === "Enter") openDocument(d); }}>
                  <div class="shelfthumb">
                    {#if thumbs[d.id]}<img src={thumbs[d.id]} alt="" />{:else}<div class="thumb-placeholder">PDF</div>{/if}
                  </div>
                  <span class="shelftitle">{d.title ?? "Senza titolo"}</span>
                </div>
              {/each}
            </div>
          </section>
        {/if}
        {#if query.trim() && noteResults.length && view !== "map"}
          <section class="noteresults">
            <h2 class="shelfh">Appunti ({noteResults.length})</h2>
            <div class="notehits">
              {#each noteResults as n (n.slug)}
                <button class="notehit" onclick={() => openNoteHit(n.slug)} title="Apri l'appunto">
                  <span class="nhtitle">{n.title}</span>
                  {#if n.snippet}<span class="nhsnip">{n.snippet}</span>{/if}
                </button>
              {/each}
            </div>
          </section>
        {/if}
        {#if view === "map"}
          <div class="mapwrap">
            <Constellation
              {graph}
              loading={graphLoading}
              paused={openDoc !== null}
              {selected}
              onOpen={(id) => {
                if (id < 0) {
                  // A note node: negative id carries the vault slug.
                  const slug = graph?.nodes.find((n) => n.id === id)?.slug;
                  if (slug) openNoteHit(slug);
                  return;
                }
                openById(id);
              }}
              onContext={async (e, id) => {
                if (id < 0) return; // notes have no document radial
                let d = displayed.find((x) => x.id === id) ?? docs.find((x) => x.id === id) ?? recentDocs.find((x) => x.id === id);
                if (!d) {
                  // Node outside the current filter: fetch it so the radial menu
                  // works on every star, not just the docs already loaded.
                  try {
                    d = (await listDocuments()).find((x) => x.id === id);
                  } catch {
                    /* ignore */
                  }
                }
                if (d) openRadialDoc(e, d);
              }}
              onToggleSelect={(id) => { if (id >= 0) toggleSelect(id); }}
              onGenerate={() => generateIndex()}
              onRefresh={() => loadGraph(true)}
              params={{ k: graphK, minSim: graphMinSim }}
              onParams={(k, minSim) => setGraphParams(k, minSim)}
              onSavePositions={(pos) => saveGraphPositions(pos).catch(() => {})}
              ghosts={mapGhosts}
              onExplore={(id, rel) => exploreFromNode(id, rel)}
              onGhostAdd={(key) => addGhostToLibrary(key)}
              onGhostExplore={(key, rel) => exploreFromGhost(key, rel)}
              onGhostsClear={() => (mapGhosts = [])}
              resolve={(id) => {
                const d = displayed.find((x) => x.id === id) ?? docs.find((x) => x.id === id) ?? recentDocs.find((x) => x.id === id);
                return d ? { authors: d.authors, venue: d.venue, tags: d.tags.map((t) => ({ name: t.name, color: t.color })) } : undefined;
              }}
            />
          </div>
        {:else if displayed.length === 0}
          <div class="empty">
            {#if query.trim()}
              {#if noteResults.length}
                <p class="big">Nessun documento</p><p>Ma {noteResults.length === 1 ? "c'è 1 appunto" : `ci sono ${noteResults.length} appunti`} qui sopra che corrispond{noteResults.length === 1 ? "e" : "ono"}.</p>
              {:else}
                <p class="big">Nessun risultato</p><p>Prova un'altra ricerca o cambia modalità.</p>
              {/if}
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
              <article class="card" class:selcard={selectedSet.has(d.id)} class:kfocus={focusId === d.id} role="button" tabindex="0" onclick={() => focusCard(d)} ondblclick={() => openDocument(d)} oncontextmenu={(e) => onContext(e, d)} onkeydown={(e) => { if (e.key === "Enter") openDocument(d); }}>
                <button class="dots" title="Altre azioni (anche col tasto destro)" onclick={(e) => openCardMenu(e, d)}>⋯</button>
                <button class="cardsel" class:on={selectedSet.has(d.id)} title="Seleziona per azioni multiple" aria-label="Seleziona" onclick={(e) => { e.stopPropagation(); toggleSelect(d.id); }}>{selectedSet.has(d.id) ? "✓" : ""}</button>
                <button class="starbtn" class:on={d.favorite} title={d.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti"} aria-label="Preferito" onclick={(e) => { e.stopPropagation(); toggleFavorite(d); }}>{d.favorite ? "★" : "☆"}</button>
                <div class="thumb">
                  {#if thumbs[d.id]}<img src={thumbs[d.id]} alt="" />{:else}<div class="thumb-placeholder" class:refonly={!d.has_file}>{d.has_file ? "PDF" : "Riferimento — senza PDF"}</div>{/if}
                  {#if readPct(d) !== null}
                    {@const pct = readPct(d)}
                    <div class="progress" class:done={d.is_read} title={d.is_read ? "Letto" : `Letto al ${pct}%${d.page_count ? ` (pag. ${d.last_page}/${d.page_count})` : ""}`}>
                      <div class="pfill" style="width:{pct}%"></div>
                    </div>
                  {/if}
                </div>
                <div class="meta">
                  <h3 title={d.title ?? ""}>{d.title ?? "Senza titolo"}</h3>
                  {#if authorLine(d)}<p class="authors"><button type="button" class="authorlink" title={`Mostra tutti i lavori di ${d.authors[0]}`} onclick={(e) => { e.stopPropagation(); showAuthor(d.authors[0]); }}>{authorLine(d)}</button></p>{/if}
                  {#if d.year || d.venue}<p class="venue">{[d.venue, d.year].filter(Boolean).join(" · ")}</p>{/if}
                  {#if d.citekey && !isBare(d)}<button type="button" class="ckey" title={`Citekey: ${d.citekey} — clic per copiare`} aria-label={`Copia citekey ${d.citekey}`} onclick={(e) => { e.stopPropagation(); copyCitekey(d); }}>{d.citekey}</button>{/if}
                  {#if d.has_summary}<span class="aisum" title="Riassunto AI già presente — lo trovi in «Modifica metadati» (il batch AI salta questo documento)">✦ AI</span>{/if}
                  {#if isBare(d)}<p class="metamiss" title="Autori, anno e rivista non ancora recuperati. Premi «Metadati» (in alto) per recuperarli da Crossref.">ⓘ metadati non recuperati</p>{/if}
                  {#if d.pub_status}<div class="badgerow">{@render pubBadge(d.pub_status, d.paper_url)}</div>{/if}
                  {#if d.github_url}
                    <button class="ghchip" title={`Apri il repository GitHub: ${d.github_url}`} aria-label="Apri repository GitHub" onclick={(e) => { e.stopPropagation(); openInBrowser(d.github_url!); }}>{@render githubMark()} codice</button>
                  {/if}
                  {#if d.tags.length}
                    <div class="chips">
                      {#each d.tags as t (t.id)}<span role="button" tabindex="0" class="chip chipsel" class:on={tagFilter.includes(t.id)} style="background:{(t.color ?? '#888')}33; border-color:{t.color ?? '#888'}" title={`Filtra: mostra solo i paper col tag «${t.name}» (clicca altri tag per restringere; ri-clic per togliere)`} onclick={(e) => { e.stopPropagation(); toggleTagFilter(t.id); }} onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); toggleTagFilter(t.id); } }}>{#if tagFilter.includes(t.id)}✓ {/if}{t.name}</span>{/each}
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
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => cycleSort("venue")} title="Ordina per rivista (clicca di nuovo per invertire, ancora per togliere)">Rivista<span class="ar">{sortArrow("venue")}</span>{#if sortChain.length > 1 && sortRank("venue")}<span class="ar rnk">{sortRank("venue")}</span>{/if}</th>
                  <th>Tag</th>
                  <th>DOI</th>
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <th class="sortable" onclick={() => cycleSort("added")} title="Ordina per data di aggiunta (clicca di nuovo per invertire, ancora per togliere)">Aggiunto<span class="ar">{sortArrow("added")}</span>{#if sortChain.length > 1 && sortRank("added")}<span class="ar rnk">{sortRank("added")}</span>{/if}</th>
                </tr>
              </thead>
              <tbody>
                {#each displayed as d (d.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                  <tr onclick={() => focusCard(d)} ondblclick={() => openDocument(d)} oncontextmenu={(e) => onContext(e, d)} class:selrow={selectedSet.has(d.id)} class:kfocus={focusId === d.id}>
                    <td class="sel"><input type="checkbox" checked={selectedSet.has(d.id)} onclick={(e) => e.stopPropagation()} onchange={() => toggleSelect(d.id)} title="Seleziona" /></td>
                    <td class="ttl" title={d.title ?? ""}><button class="starinline" class:on={d.favorite} title={d.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti"} aria-label="Preferito" onclick={(e) => { e.stopPropagation(); toggleFavorite(d); }}>{d.favorite ? "★" : "☆"}</button>{d.title ?? "Senza titolo"}{#if d.github_url}<button class="ghicon" title={`Apri il repository GitHub: ${d.github_url}`} aria-label="Apri repository GitHub" onclick={(e) => { e.stopPropagation(); openInBrowser(d.github_url!); }}>{@render githubMark()}</button>{/if}{#if d.citekey && !isBare(d)}<button type="button" class="ckey-inline" title={`Citekey: ${d.citekey} — clic per copiare`} aria-label={`Copia citekey ${d.citekey}`} onclick={(e) => { e.stopPropagation(); copyCitekey(d); }}>{d.citekey}</button>{/if}{#if d.has_summary}<span class="aisum inline" title="Riassunto AI già presente (il batch AI salta questo documento)">✦</span>{/if}{#if isBare(d)}<span class="metamiss-inline" title="Autori, anno e rivista non ancora recuperati. Premi «Metadati» (in alto) per recuperarli da Crossref.">ⓘ</span>{/if}</td>
                    <td class="dim" title={authorLine(d)}>{#if authorLine(d)}<button type="button" class="authorlink" title={`Mostra tutti i lavori di ${d.authors[0]}`} onclick={(e) => { e.stopPropagation(); showAuthor(d.authors[0]); }}>{authorLine(d)}</button>{:else}—{/if}</td>
                    <td class="num dim">{d.year ?? "—"}</td>
                    <td class="dim" title={d.venue ?? ""}>{d.venue || "—"}{#if d.pub_status}<span class="badgeinline">{@render pubBadge(d.pub_status, d.paper_url)}</span>{/if}</td>
                    <td>
                      <div class="tagcell">
                        {#each d.tags.slice(0, 2) as t (t.id)}<span role="button" tabindex="0" class="chip chipsel" class:on={tagFilter.includes(t.id)} style="background:{(t.color ?? '#888')}33; border-color:{t.color ?? '#888'}" title={`Filtra: mostra solo i paper col tag «${t.name}» (clicca altri tag per restringere; ri-clic per togliere)`} onclick={(e) => { e.stopPropagation(); toggleTagFilter(t.id); }} onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); toggleTagFilter(t.id); } }}>{#if tagFilter.includes(t.id)}✓ {/if}{t.name}</span>{/each}
                        {#if d.tags.length > 2}<span class="more">+{d.tags.length - 2}</span>{/if}
                      </div>
                    </td>
                    <td class="doi dim" title={d.doi ?? ""}>{d.doi || "—"}</td>
                    <td class="num dim">{(d.added_at ?? "").slice(0, 10)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/if}
    </main>

    {#if panelDoc}
      <DetailPanel
        doc={panelDoc}
        {tags}
        aiEnabled={!!aiStat?.enabled}
        aiBusy={aiBusyAny}
        thumb={thumbs[panelDoc.id] ?? null}
        tagColors={PALETTE}
        onOpen={() => openDocument(panelDoc!)}
        onClose={() => (focusId = null)}
        onRadial={(e) => openRadialDoc(e, panelDoc!)}
        onAuthor={(name) => showAuthor(name)}
        onFavorite={() => toggleFavorite(panelDoc!)}
        onRead={() => toggleRead(panelDoc!)}
        onCitations={() => openCitations(panelDoc!)}
        onAttach={() => (refPanel = { doc: panelDoc!, url: "", busy: false })}
        onSummarize={() => summarizeDoc(panelDoc!)}
        onChanged={async () => {
          await loadDocs();
          await loadSidebar();
        }}
        onSendToNote={(p, ev) => { ev.stopPropagation(); openSendToNote(panelDoc, p, { x: ev.clientX, y: ev.clientY }); }}
      />
    {/if}
  </div>

  {#if dragOver}<div class="dropmask"><span>{noteEditorActive() ? "Rilascia l'immagine per inserirla nell'appunto (o un PDF per importarlo)" : "Rilascia i PDF per importarli"}</span></div>{/if}

  {#if showCoach}
    <div class="coach" role="dialog" aria-label="Suggerimento iniziale">
      <div class="coachh">Benvenuto in Scriptorium</div>
      <p class="coachp">Tre modi per arrivare a tutto:</p>
      <ul class="coachlist">
        <li>La <strong>barra in alto</strong>: un'icona per ogni gruppo di funzioni (Importa, Cerca, Novità, Strumenti, Aspetto, Sistema…).</li>
        <li><strong>Tasto destro</strong> su un documento (o nel vuoto) → menu <strong>radiale</strong> con ogni azione.</li>
        <li><kbd>Ctrl</kbd>+<kbd>K</kbd> → la <strong>palette</strong>: cerca qualsiasi comando scrivendo.</li>
        <li><strong>Un click</strong> apre il pannello di dettaglio, <strong>doppio click</strong> legge.</li>
      </ul>
      <div class="coachact">
        <button class="ghost small" onclick={() => { dismissCoach(); openHelp(); }}>Guida completa</button>
        <button class="primary small" onclick={dismissCoach}>Ho capito</button>
      </div>
    </div>
  {/if}

  {#if openDoc}
    <!-- keyed: scegliendo un ALTRO documento (es. dalla palette) mentre il
         lettore è aperto, il viewer si rimonta e carica il PDF giusto -->
    {#key openDoc.id}
    <Viewer
      id={openDoc.id}
      title={openDoc.title ?? "PDF"}
      link={paperLink(openDoc)}
      aiEnabled={!!aiStat?.enabled}
      initialPage={openDocPage}
      onClose={() => { openDoc = null; openDocPage = null; }}
      onSendToNote={(content, page, pos, opts) => openSendToNote(openDoc, { content, page, collapse: !opts?.code && !opts?.raw, label: opts?.label, code: opts?.code, raw: opts?.raw }, pos)}
      onOpenNotes={() => { openDoc = null; openDocPage = null; openNotesView(); }}
    />
    {/key}
  {/if}

  {#if sendNote}
    <SendToNotePicker
      payload={sendNote.payload}
      pos={sendNote.pos}
      currentNote={sendNoteCurrent}
      onstatus={(s) => (status = s)}
      onclose={() => (sendNote = null)}
      ondone={afterSendToNote}
    />
  {/if}

  {#if toolMenu}
    {@const grp = buildGlobalRadial().find((g) => g.id === toolMenu?.id)}
    <div class="menu toolmenu" use:clamp={{ x: toolMenu.x, y: toolMenu.y }}>
      {#if grp}<div class="mtitle">{grp.label}</div>{/if}
      {#each grp?.children ?? [] as it (it.id)}
        {#if it.children && it.children.length}
          <div class="menusec">{it.label}</div>
          {#each it.children as ch (ch.id)}
            <button class="medit sub" class:danger={ch.danger} disabled={ch.disabled} title={ch.hint ?? null} onclick={() => runTool(ch)}>
              <span>{ch.label}</span>
              {#if ch.checked}<span class="mtick">✓</span>{/if}
              {#if ch.badge}<span class="mbadge">{ch.badge}</span>{/if}
            </button>
          {/each}
        {:else}
          <button class="medit" class:danger={it.danger} disabled={it.disabled} title={it.hint ?? null} onclick={() => runTool(it)}>
            <span>{it.label}</span>
            {#if it.checked}<span class="mtick">✓</span>{/if}
            {#if it.badge}<span class="mbadge">{it.badge}</span>{/if}
          </button>
        {/if}
      {/each}
    </div>
  {/if}

  {#if radial}
    {#key radial}
      <RadialMenu
        x={radial.x}
        y={radial.y}
        items={radial.items}
        title={radial.title}
        subtitle={radial.subtitle}
        thumb={radial.thumb}
        onclose={() => (radial = null)}
      />
    {/key}
  {/if}

  {#if paletteOpen}
    <CommandPalette entries={paletteEntries()} onclose={() => (paletteOpen = false)} />
  {/if}

  {#if tagPanel}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="menu flyout" use:clamp={{ x: tagPanel.x, y: tagPanel.y }} onclick={(e) => e.stopPropagation()}>
      <div class="mtitle">Tag — {tagPanel.doc.title ?? "Senza titolo"}</div>
      {#each tags as t (t.id)}
        <label class="mtag">
          <input type="checkbox" checked={tagPanel.doc.tags.some((x) => x.id === t.id)} onchange={() => toggleTag(tagPanel!.doc, t)} />
          <span class="dot" style="background:{t.color ?? '#888'}"></span>{t.name}
        </label>
      {/each}
      <div class="mnew">
        <input placeholder="nuovo tag…" bind:value={newTagName} onkeydown={(e) => e.key === "Enter" && makeTagAndAssign(tagPanel!.doc)} />
        <button class="ghost small" onclick={() => makeTagAndAssign(tagPanel!.doc)}>+</button>
      </div>
      <button class="mdone" onclick={() => (tagPanel = null)}>Fatto</button>
    </div>
  {/if}

  {#if collPanel}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="menu flyout" use:clamp={{ x: collPanel.x, y: collPanel.y }} onclick={(e) => e.stopPropagation()}>
      <div class="mtitle">Aggiungi a — {collPanel.doc.title ?? "Senza titolo"}</div>
      {#if collections.filter((c) => !c.is_smart).length}
        {#each collections.filter((c) => !c.is_smart) as c (c.id)}
          <button class="mcoll" onclick={() => { addDocToCollection(collPanel!.doc, c); collPanel = null; }}>{c.name}</button>
        {/each}
      {:else}
        <p class="mempty">Nessuna collezione: creane una dalla barra laterale.</p>
      {/if}
      <button class="mdone" onclick={() => (collPanel = null)}>Chiudi</button>
    </div>
  {/if}

  {#if aiDoc}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) aiDoc = null; }} role="presentation">
      <div class="idmodal aidocmodal" role="dialog" tabindex="-1" aria-label={aiDoc.title} onclick={(e) => e.stopPropagation()}>
        <h2>{aiDoc.title}</h2>
        <article class="wikihtml aidochtml" use:aiDocLinksAction use:mathRender={aiDoc.html}>
          <!-- HTML sanificato dal backend (ammonia) -->
          {@html aiDoc.html}
        </article>
        <div class="aidocsrc">
          {#each aiDoc.sources as s (s.n)}
            <button class="hflink small" onclick={() => openById(s.document_id)} title="Apri il documento">[{s.n}] {s.title}{s.year ? ` (${s.year})` : ""}</button>
          {/each}
        </div>
        <div class="modactions">
          <button class="ghost small" onclick={() => copyAiDoc("md")}>Copia Markdown</button>
          {#if aiDoc.kind === "Rassegna"}
            <button class="ghost small" onclick={() => copyAiDoc("latex")} title={"Le [n] diventano \\cite{citekey}"}>Copia per LaTeX</button>
            <button class="ghost small" onclick={() => copyAiDoc("pandoc")} title="Le [n] diventano [@citekey]">Copia per Pandoc</button>
          {/if}
          <button class="ghost small" onclick={aiDocToNote} title="Crea un appunto .md, con le fonti come backlink [[@citekey]] cliccabili">📝 Salva negli Appunti</button>
          <button class="ghost small" onclick={saveAiDoc}>Salva .md…</button>
          <button class="primary" onclick={() => (aiDoc = null)}>Chiudi</button>
        </div>
      </div>
    </div>
  {/if}

  {#if resultsGrid}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) resultsGrid = null; }} role="presentation">
      <div class="idmodal gridmodal" role="dialog" tabindex="-1" aria-label="Tabella risultati" onclick={(e) => e.stopPropagation()}>
        <h2>Risultati raccolti dai paper</h2>
        <p class="dimtext">Valori estratti testualmente dai documenti selezionati (verifica sempre sul PDF: clic sul paper per aprirlo).</p>
        <div class="gridwrap">
          <table class="resgrid">
            <thead><tr>{#each resultsGrid[0] as h, hi (hi)}<th>{h}</th>{/each}</tr></thead>
            <tbody>
              {#each resultsGrid.slice(1) as row, ri (ri)}
                <tr>{#each row as cell, ci (ci)}<td>{cell}</td>{/each}</tr>
              {/each}
            </tbody>
          </table>
        </div>
        <div class="modactions">
          <button class="ghost small" onclick={() => exportResults("csv")}>CSV…</button>
          <button class="ghost small" onclick={() => exportResults("md")}>Markdown…</button>
          <button class="ghost small" onclick={() => exportResults("xlsx")}>Excel…</button>
          <button class="primary" onclick={() => (resultsGrid = null)}>Chiudi</button>
        </div>
      </div>
    </div>
  {/if}

  {#if pathModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) pathModal = null; }} role="presentation">
      <div class="idmodal pathmodal" role="dialog" tabindex="-1" aria-label="Percorso di lettura" onclick={(e) => e.stopPropagation()}>
        <h2>Percorso di lettura</h2>
        <p class="dimtext">Per capire «{pathModal.doc.title ?? "questo paper"}», in ordine consigliato: prima i fondamenti che cita, poi i vicini di contenuto già tuoi, infine i riferimenti che ancora non possiedi.</p>
        <ol class="pathlist">
          {#each pathModal.steps as step, si (si)}
            <li class="pathstep" class:ext={!step.in_library}>
              <div class="pathmain">
                {#if step.document_id != null}
                  <button class="hflink" onclick={() => openById(step.document_id!)} title="Apri il documento">{step.title}{step.year ? ` (${step.year})` : ""}</button>
                {:else}
                  <span class="pathtitle">{step.title}</span>
                  {#if step.doi}
                    <button class="hflink small" onclick={() => openInBrowser(`https://doi.org/${step.doi}`)} title="Apri il DOI nel browser">DOI ↗</button>
                    <button class="ghost small" onclick={() => addPathStep(step)} title="Aggiungi come riferimento alla libreria">+ Aggiungi</button>
                  {/if}
                {/if}
              </div>
              <span class="pathwhy">{step.reason}</span>
            </li>
          {/each}
        </ol>
        <div class="modactions"><button class="primary" onclick={() => (pathModal = null)}>Chiudi</button></div>
      </div>
    </div>
  {/if}

  {#if wikiFromSel}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) wikiFromSel = null; }} role="presentation">
      <div class="idmodal wikiselmodal" role="dialog" tabindex="-1" aria-label="Pagina wiki dalla selezione" onclick={(e) => e.stopPropagation()}>
        <h2>Pagina wiki dalla selezione</h2>
        <p class="dimtext">
          La pagina userà come fonti <strong>esattamente</strong> {wikiFromSel.ids.length === 1 ? "il documento selezionato" : `i ${wikiFromSel.ids.length} documenti selezionati`}
          {selected.length > 10 ? " (i primi 10)" : ""} — ognuno dovrà comparire nel testo, o essere dichiarato non pertinente.
        </p>
        <div class="refurl">
          <!-- svelte-ignore a11y_autofocus -->
          <input
            placeholder="Titolo del concetto (es. «ragionamento negli LLM»)…"
            bind:value={wikiFromSel.concept}
            autofocus
            onkeydown={(e) => e.key === "Enter" && runWikiFromSelection()}
          />
          <button class="primary" onclick={runWikiFromSelection} disabled={wikiBusy || !wikiFromSel.concept.trim()}>{wikiBusy ? "…" : "Genera"}</button>
          <button class="ghost small" onclick={() => (wikiFromSel = null)}>Annulla</button>
        </div>
        <p class="refhint">Se esiste già una pagina con lo stesso titolo verrà sostituita. La trovi poi in «Wiki della libreria».</p>
      </div>
    </div>
  {/if}

  {#if refPanel}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget && !refPanel!.busy) refPanel = null; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
      <div class="idmodal refmodal" role="dialog" tabindex="-1" aria-label="Riferimento senza PDF" onclick={(e) => e.stopPropagation()}>
        <h2>Riferimento senza PDF</h2>
        <p class="dimtext">
          «{refPanel.doc.title ?? "Senza titolo"}» è in libreria come <strong>citazione</strong>: non ha ancora un file
          allegato (quando è stato aggiunto non c'era un PDF Open Access scaricabile).
        </p>
        <div class="refactions">
          <button class="primary" onclick={refFindPdf} disabled={refPanel.busy} title="Cerca i candidati online — arXiv, Unpaywall, OpenAlex, Semantic Scholar, Crossref, per identificativo e per titolo — e scegli tu quale scaricare e allegare">
            {refPanel.busy ? "…" : "Trova PDF…"}
          </button>
          <button class="ghost" onclick={() => (editingId = refPanel!.doc.id)}>Modifica metadati</button>
          <button class="ghost" onclick={() => (refPanel = null)}>Chiudi</button>
        </div>
        <div class="refor">…oppure allega tu un link al PDF:</div>
        <div class="refurl">
          <input
            placeholder="https://…/file.pdf (vanno bene anche le pagine GitHub /blob/)"
            bind:value={refPanel.url}
            onkeydown={(e) => e.key === "Enter" && refAttach()}
          />
          <button class="ghost small" onclick={refPaste} title="Incolla il link dagli appunti">📋</button>
          <button class="ghost small" onclick={refAttach} disabled={refPanel.busy || !refPanel.url.trim()}>{refPanel.busy ? "…" : "Allega"}</button>
        </div>
        <p class="refhint">Il file viene scaricato e allegato a <strong>questa</strong> voce — tag, citazioni e metadati restano; nessun duplicato.</p>
      </div>
    </div>
  {/if}

  {#if spotlight}
    {@const sd = spotlight.doc}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) spotlight = null; }} role="presentation">
      <div class="spotcard" role="dialog" aria-label="Riscopri">
        <p class="spotkicker">Riscopri</p>
        <div class="spotbody">
          <div class="spotthumb">
            {#if thumbs[sd.id]}<img src={thumbs[sd.id]} alt="" />{:else}<div class="thumb-placeholder" class:refonly={!sd.has_file}>{sd.has_file ? "PDF" : "Riferimento"}</div>{/if}
          </div>
          <div class="spotmeta">
            <h2 class="spottitle">{sd.title ?? "Senza titolo"}</h2>
            {#if authorLine(sd)}<p class="spotauthors">{authorLine(sd)}</p>{/if}
            {#if sd.year || sd.venue}<p class="spotvenue">{[sd.venue, sd.year].filter(Boolean).join(" · ")}</p>{/if}
            {#if spotlight.blurb}<p class="spotblurb">{spotlight.blurb}</p>{/if}
          </div>
        </div>
        <div class="spotactions">
          <button class="primary" onclick={() => { openDocument(sd); spotlight = null; }}>Leggi ora</button>
          <button class="ghost" onclick={() => rediscover()}>Un altro</button>
          <button class="ghost" onclick={() => (spotlight = null)}>Chiudi</button>
        </div>
      </div>
    </div>
  {/if}

  <div class="toasts" aria-live="polite">
    {#if wikiProg && filter.kind !== "wiki"}
      <div class="toast">
        <span>{wikiProg.concept}: {wikiProg.phase === "estrazione" ? `leggo le fonti ${wikiProg.done + 1}/${Math.max(wikiProg.total - 1, 1)}` : wikiProg.phase === "sintesi" ? "scrivo…" : "controllo le fonti…"}</span>
        <div class="bar"><div class="fill" style="width:{wikiProg.total ? (wikiProg.done / wikiProg.total) * 100 : 6}%"></div></div>
        <button class="ghost small" onclick={stopWiki} title="Ferma al prossimo passaggio">Stop</button>
      </div>
    {/if}
    {#if clipOffer}
      <div class="toast clipoffer">
        <div class="clipbody">
          <span class="cliptitle">Link PDF negli appunti</span>
          <span class="clipurl" title={clipOffer}>{clipOffer}</span>
        </div>
        <button class="ghost small" onclick={clipGrab} disabled={clipBusy}>{clipBusy ? "…" : "Aggancia"}</button>
        <button class="ghost small clipx" onclick={() => (clipOffer = null)} title="Ignora questo link" aria-label="Ignora">✕</button>
      </div>
    {/if}
    {#if aiBatch}
      <div class="toast">
        <span>{aiBatch.kind === "summary" ? "Riassunto AI" : "Tag automatici AI"}: {aiBatch.done}/{aiBatch.total}</span>
        <div class="bar"><div class="fill" style="width:{aiBatch.total ? (aiBatch.done / aiBatch.total) * 100 : 0}%"></div></div>
        <button class="ghost small" onclick={() => (batchCancel = true)} title="Interrompi l'operazione AI in corso">Stop</button>
      </div>
    {/if}
    {#if metaScan}
      <div class="toast">
        <span>Metadati: {metaScan.done}/{metaScan.total} — {metaScan.updated} aggiornati</span>
        <div class="bar"><div class="fill" style="width:{metaScan.total ? (metaScan.done / metaScan.total) * 100 : 0}%"></div></div>
        <button class="ghost small" onclick={() => cancelRecoverMetadata()} title="Interrompi il recupero (quanto già aggiornato resta)">Stop</button>
      </div>
    {/if}
    {#if pdfBatch}
      <div class="toast">
        <span>Trova PDF: {pdfBatch.done}/{pdfBatch.total} — {pdfBatch.found} allegati</span>
        <div class="bar"><div class="fill" style="width:{pdfBatch.total ? (pdfBatch.done / pdfBatch.total) * 100 : 0}%"></div></div>
        <button class="ghost small" onclick={() => (pdfBatchCancel = true)} title="Interrompi la ricerca dei PDF (quelli già allegati restano)">Stop</button>
      </div>
    {/if}
    {#if status}
      <div class="toast">{status}</div>
    {/if}
  </div>

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

  {#if metaFindId !== null}
    <MetaFinder
      id={metaFindId}
      onClose={() => (metaFindId = null)}
      onApplied={async () => {
        metaFindId = null;
        status = "Metadati applicati ✓";
        await loadDocs();
        await loadSidebar();
        if (careModal && careTab === "salute") openHealth();
      }}
      onEditManual={() => {
        const x = metaFindId;
        metaFindId = null;
        editingId = x;
      }}
    />
  {/if}

  {#if pdfFindId !== null}
    <PdfFinder
      id={pdfFindId}
      onClose={() => (pdfFindId = null)}
      onApplied={async () => {
        const x = pdfFindId;
        pdfFindId = null;
        refPanel = null;
        status = "PDF trovato e allegato ✓";
        await loadDocs();
        await loadSidebar();
        const fresh = docs.find((d) => d.id === x);
        if (fresh?.has_file) openDocument(fresh);
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

  {#if urlModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) urlModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div class="idmodal" onclick={(e) => e.stopPropagation()}>
        <h2>Aggancia da URL</h2>
        <p class="dimtext">
          Incolla il link diretto a un PDF (deve terminare in <code>.pdf</code> o essere servito come PDF): lo scarico
          e lo aggiungo alla libreria, con i metadati se trovo un DOI. Per farlo con <strong>un clic dal browser</strong>
          usa il bookmarklet in <button class="linklike" onclick={() => { urlModal = false; settingsModal = true; settingsTab = "connector"; loadConnector(); }}>Impostazioni → Connettore browser</button>.
        </p>
        <div style="display:flex;gap:8px;align-items:center;">
          <input
            style="flex:1;"
            type="text"
            bind:value={urlInput}
            placeholder="https://arxiv.org/pdf/2401.12345"
            onkeydown={(e) => { if (e.key === "Enter") doAddFromUrl(); }}
          />
          <button class="ghost small" onclick={pasteUrlFromClipboard} title="Incolla dagli appunti">📋</button>
        </div>
        <div class="modactions">
          <button class="ghost" onclick={() => (urlModal = false)}>Annulla</button>
          <button class="primary" onclick={doAddFromUrl} disabled={urlBusy || !urlInput.trim()}>{urlBusy ? "Scarico…" : "Aggancia"}</button>
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
                      <span class="libdot in" title="Già in libreria"></span>
                      <button class="hflink" onclick={() => openById(r.in_library!)} title="Nella tua libreria — apri">{r.title ?? r.raw ?? r.ref_doi}</button>
                      <span class="badge2 inlibref">in libreria</span>
                    {:else}
                      <span class="libdot" title="Non in libreria"></span>
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

  {#snippet neighborRow(r: SearchResult)}
    <li class="exrow">
      <div class="exmain">
        <span class="extitle" title={r.title ?? ""}><span class="libdot" class:in={r.in_library} title={r.in_library ? "Già in libreria" : "Non in libreria"}></span>{r.title ?? "Senza titolo"}</span>
        <span class="exmeta">{[r.authors?.[0], r.year, r.venue].filter(Boolean).join(" · ")}{r.citations ? ` · ${r.citations} cit.` : ""}</span>
      </div>
      <div class="exacts">
        {#if r.in_library}
          <span class="badge2 inlibref">in libreria</span>
        {:else}
          <button class="hflink small" disabled={addingExt === r.external_id} onclick={() => addNeighbor(r)} title="Aggiungi alla libreria (scarica il PDF se Open Access, altrimenti come riferimento)">{addingExt === r.external_id ? "…" : "+ Aggiungi"}</button>
          <button class="hflink small" class:on={pdfInputFor === r.external_id} onclick={() => { pdfInputFor = pdfInputFor === r.external_id ? null : r.external_id; pdfUrlInput = ""; }} title="Aggiungi questo paper col PDF che stai guardando nel browser: apri il PDF (↗), copia il suo link e incollalo qui">+ PDF</button>
        {/if}
        <button class="hflink small" onclick={() => navExplore({ openalexId: r.external_id, doi: r.doi, title: r.title }, r.title ?? "documento")} title="Esplora le citazioni di questo paper">Esplora ↗</button>
        {#if r.url}<button class="hflink small" onclick={() => openInBrowser(r.url!)} title="Apri la pagina del paper">↗</button>{/if}
      </div>
    </li>
    {#if pdfInputFor === r.external_id}
      <li class="expdfrow">
        <input
          class="expdfinput"
          type="url"
          placeholder="incolla il link diretto al PDF (https://…)"
          bind:value={pdfUrlInput}
          use:pdfFocus
          onkeydown={(e) => { if (e.key === "Enter") addNeighborWithPdf(r); if (e.key === "Escape") { pdfInputFor = null; pdfUrlInput = ""; } }}
        />
        <button class="hflink small" onclick={pastePdfUrlFromClipboard} title="Incolla dagli appunti">📋</button>
        <button class="hflink small" disabled={addingExt === r.external_id || !pdfUrlInput.trim()} onclick={() => addNeighborWithPdf(r)} title="Scarica e aggiungi col PDF">{addingExt === r.external_id ? "…" : "OK"}</button>
        <button class="hflink small" onclick={() => { pdfInputFor = null; pdfUrlInput = ""; }} title="Annulla" aria-label="Annulla">✕</button>
      </li>
    {/if}
  {/snippet}

  {#if exploreModal}
    <!-- Stays open on outside click (lots of info to read); close only with the ✕. -->
    <div class="modalback" role="presentation">
      <div class="idmodal exwide exdialog" role="dialog" tabindex="-1">
        <button class="modal-x" onclick={() => (exploreModal = false)} aria-label="Chiudi" title="Chiudi">✕</button>
        <div class="exhead">
          <h2>Esplora citazioni</h2>
          {#if exploreStack.length}
            <button class="hflink small exback" onclick={backExplore} title="Torna al paper precedente">← Indietro</button>
          {/if}
        </div>
        <p class="dimtext" title={exploreTitle}>{exploreTitle} — da OpenAlex; clicca un nodo (o «Esplora ↗») per spostarti di paper in paper (snowball)</p>
        {#if exploreLoading}
          <p class="dimtext">Carico la rete di citazioni…</p>
        {:else if exploreData}
          {#if exploreData.seed_unresolved}
            <p class="dimtext">OpenAlex non riconosce questo paper — né per DOI né per titolo (per l'aggancio senza DOI serve un titolo che corrisponda esattamente). Recupera prima i metadati con «✦ senza metadati» o «Recupera metadati…».</p>
          {:else}
            <div class="exbar">
              <div class="seg" role="group" aria-label="Vista esplorazione">
                <button class="segbtn wide" class:active={exploreView === "map"} onclick={() => (exploreView = "map")} title="Mappa temporale: riferimenti a sinistra, citazioni a destra">Mappa</button>
                <button class="segbtn wide" class:active={exploreView === "list"} onclick={() => (exploreView = "list")} title="Liste con tutte le azioni (+ PDF, salva…)">Lista</button>
              </div>
              <span class="exlegend">
                <span class="libdot in"></span>{exploreData.references.filter((r) => r.in_library).length + exploreData.citations.filter((r) => r.in_library).length} in libreria
                <span class="libdot"></span>{exploreData.references.filter((r) => !r.in_library).length + exploreData.citations.filter((r) => !r.in_library).length} mancanti
                · nodo più grande = più citato
              </span>
            </div>
            {#if exploreView === "map"}
              <CitationMap
                refs={exploreData.references}
                cits={exploreData.citations}
                title={exploreTitle}
                onNode={(r, e) => { e.stopPropagation(); mapPop = { r, x: e.clientX, y: e.clientY }; }}
              />
            {:else}
            <div class="exgrid">
              <div class="hfsec">
                <div class="exsechead">
                  <h3>Riferimenti — cita ({exploreData.references.length})</h3>
                  {#if exploreData.references.length}<button class="hflink small" onclick={() => saveNeighborList("references")} title="Salva questa lista (con i link ai paper) in un file Markdown">⬇ Salva</button>{/if}
                </div>
                {#if exploreData.references.length}
                  <ul class="hflist exlist">{#each exploreData.references as r (r.external_id)}{@render neighborRow(r)}{/each}</ul>
                {:else}<p class="dimtext">Nessun riferimento noto a OpenAlex per questo paper.</p>{/if}
              </div>
              <div class="hfsec ghsec">
                <div class="exsechead">
                  <h3>Citato da ({exploreData.citations.length})</h3>
                  {#if exploreData.citations.length}<button class="hflink small" onclick={() => saveNeighborList("citations")} title="Salva questa lista (con i link ai paper) in un file Markdown">⬇ Salva</button>{/if}
                </div>
                {#if exploreData.citations.length}
                  <ul class="hflist exlist">{#each exploreData.citations as r (r.external_id)}{@render neighborRow(r)}{/each}</ul>
                {:else}<p class="dimtext">Nessun paper che cita questo (ancora) su OpenAlex.</p>{/if}
              </div>
            </div>
            {/if}
          {/if}
        {/if}
      </div>
    </div>
  {/if}

  {#if mapPop}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
    <div class="menu mappop" role="menu" tabindex="-1" use:clamp={{ x: mapPop.x, y: mapPop.y }} onclick={(e) => e.stopPropagation()}>
      <div class="mtitle" title={mapPop.r.title ?? ""}>{mapPop.r.title ?? "Senza titolo"}</div>
      <div class="mapmeta">{[mapPop.r.authors?.[0], mapPop.r.year, mapPop.r.venue].filter(Boolean).join(" · ")}{mapPop.r.citations ? ` · ${mapPop.r.citations} cit.` : ""}</div>
      {#if mapPop.r.in_library}
        {#if mapPop.r.doi}
          <button class="medit" onclick={() => { const doi = mapPop!.r.doi!; mapPop = null; openByDoi(doi); }}>Apri in libreria</button>
        {:else}
          <div class="mapmeta">✓ già in libreria</div>
        {/if}
      {:else}
        <button class="medit" disabled={addingExt === mapPop.r.external_id} onclick={() => { const r = mapPop!.r; addNeighbor(r); }} title="Scarica il PDF se Open Access, altrimenti aggiunge come riferimento">{addingExt === mapPop.r.external_id ? "…" : "+ Aggiungi alla libreria"}</button>
      {/if}
      <button class="medit" onclick={() => { const r = mapPop!.r; mapPop = null; navExplore({ openalexId: r.external_id, doi: r.doi, title: r.title }, r.title ?? "documento"); }} title="Ricentra la mappa su questo paper (← Indietro per tornare)">Esplora da qui</button>
      {#if mapPop.r.doi}
        <button class="medit" onclick={() => { const doi = mapPop!.r.doi!; mapPop = null; openInBrowser(`https://doi.org/${doi}`); }}>DOI ↗</button>
      {/if}
      {#if mapPop.r.url}
        <button class="medit" onclick={() => { const u = mapPop!.r.url!; mapPop = null; openInBrowser(u); }}>Pagina del paper ↗</button>
      {/if}
    </div>
  {/if}

  {#if tagEdit}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="menu tagedit" role="menu" tabindex="-1" use:clamp={{ x: tagEdit.x, y: tagEdit.y }} onclick={(e) => e.stopPropagation()}>
      <input
        class="teinput"
        bind:value={tagEdit.name}
        placeholder="nome del tag"
        aria-label="Nome del tag"
        onkeydown={(e) => { if (e.key === "Enter") saveTagEdit(); else if (e.key === "Escape") tagEdit = null; }}
      />
      <div class="teswatches">
        {#each PALETTE as c (c)}
          <button
            class="teswatch"
            class:on={tagEdit.color === c}
            style="background:{c}"
            aria-label={`Colore ${c}`}
            onclick={() => { if (tagEdit) tagEdit = { ...tagEdit, color: c }; }}
          ></button>
        {/each}
      </div>
      <div class="teact">
        <button class="ghost small" onclick={() => (tagEdit = null)}>Annulla</button>
        <button class="primary small" disabled={!tagEdit.name.trim()} onclick={saveTagEdit}>Salva</button>
      </div>
    </div>
  {/if}

  {#if careModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modalback" onmousedown={(e) => { if (e.target === e.currentTarget) careModal = false; }} role="presentation">
      <div class="idmodal hfwide" role="dialog" tabindex="-1">
        <h2>Cura della libreria</h2>
        <div class="seg caretabs" role="group" aria-label="Sezioni">
          <button class="segbtn wide" class:active={careTab === "salute"} onclick={() => openCare("salute")}>Salute</button>
          <button class="segbtn wide" class:active={careTab === "gap"} onclick={() => openCare("gap")}>Gap di citazioni</button>
          <button class="segbtn wide" class:active={careTab === "duplicati"} onclick={() => openCare("duplicati")}>Duplicati</button>
        </div>
        {#if careTab === "salute"}
        {#if healthLoading}
          <p class="dimtext">Analisi in corso…</p>
        {:else if health}
          {@const cats = [
            { label: "File mancanti sul disco", rows: health.missing_file, hint: "Il PDF non è più al percorso salvato.", ocr: false, find: false },
            { label: "PDF senza testo estratto", rows: health.no_text, hint: "Probabili scansioni (immagine): non cercabili né indicizzabili. «OCR» riconosce il testo con il motore di Windows.", ocr: true, find: false },
            { label: "Metadati incompleti", rows: health.no_metadata, hint: "Manca titolo, anno o autori. «✦ senza metadati» in alto li recupera in blocco; «Trova…» cerca i candidati online per il singolo documento e scegli tu.", ocr: false, find: true },
            { label: "Senza incorporamento semantico", rows: health.no_embedding, hint: "Esclusi dalla ricerca semantica e da «Correlati».", ocr: false, find: false },
            { label: "Senza copertina", rows: health.no_thumbnail, hint: "Nessuna anteprima generata.", ocr: false, find: false },
          ]}
          <p class="dimtext">{health.total} documenti analizzati.</p>
          {#each cats as cat (cat.label)}
            <div class="hfsec">
              <h3>{cat.label} ({cat.rows.length})</h3>
              {#if cat.rows.length}
                <p class="dimtext">{cat.hint}</p>
                <ul class="hflist">
                  {#each cat.rows.slice(0, 50) as r (r.id)}
                    <li class="refrow">
                      <button class="hflink" onclick={() => openHealthRow(r.id)} title={r.path}>{r.title ?? r.path.split(/[\\/]/).pop()}</button>
                      {#if cat.ocr}<button class="hflink small" disabled={ocrBusy === r.id} onclick={() => runOcr(r.id)} title="Riconosci il testo della scansione (motore OCR di Windows) e rendilo cercabile">{ocrBusy === r.id ? "OCR…" : "OCR"}</button>{/if}
                      {#if cat.find}<button class="hflink small" onclick={() => (metaFindId = r.id)} title="Cerca online la scheda giusta (Crossref, arXiv, OpenAlex) e confermala tu">Trova…</button>{/if}
                    </li>
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
              <p class="dimtext">Unisci i duplicati dalla scheda <strong>Duplicati</strong> qui sopra.</p>
            {:else}<p class="dimtext">Nessun duplicato ✓</p>{/if}
          </div>
        {/if}

        {:else if careTab === "gap"}
        <p class="dimtext">I DOI che la tua libreria cita di più ma che non possiedi ancora. Si basa sui riferimenti estratti — recupera i <strong>Metadati</strong> dei tuoi paper (Crossref) per arricchirli.</p>
        <div class="refdoibar">
          {#if refdoiRunning}
            <button class="ghost small" onclick={cancelRefDois}>Interrompi</button>
            <span class="dimtext">{refdoiProg ? `Risolvo ${refdoiProg.done}/${refdoiProg.total} — ${refdoiProg.resolved} DOI trovati…` : "Avvio…"}</span>
          {:else}
            <button class="ghost small" onclick={runResolveRefDois} title="Cerca online (Crossref) un DOI per i riferimenti che ne sono privi, così entrano nel conteggio dei gap">Risolvi DOI dei riferimenti (online)</button>
            <span class="dimtext">Recupera i DOI mancanti dei riferimenti già in libreria — precision-first, nessun abbinamento incerto.</span>
          {/if}
        </div>
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

        {:else}
        <p class="dimtext">Copie dello stesso lavoro (per DOI o titolo+anno). «Unisci» tiene la prima e vi trasferisce tag, collezioni e annotazioni; le altre finiscono nel cestino.</p>
        {#if dupGroups.length === 0}
          <p class="dimtext">Nessun duplicato ✓</p>
        {:else}
          <div class="dupwrap inmodal">
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
        {/if}
        <div class="modactions"><button class="ghost" onclick={() => (careModal = false)}>Chiudi</button></div>
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
    <!-- Finestra flottante NON modale: l'app resta usabile, la guida si trascina
         dalla barra del titolo e (a scelta) resta in primo piano sopra il lettore. -->
    <div class="helpwin" class:pinned={helpPin} style="left:{helpPos.x}px; top:{helpPos.y}px" role="dialog" aria-label="Guida a Scriptorium">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="helpdrag" onmousedown={startHelpDrag} title="Trascina per spostare la guida">
        <h2>Guida a Scriptorium</h2>
        <label class="helppinlbl" title="Tieni la guida sopra ogni altra vista, anche il lettore">
          <input type="checkbox" bind:checked={helpPin} /> in primo piano
        </label>
        <button class="helpx" title="Chiudi la guida" onclick={() => (helpModal = false)}>×</button>
      </div>
      <div class="helpbody">
        <p class="dimtext">Gestore locale di PDF, riferimenti e appunti: tutto resta sul tuo computer, le funzioni di rete e AI sono opzionali. <strong>Regola d'oro</strong>: qualunque cosa cerchi, premi <kbd>Ctrl</kbd>+<kbd>K</kbd> e digitala.</p>

        <div class="helptabs">
          <button class:active={helpTab === "inizia"} onclick={() => (helpTab = "inizia")}>Inizia qui</button>
          <button class:active={helpTab === "libreria"} onclick={() => (helpTab = "libreria")}>Libreria</button>
          <button class:active={helpTab === "lettura"} onclick={() => (helpTab = "lettura")}>Lettura</button>
          <button class:active={helpTab === "scrittura"} onclick={() => (helpTab = "scrittura")}>Scrittura</button>
          <button class:active={helpTab === "scoperta"} onclick={() => (helpTab = "scoperta")}>Scoperta</button>
          <button class:active={helpTab === "ai"} onclick={() => (helpTab = "ai")}>AI &amp; dati</button>
          <button class:active={helpTab === "faq"} onclick={() => (helpTab = "faq")}>FAQ</button>
        </div>

        {#if helpTab === "inizia"}
        <div class="helpsec">
          <h3>Le tre porte d'ingresso</h3>
          <ul>
            <li><strong>Barra strumenti</strong> (in alto): un'icona per ogni strumento — passaci sopra col mouse per il nome. Nell'ordine: <strong>I miei paper</strong> (torna alla griglia), <strong>Importa</strong>, <strong>Vista</strong>, <strong>Riprendi lettura</strong>, <strong>Chiedi alla libreria</strong>, <strong>Wiki</strong>, <strong>Cerca online</strong>, <strong>Appunti</strong>, <strong>Progetti (LaTeX)</strong>, <strong>Archivio</strong> (raccolte e sotto-raccolte, in vista sinottica), <strong>Riscopri</strong>, <strong>Novità</strong> (🔔 col conteggio dei nuovi paper), <strong>Esporta</strong>, <strong>Cura della libreria</strong>, <strong>Indice semantico</strong>, <strong>Memoria AI</strong> (o <em>Attiva AI</em> quando è spenta), <strong>Backup</strong>, <strong>Cestino</strong>, <strong>Terminale</strong> (&gt;_), <strong>Plancia</strong> (il sinottico dei processi, in finestra separata), <strong>Guida</strong>, <strong>Aspetto</strong>, <strong>Sistema</strong> (Impostazioni · Controlla aggiornamenti · Informazioni). Le voci con un menu si aprono al clic, le altre eseguono; l'icona è evidenziata quando sei nella vista corrispondente. In alto trovi anche il chip <strong>AI</strong> (stato dell'AI locale), «✦ N senza metadati» quando serve, e l'icona della <strong>palette</strong>.</li>
            <li><strong>Menu radiale</strong> (tasto destro): su un <strong>documento</strong> → le azioni su quel documento; sullo <strong>spazio vuoto</strong> → il menu globale (gli stessi gruppi della barra); su una <strong>selezione multipla</strong> → le azioni in blocco. Muovi verso un petalo e clicca (basta la direzione); <strong>rotella</strong> per ruotare; <strong>digita</strong> per filtrare tutte le voci a qualsiasi profondità; il centro torna indietro, <kbd>Esc</kbd> chiude. La <strong>descrizione</strong> della voce evidenziata compare sotto l'anello.</li>
            <li><strong>Palette comandi</strong> (<kbd>Ctrl</kbd>+<kbd>K</kbd>): ogni azione, documento, <strong>appunto</strong>, <strong>pagina wiki</strong>, <strong>progetto LaTeX</strong>, filtro, sezione della guida e tema — digitando. Funziona <strong>anche dentro il lettore</strong>. Barra, radiale e palette pescano dallo <strong>stesso registro</strong>: se non trovi un comando, è comunque lì.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>La griglia e la barra laterale</h3>
          <ul>
            <li><strong>Un clic</strong> su una scheda apre il <strong>pannello di dettaglio</strong> a destra (abstract, riassunto AI, tag modificabili, citazioni, note); <strong>doppio clic</strong> o <kbd>Invio</kbd> aprono il lettore.</li>
            <li>La <strong>barra laterale</strong> è la navigazione: filtri rapidi (Preferiti, Da leggere, Con codice, Peer-reviewed, Il mio lavoro), tag, collezioni, ricerche salvate, cartella sorvegliata. <kbd>Ctrl</kbd>+<kbd>B</kbd> la mostra/nasconde.</li>
            <li>Nella vista «Tutti»: la <strong>Panoramica</strong> (da leggere, in lettura, aggiunti questo mese + un paper da riscoprire al giorno) e <strong>Continua a leggere</strong> (gli ultimi PDF aperti). Nel lettore la barra svanisce mentre leggi e lo zoom è ricordato per documento.</li>
            <li><strong>Questa guida è una finestra</strong>: trascinala dalla barra del titolo, ridimensionala dall'angolo in basso a destra (come quasi tutte le finestre di dialogo) e spunta <strong>«in primo piano»</strong> per tenerla visibile — anche sopra il lettore — mentre segui i passaggi.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>K</kbd></td><td>Palette comandi — tutto, digitando</td></tr>
              <tr><td><kbd>/</kbd></td><td>Ricerca in libreria</td></tr>
              <tr><td><kbd>Ctrl</kbd>+<kbd>B</kbd></td><td>Mostra/nascondi la barra laterale</td></tr>
              <tr><td><kbd>frecce</kbd> / <kbd>Spazio</kbd></td><td>Muoviti tra le schede / apri-chiudi il pannello</td></tr>
              <tr><td><kbd>X</kbd> / <kbd>F</kbd></td><td>Seleziona / preferito</td></tr>
              <tr><td><kbd>Invio</kbd> / <kbd>Esc</kbd></td><td>Apri il lettore / chiudi</td></tr>
              <tr><td>Tasto destro</td><td>Menu radiale (documento, selezione o globale)</td></tr>
            </tbody>
          </table>
        </div>

        {:else if helpTab === "libreria"}
        <div class="helpsec">
          <h3>Importare</h3>
          <ul>
            <li><strong>Sei vie</strong> (barra → Importa): <strong>PDF dal disco</strong> (anche trascinandoli nella finestra — restano dove sono, l'app li indicizza; i duplicati si riconoscono dal contenuto); <strong>Da gestore bibliografico</strong> — l'export di <strong>Zotero, Mendeley, EndNote, JabRef…</strong> in <strong>.bib / .ris / CSL-JSON</strong>: porta metadati, aggancia i PDF (dal campo <em>file</em> o da una cartella d'export che indichi) e trasforma le parole chiave in <strong>tag</strong>, senza doppioni (dedup per DOI e per contenuto del PDF; i lavori già nel Cestino tornano visibili); per <strong>identificatore</strong> (DOI / arXiv / ISBN / PMID); <strong>da URL</strong>; <strong>progetto LaTeX (.zip)</strong> — i tuoi paper con la loro bibliografia, marcati «Il mio lavoro»; <strong>Cartella sorvegliata</strong> (importa da sola ciò che ci finisce dentro).</li>
            <li><strong>Dal browser</strong>: copia il link del PDF e torna su Scriptorium — compare «Aggancia» (interruttore in Impostazioni → Connettore); in alternativa il <strong>bookmarklet</strong>, o la Cartella sorvegliata puntata su Download.</li>
            <li><strong>Riferimenti senza PDF</strong> (aggiunti da ricerca online, citazioni, BibTeX o ID): <strong>Trova PDF…</strong> (radiale della scheda, o aprendo la voce) mostra i <strong>candidati</strong> trovati online per identificativo e per titolo (arXiv, Unpaywall, OpenAlex, Semantic Scholar, Crossref) con le prove — «Scarica e allega» quello giusto, «Apri pagina» per controllare, o incolla un link diretto. Sulla <strong>selezione multipla</strong> e in blocco (Cura della libreria → «Trova PDF dei riferimenti») resta automatico: allega solo abbinamenti sicuri, ora anche per titolo su arXiv/S2.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Organizzare</h3>
          <ul>
            <li><strong>Tag</strong> colorati (la <strong>✎</strong> in sidebar rinomina/ricolora, la <strong>×</strong> elimina; dal pannello dettagli li applichi al volo) e <strong>Collezioni</strong>, anche <em>smart</em> (si popolano da sole con una regola).</li>
            <li><strong>Filtri</strong> in sidebar (Preferiti, Da leggere, Con codice, Peer-reviewed, Il mio lavoro), <strong>ordinamento combinabile</strong> (chip «Ordina ▾»: un clic attiva, un altro inverte, un terzo toglie), badge <em>preprint / peer-reviewed</em> sulle schede.</li>
            <li><strong>Viste</strong> (barra → Vista): griglia (copertine ridimensionabili con − ▭ +), lista a colonne, <strong>Costellazione</strong> (la mappa semantica — vedi la scheda <em>Scoperta</em>). Clic su un <strong>autore</strong> → tutti i suoi lavori.</li>
            <li><strong>Archivio</strong> (icona cartella sulla barra): le collezioni come <strong>albero navigabile</strong> — sotto-raccolte a piacere, <strong>trascina un paper</strong> su una raccolta per spostarlo (Ctrl = aggiungi anche lì: l'appartenenza è multipla), trascina una raccolta su un'altra per annidarla. Eliminare una raccolta non tocca mai i paper (le sotto-raccolte risalgono). Nel pannello: <strong>✦ Suggerisci</strong> propone i paper affini (somiglianza semantica locale, con soglia di confidenza — mai automatico) scegliendo la sorgente: <em>Nome</em> della raccolta, <em>Contenuto</em> (i paper già dentro) o <em>Entrambi</em> col <strong>peso regolabile</strong>, il toggle <strong>Ricerca «Novità»</strong> aggancia una ricerca online alla raccolta (le novità accettate <em>entrano da sole nella raccolta</em>, filtrate per pertinenza quando la raccolta ha ≥3 paper indicizzati).</li>
            <li><strong>Specchio su disco</strong> (chip in alto nell'Archivio): proietta le raccolte in una cartella vera — <code>Raccolta\Sottoraccolta\Autore Anno — Titolo.pdf</code> — con <strong>hardlink</strong> (zero spazio extra), aggiornata da sola a ogni cambio. Comodissima da Esplora risorse e dal terminale. Cancellare o spostare file nello specchio non tocca la libreria (si rigenera); <em>modificare il contenuto</em> di un PDF lì dentro sì, perché è lo stesso file: per annotare usa il lettore.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Condividere e stampare</h3>
          <ul>
            <li>Tasto destro → <strong>Condividi</strong>: <strong>WhatsApp / Teams / Gmail</strong> aprono la bozza col messaggio pronto e il <strong>PDF è già copiato negli appunti di sistema</strong> — incollalo con <kbd>Ctrl</kbd>+<kbd>V</kbd>; <strong>Outlook desktop</strong> allega il file da solo. Funziona anche sulla <strong>selezione multipla</strong>.</li>
            <li><strong>Stampa</strong>: dal lettore (menu <strong>⋯ Altro</strong> o radiale) per il documento aperto, o dal radiale della selezione per stamparne più d'uno.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Metadati</h3>
          <ul>
            <li>Quando ci sono schede incomplete compare «<strong>✦ N senza metadati</strong>»: un clic le recupera in blocco (con barra di avanzamento e Stop) — prima l'id arXiv nel <strong>nome del file</strong> (funziona anche sulle scansioni), poi DOI e <strong>titolo</strong> dal PDF (Crossref/arXiv). Mai dal primo DOI trovato nel testo: niente etichette sbagliate — ciò che non è sicuro resta com'è.</li>
            <li>Per il caso singolo o ostinato: tasto destro → Organizza → <strong>Recupera metadati…</strong> fa la ricerca <em>estesa</em> (Crossref, arXiv, OpenAlex, ogni DOI/arXiv stampato nel PDF, nome del file) e mostra i <strong>candidati con le prove</strong> trovate nel PDF (titolo, autori, anno): scegli tu quale applicare, o incolla un DOI/arXiv. Lo stesso da Salute libreria («Trova…») accanto a ogni documento incompleto.</li>
            <li><strong>Impostazioni → Manutenzione → «Verifica e ripara metadati»</strong> ricontrolla tutta la libreria e corregge le schede il cui titolo non corrisponde al PDF. Sicuro e ripetibile. A mano: tasto destro → <strong>Modifica metadati</strong>.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Cura, backup, cestino</h3>
          <ul>
            <li><strong>Cura della libreria</strong> (barra): <strong>Salute</strong> (file mancanti, PDF senza testo → OCR, metadati incompleti), <strong>Gap di citazioni</strong> (i DOI più citati dai tuoi paper che non possiedi; «Risolvi DOI dei riferimenti» ne recupera altri online, senza mai un abbinamento incerto), <strong>Duplicati</strong> (unione), <strong>Rigenera anteprime</strong>.</li>
            <li><strong>Backup libreria</strong> (barra): copia completa — PDF + database — in una cartella a tua scelta. <strong>Cestino</strong>: gli eliminati restano ripristinabili finché non lo svuoti.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Cercare in libreria</h3>
          <ul>
            <li>Barra di ricerca in alto (<kbd>/</kbd>), tre modalità: <em>Tutto</em> (testo + semantica), <em>Testo</em> o <em>Semantica</em> (per significato — serve l'<strong>Indice semantico</strong>). Cerca anche nelle <strong>annotazioni</strong> e nelle <strong>note dei documenti</strong>; gli <strong>Appunti .md</strong> che corrispondono compaiono in un gruppo dedicato sopra i risultati.</li>
          </ul>
        </div>

        {:else if helpTab === "scoperta"}
        <div class="helpsec">
          <h3>Cerca online</h3>
          <ul>
            <li>Barra → <strong>Cerca online</strong>: arXiv, OpenAlex, ADS, Semantic Scholar, Europe PMC, CORE, DOAJ, <strong>HF Papers</strong> (l'indice con i repo GitHub dei paper). Filtri anno/autore/solo-OA, chip <strong>Con codice</strong> / <strong>Peer-reviewed</strong> / <strong>Preprint</strong>, colonne ordinabili. I PDF Open Access si <strong>scaricano</strong>, gli altri entrano come <strong>riferimento</strong> (allegherai il PDF dopo).</li>
            <li><strong>Ricerche salvate → Novità</strong>: dopo una ricerca premi <em>★ Salva</em> → va in sidebar. I risultati <em>nuovi</em> dall'ultima volta si raccolgono nella campana <strong>Novità</strong> (🔔, ricontrollata a ogni avvio): da lì <em>Aggiungi</em> o <em>Ignora</em>.</li>
            <li><strong>Riscopri</strong> (barra): ti ripesca un documento dimenticato o mai letto.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Codice, citazioni e repository</h3>
          <ul>
            <li>I paper che citano un repo mostrano l'icona <strong>GitHub</strong> (card, lista, risultati online): cliccala per aprire il repository.</li>
            <li>Tasto destro → <strong>Codice & repo</strong>: anteprima del <strong>README</strong> nell'app, più i modelli/dataset collegati su <strong>Hugging Face</strong>.</li>
            <li>Filtro <strong>“Con codice (GitHub)”</strong> nella sidebar per vedere solo i paper con codice disponibile.</li>
            <li>Tasto destro → Cita → <strong>Riferimenti e citazioni</strong>: la bibliografia del paper (con i riferimenti già nella tua libreria cliccabili) e i documenti che lo <strong>citano</strong>. Lì trovi anche <strong>Copia APA / IEEE / BibTeX / citekey / \cite / [@…]</strong>. Con più paper <strong>selezionati</strong>, tasto destro → Cita copia un solo <code>\cite&#123;k1,k2&#125;</code> o tutte le voci BibTeX insieme (per LaTeX/Pandoc).</li>
            <li><strong>Esplora citazioni (online)</strong> si apre sulla <strong>Mappa</strong>: riferimenti a sinistra (il passato su cui si fonda), citazioni a destra (il futuro), in ordine di anno; pallino <strong>pieno</strong> = già in libreria, <strong>tratteggiato</strong> = mancante, nodo più grande = paper più citato. Clic su un nodo → aggiungi / apri / <em>esplora da qui</em> (la mappa si ricentra, «← Indietro» per tornare). La <strong>Lista</strong> con tutte le azioni (+ PDF, salva) resta a un click.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Costellazione — la mappa della libreria</h3>
          <ul>
            <li>Barra → Vista → <strong>Costellazione</strong>: ogni <strong>stella</strong> è un documento, i <strong>rombi viola</strong> sono i tuoi appunti, i legami sono la <strong>somiglianza di significato</strong>. Serve l'<strong>Indice semantico</strong>; i documenti nuovi entrano al prossimo aggiornamento dell'indice. Le posizioni si ricordano tra le sessioni.</li>
            <li><strong>Clic</strong> = scheda del nodo (con i legami in %, cliccabili); <strong>doppio clic</strong> = apri; <strong>tasto destro</strong> = menu; <strong>Ctrl+clic</strong> = seleziona. Da lontano vedi le <strong>nebulose</strong> — le comunità semantiche con la loro etichetta; da vicino compaiono i badge <strong>✓</strong> (peer-reviewed) e la forcella (codice GitHub).</li>
            <li>In alto a destra: <strong>Cerca nel grafo</strong> (bastano 2-3 lettere del titolo o di un autore → candidati suggeriti; Invio o clic evidenzia la stella con un alone pulsante e centra la vista lì; × o Esc pulisce), <strong>Colora per</strong> (Tag dominante / Comunità semantiche / Anno / Stato lettura), <strong>Nebulose</strong> e il pannello <strong>⚙</strong> (Legami per nodo, Soglia somiglianza).</li>
            <li><strong>Esplora dintorni (online)</strong>, nella scheda di una stella: <em>Citazioni</em> / <em>Simili</em> / <em>Autore</em> → i risultati appaiono come <strong>stelle fantasma</strong> tratteggiate attorno a quella; clic → «Aggiungi alla libreria». Entrano nel grafo al prossimo aggiornamento dell'indice.</li>
            <li><strong>Catena di esplorazione</strong>: anche la scheda di una <em>stella fantasma</em> ha Citazioni / Simili / Autore — le nuove scoperte si agganciano a quella, in catena (snowball), così scavi di paper in paper senza aggiungere nulla finché non trovi quello giusto. Mentre esplori la mappa entra in <strong>modalità esplorazione</strong>: la libreria si attenua, i seed hanno un anello «scanner», ogni generazione della catena ha il suo colore e i collegamenti scorrono animati; le catene si dispongono da sole senza sovrapporsi. <strong>Citazioni</strong> funziona anche senza DOI (id OpenAlex per le scoperte; titolo con corrispondenza rigorosa per i tuoi paper).</li>
          </ul>
        </div>

        {:else if helpTab === "lettura"}
        <div class="helpsec">
          <h3>Il lettore</h3>
          <ul>
            <li><strong>Immersivo</strong>: la barra svanisce mentre leggi (torna col mouse), gli strumenti rari sono sotto <strong>⋯ Altro</strong>, lo zoom è ricordato per documento e si riparte dall'<strong>ultima pagina</strong>. «<strong>Riprendi lettura</strong>» (barra/radiale/palette) riapre l'ultimo PDF al punto in cui eri.</li>
            <li><strong>Annotazioni</strong> (<kbd>A</kbd>): evidenziazioni con colore e commento, o <strong>note puntuali</strong> «a spillo» — ancorate alla pagina. <strong>Nota del documento</strong> (<kbd>E</kbd>): un appunto per l'<em>intero</em> paper. Gli <strong>Appunti .md</strong> sono invece file indipendenti (scheda <em>Scrittura</em>).</li>
            <li><strong>Cerca nel documento</strong> (<kbd>Ctrl</kbd>+<kbd>F</kbd>) in una fascia dedicata; <kbd>Invio</kbd>/<kbd>Maiusc</kbd>+<kbd>Invio</kbd> scorrono i risultati. Più indice, zoom/adatta, rotazione, due pagine, modalità notte.</li>
            <li><strong>Lente AI</strong>: seleziona un passaggio → <em>Spiega</em> / <em>Traduci</em> / <em>Chiedi</em> (AI locale attiva); la risposta si può salvare nella Nota del documento.</li>
            <li><strong>Manda agli Appunti</strong>: selezione → radiale o barretta dell'evidenziazione → scegli l'appunto → entra come citazione col riferimento <code>[[@citekey]]</code> in coda. Le <strong>fonti numerate</strong> di «Chiedi alla libreria» aprono il PDF alla pagina giusta.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Estrarre dal PDF: tabelle, testo, formule, figure</h3>
          <ul>
            <li>Premi il tasto della modalità (<kbd>T</kbd> tabella, <kbd>X</kbd> testo, <kbd>F</kbd> formula, <kbd>G</kbd> figura — o le icone/⋯) e <strong>trascina un rettangolo</strong> sulla zona. La finestra che si apre è trascinabile e ridimensionabile; ovunque hai <strong>Copia / Salva / → Appunti</strong> con il <strong>formato a scelta</strong> (l'anteprima mostra esattamente cosa esce). Ciò che mandi agli Appunti porta con sé il riferimento <code>[[@citekey]]</code>.</li>
            <li><strong>Tabella</strong> (<kbd>T</kbd>) — tre motori nell'intestazione: <strong>Nativa</strong> (dal testo del PDF, veloce, per tabelle semplici), <strong>Modello</strong> (riconosce righe/colonne/intestazioni dall'immagine e prende il testo <em>esatto</em> dal PDF — il migliore sui paper; ~111 MB scaricati al primo uso; non lavora su pagine ruotate), <strong>Ollama</strong> (modello di visione, per scansioni). Export: griglia, CSV, Markdown, Excel, LaTeX (booktabs), più «Migliora con AI».</li>
            <li><strong>Testo</strong> (<kbd>X</kbd>) — <strong>Nativa</strong> conserva la <strong>formattazione</strong> (corsivo, grassetto, apici/pedici → Markdown, es. H<sub>2</sub>O); <strong>Ollama</strong> fa l'OCR delle pagine scansionate. Export: testo, Markdown, LaTeX.</li>
            <li><strong>Formula</strong> (<kbd>F</kbd>) — motore <strong>Locale</strong> (math-OCR integrato, ~115 MB al primo uso) o <strong>Ollama</strong>; «<strong>Più righe</strong>» per equazioni impilate. Il LaTeX riconosciuto è <strong>modificabile</strong> con anteprima resa in tempo reale; il pulsante <code>\mathrm&#123;&#125;</code> mette tutto in tondo. Export: LaTeX o <code>$$…$$</code>.</li>
            <li><strong>Figura</strong> (<kbd>G</kbd>) — ritaglia qualsiasi zona come immagine: <strong>Salva PNG…</strong> o <strong>→ Appunti</strong>.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>F</kbd></td><td>Cerca nel documento</td></tr>
              <tr><td><kbd>+</kbd> / <kbd>−</kbd> / <kbd>0</kbd></td><td>Ingrandisci / riduci / zoom 100%</td></tr>
              <tr><td><kbd>W</kbd> / <kbd>H</kbd></td><td>Adatta alla larghezza / alla pagina</td></tr>
              <tr><td><kbd>2</kbd></td><td>Vista a due pagine</td></tr>
              <tr><td><kbd>N</kbd></td><td>Aggiungi una nota puntuale</td></tr>
              <tr><td><kbd>A</kbd> / <kbd>E</kbd></td><td>Pannello Annotazioni / Nota del documento</td></tr>
              <tr><td><kbd>T</kbd> / <kbd>X</kbd> / <kbd>F</kbd> / <kbd>G</kbd></td><td>Estrai tabella / testo / formula / figura (ripremi per annullare)</td></tr>
              <tr><td><kbd>I</kbd></td><td>Modalità notte (inverti colori)</td></tr>
              <tr><td><kbd>[</kbd> / <kbd>]</kbd></td><td>Ruota a sinistra / destra</td></tr>
              <tr><td><kbd>Ctrl</kbd>+rotella</td><td>Zoom continuo</td></tr>
              <tr><td><kbd>Esc</kbd></td><td>Chiudi / annulla</td></tr>
              <tr><td><kbd>?</kbd></td><td>Scorciatoie (dentro il lettore)</td></tr>
            </tbody>
          </table>
        </div>

        {:else if helpTab === "scrittura"}
        <div class="helpsec">
          <h3>Wiki della libreria</h3>
          <ul>
            <li>La tua <strong>enciclopedia privata</strong>: una pagina per concetto, scritta dall'AI locale leggendo solo i tuoi documenti (icona <strong>Wiki</strong> sulla barra).</li>
            <li>Le fonti, in tre modi: <strong>«Genera/aggiorna dai tag»</strong> (una pagina per ogni tag con almeno 2 documenti); un <strong>concetto libero</strong> (i paper pertinenti li trova la ricerca semantica); oppure <strong>scegli tu le fonti</strong> — seleziona i documenti nella griglia → tasto destro → <em>Pagina wiki (AI)</em>: la pagina usa esattamente quelli (max 10).</li>
            <li>Le citazioni <strong>[n]</strong> nel testo aprono il PDF <strong>alla pagina giusta</strong>; i concetti citati in altre pagine diventano <strong>link</strong> tra pagine; in fondo trovi le fonti con i passaggi usati (chip «p. N»).</li>
            <li>Ogni fonte <em>deve</em> comparire nella pagina: se la sintesi non la usa, viene dichiarata in «Fonti non integrate» — mai omessa in silenzio. Il pallino <strong>●</strong> sull'elenco segnala che la libreria è cambiata e conviene rigenerare.</li>
            <li>Richiede l'AI locale attiva e l'indice dei passaggi (Chiedi alla libreria → Costruisci indice). Consiglio: un modello ≥ 8B (es. <code>gemma3:27b</code>) per una prosa all'altezza.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Appunti</h3>
          <ul>
            <li><strong>File .md veri</strong> nella cartella dell'app: li apri e modifichi anche da un editor esterno o dal terminale (l'indice si riallinea a ogni avvio). Icona <strong>Appunti</strong> sulla barra; elenco ordinabile per modifica/creazione/titolo. <em>Diversi</em> dalle Annotazioni (ancorate al PDF) e dalla Nota del documento (una per paper).</li>
            <li><strong>[[Collegamenti]]</strong>: <code>[[Titolo di un appunto]]</code> collega un appunto; <code>[[@citekey]]</code> o <code>[[Titolo di un paper]]</code> collegano un documento (clic → si apre). In fondo i <strong>backlink</strong>. Cercabili dalla barra di ricerca.</li>
            <li><strong>Editor</strong> con salvataggio automatico e tre modalità: <em>Modifica</em>, <em>Affiancato</em> (anteprima live), <em>Anteprima</em>. Barra di formattazione (titoli, grassetto, liste, sposta blocchi), <strong>formule</strong> con <code>$$ … $$</code> (rese in anteprima), <strong>immagini</strong> trascinate o incollate (salvate come file in <code>assets/</code>, nel testo resta un riferimento breve). <strong>Rinomina</strong> (o doppio clic sul titolo) rinomina anche il file.</li>
            <li><strong>Esporta</strong> (sotto il titolo): <strong>MD</strong> · <strong>HTML</strong> (pagina autonoma) · <strong>LaTeX</strong> (.tex + figure in cartella) · <strong>PDF</strong> — formule e figure incluse.</li>
            <li>Gli appunti <strong>ricevono</strong> dal lettore: testo selezionato, tabelle, formule e figure estratte, sempre con il riferimento <code>[[@citekey]]</code>; dal pannello dettagli anche Abstract e Riassunto AI; le <strong>Rassegne AI</strong> si salvano come appunto. Con l'Indice semantico, gli appunti compaiono anche nella <strong>Costellazione</strong> (rombi viola).</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Progetti (LaTeX) — un piccolo Overleaf locale</h3>
          <ul>
            <li>Icona <strong>Progetti (LaTeX)</strong> sulla barra. Ogni progetto è una <strong>cartella vera</strong> (in <code>projects/</code> nei dati dell'app) con <code>main.tex</code> e <code>refs.bib</code>. Crea da <strong>5 modelli</strong> (articolo, paper a due colonne, relazione/tesi, presentazione beamer, minimale) oppure «<strong>Da .zip…</strong>» con un template scaricato — i link a <strong>Overleaf / IEEE / ACM / Springer / Elsevier</strong> sono lì sotto.</li>
            <li><strong>Cita</strong>: cerca nella tua libreria e inserisce <code>\cite&#123;citekey&#125;</code> al cursore. <strong>Sincronizza bibliografia</strong>: riscrive <code>refs.bib</code> con tutta la libreria. Salvataggio automatico (o <kbd>Ctrl</kbd>+<kbd>S</kbd>).</li>
            <li><strong>Compila</strong> usa il compilatore di sistema: <strong>Tectonic</strong>, oppure <strong>MiKTeX</strong> (via texify, non serve Perl), oppure latexmk. L'anteprima del PDF appare accanto all'editor; se il PDF esce con avvisi lo vedi comunque, col log a un clic. Senza compilatore: <code>winget install Tectonic.Tectonic</code>.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Strumenti di sintesi sulla selezione</h3>
          <ul>
            <li><strong>Confronta (AI)</strong>: seleziona 2-3 paper → tasto destro → <em>Confronta</em>: tabella obiettivo/metodo/dati/risultati/limiti + cosa aggiunge ciascuno.</li>
            <li><strong>Rassegna (AI)</strong>: <strong>seleziona da 2 a 10 paper</strong> → tasto destro → <em>Rassegna (AI)</em>. Ottieni una <strong>mini related-work</strong> (300-500 parole) <strong>organizzata per temi</strong> (non paper-per-paper): confronta gli approcci, evidenzia disaccordi e chiude con <em>«Lacune aperte»</em>. Ogni paper è citato con <strong>[n] cliccabili</strong> (aprono la fonte) e nessuna fonte viene omessa in silenzio (le non integrate finiscono in coda). Puoi copiarla pronta per <strong>LaTeX</strong> (<code>\cite&#123;citekey&#125;</code>) o <strong>Pandoc</strong> (<code>[@citekey]</code>), oppure <strong>«Salva negli Appunti»</strong>: diventa un <strong>appunto .md</strong> dove le <em>[n]</em> sono riscritte come <strong>backlink <code>[[@citekey]]</code></strong> — così ogni paper citato rimanda alla rassegna, ed è cercabile e modificabile come ogni appunto. Richiede l'AI locale attiva.</li>
            <li><strong>Tabella risultati (AI)</strong>: raccoglie i numeri (metodo · dataset · metrica · valore) dei paper selezionati in un'unica tabella esportabile in CSV/Markdown/Excel. I valori sono estratti testualmente: verifica sempre sul PDF.</li>
            <li><strong>Percorso di lettura</strong> (tasto destro → AI): per capire un paper, cosa leggere prima — i fondamenti che cita (già tuoi), i vicini di contenuto precedenti, e i riferimenti mancanti da aggiungere con un click. Funziona <em>senza</em> LLM.</li>
          </ul>
        </div>

        {:else if helpTab === "ai"}
        <div class="helpsec">
          <h3>AI locale (opzionale, mai automatica)</h3>
          <ul>
            <li>Serve <strong>Ollama</strong> o <strong>LM Studio</strong> sul tuo PC: configura URL e modello in <strong>Impostazioni → AI locale</strong>. Il <strong>chip AI</strong> in alto dice lo stato: se leggi «AI off», un clic la riattiva. Consiglio: un modello ≥ 8B (es. <code>gemma3:27b</code>).</li>
            <li>Cosa abilita: <strong>riassunti</strong> e <strong>tag automatici</strong>, <strong>Lente AI</strong> nel lettore, <strong>Chiedi alla libreria</strong>, <strong>Wiki</strong>, <strong>Confronta/Rassegna</strong>, i motori <strong>Ollama</strong> di tabelle/testo/formule, «Migliora con AI» sulle tabelle. Le schede con riassunto mostrano <strong>✦ AI</strong>; il batch sulla selezione <strong>salta</strong> chi ha già riassunto/tag.</li>
            <li><strong>Chiedi alla libreria</strong> (barra): fai una domanda in linguaggio naturale → risposta con <strong>fonti numerate</strong> che aprono i PDF alla pagina giusta. Alla prima visita premi «<strong>Costruisci indice</strong>» (l'indice dei passaggi, riusato anche dalla Wiki); rigeneralo quando la libreria cambia molto.</li>
            <li><strong>Memoria AI</strong> (barra): «<strong>Libera GPU</strong>» scarica i modelli dalla VRAM lasciando tutto attivo (si ricaricano al bisogno); «<strong>Ferma AI</strong>» scarica i modelli, chiude davvero il server locale e disattiva l'AI.</li>
            <li>L'<strong>Indice semantico</strong> (barra) non è l'AI generativa: è l'indice dei significati, tutto locale. Abilita ricerca <em>Semantica</em>, <em>Correlati</em>, <strong>Costellazione</strong> e gli appunti nel grafo. Rilancialo dopo aver aggiunto documenti o appunti.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>I tuoi dati — tutto locale</h3>
          <ul>
            <li>Cartella dell'app: <code>%APPDATA%\com.pdfmanage.app</code> — <code>pdfmanage.db</code> (il catalogo), <code>papers/</code> (i PDF scaricati; quelli importati dal disco <em>restano dove sono</em>), <code>notes/</code> (gli appunti .md + <code>assets/</code>), <code>projects/</code> (i progetti LaTeX), <code>thumbnails/</code> e i modelli locali (<code>mathocr</code>, <code>tablestruct</code>, <code>fastembed_cache</code>).</li>
            <li>Appunti e progetti sono <strong>file veri</strong>: modificarli da fuori è previsto. <strong>Backup libreria</strong> (barra) fa una copia completa; <strong>Esporta</strong> produce citazioni (BibTeX/RIS/CSL) o note per <strong>Obsidian</strong>.</li>
            <li><strong>Terminale</strong> integrato (&gt;_, es. per <code>claude code</code>); la CLI <code>scriptorium-cli</code> interroga da fuori, in sola lettura, libreria <em>e</em> Appunti <em>e</em> progetti LaTeX (<code>query</code>, <code>bib</code>, <code>notes</code>, <code>note</code>, <code>search-notes</code>, <code>projects</code>, <code>stats</code>…). Il <strong>server MCP</strong> <code>scriptorium-mcp</code> porta gli stessi dati (9 strumenti, sola lettura) dentro <strong>Claude Desktop / Claude Code</strong> e qualsiasi client MCP: config pronta da copiare in <strong>Impostazioni → CLI e MCP</strong>. Il <strong>connettore browser</strong> per «Aggancia» è un servizio solo-locale, spegnibile in Impostazioni. <strong>11 temi</strong> in Aspetto.</li>
          </ul>

          <h3>Plancia — il sinottico dei processi</h3>
          <ul>
            <li>La <strong>Plancia</strong> (icona tachimetro sulla barra, o Ctrl+K → «Plancia») apre una <strong>finestra separata</strong> con un <strong>sinottico visivo</strong> dei processi interni: import, estrazione, metadati, indici, AI, backup… Puoi <strong>tenerla in background</strong> su un lato dello schermo: si illumina <em>solo</em> ciò che sta lavorando davvero, con avanzamento (<code>12/96</code>) e durata; da ferma è spenta.</li>
            <li><strong>Stato ed errori a colpo d'occhio</strong>: un guasto accende il nodo in rosso con il <strong>motivo</strong> per esteso; un problema non bloccante (es. un file su cento) lo segna in ambra senza fermare il resto. I sottosistemi spenti dicono <em>perché</em> («online disattivato», «modelli da scaricare»…). Clic su un nodo: descrizione, statistiche, storico.</li>
            <li>Il <strong>registro attività</strong> in basso è filtrabile (Tutti/Errori) ed esportabile con <strong>Salva registro…</strong>; da <strong>Impostazioni → Manutenzione</strong> puoi farlo scrivere anche <strong>su file</strong> (uno al giorno, conservati 14) per capire a posteriori cosa è successo.</li>
          </ul>
        </div>

        {:else if helpTab === "faq"}
        <div class="helpsec">
          <h3>Come faccio a…?</h3>
          <dl class="faq">
            <dt>…aggiungere il PDF che ho appena scaricato col browser?</dt>
            <dd>Copia il link del PDF e torna su Scriptorium: compare «Aggancia». Oppure punta la Cartella sorvegliata su Download: entra da solo.</dd>
            <dt>…portare la mia libreria da Zotero, Mendeley o EndNote?</dt>
            <dd>Nel gestore fai <strong>Esporta</strong> in <strong>BibTeX/BibLaTeX, RIS o CSL-JSON</strong> (per avere anche i PDF, in Zotero spunta «Esporta file»). Poi barra → Importa → <strong>Da gestore bibliografico…</strong>, scegli il file e — se i PDF stanno in una cartella a parte — indicala quando te lo chiede. Metadati, PDF e parole chiave (→ tag) entrano insieme, senza doppioni.</dd>
            <dt>…sistemare un paper arrivato senza titolo o con metadati sbagliati?</dt>
            <dd>Clic su «✦ N senza metadati» in alto per il recupero in blocco (solo abbinamenti sicuri). Per il caso singolo: tasto destro → Organizza → <strong>Recupera metadati…</strong> mostra i candidati trovati online con le prove nel PDF e applichi quello giusto (o incolli un DOI/arXiv). Ritocchi a mano: Modifica metadati. Per tutta la libreria: Impostazioni → Manutenzione → «Verifica e ripara metadati».</dd>
            <dt>…organizzare i paper in cartelle e sottocartelle, anche su disco?</dt>
            <dd>Apri l'<strong>Archivio</strong> (icona cartella): crei raccolte e sotto-raccolte e trascini i paper tra i nodi; «<strong>✦ Suggerisci</strong>» propone cosa metterci (con soglia di confidenza). Vuoi vederle anche in Esplora risorse o dal terminale? Attiva lo <strong>Specchio su disco</strong> (chip in alto): una cartella con l'albero delle raccolte e i PDF con nomi leggibili, sempre sincronizzata, senza occupare spazio in più.</dd>
            <dt>…vedere cosa sta facendo l'app in questo momento (e perché qualcosa è fallito)?</dt>
            <dd>Apri la <strong>Plancia</strong> (icona tachimetro sulla barra): un sinottico in finestra separata, da tenere anche in background, dove si illumina solo ciò che sta lavorando. Gli errori accendono il nodo in rosso col motivo per esteso; clic sul nodo per dettagli e storico; «Salva registro…» esporta la sessione. Da Impostazioni → Manutenzione puoi registrare l'attività anche su file.</dd>
            <dt>…copiare una citazione pronta?</dt>
            <dd>Tasto destro sul paper → Cita: APA, IEEE, BibTeX, citekey, <code>\cite</code>, <code>[@…]</code>. Con più paper selezionati ottieni <code>\cite&#123;k1,k2&#125;</code> o tutte le voci BibTeX insieme.</dd>
            <dt>…mandare un paper a un collega?</dt>
            <dd>Tasto destro → Condividi: WhatsApp/Teams/Gmail aprono il messaggio pronto e il PDF è già negli appunti di sistema (incollalo con <kbd>Ctrl</kbd>+<kbd>V</kbd>); Outlook desktop lo allega da solo.</dd>
            <dt>…estrarre una tabella che viene male?</dt>
            <dd>Nella finestra della tabella cambia motore: <strong>Modello</strong> per le tabelle dei paper (anche senza bordi), <strong>Ollama</strong> per le scansioni. Il rettangolo deve coprire tutta la tabella.</dd>
            <dt>…correggere una formula riconosciuta male?</dt>
            <dd>Il LaTeX nella finestra è modificabile e l'anteprima si aggiorna live. Se sono più equazioni impilate prova «Più righe»; in alternativa cambia motore.</dd>
            <dt>…portare un passaggio di un paper nei miei appunti, con la fonte?</dt>
            <dd>Seleziona il testo nel lettore → «Manda agli Appunti» (radiale o barretta): entra come citazione con <code>[[@citekey]]</code> in coda, che diventa un backlink.</dd>
            <dt>…scrivere una formula in un appunto?</dt>
            <dd>Scrivi <code>$$ E = mc^2 $$</code>: la vedi resa in Affiancato/Anteprima e negli export.</dd>
            <dt>…mettere un'immagine in un appunto?</dt>
            <dd>Trascinala o incollala nell'editor: viene salvata come file in <code>assets/</code> e nel testo resta un riferimento breve.</dd>
            <dt>…esportare un appunto in PDF?</dt>
            <dd>Apri l'appunto → pulsante <strong>PDF</strong> → nella finestra di stampa scegli «Salva come PDF». Ci sono anche MD, HTML e LaTeX.</dd>
            <dt>…scrivere un articolo LaTeX citando la mia libreria?</dt>
            <dd>Barra → Progetti (LaTeX) → Crea (scegli un modello). «Cita» inserisce <code>\cite&#123;…&#125;</code> dalla libreria, «Compila» produce il PDF con anteprima. Per un template ufficiale: scaricalo in .zip e usa «Da .zip…».</dd>
            <dt>…compilare se MiKTeX chiede Perl o non ho un compilatore?</dt>
            <dd>Con MiKTeX la compilazione passa da texify e non serve Perl. Se non hai nulla: <code>winget install Tectonic.Tectonic</code> (si scarica i pacchetti da solo).</dd>
            <dt>…sapere quali paper citati mi mancano?</dt>
            <dd>Barra → Cura della libreria → Gap di citazioni. «Risolvi DOI dei riferimenti» ne fa entrare altri nel conteggio.</dd>
            <dt>…seguire un tema e accorgermi dei paper nuovi?</dt>
            <dd>Cerca online → ★ Salva la ricerca. A ogni avvio i risultati nuovi si raccolgono nella campana Novità.</dd>
            <dt>…perché la Costellazione è vuota o mancano i paper nuovi?</dt>
            <dd>Serve l'Indice semantico: l'icona sulla barra lo costruisce/aggiorna. I nuovi documenti (e appunti) entrano al prossimo aggiornamento.</dd>
            <dt>…riaprire l'ultimo PDF al punto in cui ero?</dt>
            <dd>«Riprendi lettura» sulla barra (o radiale/palette). Ogni PDF comunque riparte dall'ultima pagina letta.</dd>
            <dt>…perché le funzioni AI non partono?</dt>
            <dd>Guarda il chip in alto: se dice «AI off», cliccalo. Poi verifica in Impostazioni → AI locale che il server (Ollama/LM Studio) sia raggiungibile e il modello esista.</dd>
            <dt>…liberare la memoria della GPU senza spegnere tutto?</dt>
            <dd>Barra → Memoria AI → «Libera GPU». «Ferma AI» invece chiude anche il server e disattiva l'AI.</dd>
            <dt>…trovare un comando che non ricordo dove sta?</dt>
            <dd><kbd>Ctrl</kbd>+<kbd>K</kbd> e digitalo: trovi azioni, documenti, appunti, pagine wiki, progetti e le sezioni di questa guida — anche mentre leggi un PDF. Funziona pure nel menu radiale: apri e digita per filtrare.</dd>
          </dl>
        </div>
        {/if}

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
            <button class="setnavitem" class:active={settingsTab === "connector"} onclick={() => { settingsTab = "connector"; loadConnector(); }}>Connettore browser</button>
            <button class="setnavitem" class:active={settingsTab === "mcp"} onclick={() => { settingsTab = "mcp"; loadCompanions(); }}>CLI e MCP</button>
            <button class="setnavitem" class:active={settingsTab === "backup"} onclick={() => (settingsTab = "backup")}>Backup</button>
            <button class="setnavitem" class:active={settingsTab === "maint"} onclick={() => (settingsTab = "maint")}>Manutenzione</button>
          </nav>
          <div class="setpane">
            {#if settingsTab === "online"}
              <p class="dimtext">La ricerca online è una funzione di rete: finché è disattivata, l'app resta 100% offline. I PDF vengono scaricati solo per i lavori Open Access.</p>
              <label class="setrow"><input type="checkbox" bind:checked={discEnabled} /> Abilita funzioni online (ricerca su arXiv, OpenAlex, ADS, Semantic Scholar e altre fonti; Trova PDF, esplorazione citazioni, recupero metadati)</label>
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
            {:else if settingsTab === "connector"}
              <p class="dimtext">
                Aggancia un PDF direttamente dal browser con <strong>un clic</strong>. Trascina una volta il pulsante
                qui sotto nella <strong>barra dei preferiti</strong>; poi, quando sei su un PDF (o su una pagina che
                ne contiene uno), cliccalo e il file finisce nella tua libreria. Perfetto con arXiv e i PDF ad
                accesso aperto.
              </p>
              <label class="setrow">
                <input
                  type="checkbox"
                  checked={connectorInfo?.enabled ?? false}
                  onchange={(e) => toggleConnector(e.currentTarget.checked)}
                />
                Abilita il connettore (server locale su 127.0.0.1) — disattivato di default
              </label>
              {#if connectorInfo}
                <p class="sethint" style="margin-top:-6px;">
                  {#if !connectorInfo.enabled}Disattivato — attiva l'interruttore per usare il bookmarklet.
                  {:else if connectorInfo.running}✓ In ascolto sulla porta <code>{connectorInfo.port}</code>.
                  {:else}⚠ Attivo, ma non ho trovato una porta libera: chiudi eventuali conflitti e riattiva.{/if}
                </p>
              {/if}

              <div class="setlbl">
                Trascina nella barra dei preferiti
                <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin-top:4px;">
                  <!-- svelte-ignore a11y_no_static_element_interactions a11y_missing_attribute -->
                  <a
                    href={bookmarklet}
                    draggable="true"
                    onclick={(e) => { e.preventDefault(); status = "Trascina il pulsante nella barra dei preferiti (non cliccarlo qui)"; }}
                    style="display:inline-block;padding:8px 14px;background:var(--accent);color:var(--on-accent);border-radius:var(--r-pill,999px);font-weight:600;text-decoration:none;cursor:grab;user-select:none;"
                    title="Trascinami nella barra dei preferiti del browser"
                  >📎 Scriptorium</a>
                  <button class="ghost small" onclick={copyBookmarklet} disabled={!bookmarklet}>Copia bookmarklet</button>
                </div>
                <span class="sethint">
                  Non cliccarlo qui: <strong>trascinalo</strong> nella barra dei preferiti. In alternativa premi
                  «Copia bookmarklet», crea un nuovo preferito e incolla il testo come <em>indirizzo/URL</em>.
                </span>
              </div>

              <p class="sethint">
                🔒 Il server ascolta solo in locale (<code>127.0.0.1</code>, non raggiungibile dalla rete) ed è protetto
                da un <strong>token segreto</strong> incluso solo nel tuo bookmarklet: nessun altro sito può aggiungere
                PDF. Il download passa dagli stessi controlli anti-abuso del resto dell'app (solo https, solo file PDF).
                Se disattivi e riattivi il connettore, o cambia la porta, ri-trascina il bookmarklet aggiornato.
              </p>
              <p class="sethint">
                Sui siti che bloccano le richieste dirette (es. <strong>GitHub</strong>), il bookmarklet apre una piccola
                scheda di conferma di Scriptorium che completa l'aggancio. I link ai PDF nelle pagine GitHub
                (<code>…/blob/…</code>) vengono riscritti automaticamente verso il file vero.
              </p>

              <h3 class="settitle">Appunti intelligenti</h3>
              <label class="setrow" title="Quando torni su Scriptorium, se negli appunti c'è un link che sembra un PDF compare un suggerimento «Aggancia»">
                <input type="checkbox" bind:checked={clipAssist} /> Suggerisci l'aggancio dei link PDF copiati
              </label>
              <p class="sethint">
                Il metodo più semplice: <strong>copia il link</strong> del PDF nel browser e torna su Scriptorium —
                comparirà il suggerimento in basso a destra. Gli appunti vengono letti solo quando l'app torna in primo
                piano e non lasciano mai il tuo computer; non parte nulla finché non clicchi «Aggancia».
              </p>
            {:else if settingsTab === "mcp"}
              <p class="dimtext">
                Due compagni <strong>in sola lettura</strong> per usare la libreria da fuori (sicuri anche con l'app
                aperta): la <strong>CLI</strong> per il terminale e il <strong>server MCP</strong> per Claude Desktop /
                Claude Code e qualsiasi client MCP. Nessun servizio resta in ascolto: è il client ad avviare il
                processo quando serve e a chiuderlo a fine sessione.
              </p>
              {#if companions}
                <div class="setlbl">
                  Server MCP — registralo in Claude Code con questo comando
                  <div class="airow">
                    <input readonly value={mcpAddCmd} />
                    <button class="ghost small" onclick={() => copyPlain(mcpAddCmd, "Comando copiato ✓")}>Copia</button>
                  </div>
                  <span class="sethint">
                    {companions.mcp_exists ? "✓ binario presente accanto all'app" : "⚠ binario non trovato — scarica scriptorium-mcp.exe dalle Release e mettilo accanto all'app"}
                    — 9 strumenti: ricerca libreria, schede, BibTeX, appunti, ricerca appunti, progetti LaTeX, statistiche. Solo lettura.
                  </span>
                </div>
                <div class="setlbl">
                  Per Claude Desktop: aggiungi questa voce in <code>claude_desktop_config.json</code> → <code>"mcpServers"</code>
                  <div class="airow">
                    <input readonly value={mcpJsonSnippet} />
                    <button class="ghost small" onclick={() => copyPlain(mcpJsonSnippet, "Config copiata ✓")}>Copia</button>
                  </div>
                </div>
                <div class="setlbl">
                  CLI da terminale (query, list, show, bib, notes, note, search-notes, projects, stats)
                  <div class="airow">
                    <input readonly value={companions.cli} />
                    <button class="ghost small" onclick={() => copyPlain(companions!.cli, "Percorso copiato ✓")}>Copia</button>
                  </div>
                  <span class="sethint">
                    {companions.cli_exists ? "✓ binario presente accanto all'app" : "⚠ binario non trovato — scarica scriptorium-cli.exe dalle Release"}
                    — <code>scriptorium-cli help</code> per tutti i comandi; output JSON, comodo per script e Claude Code.
                  </span>
                </div>
              {:else}
                <p class="dimtext">Carico i percorsi…</p>
              {/if}
            {:else if settingsTab === "backup"}
              <p class="dimtext">Salva una copia completa (database + PDF + miniature) in una cartella a tua scelta.</p>
              <button class="ghost" onclick={doBackup}>Scegli cartella e salva backup…</button>
              <p class="dimtext" style="margin-top:20px;">
                <strong>Ripristina</strong> la libreria da un backup precedente. <strong>Sostituisce</strong> i dati attuali
                (ne salva prima una copia di sicurezza) e riavvia l'app. I dati non vengono mai persi installando o
                disinstallando: vivono in <code>%APPDATA%\com.pdfmanage.app</code>, separati dal programma.
              </p>
              <button class="ghost" onclick={doRestoreFolder}>Ripristina da cartella di backup…</button>
              <button class="ghost" style="margin-top:8px;" onclick={doRestoreDbFile}>…o da un file .db (backup automatici in <code>backups\</code>)</button>
            {:else}
              <p class="dimtext">
                <strong>Verifica e ripara metadati.</strong> Controlla ogni documento e corregge quelli il cui
                titolo non corrisponde al PDF — di solito perché l'arricchimento ha pescato il DOI di un
                <em>lavoro citato</em> invece di quello del documento. I paper <strong>arXiv</strong> recuperano
                i dati corretti da arXiv (anche quelli ancora senza metadati); per gli altri il titolo viene
                ricavato dalla prima riga del PDF. I documenti già corretti non vengono toccati.
                È sicuro e ripetibile; può richiedere fino a un minuto.
              </p>
              <button class="ghost" onclick={repairMeta} disabled={repairing || docs.length === 0}>
                {repairing ? "Riparazione in corso…" : "Verifica e ripara metadati"}
              </button>
              {#if repairMsg}<p class="sethint" style="margin-top:8px;">{repairMsg}</p>{/if}

              <p class="dimtext" style="margin-top:20px;">
                <strong>Plancia — registro su file.</strong> La Plancia (icona tachimetro) mostra in tempo reale i
                processi interni; qui puoi far scrivere lo stesso registro anche su file, uno al giorno
                (conservati gli ultimi 14), utile per capire a posteriori cosa è successo.
              </p>
              <label class="setrow" style="display:flex;align-items:center;gap:8px;">
                <input type="checkbox" checked={pulseLog} onchange={togglePulseLog} />
                Registra l'attività della Plancia su file
              </label>
              {#if pulseLog && pulseLogDir}
                <p class="sethint">
                  I file sono in <code>{pulseLogDir}</code>
                  <button class="linklike" onclick={() => void pulseRevealLogs()}>apri cartella</button>
                </p>
              {/if}
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
  /* Lock the chrome: the app fills the window exactly, so header/strip/sidebar
     stay put and only the document area (.main, overflow:auto) scrolls. */
  .app { height: 100vh; overflow: hidden; display: flex; flex-direction: column; position: relative; }
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
  /* AI switched off: muted but clearly clickable (one click re-enables). */
  .aichip.off { color: var(--faint); border-style: dashed; }
  .aichip.off:hover { color: var(--accent); border-color: var(--accent); border-style: solid; }
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
  /* ===== Orbita chrome: header icon controls, ambient chips, popovers ===== */
  .railtoggle, .iconbtn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 34px; height: 34px; flex: 0 0 auto;
    background: transparent; color: var(--dim);
    border: 1px solid transparent; border-radius: var(--r-sm); cursor: pointer;
    transition: background var(--ease), color var(--ease), border-color var(--ease);
  }
  .railtoggle:hover, .iconbtn:hover { background: var(--hover); color: var(--accent); border-color: var(--border-soft); }
  .railtoggle svg, .iconbtn svg { width: 19px; height: 19px; }
  .idxchip { font-variant-numeric: tabular-nums; }
  .idxchip.busy .aidot { background: var(--accent); animation: idxpulse 1.1s ease-in-out infinite; }
  @keyframes idxpulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.25; } }
  .ambient {
    display: inline-flex; align-items: center; gap: 6px; white-space: nowrap;
    background: var(--accent-soft); color: var(--accent);
    border: 1px solid var(--accent-soft2); border-radius: var(--r-pill);
    padding: 6px 13px; font-size: 12.5px; font-weight: 600; cursor: pointer;
    transition: background var(--ease), box-shadow var(--ease);
  }
  .ambient:hover:not(:disabled) { box-shadow: var(--shadow-sm); }
  .ambient:disabled { opacity: 0.6; cursor: default; }
  /* slim indeterminate-ish progress line under the header while embedding */
  .headprog { height: 3px; background: var(--border-soft); }
  .headprog .fill { height: 100%; }
  .pop {
    position: fixed; z-index: 70;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(12px);
    border: 1px solid var(--border); border-radius: var(--r-md);
    box-shadow: var(--shadow-lg); padding: 12px 14px; max-width: 380px;
  }
  .sortpop { top: 108px; right: 22px; }
  .indexpop { top: 58px; left: 220px; }
  .poptitle { font-size: 12px; font-weight: 700; color: var(--text); margin-bottom: 8px; font-family: var(--serif); }
  .popnote { font-weight: 400; color: var(--faint); font-size: 11px; margin-left: 6px; }
  .popbody { font-size: 12.5px; color: var(--dim); margin: 0 0 10px; line-height: 1.5; }
  .poprow { display: flex; align-items: center; gap: 10px; }
  .popchips { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 6px; }
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
  /* one quiet strip replaces the old toolbar + sort bar */
  .strip {
    position: relative;
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 6px 22px; background: var(--bg); border-bottom: 1px solid var(--border-soft);
    min-height: 40px;
  }
  .stripleft, .stripright { display: flex; align-items: center; gap: 10px; }
  .seg {
    display: inline-flex; align-items: center; gap: 2px;
    background: var(--panel); border: 1px solid var(--border-soft);
    border-radius: var(--r-pill); padding: 2px;
  }
  .segbtn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 26px; border: none; border-radius: var(--r-pill);
    background: transparent; color: var(--dim); cursor: pointer;
    transition: background var(--ease), color var(--ease);
  }
  .segbtn svg { width: 16px; height: 16px; }
  .segbtn:hover { color: var(--accent); }
  .segbtn.active { background: var(--surface); color: var(--accent); box-shadow: var(--shadow-sm); }
  .chipbtn {
    background: transparent; color: var(--dim); border: 1px solid transparent;
    border-radius: var(--r-pill); padding: 4px 12px; font-size: 12.5px; cursor: pointer;
    transition: background var(--ease), color var(--ease), border-color var(--ease);
  }
  .chipbtn:hover { color: var(--accent); border-color: var(--border); }
  .chipbtn.on { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); font-weight: 600; }
  /* grid thumbnail zoom (− slider +) */
  .gridzoom { display: flex; align-items: center; gap: 4px; margin-left: 4px; padding-left: 8px; border-left: 1px solid var(--border); }
  .zbtn {
    width: 24px; height: 24px; border-radius: 6px; border: 1px solid var(--border);
    background: transparent; color: var(--dim); cursor: pointer; font-size: 15px; line-height: 1;
    display: flex; align-items: center; justify-content: center;
  }
  .zbtn:hover { border-color: var(--accent-soft2); color: var(--accent); }
  .zrange { width: 96px; accent-color: var(--accent); cursor: pointer; }
  .hint { font-size: 12px; color: var(--faint); white-space: nowrap; }
  .bar { width: 140px; height: 6px; background: var(--border); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 0.2s; }
  .sortchip {
    display: inline-flex; align-items: center; gap: 4px;
    background: transparent; color: var(--dim); border: 1px solid var(--border);
    border-radius: 999px; padding: 3px 11px; font-size: 12.5px; cursor: pointer; transition: background 0.12s, color 0.12s;
  }
  .sortchip:hover { color: var(--accent); border-color: var(--accent-soft2); }
  .sortchip.on { background: var(--accent-soft); color: var(--accent); border-color: var(--accent-soft2); font-weight: 600; }
  .sortchip .sar { font-size: 9px; }
  .sortchip .srank {
    font-size: 9px; font-weight: 700; background: var(--accent); color: var(--on-accent);
    border-radius: 50%; width: 14px; height: 14px; display: inline-flex; align-items: center; justify-content: center;
  }
  .sortclear { background: transparent; border: none; color: var(--faint); font-size: 12px; cursor: pointer; padding: 3px 6px; text-decoration: underline; }
  .sortclear:hover { color: var(--danger); }
  .list th .ar.rnk {
    font-size: 8px; font-weight: 700; background: var(--accent); color: var(--on-accent);
    border-radius: 50%; padding: 0 3px; margin-left: 2px;
  }
  /* quiet toast stack, bottom-right: status messages + batch-AI progress */
  .toasts {
    position: fixed; right: 18px; bottom: 18px; z-index: 74;
    display: flex; flex-direction: column; align-items: flex-end; gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex; align-items: center; gap: 10px; pointer-events: auto;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(10px);
    border: 1px solid var(--border); border-radius: var(--r-md);
    box-shadow: var(--shadow-md); padding: 9px 14px;
    color: var(--text); font-size: 12.5px; max-width: min(460px, 80vw);
    animation: toastin 0.16s cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes toastin { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) { .toast { animation: none; } }
  /* "appunti intelligenti": the offered clipboard link */
  .toast.clipoffer { border-color: var(--accent-soft2); }
  .clipbody { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .cliptitle { font-weight: 700; color: var(--accent); font-size: 12px; }
  .clipurl { max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--dim); font-size: 11.5px; }
  .clipx { color: var(--dim); }

  .body { flex: 1; display: flex; min-height: 0; }
  .sidebar {
    width: 222px; flex: 0 0 222px; background: var(--panel); border-right: 1px solid var(--border);
    padding: 12px 10px; overflow: auto;
    transition: margin-left 0.22s cubic-bezier(0.4, 0, 0.2, 1), opacity 0.18s ease;
  }
  .sidebar.collapsed { margin-left: -223px; opacity: 0; pointer-events: none; }
  @media (prefers-reduced-motion: reduce) { .sidebar { transition: none; } }
  .sidehint { margin: 14px 6px 4px; font-size: 10.5px; color: var(--faint); line-height: 1.5; cursor: help; }
  .sec { font-size: 11px; text-transform: uppercase; letter-spacing: 0.6px; color: var(--faint); font-weight: 600; margin: 16px 6px 6px; }
  .navrow { display: flex; align-items: center; }
  .navitem {
    flex: 1; text-align: left; background: transparent; border: none; color: var(--text);
    border-radius: 7px; padding: 7px 9px; font-size: 13px; cursor: pointer;
    display: flex; align-items: center; gap: 8px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
  }
  .navitem:hover { background: var(--hover); }
  .navitem.active { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  .navcheck { margin-left: 5px; color: var(--accent); font-size: 12px; }
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
  .topbars > .fbanner { position: static; }
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
    /* Skip rendering/layout of off-screen cards (each carries a base64 cover <img>):
       the browser paints them only as they near the viewport. `auto` in the intrinsic
       size makes it remember each card's real height after first render, so the
       scrollbar stays stable. Big win on large libraries; no windowing JS needed. */
    content-visibility: auto;
    contain-intrinsic-size: auto 320px;
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
  .thumb { position: relative; aspect-ratio: 3 / 4; background: var(--thumb-bg); display: flex; align-items: center; justify-content: center; overflow: hidden; border-bottom: 1px solid var(--border); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  /* Reading-progress bar pinned to the bottom of the cover. */
  .progress { position: absolute; left: 0; right: 0; bottom: 0; height: 4px; background: color-mix(in srgb, var(--border) 70%, transparent); }
  .progress .pfill { height: 100%; background: var(--accent); transition: width var(--ease); }
  .progress.done .pfill { background: var(--ok, #3a9d5b); }
  .thumb-placeholder { color: var(--thumb-fg); font-size: 28px; font-weight: 700; font-family: var(--serif); }
  /* reference-only entries: no file attached, say it instead of a misleading "PDF" */
  .thumb-placeholder.refonly {
    font-size: 13px; font-weight: 600; font-family: var(--sans);
    color: var(--dim); text-align: center; line-height: 1.4; padding: 0 14px;
    border: 1.5px dashed var(--border); border-radius: var(--r-sm); margin: 14px;
  }
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
  .inlibtag { background: var(--border); color: var(--dim); margin-left: 6px; margin-right: 0; }
  /* "Novità" feed view */
  .novhead { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; padding: 4px 4px 14px; border-bottom: 1px solid var(--border-soft); margin-bottom: 14px; flex-wrap: wrap; }
  .novtitle h2 { margin: 0; font-family: var(--serif); font-size: 22px; font-weight: 600; color: var(--text); }
  .novsub { font-size: 12.5px; color: var(--faint); }
  .novintro .big { font-size: 17px; }
  .novfeed { display: flex; flex-direction: column; gap: 22px; }
  .novgroup { display: flex; flex-direction: column; gap: 8px; }
  .novgh { display: flex; align-items: center; gap: 10px; padding: 0 2px 2px; }
  .novgname { font-family: var(--serif); font-size: 15px; font-weight: 600; color: var(--text); }
  .novgcount {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 18px; height: 18px; padding: 0 6px; font-size: 11px; font-weight: 700;
    border-radius: 9px; background: var(--accent-soft); color: var(--accent);
  }
  .novgh .hflink { margin-left: auto; }
  .novcard {
    display: flex; align-items: flex-start; gap: 14px; justify-content: space-between;
    background: var(--panel); border: 1px solid var(--border-soft); border-radius: var(--r-md);
    padding: 11px 14px; transition: border-color var(--ease);
  }
  .novcard:hover { border-color: var(--border); }
  .novmain { min-width: 0; flex: 1 1 auto; }
  .novtl { display: flex; align-items: baseline; gap: 6px; flex-wrap: wrap; }
  .novt { font-size: 14px; font-weight: 600; color: var(--text); line-height: 1.35; }
  .novmeta { font-size: 12px; color: var(--faint); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; }
  .novabs { font-size: 12.5px; color: var(--dim); line-height: 1.5; margin: 8px 0 2px; max-width: 70ch; }
  .novact { display: flex; align-items: center; gap: 8px; flex: 0 0 auto; }
  .ghost.primary { border-color: var(--accent); color: var(--accent); }
  .ghost.primary:hover:not(:disabled) { background: var(--accent-soft); }
  /* Guida: finestra flottante non modale — trascinabile, ridimensionabile,
     opzionalmente in primo piano (sopra lettore e modali, sotto radiale/conferme) */
  .helpwin {
    position: fixed; z-index: 45; width: 660px; height: min(78vh, 800px);
    min-width: 380px; min-height: 260px; max-width: calc(100vw - 24px); max-height: calc(100vh - 24px);
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-lg);
    box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; overflow: hidden; resize: both;
  }
  .helpwin.pinned { z-index: 85; }
  .helpdrag {
    display: flex; align-items: center; gap: 12px; padding: 9px 14px;
    background: var(--panel); border-bottom: 1px solid var(--border-soft);
    cursor: grab; user-select: none; flex: 0 0 auto;
  }
  .helpdrag:active { cursor: grabbing; }
  .helpdrag h2 { margin: 0; flex: 1; font-size: 15px; font-family: var(--serif); font-weight: 600; color: var(--text); }
  .helppinlbl { display: flex; align-items: center; gap: 5px; font-size: 12px; color: var(--dim); cursor: pointer; white-space: nowrap; }
  .helpx { background: none; border: none; color: var(--dim); font-size: 18px; line-height: 1; cursor: pointer; padding: 0 2px; }
  .helpx:hover { color: var(--danger); }
  .helpbody { flex: 1; overflow-y: auto; padding: 12px 20px 20px; }
  .helpbody > .dimtext { color: var(--dim); font-size: 13px; margin: 0 0 10px; }
  /* linguette come semplici tab sottolineate, senza riquadro */
  .helptabs { display: flex; flex-wrap: wrap; gap: 2px 16px; border-bottom: 1px solid var(--border); }
  .helptabs button {
    background: none; border: none; padding: 5px 2px 7px; margin-bottom: -1px;
    font-size: 13px; color: var(--dim); cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: color var(--ease), border-color var(--ease);
  }
  .helptabs button:hover { color: var(--accent); }
  .helptabs button.active { color: var(--accent); font-weight: 600; border-bottom-color: var(--accent); }
  .faq { margin: 0; }
  .faq dt { font-size: 13px; font-weight: 600; color: var(--text); margin-top: 10px; }
  .faq dd { margin: 2px 0 0; font-size: 13px; line-height: 1.5; color: var(--dim); }
  .helpsec { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border); }
  .helpsec h3 { font-size: 14px; font-family: var(--serif); margin: 0 0 8px; color: var(--text); }
  .helpsec ul { margin: 0; padding-left: 18px; }
  .helpsec li { font-size: 13px; line-height: 1.55; color: var(--text); margin: 4px 0; }
  .kbdtable { width: 100%; border-collapse: collapse; margin-top: 10px; }
  .kbdtable td { padding: 4px 8px; font-size: 12.5px; border-bottom: 1px solid var(--border-soft); color: var(--dim); }
  .kbdtable td:first-child { white-space: nowrap; width: 1%; }
  .helpwin kbd {
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
  /* Clickable tag chip: click selects every paper with that tag (AND-refine with more). */
  .chipsel { cursor: pointer; font-family: inherit; margin: 0; line-height: 1.45; }
  .chipsel:hover { filter: brightness(1.1); }
  .chipsel.on { box-shadow: 0 0 0 2px var(--accent); font-weight: 600; }
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

  /* floating selection pill: appears bottom-center while documents are selected */
  .floatpill {
    position: fixed; left: 50%; bottom: 22px; transform: translateX(-50%);
    z-index: 72; display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    max-width: min(92vw, 900px); justify-content: center;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(14px);
    border: 1px solid var(--border); border-radius: var(--r-pill);
    box-shadow: var(--shadow-lg); padding: 8px 14px;
    font-size: 13px; color: var(--accent);
    animation: pillin 0.18s cubic-bezier(0.2, 0.9, 0.3, 1.15);
  }
  @keyframes pillin { from { opacity: 0; transform: translateX(-50%) translateY(12px); } to { opacity: 1; transform: translateX(-50%); } }
  @media (prefers-reduced-motion: reduce) { .floatpill { animation: none; } }
  .floatpill .pillcount { font-weight: 700; font-variant-numeric: tabular-nums; margin-right: 2px; }
  .floatpill button, .floatpill select {
    background: transparent; color: var(--accent); border: 1px solid transparent;
    border-radius: var(--r-pill); padding: 5px 11px; font-size: 12px; cursor: pointer; outline: none;
    transition: background var(--ease), border-color var(--ease);
  }
  .floatpill button:hover, .floatpill select:hover { border-color: var(--accent-soft2); background: var(--accent-soft); }
  .floatpill button.del:hover { border-color: var(--danger); color: var(--danger); background: var(--danger-soft); }
  .floatpill .pillx { padding: 5px 9px; color: var(--dim); }

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
  .confirmmsg { margin: 0 0 18px; font-size: 14.5px; line-height: 1.5; color: var(--text); white-space: pre-line; }
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
  /* Citation explorer (snowball): two columns of neighbour papers. */
  .exwide { width: min(880px, 94vw); }
  .exgrid { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; }
  .exgrid .hfsec { margin-top: 12px; }
  .exgrid .ghsec { border-top: 1px solid var(--border); padding-top: 12px; margin-top: 12px; }
  .exlist { max-height: 50vh; }
  .exrow { display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border-soft); }
  .exmain { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .extitle { font-size: 12.5px; color: var(--text); line-height: 1.35; overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
  .exmeta { font-size: 11px; color: var(--faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .exacts { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  /* «+ PDF»: inline field to paste the direct PDF link found in the browser. */
  .hflink.on { font-weight: 600; text-decoration: underline; }
  .expdfrow { display: flex; align-items: center; gap: 6px; padding: 0 0 8px; }
  .expdfinput {
    flex: 1; min-width: 0; background: var(--field); border: 1px solid var(--border);
    color: var(--text); border-radius: 6px; padding: 4px 8px; font-size: 11.5px; outline: none;
  }
  .expdfinput:focus { border-color: var(--accent); }
  @media (max-width: 720px) { .exgrid { grid-template-columns: 1fr; } }
  /* Citation explorer dialog: fixed-height flex column so ONLY the two lists
     scroll (no nested dialog scrollbar); wider for two comfortable columns;
     closeable only via the ✕ (top-right), never by an outside click. */
  .exdialog {
    width: min(980px, 95vw);
    height: min(86vh, 800px);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
  }
  .exhead { display: flex; align-items: center; gap: 12px; flex-shrink: 0; padding-right: 30px; }
  .exhead h2 { margin: 0; }
  .exback { flex-shrink: 0; }
  .exdialog > .dimtext { flex-shrink: 0; }
  .exdialog .exgrid { flex: 1; min-height: 0; align-items: stretch; }
  .exdialog .exgrid .hfsec { display: flex; flex-direction: column; min-height: 0; }
  .exsechead { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  .exsechead h3 { margin: 0 0 8px; }
  /* The list fills its column and scrolls; discreet, hover-revealed scrollbar. */
  .exlist {
    flex: 1; min-height: 0; max-height: none; overflow-y: auto; padding-right: 5px;
    scrollbar-width: thin; scrollbar-color: transparent transparent;
  }
  .exlist:hover { scrollbar-color: var(--border) transparent; }
  .exlist::-webkit-scrollbar { width: 8px; }
  .exlist::-webkit-scrollbar-track { background: transparent; }
  .exlist::-webkit-scrollbar-thumb { background: transparent; border-radius: 8px; border: 2px solid transparent; background-clip: padding-box; }
  .exlist:hover::-webkit-scrollbar-thumb { background: var(--border); background-clip: padding-box; }
  .exlist::-webkit-scrollbar-thumb:hover { background: var(--faint); background-clip: padding-box; }

  /* ===== Scrollbar coerenti col tema, ovunque =====
     Pollice traslucido (derivato da --text) su binario trasparente: assume il
     colore del fondo su cui scorre — bg, surface o panel — così si "confonde"
     con lo sfondo del tema attivo in tutti gli 11 temi (chiari e scuri),
     firmandosi appena all'hover. Le barre già personalizzate (.taglist, .exlist)
     hanno selettori più specifici e restano com'erano. */
  :global(*) {
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--text) 15%, transparent) transparent;
  }
  :global(::-webkit-scrollbar) { width: 12px; height: 12px; }
  :global(::-webkit-scrollbar-track) { background: transparent; }
  :global(::-webkit-scrollbar-thumb) {
    background: color-mix(in srgb, var(--text) 15%, transparent);
    border-radius: 8px;
    border: 3px solid transparent;
    background-clip: padding-box;
  }
  :global(::-webkit-scrollbar-thumb:hover) {
    background: color-mix(in srgb, var(--text) 32%, transparent);
    background-clip: padding-box;
  }
  :global(::-webkit-scrollbar-corner) { background: transparent; }
  /* ✕ close button for dialogs that drop the bottom “Chiudi” button. */
  .modal-x {
    position: absolute; top: 10px; right: 12px; z-index: 2;
    width: 28px; height: 28px; border-radius: 7px;
    background: none; border: 1px solid transparent; color: var(--dim);
    font-size: 15px; line-height: 1; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
  }
  .modal-x:hover { background: var(--field); border-color: var(--border); color: var(--text); }
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
  .settitle { margin: 18px 0 6px; font-size: 13.5px; font-weight: 700; font-family: var(--serif); color: var(--text); }
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
  /* Compact tool bar — global-radial groups surfaced as always-visible icons */
  .toolbar {
    display: flex; align-items: center; gap: 4px;
    padding: 4px 18px; background: var(--surface);
    border-bottom: 1px solid var(--border-soft);
  }
  .toolbar .iconbtn { position: relative; width: 36px; height: 32px; }
  .toolbar .iconbtn:disabled { opacity: 0.4; cursor: default; }
  .toolbar .iconbtn:disabled:hover { background: transparent; color: inherit; border-color: transparent; }
  .toolbar .iconbtn.active { color: var(--accent); }
  .toolbar .iconbtn.active::after {
    content: ""; position: absolute; left: 8px; right: 8px; bottom: 1px; height: 2px;
    background: var(--accent); border-radius: 2px;
  }
  .toolbadge {
    position: absolute; top: 1px; right: 1px; min-width: 15px; height: 15px; padding: 0 3px;
    display: flex; align-items: center; justify-content: center;
    font-size: 9px; font-weight: 700; line-height: 1; color: var(--on-accent); background: var(--accent);
    border-radius: 8px; box-shadow: 0 0 0 1.5px var(--surface); pointer-events: none;
  }
  .toolmenu { width: 250px; }
  .menusec {
    font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.04em;
    color: var(--faint); margin: 8px 6px 3px; padding-top: 5px; border-top: 1px solid var(--border-soft);
  }
  .toolmenu .medit {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    border-bottom: none; margin-bottom: 1px; padding: 6px 8px;
  }
  .toolmenu .menusec:first-child { margin-top: 2px; padding-top: 0; border-top: none; }
  .medit.sub { padding-left: 16px; }
  .medit:disabled { color: var(--faint); cursor: default; opacity: 0.55; }
  .medit:disabled:hover { background: transparent; }
  .medit.danger { color: #c0392b; }
  .mtick { color: var(--accent); font-size: 12px; flex: 0 0 auto; }
  .mbadge {
    flex: 0 0 auto; font-size: 10px; color: var(--dim);
    background: var(--hover); border-radius: var(--r-pill); padding: 1px 6px; min-width: 14px; text-align: center;
  }
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
  /* tag/collection flyouts opened from the radial menu */
  .menu.flyout {
    width: 250px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(12px);
  }
  .mdone {
    display: block; width: 100%; margin-top: 8px;
    background: var(--accent-soft); color: var(--accent); border: 1px solid var(--accent-soft2);
    border-radius: var(--r-sm); padding: 6px; font-size: 12px; font-weight: 600; cursor: pointer;
  }
  .mdone:hover { background: var(--accent-soft2); }
  .mempty { color: var(--faint); font-size: 12px; margin: 4px; }

  /* Costellazione host: fills the visible main area (header + strip ≈ 105px) */
  .mapwrap { position: relative; height: calc(100vh - 108px); min-height: 340px; }

  /* "Riscopri" spotlight card */
  .spotcard {
    width: 560px; max-width: 94vw;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--r-lg); box-shadow: var(--shadow-lg); padding: 26px 28px;
  }
  .spotkicker {
    margin: 0 0 14px; font-size: 11px; font-weight: 700; letter-spacing: 2px;
    text-transform: uppercase; color: var(--accent);
  }
  .spotbody { display: flex; gap: 20px; align-items: flex-start; }
  .spotthumb {
    flex: 0 0 128px; width: 128px; aspect-ratio: 3 / 4; overflow: hidden;
    border-radius: var(--r-sm); border: 1px solid var(--border); background: var(--thumb-bg);
    box-shadow: var(--shadow-md);
  }
  .spotthumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .spotmeta { flex: 1; min-width: 0; }
  .spottitle { margin: 0 0 6px; font-size: 19px; line-height: 1.3; font-family: var(--serif); font-weight: 600; }
  .spotauthors { margin: 0 0 2px; font-size: 13px; color: var(--dim); }
  .spotvenue { margin: 0 0 10px; font-size: 12px; color: var(--faint); }
  .spotblurb {
    margin: 0; font-size: 12.5px; color: var(--dim); line-height: 1.55;
    max-height: 132px; overflow: hidden;
  }
  .spotactions { display: flex; gap: 8px; margin-top: 20px; justify-content: flex-end; }

  /* ===== Wiki della libreria ===== */
  .wikiwrap { display: flex; min-height: calc(100vh - 60px); }
  .wikinav {
    width: 260px; flex: 0 0 260px; border-right: 1px solid var(--border-soft);
    padding: 14px 12px; background: var(--bg);
    display: flex; flex-direction: column; gap: 8px;
  }
  .wikinew { display: flex; gap: 6px; }
  .wikinew input {
    flex: 1; min-width: 0; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: var(--r-sm); padding: 7px 10px; font-size: 12.5px; outline: none;
  }
  .wikinew input:focus { border-color: var(--accent); }
  .wikiall { width: 100%; }
  .notesort { display: flex; align-items: center; gap: 6px; margin-top: 4px; }
  .notesortlbl { font-size: 11px; color: var(--dim); text-transform: uppercase; letter-spacing: 0.03em; }
  .notesortsel {
    flex: 1; min-width: 0; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: var(--r-sm); padding: 5px 8px; font-size: 12px; outline: none; cursor: pointer;
  }
  .notesortsel:focus { border-color: var(--accent); }
  .wikiprog { display: flex; flex-direction: column; gap: 6px; padding: 8px; background: var(--panel); border-radius: var(--r-sm); }
  .wikiprog .bar { width: 100%; }
  .wikilist { flex: 1; overflow: auto; margin-top: 4px; }
  .wikistale { color: var(--accent); font-size: 9px; margin-left: 6px; }
  .wikiempty { font-size: 12px; color: var(--faint); line-height: 1.5; padding: 4px 6px; }
  .wikibody { flex: 1; min-width: 0; overflow: auto; padding: 26px 36px 60px; }
  .wikihead { display: flex; align-items: baseline; gap: 12px; border-bottom: 1px solid var(--border-soft); padding-bottom: 10px; margin-bottom: 6px; max-width: 780px; }
  .wikititle { margin: 0; font-family: var(--serif); font-size: 26px; font-weight: 600; flex: 1; }
  .wikimeta { font-size: 11.5px; color: var(--faint); white-space: nowrap; }
  .wikihtml { max-width: 780px; font-size: 14.5px; line-height: 1.7; color: var(--text); }
  .wikihtml :global(h2) { font-family: var(--serif); font-size: 18px; margin: 22px 0 8px; }
  .wikihtml :global(h3) { font-family: var(--serif); font-size: 15px; margin: 18px 0 6px; }
  .wikihtml :global(p) { margin: 8px 0; }
  .wikihtml :global(a) { color: var(--accent); text-decoration: none; border-bottom: 1px dotted var(--accent-soft2); }
  .wikihtml :global(a[href^="#src-"]) {
    font-size: 11px; vertical-align: 2px; border: none;
    background: var(--accent-soft); border-radius: 4px; padding: 0 3px; margin: 0 1px;
  }
  .wikihtml :global(ul) { padding-left: 22px; }
  .wikisources { max-width: 780px; margin-top: 26px; border-top: 1px solid var(--border-soft); padding-top: 12px; }
  .wikisources h3 { font-family: var(--serif); font-size: 14px; margin: 0 0 8px; color: var(--dim); }
  .wikisrc { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; padding: 3px 0; font-size: 13px; }
  .wikisrc.unused { opacity: 0.55; }
  .wikipages { display: inline-flex; gap: 4px; flex-wrap: wrap; }
  .wikiintro { max-width: 520px; margin: 0 auto; }

  /* ===== Note (.md vault) ===== */
  .noteitem { flex-direction: column; align-items: flex-start; gap: 1px; height: auto; padding: 6px 10px; }
  .notetitle { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .noteexc { font-size: 11px; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .notedates { font-size: 10px; color: var(--faint); opacity: 0.85; white-space: nowrap; max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  /* Title on its own line; the actions live in the row below (.noteactions). */
  .notehead { border-bottom: none; padding-bottom: 0; margin-bottom: 4px; }
  .notehead .notesaved { font-size: 11.5px; color: var(--faint); white-space: nowrap; }
  .notehead .notesaved.pending { color: var(--accent); }
  .noteactions {
    display: flex; align-items: center; gap: 10px; flex-wrap: wrap;
    max-width: 780px; border-bottom: 1px solid var(--border-soft);
    padding-bottom: 10px; margin-bottom: 6px;
  }
  .notemodes { display: inline-flex; gap: 4px; }
  .notemodes .ghost.on { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
  .noteexport { display: inline-flex; align-items: center; gap: 4px; margin-left: auto; }
  .noteexplbl { font-size: 11px; color: var(--faint); text-transform: uppercase; letter-spacing: 0.04em; }
  .noteedtoolbar {
    display: flex; flex-wrap: wrap; align-items: center; gap: 3px;
    max-width: 820px; margin: 14px 0 0; padding: 5px 6px;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--r-sm) var(--r-sm) 0 0; border-bottom: none;
  }
  .noteedtoolbar button {
    min-width: 28px; height: 28px; padding: 0 7px; display: inline-flex;
    align-items: center; justify-content: center; background: transparent;
    border: 1px solid transparent; border-radius: 6px; color: var(--text);
    font-size: 13px; cursor: pointer;
  }
  .noteedtoolbar button:hover { background: var(--accent-soft); border-color: var(--border); }
  .noteedtoolbar button code { font-family: var(--mono, monospace); font-size: 12px; }
  .edsep { width: 1px; align-self: stretch; margin: 3px 4px; background: var(--border-soft); }
  .noteedwrap { margin-top: 14px; }
  .noteedtoolbar + .noteedwrap { margin-top: 0; }
  .noteedtoolbar + .noteedwrap .noteeditor { margin-top: 0; border-top-left-radius: 0; border-top-right-radius: 0; }
  .noteeditor {
    display: block; width: 100%; max-width: 820px; box-sizing: border-box;
    min-height: 60vh; margin-top: 0; resize: vertical;
    background: var(--field); border: 1px solid var(--border); border-radius: var(--r-sm);
    color: var(--text); padding: 14px 16px; outline: none;
    font-family: var(--mono, ui-monospace, "Cascadia Code", Consolas, monospace);
    font-size: 13.5px; line-height: 1.65; tab-size: 2;
  }
  .noteeditor:focus { border-color: var(--accent); }
  /* Side-by-side ("Affiancato") mode: editor left, live preview right. */
  .noteedwrap.split {
    display: grid; grid-template-columns: 1fr 1fr; gap: 14px;
    max-width: 1280px; align-items: stretch;
  }
  .noteedwrap.split .noteeditor { max-width: none; height: 68vh; min-height: 0; resize: none; }
  /* `.notehtml { margin-top: 14px }` would otherwise win by source order and push the
     preview column 14px below the editor — pin it flush with a higher-specificity rule. */
  .noteedwrap.split .livepreview {
    margin-top: 0; overflow: auto; height: 68vh; box-sizing: border-box;
    background: var(--field); border: 1px solid var(--border); border-radius: var(--r-sm);
    padding: 14px 18px;
  }
  .noteedtoolbar + .noteedwrap.split .livepreview { border-top-left-radius: 0; border-top-right-radius: 0; }
  @media (max-width: 900px) {
    .noteedwrap.split { grid-template-columns: 1fr; }
    .noteedwrap.split .noteeditor, .livepreview { height: 44vh; }
  }
  .notehtml { margin-top: 14px; }
  /* KaTeX math (in-app) — display math on its own centered line; long formulas scroll. */
  :global(span.tex.block) { display: block; margin: 0.4em 0; }
  :global(span.tex.block .katex-display) { overflow-x: auto; overflow-y: hidden; padding: 2px 0; }
  /* Rendered math (MathML, exports) and the raw-LaTeX fallback in the note/wiki preview. */
  .notehtml :global(math[display="block"]), .wikihtml :global(math[display="block"]) {
    display: block; margin: 1em 0; overflow-x: auto; font-size: 1.08em;
  }
  .notehtml :global(math), .wikihtml :global(math) { font-size: 1.05em; }
  .notehtml :global(.mathraw), .wikihtml :global(.mathraw) {
    background: var(--accent-soft); color: var(--accent); padding: 0 4px;
    border-radius: 4px; font-family: var(--mono, monospace); font-size: 0.9em;
  }
  .notebacklinks { max-width: 780px; margin-top: 28px; border-top: 1px solid var(--border-soft); padding-top: 12px; }
  .notebacklinks h3 { font-family: var(--serif); font-size: 14px; margin: 0 0 8px; color: var(--dim); }
  .notetitleh { cursor: text; }
  .noterename {
    flex: 1; min-width: 0; font-family: var(--serif); font-size: 24px; font-weight: 600;
    background: var(--field); border: 1px solid var(--accent); border-radius: var(--r-sm);
    color: var(--text); padding: 2px 8px; outline: none;
  }
  .noteinfo { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; max-width: 820px; margin: 4px 0 2px; font-size: 11.5px; color: var(--faint); }
  .noteinfopath {
    max-width: 62ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    background: none; border: none; color: var(--faint); cursor: pointer; padding: 0; font: inherit;
  }
  .noteinfopath:hover { color: var(--accent); text-decoration: underline; }
  .noteinfodate { white-space: nowrap; }
  /* "Note" group shown above the grid when a search matches standalone notes */
  .noteresults { max-width: 1100px; margin: 4px 0 18px; }
  .notehits { display: flex; flex-direction: column; gap: 6px; margin-top: 8px; }
  .notehit {
    display: flex; flex-direction: column; gap: 2px; text-align: left; width: 100%;
    background: var(--surface); border: 1px solid var(--border-soft); border-radius: var(--r-sm);
    padding: 8px 12px; cursor: pointer; transition: border-color var(--ease), background var(--ease);
  }
  .notehit:hover { border-color: var(--accent); background: var(--hover); }
  .nhtitle { font-family: var(--serif); font-size: 14px; color: var(--text); }
  .nhsnip { font-size: 12px; color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .refdoibar { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin: 6px 0 12px; }
  .refdoibar .dimtext { margin: 0; }
  .wikiintro code { background: var(--accent-soft); border-radius: 4px; padding: 1px 5px; font-size: 12.5px; }

  /* ===== Esplora citazioni: mappa a due ali + indicatore in-libreria ===== */
  .libdot {
    display: inline-block; width: 9px; height: 9px; border-radius: 50%;
    border: 1.4px dashed var(--dim); margin-right: 6px; vertical-align: -1px; flex: 0 0 auto;
  }
  .libdot.in { background: var(--accent); border: 1.4px solid var(--accent); }
  .exbar { display: flex; align-items: center; gap: 14px; margin: 6px 0 10px; flex-wrap: wrap; }
  .exbar .segbtn { width: auto; padding: 0 14px; font-size: 12px; }
  .exlegend { font-size: 11px; color: var(--faint); display: inline-flex; align-items: center; gap: 6px; }
  /* Sopra il modale «Esplora citazioni» (z 80): il popup nasceva DIETRO la finestra. */
  .menu.mappop { width: 300px; z-index: 86; }
  .mapmeta { font-size: 11px; color: var(--faint); margin: 0 4px 6px; }

  /* ===== 0.5.1: tag editor, home leggera, coach mark, care tabs ===== */
  button.primary.small { margin-left: 0; padding: 5px 12px; font-size: 12px; }

  /* tag: matitina + popover di modifica */
  .x.edit { font-size: 12px; }
  .menu.tagedit { width: 210px; padding: 10px; display: flex; flex-direction: column; gap: 8px; }
  .teinput {
    width: 100%; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: var(--r-sm); padding: 6px 9px; font-size: 13px; outline: none;
  }
  .teinput:focus { border-color: var(--accent); }
  .teswatches { display: flex; flex-wrap: wrap; gap: 6px; }
  .teswatch {
    width: 20px; height: 20px; border-radius: 50%; border: 2px solid transparent; cursor: pointer; padding: 0;
  }
  .teswatch.on { border-color: var(--text); box-shadow: 0 0 0 2px var(--surface) inset; }
  .teact { display: flex; justify-content: flex-end; gap: 8px; }

  /* care: schede a tutta larghezza nel modal */
  .caretabs { margin: 4px 0 14px; }
  .segbtn.wide { width: auto; padding: 0 16px; height: 30px; font-size: 12.5px; font-weight: 600; }
  .dupwrap.inmodal { padding: 0; max-height: 52vh; overflow-y: auto; }

  /* home leggera (vista «Tutti») */
  .home {
    padding: 14px 22px 2px;
    display: flex; align-items: center; justify-content: space-between;
    gap: 12px 28px; flex-wrap: wrap;
  }
  .homehead { display: flex; align-items: center; gap: 18px; flex-wrap: wrap; }
  .homefold {
    background: none; border: none; color: var(--dim); cursor: pointer; padding: 0;
    font-size: 12px; font-weight: 700; letter-spacing: 0.5px; text-transform: uppercase;
    display: inline-flex; align-items: center; gap: 6px;
  }
  .homefold:hover { color: var(--accent); }
  .homechev { font-size: 10px; }
  .homestats { display: flex; align-items: stretch; gap: 10px; }
  .hstat {
    display: inline-flex; flex-direction: column; align-items: flex-start; gap: 1px;
    background: var(--panel); border: 1px solid var(--border-soft); border-radius: var(--r-md);
    padding: 5px 12px; text-align: left;
  }
  button.hstat { cursor: pointer; transition: border-color var(--ease), background var(--ease); }
  button.hstat:hover { border-color: var(--accent); background: var(--accent-soft); }
  .hnum { font-size: 17px; font-weight: 700; color: var(--text); font-family: var(--serif); line-height: 1.1; }
  .hlab { font-size: 10.5px; color: var(--faint); }
  .rediscover { display: flex; align-items: center; gap: 12px; }
  .rdlabel {
    display: flex; align-items: center; gap: 6px; flex: 0 0 auto;
    font-size: 11px; font-weight: 700; letter-spacing: 0.5px; text-transform: uppercase; color: var(--faint);
  }
  .rdshuffle {
    background: none; border: 1px solid var(--border); color: var(--dim);
    border-radius: 50%; width: 22px; height: 22px; cursor: pointer; font-size: 12px; line-height: 1;
  }
  .rdshuffle:hover { color: var(--accent); border-color: var(--accent); }
  .rdcard {
    display: flex; align-items: center; gap: 10px; cursor: pointer;
    background: var(--panel); border: 1px solid var(--border-soft); border-radius: var(--r-md);
    padding: 6px 12px 6px 6px; max-width: 420px; transition: border-color var(--ease), transform var(--ease);
  }
  .rdcard:hover { border-color: var(--accent); transform: translateY(-1px); }
  .rdthumb {
    width: 34px; height: 46px; flex: 0 0 auto; border-radius: 3px; overflow: hidden;
    background: var(--thumb-bg); display: flex; align-items: center; justify-content: center;
  }
  .rdthumb img { width: 100%; height: 100%; object-fit: cover; object-position: top; }
  .rdmeta { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .rdtitle {
    font-size: 13px; font-weight: 600; color: var(--text); line-height: 1.3;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .rdsub { font-size: 11px; color: var(--faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* coach mark una-tantum */
  .coach {
    position: fixed; left: 50%; bottom: 26px; transform: translateX(-50%); z-index: 80;
    width: min(440px, calc(100vw - 40px));
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-lg);
    box-shadow: var(--shadow-lg); padding: 16px 18px;
    animation: coachup 0.24s cubic-bezier(0.2, 0.9, 0.3, 1);
  }
  @keyframes coachup { from { opacity: 0; transform: translate(-50%, 12px); } }
  @media (prefers-reduced-motion: reduce) { .coach { animation: none; } }
  .coachh { font-family: var(--serif); font-size: 16px; font-weight: 600; color: var(--text); margin-bottom: 4px; }
  .coachp { font-size: 12.5px; color: var(--dim); margin: 0 0 8px; }
  .coachlist { margin: 0 0 12px; padding-left: 18px; display: flex; flex-direction: column; gap: 5px; }
  .coachlist li { font-size: 12.5px; color: var(--text); line-height: 1.4; }
  .coachact { display: flex; justify-content: flex-end; gap: 8px; }

  /* keyboard/click focus cursor on the library (detail panel target) */
  .card { user-select: none; } /* il doppio click apre il lettore, non seleziona testo */
  .card.kfocus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--ring), var(--shadow-sm); }
  .list tbody tr.kfocus { outline: 2px solid var(--accent); outline-offset: -2px; }

  /* AI-summary indicator: this document already has a cached summary */
  .aisum {
    display: inline-block; margin-left: 6px; padding: 0 6px;
    font-size: 10px; font-weight: 700; letter-spacing: 0.3px; line-height: 15px;
    color: var(--accent); background: var(--accent-soft);
    border: 1px solid var(--accent-soft2); border-radius: var(--r-pill);
    vertical-align: 1px; cursor: help;
  }
  .aisum.inline { margin-left: 5px; padding: 0 4px; }

  /* ===== Sintesi sulla selezione + percorso di lettura ===== */
  .aidocmodal { width: 760px; }
  .aidochtml { max-height: 52vh; overflow: auto; padding-right: 6px; }
  .aidochtml :global(table) { border-collapse: collapse; font-size: 12.5px; margin: 8px 0; }
  .aidochtml :global(th), .aidochtml :global(td) { border: 1px solid var(--border); padding: 5px 9px; text-align: left; vertical-align: top; }
  .aidochtml :global(th) { background: var(--panel); font-weight: 600; }
  .aidocsrc { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; margin-top: 10px; border-top: 1px solid var(--border-soft); padding-top: 8px; }
  .gridmodal { width: 780px; }
  .gridwrap { max-height: 54vh; overflow: auto; border: 1px solid var(--border-soft); border-radius: var(--r-sm); }
  .resgrid { width: 100%; border-collapse: collapse; font-size: 12.5px; }
  .resgrid th, .resgrid td { border-bottom: 1px solid var(--border-soft); padding: 6px 10px; text-align: left; }
  .resgrid th { position: sticky; top: 0; background: var(--panel); font-weight: 600; }
  .resgrid tbody tr:nth-child(odd) { background: var(--zebra); }
  .pathmodal { width: 640px; }
  .pathlist { margin: 12px 0 0; padding-left: 22px; display: flex; flex-direction: column; gap: 10px; }
  .pathstep.ext { opacity: 0.9; }
  .pathmain { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; }
  .pathtitle { font-size: 13px; color: var(--text); }
  .pathwhy { display: block; font-size: 11.5px; color: var(--faint); margin-top: 1px; }

  /* "Riferimento senza PDF" panel */
  .refmodal { width: 560px; }
  .refactions { display: flex; gap: 8px; flex-wrap: wrap; margin: 14px 0 4px; }
  .refor { font-size: 12px; color: var(--faint); margin: 14px 0 6px; }
  .refurl { display: flex; gap: 6px; align-items: stretch; }
  .refurl input {
    flex: 1; min-width: 0; background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: var(--r-sm); padding: 8px 10px; font-size: 13px; outline: none;
  }
  .refurl input:focus { border-color: var(--accent); }
  .refhint { font-size: 11.5px; color: var(--faint); line-height: 1.45; margin: 10px 0 0; }
</style>
