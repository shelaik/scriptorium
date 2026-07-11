// Typed wrappers around the Rust backend commands.
import { invoke } from "@tauri-apps/api/core";

export interface Tag {
  id: number;
  name: string;
  color: string | null;
  /** How many (non-deleted) documents carry this tag (0 for per-document tag lists). */
  count: number;
}

export interface Collection {
  id: number;
  name: string;
  is_smart: boolean;
  rule_json: string | null;
}

export interface DocumentItem {
  id: number;
  title: string | null;
  year: number | null;
  venue: string | null;
  doi: string | null;
  authors: string[];
  tags: Tag[];
  has_thumb: boolean;
  /** False for reference-only entries (no PDF attached yet). */
  has_file: boolean;
  /** True when an AI summary is already stored (batch AI skips these). */
  has_summary: boolean;
  added_at: string | null;
  is_read: boolean;
  favorite: boolean;
  github_url: string | null;
  pub_status: string | null;
  paper_url: string | null;
  /** Persistent, library-unique citation key (firstauthor+year+word). */
  citekey: string | null;
  /** Last viewed page (1-based) and total pages, for the reading-progress bar. */
  last_page: number | null;
  page_count: number | null;
  /** The user's own work (imported from a LaTeX project .zip). */
  is_own: boolean;
}

export interface ImportSummary {
  imported: number[];
  duplicates: number[];
  errors: string[];
  warnings: string[];
}

/** Import PDFs by absolute path; returns what was imported / skipped / failed. */
export const importFiles = (paths: string[]) =>
  invoke<ImportSummary>("import_files", { paths });

/** Most recently opened documents (for the "Continue reading" shelf). */
export const recentDocuments = (limit: number) =>
  invoke<DocumentItem[]>("recent_documents", { limit });
/** All documents by a given author (exact, case-insensitive). */
export const documentsByAuthor = (name: string) =>
  invoke<DocumentItem[]>("documents_by_author", { name });

/** Import a BibTeX (.bib) file as reference-only items. Returns add/skip/error counts. */
export const importBibtex = (path: string) =>
  invoke<{ added: number; skipped: number; errors: string[] }>("import_bibtex", { path });

export interface LatexImportSummary {
  imported: number;
  duplicates: number;
  pdfs_found: number;
  bib_entries: number;
  references_linked: number;
  refs_without_doi: number;
  dois_resolved: number;
  errors: string[];
}
/** Import a LaTeX project .zip: add its compiled PDF(s) as own work and link the
 *  .bib bibliography as the paper's citation graph. If online discovery is on,
 *  recovers missing DOIs (title → Crossref) so the gap-finder can see them. */
export const importLatexZip = (path: string) =>
  invoke<LatexImportSummary>("import_latex_zip", { path });
/** Try to attach an Open-Access PDF to a reference-only doc. "attached"|"already"|"not_found". */
export const findPdf = (id: number) => invoke<string>("find_pdf", { id });
/** Attach the PDF at `url` to an EXISTING reference-only doc (no new entry).
 *  "attached" | "already" | "duplicate" | "not_pdf". GitHub blob links are normalized. */
export const attachFromUrl = (id: number, url: string) =>
  invoke<string>("attach_from_url", { id, url });

export interface HfItem { id: string; likes: number; downloads: number; url: string }
export interface HfResources {
  arxiv_id: string | null;
  paper_url: string | null;
  models: HfItem[];
  datasets: HfItem[];
}
/** Hugging Face models & datasets that cite this document's paper (by arXiv id). */
export const hfResources = (id: number) => invoke<HfResources>("hf_resources", { id });

export interface GhRepo {
  owner: string;
  repo: string;
  full_name: string;
  description: string | null;
  stars: number;
  language: string | null;
  license: string | null;
  url: string;
  pushed_at: string | null;
}
/** GitHub repositories referenced in a document's text, with live metadata. */
export const githubRepos = (id: number) => invoke<GhRepo[]>("github_repos", { id });
/** A repo's README rendered to sanitized HTML. */
export const githubReadme = (owner: string, repo: string) =>
  invoke<string>("github_readme", { owner, repo });

// ----- Table extraction from a selected PDF region -----
/** Reconstruct a table grid from a normalized region of a page (1-based). */
export const extractTable = (
  id: number,
  page: number,
  rect: { x: number; y: number; w: number; h: number },
) => invoke<string[][]>("extract_table", { id, page, x: rect.x, y: rect.y, w: rect.w, h: rect.h });
/** Write a grid to a file as "csv" | "md" | "xlsx". */
export const exportTable = (grid: string[][], format: string, path: string) =>
  invoke<void>("export_table", { grid, format, path });
