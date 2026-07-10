<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import * as pdfjsLib from "pdfjs-dist";
  import { TextLayer } from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import {
    listAnnotations,
    addAnnotation,
    updateAnnotationNote,
    deleteAnnotation,
    setDocumentNotes,
    getDocumentMeta,
    getLastPage,
    setLastPage,
    type Annotation,
    type AnnotationKind,
  } from "$lib/api";
  import { printDocument } from "$lib/print";
  import { revealDocument } from "$lib/share";
  import { extractTable, exportTable, aiCleanTable, extractRegionText, writeTextFile, writeBinaryFile, aiExplain, formulaToLatex, mathocrStatus, formulaToLatexAi, tableFromImageAi, textFromImageAi, getAiSettings, aiListModels } from "$lib/api";
  import { escapeLatex, textToLatex, tableToLatex, tableToMarkdown, tableToCsv } from "$lib/latex";
  import { renderMathString, isMathValid } from "$lib/math";
  import { save } from "@tauri-apps/plugin-dialog";
  import ShareMenu from "$lib/ShareMenu.svelte";
  import RadialMenu from "$lib/RadialMenu.svelte";
  import type { RadialItem } from "$lib/radial";

  pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

  let {
    id,
    title,
    link = null,
    aiEnabled = false,
    initialPage = null,
    onClose,
    onSendToNote,
    onOpenNotes,
  }: {
    id: number;
    title: string;
    link?: string | null;
    aiEnabled?: boolean;
    initialPage?: number | null;
    onClose: () => void;
    /** Send content (with its page) to an appunto; handled by +page. `opts.code`
     *  wraps it as a fenced code block (e.g. LaTeX); `opts.raw` inserts it verbatim
     *  as Markdown (an image embed or a Markdown table). */
    onSendToNote?: (
      content: string,
      page: number | null,
      pos: { x: number; y: number },
      opts?: { code?: string; label?: string; raw?: boolean },
    ) => void;
    /** Close the reader and jump to the Appunti (.md notes) surface; handled by +page. */
    onOpenNotes?: () => void;
  } = $props();

  // ----- Zoom memory: remember the last used scale per document (localStorage) -----
  const ZOOM_KEY = "scriptorium-viewer-zoom";
  let zoomSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let zoomDirty = false;

  /** Saved zoom for this document, clamped to the viewer's range; null if absent or corrupt. */
  function loadSavedZoom(): number | null {
    try {
      const raw = localStorage.getItem(ZOOM_KEY);
      if (!raw) return null;
      const map: unknown = JSON.parse(raw);
      if (!map || typeof map !== "object" || Array.isArray(map)) return null;
      const v = Number((map as Record<string, unknown>)[String(id)]);
      return Number.isFinite(v) ? Math.min(4, Math.max(0.4, v)) : null;
    } catch {
      return null; // a corrupt store must never break opening the viewer
    }
  }
  function writeZoomNow() {
    zoomDirty = false;
    try {
      let map: Record<string, number> = {};
      try {
        const parsed: unknown = JSON.parse(localStorage.getItem(ZOOM_KEY) ?? "{}");
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
          map = parsed as Record<string, number>;
        }
      } catch {
        /* corrupt store: start a fresh map */
      }
      map[String(id)] = scale;
      // Cap the map at 200 entries, dropping from the front of the key order.
      const keys = Object.keys(map);
      if (keys.length > 200) {
        for (const k of keys.slice(0, keys.length - 200)) delete map[k];
      }
      localStorage.setItem(ZOOM_KEY, JSON.stringify(map));
    } catch {
      /* quota/serialization errors are non-fatal */
    }
  }
  function saveZoomSoon() {
    zoomDirty = true;
    clearTimeout(zoomSaveTimer);
    zoomSaveTimer = setTimeout(writeZoomNow, 500);
  }

  const HL_COLOR = "#ffd54a";

  /** Only allow hex colors into inline styles (defense-in-depth vs CSS injection). */
  function safeColor(c: string | null | undefined): string {
    return c && /^#[0-9a-fA-F]{3,8}$/.test(c) ? c : HL_COLOR;
  }

  // Selection-time palette: swatch colors + the active markup style.
  const PALETTE: { color: string; label: string }[] = [
    { color: "#ffd54a", label: "Giallo" },
    { color: "#7ed957", label: "Verde" },
    { color: "#5aa9ff", label: "Blu" },
    { color: "#ff8fb1", label: "Rosa" },
    { color: "#ffb454", label: "Arancio" },
  ];
  const KINDS: { kind: AnnotationKind; label: string; glyph: string }[] = [
    { kind: "highlight", label: "Evidenzia", glyph: "▆" },
    { kind: "underline", label: "Sottolinea", glyph: "U" },
    { kind: "strikethrough", label: "Barra", glyph: "S" },
  ];
  let hlKind = $state<AnnotationKind>("highlight");

  // Right-hand side panel: free-text notes + the annotations index.
  let panel = $state<"none" | "notes" | "annos">("none");
  let docNotes = $state("");
  let notesSaved = $state(true);
  let notesTimer: ReturnType<typeof setTimeout> | undefined;
  // Annotations-panel filters.
  let annoKindFilter = $state<AnnotationKind | "all">("all");
  let annoColorFilter = $state<string | "all">("all");
  let editingAnno = $state<number | null>(null);
  let editingNote = $state("");

  let pagesEl: HTMLDivElement;
  let overlayEl: HTMLDivElement;
  let scale = $state(loadSavedZoom() ?? 1.3);
  let loading = $state(true);
  let error = $state("");
  let annos = $state<Annotation[]>([]);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let pdf: any = null;
  let pageWraps: HTMLDivElement[] = [];

  let selBtn = $state<{ x: number; y: number } | null>(null);
  let pending: { byPage: Map<number, number[][]>; quote: string } | null = null;
  let popover = $state<{ id: number; quote: string; note: string; x: number; y: number } | null>(
    null,
  );
  // Inline "are you sure?" for deleting an annotation/note; resets whenever the popover changes.
  let confirmDel = $state(false);
  $effect(() => {
    popover;
    confirmDel = false;
  });
  let rotation = $state(0);
  // Reading options
  let noteMode = $state(false); // click on a page to drop a sticky note
  let night = $state(false); // invert page colors for night reading
  let spread = $state(false); // two pages side by side
  let showHelp = $state(false); // keyboard-shortcuts overlay
  const NOTE_COLOR = "#f4b73e";
  // Table extraction: drag a rectangle over a table, reconstruct it, export.
  let tableMode = $state(false);
  let dragRect = $state<{ x: number; y: number; w: number; h: number } | null>(null); // screen px while dragging
  let dragStart: { x: number; y: number; wrap: HTMLDivElement } | null = null;
  let tableModal = $state(false);
  let tableLoading = $state(false);
  let tableGrid = $state<string[][]>([]);
  let aiCleaning = $state(false);
  // Table engine chosen in the modal: native (pdfium text) or a local vision LLM.
  let tableEngine = $state<"native" | "ollama">("native");
  let tableImg = $state(""); // cropped table image (data URL), for the vision engine
  let tableRegion: { x: number; y: number; w: number; h: number } | null = null;
  let tablePage = 0;
  let tableError = $state("");
  let tableReq = 0; // epoch token, like formulaReq
  // Text extraction: drag a region to pull its plain text (native) or OCR it (vision LLM).
  let textMode = $state(false);
  let textModal = $state(false);
  let textLoading = $state(false);
  let textContent = $state("");
  let textEngine = $state<"native" | "ollama">("native");
  let textImg = $state(""); // cropped region (data URL), for the vision OCR engine
  let textRegion: { x: number; y: number; w: number; h: number } | null = null;
  let textPageId = 0;
  let textError = $state("");
  let textReq = 0; // epoch token, like formulaReq
  // Page a table/text region was extracted from — cited when sending LaTeX to an appunto.
  let extractPage = $state<number | null>(null);
  // Table preview toggle: grid vs. the chosen export format.
  let tableView = $state<"grid" | "latex">("grid");
  // Export format chosen in each modal (drives Copia / Salva / → Appunti).
  let textFormat = $state<"txt" | "latex" | "md">("txt");
  let tableFormat = $state<"md" | "latex" | "csv">("md");
  let formulaFormat = $state<"latex" | "md">("latex");
  // Figure extraction: drag a region, keep it as a PNG (save or embed in an appunto).
  let figureMode = $state(false);
  let figureModal = $state(false);
  let figureImg = $state(""); // data URL of the cropped region
  let figurePage = $state<number | null>(null);
  let figureError = $state("");
  // Formula → LaTeX (local math-OCR): drag a region over an equation, recognize it.
  let formulaMode = $state(false);
  let formulaModal = $state(false);
  let formulaLoading = $state(false);
  let formulaLatex = $state("");
  let formulaImg = $state(""); // data URL of the cropped region, shown as a preview
  let formulaMulti = $state(false); // recognize several stacked equations as one block
  let formulaError = $state(""); // a hard error (download/decode), distinct from "no formula"
  let formulaEmpty = $state(false); // OCR ran and recognized nothing (an emptied EDITOR must stay mounted)
  let formulaReq = 0; // epoch token: a stale recognition must not overwrite a newer one
  let formulaEngine = $state<"local" | "ollama">("local"); // bundled math-OCR vs vision LLM
  // KaTeX preview of the recognized LaTeX, updated live while editing so OCR mistakes
  // can be spotted and corrected on the spot. `formulaInvalid` carries KaTeX's parse
  // error (empty when the formula is valid) → the editor flags a broken formula.
  let formulaPreviewHtml = $state("");
  let formulaInvalid = $state("");
  // True when the local OCR's top hypothesis didn't render and we fell back to a
  // lower-ranked candidate that does — surfaced so the substitution isn't silent.
  let formulaReranked = $state(false);
  let formulaPrevTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleFormulaPreview() {
    formulaReranked = false; // a hand-edit supersedes the auto-pick note
    clearTimeout(formulaPrevTimer);
    formulaPrevTimer = setTimeout(renderFormulaPreview, 200);
  }
  function renderFormulaPreview() {
    // The editor holds pure LaTeX — render it straight with KaTeX (no markdown round-trip):
    // richer coverage than MathML, correct upright \mathrm (fractions/scripts included),
    // native gathered/aligned. Strip any math delimiters the user may have pasted back.
    const l = formulaLatex
      .trim()
      .replace(/^\$\$?\s*/, "")
      .replace(/\s*\$\$?$/, "")
      .replace(/^\\\[\s*/, "")
      .replace(/\s*\\\]$/, "")
      .trim();
    if (!l) {
      formulaPreviewHtml = "";
      formulaInvalid = "";
      return;
    }
    const { html, error } = renderMathString(l, true);
    formulaPreviewHtml = html;
    formulaInvalid = error;
  }

  /** Makes a modal card draggable by its header (.tablehd): the extraction windows
   *  can be moved aside to compare with the page beneath. Buttons/selects inside
   *  the header still work — dragging only starts on inert header space. */
  function dragCard(node: HTMLElement) {
    let sx = 0, sy = 0, dx = 0, dy = 0;
    let baseLeft = 0, baseTop = 0, baseW = 0; // untranslated card position/size
    const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));
    function down(ev: MouseEvent) {
      if (ev.button !== 0) return;
      const t = ev.target;
      if (!(t instanceof HTMLElement)) return;
      if (!t.closest(".tablehd")) return; // drag by the header only
      if (t.closest("button, select, input, textarea, a")) return; // keep controls clickable
      const r = node.getBoundingClientRect();
      baseLeft = r.left - dx;
      baseTop = r.top - dy;
      baseW = r.width;
      sx = ev.clientX - dx;
      sy = ev.clientY - dy;
      ev.preventDefault(); // no text selection while dragging
      window.addEventListener("mousemove", move);
      window.addEventListener("mouseup", up);
      window.addEventListener("blur", up);
    }
    function move(ev: MouseEvent) {
      // The mouseup can be lost (Alt+Tab mid-drag): stop as soon as the primary
      // button is no longer held, so the card doesn't glue to the cursor.
      if (!(ev.buttons & 1)) return up();
      // Clamp so the header always stays reachable: never above/below the
      // viewport, and at least 80px of card width inside it horizontally.
      dx = clamp(ev.clientX - sx, -(baseLeft + baseW - 80), window.innerWidth - baseLeft - 80);
      dy = clamp(ev.clientY - sy, -baseTop, window.innerHeight - baseTop - 40);
      node.style.transform = `translate(${dx}px, ${dy}px)`;
    }
    function up() {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      window.removeEventListener("blur", up);
    }
    node.addEventListener("mousedown", down);
    return {
      destroy() {
        node.removeEventListener("mousedown", down);
        up();
      },
    };
  }
  // Shared vision-LLM state for the "Ollama" engine in both modals. `aiEnabled` is
  // already a prop; here we only cache the resolved settings for the model list.
  let aiSettings: { provider: string; ollama_url: string; lmstudio_url: string; model: string } | null = null;
  let visionModels = $state<string[]>([]);
  let visionModel = $state("");
  let visionModelsLoading = false; // in-flight guard: avoid concurrent model-list fetches
  // A model whose name looks vision-capable (multimodal / VLM). Used to highlight
  // VLMs in the picker and to auto-select a sensible default for the OCR engines.
  const VISION_RE = /vl|vision|llava|minicpm|moondream|bakllava|granite.*vision|qwen.*vl|gemma.*3|pixtral|llama.*vision/i;
  const isVisionModel = (m: string) => VISION_RE.test(m);
  let visionGroups = $derived({
    vision: visionModels.filter(isVisionModel),
    other: visionModels.filter((m) => !isVisionModel(m)),
  });
  let printing = $state(false);
  let notice = $state("");
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let renderTimer: ReturnType<typeof setTimeout> | undefined;
  let renderToken = 0; // guards against overlapping re-renders (rapid zoom)

  // ----- find in document -----
  let findOpen = $state(false);
  let findQuery = $state("");
  let findHits = $state<{ page: number; rects: number[][] }[]>([]);
  let findActive = $state(-1);
  let findPending = $state(false);
  let findCapped = $state(false);
  let findInput = $state<HTMLInputElement>();
  let findTimer: ReturnType<typeof setTimeout> | undefined;
  const MAX_FIND_HITS = 2000;

  function setNotice(s: string) {
    notice = s;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 6000);
  }
  let outline = $state<{ title: string; page: number; depth: number }[]>([]);
  let showToc = $state(false);
  let currentPage = 0;
  let restorePage = 0;

  async function buildOutline() {
    outline = [];
    try {
      const raw = await pdf.getOutline();
      if (!raw) return;
      const flat: { title: string; page: number; depth: number }[] = [];
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const walk = async (items: any[], depth: number) => {
        for (const it of items) {
          let page = 0;
          try {
            const dest = typeof it.dest === "string" ? await pdf.getDestination(it.dest) : it.dest;
            if (dest && dest[0]) page = (await pdf.getPageIndex(dest[0])) + 1;
          } catch {
            /* unresolved dest */
          }
          flat.push({ title: it.title ?? "—", page, depth });
          if (it.items && it.items.length) await walk(it.items, depth + 1);
        }
      };
      await walk(raw, 0);
      outline = flat;
    } catch {
      /* no outline */
    }
  }

  function scrollToPage(n: number, behavior: ScrollBehavior = "smooth") {
    const w = pageWraps[n - 1];
    if (!w || !pagesEl) return;
    // Scroll the pages container directly (deterministic; survives re-render).
    const top = w.getBoundingClientRect().top - pagesEl.getBoundingClientRect().top + pagesEl.scrollTop;
    pagesEl.scrollTo({ top: Math.max(0, top), behavior });
  }
  function goTo(page: number) {
    if (page > 0) scrollToPage(page);
  }
  function onScroll() {
    if (!pagesEl) return;
    const top = pagesEl.getBoundingClientRect().top;
    let best = 1;
    let bestDist = Infinity;
    for (let i = 0; i < pageWraps.length; i++) {
      const w = pageWraps[i];
      if (!w) continue;
      const d = Math.abs(w.getBoundingClientRect().top - top);
      if (d < bestDist) {
        bestDist = d;
        best = i + 1;
      }
    }
    currentPage = best;
  }

  /** Rotate a normalized [x,y,w,h] rect within the unit square by deg (0/90/180/270, clockwise). */
  function rotateRect([x, y, w, h]: number[], deg: number): number[] {
    switch (((deg % 360) + 360) % 360) {
      case 90:
        return [1 - y - h, x, h, w];
      case 180:
        return [1 - x - w, 1 - y - h, w, h];
      case 270:
        return [y, 1 - x - w, h, w];
      default:
        return [x, y, w, h];
    }
  }

  function drawHighlights(page: number) {
    const wrap = pageWraps[page - 1];
    if (!wrap) return;
    const layer = wrap.querySelector(".annolayer") as HTMLDivElement | null;
    if (!layer) return;
    layer.innerHTML = "";
    for (const a of annos.filter((x) => x.page === page)) {
      let rects: number[][] = [];
      try {
        rects = JSON.parse(a.rects_json);
      } catch {
        continue;
      }
      for (const rect of rects) {
        const [x, y, w, h] = rotateRect(rect, rotation);
        // A zero-size rect is a sticky note (a point), drawn as a pin icon.
        if (w === 0 && h === 0) {
          const pin = document.createElement("div");
          pin.className = "notepin";
          pin.style.left = x * 100 + "%";
          pin.style.top = y * 100 + "%";
          // Monochrome marker: a filled note/comment glyph that reads professionally.
          pin.innerHTML =
            '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">' +
            '<path fill="currentColor" stroke="#fffdf8" stroke-width="1" d="M6 3h12a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-6.2L7 21v-4.2A2 2 0 0 1 5 14.8V5a2 2 0 0 1 1-2z"/>' +
            '<rect x="8" y="7.4" width="8" height="1.5" rx="0.75" fill="#fffdf8"/>' +
            '<rect x="8" y="10.6" width="5.5" height="1.5" rx="0.75" fill="#fffdf8"/>' +
            "</svg>";
          if (a.note) pin.title = a.note;
          pin.dataset.aid = String(a.id);
          pin.onclick = (e) => {
            e.stopPropagation();
            popover = { id: a.id, quote: "", note: a.note ?? "", x: e.clientX, y: e.clientY };
          };
          layer.appendChild(pin);
          continue;
        }
        const hl = document.createElement("div");
        hl.className = "hl";
        hl.dataset.aid = String(a.id);
        hl.style.left = x * 100 + "%";
        hl.style.top = y * 100 + "%";
        hl.style.width = w * 100 + "%";
        hl.style.height = h * 100 + "%";
        const col = safeColor(a.color);
        if (a.kind === "underline") {
          hl.style.borderBottom = "2px solid " + col;
        } else if (a.kind === "strikethrough") {
          // A line drawn through the vertical middle of the selection box.
          hl.style.backgroundImage = "linear-gradient(" + col + "," + col + ")";
          hl.style.backgroundSize = "100% 2px";
          hl.style.backgroundPosition = "0 50%";
          hl.style.backgroundRepeat = "no-repeat";
        } else {
          hl.style.background = col + "59";
        }
        if (a.note) hl.title = a.note;
        hl.onclick = (e) => {
          e.stopPropagation();
          popover = {
            id: a.id,
            quote: a.quote ?? "",
            note: a.note ?? "",
            x: e.clientX,
            y: e.clientY,
          };
        };
        layer.appendChild(hl);
      }
    }
  }

  async function renderPages() {
    if (!pdf || !pagesEl) return;
    // Clearing innerHTML resets scrollTop to 0; remember the page so zoom/rotate
    // don't jump the reader back to page 1.
    const keepPage = currentPage;
    const myToken = ++renderToken;
    findIndex = []; // text layers are rebuilt, so the cached index is stale
    const dpr = window.devicePixelRatio || 1;

    // --- Phase 1: build all page wraps at the correct size (no content yet).
    // Doing this first means the full layout/scroll height is known immediately,
    // so we can restore the reading position BEFORE the slow per-page rendering
    // — no visible flash back to page 1.
    type Slot = { n: number; page: any; viewport: any; canvas: HTMLCanvasElement; textDiv: HTMLDivElement };
    const slots: Slot[] = [];
    const wraps: HTMLDivElement[] = [];
    for (let n = 1; n <= pdf.numPages; n++) {
      if (myToken !== renderToken) return;
      const page = await pdf.getPage(n);
      const viewport = page.getViewport({ scale, rotation });
      const wrap = document.createElement("div");
      wrap.className = "pagewrap";
      wrap.dataset.page = String(n);
      wrap.style.width = viewport.width + "px";
      wrap.style.height = viewport.height + "px";
      wrap.style.setProperty("--total-scale-factor", String(scale));
      const canvas = document.createElement("canvas");
      canvas.className = "pdfcanvas";
      canvas.width = Math.floor(viewport.width * dpr);
      canvas.height = Math.floor(viewport.height * dpr);
      canvas.style.width = viewport.width + "px";
      canvas.style.height = viewport.height + "px";
      wrap.appendChild(canvas);
      const textDiv = document.createElement("div");
      textDiv.className = "textLayer";
      wrap.appendChild(textDiv);
      const annolayer = document.createElement("div");
      annolayer.className = "annolayer";
      wrap.appendChild(annolayer);
      const findlayer = document.createElement("div");
      findlayer.className = "findlayer";
      wrap.appendChild(findlayer);
      wraps.push(wrap);
      slots.push({ n, page, viewport, canvas, textDiv });
    }
    if (myToken !== renderToken) return;
    // Swap in the new pages in one shot and restore scroll before rendering.
    pagesEl.replaceChildren(...wraps);
    pageWraps = wraps;
    await tick();
    if (myToken !== renderToken) return;
    if (keepPage > 1) scrollToPage(keepPage, "instant");

    // --- Phase 2: render canvas + text layer for each page.
    for (const s of slots) {
      if (myToken !== renderToken) return;
      const ctx = s.canvas.getContext("2d");
      if (ctx) {
        try {
          await s.page.render({
            canvasContext: ctx,
            viewport: s.viewport,
            transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
          }).promise;
        } catch (e) {
          console.error("page render failed", e);
        }
      }
      if (myToken !== renderToken) return;
      try {
        const textLayer = new TextLayer({
          textContentSource: s.page.streamTextContent(),
          container: s.textDiv,
          viewport: s.viewport,
        });
        await textLayer.render();
      } catch (e) {
        console.error("text layer failed", e);
      }
      drawHighlights(s.n);
      drawFind(s.n);
    }
  }

  async function load() {
    loading = true;
    error = "";
    try {
      const buf = (await invoke("read_pdf", { id })) as ArrayBuffer;
      pdf = await pdfjsLib.getDocument({ data: new Uint8Array(buf) }).promise;
      annos = await listAnnotations(id);
      docNotes = (await getDocumentMeta(id).then((m) => m.notes ?? "").catch(() => "")) ?? "";
      notesSaved = true;
      restorePage = (await getLastPage(id).catch(() => null)) ?? 0;
      // A parent-provided initial page (e.g. a citation/deep link) wins over the
      // persisted last-read page for the first scroll only; what gets persisted
      // (set_last_page on close) is untouched.
      if (
        initialPage != null &&
        Number.isInteger(initialPage) &&
        initialPage >= 1 &&
        initialPage <= pdf.numPages
      ) {
        restorePage = initialPage;
      }
      await buildOutline();
      await renderPages();
      if (restorePage > 1) scrollToPage(restorePage, "instant");
    } catch (e) {
      error = "Impossibile aprire il PDF: " + e;
    } finally {
      loading = false;
    }
  }

  function clearSelection() {
    selBtn = null;
    pending = null;
    window.getSelection()?.removeAllRanges();
  }

  /** Cancel a selection rectangle mid-drag: turning a mode off must also drop the
   *  drag in progress, or the mouseup would still hand the rectangle to an
   *  extractor (the tail of onTableUp defaults to the table one). */
  function cancelExtractDrag() {
    dragStart = null;
    dragRect = null;
  }
  function toggleTable() {
    tableMode = !tableMode;
    if (tableMode) {
      noteMode = false;
      textMode = false;
      formulaMode = false;
      figureMode = false;
      clearSelection();
      cancelExtractDrag();
      setNotice("Modalità tabella: trascina un rettangolo attorno a una tabella");
    } else cancelExtractDrag();
  }
  function toggleText() {
    textMode = !textMode;
    if (textMode) {
      noteMode = false;
      tableMode = false;
      formulaMode = false;
      figureMode = false;
      clearSelection();
      cancelExtractDrag();
      setNotice("Estrai testo: trascina un rettangolo attorno al testo da copiare");
    } else cancelExtractDrag();
  }
  function toggleFormula() {
    formulaMode = !formulaMode;
    if (formulaMode) {
      noteMode = false;
      tableMode = false;
      textMode = false;
      figureMode = false;
      clearSelection();
      cancelExtractDrag();
      setNotice("Formula → LaTeX: trascina un rettangolo attorno a una formula");
    } else cancelExtractDrag();
  }
  function toggleFigure() {
    figureMode = !figureMode;
    if (figureMode) {
      noteMode = false;
      tableMode = false;
      textMode = false;
      formulaMode = false;
      clearSelection();
      cancelExtractDrag();
      setNotice("Estrai figura: trascina un rettangolo attorno alla figura da ritagliare come PNG");
    } else cancelExtractDrag();
  }
  /** Crop a screen-space rectangle out of a page's rendered canvas to a PNG data
   *  URL — used to hand a formula image to the local math-OCR. Copies the real
   *  (non-inverted) pixels, so night mode doesn't affect recognition. */
  function cropCanvasRegion(
    wrap: HTMLDivElement,
    rect: { x: number; y: number; w: number; h: number },
  ): string | null {
    const canvas = wrap.querySelector("canvas.pdfcanvas") as HTMLCanvasElement | null;
    if (!canvas) return null;
    const cb = canvas.getBoundingClientRect();
    const scaleX = canvas.width / cb.width;
    const scaleY = canvas.height / cb.height;
    const left = Math.max(rect.x, cb.left);
    const top = Math.max(rect.y, cb.top);
    const right = Math.min(rect.x + rect.w, cb.right);
    const bottom = Math.min(rect.y + rect.h, cb.bottom);
    const dw = right - left;
    const dh = bottom - top;
    if (dw < 2 || dh < 2) return null;
    const sx = (left - cb.left) * scaleX;
    const sy = (top - cb.top) * scaleY;
    const sw = dw * scaleX;
    const sh = dh * scaleY;
    const out = document.createElement("canvas");
    out.width = Math.max(1, Math.round(sw));
    out.height = Math.max(1, Math.round(sh));
    const ctx = out.getContext("2d");
    if (!ctx) return null;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, out.width, out.height);
    ctx.drawImage(canvas, sx, sy, sw, sh, 0, 0, out.width, out.height);
    return out.toDataURL("image/png");
  }
  function wrapAt(cx: number, cy: number): HTMLDivElement | undefined {
    for (const w of pageWraps) {
      if (!w) continue;
      const b = w.getBoundingClientRect();
      if (cx >= b.left && cx <= b.right && cy >= b.top && cy <= b.bottom) return w;
    }
    return undefined;
  }
  function onTableDown(e: MouseEvent) {
    if (!tableMode && !textMode && !formulaMode && !figureMode) return;
    const wrap = wrapAt(e.clientX, e.clientY);
    if (!wrap) return;
    e.preventDefault();
    dragStart = { x: e.clientX, y: e.clientY, wrap };
    dragRect = { x: e.clientX, y: e.clientY, w: 0, h: 0 };
  }
  function onTableMove(e: MouseEvent) {
    if (!dragStart) return;
    dragRect = {
      x: Math.min(dragStart.x, e.clientX),
      y: Math.min(dragStart.y, e.clientY),
      w: Math.abs(e.clientX - dragStart.x),
      h: Math.abs(e.clientY - dragStart.y),
    };
  }
  async function onTableUp() {
    const start = dragStart;
    const rect = dragRect;
    dragStart = null;
    dragRect = null;
    if (!start || !rect || rect.w < 8 || rect.h < 8) return;
    const b = start.wrap.getBoundingClientRect();
    const page = Number(start.wrap.dataset.page);
    extractPage = Number.isFinite(page) ? page : null;
    const norm = [
      (rect.x - b.left) / b.width,
      (rect.y - b.top) / b.height,
      rect.w / b.width,
      rect.h / b.height,
    ];
    // Query in the unrotated (rotation-0) frame, like annotations.
    const r0 = rotateRect(norm, (360 - rotation) % 360);
    const region = { x: r0[0], y: r0[1], w: r0[2], h: r0[3] };
    if (figureMode) {
      figureMode = false;
      const dataUrl = cropCanvasRegion(start.wrap, rect);
      if (!dataUrl) {
        setNotice("Impossibile ritagliare la figura");
        return;
      }
      figureImg = dataUrl;
      figurePage = page;
      figureError = "";
      figureModal = true;
      return;
    }
    if (formulaMode) {
      formulaMode = false;
      const dataUrl = cropCanvasRegion(start.wrap, rect);
      if (!dataUrl) {
        setNotice("Impossibile ritagliare la formula");
        return;
      }
      formulaImg = dataUrl;
      formulaMulti = false;
      formulaEngine = "local";
      formulaModal = true;
      runFormula(false, "local");
      return;
    }
    if (textMode) {
      textMode = false;
      // Keep both the region (native pdfium text) and a cropped image (the vision
      // OCR engine); the modal's engine toggle chooses which to use.
      textRegion = region;
      textPageId = page;
      textImg = cropCanvasRegion(start.wrap, rect) ?? "";
      textEngine = "native";
      textModal = true;
      runTextExtract("native");
      return;
    }
    // Defense in depth: if no mode is armed anymore (cancelled mid-drag via key
    // or Esc), the rectangle must not fall through to the table extractor.
    if (!tableMode) return;
    tableMode = false;
    // Keep both the region (native pdfium extraction) and a cropped image (the
    // vision engine); the modal's engine toggle chooses which to use.
    tableRegion = region;
    tablePage = page;
    tableImg = cropCanvasRegion(start.wrap, rect) ?? "";
    tableEngine = "native";
    tableModal = true;
    runTable("native");
  }
  async function selectTextEngine(engine: "native" | "ollama") {
    textEngine = engine; // highlight the picked toggle right away
    if (engine === "ollama") {
      textLoading = true; // show progress while the model list loads
      const req = textReq; // a competing selection bumps this via runTextExtract
      await ensureVisionModels();
      if (textReq !== req) return; // a newer engine choice superseded us
    }
    runTextExtract(engine);
  }
  /** (Re)extract the text region with the chosen engine: native pdfium or vision OCR. */
  async function runTextExtract(engine: "native" | "ollama" = textEngine) {
    const req = ++textReq;
    textEngine = engine;
    textLoading = true;
    textError = "";
    textContent = "";
    try {
      let out: string;
      if (engine === "ollama") {
        if (!textImg) throw "Immagine del testo non disponibile";
        if (!visionModel) throw "Nessun modello di visione disponibile — abilita l'AI e scarica un modello vision (es. qwen2.5vl, minicpm-v).";
        out = await textFromImageAi(textImg, visionModel);
      } else {
        if (!textRegion) throw "Regione non disponibile";
        out = await extractRegionText(id, textPageId, textRegion);
      }
      if (textReq !== req) return;
      textContent = out;
    } catch (err) {
      if (textReq !== req) return;
      textError = String(err);
      setNotice("Errore estrazione testo: " + err);
    } finally {
      if (textReq === req) textLoading = false;
    }
  }

  // ----- Unified format-aware export (Copia / Salva / → Appunti) for the modals -----
  /** The current text/table/formula rendered in the given export format. */
  function textOut(fmt: "txt" | "latex" | "md" = textFormat): string {
    return fmt === "latex" ? textToLatex(textContent) : textContent;
  }
  function tableOut(fmt: "md" | "latex" | "csv" = tableFormat): string {
    if (fmt === "latex") return tableToLatex(tableGrid);
    if (fmt === "csv") return tableToCsv(tableGrid);
    return tableToMarkdown(tableGrid);
  }
  function formulaOut(fmt: "latex" | "md" = formulaFormat): string {
    const l = formulaLatex.trim();
    return fmt === "md" ? `$$\n${l}\n$$` : l;
  }
  async function copyFormatted(str: string) {
    const t = (str ?? "").trim();
    if (!t) {
      setNotice("Niente da copiare");
      return;
    }
    try {
      await navigator.clipboard.writeText(str);
      setNotice("Copiato negli appunti ✓");
    } catch {
      setNotice("Impossibile copiare negli appunti");
    }
  }
  async function saveFormatted(str: string, base: string, ext: string) {
    if (!str.trim()) {
      setNotice("Niente da salvare");
      return;
    }
    const path = await save({
      defaultPath: `${base}.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    try {
      await writeTextFile(path, str);
      setNotice("Salvato ✓");
    } catch (e) {
      error = "Errore salvataggio: " + e;
    }
  }
  /** Send content to the appunto picker; `code` wraps it in a fenced block, `raw`
   *  inserts it verbatim as Markdown, otherwise it becomes a blockquote. */
  function sendFormatted(
    content: string,
    opts: { code?: string; raw?: boolean; label: string },
    e: MouseEvent,
  ) {
    if (!content.trim()) {
      setNotice("Niente da mandare agli Appunti");
      return;
    }
    onSendToNote?.(content, extractPage, { x: e.clientX, y: e.clientY }, opts);
  }
  const textExt = () => (textFormat === "latex" ? "tex" : textFormat === "md" ? "md" : "txt");
  const tableExt = () => (tableFormat === "latex" ? "tex" : tableFormat === "csv" ? "csv" : "md");
  function textCopy() {
    copyFormatted(textOut());
  }
  function textSave() {
    saveFormatted(textOut(), "testo", textExt());
  }
  function textToNote(e: MouseEvent) {
    textModal = false;
    if (textFormat === "latex") sendFormatted(textToLatex(textContent), { code: "latex", label: "Testo (LaTeX) da" }, e);
    else if (textFormat === "md") sendFormatted(textContent, { raw: true, label: "Testo da" }, e);
    else sendFormatted(textContent, { label: "Testo da" }, e);
  }
  function tableCopy() {
    copyFormatted(tableOut());
  }
  function tableSave() {
    saveFormatted(tableOut(), "tabella", tableExt());
  }
  function tableToNote(e: MouseEvent) {
    tableModal = false;
    if (tableFormat === "latex") sendFormatted(tableToLatex(tableGrid), { code: "latex", label: "Tabella (LaTeX) da" }, e);
    else if (tableFormat === "csv") sendFormatted(tableToCsv(tableGrid), { code: "csv", label: "Tabella (CSV) da" }, e);
    else sendFormatted(tableToMarkdown(tableGrid), { raw: true, label: "Tabella da" }, e);
  }
  // ----- Wrap / unwrap the whole formula in \mathrm{…} (renders it all upright) -----
  const MATHRM_PRE = "\\mathrm{";
  /** True if `l` is exactly `\mathrm{ … }` with the outer braces enclosing everything
   *  (the closing brace of the \mathrm is the very last char), honoring `\{`/`\}`. */
  function isWrappedInMathrm(l: string): boolean {
    if (!l.startsWith(MATHRM_PRE) || !l.endsWith("}")) return false;
    let depth = 0;
    for (let i = MATHRM_PRE.length - 1; i < l.length; i++) {
      const c = l[i];
      if (c === "\\") { i++; continue; } // skip an escaped char (\{ \} \\)
      if (c === "{") depth++;
      else if (c === "}") { depth--; if (depth === 0) return i === l.length - 1; }
    }
    return false;
  }
  const formulaWrapped = $derived(isWrappedInMathrm(formulaLatex.trim()));
  // A multi-line formula carries a LaTeX environment (\begin{gathered}…). Wrapping
  // the whole thing in \mathrm{…} produces invalid LaTeX — an environment cannot
  // live inside a \mathrm box — so we forbid *wrapping* it (the button is disabled),
  // while still allowing an existing (hand-made) wrap to be removed.
  const formulaHasEnv = $derived(formulaLatex.includes("\\begin{"));
  /** Toggle wrapping the current formula in \mathrm{…}; refresh the live preview. */
  function toggleMathrmWrap() {
    const l = formulaLatex.trim();
    if (!l) return;
    if (isWrappedInMathrm(l)) {
      formulaLatex = l.slice(MATHRM_PRE.length, -1).trim();
    } else {
      if (l.includes("\\begin{")) return; // can't wrap a whole environment (invalid LaTeX)
      formulaLatex = `${MATHRM_PRE}${l}}`;
    }
    renderFormulaPreview();
  }
  function formulaCopyFmt() {
    copyFormatted(formulaOut());
  }
  function formulaSave() {
    saveFormatted(formulaOut(), "formula", formulaFormat === "md" ? "md" : "tex");
  }
  function formulaToNoteFmt(e: MouseEvent) {
    formulaModal = false;
    if (formulaFormat === "md") sendFormatted(formulaOut("md"), { raw: true, label: "Formula da" }, e);
    else sendFormatted(formulaLatex, { code: "latex", label: "Formula (LaTeX) da" }, e);
  }
  // ----- Figure (PNG) export -----
  async function saveFigurePng() {
    if (!figureImg) return;
    const path = await save({
      defaultPath: `figura${figurePage != null ? "-p" + figurePage : ""}.png`,
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (!path) return;
    try {
      await writeBinaryFile(path, figureImg);
      setNotice("Figura salvata ✓");
    } catch (e) {
      figureError = "Errore salvataggio PNG: " + e;
    }
  }
  function figureToNote(e: MouseEvent) {
    if (!figureImg) return;
    figureModal = false;
    const cap = `Figura${figurePage != null ? ` — p. ${figurePage}` : ""}`;
    const md = `![${cap}](${figureImg})`;
    onSendToNote?.(md, figurePage, { x: e.clientX, y: e.clientY }, { raw: true, label: "Figura da" });
  }
  /** Copy an arbitrary snippet (the current PDF text selection) to the clipboard. */
  async function copySelection(text: string) {
    const t = text.trim();
    if (!t) return;
    try {
      await navigator.clipboard.writeText(t);
      setNotice("Testo copiato negli appunti ✓");
    } catch {
      setNotice("Impossibile copiare negli appunti");
    }
  }
  /** The live PDF text selection, if any — read at right-click time for the radial.
   *  Trusts the browser's live selection (right-click preserves it) so a stale
   *  `pending` from an earlier selection can never be copied; `pending` is only a
   *  fallback when the live range is unavailable but still non-collapsed. */
  function currentSelection(): string {
    const live = window.getSelection();
    if (live && !live.isCollapsed && live.rangeCount > 0) return live.toString().trim();
    return "";
  }
  async function doExportTable(fmt: "csv" | "md" | "xlsx") {
    const path = await save({
      defaultPath: `tabella.${fmt}`,
      filters: [{ name: fmt.toUpperCase(), extensions: [fmt] }],
    });
    if (!path) return;
    try {
      await exportTable(tableGrid, fmt, path);
      setNotice("Tabella esportata ✓");
    } catch (e) {
      error = "Errore export tabella: " + e;
    }
  }
  async function doCleanTable() {
    const req = ++tableReq; // claim this epoch so a re-extract mid-clean isn't clobbered
    aiCleaning = true;
    try {
      const cleaned = await aiCleanTable(tableGrid);
      if (tableReq !== req) return; // a newer extraction superseded this clean
      tableGrid = cleaned;
      setNotice("Tabella rifinita con AI");
    } catch (e) {
      if (tableReq !== req) return;
      setNotice("AI non disponibile: " + e);
    } finally {
      aiCleaning = false;
    }
  }
  // ----- LaTeX from a selection / extracted table / extracted text -----
  async function copyLatex(latex: string) {
    if (!latex.trim()) {
      setNotice("Niente da convertire in LaTeX");
      return;
    }
    try {
      await navigator.clipboard.writeText(latex);
      setNotice("LaTeX copiato negli appunti ✓");
    } catch {
      setNotice("Impossibile copiare negli appunti");
    }
  }
  /** Recognize the currently-cropped formula image (in the modal) as LaTeX.
   *  `multi` segments several stacked equations into one aligned block. */
  /** Lazily fetch the provider's model list for the "Ollama" engine and pick a
   *  sensible default vision model (a name that looks vision-capable). */
  async function ensureVisionModels() {
    if (!aiEnabled || visionModels.length || visionModelsLoading) return;
    visionModelsLoading = true;
    try {
      if (!aiSettings) aiSettings = await getAiSettings();
      const url = aiSettings.provider === "lmstudio" ? aiSettings.lmstudio_url : aiSettings.ollama_url;
      const models = await aiListModels(aiSettings.provider as "ollama" | "lmstudio", url);
      visionModels = models;
      if (!visionModel || !models.includes(visionModel)) {
        // Only auto-select something that looks vision-capable. Never silently
        // fall back to a text or embedding model — leave the picker empty so the
        // run shows a clear "select a vision model" error while the dropdown
        // still lists every model for a manual pick.
        visionModel = models.find(isVisionModel) ?? "";
      }
    } catch {
      /* leave the list empty; the run surfaces a clear error */
    } finally {
      visionModelsLoading = false;
    }
  }
  function pickVisionModel(m: string, kind: "formula" | "table" | "text") {
    visionModel = m;
    try {
      localStorage.setItem("scriptorium-vision-model", m);
    } catch {
      /* ignore */
    }
    if (kind === "formula") runFormula(formulaMulti, "ollama");
    else if (kind === "table") runTable("ollama");
    else runTextExtract("ollama");
  }
  async function selectFormulaEngine(engine: "local" | "ollama") {
    formulaEngine = engine; // highlight the picked toggle right away
    if (engine === "ollama") {
      formulaLoading = true; // show progress while the model list loads
      const req = formulaReq; // a competing selection bumps this via runFormula
      await ensureVisionModels();
      if (formulaReq !== req) return; // a newer engine choice superseded us
    }
    runFormula(formulaMulti, engine);
  }

  async function runFormula(multi: boolean, engine: "local" | "ollama" = formulaEngine) {
    if (!formulaImg) return;
    const req = ++formulaReq; // this run's epoch; a later run invalidates it
    formulaMulti = multi;
    formulaEngine = engine;
    formulaLoading = true;
    formulaLatex = "";
    formulaError = "";
    formulaEmpty = false;
    formulaPreviewHtml = "";
    formulaInvalid = "";
    formulaReranked = false;
    // Cancel any scheduled preview from the previous formula.
    clearTimeout(formulaPrevTimer);
    try {
      let out: string;
      let reranked = false; // committed to formulaReranked only after the epoch guard below
      if (engine === "ollama") {
        if (!visionModel) throw "Nessun modello di visione disponibile — abilita l'AI e scarica un modello vision (es. qwen2.5vl, minicpm-v).";
        out = await formulaToLatexAi(formulaImg, visionModel, multi);
      } else {
        const st = await mathocrStatus();
        if (formulaReq !== req) return; // superseded while awaiting
        if (!st.ready) {
          setNotice(`Primo uso: scarico il modello formula (~${st.downloadMb} MB), attendi…`);
        }
        // The local engine returns several beam hypotheses, best-first. Keep the model's
        // top guess unless it doesn't render, then take the first candidate KaTeX accepts
        // (never downgrade an already-valid best). Broken OCR output is thus auto-corrected.
        const cands = (await formulaToLatex(formulaImg, multi)).filter((c) => c.trim());
        if (formulaReq !== req) return; // superseded during recognition — skip rerank + shared writes
        const best = cands[0] ?? "";
        out = cands.find((c) => isMathValid(c)) ?? best;
        reranked = !!out && out !== best; // note when we swapped away from the top guess
      }
      if (formulaReq !== req) return;
      formulaLatex = out;
      formulaReranked = reranked;
      formulaEmpty = !out.trim(); // outcome of the RUN, not of later hand-edits
      renderFormulaPreview(); // show the rendered result right away
    } catch (err) {
      if (formulaReq !== req) return;
      formulaError = String(err);
      setNotice("Errore riconoscimento formula: " + err);
    } finally {
      if (formulaReq === req) formulaLoading = false;
    }
  }

  async function selectTableEngine(engine: "native" | "ollama") {
    tableEngine = engine; // highlight the picked toggle right away
    if (engine === "ollama") {
      tableLoading = true; // show progress while the model list loads
      const req = tableReq; // a competing selection bumps this via runTable
      await ensureVisionModels();
      if (tableReq !== req) return; // a newer engine choice superseded us
    }
    runTable(engine);
  }
  /** (Re)build the extracted-table grid with the chosen engine. */
  async function runTable(engine: "native" | "ollama" = tableEngine) {
    const req = ++tableReq;
    tableEngine = engine;
    tableLoading = true;
    tableGrid = [];
    tableView = "grid";
    tableError = "";
    try {
      let grid: string[][];
      if (engine === "ollama") {
        if (!tableImg) throw "Immagine della tabella non disponibile";
        if (!visionModel) throw "Nessun modello di visione disponibile — abilita l'AI e scarica un modello vision (es. qwen2.5vl, minicpm-v).";
        grid = await tableFromImageAi(tableImg, visionModel);
      } else {
        if (!tableRegion) throw "Regione non disponibile";
        grid = await extractTable(id, tablePage, tableRegion);
      }
      if (tableReq !== req) return;
      tableGrid = grid;
    } catch (err) {
      if (tableReq !== req) return;
      tableError = String(err);
    } finally {
      if (tableReq === req) tableLoading = false;
    }
  }

  function onMouseUp(e: MouseEvent) {
    if (dragStart) {
      onTableUp();
      return;
    }
    if (noteMode) {
      placeNoteAt(e);
      return;
    }
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
      selBtn = null;
      pending = null;
      return;
    }
    const range = sel.getRangeAt(0);
    const clientRects = Array.from(range.getClientRects());
    // Bucket each rect into the page that actually contains it (selections can
    // span multiple stacked pages), normalized against that page's box.
    const byPage = new Map<number, number[][]>();
    for (const r of clientRects) {
      if (r.width < 1 || r.height < 1) continue;
      const cx = r.left + r.width / 2;
      const cy = r.top + r.height / 2;
      let wrap: HTMLDivElement | undefined;
      for (const w of pageWraps) {
        if (!w) continue;
        const b = w.getBoundingClientRect();
        if (cx >= b.left && cx <= b.right && cy >= b.top && cy <= b.bottom) {
          wrap = w;
          break;
        }
      }
      if (!wrap) continue;
      const page = Number(wrap.dataset.page);
      const b = wrap.getBoundingClientRect();
      const norm = [
        (r.left - b.left) / b.width,
        (r.top - b.top) / b.height,
        r.width / b.width,
        r.height / b.height,
      ];
      // Store in the unrotated (rotation-0) frame so highlights stay correct at any rotation.
      const stored = rotateRect(norm, (360 - rotation) % 360);
      if (!byPage.has(page)) byPage.set(page, []);
      byPage.get(page)!.push(stored);
    }
    if (byPage.size === 0) {
      selBtn = null;
      pending = null;
      return;
    }
    pending = { byPage, quote: sel.toString() };
    const last = clientRects[clientRects.length - 1];
    selBtn = { x: last.right, y: last.bottom };
  }

  async function saveHighlight(color: string = HL_COLOR, kind: AnnotationKind = hlKind) {
    if (!pending) return;
    const p = pending;
    try {
      for (const [page, rects] of p.byPage) {
        await addAnnotation({
          documentId: id,
          page,
          kind,
          color,
          rectsJson: JSON.stringify(rects),
          quote: p.quote || null,
          note: null,
        });
      }
      annos = await listAnnotations(id);
      for (const page of p.byPage.keys()) drawHighlights(page);
    } catch (e) {
      error = "Errore salvataggio: " + e;
    } finally {
      clearSelection();
    }
  }

  /** Send the current selection (with its first page) to a note, via +page. */
  function sendSelToNote(e: MouseEvent) {
    if (!pending) return;
    const quote = pending.quote?.trim() ?? "";
    const page = [...pending.byPage.keys()][0] ?? null;
    const pos = { x: e.clientX, y: e.clientY };
    clearSelection();
    if (quote) onSendToNote?.(quote, page, pos);
  }

  async function saveNote() {
    if (!popover) return;
    const pid = popover.id;
    try {
      await updateAnnotationNote(pid, popover.note || null);
      annos = await listAnnotations(id);
      const a = annos.find((x) => x.id === pid);
      if (a) drawHighlights(a.page);
    } catch (e) {
      error = "Errore nota: " + e;
    } finally {
      popover = null;
    }
  }

  async function removeHighlight() {
    if (!popover) return;
    const a = annos.find((x) => x.id === popover!.id);
    const pid = popover.id;
    try {
      await deleteAnnotation(pid);
      annos = await listAnnotations(id);
      if (a) drawHighlights(a.page);
    } catch (e) {
      error = "Errore eliminazione: " + e;
    } finally {
      popover = null;
    }
  }

  // ----- Notes (free-text, debounced autosave) -----
  function onNotesInput() {
    notesSaved = false;
    clearTimeout(notesTimer);
    notesTimer = setTimeout(flushNotes, 700);
  }
  async function flushNotes() {
    clearTimeout(notesTimer);
    try {
      await setDocumentNotes(id, docNotes);
      notesSaved = true;
    } catch (e) {
      setNotice("Nota del documento non salvata: " + e);
    }
  }

  // ----- Annotations panel -----
  let annoColors = $derived([...new Set(annos.map((a) => safeColor(a.color)))]);
  let annoList = $derived(
    annos.filter(
      (a) =>
        (annoKindFilter === "all" || a.kind === annoKindFilter) &&
        (annoColorFilter === "all" || safeColor(a.color) === annoColorFilter),
    ),
  );

  function kindLabel(k: AnnotationKind): string {
    return k === "underline"
      ? "Sottolineato"
      : k === "strikethrough"
        ? "Barrato"
        : k === "note"
          ? "Nota"
          : "Evidenziazione";
  }

  async function jumpToAnno(a: Annotation) {
    scrollToPage(a.page);
    await tick();
    const wrap = pageWraps[a.page - 1];
    const els = wrap?.querySelectorAll<HTMLElement>(`[data-aid="${a.id}"]`);
    els?.forEach((el) => {
      el.classList.add("aflash");
      setTimeout(() => el.classList.remove("aflash"), 1400);
    });
  }

  function startEditAnno(a: Annotation) {
    editingAnno = a.id;
    editingNote = a.note ?? "";
  }
  async function saveEditAnno(a: Annotation) {
    try {
      await updateAnnotationNote(a.id, editingNote.trim() || null);
      annos = await listAnnotations(id);
      drawHighlights(a.page);
    } catch (e) {
      setNotice("Nota non salvata: " + e);
    } finally {
      editingAnno = null;
    }
  }
  async function deleteAnno(a: Annotation) {
    try {
      await deleteAnnotation(a.id);
      annos = await listAnnotations(id);
      drawHighlights(a.page);
    } catch (e) {
      setNotice("Eliminazione non riuscita: " + e);
    }
  }

  /** Render the document's annotations (+ notes) as a Markdown document, page-ordered. */
  function buildAnnoMarkdown(): string {
    const lines: string[] = [`# ${title} — evidenziazioni\n`];
    if (docNotes.trim()) lines.push(`## Nota del documento\n\n${docNotes.trim()}\n`);
    lines.push(`## Annotazioni\n`);
    for (const a of [...annos].sort((x, y) => x.page - y.page || x.id - y.id)) {
      const q = a.quote?.trim();
      const n = a.note?.trim();
      const tag = a.kind !== "highlight" ? ` _(${kindLabel(a.kind).toLowerCase()})_` : "";
      if (q && n) lines.push(`- **p.${a.page}**${tag} «${q}» — ${n}`);
      else if (q) lines.push(`- **p.${a.page}**${tag} «${q}»`);
      else if (n) lines.push(`- **p.${a.page}**${tag} ${n}`);
    }
    return lines.join("\n") + "\n";
  }
  async function copyAnnoMarkdown() {
    try {
      await navigator.clipboard.writeText(buildAnnoMarkdown());
      setNotice("Annotazioni copiate in Markdown ✓");
    } catch {
      setNotice("Impossibile copiare negli appunti");
    }
  }
  async function exportAnnoMarkdown() {
    const path = await save({
      defaultPath: `${title.replace(/[^\w.-]+/g, "_").slice(0, 80) || "annotazioni"}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    try {
      await writeTextFile(path, buildAnnoMarkdown());
      setNotice("Annotazioni esportate ✓");
    } catch (e) {
      setNotice("Errore export: " + e);
    }
  }

  function zoom(delta: number) {
    scale = Math.min(4, Math.max(0.4, +(scale + delta).toFixed(2)));
    saveZoomSoon();
    clearSelection();
    popover = null;
    // Debounce the (raster) re-render so wheel-zoom stays smooth.
    clearTimeout(renderTimer);
    renderTimer = setTimeout(renderPages, 90);
  }

  function rotate(delta: number) {
    rotation = (rotation + delta + 360) % 360;
    clearSelection();
    popover = null;
    renderPages();
  }

  /** Set an absolute zoom and re-render (used by the fit presets / reset). */
  function setScale(s: number) {
    scale = Math.min(4, Math.max(0.4, +s.toFixed(2)));
    saveZoomSoon();
    clearSelection();
    popover = null;
    clearTimeout(renderTimer);
    renderTimer = setTimeout(renderPages, 30);
  }
  /** Natural (scale-1) size of page 1 in the current rotation. */
  async function pageSize(): Promise<{ w: number; h: number } | null> {
    if (!pdf) return null;
    const vp = (await pdf.getPage(1)).getViewport({ scale: 1, rotation });
    return { w: vp.width, h: vp.height };
  }
  async function fitWidth() {
    const s = await pageSize();
    if (!s || !pagesEl) return;
    const avail = pagesEl.clientWidth - 48 - (spread ? 14 : 0);
    setScale((spread ? avail / 2 : avail) / s.w);
  }
  async function fitPage() {
    const s = await pageSize();
    if (!s || !pagesEl) return;
    const availW = (pagesEl.clientWidth - 48 - (spread ? 14 : 0)) / (spread ? 2 : 1);
    const availH = pagesEl.clientHeight - 48;
    setScale(Math.min(availW / s.w, availH / s.h));
  }
  function toggleSpread() {
    spread = !spread;
    // Re-fit so two pages actually sit side by side (or one fills the width).
    fitWidth();
  }
  function toggleNote() {
    noteMode = !noteMode;
    if (noteMode) {
      tableMode = false;
      textMode = false;
      formulaMode = false;
      figureMode = false;
      clearSelection();
      setNotice("Modalità nota: clicca un punto della pagina per aggiungere un appunto");
    }
  }

  /** Drop a sticky note at the clicked point (only while note mode is on). */
  async function placeNoteAt(e: MouseEvent) {
    if (!noteMode) return;
    const cx = e.clientX;
    const cy = e.clientY;
    let wrap: HTMLDivElement | undefined;
    for (const w of pageWraps) {
      if (!w) continue;
      const b = w.getBoundingClientRect();
      if (cx >= b.left && cx <= b.right && cy >= b.top && cy <= b.bottom) {
        wrap = w;
        break;
      }
    }
    if (!wrap) return;
    const page = Number(wrap.dataset.page);
    const b = wrap.getBoundingClientRect();
    // Store the point in the unrotated frame (w=h=0 marks it as a sticky note).
    const stored = rotateRect([(cx - b.left) / b.width, (cy - b.top) / b.height, 0, 0], (360 - rotation) % 360);
    noteMode = false;
    try {
      const newId = await addAnnotation({
        documentId: id,
        page,
        color: NOTE_COLOR,
        rectsJson: JSON.stringify([stored]),
        quote: null,
        note: null,
      });
      annos = await listAnnotations(id);
      drawHighlights(page);
      // Open the popover right away to type the note.
      popover = { id: newId, quote: "", note: "", x: cx, y: cy };
    } catch (err) {
      error = "Errore nota: " + err;
    }
  }

  function onWheel(e: WheelEvent) {
    if (!e.ctrlKey) return; // plain wheel keeps scrolling the pages
    e.preventDefault();
    zoom(e.deltaY < 0 ? 0.12 : -0.12);
  }

  async function doReveal() {
    try {
      await revealDocument(id);
    } catch {
      setNotice("Questo documento non ha un file da mostrare");
    }
  }

  async function doPrint() {
    if (printing || loading || error) return;
    printing = true;
    try {
      await printDocument(id);
    } catch (e) {
      error = "Stampa non riuscita: " + e;
    } finally {
      printing = false;
    }
  }

  // ----- find in document -----
  function clearFindLayers() {
    for (const w of pageWraps) {
      const l = w?.querySelector(".findlayer");
      if (l) l.innerHTML = "";
    }
  }

  type PageIndex = {
    text: string;
    nodes: { node: Text; start: number }[];
    seps: { at: number; node: Text; offset: number }[];
  };
  // Per-page text index, cached and invalidated whenever pages re-render (zoom/rotate).
  let findIndex: (PageIndex | null)[] = [];

  /** Build a page's searchable text, inserting a synthetic space between adjacent
   *  runs/lines that pdf.js renders without separating whitespace — so multi-word
   *  and line-wrapped phrases match. Separators map back to the previous node's end. */
  function buildPageIndex(tl: HTMLElement): PageIndex {
    const nodes: { node: Text; start: number }[] = [];
    const seps: { at: number; node: Text; offset: number }[] = [];
    let text = "";
    let prev: Text | null = null;
    const walker = document.createTreeWalker(tl, NodeFilter.SHOW_TEXT);
    let nd: Node | null;
    while ((nd = walker.nextNode())) {
      const t = nd as Text;
      if (!t.data.length) continue;
      if (prev && text.length && !/\s$/.test(text) && !/^\s/.test(t.data)) {
        seps.push({ at: text.length, node: prev, offset: prev.data.length });
        text += " ";
      }
      nodes.push({ node: t, start: text.length });
      text += t.data;
      prev = t;
    }
    return { text, nodes, seps };
  }

  /** Map a haystack offset back to a (text node, local offset), incl. synthetic separators. */
  function locate(idx: PageIndex, offset: number) {
    for (const s of idx.seps) {
      if (s.at === offset) return { node: s.node, offset: s.offset };
    }
    for (const e of idx.nodes) {
      if (offset >= e.start && offset < e.start + e.node.data.length) {
        return { node: e.node, offset: offset - e.start };
      }
    }
    const last = idx.nodes[idx.nodes.length - 1];
    if (last && offset === last.start + last.node.data.length) {
      return { node: last.node, offset: last.node.data.length };
    }
    return null;
  }

  /** Search every rendered text layer; return matches as page + rotation-0 normalized rects. */
  function computeHits(): { page: number; rects: number[][] }[] {
    const q = findQuery.toLowerCase();
    const out: { page: number; rects: number[][] }[] = [];
    findCapped = false;
    if (q.length < 2) return out;
    for (let n = 1; n <= pageWraps.length; n++) {
      const wrap = pageWraps[n - 1];
      const tl = wrap?.querySelector(".textLayer") as HTMLElement | null;
      if (!tl) continue;
      let idx = findIndex[n - 1];
      if (!idx) {
        idx = buildPageIndex(tl);
        findIndex[n - 1] = idx;
      }
      const hay = idx.text.toLowerCase();
      const box = wrap!.getBoundingClientRect();
      let from = 0;
      for (;;) {
        const i = hay.indexOf(q, from);
        if (i < 0) break;
        from = i + q.length;
        const a = locate(idx, i);
        const b = locate(idx, i + q.length);
        if (!a || !b) continue;
        const range = document.createRange();
        try {
          range.setStart(a.node, a.offset);
          range.setEnd(b.node, b.offset);
        } catch {
          continue;
        }
        const rects: number[][] = [];
        for (const r of Array.from(range.getClientRects())) {
          if (r.width < 1 || r.height < 1) continue;
          const norm = [
            (r.left - box.left) / box.width,
            (r.top - box.top) / box.height,
            r.width / box.width,
            r.height / box.height,
          ];
          rects.push(rotateRect(norm, (360 - rotation) % 360));
        }
        if (rects.length) out.push({ page: n, rects });
        if (out.length >= MAX_FIND_HITS) {
          findCapped = true;
          return out;
        }
      }
    }
    return out;
  }

  function drawFind(page: number) {
    const wrap = pageWraps[page - 1];
    const layer = wrap?.querySelector(".findlayer") as HTMLDivElement | null;
    if (!layer) return;
    layer.innerHTML = "";
    findHits.forEach((hit, idx) => {
      if (hit.page !== page) return;
      for (const rect of hit.rects) {
        const [x, y, w, h] = rotateRect(rect, rotation);
        const el = document.createElement("div");
        el.className = idx === findActive ? "fh active" : "fh";
        el.style.left = x * 100 + "%";
        el.style.top = y * 100 + "%";
        el.style.width = w * 100 + "%";
        el.style.height = h * 100 + "%";
        layer.appendChild(el);
      }
    });
  }

  function redrawFind() {
    for (const p of new Set(findHits.map((h) => h.page))) drawFind(p);
  }

  function scrollToActive() {
    const hit = findHits[findActive];
    if (!hit) return;
    const wrap = pageWraps[hit.page - 1];
    const el = wrap?.querySelector(".findlayer .fh.active") as HTMLElement | null;
    if (el) el.scrollIntoView({ behavior: "smooth", block: "center" });
    else scrollToPage(hit.page);
  }

  function runFind() {
    findPending = true;
    clearTimeout(findTimer);
    findTimer = setTimeout(() => {
      clearFindLayers();
      findHits = computeHits();
      findActive = findHits.length ? 0 : -1;
      findPending = false;
      redrawFind();
      scrollToActive();
    }, 180);
  }

  function gotoHit(delta: number) {
    if (!findHits.length) return;
    findActive = (findActive + delta + findHits.length) % findHits.length;
    redrawFind();
    scrollToActive();
  }

  async function openFind() {
    findOpen = true;
    await tick();
    findInput?.focus();
    findInput?.select();
  }

  function closeFind() {
    clearTimeout(findTimer);
    findOpen = false;
    findQuery = "";
    findHits = [];
    findActive = -1;
    findPending = false;
    findCapped = false;
    clearFindLayers();
  }

  // ----- Lente AI: explain / translate / ask on the selected text -----
  type LensTask = "explain" | "translate" | "ask";
  let lens = $state<{
    task: LensTask;
    quote: string; // own copy of the selection — `pending` may clear underneath us
    answer: string;
    busy: boolean;
    asking: boolean; // "ask" mode: inline question input shown, request not sent yet
  } | null>(null);
  let lensPos = $state<{ x: number; top: number | null; bottom: number | null } | null>(null);
  let lensQuestion = $state("");
  let lensAskInput = $state<HTMLInputElement>();
  let lensReq = 0; // monotonic counter: source of per-request correlation ids
  // Id of the in-flight request; the backend echoes it as `req` in every
  // explain-token event, so a stale stream (from a closed or superseded run)
  // can never bleed tokens into the current card.
  let lensActiveReq: string | null = null;
  let unlistenExplain: (() => void) | undefined;

  const LENS_LABEL: Record<LensTask, string> = {
    explain: "Spiega",
    translate: "Traduci",
    ask: "Chiedi",
  };

  function lensSnippet(q: string): string {
    const t = q.replace(/\s+/g, " ").trim();
    return t.length > 140 ? t.slice(0, 140) + "…" : t;
  }

  /** Place the card near the selection button: below it if there's room, else above.
   *  Clamped to the viewport with 12px margins (assuming the 46vh max height).
   *  Called on every run so the card re-anchors to the current selection; with
   *  no selection anchor an already-placed card simply stays where it is. */
  function placeLens() {
    if (!selBtn && lensPos) return;
    const margin = 12;
    const cw = Math.min(430, window.innerWidth * 0.9);
    const maxH = window.innerHeight * 0.46;
    const ax = selBtn?.x ?? window.innerWidth / 2;
    const ay = selBtn?.y ?? window.innerHeight / 2;
    const x = Math.min(Math.max(margin, ax - cw / 2), Math.max(margin, window.innerWidth - cw - margin));
    const spaceBelow = window.innerHeight - ay - margin - 14;
    if (spaceBelow >= Math.min(maxH, 160)) {
      lensPos = { x, top: Math.min(ay + 14, Math.max(margin, window.innerHeight - margin - maxH)), bottom: null };
    } else {
      const bottom = Math.min(
        Math.max(margin, window.innerHeight - ay + 14),
        Math.max(margin, window.innerHeight - margin - maxH),
      );
      lensPos = { x, top: null, bottom };
    }
  }

  /** Close the lens card only — never touches `pending`/`selBtn` or the browser selection. */
  function closeLens() {
    lensActiveReq = null; // in-flight stream + resolution become no-ops
    lens = null;
    lensPos = null;
    lensQuestion = "";
  }

  /** "Chiedi": open the card immediately with the inline question input. */
  async function openLensAsk() {
    if (lens?.busy) return;
    const quote = pending?.quote ?? lens?.quote ?? "";
    if (!quote) return;
    placeLens();
    lensQuestion = "";
    lens = { task: "ask", quote, answer: "", busy: false, asking: true };
    await tick();
    lensAskInput?.focus();
  }

  async function runLens(task: LensTask, question?: string) {
    if (lens?.busy) return;
    // "ask" runs on the quote already captured in the card; the palette actions
    // run on the live selection (falling back to the card's quote if any).
    const quote = (task === "ask" ? (lens?.quote ?? pending?.quote) : (pending?.quote ?? lens?.quote)) ?? "";
    if (!quote) return;
    placeLens(); // re-anchor to the current selection on every run
    const reqId = String(++lensReq);
    lensActiveReq = reqId;
    lens = { task, quote, answer: "", busy: true, asking: false };
    try {
      const full = await aiExplain({ text: quote, task, question: question ?? null, docId: id, req: reqId });
      if (lensActiveReq === reqId && lens) {
        lens.answer = full; // replace the streamed text with the authoritative result
        lens.busy = false;
      }
    } catch (e) {
      if (lensActiveReq === reqId && lens) {
        lens.answer = "⚠ " + (e instanceof Error ? e.message : String(e));
        lens.busy = false;
      }
    }
  }

  function submitLensQuestion() {
    const q = lensQuestion.trim();
    if (!q || lens?.busy) return;
    runLens("ask", q);
  }

  async function copyLensAnswer() {
    if (!lens?.answer) return;
    try {
      await navigator.clipboard.writeText(lens.answer);
      setNotice("Risposta copiata negli appunti ✓");
    } catch {
      setNotice("Impossibile copiare negli appunti");
    }
  }

  /** Append quote + answer to the document notes, via the same autosave path as the textarea. */
  function lensToNotes() {
    if (!lens?.answer) return;
    const quoteSnippet = lensSnippet(lens.quote);
    const answer = lens.answer;
    docNotes = docNotes
      ? docNotes + "\n\n> " + quoteSnippet + "\n" + answer
      : "> " + quoteSnippet + "\n" + answer;
    onNotesInput(); // marks unsaved + debounced flushNotes, exactly like typing
    setNotice("Aggiunto alla Nota del documento");
  }

  // ----- "⋯ Altro": overflow menu for the regrouped toolbar -----
  let moreOpen = $state<{ x: number; y: number } | null>(null);

  function toggleMore(e: MouseEvent) {
    e.stopPropagation(); // the window-level click handler would close it right away
    if (moreOpen) {
      moreOpen = null;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const w = 232; // keep in sync with .morepop width
    moreOpen = { x: Math.max(8, Math.min(r.left, window.innerWidth - w - 8)), y: r.bottom + 6 };
  }
  /** Run an overflow-menu action and close the menu (handlers reused verbatim). */
  function moreDo(fn: () => void) {
    moreOpen = null;
    fn();
  }

  // ----- Auto-hiding chrome (lettura immersiva) -----
  const CHROME_IDLE_MS = 2500;
  let chromeHidden = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;

  // Any engaged surface pins the toolbar: it must never hide underneath the user.
  let chromePinned = $derived(
    findOpen ||
      panel !== "none" ||
      showToc ||
      showHelp ||
      tableModal ||
      textModal ||
      formulaModal ||
      tableMode ||
      textMode ||
      formulaMode ||
      noteMode ||
      popover !== null ||
      lens !== null ||
      selBtn !== null ||
      moreOpen !== null ||
      printing,
  );
  $effect(() => {
    if (chromePinned && chromeHidden) chromeHidden = false;
  });

  function armIdle() {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      if (!chromePinned) chromeHidden = true;
    }, CHROME_IDLE_MS);
  }
  /** Single mousemove listener on the viewer root: any movement reveals the bar;
   *  parking the pointer within 60px of the top edge keeps it visible. */
  function onViewerMove(e: MouseEvent) {
    if (chromeHidden) chromeHidden = false;
    if (e.clientY <= 60) {
      clearTimeout(idleTimer); // near the top edge: never re-hide while parked there
      return;
    }
    armIdle();
  }

  // ----- Right-click radial menu (coerenza con il resto dell'app) -----
  let viewerRadial = $state<{ x: number; y: number; items: RadialItem[] } | null>(null);
  let radialAt = { x: 0, y: 0 }; // pointer at right-click, for a "Manda a nota" picker

  function buildViewerRadial(): RadialItem[] {
    const sel = currentSelection();
    const items: RadialItem[] = [];
    if (sel) {
      const preview = sel.length > 40 ? sel.slice(0, 40) + "…" : sel;
      items.push({ id: "copysel", label: "Copia", hint: `Copia il testo selezionato: “${preview}”`, action: () => copySelection(sel) });
      if (onSendToNote) {
        const selText = sel;
        const page = pending ? [...pending.byPage.keys()][0] ?? null : null;
        const at = { x: radialAt.x, y: radialAt.y };
        items.push({
          id: "tonote",
          label: "Manda agli Appunti",
          hint: "Manda la selezione a un appunto, con citazione al paper",
          action: () => onSendToNote?.(selText, page, at),
        });
      }
      items.push({
        id: "sellatex",
        label: "Copia come LaTeX",
        hint: "Copia la selezione come LaTeX (escape dei caratteri speciali)",
        action: () => copyLatex(escapeLatex(sel)),
      });
    }
    items.push(
      { id: "fitw", label: "Adatta larghezza", hint: "Larghezza pagina = finestra (W)", action: fitWidth },
      { id: "fith", label: "Adatta pagina", hint: "Pagina intera nella finestra (H)", action: fitPage },
      { id: "spread", label: "Due pagine", checked: spread, hint: "Vista a due pagine (2)", action: toggleSpread },
      { id: "night", label: "Notte", checked: night, hint: "Inverti i colori (I)", action: () => (night = !night) },
      { id: "find", label: "Cerca", hint: "Cerca nel documento (Ctrl+F)", action: () => { openFind(); } },
      { id: "toc", label: "Indice", disabled: !outline.length, checked: showToc, hint: "Indice del documento", action: () => (showToc = !showToc) },
      { id: "annos", label: "Annotazioni", checked: panel === "annos", hint: "Pannello delle annotazioni (A)", action: () => (panel = panel === "annos" ? "none" : "annos") },
      { id: "notes", label: "Nota del documento", checked: panel === "notes", hint: "Appunto libero su questo documento (E)", action: () => (panel = panel === "notes" ? "none" : "notes") },
      {
        id: "tools",
        label: "Strumenti",
        hint: "Nota puntuale, estrazione, rotazione…",
        children: [
          { id: "note", label: "Nota puntuale", checked: noteMode, action: toggleNote },
          { id: "table", label: "Estrai tabella", checked: tableMode, action: toggleTable },
          { id: "text", label: "Estrai testo", checked: textMode, action: toggleText },
          { id: "formula", label: "Formula → LaTeX", checked: formulaMode, action: toggleFormula },
          { id: "figure", label: "Estrai figura", checked: figureMode, action: toggleFigure },
          { id: "rotl", label: "Ruota sx", action: () => rotate(-90) },
          { id: "rotr", label: "Ruota dx", action: () => rotate(90) },
          { id: "reveal", label: "Posizione", action: doReveal },
          { id: "print", label: "Stampa", action: doPrint },
        ],
      },
      { id: "close", label: "Chiudi lettore", danger: true, hint: "Torna alla libreria (Esc)", action: onClose },
    );
    if (onOpenNotes) {
      // Jump to the standalone Appunti (.md) surface, leaving the reader.
      items.splice(items.length - 1, 0, {
        id: "gotonotes",
        label: "Vai agli Appunti",
        hint: "Chiudi il lettore e apri gli Appunti (.md)",
        action: () => onOpenNotes?.(),
      });
    }
    return items;
  }

  function onViewerContext(e: MouseEvent) {
    if (viewerRadial) return; // the open radial owns its own right-clicks
    // Drag-to-extract and note placement keep the default flow untouched.
    if (tableMode || textMode || formulaMode || figureMode || noteMode) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest(".hlpalette, .lenscard, .popover, .findbar, .morepop, .sharemenu, input, textarea")) return;
    e.preventDefault();
    radialAt = { x: e.clientX, y: e.clientY }; // where a "Manda a nota" picker should open
    viewerRadial = { x: e.clientX, y: e.clientY, items: buildViewerRadial() };
  }

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      openFind();
      return;
    }
    if (e.key === "Escape") {
      // Already consumed by a focused inner surface (the radial handles its own
      // Esc on its element and preventDefaults it before this window listener).
      if (e.defaultPrevented) return;
      if (viewerRadial) viewerRadial = null;
      else if (moreOpen) moreOpen = null;
      else if (showHelp) showHelp = false;
      else if (tableModal) tableModal = false;
      else if (textModal) textModal = false;
      else if (formulaModal) formulaModal = false;
      else if (figureModal) figureModal = false;
      else if (tableMode) { tableMode = false; cancelExtractDrag(); }
      else if (textMode) { textMode = false; cancelExtractDrag(); }
      else if (formulaMode) { formulaMode = false; cancelExtractDrag(); }
      else if (figureMode) { figureMode = false; cancelExtractDrag(); }
      else if (noteMode) noteMode = false;
      else if (lens) closeLens();
      else if (findOpen) closeFind();
      else if (popover) popover = null;
      else onClose();
      return;
    }
    // Single-key shortcuts: skip while typing or with modifiers held.
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.defaultPrevented || viewerRadial) return; // the radial owns plain keys (filter mode)
    switch (e.key) {
      case "+":
      case "=": zoom(0.2); break;
      case "-":
      case "_": zoom(-0.2); break;
      case "0": setScale(1); break;
      case "w":
      case "W": fitWidth(); break;
      case "h":
      case "H": fitPage(); break;
      case "n":
      case "N": toggleNote(); break;
      case "2": toggleSpread(); break;
      case "i":
      case "I": night = !night; break;
      case "[": rotate(-90); break;
      case "]": rotate(90); break;
      case "a":
      case "A": panel = panel === "annos" ? "none" : "annos"; break;
      case "e":
      case "E": panel = panel === "notes" ? "none" : "notes"; break;
      // Extraction modes: arm the drag-selection (press again to cancel). Not
      // while an extraction window is already open — Esc closes it first.
      case "t":
      case "T": if (tableModal || textModal || formulaModal || figureModal) return; toggleTable(); break;
      case "x":
      case "X": if (tableModal || textModal || formulaModal || figureModal) return; toggleText(); break;
      case "f":
      case "F": if (tableModal || textModal || formulaModal || figureModal) return; toggleFormula(); break;
      case "g":
      case "G": if (tableModal || textModal || formulaModal || figureModal) return; toggleFigure(); break;
      case "?": showHelp = !showHelp; break;
      default: return;
    }
    e.preventDefault();
  }

  onMount(() => {
    load();
    // Remember the last-picked vision model; learn whether AI is even enabled.
    try {
      visionModel = localStorage.getItem("scriptorium-vision-model") ?? "";
    } catch {
      /* localStorage unavailable */
    }
    getAiSettings()
      .then((s) => {
        aiSettings = s;
      })
      .catch(() => {});
    // Non-passive so Ctrl+wheel zoom can preventDefault the page scroll/zoom.
    pagesEl?.addEventListener("wheel", onWheel, { passive: false });
    // Immersive chrome + right-click radial: single listeners on the viewer root.
    overlayEl?.addEventListener("mousemove", onViewerMove);
    overlayEl?.addEventListener("contextmenu", onViewerContext);
    armIdle();
    // Lente AI: stream tokens into the open card while a request is in flight.
    // Each event echoes the request id, so stale streams are dropped on the floor.
    let unmounted = false;
    listen<{ token: string; req: string | null }>("explain-token", (e) => {
      if (lens?.busy && lensActiveReq !== null && e.payload.req === lensActiveReq) {
        lens.answer += e.payload.token;
      }
    }).then((un) => {
      if (unmounted) un();
      else unlistenExplain = un;
    });
    return () => {
      unmounted = true;
      pagesEl?.removeEventListener("wheel", onWheel);
      overlayEl?.removeEventListener("mousemove", onViewerMove);
      overlayEl?.removeEventListener("contextmenu", onViewerContext);
      unlistenExplain?.();
      clearTimeout(noticeTimer);
      clearTimeout(findTimer);
      clearTimeout(notesTimer);
      clearTimeout(idleTimer);
      clearTimeout(zoomSaveTimer);
      if (zoomDirty) writeZoomNow(); // don't lose a zoom made within the debounce window
      if (!notesSaved) flushNotes();
      if (currentPage > 0) setLastPage(id, currentPage, pdf?.numPages ?? undefined).catch(() => {});
    };
  });
</script>

<svelte:window onkeydown={onKey} onclick={() => (moreOpen = null)} />

<div class="overlay" bind:this={overlayEl}>
  <div class="bar" class:hidden={chromeHidden}>
    <span class="title" title={title}>{title}</span>
    <span class="acount">{annos.length} evidenziazioni</span>
    <div class="ctrl">
      <button onclick={() => zoom(-0.2)} title="Riduci (−)">−</button>
      <span class="pct">{Math.round(scale * 100)}%</span>
      <button onclick={() => zoom(0.2)} title="Ingrandisci (+, oppure Ctrl + rotella)">+</button>
      <span class="tsep"></span>
      <button onclick={fitWidth} title="Adatta alla larghezza (W)">↔</button>
      <button onclick={fitPage} title="Adatta alla pagina (H)">⤢</button>
      <span class="tsep"></span>
      <button class:active={findOpen} onclick={openFind} title="Cerca nel documento (Ctrl+F)">Cerca</button>
      {#if outline.length}
        <button class:active={showToc} onclick={() => (showToc = !showToc)} title="Mostra/nascondi l'indice del documento">Indice</button>
      {/if}
      <button class:active={panel === "annos"} onclick={() => (panel = panel === "annos" ? "none" : "annos")} title="Indice delle annotazioni di questo documento (A)">Annotazioni</button>
      <button class:active={panel === "notes"} onclick={() => (panel = panel === "notes" ? "none" : "notes")} title="Nota del documento: un appunto libero su questo paper (E)">Nota doc{#if !notesSaved}<span class="dot" aria-label="non salvate">•</span>{/if}</button>
      <span class="tsep"></span>
      <button class:active={!!moreOpen} onclick={toggleMore} title="Altri strumenti: rotazione, estrazione, stampa, condivisione…">⋯ Altro</button>
      <span class="tsep"></span>
      <button class="close" onclick={onClose} title="Chiudi il lettore (Esc)">Chiudi</button>
    </div>
  </div>

  {#if moreOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
    <div class="morepop" style="left:{moreOpen.x}px; top:{moreOpen.y}px" role="menu" tabindex="-1" aria-label="Altri strumenti" onclick={(e) => e.stopPropagation()}>
      <button class="mitem" onclick={() => moreDo(() => rotate(-90))} title="Ruota a sinistra 90° ([)"><span class="mck"></span>⟲ Ruota sinistra</button>
      <button class="mitem" onclick={() => moreDo(() => rotate(90))} title="Ruota a destra 90° (])"><span class="mck"></span>⟳ Ruota destra</button>
      <div class="msep"></div>
      <button class="mitem" onclick={() => moreDo(toggleSpread)} title="Vista a due pagine (2)"><span class="mck">{spread ? "✓" : ""}</span>▥ Due pagine</button>
      <button class="mitem" onclick={() => moreDo(toggleNote)} title="Aggiungi una nota a un punto della pagina (N)"><span class="mck">{noteMode ? "✓" : ""}</span>Nota puntuale</button>
      <button class="mitem" onclick={() => moreDo(() => (night = !night))} title="Modalità notte: inverti i colori (I)"><span class="mck">{night ? "✓" : ""}</span>Notte</button>
      <button class="mitem" onclick={() => moreDo(toggleTable)} title="Estrai una tabella: trascina un rettangolo attorno alla tabella (scorciatoia: T)"><span class="mck">{tableMode ? "✓" : ""}</span>Estrai tabella <span class="mkey">T</span></button>
      <button class="mitem" onclick={() => moreDo(toggleText)} title="Estrai testo: trascina un rettangolo attorno al testo (scorciatoia: X)"><span class="mck">{textMode ? "✓" : ""}</span>Estrai testo <span class="mkey">X</span></button>
      <button class="mitem" onclick={() => moreDo(toggleFormula)} title="Riconosci una formula come LaTeX: trascina un rettangolo attorno all'equazione (scorciatoia: F)"><span class="mck">{formulaMode ? "✓" : ""}</span>Formula → LaTeX <span class="mkey">F</span></button>
      <button class="mitem" onclick={() => moreDo(toggleFigure)} title="Ritaglia una figura come immagine PNG: trascina un rettangolo attorno alla figura (scorciatoia: G)"><span class="mck">{figureMode ? "✓" : ""}</span>Estrai figura <span class="mkey">G</span></button>
      <div class="msep"></div>
      <button class="mitem" onclick={() => moreDo(doReveal)} title="Apri la posizione del PDF in Esplora risorse"><span class="mck"></span>Posizione</button>
      <button class="mitem" onclick={() => moreDo(doPrint)} disabled={printing} title="Stampa questo documento"><span class="mck"></span>{printing ? "Stampa…" : "Stampa"}</button>
      <ShareMenu ids={[id]} label={title} {link} variant="menuitem" onstatus={setNotice} onclose={() => (moreOpen = null)} />
      <div class="msep"></div>
      <button class="mitem" onclick={() => moreDo(() => (showHelp = true))} title="Scorciatoie da tastiera (?)"><span class="mck"></span>? Aiuto</button>
    </div>
  {/if}

  {#if findOpen}
    <div class="findbar">
      <span class="findlbl">Cerca nel documento</span>
      <input
        bind:this={findInput}
        bind:value={findQuery}
        oninput={runFind}
        onkeydown={(e) => {
          if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); gotoHit(e.shiftKey ? -1 : 1); }
          else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeFind(); }
        }}
        placeholder="Cerca nel documento…"
      />
      <span class="fcount">
        {findQuery.length < 2
          ? ""
          : findPending
            ? "…"
            : findHits.length
              ? `${findActive + 1}/${findHits.length}${findCapped ? "+" : ""}`
              : "nessun risultato"}
      </span>
      <button onclick={() => gotoHit(-1)} disabled={!findHits.length} title="Precedente (Maiusc+Invio)">↑</button>
      <button onclick={() => gotoHit(1)} disabled={!findHits.length} title="Successivo (Invio)">↓</button>
      <button onclick={closeFind} title="Chiudi (Esc)">✕</button>
    </div>
  {/if}

  <div class="viewbody">
    {#if showToc && outline.length}
      <nav class="toc">
        {#each outline as it, i (i)}
          <button class="tocitem" style="padding-left:{10 + it.depth * 12}px" onclick={() => goTo(it.page)} title={it.title}>{it.title}</button>
        {/each}
      </nav>
    {/if}
    <div
      class="pages"
      class:spread
      class:night
      class:notemode={noteMode}
      class:tablemode={tableMode || textMode || formulaMode || figureMode}
      bind:this={pagesEl}
      onmousedown={onTableDown}
      onmousemove={onTableMove}
      onmouseup={onMouseUp}
      onscroll={onScroll}
      role="document"
    ></div>

    {#if panel !== "none"}
      <aside class="sidepanel">
        <div class="sptabs">
          <button class:on={panel === "annos"} onclick={() => (panel = "annos")}>Annotazioni ({annos.length})</button>
          <button class:on={panel === "notes"} onclick={() => (panel = "notes")}>Nota del documento</button>
          <button class="spclose" onclick={() => (panel = "none")} title="Chiudi pannello" aria-label="Chiudi">✕</button>
        </div>

        {#if panel === "notes"}
          <div class="notespane">
            <textarea
              class="notesarea"
              bind:value={docNotes}
              oninput={onNotesInput}
              onblur={flushNotes}
              placeholder="Appunto libero su questo documento… (salvataggio automatico)"
            ></textarea>
            <div class="notesfoot">{notesSaved ? "Salvato ✓" : "Salvataggio…"}</div>
          </div>
        {:else}
          <div class="annospane">
            {#if annos.length === 0}
              <p class="empty">Nessuna annotazione. Seleziona del testo nel PDF per evidenziarlo, o aggiungi una nota a un punto.</p>
            {:else}
              <div class="annofilters">
                <select bind:value={annoKindFilter} title="Filtra per tipo">
                  <option value="all">Tutti i tipi</option>
                  <option value="highlight">Evidenziazioni</option>
                  <option value="underline">Sottolineati</option>
                  <option value="strikethrough">Barrati</option>
                  <option value="note">Note</option>
                </select>
                {#if annoColors.length > 1}
                  <button class="cdot all" class:on={annoColorFilter === "all"} onclick={() => (annoColorFilter = "all")} title="Tutti i colori" aria-label="Tutti i colori">∗</button>
                  {#each annoColors as c (c)}
                    <button class="cdot" class:on={annoColorFilter === c} onclick={() => (annoColorFilter = c)} style="background:{c}" title="Filtra per colore" aria-label="Filtra colore"></button>
                  {/each}
                {/if}
              </div>
              <div class="annolist">
                {#each annoList as a (a.id)}
                  <div class="annoitem">
                    <button class="annojump" onclick={() => jumpToAnno(a)} title="Vai a pagina {a.page}">
                      <span class="adot" style="background:{safeColor(a.color)}"></span>
                      <span class="apage">p.{a.page}</span>
                      <span class="aquote">{a.quote ?? (a.kind === "note" ? "(nota)" : "(selezione)")}</span>
                    </button>
                    {#if editingAnno === a.id}
                      <div class="annoedit">
                        <textarea bind:value={editingNote} rows="2" placeholder="Nota…"></textarea>
                        <div class="aerow">
                          <button onclick={() => (editingAnno = null)}>Annulla</button>
                          <button class="save" onclick={() => saveEditAnno(a)}>Salva</button>
                        </div>
                      </div>
                    {:else}
                      {#if a.note}<button class="anote" onclick={() => startEditAnno(a)}>{a.note}</button>{/if}
                      <div class="aactions">
                        <button onclick={() => startEditAnno(a)}>{a.note ? "Modifica nota" : "Aggiungi nota"}</button>
                        <button class="del" onclick={() => deleteAnno(a)}>Elimina</button>
                      </div>
                    {/if}
                  </div>
                {/each}
                {#if annoList.length === 0}<p class="empty">Nessuna annotazione con questi filtri.</p>{/if}
              </div>
              <div class="annofoot">
                <button onclick={copyAnnoMarkdown} title="Copia tutte le annotazioni come Markdown">Copia MD</button>
                <button onclick={exportAnnoMarkdown} title="Esporta come file Markdown">Esporta MD</button>
              </div>
            {/if}
          </div>
        {/if}
      </aside>
    {/if}
  </div>
  {#if dragRect}
    <div class="dragrect" style="left:{dragRect.x}px; top:{dragRect.y}px; width:{dragRect.w}px; height:{dragRect.h}px"></div>
  {/if}


  {#if loading}<div class="msg">Caricamento…</div>{/if}
  {#if error}<div class="msg err">{error}</div>{/if}
  {#if notice}<div class="toast">{notice}</div>{/if}

  {#if selBtn}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="hlpalette"
      style="left:{selBtn.x}px; top:{selBtn.y + 8}px"
      onmousedown={(e) => e.preventDefault()}
      role="toolbar"
      tabindex="-1"
      aria-label="Stile e colore evidenziazione"
    >
      <div class="hlkinds">
        {#each KINDS as k (k.kind)}
          <button class="hlk" class:on={hlKind === k.kind} onclick={() => (hlKind = k.kind)} title={k.label} aria-label={k.label}>{k.glyph}</button>
        {/each}
      </div>
      <div class="hlcolors">
        {#each PALETTE as p (p.color)}
          <button class="hlc" style="background:{p.color}" onclick={() => saveHighlight(p.color)} title={p.label} aria-label={p.label}></button>
        {/each}
      </div>
      {#if onSendToNote}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="hlnote" onmousedown={(e) => e.preventDefault()}>
          <button onclick={sendSelToNote} title="Manda la selezione agli Appunti, con citazione a questo paper">→ Appunti</button>
        </div>
      {/if}
      {#if aiEnabled}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="hlai" onmousedown={(e) => e.preventDefault()}>
          <button onclick={() => runLens("explain")} disabled={!!lens?.busy} title="Spiega la selezione con l'AI locale">Spiega</button>
          <button onclick={() => runLens("translate")} disabled={!!lens?.busy} title="Traduci la selezione con l'AI locale">Traduci</button>
          <button onclick={openLensAsk} disabled={!!lens?.busy} title="Fai una domanda sulla selezione">Chiedi</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if lens && lensPos}
    <div
      class="lenscard"
      style={lensPos.top != null
        ? `left:${lensPos.x}px; top:${lensPos.top}px`
        : `left:${lensPos.x}px; bottom:${lensPos.bottom}px`}
      role="dialog"
      tabindex="-1"
      aria-label="Lente AI"
    >
      <div class="lenshd">
        <strong>Lente AI — {LENS_LABEL[lens.task]}</strong>
        {#if lens.busy}<span class="lensdot" aria-hidden="true"></span>{/if}
        <span style="flex:1"></span>
        <button class="lensx" onclick={closeLens} title="Chiudi (Esc)" aria-label="Chiudi">✕</button>
      </div>
      <p class="lensquote">{lensSnippet(lens.quote)}</p>
      {#if lens.asking}
        <div class="lensask">
          <input
            bind:this={lensAskInput}
            bind:value={lensQuestion}
            placeholder="Fai una domanda sul testo selezionato…"
            onkeydown={(e) => {
              if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); submitLensQuestion(); }
              else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeLens(); }
            }}
          />
          <button class="lensgo" onclick={submitLensQuestion} disabled={!lensQuestion.trim()}>Invia</button>
        </div>
      {:else}
        <div class="lensbody">
          {#if lens.answer}{lens.answer}{:else if lens.busy}<span class="lensdots" aria-label="In attesa della risposta"><i></i><i></i><i></i></span>{/if}
        </div>
      {/if}
      <div class="lensft">
        <button onclick={copyLensAnswer} disabled={!lens.answer || lens.busy}>Copia</button>
        <button onclick={lensToNotes} disabled={!lens.answer || lens.busy} title="Aggiungi la risposta alla Nota del documento">→ Nota doc</button>
        <span style="flex:1"></span>
        <button onclick={closeLens}>Chiudi</button>
      </div>
    </div>
  {/if}

  {#if popover}
    <div class="popover" style="left:{popover.x}px; top:{popover.y + 8}px">
      {#if popover.quote}<p class="quote">“{popover.quote}”</p>{/if}
      <textarea bind:value={popover.note} placeholder="Aggiungi una nota…" rows="3"></textarea>
      <div class="prow">
        {#if confirmDel}
          <span class="delask">Eliminare?</span>
          <button class="del" onclick={removeHighlight}>Sì, elimina</button>
          <button onclick={() => (confirmDel = false)}>No</button>
          <span style="flex:1"></span>
        {:else}
          <button class="del" onclick={() => (confirmDel = true)}>Elimina</button>
          <span style="flex:1"></span>
          <button onclick={() => (popover = null)}>Annulla</button>
          <button class="save" onclick={saveNote}>Salva</button>
        {/if}
      </div>
    </div>
  {/if}

  {#if showHelp}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="helpback" onmousedown={(e) => { if (e.target === e.currentTarget) showHelp = false; }} role="presentation">
      <div class="helpcard" role="dialog" tabindex="-1" aria-label="Scorciatoie da tastiera" onclick={(e) => e.stopPropagation()}>
        <h3>Scorciatoie da tastiera</h3>
        <ul class="keys">
          <li><kbd>Ctrl</kbd>+<kbd>F</kbd> <span>Cerca nel documento</span></li>
          <li><kbd>+</kbd> / <kbd>−</kbd> <span>Ingrandisci / riduci</span></li>
          <li><kbd>0</kbd> <span>Zoom 100%</span></li>
          <li><kbd>W</kbd> <span>Adatta alla larghezza</span></li>
          <li><kbd>H</kbd> <span>Adatta alla pagina</span></li>
          <li><kbd>2</kbd> <span>Vista a due pagine</span></li>
          <li><kbd>N</kbd> <span>Aggiungi una nota</span></li>
          <li><kbd>I</kbd> <span>Modalità notte</span></li>
          <li><kbd>[</kbd> / <kbd>]</kbd> <span>Ruota</span></li>
          <li>Selezione testo <span>Lente AI: Spiega / Traduci / Chiedi (con AI locale attiva)</span></li>
          <li><kbd>T</kbd> / <kbd>X</kbd> / <kbd>F</kbd> / <kbd>G</kbd> <span>Estrai tabella / testo / formula / figura: attiva la selezione, poi trascina un riquadro sulla pagina (premi di nuovo per annullare)</span></li>
          <li>Formula → LaTeX <span><kbd>F</kbd>, da ⋯ Altro o dal radiale: trascina attorno a un'equazione. Motore «Locale» (math-OCR integrato, il 1º uso scarica ~180 MB) o «Ollama» (modello di visione). «Più righe» = blocco gathered. Il LaTeX riconosciuto è <strong>modificabile</strong> con <strong>anteprima resa</strong> che si aggiorna mentre correggi; il pulsante <strong>\mathrm&#123;&#125;</strong> avvolge (o toglie) tutta la formula in tondo. Esporta come LaTeX o Markdown ($$…$$): Copia, Salva… o → Appunti</span></li>
          <li>Estrai tabella / testo <span><kbd>T</kbd> / <kbd>X</kbd>. Motore «Nativa» (dal testo del PDF) o «Ollama» (modello di visione — utile per tabelle-immagine e pagine scansionate). Esporta scegliendo il formato: tabella MD/LaTeX/CSV (+ Excel), testo Testo/LaTeX/MD — con Copia, Salva… o → Appunti</span></li>
          <li>Estrai figura <span><kbd>G</kbd>, da ⋯ Altro o dal radiale: trascina attorno a una figura per ritagliarla come immagine PNG. «Salva PNG…» su file, oppure «→ Appunti» per incorporarla in un appunto</span></li>
          <li>Finestre di estrazione <span>Trascinabili dalla barra del titolo (per confrontarle con la pagina sotto) e ridimensionabili dall'angolo</span></li>
          <li>⋯ Altro <span>Rotazione, note, estrazione tabella/testo/formula/figura, stampa, condivisione</span></li>
          <li>Tasto destro <span>Menu radiale con i comandi di lettura</span></li>
          <li>Mouse fermo <span>La barra si nasconde; muovi il mouse per mostrarla</span></li>
          <li><kbd>Esc</kbd> <span>Chiudi / annulla</span></li>
          <li><kbd>?</kbd> <span>Mostra questo aiuto</span></li>
        </ul>
        <button class="save" onclick={() => (showHelp = false)}>Chiudi</button>
      </div>
    </div>
  {/if}

  <!-- Vision-model picker shared by the formula/table/text OCR modals. VLMs are
       grouped and starred so the user is nudged to pick a vision-capable model. -->
  {#snippet vmodelSelect(kind: "formula" | "table" | "text")}
    {#if visionModels.length}
      <select class="vmodel" value={visionModel} onchange={(e) => pickVisionModel(e.currentTarget.value, kind)} title="Modello — ⭐ = adatto alle immagini (VLM)">
        {#if visionGroups.vision.length}
          <optgroup label="⭐ Visione (consigliati)">
            {#each visionGroups.vision as m (m)}<option value={m}>⭐ {m}</option>{/each}
          </optgroup>
        {/if}
        {#if visionGroups.other.length}
          <optgroup label="Altri (no visione)">
            {#each visionGroups.other as m (m)}<option value={m}>{m}</option>{/each}
          </optgroup>
        {/if}
      </select>
    {/if}
  {/snippet}

  {#if tableModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="tableback" onmousedown={(e) => { if (e.target === e.currentTarget) tableModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="tablecard" role="dialog" tabindex="-1" use:dragCard onclick={(e) => e.stopPropagation()}>
        <div class="tablehd">
          <strong>Tabella estratta</strong>
          <div class="viewtoggle" title="Motore di estrazione">
            <button class:on={tableEngine === "native"} onclick={() => selectTableEngine("native")} title="Estrazione nativa dal testo del PDF">Nativa</button>
            <button class:on={tableEngine === "ollama"} disabled={!aiEnabled} onclick={() => selectTableEngine("ollama")} title={aiEnabled ? "Estrai con un modello di visione locale (Ollama / LM Studio)" : "Abilita l'AI locale nelle Impostazioni per usare Ollama"}>Ollama</button>
          </div>
          {#if tableEngine === "ollama"}{@render vmodelSelect("table")}{/if}
          {#if !tableLoading && tableGrid.length}
            <span class="tdim">{tableGrid.length}×{tableGrid[0]?.length ?? 0}</span>
            <div class="viewtoggle">
              <button class:on={tableView === "grid"} onclick={() => (tableView = "grid")} title="Anteprima come griglia">Griglia</button>
              <button class:on={tableView === "latex"} onclick={() => (tableView = "latex")} title="Anteprima del formato scelto">Formato</button>
            </div>
            <div class="viewtoggle" title="Formato di esportazione (Copia / Salva / → Appunti)">
              <button class:on={tableFormat === "md"} onclick={() => (tableFormat = "md")} title="Tabella Markdown">MD</button>
              <button class:on={tableFormat === "latex"} onclick={() => (tableFormat = "latex")} title="Tabella LaTeX in stile booktabs (nel documento serve il pacchetto booktabs)">LaTeX</button>
              <button class:on={tableFormat === "csv"} onclick={() => (tableFormat = "csv")} title="CSV">CSV</button>
            </div>
          {/if}
          <span style="flex:1"></span>
          {#if !tableLoading && tableGrid.length}
            <button onclick={doCleanTable} disabled={aiCleaning} title="Rifinisci righe/colonne con l'AI locale (Ollama/LM Studio)">{aiCleaning ? "AI…" : "Migliora con AI"}</button>
            <button onclick={tableCopy} title="Copia la tabella nel formato scelto">Copia</button>
            <button onclick={tableSave} title="Salva la tabella su file nel formato scelto">Salva…</button>
            <button onclick={() => doExportTable("xlsx")} title="Esporta come foglio Excel (.xlsx)">Excel</button>
            {#if onSendToNote}<button onclick={tableToNote} title="Manda la tabella agli Appunti nel formato scelto (con citazione al paper)">→ Appunti</button>{/if}
          {/if}
          <button class="close" onclick={() => (tableModal = false)}>Chiudi</button>
        </div>
        <div class="tablebody">
          {#if tableLoading}
            <p class="tdim">{tableEngine === "ollama" ? "Estraggo la tabella con il modello di visione…" : "Estraggo la tabella…"}</p>
          {:else if tableError}
            <p class="tdim">{tableError}</p>
          {:else if !tableGrid.length}
            <p class="tdim">Nessun testo tabellare riconosciuto nell'area selezionata. Seleziona più precisamente attorno alla tabella, prova il motore «Ollama», oppure usa un PDF con testo (non scansionato).</p>
          {:else if tableView === "latex"}
            <pre class="latexview">{tableOut()}</pre>
          {:else}
            <table class="extbl">
              <tbody>
                {#each tableGrid as row, ri (ri)}
                  <tr>{#each row as cell, ci (ci)}<td>{cell}</td>{/each}</tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if textModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="tableback" onmousedown={(e) => { if (e.target === e.currentTarget) textModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="tablecard textcard" role="dialog" tabindex="-1" use:dragCard onclick={(e) => e.stopPropagation()}>
        <div class="tablehd">
          <strong>Testo estratto</strong>
          <div class="viewtoggle" title="Motore di estrazione">
            <button class:on={textEngine === "native"} onclick={() => selectTextEngine("native")} title="Estrazione nativa dal testo del PDF">Nativa</button>
            <button class:on={textEngine === "ollama"} disabled={!aiEnabled} onclick={() => selectTextEngine("ollama")} title={aiEnabled ? "OCR con un modello di visione locale (Ollama / LM Studio) — utile per pagine scansionate" : "Abilita l'AI locale nelle Impostazioni per usare Ollama"}>Ollama</button>
          </div>
          {#if textEngine === "ollama"}{@render vmodelSelect("text")}{/if}
          {#if !textLoading && textContent.trim()}
            <div class="viewtoggle" title="Formato di esportazione (Copia / Salva / → Appunti)">
              <button class:on={textFormat === "txt"} onclick={() => (textFormat = "txt")} title="Testo semplice">Testo</button>
              <button class:on={textFormat === "latex"} onclick={() => (textFormat = "latex")} title="LaTeX (caratteri speciali con escape)">LaTeX</button>
              <button class:on={textFormat === "md"} onclick={() => (textFormat = "md")} title="Markdown">MD</button>
            </div>
          {/if}
          <span style="flex:1"></span>
          {#if !textLoading && textContent.trim()}
            <button onclick={textCopy} title="Copia il testo nel formato scelto">Copia</button>
            <button onclick={textSave} title="Salva il testo su file nel formato scelto">Salva…</button>
            {#if onSendToNote}<button onclick={textToNote} title="Manda il testo agli Appunti nel formato scelto (con citazione al paper)">→ Appunti</button>{/if}
          {/if}
          <button class="close" onclick={() => (textModal = false)}>Chiudi</button>
        </div>
        <div class="tablebody">
          {#if textLoading}
            <p class="tdim">{textEngine === "ollama" ? "Riconosco il testo con il modello di visione…" : "Estraggo il testo…"}</p>
          {:else if textError}
            <p class="tdim">{textError}</p>
          {:else if !textContent.trim()}
            <p class="tdim">Nessun testo riconosciuto nell'area selezionata. Se il PDF è scansionato, prova il motore «Ollama» (OCR con un modello di visione).</p>
          {:else if textFormat === "latex"}
            <pre class="latexview">{textToLatex(textContent)}</pre>
          {:else}
            <textarea class="exttext" bind:value={textContent} spellcheck="false"></textarea>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if formulaModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="tableback" onmousedown={(e) => { if (e.target === e.currentTarget) formulaModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="tablecard textcard" role="dialog" tabindex="-1" use:dragCard onclick={(e) => e.stopPropagation()}>
        <div class="tablehd">
          <strong>Formula → LaTeX</strong>
          <div class="viewtoggle" title="Motore di riconoscimento">
            <button class:on={formulaEngine === "local"} onclick={() => selectFormulaEngine("local")} title="Math-OCR locale integrato (pix2tex)">Locale</button>
            <button class:on={formulaEngine === "ollama"} disabled={!aiEnabled} onclick={() => selectFormulaEngine("ollama")} title={aiEnabled ? "Riconosci con un modello di visione locale (Ollama / LM Studio)" : "Abilita l'AI locale nelle Impostazioni per usare Ollama"}>Ollama</button>
          </div>
          {#if formulaEngine === "ollama"}{@render vmodelSelect("formula")}{/if}
          {#if !formulaLoading}
            <div class="viewtoggle">
              <button class:on={!formulaMulti} onclick={() => runFormula(false)} title="Una singola formula">Una</button>
              <button class:on={formulaMulti} onclick={() => runFormula(true)} title="Più equazioni impilate → un blocco gathered">Più righe</button>
            </div>
          {/if}
          {#if !formulaLoading && formulaLatex.trim()}
            <div class="viewtoggle" title="Formato di esportazione (Copia / Salva / → Appunti)">
              <button class:on={formulaFormat === "latex"} onclick={() => (formulaFormat = "latex")} title="LaTeX grezzo">LaTeX</button>
              <button class:on={formulaFormat === "md"} onclick={() => (formulaFormat = "md")} title="Markdown, blocco $$…$$">MD</button>
            </div>
            <button class:on={formulaWrapped} disabled={formulaHasEnv && !formulaWrapped} onclick={toggleMathrmWrap} title={formulaWrapped ? "Togli l'involucro \\mathrm{} (torna al corsivo matematico)" : formulaHasEnv ? "Le formule su più righe non si possono avvolgere in \\mathrm{} (avvolgi le singole righe a mano)" : "Avvolgi tutta la formula in \\mathrm{} (la rende tutta in tondo/dritto)"}>\mathrm&#123;&#125;</button>
          {/if}
          <span style="flex:1"></span>
          {#if !formulaLoading && formulaLatex.trim()}
            <button onclick={formulaCopyFmt} title="Copia la formula nel formato scelto">Copia</button>
            <button onclick={formulaSave} title="Salva la formula su file nel formato scelto">Salva…</button>
            {#if onSendToNote}<button onclick={formulaToNoteFmt} title="Manda la formula agli Appunti nel formato scelto (con citazione al paper)">→ Appunti</button>{/if}
          {/if}
          <button class="close" onclick={() => (formulaModal = false)}>Chiudi</button>
        </div>
        <div class="tablebody">
          {#if formulaImg}
            <img class="formulaimg" src={formulaImg} alt="Formula selezionata" />
          {/if}
          {#if formulaLoading}
            <p class="tdim">{formulaEngine === "ollama" ? "Riconosco la formula con il modello di visione…" : "Riconosco la formula in locale… (il primo uso scarica il modello, ~180 MB)"}</p>
          {:else if formulaError}
            <p class="tdim">{formulaError}</p>
          {:else if formulaEmpty}
            <p class="tdim">Nessuna formula riconosciuta. Riprova selezionando l'area più precisamente attorno all'equazione.</p>
          {:else}
            <div class="formulaedit">
              <div class="fecol">
                <span class="felbl">LaTeX riconosciuto — modificabile</span>
                <textarea class="exttext feta" bind:value={formulaLatex} oninput={scheduleFormulaPreview} spellcheck="false" title="Correggi qui il LaTeX: l'anteprima si aggiorna mentre scrivi"></textarea>
              </div>
              <div class="fecol">
                <span class="felbl">Anteprima resa {formulaFormat === "md" ? "(blocco $$…$$)" : ""}</span>
                <div class="formulaprev">
                  {#if formulaPreviewHtml}
                    <!-- eslint-disable-next-line svelte/no-at-html-tags -- resa da KaTeX (trust:false, nessuno script) -->
                    {@html formulaPreviewHtml}
                  {:else}
                    <p class="tdim">{formulaInvalid ? "Impossibile rendere la formula." : "Anteprima vuota."}</p>
                  {/if}
                </div>
                {#if formulaInvalid}
                  <p class="feinvalid" title={formulaInvalid}>⚠ LaTeX non valido — controlla la formula</p>
                {:else if formulaReranked}
                  <p class="tdim" title="La trascrizione più probabile non era renderizzabile: mostrata un'ipotesi alternativa del riconoscimento. Confronta con l'originale.">↺ Ipotesi alternativa (la prima non era renderizzabile)</p>
                {/if}
              </div>
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if figureModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="tableback" onmousedown={(e) => { if (e.target === e.currentTarget) figureModal = false; }} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="tablecard textcard" role="dialog" tabindex="-1" use:dragCard onclick={(e) => e.stopPropagation()}>
        <div class="tablehd">
          <strong>Figura → PNG</strong>
          <span style="flex:1"></span>
          {#if figureImg}
            <button onclick={saveFigurePng} title="Salva la figura come file PNG">Salva PNG…</button>
            {#if onSendToNote}<button onclick={figureToNote} title="Incorpora la figura in un appunto come immagine Markdown, con citazione al paper">→ Appunti</button>{/if}
          {/if}
          <button class="close" onclick={() => (figureModal = false)}>Chiudi</button>
        </div>
        <div class="tablebody">
          {#if figureError}
            <p class="tdim">{figureError}</p>
          {/if}
          {#if figureImg}
            <img class="figureimg" src={figureImg} alt="Figura ritagliata" />
          {:else}
            <p class="tdim">Nessuna figura ritagliata.</p>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if viewerRadial}
    {#key viewerRadial}
      <RadialMenu
        x={viewerRadial.x}
        y={viewerRadial.y}
        items={viewerRadial.items}
        title={title}
        thumb={null}
        onclose={() => (viewerRadial = null)}
      />
    {/key}
  {/if}
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--viewer-bg);
    z-index: 50;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 18px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    transition: opacity 0.25s ease, transform 0.25s ease;
  }
  /* immersive reading: the chrome fades away while the mouse is idle */
  .bar.hidden {
    opacity: 0;
    transform: translateY(-6px);
    pointer-events: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .bar { transition: opacity 0.25s ease; }
    .bar.hidden { transform: none; }
  }
  .title {
    font-size: 15px;
    font-weight: 600;
    font-family: var(--serif);
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 50%;
  }
  .acount { font-size: 12px; color: var(--faint); }
  .ctrl { margin-left: auto; display: flex; align-items: center; gap: 8px; }
  .ctrl button {
    background: var(--field);
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 5px 11px;
    font-size: 14px;
    cursor: pointer;
  }
  .ctrl button:hover:not(:disabled) { border-color: var(--accent); background: var(--accent-soft); }
  .ctrl button:disabled { opacity: 0.55; cursor: default; }
  .pct { font-size: 12px; color: var(--dim); min-width: 42px; text-align: center; }
  .ctrl button.active { background: var(--accent-soft); border-color: var(--accent); color: var(--accent); }
  .tsep { flex: 0 0 1px; width: 1px; height: 18px; background: var(--border); }
  /* "⋯ Altro" overflow menu */
  .morepop {
    position: fixed; z-index: 70; width: 232px; max-height: 70vh; overflow: auto;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--r-md, 10px); padding: 6px;
    box-shadow: var(--shadow-lg, 0 10px 30px rgba(44, 46, 53, 0.2));
  }
  .mitem {
    display: flex; align-items: center; gap: 6px; width: 100%; text-align: left;
    background: transparent; border: none; color: var(--text);
    padding: 7px 8px; font-size: 13px; cursor: pointer; border-radius: 6px;
  }
  .mitem:hover:not(:disabled) { background: var(--accent-soft); }
  .mitem:disabled { opacity: 0.55; cursor: default; }
  .mck { flex: 0 0 14px; text-align: center; color: var(--accent); font-weight: 700; font-size: 12px; }
  .mkey {
    margin-left: auto; flex: 0 0 auto; color: var(--dim); font-size: 10.5px;
    border: 1px solid var(--border); border-radius: 4px; padding: 0 5px; line-height: 15px;
  }
  .msep { height: 1px; background: var(--border); margin: 5px 4px; }
  .viewbody { flex: 1; display: flex; min-height: 0; }
  .toc {
    width: 250px; flex: 0 0 250px; overflow: auto;
    background: var(--surface); border-right: 1px solid var(--border); padding: 8px 6px;
  }
  .tocitem {
    display: block; width: 100%; text-align: left; background: transparent; border: none;
    color: var(--text); font-size: 12.5px; padding: 5px 8px; border-radius: 6px; cursor: pointer;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .tocitem:hover { background: var(--accent-soft); }
  .pages {
    flex: 1;
    overflow: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 20px;
  }
  /* two-page spread */
  .pages.spread {
    flex-flow: row wrap;
    align-content: flex-start;
    justify-content: center;
  }
  /* sticky-note placement */
  .pages.notemode { cursor: crosshair; }
  .pages.notemode :global(.textLayer) { cursor: crosshair; }
  .pages :global(.notepin) {
    position: absolute;
    transform: translate(-50%, -100%);
    line-height: 0;
    color: var(--accent);
    cursor: pointer;
    z-index: 6;
    user-select: none;
    /* annolayer is pointer-events:none — re-enable clicks on the pin itself */
    pointer-events: auto;
    filter: drop-shadow(0 1px 1.5px rgba(0, 0, 0, 0.3));
  }
  .pages :global(.notepin svg) { display: block; }
  .pages :global(.notepin:hover) { transform: translate(-50%, -100%) scale(1.15); }
  /* night mode: invert the rendered page (text/figures), overlays stay normal */
  .pages.night :global(.pdfcanvas) { filter: invert(1) hue-rotate(180deg); }
  .pages.night :global(.pagewrap) { background: #111; border-color: #333; }
  /* keyboard-shortcuts overlay */
  .helpback {
    position: absolute; inset: 0; z-index: 60;
    background: rgba(0, 0, 0, 0.45);
    display: flex; align-items: center; justify-content: center;
  }
  .helpcard {
    background: var(--surface); color: var(--text);
    border: 1px solid var(--border); border-radius: var(--r-lg, 14px);
    padding: 20px 24px; width: 360px; max-width: 90%;
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(0, 0, 0, 0.3));
    overflow: auto; resize: both; min-width: 300px; min-height: 220px; max-height: 90vh;
  }
  .helpcard h3 { margin: 0 0 12px; font-family: var(--serif); font-size: 17px; }
  .keys { list-style: none; margin: 0 0 8px; padding: 0; }
  .keys li { display: flex; align-items: center; gap: 8px; padding: 4px 0; font-size: 13px; }
  .keys li span { color: var(--dim); margin-left: auto; }
  .keys kbd {
    background: var(--panel); border: 1px solid var(--border); border-radius: 5px;
    padding: 1px 6px; font-family: ui-monospace, monospace; font-size: 12px;
  }
  .helpcard .save {
    background: var(--accent); color: var(--on-accent); border: none;
    border-radius: 7px; padding: 7px 14px; cursor: pointer; float: right;
  }
  /* table extraction: selection rubber-band + cursor + result modal */
  .dragrect {
    position: fixed; z-index: 55; pointer-events: none;
    border: 2px solid var(--accent); background: rgba(43, 74, 120, 0.12);
  }
  .pages.tablemode { cursor: crosshair; }
  .pages.tablemode :global(.textLayer) { cursor: crosshair; }
  .tableback {
    position: fixed; inset: 0; z-index: 60; padding: 24px;
    background: rgba(0, 0, 0, 0.45); display: flex; align-items: center; justify-content: center;
  }
  .tablecard {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-lg, 14px);
    width: 820px; max-width: 95%; max-height: 90vh; display: flex; flex-direction: column;
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(0, 0, 0, 0.3));
    overflow: auto; resize: both; min-width: 420px; min-height: 280px;
  }
  .tablehd { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; row-gap: 8px; padding: 12px 16px; border-bottom: 1px solid var(--border); cursor: move; }
  .tablehd button, .tablehd select { cursor: pointer; }
  .tablehd strong { font-family: var(--serif); font-size: 15px; }
  .tdim { color: var(--dim); font-size: 12px; }
  .tablehd button {
    flex: 0 0 auto; white-space: nowrap;
    background: var(--field); border: 1px solid var(--border); color: var(--accent);
    border-radius: 7px; padding: 5px 11px; font-size: 13px; cursor: pointer;
  }
  .tablehd button:hover:not(:disabled) { background: var(--accent-soft); border-color: var(--accent); }
  .tablehd button:disabled { opacity: 0.55; cursor: default; }
  .tablehd .close { color: var(--text); }
  .tablebody { overflow: auto; padding: 12px 16px; }
  .extbl { border-collapse: collapse; font-size: 12.5px; }
  .extbl td { border: 1px solid var(--border); padding: 4px 8px; vertical-align: top; white-space: pre-wrap; color: var(--text); }
  .extbl tr:first-child td { background: var(--panel); font-weight: 600; }
  .textcard { width: 680px; }
  .exttext {
    width: 100%; min-height: 320px; box-sizing: border-box; resize: vertical;
    background: var(--field); color: var(--text); border: 1px solid var(--border);
    border-radius: 8px; padding: 10px 12px; font-size: 13px; line-height: 1.5; outline: none;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  }
  .exttext:focus { border-color: var(--accent); }
  .formulaimg {
    display: block; max-width: 100%; max-height: 190px; margin: 0 auto 12px;
    padding: 8px; background: #fff; border: 1px solid var(--border); border-radius: 8px;
  }
  /* Formula: editable LaTeX on the left, live KaTeX-rendered preview on the right. */
  .formulaedit { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: stretch; }
  .fecol { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  .felbl { font-size: 11px; color: var(--dim); }
  .formulaedit .exttext.feta { min-height: 180px; flex: 1; resize: none; }
  .formulaprev {
    flex: 1; min-height: 180px; overflow: auto; box-sizing: border-box;
    background: var(--field); border: 1px solid var(--border); border-radius: 8px;
    padding: 12px 14px; display: flex; flex-direction: column; justify-content: center;
  }
  .formulaprev :global(.katex) { font-size: 1.35em; }
  .formulaprev :global(.katex-display) { margin: 0.3em 0; overflow-x: auto; overflow-y: hidden; }
  .formulaprev :global(p) { margin: 0; }
  .feinvalid { font-size: 12px; color: var(--warn, #b45309); margin: 4px 0 0; }
  @media (max-width: 760px) {
    .formulaedit { grid-template-columns: 1fr; }
  }
  .figureimg {
    display: block; max-width: 100%; max-height: 62vh; margin: 0 auto;
    padding: 8px; background: #fff; border: 1px solid var(--border); border-radius: 8px;
  }
  .vmodel {
    flex: 0 1 auto; min-width: 0; max-width: 190px; white-space: nowrap;
    background: var(--field); color: var(--text); border: 1px solid var(--border);
    border-radius: 7px; padding: 4px 8px; font-size: 12px; cursor: pointer;
  }
  .viewtoggle { display: inline-flex; flex: 0 0 auto; border: 1px solid var(--border); border-radius: 7px; overflow: hidden; }
  .viewtoggle button { flex: 0 0 auto; white-space: nowrap; border: none; border-radius: 0; padding: 4px 10px; font-size: 12px; }
  .viewtoggle button.on, .viewtoggle button.on:hover { background: var(--accent); color: var(--on-accent); border-color: transparent; }
  .latexview {
    width: 100%; min-height: 320px; max-height: 62vh; box-sizing: border-box; margin: 0;
    overflow: auto; white-space: pre-wrap; word-break: break-word;
    background: var(--field); color: var(--text); border: 1px solid var(--border);
    border-radius: 8px; padding: 10px 12px; font-size: 13px; line-height: 1.5;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace; user-select: text;
  }

  /* ----- page + canvas ----- */
  .pages :global(.pagewrap) {
    position: relative;
    box-shadow: 0 2px 14px rgba(44, 46, 53, 0.16);
    background: white;
    border: 1px solid var(--border);
    flex: 0 0 auto;
  }
  .pages :global(.pdfcanvas) {
    display: block;
  }

  /* ----- pdf.js text layer (minimal v6 contract, scoped) ----- */
  .pages :global(.textLayer) {
    position: absolute;
    text-align: initial;
    inset: 0;
    overflow: clip;
    opacity: 1;
    line-height: 1;
    text-size-adjust: none;
    forced-color-adjust: none;
    transform-origin: 0 0;
    z-index: 2;
    --min-font-size: 1;
    --text-scale-factor: calc(var(--total-scale-factor) * var(--min-font-size));
    --min-font-size-inv: calc(1 / var(--min-font-size));
  }
  .pages :global(.textLayer :is(span, br)) {
    color: transparent;
    position: absolute;
    white-space: pre;
    cursor: text;
    transform-origin: 0% 0%;
    user-select: text;
  }
  .pages :global(.textLayer > :not(.markedContent)),
  .pages :global(.textLayer .markedContent span:not(.markedContent)) {
    z-index: 1;
    --font-height: 0;
    font-size: calc(var(--text-scale-factor) * var(--font-height));
    --scale-x: 1;
    --rotate: 0deg;
    transform: rotate(var(--rotate)) scaleX(var(--scale-x)) scale(var(--min-font-size-inv));
  }
  .pages :global(.textLayer .markedContent) { display: contents; }
  .pages :global(.textLayer ::selection) { background: rgba(43, 74, 120, 0.28); }

  /* ----- highlight overlay ----- */
  .pages :global(.annolayer) {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 4;
  }
  .pages :global(.annolayer .hl) {
    position: absolute;
    pointer-events: auto;
    cursor: pointer;
    border-radius: 2px;
    mix-blend-mode: multiply;
  }
  .pages :global(.annolayer .hl:hover) { outline: 1px solid #d4a200; }

  /* ----- find-in-document overlay ----- */
  .pages :global(.findlayer) {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 5; /* above the annotation layer so matches aren't occluded */
  }
  .pages :global(.findlayer .fh) {
    position: absolute;
    background: rgba(255, 170, 0, 0.38);
    border-radius: 1px;
  }
  .pages :global(.findlayer .fh.active) {
    background: rgba(255, 130, 0, 0.6);
    outline: 1px solid #e06600;
  }

  /* ---- selection palette (style + colour) ---- */
  .hlpalette {
    position: fixed;
    z-index: 60;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 5px 7px;
    box-shadow: 0 4px 16px rgba(44, 46, 53, 0.28);
  }
  .hlkinds { display: flex; gap: 2px; padding-right: 6px; border-right: 1px solid var(--border); }
  .hlk {
    width: 24px; height: 24px; border: 1px solid var(--border); background: var(--field);
    color: var(--text); border-radius: 6px; font-size: 12px; font-weight: 700; cursor: pointer;
    display: grid; place-items: center;
  }
  .hlk.on { background: var(--accent); color: var(--on-accent); border-color: var(--accent); }
  .hlcolors { display: flex; gap: 5px; }
  .hlc {
    width: 22px; height: 22px; border-radius: 50%; border: 2px solid rgba(0, 0, 0, 0.12);
    cursor: pointer; padding: 0;
  }
  .hlc:hover { transform: scale(1.12); }
  /* AI actions inside the selection palette */
  .hlai { display: flex; gap: 4px; padding-left: 8px; border-left: 1px solid var(--border); }
  .hlai button {
    height: 24px; padding: 0 8px; font-size: 12px; cursor: pointer;
    background: var(--field); color: var(--text);
    border: 1px solid var(--border); border-radius: 6px;
  }
  .hlai button:hover:not(:disabled) { background: var(--accent-soft); border-color: var(--accent); color: var(--accent); }
  .hlai button:disabled { opacity: 0.55; cursor: default; }
  .hlnote { display: flex; padding-left: 8px; border-left: 1px solid var(--border); }
  .hlnote button {
    height: 24px; padding: 0 8px; font-size: 12px; cursor: pointer;
    background: var(--field); color: var(--accent);
    border: 1px solid var(--border); border-radius: 6px; white-space: nowrap;
  }
  .hlnote button:hover { background: var(--accent-soft); border-color: var(--accent); }

  /* ---- Lente AI: floating result card ---- */
  .lenscard {
    position: fixed;
    z-index: 62;
    width: min(430px, 90vw);
    max-height: 46vh;
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(12px);
    border: 1px solid var(--border);
    border-radius: var(--r-lg, 14px);
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(0, 0, 0, 0.3));
    overflow: hidden;
  }
  .lenshd { display: flex; align-items: center; gap: 8px; padding: 9px 12px 8px; border-bottom: 1px solid var(--border); }
  .lenshd strong { font-family: var(--serif); font-size: 12px; font-weight: 600; color: var(--text); }
  .lensdot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--accent);
    animation: lenspulse 1.1s ease-in-out infinite;
  }
  @keyframes lenspulse {
    0%, 100% { opacity: 0.35; transform: scale(0.8); }
    50% { opacity: 1; transform: scale(1); }
  }
  .lensx { background: transparent; border: none; color: var(--dim); font-size: 13px; cursor: pointer; padding: 2px 4px; }
  .lensx:hover { color: var(--text); }
  .lensquote {
    margin: 0; padding: 7px 12px; font-size: 11px; font-style: italic; color: var(--faint);
    border-left: 3px solid var(--accent);
    background: color-mix(in srgb, var(--accent-soft) 60%, transparent);
  }
  .lensask { display: flex; gap: 6px; padding: 9px 12px; border-bottom: 1px solid var(--border); }
  .lensask input {
    flex: 1; background: var(--field); border: 1px solid var(--border); border-radius: var(--r-md, 8px);
    color: var(--text); padding: 6px 9px; font-size: 13px; outline: none;
  }
  .lensask input:focus { border-color: var(--accent); }
  .lensgo {
    background: var(--accent); color: var(--on-accent); border: none; border-radius: var(--r-md, 8px);
    padding: 6px 12px; font-size: 12px; cursor: pointer;
  }
  .lensgo:disabled { opacity: 0.55; cursor: default; }
  .lensbody {
    flex: 1; min-height: 0; overflow: auto; padding: 10px 12px;
    font-size: 13px; line-height: 1.55; white-space: pre-wrap; color: var(--text);
  }
  .lensdots { display: inline-flex; gap: 4px; padding: 3px 0; }
  .lensdots i {
    width: 5px; height: 5px; border-radius: 50%; background: var(--dim);
    animation: lensblink 1s ease-in-out infinite;
  }
  .lensdots i:nth-child(2) { animation-delay: 0.18s; }
  .lensdots i:nth-child(3) { animation-delay: 0.36s; }
  @keyframes lensblink {
    0%, 100% { opacity: 0.25; transform: translateY(0); }
    50% { opacity: 1; transform: translateY(-2px); }
  }
  .lensft { display: flex; align-items: center; gap: 6px; padding: 8px 12px; border-top: 1px solid var(--border); }
  .lensft button {
    background: transparent; color: var(--accent); border: 1px solid var(--border);
    border-radius: 6px; padding: 4px 10px; font-size: 12px; cursor: pointer;
  }
  .lensft button:hover:not(:disabled) { background: var(--accent-soft); border-color: var(--accent); }
  .lensft button:disabled { opacity: 0.5; cursor: default; }

  /* unsaved-notes indicator on the toolbar button */
  .bar .dot { color: var(--accent); margin-left: 3px; font-weight: 700; }

  /* flash an annotation when jumped to from the sidebar */
  .pages :global(.hl.aflash), .pages :global(.notepin.aflash) {
    outline: 2px solid var(--accent); outline-offset: 1px;
    animation: aflash 1.4s ease;
  }
  @keyframes aflash {
    0%, 40% { box-shadow: 0 0 0 4px var(--accent-soft); }
    100% { box-shadow: 0 0 0 0 transparent; }
  }

  /* ---- side panel: notes + annotations index ---- */
  .sidepanel {
    width: 300px; flex: 0 0 300px; overflow: hidden;
    background: var(--surface); border-left: 1px solid var(--border);
    display: flex; flex-direction: column;
  }
  .sptabs { display: flex; gap: 4px; padding: 7px; border-bottom: 1px solid var(--border); }
  .sptabs button {
    flex: 1; background: var(--field); color: var(--text); border: 1px solid var(--border);
    border-radius: 6px; padding: 5px 8px; font-size: 12.5px; cursor: pointer;
  }
  .sptabs button.on { background: var(--accent); color: var(--on-accent); border-color: var(--accent); }
  .sptabs .spclose { flex: 0 0 30px; }

  .notespane { display: flex; flex-direction: column; flex: 1; min-height: 0; padding: 8px; }
  .notesarea {
    flex: 1; width: 100%; box-sizing: border-box; resize: none;
    background: var(--field); border: 1px solid var(--border); border-radius: 8px;
    color: var(--text); padding: 9px; font-size: 13px; line-height: 1.5; outline: none;
  }
  .notesfoot { font-size: 11px; color: var(--faint); padding: 5px 2px 0; text-align: right; }

  .annospane { display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .annofilters {
    display: flex; flex-wrap: wrap; align-items: center; gap: 5px;
    padding: 7px 8px; border-bottom: 1px solid var(--border);
  }
  .annofilters select {
    background: var(--field); color: var(--text); border: 1px solid var(--border);
    border-radius: 6px; padding: 4px 6px; font-size: 12px;
  }
  .cdot {
    width: 18px; height: 18px; border-radius: 50%; border: 2px solid transparent; cursor: pointer;
    padding: 0; color: #fff; font-size: 11px; line-height: 1; display: grid; place-items: center;
  }
  .cdot.on { border-color: var(--text); }
  .cdot.all { background: var(--field); color: var(--dim); }

  .annolist { flex: 1; overflow: auto; padding: 6px; display: flex; flex-direction: column; gap: 6px; }
  .annoitem { border: 1px solid var(--border); border-radius: 8px; padding: 6px 7px; background: var(--field); }
  .annojump {
    display: flex; align-items: baseline; gap: 6px; width: 100%; text-align: left;
    background: transparent; border: none; color: var(--text); cursor: pointer; padding: 0;
  }
  .adot { flex: 0 0 9px; width: 9px; height: 9px; border-radius: 50%; align-self: center; }
  .apage { font-size: 11px; font-weight: 700; color: var(--dim); flex: 0 0 auto; }
  .aquote {
    font-size: 12.5px; color: var(--text);
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .anote {
    display: block; width: 100%; text-align: left; margin: 5px 0 0;
    background: var(--accent-soft); border: none; border-radius: 6px;
    color: var(--text); font-size: 12px; padding: 5px 7px; cursor: pointer; white-space: pre-wrap;
  }
  .aactions { display: flex; gap: 6px; margin-top: 6px; }
  .aactions button, .aerow button {
    background: transparent; color: var(--accent); border: 1px solid var(--border);
    border-radius: 6px; padding: 3px 8px; font-size: 11.5px; cursor: pointer;
  }
  .aactions .del { color: var(--danger); border-color: var(--danger); }
  .annoedit textarea {
    width: 100%; box-sizing: border-box; resize: vertical; margin-top: 5px;
    background: var(--surface); border: 1px solid var(--border); border-radius: 6px;
    color: var(--text); padding: 6px; font-size: 12.5px; outline: none;
  }
  .aerow { display: flex; justify-content: flex-end; gap: 6px; margin-top: 5px; }
  .aerow .save { background: var(--accent); color: var(--on-accent); border: none; }
  .annofoot { display: flex; gap: 6px; padding: 7px 8px; border-top: 1px solid var(--border); }
  .annofoot button {
    flex: 1; background: var(--field); color: var(--accent); border: 1px solid var(--border);
    border-radius: 6px; padding: 5px; font-size: 12px; cursor: pointer;
  }
  .empty { color: var(--faint); font-size: 12.5px; padding: 14px; text-align: center; line-height: 1.5; }
  .popover {
    position: fixed;
    z-index: 60;
    width: 280px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px;
    box-shadow: 0 10px 30px rgba(44, 46, 53, 0.2);
  }
  .popover .quote {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--dim);
    font-style: italic;
    max-height: 60px;
    overflow: auto;
  }
  .popover textarea {
    width: 100%;
    box-sizing: border-box;
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
    padding: 7px;
    font-size: 13px;
    resize: vertical;
    outline: none;
  }
  .popover .prow { display: flex; align-items: center; gap: 8px; margin-top: 9px; }
  .popover .prow button {
    background: var(--field);
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 11px;
    font-size: 13px;
    cursor: pointer;
  }
  .popover .prow .save { background: var(--accent); color: var(--on-accent); border: none; }
  .popover .prow .del { background: transparent; color: var(--danger); border-color: var(--danger); }
  .popover .prow .delask { font-size: 12px; color: var(--danger); align-self: center; margin-right: 2px; }
  .msg {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: var(--dim);
    font-size: 15px;
  }
  .msg.err { color: var(--danger); max-width: 60%; text-align: center; }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 26px;
    transform: translateX(-50%);
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 9px 16px;
    font-size: 13px;
    box-shadow: 0 6px 20px rgba(44, 46, 53, 0.25);
    z-index: 70;
    max-width: 70%;
    text-align: center;
  }
  .findbar {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    padding: 8px 22px;
  }
  .findbar .findlbl {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--dim);
  }
  .findbar input {
    background: var(--field);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
    padding: 5px 9px;
    font-size: 13px;
    outline: none;
    width: 200px;
  }
  .findbar input:focus { border-color: var(--accent); }
  .findbar .fcount { font-size: 12px; color: var(--dim); min-width: 58px; text-align: center; white-space: nowrap; }
  .findbar button {
    background: var(--field);
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 9px;
    font-size: 13px;
    cursor: pointer;
  }
  .findbar button:hover:not(:disabled) { border-color: var(--accent); background: var(--accent-soft); }
  .findbar button:disabled { opacity: 0.45; cursor: default; }
</style>