/** Refine a roughly-extracted grid with the local AI. */
export const aiCleanTable = (grid: string[][]) => invoke<string[][]>("ai_clean_table", { grid });
/** Extract the plain text of a normalized region of a page (1-based). */
export const extractRegionText = (
  id: number,
  page: number,
  rect: { x: number; y: number; w: number; h: number },
) => invoke<string>("extract_region_text", { id, page, x: rect.x, y: rect.y, w: rect.w, h: rect.h });
/** Recognize a cropped formula image (base64 PNG) as LaTeX, locally.
 *  `multi` segments a multi-line selection into separate equations. */
export const formulaToLatex = (imageBase64: string, multi = false) =>
  invoke<string>("formula_to_latex", { imageBase64, multi });
/** Whether the formula→LaTeX models are ready, and MB to download if not. */
export const mathocrStatus = () =>
  invoke<{ ready: boolean; downloadMb: number }>("mathocr_status");
/** Recognize a cropped formula image as LaTeX via a local vision LLM (Ollama/LM Studio). */
export const formulaToLatexAi = (imageBase64: string, model: string, multi = false) =>
  invoke<string>("formula_to_latex_ai", { imageBase64, model, multi });
/** Extract a cropped table image into a grid via a local vision LLM. */
export const tableFromImageAi = (imageBase64: string, model: string) =>
  invoke<string[][]>("table_from_image_ai", { imageBase64, model });
/** Extract a table with the local STRUCTURE model (TATR): rows/columns/spanning
 *  cells from the crop image, cell text byte-exact from the PDF's own words. */
export const extractTableModel = (
  imageBase64: string,
  id: number,
  page: number,
  rect: { x: number; y: number; w: number; h: number },
) =>
  invoke<string[][]>("extract_table_model", {
    imageBase64,
    id,
    page,
    x: rect.x,
    y: rect.y,
    w: rect.w,
    h: rect.h,
  });
/** Whether the table-structure model is ready, and MB to download if not. */
export const tablestructStatus = () =>
  invoke<{ ready: boolean; downloadMb: number }>("tablestruct_status");
/** OCR a cropped text region via a local vision LLM (for scanned pages). */
export const textFromImageAi = (imageBase64: string, model: string) =>
  invoke<string>("text_from_image_ai", { imageBase64, model });
/** Unload the active provider's models from VRAM; returns how many were freed (Ollama). */
export const aiUnloadModels = () => invoke<number>("ai_unload_models");
/** Write arbitrary text to a file. */
export const writeTextFile = (path: string, content: string) =>
  invoke<void>("write_text_file", { path, content });
/** Write raw bytes (from a base64/data-URL string) to a file — used to save a PNG. */
export const writeBinaryFile = (path: string, base64: string) =>
  invoke<void>("write_binary_file", { path, base64 });
/** Store a pasted image (base64 data-URL) in the notes vault's assets/; returns the short `assets/…` ref. */
export const saveNoteAsset = (base64: string) => invoke<string>("save_note_asset", { base64 });
/** Copy an image file from disk into the notes vault's assets/ (OS drag&drop); returns the `assets/…` ref. */
export const importNoteAsset = (path: string) => invoke<string>("import_note_asset", { path });

/** List documents, newest first, optionally filtered by tag, collection or flag. */
export const listDocuments = (filter?: {
  tagId?: number;
  collectionId?: number;
  flag?: "favorite" | "unread" | "github" | "peerreviewed" | "mywork";
}) =>
  invoke<DocumentItem[]>("list_documents", {
    tagId: filter?.tagId ?? null,
    collectionId: filter?.collectionId ?? null,
    flag: filter?.flag ?? null,
  });

// ----- Read / favorite / last page / backup -----
export const setRead = (id: number, value: boolean) =>
  invoke<void>("set_read", { id, value });
export const setFavorite = (id: number, value: boolean) =>
  invoke<void>("set_favorite", { id, value });
export const setLastPage = (id: number, page: number, pages?: number) =>
  invoke<void>("set_last_page", { id, page, pages: pages ?? null });
export const getLastPage = (id: number) => invoke<number | null>("get_last_page", { id });
/** Copy the whole library data folder into `dest`; returns the backup path. */
export const backupLibrary = (dest: string) => invoke<string>("backup_library", { dest });

// ----- Tags -----
export const listTags = () => invoke<Tag[]>("list_tags");
export const createTag = (name: string, color: string | null) =>
  invoke<Tag>("create_tag", { name, color });
/** Rename and/or recolor a tag (errors if the name is taken by another tag). */
export const updateTag = (id: number, name: string, color: string | null) =>
  invoke<void>("update_tag", { id, name, color });
export const deleteTag = (id: number) => invoke<void>("delete_tag", { id });
export const setDocumentTags = (documentId: number, tagIds: number[]) =>
  invoke<void>("set_document_tags", { documentId, tagIds });

// ----- Collections -----
export const listCollections = () => invoke<Collection[]>("list_collections");
export const createCollection = (
  name: string,
  isSmart: boolean,
  ruleJson: string | null,
) => invoke<Collection>("create_collection", { name, isSmart, ruleJson });
export const deleteCollection = (id: number) => invoke<void>("delete_collection", { id });
export const addToCollection = (collectionId: number, documentId: number) =>
  invoke<void>("add_to_collection", { collectionId, documentId });
export const removeFromCollection = (collectionId: number, documentId: number) =>
  invoke<void>("remove_from_collection", { collectionId, documentId });

// ----- Editable metadata -----
export interface EditableMeta {
  title: string | null;
  authors: string[];
  year: number | null;
  venue: string | null;
  doi: string | null;
  abstract_text: string | null;
  notes: string | null;
  summary: string | null;
}
export const getDocumentMeta = (id: number) =>
  invoke<EditableMeta>("get_document_meta", { id });
export const updateDocumentMetadata = (id: number, m: EditableMeta) =>
  invoke<void>("update_document_metadata", {
    id,
    title: m.title,
    authors: m.authors,
    year: m.year,
    venue: m.venue,
    doi: m.doi,
    abstractText: m.abstract_text,
    notes: m.notes,
  });

// ----- Citation links (references + cited-by) -----
export interface RefItem {
  raw: string | null;
  ref_doi: string | null;
  in_library: number | null;
  title: string | null;
}
export interface DocBrief {
  id: number;
  title: string | null;
  year: number | null;
}
export interface CitationLinks {
  references: RefItem[];
  cited_by: DocBrief[];
}
export const citationLinks = (id: number) => invoke<CitationLinks>("citation_links", { id });

// ----- Saved searches -----
export interface SavedSearch {
  id: number;
  name: string;
  source: string;
  query: string;
  author: string | null;
  year_from: number | null;
  year_to: number | null;
  oa_only: boolean;
  sort: string;
  last_run_at: string | null;
  auto_run: boolean;
}
export const listSavedSearches = () => invoke<SavedSearch[]>("list_saved_searches");
export const deleteSavedSearch = (id: number) => invoke<void>("delete_saved_search", { id });
export const setWatchAutoRun = (id: number, autoRun: boolean) =>
  invoke<void>("set_watch_auto_run", { id, autoRun });
export const createSavedSearch = (s: {
  name: string;
  source: string;
  query: string;
  author: string | null;
  yearFrom: number | null;
  yearTo: number | null;
  oaOnly: boolean;
  sort: string;
  seenIds: string[];
}) =>
  invoke<SavedSearch>("create_saved_search", {
    name: s.name,
    source: s.source,
    query: s.query,
    author: s.author,
    yearFrom: s.yearFrom,
    yearTo: s.yearTo,
    oaOnly: s.oaOnly,
    sort: s.sort,
    seenIds: s.seenIds,
  });
export const runSavedSearch = (id: number) =>
  invoke<{ name: string; results: SearchResult[]; new_ids: string[] }>("run_saved_search", { id });

// ----- Online discovery -----
export interface SearchResult {
  source: string;
  external_id: string;
  doi: string | null;
  title: string | null;
  authors: string[];
  year: number | null;
  venue: string | null;
  abstract_text: string | null;
  oa_pdf_url: string | null;
  url: string | null;
  is_oa: boolean;
  citations: number;
  in_library: boolean;
  github_url: string | null;
  pub_status: string | null;
}
export interface DiscoverySettings {
  enabled: boolean;
  email: string;
  has_openalex_key: boolean;
  has_ads_token: boolean;
  has_s2_key: boolean;
  has_core_key: boolean;
  has_github_token: boolean;
}
export const getDiscoverySettings = () => invoke<DiscoverySettings>("get_discovery_settings");
export const setDiscoverySettings = (enabled: boolean, email: string) =>
  invoke<void>("set_discovery_settings", { enabled, email });
/** Store (or, if empty, clear) one API key in the OS credential vault. */
export const setApiKey = (name: string, value: string) =>
  invoke<void>("set_api_key", { name, value });
export const discoverSearch = (
  query: string,
  source: string,
  opts: {
    author: string | null;
    yearFrom: number | null;
    yearTo: number | null;
    oaOnly: boolean;
    sort: string;
  },
) =>
  invoke<SearchResult[]>("discover_search", {
    query,
    source,
    author: opts.author,
    yearFrom: opts.yearFrom,
    yearTo: opts.yearTo,
    oaOnly: opts.oaOnly,
    sort: opts.sort,
  });
export const discoverAdd = (result: SearchResult) =>
  invoke<string>("discover_add", { result });

// ----- Novità (saved-search monitoring feed) -----
export interface NovitaHit {
  hit_id: number;
  found_at: string | null;
  result: SearchResult;
}
export interface NovitaGroup {
  watch_id: number;
  watch_name: string;
  hits: NovitaHit[];
}
export const novitaCount = () => invoke<number>("novita_count");
export const listNovita = () => invoke<NovitaGroup[]>("list_novita");
export const dismissHit = (hitId: number) => invoke<void>("dismiss_hit", { hitId });
export const dismissWatchHits = (watchId: number) => invoke<void>("dismiss_watch_hits", { watchId });
export const acceptHit = (hitId: number) => invoke<string>("accept_hit", { hitId });
/** Run the auto-run watches now (ignores the debounce); returns fresh-hit count. */
export const sweepWatchesNow = () => invoke<number>("sweep_watches_now");

// ----- "Aggancia da URL" + browser connector -----
/** Download a PDF from a URL and import it. Returns "added"|"duplicate"|"not_pdf". */
export const addFromUrl = (url: string) => invoke<string>("add_from_url", { url });

export interface ConnectorInfo {
  enabled: boolean;
  running: boolean;
  port: number;
  token: string;
}
/** Current state of the loopback browser connector + port/token for the bookmarklet. */
export const getConnectorInfo = () => invoke<ConnectorInfo>("get_connector_info");
/** Enable/disable the loopback connector (starts/stops it live). */
export const setConnectorEnabled = (enabled: boolean) =>
  invoke<ConnectorInfo>("set_connector_enabled", { enabled });

// ----- Snowball / citation explorer -----
export interface CitationNeighbors {
  references: SearchResult[];
  citations: SearchResult[];
  seed_unresolved: boolean;
}
/** OpenAlex references (cited by this paper) + citing papers, for a DOI. */
export const exploreCitations = (doi: string) =>
  invoke<CitationNeighbors>("explore_citations", { doi });

// ----- AI (Ollama / LM Studio) — optional -----
export type AiProvider = "ollama" | "lmstudio";
export interface AiSettings {
  enabled: boolean;
  provider: AiProvider;
  ollama_url: string;
  lmstudio_url: string;
  model: string;
  embed_gpu: boolean;
  embed_batch: number;
}
export const getAiSettings = () => invoke<AiSettings>("get_ai_settings");
export const setAiSettings = (s: AiSettings) =>
  invoke<void>("set_ai_settings", {
    enabled: s.enabled,
    provider: s.provider,
    ollamaUrl: s.ollama_url,
    lmstudioUrl: s.lmstudio_url,
    model: s.model,
    embedGpu: s.embed_gpu,
    embedBatch: s.embed_batch,
  });
/** List the models a provider serves at the given URL (also a reachability check). */
export const aiListModels = (provider: AiProvider, url: string) =>
  invoke<string[]>("ai_list_models", { provider, url });

export interface AiStatus {
  enabled: boolean;
  provider: AiProvider;
  model: string;
  reachable: boolean;
  model_available: boolean;
  detail: string;
}
/** Live status of the configured AI provider — drives the header AI indicator. */
export const aiStatus = () => invoke<AiStatus>("ai_status");
/** Start the local server for a provider (Ollama `serve` / LM Studio `lms server start`). */
export const aiServerStart = (provider: AiProvider) =>
  invoke<void>("ai_server_start", { provider });
/** Stop the local server for a provider. */
export const aiServerStop = (provider: AiProvider) =>
  invoke<void>("ai_server_stop", { provider });
/** Generate + cache an Italian summary for a document. */
export const summarizeDocument = (id: number) => invoke<string>("summarize_document", { id });
/** Suggest + assign 3-6 topical tags for a document. */
export const autotagDocument = (id: number) => invoke<string[]>("autotag_document", { id });

// ----- Add by identifier -----
export interface AddSummary {
  added: number;
  skipped: number;
  errors: string[];
}
/** Add reference-only items from pasted DOI/arXiv/ISBN/PMID identifiers. */
export const addByIdentifiers = (identifiers: string[]) =>
  invoke<AddSummary>("add_by_identifiers", { identifiers });

// ----- Trash / hygiene -----
export const deleteDocuments = (ids: number[]) => invoke<void>("delete_documents", { ids });
export const restoreDocuments = (ids: number[]) => invoke<void>("restore_documents", { ids });
export const purgeDocuments = (ids: number[]) => invoke<void>("purge_documents", { ids });
export const listTrash = () => invoke<DocumentItem[]>("list_trash");
export const addDocumentTag = (documentId: number, tagId: number) =>
  invoke<void>("add_document_tag", { documentId, tagId });
export const findDuplicates = () => invoke<number[][]>("find_duplicates");
export const mergeDocuments = (masterId: number, otherIds: number[]) =>
  invoke<void>("merge_documents", { masterId, otherIds });

// ----- Watched folder -----
export const getWatchedFolder = () => invoke<string | null>("get_watched_folder");
export const setWatchedFolder = (path: string | null) =>
  invoke<void>("set_watched_folder", { path });

/** Fetch a document's thumbnail as a PNG data URL, or null. */
export const getThumbnail = (id: number) =>
  invoke<string | null>("get_thumbnail", { id });

/** Re-render all cover thumbnails at the current (high) resolution. Returns the count. */
export const rebuildThumbnails = () =>
  invoke<number>("rebuild_thumbnails");

export interface EnrichSummary {
  updated: number;
  /** No identity could be resolved online (not indexed, or only cited DOIs present). */
  no_doi: number;
  /** Had a DOI in the text but it (a cited work) didn't match this paper, and no
   *  title search matched either. */
  skipped_mismatch: number;
  errors: string[];
}

/** Resolve metadata for documents lacking it, by DOI or by title (Crossref/arXiv). */
export const enrichAll = () => invoke<EnrichSummary>("enrich_all");

export interface RepairSummary {
  checked: number;
  repaired_arxiv: number;
  /** Full record re-resolved online from the recovered title. */
  resolved_online: number;
  retitled: number;
  cleared: number;
  details: string[];
}

/** Re-verify enriched documents and fix any whose title doesn't match the PDF. */
export const repairMetadata = () => invoke<RepairSummary>("repair_metadata");

export type SearchMode = "fulltext" | "semantic" | "hybrid";

/** Search the library by full-text, semantic similarity, or both (RRF). */
export const searchDocuments = (query: string, mode: SearchMode) =>
  invoke<DocumentItem[]>("search", { query, mode });

/** A standalone note that matched a full-text search. */
export interface NoteHit {
  slug: string;
  title: string;
  snippet: string;
}
/** Full-text search over the standalone Markdown notes vault. */
export const searchNotes = (query: string) =>
  invoke<NoteHit[]>("search_notes", { query });

/** Documents semantically similar to the given one (by embedding). */
export const relatedDocuments = (id: number) =>
  invoke<DocumentItem[]>("related_documents", { id });

// ----- Citations / export -----
/** Citation text. format: bibtex | ris | csljson | apa | ieee | citekey | latex | pandoc. */
export const citeText = (ids: number[], format: string) =>
  invoke<string>("cite_text", { ids, format });
/** Write citations for the given documents to a file. */
export const exportCitations = (ids: number[], format: string, path: string) =>
  invoke<void>("export_citations", { ids, format, path });

// ----- Library health (maintenance scan) -----
export interface HealthRow {
  id: number;
  title: string | null;
  path: string;
}
export interface DupGroup {
  file_hash: string;
  ids: number[];
  titles: string[];
}
export interface LibraryHealth {
  total: number;
  missing_file: HealthRow[];
  no_text: HealthRow[];
  no_metadata: HealthRow[];
  no_embedding: HealthRow[];
  no_thumbnail: HealthRow[];
  duplicates: DupGroup[];
}
/** Read-only scan of the library for rot signals (missing files, no text, dups, …). */
export const libraryHealth = () => invoke<LibraryHealth>("library_health");

// ----- OCR fallback for scanned PDFs (Windows OCR engine) -----
export interface OcrSummary {
  pages: number;
  total_pages: number;
  chars: number;
  truncated: boolean;
}
/** OCR a scanned PDF and store the recognised text as its fulltext. */
export const ocrDocument = (id: number) => invoke<OcrSummary>("ocr_document", { id });

// ----- Sidebar facet counts -----
export interface LibraryFacets {
  all: number;
  favorite: number;
  unread: number;
  github: number;
  peerreviewed: number;
  own: number;
}
/** Per-filter document counts over the whole library, for the sidebar badges. */
export const libraryFacets = () => invoke<LibraryFacets>("library_facets");

// ----- Citation gap-finder -----
export interface GapItem {
  doi: string;
  count: number;
  sample: string | null;
}
/** DOIs the library cites most but doesn't own, ranked by citation count (offline). */
export const citationGaps = (limit?: number) =>
  invoke<GapItem[]>("citation_gaps", { limit: limit ?? null });

export interface BackfillDoiSummary {
  scanned: number;
  resolved: number;
  updated_rows: number;
  remaining: number;
}
/** Recover DOIs for already-imported references that lack one (online, precision-gated). */
export const resolveReferenceDois = () =>
  invoke<BackfillDoiSummary>("resolve_reference_dois");
/** Request cancellation of an in-progress reference-DOI backfill. */
export const cancelReferenceDois = () => invoke<void>("cancel_reference_dois");

// ----- Obsidian / Markdown vault export -----
export const getObsidianVault = () => invoke<string>("get_obsidian_vault");
export const setObsidianVault = (path: string) =>
  invoke<void>("set_obsidian_vault", { path });
/** Export the given documents as Markdown notes into <vault>/Scriptorium/. Returns the count. */
export const exportToObsidian = (ids: number[], vaultDir: string) =>
  invoke<number>("export_obsidian", { ids, vaultDir });

export interface EmbedStatus {
  total: number;
  embedded: number;
}
export interface EmbedSummary {
  embedded: number;
  errors: string[];
}

/** How many documents already have a semantic embedding. */
export const embeddingStatus = () => invoke<EmbedStatus>("embedding_status");

/** Build embeddings for documents missing them (downloads model on first run). */
export const generateEmbeddings = () => invoke<EmbedSummary>("generate_embeddings");

/** Request cancellation of an in-progress embedding job. */
export const cancelEmbeddings = () => invoke<void>("cancel_embeddings");

export interface EmbedProgress {
  done: number;
  total: number;
  phase: "model" | "running" | "done" | "cancelled";
}

// ----- RAG engine ("Chiedi alla libreria") -----
export interface RagStatus {
  indexed_docs: number;
  total_docs: number;
  chunks: number;
}
export interface RagProgress {
  done: number;
  total: number;
  phase: "running" | "done" | "cancelled";
}
export interface AskSource {
  n: number;
  document_id: number;
  title: string;
  ord: number;
  page: number | null;
  excerpt: string;
  relation: string; // "match" | "citazione" | "simile"
}
export interface AskResult {
  answer: string;
  sources: AskSource[];
}
export const ragIndexStatus = () => invoke<RagStatus>("rag_index_status");
export const buildRagIndex = () => invoke<number>("build_rag_index");
export const cancelRagIndex = () => invoke<void>("cancel_rag_index");
export const clearRagIndex = () => invoke<void>("clear_rag_index");
export const askLibrary = (question: string, scopeKind?: string, scopeId?: number) =>
  invoke<AskResult>("ask_library", { question, scopeKind: scopeKind ?? null, scopeId: scopeId ?? null });

export type AnnotationKind = "highlight" | "underline" | "strikethrough" | "note";

export interface Annotation {
  id: number;
  page: number;
  kind: AnnotationKind;
  color: string | null;
  rects_json: string;
  quote: string | null;
  note: string | null;
  created_at: string | null;
}

/** List a document's annotations. */
export const listAnnotations = (documentId: number) =>
  invoke<Annotation[]>("list_annotations", { documentId });

/** Add an annotation (highlight by default); returns the new annotation id. */
export const addAnnotation = (a: {
  documentId: number;
  page: number;
  kind?: AnnotationKind;
  color: string;
  rectsJson: string;
  quote: string | null;
  note: string | null;
}) => invoke<number>("add_annotation", a);

/** Update an annotation's note. */
export const updateAnnotationNote = (id: number, note: string | null) =>
  invoke<void>("update_annotation_note", { id, note });

/** Delete an annotation. */
export const deleteAnnotation = (id: number) =>
  invoke<void>("delete_annotation", { id });

/** Save just a document's free-text notes (cheap autosave path from the reader). */
export const setDocumentNotes = (id: number, notes: string) =>
  invoke<void>("set_document_notes", { id, notes });

// ----- Similarity graph (embedding KNN over the whole library) -----
export interface GraphNode {
  id: number;
  title: string | null;
  year: number | null;
  /** Color of the document's most-used tag (null if untagged / colorless). */
  color: string | null;
  /** Number of edges incident to this node (0 = isolated). */
  degree: number;
  unread: boolean;
  favorite: boolean;
  /** Published in a peer-reviewed venue (same derivation as the sidebar facet). */
  peer_reviewed: boolean;
  /** Has a linked GitHub repository. */
  has_github: boolean;
  /** PCA projection of the embedding (−1..1): semantic seed position for the layout. */
  px: number;
  py: number;
  /** Layout position saved from a previous session (world units), if any. */
  sx: number | null;
  sy: number | null;
  /** Semantic community index (−1 = no sizeable cluster). */
  community: number;
  /** "doc" for papers; "note" for vault appunti (their id is the NEGATED note id). */
  kind: "doc" | "note";
  /** The note's slug ("note" kind only). */
  slug: string | null;
}

export interface ClusterInfo {
  id: number;
  /** Short label from the most characteristic title terms. */
  label: string;
  size: number;
}
export interface GraphEdge {
  a: number;
  b: number;
  /** Cosine similarity of the pair (minSim..1). */
  w: number;
}
export interface SimilarityGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  /** Sizeable semantic communities (size ≥ 3), largest first. */
  clusters: ClusterInfo[];
  embedded: number;
  total: number;
}

/** K-nearest-neighbour similarity graph over all embedded documents (k default 4, minSim default 0.55). */
export async function similarityGraph(k?: number, minSim?: number): Promise<SimilarityGraph> {
  return invoke<SimilarityGraph>("similarity_graph", { k: k ?? null, minSim: minSim ?? null });
}

/** Persist the Costellazione's settled node positions (map stays stable across sessions). */
export const saveGraphPositions = (positions: { id: number; x: number; y: number }[]) =>
  invoke<void>("save_graph_positions", { positions });

/** Explain / translate / answer a question about a selected passage with the local LLM.
 *  Streams "explain-token" events ({ token: string; req: string | null }) while generating —
 *  `req` echoes the caller's correlation id so stale streams can be filtered out.
 *  Resolves with the full text. */
export async function aiExplain(args: {
  text: string;
  task: "explain" | "translate" | "ask";
  question?: string | null;
  docId?: number | null;
  req?: string | null;
}): Promise<string> {
  return invoke<string>("ai_explain", {
    text: args.text,
    task: args.task,
    question: args.question ?? null,
    docId: args.docId ?? null,
    req: args.req ?? null,
  });
}

/** Absolute path of a document's PDF on disk (null for reference-only items). */
export async function documentPath(id: number): Promise<string | null> {
  return invoke<string | null>("document_path", { id });
}


// ===== Wiki della libreria =====
export interface WikiClaim { text: string; page: number | null }
export interface WikiSource {
  n: number;
  document_id: number;
  title: string;
  year: number | null;
  claims: WikiClaim[];
  used: boolean;
}
export interface WikiPageMeta {
  slug: string;
  concept: string;
  title: string;
  generated_at: string | null;
  model: string | null;
  n_sources: number;
  stale: boolean;
}
export interface WikiPage {
  slug: string;
  concept: string;
  title: string;
  html: string;
  sources: WikiSource[];
  generated_at: string | null;
  model: string | null;
}
/** All wiki pages (metadata only), alphabetical. */
export const wikiList = () => invoke<WikiPageMeta[]>("wiki_list");
/** One wiki page rendered to sanitized HTML (citations and cross-links woven in). */
export const wikiGet = (slug: string) => invoke<WikiPage>("wiki_get", { slug });
/** Generate (or regenerate) the page for a concept with the local LLM.
 *  Sources: `ids` when given (explicit selection, max 10), else the tag with
 *  the same name / `tagId`, else semantic search. Emits "wiki-progress"
 *  events ({ phase, done, total, concept }); returns the slug. */
export const wikiGenerate = (concept: string, tagId?: number | null, ids?: number[] | null) =>
  invoke<string>("wiki_generate", { concept, tagId: tagId ?? null, ids: ids ?? null });
export const wikiDelete = (slug: string) => invoke<void>("wiki_delete", { slug });
/** Ask the running generation to stop at the next step. */
export const wikiCancel = () => invoke<void>("wiki_cancel");

// ===== Note (.md vault) =====
export interface NoteMeta {
  slug: string;
  title: string;
  excerpt: string;
  /** File creation time as epoch milliseconds. */
  created_at: number | null;
  /** File mtime as epoch milliseconds. */
  updated_at: number | null;
}
export interface NoteLink {
  slug: string;
  title: string;
}
export interface NoteView {
  slug: string;
  title: string;
  /** Raw Markdown, for the editor. */
  content_md: string;
  /** Sanitized HTML with [[wikilinks]] woven to #note-<slug> / #doc-<id>. */
  html: string;
  backlinks: NoteLink[];
  /** Absolute path of the .md file on disk. */
  path: string;
  /** Creation / last-modified time as epoch milliseconds. */
  created_at: number | null;
  updated_at: number | null;
}
/** All notes (metadata only), newest first. */
export const listNotes = () => invoke<NoteMeta[]>("list_notes");
/** One note: raw body + rendered HTML + backlinks. */
export const getNote = (slug: string) => invoke<NoteView>("get_note", { slug });
/** Export a note to a file: format "html" (self-contained) or "latex" (.tex + figures). */
export const exportNote = (slug: string, format: "html" | "latex", path: string) =>
  invoke<void>("export_note", { slug, format, path });
/** Render draft Markdown to sanitized HTML (live editor preview; math + images). */
export const previewMarkdown = (md: string) => invoke<string>("preview_markdown", { md });
/** The note as a standalone HTML document (for printing to PDF). */
export const noteExportHtml = (slug: string) => invoke<string>("note_export_html", { slug });
/** Create a note from a title; returns its slug. */
export const createNote = (title: string) => invoke<string>("create_note", { title });
/** Overwrite a note's body; returns refreshed metadata. */
export const saveNote = (slug: string, contentMd: string) =>
  invoke<NoteMeta>("save_note", { slug, contentMd });
/** Append a Markdown block to a note (never overwrites); returns refreshed metadata. */
export const appendToNote = (slug: string, markdown: string) =>
  invoke<NoteMeta>("append_to_note", { slug, markdown });
/** Rename a note (new title + renamed .md file); returns the new slug. */
export const renameNote = (slug: string, newTitle: string) =>
  invoke<string>("rename_note", { slug, newTitle });
export const deleteNote = (slug: string) => invoke<void>("delete_note", { slug });
/** Open the notes vault folder in the file explorer. */
export const revealNotesDir = () => invoke<void>("reveal_notes_dir");

// ===== Sintesi sulla selezione + percorso di lettura =====
export interface ReviewSource {
  n: number;
  document_id: number;
  title: string;
  year: number | null;
  citekey: string | null;
}
export interface AiDocResult {
  /** Markdown grezzo (citazioni come [n]). */
  md: string;
  /** HTML sanificato con le [n] linkate a #src-n. */
  html: string;
  sources: ReviewSource[];
}
/** Confronto strutturato di 2-3 paper (tabella + sintesi, AI locale). */
export const compareDocuments = (ids: number[]) =>
  invoke<AiDocResult>("compare_documents", { ids });
/** Mini rassegna della letteratura sulla selezione (2-10 paper, AI locale). */
export const generateReview = (ids: number[]) =>
  invoke<AiDocResult>("generate_review", { ids });
/** Raccoglie i risultati quantitativi dei paper in un'unica griglia confrontabile. */
export const harvestResults = (ids: number[]) =>
  invoke<string[][]>("harvest_results", { ids });

export interface PathStep {
  document_id: number | null;
  title: string;
  year: number | null;
  reason: string;
  in_library: boolean;
  doi: string | null;
}
/** Cosa leggere prima per capire un paper (grafo citazioni + embedding, senza LLM). */
export const readingPath = (id: number) => invoke<PathStep[]>("reading_path", { id });

// ===== Progetti LaTeX (cartelle in app_data/projects, compilazione via toolchain di sistema) =====
export interface ProjectMeta {
  slug: string;
  name: string;
  /** mtime del file più recente, epoch ms. */
  updated_at: number | null;
}
export interface ProjectFile {
  rel: string;
  size: number;
}
export interface CompileResult {
  ok: boolean;
  /** Strumento usato ("tectonic" / "latexmk"), vuoto se nessuno installato. */
  tool: string;
  /** Coda del log di compilazione (contiene l'errore quando fallisce). */
  log: string;
  pdf_rel: string | null;
}
export const listProjects = () => invoke<ProjectMeta[]>("list_projects");
/** Crea cartella + main.tex + refs.bib (sincronizzato dalla libreria); ritorna lo slug. */
export const createProject = (name: string) => invoke<string>("create_project", { name });
export const projectFiles = (slug: string) => invoke<ProjectFile[]>("project_files", { slug });
export const readProjectFile = (slug: string, rel: string) =>
  invoke<string>("read_project_file", { slug, rel });
export const writeProjectFile = (slug: string, rel: string, content: string) =>
  invoke<void>("write_project_file", { slug, rel, content });
/** Contenuto binario in base64 (per l'anteprima del PDF compilato). */
export const readProjectFileB64 = (slug: string, rel: string) =>
  invoke<string>("read_project_file_b64", { slug, rel });
/** Riscrive refs.bib con tutta la libreria; ritorna il numero di voci. */
export const syncProjectBib = (slug: string) => invoke<number>("sync_project_bib", { slug });
export const compileProject = (slug: string) => invoke<CompileResult>("compile_project", { slug });
export const revealProjectDir = (slug: string) => invoke<void>("reveal_project_dir", { slug });