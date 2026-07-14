<script lang="ts">
  import { onMount, untrack } from "svelte";

  // ----- Data contract (local structural copy of $lib/api's SimilarityGraph: no coupling) -----
  interface GraphNode {
    id: number;
    title: string | null;
    year: number | null;
    color: string | null;
    degree: number;
    unread: boolean;
    favorite: boolean;
    peer_reviewed: boolean;
    has_github: boolean;
    px: number;
    py: number;
    sx: number | null;
    sy: number | null;
    community: number;
    kind: "doc" | "note";
    slug: string | null;
  }
  interface GraphEdge {
    a: number;
    b: number;
    w: number; // cosine similarity 0..1
  }
  interface ClusterInfo {
    id: number;
    label: string;
    size: number;
  }
  interface SimilarityGraph {
    nodes: GraphNode[];
    edges: GraphEdge[];
    clusters: ClusterInfo[];
    embedded: number;
    total: number;
  }

  /** Extra card details the graph itself doesn't carry, resolved by the parent
   *  from its loaded documents (best-effort: undefined is fine). */
  interface DocExtra {
    authors?: string[];
    venue?: string | null;
    tags?: { name: string; color: string | null }[];
  }

  /** An online discovery ("ghost star"): a paper found around a seed node that is
   *  not (yet) in the library. Owned by the parent; this component only draws it
   *  anchored to its seed and reports clicks/adds. */
  export interface GhostStar {
    key: string;
    seedId: number;
    title: string;
    year: number | null;
    venue: string | null;
    inLibrary: boolean;
    added: boolean;
    /** Ghost this one was discovered FROM (exploration chain); null/absent = anchored to the seed node. */
    parentKey?: string | null;
    /** DOI, when known — enables "Citazioni" on the ghost card. */
    doi?: string | null;
    /** First author, when known — enables "Autore" on the ghost card. */
    author?: string | null;
  }
  type ExploreRelation = "citations" | "similar" | "author";

  let {
    graph,
    loading,
    selected = [],
    onOpen,
    onContext,
    onToggleSelect,
    onGenerate,
    onRefresh,
    resolve,
    params,
    onParams,
    onSavePositions,
    ghosts,
    onExplore,
    onGhostAdd,
    onGhostExplore,
    onGhostsClear,
  }: {
    graph: SimilarityGraph | null;
    loading: boolean;
    selected?: number[];
    onOpen: (id: number) => void;
    onContext: (e: MouseEvent, id: number) => void;
    onToggleSelect: (id: number) => void;
    onGenerate: () => void;
    onRefresh: () => void;
    resolve?: (id: number) => DocExtra | undefined;
    /** Current graph density parameters (k neighbours, similarity floor). */
    params?: { k: number; minSim: number };
    /** Apply new density parameters (triggers a graph rebuild in the parent). */
    onParams?: (k: number, minSim: number) => void;
    /** Persist settled node positions (called when the simulation cools). */
    onSavePositions?: (positions: { id: number; x: number; y: number }[]) => void;
    /** Online discoveries to draw as dashed "ghost stars" around their seeds. */
    ghosts?: GhostStar[];
    /** Fetch online papers related to a node (the parent populates `ghosts`). */
    onExplore?: (id: number, relation: ExploreRelation) => void;
    /** Add a ghost's paper to the library. */
    onGhostAdd?: (key: string) => void;
    /** Explore onward FROM a ghost (snowball chain: new ghosts anchor to it). */
    onGhostExplore?: (key: string, relation: ExploreRelation) => void;
    /** Dismiss all ghost stars. */
    onGhostsClear?: () => void;
  } = $props();

  // ----- Simulation state (plain, non-reactive: the canvas is redrawn manually) -----
  interface SimNode {
    id: number;
    title: string;
    year: number | null;
    color: string | null;
    degree: number;
    unread: boolean;
    favorite: boolean;
    peer: boolean; // peer-reviewed
    gh: boolean; // has a GitHub repo
    community: number; // semantic cluster index (−1 = none)
    kind: "doc" | "note";
    x: number;
    y: number;
    vx: number;
    vy: number;
    r: number; // base radius in world units
  }
  interface SimEdge {
    ai: number;
    bi: number;
    w: number;
    rest: number;
    k: number;
  }

  const TAU = Math.PI * 2;
  const MIN_ALPHA = 0.02;
  const CELL = 120; // uniform-grid cell (world units) for chunked repulsion
  const REPEL = 3200;
  const LABEL_FONT = '11px system-ui, -apple-system, "Segoe UI", Roboto, sans-serif';

  let container: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;

  let vw = 0;
  let vh = 0;
  let dpr = 1;
  let zoom = 1;
  let tx = 0;
  let ty = 0;

  let nodes: SimNode[] = [];
  let edges: SimEdge[] = [];
  let idToIdx = new Map<number, number>();
  let adj = new Map<number, number[]>();
  let adjW = new Map<number, { id: number; w: number }[]>(); // neighbors with similarity, for the info card
  let clusterMeta: ClusterInfo[] = []; // sizeable communities (labels for the nebulae)
  let yearMin = 0;
  let yearMax = 0; // year range for the "Anno" color mode
  let topLabelIds = new Set<number>();
  let fx = new Float32Array(0);
  let fy = new Float32Array(0);
  let grid = new Map<string, number[]>();

  let alpha = 0;
  let repCursor = 0; // 0 = start of a fresh physics pass; >0 = pass paused mid-repulsion
  let hoverIdx = -1;
  let hoverSet: Set<number> | null = null; // ids kept at full alpha while hovering
  let selectedSet = new Set<number>();
  let dragIdx = -1;
  let camAnim: { t0: number; from: [number, number, number]; to: [number, number, number] } | null = null;
  let needFit = false;
  let rafId: number | null = null;
  let ro: ResizeObserver | null = null;
  let mo: MutationObserver | null = null;

  let ptr = { down: false, startX: 0, startY: 0, moved: false, mode: "pan" as "pan" | "node", nodeIdx: -1, panTx: 0, panTy: 0 };

  let tip = $state<{ x: number; y: number; title: string; meta: string } | null>(null);
  let generating = $state(false); // local guard: the "Genera indice" CTA must not double-fire

  // Node color mode: dominant tag (default) · semantic community · year · read state.
  type ColorMode = "tag" | "community" | "year" | "read";
  const COLOR_MODES: { value: ColorMode; label: string }[] = [
    { value: "tag", label: "Tag dominante" },
    { value: "community", label: "Comunità semantiche" },
    { value: "year", label: "Anno" },
    { value: "read", label: "Stato lettura" },
  ];
  let colorMode = $state<ColorMode>(
    (localStorage.getItem("scriptorium-map-color") as ColorMode) || "tag",
  );
  $effect(() => {
    localStorage.setItem("scriptorium-map-color", colorMode);
    schedule();
  });
  // Nebulose delle comunità: aloni + nomi, solo aloni, o niente (persistito).
  type NebulaMode = "full" | "tint" | "off";
  const NEBULA_MODES: { value: NebulaMode; label: string }[] = [
    { value: "full", label: "Nebulose + nomi" },
    { value: "tint", label: "Solo nebulose" },
    { value: "off", label: "Senza nebulose" },
  ];
  let nebulaMode = $state<NebulaMode>(
    (localStorage.getItem("scriptorium-map-nebula") as NebulaMode) || "full",
  );
  $effect(() => {
    localStorage.setItem("scriptorium-map-nebula", nebulaMode);
    schedule();
  });
  /** Stable, well-spread hue per community (golden-angle walk). */
  function commColor(i: number, alpha = 1): string {
    return `hsla(${(i * 137.508) % 360}, 55%, 52%, ${alpha})`;
  }
  const NOTE_FILL = "#8b6fae"; // violet: vault appunti stand apart from papers
  function nodeFill(n: SimNode): string {
    switch (colorMode) {
      case "community":
        return n.community >= 0 ? commColor(n.community) : withAlpha(theme.faint, 0.5);
      case "year": {
        if (n.year === null || yearMax <= yearMin) return withAlpha(theme.faint, 0.5);
        const t = (n.year - yearMin) / (yearMax - yearMin);
        return `hsl(${210 - 175 * t}, 60%, 52%)`; // blu (vecchio) → arancio (recente)
      }
      case "read":
        return n.kind === "note" ? NOTE_FILL : n.unread ? theme.accent : withAlpha(theme.dim, 0.55);
      default:
        return n.kind === "note" ? NOTE_FILL : n.color || paint.nodeFill;
    }
  }

  // Density tuning panel (k / minSim → parent rebuilds the graph).
  let tuneOpen = $state(false);
  let tuneK = $state(4);
  let tuneSim = $state(0.55);
  function toggleTune() {
    if (!tuneOpen) {
      tuneK = params?.k ?? 4;
      tuneSim = params?.minSim ?? 0.55;
    }
    tuneOpen = !tuneOpen;
  }
  function applyTune() {
    tuneOpen = false;
    onParams?.(tuneK, tuneSim);
  }

  // ----- Focused node ("scheda"): single click pins a paper card with its
  //       connections; double click opens the paper. -----
  interface PanelNeighbor {
    id: number;
    title: string;
    year: number | null;
    w: number;
  }
  interface PanelData {
    id: number;
    kind: "doc" | "note";
    title: string;
    year: number | null;
    degree: number;
    unread: boolean;
    favorite: boolean;
    peer: boolean;
    gh: boolean;
    extra: DocExtra | undefined;
    neighbors: PanelNeighbor[];
  }
  let panel = $state<PanelData | null>(null);
  let pinnedSet: Set<number> | null = null; // node + neighbors kept at full alpha while the card is open

  /** Pin (or clear) the info card for a node id; keeps the neighborhood lit. */
  function setFocus(id: number | null) {
    if (id === null || !idToIdx.has(id)) {
      panel = null;
      pinnedSet = null;
      schedule();
      return;
    }
    tuneOpen = false; // the card and the tune panel share the top-right corner
    const n = nodes[idToIdx.get(id)!];
    const neighbors: PanelNeighbor[] = (adjW.get(id) ?? [])
      .map((e) => {
        const i = idToIdx.get(e.id);
        const m = i !== undefined ? nodes[i] : undefined;
        return { id: e.id, title: m?.title || "Senza titolo", year: m?.year ?? null, w: e.w };
      })
      .sort((a, b) => b.w - a.w);
    panel = {
      id,
      kind: n.kind,
      title: n.title || "Senza titolo",
      year: n.year,
      degree: n.degree,
      unread: n.unread,
      favorite: n.favorite,
      peer: n.peer,
      gh: n.gh,
      extra: n.kind === "doc" ? resolve?.(id) : undefined,
      neighbors,
    };
    pinnedSet = new Set<number>([id, ...neighbors.map((x) => x.id)]);
    schedule();
  }
  // ----- Ghost stars: online discoveries anchored around their seed node (or,
  //       in an exploration chain, around the ghost they were discovered from).
  //       While ghosts exist the map is in "exploration mode": they get a tiny
  //       physics of their own (springs to their base + repulsion from each
  //       other AND from library stars) so multi-hop chains never pile up. -----
  let ghostAnchor = new Map<string, { angle: number; dist: number }>();
  let ghostByKey = new Map<string, GhostStar>();
  let ghostHop = new Map<string, number>(); // chain depth (0 = straight from the seed)
  let ghostPos = new Map<string, { x: number; y: number; vx: number; vy: number }>();
  let ghostAlpha = 0; // relaxation heat of the ghost mini-sim
  let ghostCard = $state<GhostStar | null>(null);
  $effect(() => {
    const list = ghosts ?? [];
    const byKey = new Map(list.map((g) => [g.key, g]));
    ghostByKey = byKey;
    // Chain depth per ghost (bounded walk; malformed cycles collapse to 0).
    const hops = new Map<string, number>();
    const hopOf = (g: GhostStar, depth = 0): number => {
      if (!g.parentKey || depth > 8) return 0;
      const p = byKey.get(g.parentKey);
      return p ? hopOf(p, depth + 1) + 1 : 0;
    };
    for (const g of list) hops.set(g.key, hopOf(g));
    ghostHop = hops;
    // Fan each anchor-group's ghosts evenly around its base (seed node, or
    // parent ghost for chain children); the fan is only the SEED layout — the
    // mini-sim below relaxes overlaps from there.
    const groups = new Map<string, GhostStar[]>();
    for (const g of list) {
      const k = `${g.seedId}|${g.parentKey ?? ""}`;
      let arr = groups.get(k);
      if (!arr) groups.set(k, (arr = []));
      arr.push(g);
    }
    const anchors = new Map<string, { angle: number; dist: number }>();
    for (const gs of groups.values()) {
      const n = gs.length;
      const child = !!gs[0]?.parentKey; // chain children: tighter fan, offset angle
      gs.forEach((g, i) => {
        const angle = -Math.PI / 2 + (i * TAU) / Math.max(n, 7) + (child ? 0.35 : 0);
        anchors.set(g.key, { angle, dist: child ? 82 + (i % 2) * 38 : 95 + (i % 2) * 44 });
      });
    }
    ghostAnchor = anchors;
    seedGhostPositions();
    // Keep the open card in sync (added flag) or close it if its ghost
    // vanished. Untracked: ghostCard is $state — reading it here would make
    // every card open/close re-run this effect (and re-heat the sim).
    untrack(() => {
      if (ghostCard) ghostCard = list.find((g) => g.key === ghostCard!.key) ?? null;
    });
    schedule();
  });
  /** (Re)seed sim positions: NEW ghosts start on their fan point (parents
   *  precede children in the list, so chains resolve in one pass); settled
   *  ghosts keep their relaxed position; ghosts whose base is GONE (seed node
   *  left the graph) are dropped — hidden, like the pre-sim behavior. Also
   *  called from rebuild(): on remount this effect runs BEFORE the graph is
   *  built (idToIdx still empty), so rebuild must re-seed once nodes exist. */
  function seedGhostPositions() {
    const list = ghosts ?? [];
    const next = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    let changed = false;
    for (const g of list) {
      const a = ghostAnchor.get(g.key);
      if (!a) continue;
      // The base must be resolvable NOW — otherwise the ghost stays hidden.
      let bx: number;
      let by: number;
      if (g.parentKey) {
        const pp = next.get(g.parentKey);
        if (!pp) continue;
        bx = pp.x;
        by = pp.y;
      } else {
        const i = idToIdx.get(g.seedId);
        if (i === undefined) continue;
        bx = nodes[i].x;
        by = nodes[i].y;
      }
      const prev = ghostPos.get(g.key);
      if (prev) {
        next.set(g.key, prev);
        continue;
      }
      changed = true;
      next.set(g.key, { x: bx + Math.cos(a.angle) * a.dist, y: by + Math.sin(a.angle) * a.dist, vx: 0, vy: 0 });
    }
    if (next.size !== ghostPos.size) changed = true;
    ghostPos = next;
    if (changed) ghostAlpha = 1; // re-relax only when the population changed
  }
  /** World position of a ghost: its simulated position. Null = not placeable. */
  function ghostWorld(g: GhostStar): { x: number; y: number } | null {
    const p = ghostPos.get(g.key);
    return p ? { x: p.x, y: p.y } : null;
  }
  /** World position of a ghost's base: the parent ghost, or the seed node. */
  function ghostBase(g: GhostStar): { x: number; y: number } | null {
    if (g.parentKey) {
      const pp = ghostPos.get(g.parentKey);
      return pp ? { x: pp.x, y: pp.y } : null;
    }
    const i = idToIdx.get(g.seedId);
    return i === undefined ? null : { x: nodes[i].x, y: nodes[i].y };
  }
  /** One relaxation step of the ghost mini-sim (spring to base at the ring
   *  distance, ghost↔ghost repulsion, clearance from library stars). Cheap:
   *  ghosts are few and library nodes are only pushed AGAINST, never moved. */
  function ghostTick() {
    const list = ghosts ?? [];
    if (!list.length) {
      ghostAlpha = 0;
      return;
    }
    const entries: { g: GhostStar; p: { x: number; y: number; vx: number; vy: number } }[] = [];
    for (const g of list) {
      const p = ghostPos.get(g.key);
      if (p) entries.push({ g, p });
    }
    for (const { g, p } of entries) {
      const a = ghostAnchor.get(g.key);
      const base = ghostBase(g);
      if (!a || !base) continue;
      const dx = p.x - base.x;
      const dy = p.y - base.y;
      const d = Math.hypot(dx, dy) || 1;
      const f = (d - a.dist) * 0.08; // spring toward the ring radius (any direction)
      p.vx -= (dx / d) * f;
      p.vy -= (dy / d) * f;
    }
    for (let i = 0; i < entries.length; i++) {
      for (let j = i + 1; j < entries.length; j++) {
        const A = entries[i].p;
        const B = entries[j].p;
        let dx = A.x - B.x;
        let dy = A.y - B.y;
        let d2 = dx * dx + dy * dy;
        if (d2 > 16000) continue;
        if (d2 < 0.01) {
          dx = 0.37 * (i + 1);
          dy = 0.53;
          d2 = dx * dx + dy * dy;
        }
        const d = Math.sqrt(d2);
        const f = Math.min(9, 900 / d2) + (d < 44 ? (44 - d) * 0.35 : 0);
        const ux = (dx / d) * f;
        const uy = (dy / d) * f;
        A.vx += ux;
        A.vy += uy;
        B.vx -= ux;
        B.vy -= uy;
      }
    }
    for (const { p } of entries) {
      for (const n of nodes) {
        const dx = p.x - n.x;
        const dy = p.y - n.y;
        const d2 = dx * dx + dy * dy;
        const min = n.r * 2.4 + 26;
        if (d2 < min * min && d2 > 0.01) {
          const d = Math.sqrt(d2);
          const f = (min - d) * 0.3;
          p.vx += (dx / d) * f;
          p.vy += (dy / d) * f;
        }
      }
    }
    for (const { p } of entries) {
      p.vx *= 0.78;
      p.vy *= 0.78;
      const sp = Math.hypot(p.vx, p.vy);
      if (sp > 28) {
        p.vx *= 28 / sp;
        p.vy *= 28 / sp;
      }
      p.x += p.vx * ghostAlpha;
      p.y += p.vy * ghostAlpha;
    }
    ghostAlpha *= 0.97;
    if (ghostAlpha < 0.02) ghostAlpha = 0;
  }
  function hitGhost(sx: number, sy: number): GhostStar | null {
    for (const g of ghosts ?? []) {
      const w = ghostWorld(g);
      if (!w) continue;
      const dx = sx - (w.x * zoom + tx);
      const dy = sy - (w.y * zoom + ty);
      if (dx * dx + dy * dy <= 100) return g; // ~10px radius
    }
    return null;
  }

  /** Glide the camera so the node is centered; with `minZoom` also zooms in
   *  (never out) so the star and its label are readable after a search. */
  function panToNode(id: number, minZoom?: number) {
    const i = idToIdx.get(id);
    if (i === undefined) return;
    const n = nodes[i];
    const z = minZoom ? clamp(Math.max(zoom, minZoom), 0.25, 3) : zoom;
    camAnim = {
      t0: performance.now(),
      from: [zoom, tx, ty],
      to: [z, vw / 2 - n.x * z, vh / 2 - n.y * z],
    };
    schedule();
  }

  // ----- Ricerca nel grafo: casella HUD con candidati + highlight sulla mappa -----
  interface SearchHit {
    id: number;
    title: string;
    year: number | null;
    kind: "doc" | "note";
  }
  let searchQ = $state("");
  let searchList = $state<SearchHit[]>([]);
  let searchOpen = $state(false);
  let searchSel = $state(0);
  // Read by draw() every frame: plain non-reactive vars, like hoverSet/pinnedSet.
  let matchSet: Set<number> | null = null; // nodes matching while typing (rings + dim others)
  let foundId: number | null = null; // confirmed pick: pulsing halo
  let foundAt = 0;

  /** Accent-insensitive lowercase fold (Poincaré → poincare), for matching. */
  function foldq(s: string): string {
    return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  }
  /** Candidates among the LOADED graph nodes: title prefix first, then title
   *  substring, then author match (via the parent's resolve). Max 8. */
  function hitsFor(qRaw: string): SearchHit[] {
    const q = foldq(qRaw.trim());
    if (q.length < 2) return [];
    const starts: SearchHit[] = [];
    const subs: SearchHit[] = [];
    const auth: SearchHit[] = [];
    for (const n of nodes) {
      const t = foldq(n.title || "");
      const hit: SearchHit = { id: n.id, title: n.title || "Senza titolo", year: n.year, kind: n.kind };
      if (t.startsWith(q)) starts.push(hit);
      else if (t.includes(q)) subs.push(hit);
      else if (n.kind === "doc" && resolve?.(n.id)?.authors?.some((a) => foldq(a).includes(q))) auth.push(hit);
    }
    return [...starts, ...subs, ...auth].slice(0, 8);
  }
  function onSearchInput() {
    searchOpen = true;
    searchSel = 0;
    searchList = hitsFor(searchQ);
    matchSet = searchList.length ? new Set(searchList.map((h) => h.id)) : null;
    if (searchQ.trim().length === 0) {
      matchSet = null;
      foundId = null;
      searchOpen = false;
    }
    schedule();
  }
  function pickSearchHit(h: SearchHit) {
    searchOpen = false;
    searchQ = h.title;
    matchSet = null;
    foundId = h.id;
    foundAt = performance.now();
    setFocus(h.id);
    panToNode(h.id, 1.1);
    schedule();
  }
  function clearSearch() {
    searchQ = "";
    searchList = [];
    searchOpen = false;
    searchSel = 0;
    matchSet = null;
    foundId = null;
    schedule();
  }
  function onSearchKey(e: KeyboardEvent) {
    e.stopPropagation(); // typing must never trigger the app's global shortcuts
    if (e.key === "ArrowDown" && searchList.length) {
      e.preventDefault();
      searchSel = (searchSel + 1) % searchList.length;
    } else if (e.key === "ArrowUp" && searchList.length) {
      e.preventDefault();
      searchSel = (searchSel - 1 + searchList.length) % searchList.length;
    } else if (e.key === "Enter") {
      const h = searchList[searchSel] ?? searchList[0];
      if (h) pickSearchHit(h);
    } else if (e.key === "Escape") {
      clearSearch();
      (e.currentTarget as HTMLInputElement | null)?.blur();
    }
  }

  const showMap = $derived(graph !== null && graph.embedded >= 2 && graph.nodes.length > 0);
  const countsText = $derived(
    graph === null
      ? ""
      : `${graph.nodes.length} ${graph.nodes.length === 1 ? "documento" : "documenti"} · ` +
        `${graph.edges.length} ${graph.edges.length === 1 ? "legame" : "legami"}`
  );

  // ----- Theme tokens (re-read on body[data-theme] changes) -----
  let theme = {
    bg: "#f6f2e9",
    surface: "#fffdf8",
    text: "#2c2e35",
    dim: "#63666e",
    faint: "#8c8f97",
    border: "#e2dccd",
    accent: "#2b4a78",
    accentSoft2: "#d6e0ef",
    danger: "#b0322a",
  };
  let paint = { edge: "#98a4b8", nodeStroke: "#b8b2a3", nodeFill: "rgba(43, 74, 120, 0.7)" };

  function readTheme() {
    const cs = getComputedStyle(document.body);
    const tok = (k: string, fb: string) => cs.getPropertyValue(k).trim() || fb;
    theme = {
      bg: tok("--bg", theme.bg),
      surface: tok("--surface", theme.surface),
      text: tok("--text", theme.text),
      dim: tok("--dim", theme.dim),
      faint: tok("--faint", theme.faint),
      border: tok("--border", theme.border),
      accent: tok("--accent", theme.accent),
      accentSoft2: tok("--accent-soft2", theme.accentSoft2),
      danger: tok("--danger", theme.danger),
    };
    paint = {
      edge: mixColors(theme.accent, theme.border, 0.5),
      nodeStroke: mixColors(theme.border, "#000000", 0.35),
      nodeFill: withAlpha(theme.accent, 0.7),
    };
  }

  function parseColor(c: string): [number, number, number] {
    const s = c.trim();
    if (s.startsWith("#")) {
      const h = s.slice(1);
      if (h.length === 3 || h.length === 4)
        return [parseInt(h[0] + h[0], 16), parseInt(h[1] + h[1], 16), parseInt(h[2] + h[2], 16)];
      if (h.length >= 6)
        return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
    }
    const m = s.match(/rgba?\(([^)]+)\)/);
    if (m) {
      const p = m[1].split(/[\s,/]+/).map(Number);
      if (p.length >= 3 && !p.slice(0, 3).some(Number.isNaN)) return [p[0], p[1], p[2]];
    }
    return [128, 128, 128];
  }
  function mixColors(a: string, b: string, t: number): string {
    const ca = parseColor(a);
    const cb = parseColor(b);
    return `rgb(${Math.round(ca[0] + (cb[0] - ca[0]) * t)}, ${Math.round(ca[1] + (cb[1] - ca[1]) * t)}, ${Math.round(ca[2] + (cb[2] - ca[2]) * t)})`;
  }
  function withAlpha(c: string, a: number): string {
    const [r, g, b] = parseColor(c);
    return `rgba(${r}, ${g}, ${b}, ${a})`;
  }
  /** Hue (0-360) of a CSS color — anchor for the exploration-chain palette. */
  function hueOf(color: string): number {
    const [r, g, b] = parseColor(color).map((v) => v / 255);
    const mx = Math.max(r, g, b);
    const mn = Math.min(r, g, b);
    if (mx === mn) return 210;
    const d = mx - mn;
    const h = mx === r ? (g - b) / d + (g < b ? 6 : 0) : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return h * 60;
  }
  /** Chain color per hop: the accent hue, rotated a step per hop — so each
   *  exploration generation reads as its own band of the spectrum. */
  function ghostColor(hop: number, alpha = 1): string {
    return `hsla(${(hueOf(theme.accent) + hop * 34) % 360}, 62%, 56%, ${alpha})`;
  }
  function clamp(v: number, lo: number, hi: number): number {
    return v < lo ? lo : v > hi ? hi : v;
  }
  function ellipsize(s: string, max: number): string {
    return s.length > max ? s.slice(0, max - 1).trimEnd() + "…" : s;
  }

  // ----- Build / rebuild the simulation from the graph prop -----
  function rebuild(g: SimilarityGraph | null) {
    const hadNodes = nodes.length > 0;
    const old = new Map<number, SimNode>();
    for (const n of nodes) old.set(n.id, n);
    nodes = [];
    edges = [];
    idToIdx.clear();
    adj.clear();
    adjW.clear();
    topLabelIds.clear();
    hoverIdx = -1;
    hoverSet = null;
    dragIdx = -1;
    repCursor = 0;
    tip = null;
    matchSet = null; // stale ids: the highlight recomputes on the next keystroke
    if (!g || g.embedded < 2 || g.nodes.length === 0) {
      alpha = 0;
      foundId = null;
      untrack(() => setFocus(null));
      schedule();
      return;
    }

    const count = g.nodes.length;
    const isolated = g.nodes.reduce((s, n) => s + (n.degree === 0 ? 1 : 0), 0);
    const connected = Math.max(1, count - isolated);
    const spread = Math.max(180, 30 * Math.sqrt(count));
    const golden = Math.PI * (3 - Math.sqrt(5));
    let si = 0; // spiral index (connected nodes)
    let ri = 0; // outer-ring index (isolated nodes)
    let anySaved = false;
    for (const gn of g.nodes) {
      const prev = old.get(gn.id);
      let x: number;
      let y: number;
      // Position priority: this session's live position → position saved on disk
      // → PCA seed (semantically meaningful) → legacy spiral/ring fallback.
      // Notes have no PCA seed: they get placed next to their strongest paper
      // in a second pass below (x stays NaN as the marker).
      if (prev) {
        x = prev.x;
        y = prev.y;
      } else if (gn.sx != null && gn.sy != null) {
        x = gn.sx;
        y = gn.sy;
        anySaved = true;
      } else if (gn.kind === "note") {
        x = Number.NaN;
        y = Number.NaN;
      } else if (gn.px !== 0 || gn.py !== 0) {
        // Small deterministic jitter so coincident projections don't stack.
        x = gn.px * spread * 2.2 + ((gn.id % 13) - 6) * 2.2;
        y = gn.py * spread * 2.2 + ((gn.id % 7) - 3) * 2.2;
      } else if (gn.degree > 0) {
        const rr = spread * Math.sqrt((si + 0.5) / connected);
        const a = si * golden;
        x = rr * Math.cos(a);
        y = rr * Math.sin(a);
        si++;
      } else {
        const a = (ri / Math.max(1, isolated)) * TAU + 0.4;
        x = (spread + 130) * Math.cos(a);
        y = (spread + 130) * Math.sin(a);
        ri++;
      }
      idToIdx.set(gn.id, nodes.length);
      nodes.push({
        id: gn.id,
        title: gn.title ?? "",
        year: gn.year,
        color: gn.color,
        degree: gn.degree,
        unread: gn.unread,
        favorite: gn.favorite,
        peer: gn.peer_reviewed,
        gh: gn.has_github,
        community: gn.community,
        kind: gn.kind,
        x,
        y,
        vx: 0,
        vy: 0,
        r: 4.5 + Math.min(gn.degree, 9) * 1.1,
      });
    }
    const degOf = new Map(g.nodes.map((n) => [n.id, n.degree]));
    for (const ge of g.edges) {
      const ai = idToIdx.get(ge.a);
      const bi = idToIdx.get(ge.b);
      if (ai === undefined || bi === undefined || ai === bi) continue;
      // Longer rest lengths + degree-normalized soft springs: a hub with many
      // ties would otherwise sum their pulls and crush its neighbourhood into a
      // clump — dividing by √(min degree) keeps dense stars breathable.
      const dmin = Math.max(1, Math.min(degOf.get(ge.a) ?? 1, degOf.get(ge.b) ?? 1));
      edges.push({
        ai,
        bi,
        w: ge.w,
        rest: 160 + (1 - ge.w) * 170,
        k: (0.032 * ge.w) / Math.sqrt(dmin),
      });
      let la = adj.get(ge.a);
      if (!la) adj.set(ge.a, (la = []));
      la.push(ge.b);
      let lb = adj.get(ge.b);
      if (!lb) adj.set(ge.b, (lb = []));
      lb.push(ge.a);
      let wa = adjW.get(ge.a);
      if (!wa) adjW.set(ge.a, (wa = []));
      wa.push({ id: ge.b, w: ge.w });
      let wb = adjW.get(ge.b);
      if (!wb) adjW.set(ge.b, (wb = []));
      wb.push({ id: ge.a, w: ge.w });
    }
    // Second pass: a note without a saved position sits beside its strongest
    // paper (deterministic angle from its id); orphan notes go to the outer ring.
    for (const n of nodes) {
      if (!Number.isNaN(n.x)) continue;
      const best = (adjW.get(n.id) ?? []).reduce(
        (m, e) => (m === null || e.w > m.w ? e : m),
        null as { id: number; w: number } | null,
      );
      const anchor = best ? nodes[idToIdx.get(best.id) ?? -1] : undefined;
      if (anchor && !Number.isNaN(anchor.x)) {
        const a = ((Math.abs(n.id) * 61) % 360) * (Math.PI / 180);
        n.x = anchor.x + Math.cos(a) * 46;
        n.y = anchor.y + Math.sin(a) * 46;
      } else {
        const a = ((Math.abs(n.id) % 97) / 97) * TAU;
        n.x = (spread + 170) * Math.cos(a);
        n.y = (spread + 170) * Math.sin(a);
      }
    }
    for (const gn of [...g.nodes].sort((a, b) => b.degree - a.degree).slice(0, 12)) topLabelIds.add(gn.id);
    clusterMeta = g.clusters ?? [];
    const years = g.nodes.map((n) => n.year).filter((y): y is number => y !== null);
    yearMin = years.length ? Math.min(...years) : 0;
    yearMax = years.length ? Math.max(...years) : 0;
    if (foundId !== null && !idToIdx.has(foundId)) foundId = null; // the found star left the graph
    fx = new Float32Array(nodes.length);
    fy = new Float32Array(nodes.length);
    // Re-heat gently on refresh; on first build, saved positions get a real
    // relaxation pass (so tuned physics can open up an older, tighter layout),
    // a PCA seed a bit more.
    alpha = hadNodes ? 0.25 : anySaved ? 0.5 : 0.85;
    needFit = !hadNodes;
    // The ghost mini-sim seeds against idToIdx: on remount its effect ran
    // BEFORE this rebuild (empty graph → nothing placeable), so re-seed now
    // that the nodes exist. Untracked for the same reason as setFocus below.
    untrack(() => seedGhostPositions());
    // Refresh (or drop) the pinned card against the fresh graph data. `untrack`
    // is essential: rebuild() runs inside an $effect, and without it the `panel`
    // read + write (and the parent state read by resolve()) would register as
    // effect dependencies — every click would then re-trigger the rebuild in an
    // unbounded self-invalidation loop (effect_update_depth_exceeded).
    untrack(() => setFocus(panel && idToIdx.has(panel.id) ? panel.id : null));
    schedule();
  }

  // ----- Physics: one chunked pass per frame, ~8ms budget -----
  function buildGrid() {
    grid.clear();
    for (let i = 0; i < nodes.length; i++) {
      const key = Math.floor(nodes[i].x / CELL) + "|" + Math.floor(nodes[i].y / CELL);
      const cell = grid.get(key);
      if (cell) cell.push(i);
      else grid.set(key, [i]);
    }
  }
  function repelPair(i: number, j: number) {
    const a = nodes[i];
    const b = nodes[j];
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let d2 = dx * dx + dy * dy;
    if (d2 > 250000) return; // negligible beyond ~500 world units
    if (d2 < 0.01) {
      dx = 0.5 + (i % 3) * 0.17;
      dy = 0.5 - (j % 3) * 0.17;
      d2 = dx * dx + dy * dy;
    }
    const d = Math.sqrt(d2);
    let f = Math.min(14, REPEL / d2);
    // Collision guard: below the "personal space" of the pair (their radii plus
    // breathing room) the push grows linearly, so stars never sit on each other.
    const minD = (a.r + b.r) * 2.4 + 20;
    if (d < minD) f += (minD - d) * 0.8;
    const ux = (dx / d) * f;
    const uy = (dy / d) * f;
    fx[i] += ux;
    fy[i] += uy;
    fx[j] -= ux;
    fy[j] -= uy;
  }
  function repelGrid(i: number) {
    const gx = Math.floor(nodes[i].x / CELL);
    const gy = Math.floor(nodes[i].y / CELL);
    for (let cx = gx - 1; cx <= gx + 1; cx++) {
      for (let cy = gy - 1; cy <= gy + 1; cy++) {
        const cell = grid.get(cx + "|" + cy);
        if (!cell) continue;
        for (const j of cell) if (j > i) repelPair(i, j);
      }
    }
  }
  /** Runs (or resumes) one physics pass; returns false when the 8ms budget ran out mid-pass. */
  function tickChunk(deadline: number): boolean {
    const n = nodes.length;
    const useGrid = n > 400;
    if (repCursor === 0) {
      fx.fill(0);
      fy.fill(0);
      if (useGrid) buildGrid();
    }
    for (let i = repCursor; i < n; i++) {
      if ((i & 15) === 0 && performance.now() > deadline) {
        repCursor = i;
        return false;
      }
      if (useGrid) repelGrid(i);
      else for (let j = i + 1; j < n; j++) repelPair(i, j);
    }
    repCursor = 0;
    for (const e of edges) {
      const a = nodes[e.ai];
      const b = nodes[e.bi];
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const d = Math.hypot(dx, dy) || 1;
      const f = e.k * (d - e.rest);
      const ux = (dx / d) * f;
      const uy = (dy / d) * f;
      fx[e.ai] += ux;
      fy[e.ai] += uy;
      fx[e.bi] -= ux;
      fy[e.bi] -= uy;
    }
    for (let i = 0; i < n; i++) {
      const nd = nodes[i];
      if (i === dragIdx) {
        nd.vx = 0;
        nd.vy = 0;
        continue; // pinned under the cursor
      }
      const ax = fx[i] - nd.x * 0.006; // centering gravity toward the origin (gentle: let the map breathe)
      const ay = fy[i] - nd.y * 0.006;
      nd.vx = (nd.vx + ax * alpha) * 0.86;
      nd.vy = (nd.vy + ay * alpha) * 0.86;
      const sp = Math.hypot(nd.vx, nd.vy);
      if (sp > 36) {
        nd.vx *= 36 / sp;
        nd.vy *= 36 / sp;
      }
      nd.x += nd.vx;
      nd.y += nd.vy;
    }
    alpha *= 0.985;
    if (alpha < MIN_ALPHA) alpha = 0;
    return true;
  }
  function reheat(a: number) {
    alpha = Math.max(alpha, a);
    schedule();
  }

  // ----- Persist the settled layout (map stability across sessions) -----
  let layoutDirty = false;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function flushPositions(now = false) {
    if (!onSavePositions || nodes.length === 0 || !layoutDirty) return;
    clearTimeout(saveTimer);
    const run = () => {
      layoutDirty = false;
      onSavePositions?.(nodes.map((n) => ({ id: n.id, x: n.x, y: n.y })));
    };
    if (now) run();
    else saveTimer = setTimeout(run, 1200);
  }

  // ----- Frame loop: physics while hot, otherwise redraw-on-interaction only -----
  function schedule() {
    if (rafId === null) rafId = requestAnimationFrame(frame);
  }
  function frame() {
    rafId = null;
    let animating = false;
    if (needFit && vw > 0 && nodes.length > 0) {
      needFit = false;
      fitToView(false);
    }
    if (alpha >= MIN_ALPHA && nodes.length > 0) {
      const complete = tickChunk(performance.now() + 8);
      animating = !complete || alpha >= MIN_ALPHA;
      layoutDirty = true;
      if (alpha < MIN_ALPHA) flushPositions(); // just cooled: persist (debounced)
    }
    if (camAnim) {
      const t = Math.min(1, (performance.now() - camAnim.t0) / 250);
      const e = 1 - Math.pow(1 - t, 3);
      zoom = camAnim.from[0] + (camAnim.to[0] - camAnim.from[0]) * e;
      tx = camAnim.from[1] + (camAnim.to[1] - camAnim.from[1]) * e;
      ty = camAnim.from[2] + (camAnim.to[2] - camAnim.from[2]) * e;
      if (t >= 1) camAnim = null;
      else animating = true;
    }
    // Keep animating while the search-found pulse is still expanding.
    if (foundId !== null && performance.now() - foundAt < 2600) animating = true;
    // Exploration mode: relax the ghost mini-sim (following moving seeds while
    // the main physics is hot) and keep the chain links / pulses alive.
    if ((ghosts?.length ?? 0) > 0) {
      if (alpha >= MIN_ALPHA) ghostAlpha = Math.max(ghostAlpha, 0.3);
      if (ghostAlpha > 0) ghostTick();
      animating = true;
    }
    draw();
    if (animating) schedule();
  }

  // ----- Camera -----
  function zoomAt(sx: number, sy: number, factor: number) {
    const z = clamp(zoom * factor, 0.25, 3);
    tx = sx - ((sx - tx) / zoom) * z;
    ty = sy - ((sy - ty) / zoom) * z;
    zoom = z;
    schedule();
  }
  function fitToView(animate: boolean) {
    if (nodes.length === 0 || vw === 0 || vh === 0) return;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const n of nodes) {
      if (n.x - n.r < minX) minX = n.x - n.r;
      if (n.x + n.r > maxX) maxX = n.x + n.r;
      if (n.y - n.r < minY) minY = n.y - n.r;
      if (n.y + n.r > maxY) maxY = n.y + n.r;
    }
    const bw = Math.max(60, maxX - minX);
    const bh = Math.max(60, maxY - minY);
    const z = clamp(Math.min((vw - 90) / bw, (vh - 90) / bh), 0.25, 3);
    const ntx = vw / 2 - ((minX + maxX) / 2) * z;
    const nty = vh / 2 - ((minY + maxY) / 2) * z;
    if (animate) {
      camAnim = { t0: performance.now(), from: [zoom, tx, ty], to: [z, ntx, nty] };
    } else {
      zoom = z;
      tx = ntx;
      ty = nty;
    }
    schedule();
  }

  // ----- Background starfield: pre-rendered once per size/theme (free per frame) -----
  let skyCanvas: HTMLCanvasElement | null = null;
  function buildSky() {
    if (vw <= 0 || vh <= 0) {
      skyCanvas = null;
      return;
    }
    const cv = document.createElement("canvas");
    cv.width = Math.max(1, Math.round(vw * dpr));
    cv.height = Math.max(1, Math.round(vh * dpr));
    const s = cv.getContext("2d");
    if (!s) return;
    s.setTransform(dpr, 0, 0, dpr, 0, 0);
    let seed = 987654321; // fixed → the sky doesn't shimmer between rebuilds
    const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
    const count = Math.round((vw * vh) / 9000);
    s.fillStyle = theme.faint;
    for (let i = 0; i < count; i++) {
      const x = rnd() * vw;
      const y = rnd() * vh;
      const r = 0.4 + rnd() * 1.1;
      s.globalAlpha = 0.07 + rnd() * 0.16;
      s.beginPath();
      s.arc(x, y, r, 0, TAU);
      s.fill();
    }
    skyCanvas = cv;
  }

  // ----- Drawing (all in screen space, DPR-aware) -----
  function chipPath(c: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
    const rr = Math.min(r, h / 2, w / 2);
    c.beginPath();
    c.moveTo(x + rr, y);
    c.arcTo(x + w, y, x + w, y + h, rr);
    c.arcTo(x + w, y + h, x, y + h, rr);
    c.arcTo(x, y + h, x, y, rr);
    c.arcTo(x, y, x + w, y, rr);
    c.closePath();
  }
  function drawSparkle(c: CanvasRenderingContext2D, x: number, y: number, R: number) {
    const r = R * 0.42;
    c.beginPath();
    for (let i = 0; i < 4; i++) {
      const a = -Math.PI / 2 + (i * Math.PI) / 2;
      c.lineTo(x + Math.cos(a) * R, y + Math.sin(a) * R);
      const b = a + Math.PI / 4;
      c.lineTo(x + Math.cos(b) * r, y + Math.sin(b) * r);
    }
    c.closePath();
    c.fillStyle = theme.accent;
    c.fill();
    c.strokeStyle = theme.surface;
    c.lineWidth = 0.75;
    c.stroke();
  }
  /// Above this zoom the status badges (peer-reviewed ✓, GitHub fork) are drawn.
  const BADGE_ZOOM = 0.9;
  /// Below this zoom the semantic communities render as labelled "nebulae".
  const NEBULA_ZOOM = 0.5;
  /** Peer-reviewed: a small green disc with a check mark. */
  function drawCheckBadge(c: CanvasRenderingContext2D, x: number, y: number, R: number) {
    c.fillStyle = "#2e9e63";
    c.beginPath();
    c.arc(x, y, R, 0, TAU);
    c.fill();
    c.strokeStyle = theme.surface;
    c.lineWidth = Math.max(1, R * 0.32);
    c.lineCap = "round";
    c.beginPath();
    c.moveTo(x - R * 0.45, y + R * 0.05);
    c.lineTo(x - R * 0.1, y + R * 0.42);
    c.lineTo(x + R * 0.5, y - R * 0.35);
    c.stroke();
  }
  /** GitHub repo: a small disc with a tiny fork (two branch dots joined to one). */
  function drawForkBadge(c: CanvasRenderingContext2D, x: number, y: number, R: number) {
    c.fillStyle = theme.dim;
    c.beginPath();
    c.arc(x, y, R, 0, TAU);
    c.fill();
    c.strokeStyle = theme.surface;
    c.lineWidth = Math.max(1, R * 0.26);
    c.lineCap = "round";
    const t = -R * 0.42; // branch dots height
    c.beginPath();
    c.moveTo(x - R * 0.4, y + t);
    c.lineTo(x - R * 0.4, y);
    c.quadraticCurveTo(x - R * 0.4, y + R * 0.3, x, y + R * 0.3);
    c.quadraticCurveTo(x + R * 0.4, y + R * 0.3, x + R * 0.4, y);
    c.lineTo(x + R * 0.4, y + t);
    c.moveTo(x, y + R * 0.3);
    c.lineTo(x, y + R * 0.55);
    c.stroke();
  }
  function draw() {
    if (!ctx || vw === 0 || vh === 0) return;
    const c = ctx;
    c.setTransform(dpr, 0, 0, dpr, 0, 0);
    c.clearRect(0, 0, vw, vh);
    if (skyCanvas) c.drawImage(skyCanvas, 0, 0, vw, vh); // il cielo dietro le stelle
    if (nodes.length === 0) return;

    // Focus: the hovered neighborhood wins while the pointer is on a node;
    // otherwise the pinned card's neighborhood (if any) stays lit.
    const hovId = hoverIdx >= 0 && hoverIdx < nodes.length ? nodes[hoverIdx].id : (panel?.id ?? -1);
    const focus = hoverIdx >= 0 ? hoverSet : pinnedSet;
    // Exploration mode: the library recedes to a dim backdrop, the seeds and
    // the discovery chains carry the scene.
    const exploring = (ghosts?.length ?? 0) > 0;
    const seedSet = exploring ? new Set((ghosts ?? []).map((g) => g.seedId)) : null;
    const tsec = performance.now() / 1000;

    // Nebulae: soft halos over each semantic community, at EVERY zoom — strong
    // from afar (the map reads as thematic areas), a subtle tint up close.
    // I NOMI dei cluster vengono raccolti qui ma disegnati in coda al frame,
    // SOPRA nodi ed etichette, su targhette leggibili (vedi fine di draw()).
    const nebLabels: { title: string; sub: string; cx: number; cy: number; color: string }[] = [];
    if (clusterMeta.length > 0 && nebulaMode !== "off") {
      let nebAlpha = zoom < NEBULA_ZOOM ? 0.16 : Math.max(0.06, 0.16 - (zoom - NEBULA_ZOOM) * 0.08);
      if (exploring) nebAlpha *= 0.4; // recede during exploration
      const acc: Map<number, { sx: number; sy: number; n: number; r2: number }> = new Map();
      for (const n of nodes) {
        if (n.community < 0) continue;
        let a = acc.get(n.community);
        if (!a) acc.set(n.community, (a = { sx: 0, sy: 0, n: 0, r2: 0 }));
        a.sx += n.x;
        a.sy += n.y;
        a.n++;
      }
      for (const n of nodes) {
        const a = n.community >= 0 ? acc.get(n.community) : undefined;
        if (!a) continue;
        const dx = n.x - a.sx / a.n;
        const dy = n.y - a.sy / a.n;
        a.r2 = Math.max(a.r2, dx * dx + dy * dy);
      }
      for (const meta of clusterMeta) {
        const a = acc.get(meta.id);
        if (!a || a.n < 3) continue;
        const cx = (a.sx / a.n) * zoom + tx;
        const cy = (a.sy / a.n) * zoom + ty;
        const rr = (Math.sqrt(a.r2) * 0.85 + 60) * zoom;
        if (cx < -rr || cx > vw + rr || cy < -rr || cy > vh + rr) continue;
        const g = c.createRadialGradient(cx, cy, rr * 0.15, cx, cy, rr);
        g.addColorStop(0, commColor(meta.id, nebAlpha));
        g.addColorStop(1, commColor(meta.id, 0));
        c.fillStyle = g;
        c.beginPath();
        c.arc(cx, cy, rr, 0, TAU);
        c.fill();
        if (nebulaMode === "full" && zoom < 0.9 && !exploring) {
          nebLabels.push({
            title: ellipsize(meta.label, 34),
            sub: `${a.n} paper`,
            cx,
            cy,
            color: commColor(meta.id, 0.85),
          });
        }
      }
    }

    // Edges
    c.strokeStyle = paint.edge;
    c.lineCap = "round";
    for (const e of edges) {
      const a = nodes[e.ai];
      const b = nodes[e.bi];
      const ax = a.x * zoom + tx;
      const ay = a.y * zoom + ty;
      const bx = b.x * zoom + tx;
      const by = b.y * zoom + ty;
      if (
        (ax < -40 && bx < -40) ||
        (ax > vw + 40 && bx > vw + 40) ||
        (ay < -40 && by < -40) ||
        (ay > vh + 40 && by > vh + 40)
      )
        continue;
      let al = 0.09 + e.w * 0.3;
      if (focus && a.id !== hovId && b.id !== hovId) al *= 0.18;
      if (matchSet && !matchSet.has(a.id) && !matchSet.has(b.id)) al *= 0.3;
      if (exploring) al *= 0.35; // library ties recede behind the chains
      c.globalAlpha = al;
      c.lineWidth = 0.7 + e.w * 1.1; // heavier ties read thicker, continuously
      c.beginPath();
      c.moveTo(ax, ay);
      c.lineTo(bx, by);
      c.stroke();
    }

    // Nodes
    for (const n of nodes) {
      const sx = n.x * zoom + tx;
      const sy = n.y * zoom + ty;
      const rs = clamp(n.r * zoom, 3, 26);
      if (sx < -34 || sx > vw + 34 || sy < -34 || sy > vh + 34) continue;
      let na = (focus && !focus.has(n.id)) || (matchSet && !matchSet.has(n.id)) ? 0.18 : 1;
      if (seedSet && !seedSet.has(n.id)) na = Math.min(na, 0.32); // backdrop during exploration
      c.globalAlpha = na;
      // Soft glow behind the star, in its own color: the "shine" that makes the
      // sky read as stars instead of dots (one extra arc: negligible cost).
      const fill = nodeFill(n);
      const a0 = c.globalAlpha;
      c.globalAlpha = a0 * 0.16;
      c.fillStyle = fill;
      c.beginPath();
      c.arc(sx, sy, rs * 2.2, 0, TAU);
      c.fill();
      c.globalAlpha = a0;
      if (selectedSet.has(n.id)) {
        c.save();
        c.shadowColor = theme.accent;
        c.shadowBlur = 10;
        c.strokeStyle = theme.accent;
        c.lineWidth = 2.5;
        c.beginPath();
        c.arc(sx, sy, rs + 4.5, 0, TAU);
        c.stroke();
        c.restore();
      }
      if (n.unread) {
        c.strokeStyle = theme.accent;
        c.lineWidth = 2;
        c.beginPath();
        c.arc(sx, sy, rs + 2, 0, TAU);
        c.stroke();
      }
      c.fillStyle = fill;
      c.beginPath();
      if (n.kind === "note") {
        // Diamond: an appunto, not a paper.
        const d = rs * 1.25;
        c.moveTo(sx, sy - d);
        c.lineTo(sx + d, sy);
        c.lineTo(sx, sy + d);
        c.lineTo(sx - d, sy);
        c.closePath();
      } else {
        c.arc(sx, sy, rs, 0, TAU);
      }
      c.fill();
      c.strokeStyle = paint.nodeStroke;
      c.lineWidth = 1;
      c.stroke();
      if (n.favorite) drawSparkle(c, sx + rs * 0.85, sy - rs * 0.85, Math.max(3.5, rs * 0.36));
      // Status badges only when zoomed in enough to read them (LOD).
      if (zoom > BADGE_ZOOM) {
        const br = Math.max(3.2, rs * 0.34);
        if (n.peer) drawCheckBadge(c, sx + rs * 0.85, sy + rs * 0.85, br);
        if (n.gh) drawForkBadge(c, sx - rs * 0.85, sy + rs * 0.85, br);
      }
      // Search: a dashed accent ring marks every candidate while typing.
      if (matchSet?.has(n.id)) {
        c.setLineDash([4, 3]);
        c.strokeStyle = theme.accent;
        c.lineWidth = 1.8;
        c.beginPath();
        c.arc(sx, sy, rs + 6.5, 0, TAU);
        c.stroke();
        c.setLineDash([]);
      }
      // Exploration: the seed star wears a slowly revolving "scanner" arc.
      if (seedSet?.has(n.id)) {
        const a0s = c.globalAlpha;
        c.globalAlpha = 0.85;
        c.strokeStyle = theme.accent;
        c.lineWidth = 1.6;
        const start = (tsec * 1.5) % TAU;
        c.beginPath();
        c.arc(sx, sy, rs + 9, start, start + TAU * 0.7);
        c.stroke();
        c.globalAlpha = a0s;
      }
    }
    c.globalAlpha = 1;

    // Search: the confirmed find gets an expanding pulse (~2.6s), then a steady
    // dashed halo until the search is cleared — so the eye lands on it.
    if (foundId !== null) {
      const fi = idToIdx.get(foundId);
      if (fi !== undefined) {
        const n = nodes[fi];
        const sx = n.x * zoom + tx;
        const sy = n.y * zoom + ty;
        const rs = clamp(n.r * zoom, 3, 26);
        const age = (performance.now() - foundAt) / 1000;
        c.save();
        c.strokeStyle = theme.accent;
        if (age < 2.6) {
          const ph = (age * 1.4) % 1;
          c.globalAlpha = 0.8 * (1 - ph);
          c.lineWidth = 2.2;
          c.beginPath();
          c.arc(sx, sy, rs + 6 + ph * 26, 0, TAU);
          c.stroke();
        }
        c.globalAlpha = 0.9;
        c.lineWidth = 2;
        c.setLineDash([5, 4]);
        c.beginPath();
        c.arc(sx, sy, rs + 6, 0, TAU);
        c.stroke();
        c.restore();
      }
    }

    // Ghost stars: dashed discoveries anchored around their seed.
    if (ghosts && ghosts.length > 0) {
      c.font = LABEL_FONT;
      c.textAlign = "center";
      for (const g of ghosts) {
        const w = ghostWorld(g);
        if (!w) continue;
        const gx = w.x * zoom + tx;
        const gy = w.y * zoom + ty;
        if (gx < -60 || gx > vw + 60 || gy < -40 || gy > vh + 40) continue;
        const hop = ghostHop.get(g.key) ?? 0;
        const col = ghostColor(hop);
        // Curved, gently flowing tie to the ghost's base (the seed node, or —
        // for exploration chains — the ghost it was discovered from). The dash
        // offset drifts with time: the chain reads as a live signal.
        const base = ghostBase(g);
        if (base) {
          const bx = base.x * zoom + tx;
          const by = base.y * zoom + ty;
          const mx = (bx + gx) / 2;
          const my = (by + gy) / 2;
          const ddx = gx - bx;
          const ddy = gy - by;
          const dl = Math.hypot(ddx, ddy) || 1;
          c.setLineDash([4, 5]);
          c.lineDashOffset = -((tsec * 22) % 9);
          c.strokeStyle = ghostColor(hop, 0.55);
          c.lineWidth = 1.2;
          c.beginPath();
          c.moveTo(bx, by);
          c.quadraticCurveTo(mx - (ddy / dl) * 16, my + (ddx / dl) * 16, gx, gy);
          c.stroke();
          c.lineDashOffset = 0;
        }
        // The discovery itself: a softly pulsing, glowing dashed star in its
        // hop color (each generation of the chain shifts hue).
        const pr = 6.5 + Math.sin(tsec * 2.1 + gx * 0.05) * 0.7;
        c.save();
        c.shadowColor = col;
        c.shadowBlur = 12;
        c.setLineDash([2.5, 3]);
        c.strokeStyle = g.added || g.inLibrary ? "#2e9e63" : col;
        c.fillStyle = withAlpha(theme.surface, 0.6);
        c.lineWidth = 1.5;
        c.beginPath();
        c.arc(gx, gy, pr, 0, TAU);
        c.fill();
        c.stroke();
        c.restore();
        c.setLineDash([]);
        if (g.added || g.inLibrary) {
          c.strokeStyle = "#2e9e63";
          c.lineWidth = 1.6;
          c.beginPath();
          c.moveTo(gx - 2.6, gy + 0.3);
          c.lineTo(gx - 0.6, gy + 2.4);
          c.lineTo(gx + 3.0, gy - 2.2);
          c.stroke();
        }
        if (zoom > 0.75) {
          c.fillStyle = withAlpha(theme.dim, 0.85);
          c.fillText(ellipsize(g.title || "Senza titolo", 26), gx, gy + 17);
        }
      }
      c.textAlign = "left";
    }

    // Labels (LOD): hovered node + neighbors always; the 12 top hubs at mid zoom;
    // EVERY visible node when zoomed right in (the loop below culls offscreen).
    const labelIds = new Set<number>();
    if (hovId >= 0) {
      labelIds.add(hovId);
      for (const nb of adj.get(hovId) ?? []) labelIds.add(nb);
    }
    // Search candidates and the confirmed find are always labelled.
    if (matchSet) for (const id of matchSet) labelIds.add(id);
    if (foundId !== null) labelIds.add(foundId);
    if (zoom > 0.55) for (const id of topLabelIds) labelIds.add(id);
    if (zoom > 1.6) for (const n of nodes) labelIds.add(n.id);
    if (labelIds.size > 0) {
      c.font = LABEL_FONT;
      c.textAlign = "left";
      c.textBaseline = "middle";
      for (const id of labelIds) {
        const i = idToIdx.get(id);
        if (i === undefined) continue;
        const n = nodes[i];
        const sx = n.x * zoom + tx;
        const sy = n.y * zoom + ty;
        if (sx < -140 || sx > vw + 140 || sy < -40 || sy > vh + 40) continue;
        const rs = clamp(n.r * zoom, 3, 26);
        let la = focus && !focus.has(id) ? 0.3 : 1;
        if (seedSet && !seedSet.has(id) && id !== hovId) la = Math.min(la, 0.35); // backdrop while exploring
        const label = ellipsize(n.title || "Senza titolo", 28);
        const w = c.measureText(label).width;
        const lx = sx - w / 2;
        const ly = sy + rs + 13;
        c.globalAlpha = 0.75 * la;
        c.fillStyle = theme.bg;
        chipPath(c, lx - 6, ly - 9, w + 12, 18, 6);
        c.fill();
        c.globalAlpha = la;
        c.fillStyle = theme.text;
        c.fillText(label, lx, ly);
      }
      c.globalAlpha = 1;
    }

    // Nomi delle nebulose: per ULTIMI, sopra nodi/archi/etichette, su una
    // targhetta col bordo nel colore della comunità. Visibili da lontano e in
    // dissolvenza fino a zoom 0.9 (da vicino parlano i singoli nodi); una
    // targhetta che coprirebbe la precedente scivola sotto di essa.
    if (nebLabels.length > 0) {
      const la = zoom < 0.7 ? 1 : Math.max(0, 1 - (zoom - 0.7) / 0.2);
      if (la > 0.02) {
        c.textAlign = "center";
        c.textBaseline = "middle";
        const placed: { x: number; y: number; w: number; h: number }[] = [];
        for (const nl of nebLabels) {
          c.font = '600 13px Georgia, "Times New Roman", serif';
          const w1 = c.measureText(nl.title).width;
          c.font = LABEL_FONT;
          const w2 = c.measureText(nl.sub).width;
          const bw = Math.max(w1, w2) + 26;
          const bh = 40;
          const bx = nl.cx - bw / 2;
          let by = nl.cy - bh / 2;
          for (let guard = 0; guard < 6; guard++) {
            const hit = placed.find(
              (p) => bx < p.x + p.w && bx + bw > p.x && by < p.y + p.h && by + bh > p.y,
            );
            if (!hit) break;
            by = hit.y + hit.h + 6;
          }
          placed.push({ x: bx, y: by, w: bw, h: bh });
          c.globalAlpha = 0.86 * la;
          c.fillStyle = theme.bg;
          chipPath(c, bx, by, bw, bh, 11);
          c.fill();
          c.globalAlpha = la;
          c.strokeStyle = nl.color;
          c.lineWidth = 1.2;
          chipPath(c, bx, by, bw, bh, 11);
          c.stroke();
          c.font = '600 13px Georgia, "Times New Roman", serif';
          c.fillStyle = theme.text;
          c.fillText(nl.title, bx + bw / 2, by + 14);
          c.font = LABEL_FONT;
          c.fillStyle = theme.dim;
          c.fillText(nl.sub, bx + bw / 2, by + 29);
        }
        c.globalAlpha = 1;
        c.textAlign = "left";
      }
    }
  }

  // ----- Interaction -----
  function hitTest(sx: number, sy: number): number {
    for (let i = nodes.length - 1; i >= 0; i--) {
      const n = nodes[i];
      const dx = sx - (n.x * zoom + tx);
      const dy = sy - (n.y * zoom + ty);
      const rs = clamp(n.r * zoom, 3, 26) + 3;
      if (dx * dx + dy * dy <= rs * rs) return i;
    }
    return -1;
  }
  function setHover(i: number, px: number, py: number) {
    if (i !== hoverIdx) {
      hoverIdx = i;
      if (i >= 0) {
        const n = nodes[i];
        hoverSet = new Set<number>([n.id, ...(adj.get(n.id) ?? [])]);
      } else {
        hoverSet = null;
      }
      canvas.style.cursor = i >= 0 ? "pointer" : "default";
      schedule();
    }
    if (i >= 0) {
      const n = nodes[i];
      const conn = n.degree === 1 ? "1 connessione" : `${n.degree} connessioni`;
      tip = {
        x: clamp(px + 16, 0, Math.max(0, vw - 256)),
        y: clamp(py + 18, 0, Math.max(0, vh - 84)),
        title: n.title || "Senza titolo",
        meta:
          n.kind === "note"
            ? `appunto · ${conn}`
            : n.year !== null
              ? `${n.year} · ${conn}`
              : conn,
      };
    } else {
      tip = null;
    }
  }
  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    const i = hitTest(e.offsetX, e.offsetY);
    ptr = {
      down: true,
      startX: e.offsetX,
      startY: e.offsetY,
      moved: false,
      mode: i >= 0 ? "node" : "pan",
      nodeIdx: i,
      panTx: tx,
      panTy: ty,
    };
    canvas.setPointerCapture(e.pointerId);
  }
  function onPointerMove(e: PointerEvent) {
    if (!ptr.down) {
      const i = hitTest(e.offsetX, e.offsetY);
      setHover(i, e.offsetX, e.offsetY);
      if (i < 0) {
        const g = hitGhost(e.offsetX, e.offsetY);
        if (g) {
          canvas.style.cursor = "pointer";
          tip = {
            x: clamp(e.offsetX + 16, 0, Math.max(0, vw - 256)),
            y: clamp(e.offsetY + 18, 0, Math.max(0, vh - 84)),
            title: g.title || "Senza titolo",
            meta: `${g.year ?? "s.d."} · stella fantasma — clic per la scheda`,
          };
        }
      }
      return;
    }
    const dx = e.offsetX - ptr.startX;
    const dy = e.offsetY - ptr.startY;
    if (!ptr.moved && dx * dx + dy * dy > 16) {
      ptr.moved = true; // >4px: it's a drag, not a click
      canvas.style.cursor = "grabbing";
      if (ptr.mode === "node") {
        dragIdx = ptr.nodeIdx;
        tip = null;
      }
    }
    if (!ptr.moved) return;
    if (ptr.mode === "node" && dragIdx >= 0 && dragIdx < nodes.length) {
      const n = nodes[dragIdx];
      n.x = (e.offsetX - tx) / zoom;
      n.y = (e.offsetY - ty) / zoom;
      n.vx = 0;
      n.vy = 0;
      reheat(0.5);
    } else {
      tx = ptr.panTx + dx;
      ty = ptr.panTy + dy;
      schedule();
    }
  }
  function onPointerUp(e: PointerEvent) {
    if (!ptr.down) return;
    const wasClick = !ptr.moved;
    const idx = ptr.nodeIdx;
    ptr.down = false;
    dragIdx = -1; // release unpins: the sim takes the node back
    canvas.style.cursor = hoverIdx >= 0 ? "pointer" : "default";
    if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
    if (wasClick) {
      const ghost = idx < 0 ? hitGhost(e.offsetX, e.offsetY) : null;
      if (idx >= 0 && idx < nodes.length) {
        const id = nodes[idx].id;
        ghostCard = null;
        if (e.ctrlKey || e.metaKey) onToggleSelect(id);
        else setFocus(id); // single click = info card; double click opens the paper
      } else if (ghost) {
        setFocus(null);
        tuneOpen = false;
        ghostCard = ghost; // ghost card replaces the doc card (same corner)
      } else if (ptr.mode === "pan") {
        setFocus(null); // click on empty sky dismisses the cards
        ghostCard = null;
      }
    }
  }
  function onPointerLeave() {
    if (!ptr.down) setHover(-1, 0, 0);
  }
  function onContextMenu(e: MouseEvent) {
    const i = hitTest(e.offsetX, e.offsetY);
    if (i >= 0) {
      e.preventDefault();
      e.stopPropagation();
      onContext(e, nodes[i].id);
    }
    // Empty space: neither preventDefault nor stop — the parent's global radial menu takes over.
  }
  function onDblClick(e: MouseEvent) {
    const i = hitTest(e.offsetX, e.offsetY);
    if (i >= 0) onOpen(nodes[i].id); // double click on a star opens the paper
    else fitToView(true);
  }
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    zoomAt(e.offsetX, e.offsetY, Math.exp(-e.deltaY * 0.0016));
  }

  // ----- Lifecycle -----
  function setupBackingStore() {
    dpr = window.devicePixelRatio || 1;
    canvas.width = Math.max(1, Math.round(vw * dpr));
    canvas.height = Math.max(1, Math.round(vh * dpr));
  }
  // DPR can change without a resize (window moved between mixed-DPI monitors):
  // watch a resolution media query, re-armed on every change since the query is DPR-specific.
  let dprQuery: MediaQueryList | null = null;
  function onDprChange() {
    armDprWatch();
    setupBackingStore();
    buildSky();
    schedule();
  }
  function armDprWatch() {
    dprQuery?.removeEventListener("change", onDprChange);
    dprQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
    dprQuery.addEventListener("change", onDprChange);
  }
  onMount(() => {
    ctx = canvas.getContext("2d");
    readTheme();
    ro = new ResizeObserver((entries) => {
      const en = entries[0];
      if (!en) return;
      vw = en.contentRect.width;
      vh = en.contentRect.height;
      setupBackingStore();
      buildSky();
      schedule();
    });
    ro.observe(container);
    mo = new MutationObserver(() => {
      readTheme();
      buildSky(); // the starfield uses theme tokens
      schedule();
    });
    mo.observe(document.body, { attributes: true, attributeFilter: ["data-theme"] });
    canvas.addEventListener("wheel", onWheel, { passive: false });
    armDprWatch();
    return () => {
      flushPositions(true); // leaving the map: persist the settled layout now
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = null;
      ro?.disconnect();
      mo?.disconnect();
      canvas.removeEventListener("wheel", onWheel);
      dprQuery?.removeEventListener("change", onDprChange);
      dprQuery = null;
    };
  });

  $effect(() => {
    rebuild(graph);
  });
  $effect(() => {
    selectedSet = new Set(selected);
    schedule();
  });
  $effect(() => {
    // Any change of the data props means the parent reacted to "Genera indice": re-enable the CTA.
    void graph;
    void loading;
    generating = false;
  });

  function startGenerate() {
    if (generating) return;
    generating = true;
    onGenerate();
  }
</script>

<div class="wrap" bind:this={container}>
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
  <canvas
    bind:this={canvas}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    onpointerleave={onPointerLeave}
    oncontextmenu={onContextMenu}
    ondblclick={onDblClick}
  ></canvas>

  {#if showMap}
    <div class="chip counts">
      {countsText}
      {#if (ghosts?.length ?? 0) > 0}
        · {ghosts!.length} fantasm{ghosts!.length === 1 ? "a" : "i"}
        {#if onGhostsClear}<button class="chipx" onclick={onGhostsClear} title="Nascondi le stelle fantasma">×</button>{/if}
      {/if}
    </div>
    <div class="chip legend">clic = scheda · doppio clic = apri · ✦ preferito · ◆ appunto · da vicino: ✓ peer-reviewed, ⑂ GitHub</div>
    <div class="hud">
      <div class="srch">
        <input
          class="srchin"
          placeholder="Cerca nel grafo…"
          bind:value={searchQ}
          oninput={onSearchInput}
          onkeydown={onSearchKey}
          onfocus={() => { if (searchList.length) searchOpen = true; }}
          onblur={() => (searchOpen = false)}
          title="Trova un paper o un appunto nel grafo: digita qualche lettera del titolo (o di un autore), scegli il candidato e la vista si centra lì"
        />
        {#if searchQ}
          <button class="srchx" onmousedown={(e) => { e.preventDefault(); clearSearch(); }} title="Pulisci la ricerca (togli anche l'evidenziazione)">×</button>
        {/if}
        {#if searchOpen && searchList.length}
          <ul class="srchlist" role="listbox" aria-label="Candidati">
            {#each searchList as h, i (h.id)}
              <li>
                <button
                  class="srchit"
                  class:selrow={i === searchSel}
                  role="option"
                  aria-selected={i === searchSel}
                  onmousedown={(e) => { e.preventDefault(); pickSearchHit(h); }}
                  onmouseenter={() => (searchSel = i)}
                >
                  <span class="srchkind" style:color={h.kind === "note" ? NOTE_FILL : "var(--accent)"}>{h.kind === "note" ? "◆" : "●"}</span>
                  <span class="srchtitle">{h.title}</span>
                  {#if h.year !== null}<span class="srchyear">{h.year}</span>{/if}
                </button>
              </li>
            {/each}
          </ul>
        {:else if searchOpen && searchQ.trim().length >= 2}
          <div class="srchlist srchnone">Niente nel grafo (ci sono solo i documenti con indice semantico).</div>
        {/if}
      </div>
      <select class="hudsel" bind:value={colorMode} title="Colora le stelle per…">
        {#each COLOR_MODES as m (m.value)}<option value={m.value}>{m.label}</option>{/each}
      </select>
      <select class="hudsel" bind:value={nebulaMode} title="Nebulose delle comunità: aloni con i nomi, solo gli aloni, oppure niente">
        {#each NEBULA_MODES as m (m.value)}<option value={m.value}>{m.label}</option>{/each}
      </select>
      {#if onParams}
        <button title="Densità del grafo (vicini e soglia di somiglianza)" class:on={tuneOpen} onclick={toggleTune}>⚙</button>
      {/if}
      <button title="Adatta alla vista" onclick={() => fitToView(true)}>⤢</button>
      <button title="Ingrandisci" onclick={() => zoomAt(vw / 2, vh / 2, 1.3)}>+</button>
      <button title="Riduci" onclick={() => zoomAt(vw / 2, vh / 2, 1 / 1.3)}>−</button>
      <button title="Ricarica il grafo" onclick={onRefresh}>↻</button>
    </div>
    {#if tuneOpen}
      <div class="tune" role="group" aria-label="Densità del grafo">
        <label title="Quanti vicini più simili collegare a ogni paper. Più alto = rete più fitta e cluster più fusi; più basso = mappa più rada e leggibile.">
          <span>Legami per nodo <b>{tuneK}</b></span>
          <input type="range" min="1" max="8" step="1" bind:value={tuneK} />
        </label>
        <label title="Somiglianza minima perché un legame esista (0-100%). Più alta = restano solo i legami forti (mappa più frammentata ma affidabile); più bassa = più connessioni, anche deboli.">
          <span>Soglia somiglianza <b>{Math.round(tuneSim * 100)}%</b></span>
          <input type="range" min="0.4" max="0.8" step="0.05" bind:value={tuneSim} />
        </label>
        <div class="tunerow">
          <button class="tuneapply" onclick={applyTune}>Ricalcola</button>
          <button class="tunecancel" onclick={() => (tuneOpen = false)}>Annulla</button>
        </div>
      </div>
    {/if}
  {/if}

  {#if panel}
    <aside class="card" aria-label="Scheda del paper">
      <button class="card-x" title="Chiudi la scheda" onclick={() => setFocus(null)}>×</button>
      <div class="card-title">{panel.title}</div>
      <div class="card-meta">
        {#if panel.kind === "note"}
          ◆ Appunto (.md)
        {:else}
          {panel.year ?? "s.d."}
          {#if panel.favorite}· ✦ preferito{/if}
          {#if panel.unread}· da leggere{/if}
          {#if panel.peer}· <span class="card-peer">✓ peer-reviewed</span>{/if}
          {#if panel.gh}· ⑂ GitHub{/if}
        {/if}
      </div>
      {#if panel.extra?.authors?.length}
        <div class="card-authors">{panel.extra.authors.slice(0, 4).join(", ")}{panel.extra.authors.length > 4 ? " e altri" : ""}</div>
      {/if}
      {#if panel.extra?.venue}
        <div class="card-venue">{panel.extra.venue}</div>
      {/if}
      {#if panel.extra?.tags?.length}
        <div class="card-tags">
          {#each panel.extra.tags.slice(0, 6) as t (t.name)}
            <span class="card-tag" style:border-color={t.color || "var(--border)"}>{t.name}</span>
          {/each}
        </div>
      {/if}
      <div class="card-sec">{panel.neighbors.length === 1 ? "1 legame" : `${panel.neighbors.length} legami`} per somiglianza</div>
      <div class="card-links">
        {#each panel.neighbors as nb (nb.id)}
          <button class="card-link" title="Mostra la scheda di questo paper" onclick={() => { setFocus(nb.id); panToNode(nb.id); }}>
            <span class="card-link-w">{Math.round(nb.w * 100)}%</span>
            <span class="card-link-t">{nb.title}{nb.year !== null ? ` (${nb.year})` : ""}</span>
          </button>
        {:else}
          <p class="card-none">Nessun legame sopra la soglia.</p>
        {/each}
      </div>
      {#if onExplore && panel.kind === "doc"}
        <div class="card-sec">Esplora dintorni (online)</div>
        <div class="card-explore">
          <button onclick={() => onExplore(panel!.id, "citations")} title="Chi cita e chi è citato da questo paper (OpenAlex) — via DOI, o per titolo se manca">Citazioni</button>
          <button onclick={() => onExplore(panel!.id, "similar")} title="Paper simili per argomento (OpenAlex)">Simili</button>
          <button onclick={() => onExplore(panel!.id, "author")} title="Altri lavori del primo autore">Autore</button>
        </div>
        <p class="card-ghosthint">I risultati appaiono come stelle tratteggiate attorno a questa.</p>
      {/if}
      <div class="card-actions">
        <button class="card-open" onclick={() => onOpen(panel!.id)}>{panel.kind === "note" ? "Apri l'appunto" : "Apri il paper"}</button>
        <span class="card-hint">o doppio clic sulla stella</span>
      </div>
    </aside>
  {/if}

  {#if ghostCard}
    <aside class="card ghostcard" aria-label="Scheda della scoperta">
      <button class="card-x" title="Chiudi" onclick={() => (ghostCard = null)}>×</button>
      <div class="card-meta ghostlbl">✦ Stella fantasma — trovata online</div>
      <div class="card-title">{ghostCard.title || "Senza titolo"}</div>
      <div class="card-meta">
        {ghostCard.year ?? "s.d."}
        {#if ghostCard.venue}· {ghostCard.venue}{/if}
      </div>
      {#if onGhostExplore}
        <div class="card-sec">Esplora da questa scoperta</div>
        <div class="card-explore">
          <button onclick={() => onGhostExplore(ghostCard!.key, "citations")} title="Chi cita e chi è citato da questa scoperta (OpenAlex)">Citazioni</button>
          <button onclick={() => onGhostExplore(ghostCard!.key, "similar")} title="Paper simili per argomento (OpenAlex)">Simili</button>
          <button disabled={!ghostCard.author} onclick={() => onGhostExplore(ghostCard!.key, "author")} title={ghostCard.author ? `Altri lavori di ${ghostCard.author}` : "Autore non noto per questa scoperta"}>Autore</button>
        </div>
        <p class="card-ghosthint">Le nuove stelle si agganciano a questa, in catena — puoi continuare a scavare senza aggiungere nulla.</p>
      {/if}
      <div class="card-actions">
        {#if ghostCard.inLibrary}
          <span class="ghost-in">✓ Già nella libreria</span>
        {:else if ghostCard.added}
          <span class="ghost-in">✓ Aggiunto — entrerà nel grafo al prossimo aggiornamento dell'indice</span>
        {:else if onGhostAdd}
          <button class="card-open" onclick={() => onGhostAdd(ghostCard!.key)}>Aggiungi alla libreria</button>
        {/if}
      </div>
    </aside>
  {/if}

  {#if tip}
    <div class="tip" style:left={tip.x + "px"} style:top={tip.y + "px"}>
      <div class="tip-title">{tip.title}</div>
      <div class="tip-meta">{tip.meta}</div>
    </div>
  {/if}

  {#if loading && !graph}
    <div class="state">
      <div class="dots"><span></span><span></span><span></span></div>
      <p>Calcolo la mappa semantica…</p>
    </div>
  {:else if !graph}
    <div class="state">
      <h3>Mappa non disponibile</h3>
      <p>Non sono riuscito a caricare il grafo semantico. Controlla che il backend sia attivo e riprova.</p>
      <button class="cta" onclick={onRefresh}>Riprova</button>
    </div>
  {:else if graph.embedded < 2}
    <div class="state">
      <h3>La costellazione ha bisogno dell'indice semantico</h3>
      <p>
        Per disegnare la mappa servono gli embedding dei documenti: ogni stella è un documento e i legami
        nascono dalla somiglianza dei contenuti. Genera l'indice per accendere il cielo.
      </p>
      <button class="cta" disabled={generating} onclick={startGenerate}>
        {generating ? "Avvio…" : "Genera indice"}
      </button>
    </div>
  {:else if graph.nodes.length >= 2 && graph.edges.length === 0}
    <div class="hint">Nessun legame sopra la soglia — genera più embedding o riduci la soglia col ricalcolo</div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    touch-action: none;
  }
  .chip {
    position: absolute;
    z-index: 3;
    pointer-events: none;
    font-size: 11px;
    color: var(--dim);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    backdrop-filter: blur(6px);
    border: 1px solid var(--border);
    border-radius: var(--r-pill, 999px);
    padding: 4px 11px;
    white-space: nowrap;
  }
  .counts {
    top: 12px;
    left: 12px;
  }
  .legend {
    top: 44px;
    left: 12px;
    font-size: 10.5px;
    color: var(--faint);
  }
  .hud {
    position: absolute;
    right: 12px;
    top: 12px;
    z-index: 3;
    display: flex;
    gap: 6px;
  }
  /* SOLO i bottoni diretti della barra: `.hud button` (0,1,1) vinceva per
     specificità su `.srchit` (0,1,0) e schiacciava le righe del dropdown di
     ricerca in quadrati 26×26 col testo traboccante. */
  .hud > button {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--r-sm, 8px);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    backdrop-filter: blur(6px);
    color: var(--dim);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
    padding: 0;
  }
  .hud > button:hover {
    color: var(--accent);
    border-color: var(--accent-soft2);
  }
  .hud > button.on {
    color: var(--accent);
    border-color: var(--accent);
  }
  .hudsel {
    height: 26px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm, 8px);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    backdrop-filter: blur(6px);
    color: var(--dim);
    font-size: 11px;
    padding: 0 4px;
    cursor: pointer;
  }
  .hudsel:hover { color: var(--accent); border-color: var(--accent-soft2); }
  /* Search box + candidate dropdown (leftmost in the HUD). */
  .srch { position: relative; }
  .srchin {
    height: 26px;
    width: 190px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm, 8px);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    backdrop-filter: blur(6px);
    color: var(--text);
    font-size: 11.5px;
    padding: 0 22px 0 9px;
    outline: none;
  }
  .srchin::placeholder { color: var(--faint); }
  .srchin:focus { border-color: var(--accent); }
  .srchx {
    position: absolute;
    right: 3px;
    top: 4px;
    width: 18px;
    height: 18px;
    border: none;
    background: none;
    cursor: pointer;
    color: var(--faint);
    font-size: 13px;
    line-height: 1;
    padding: 0;
  }
  .srchx:hover { color: var(--danger); }
  .srchlist {
    position: absolute;
    top: 30px;
    right: 0; /* ancorata al bordo destro dell'input: si allarga verso la mappa */
    z-index: 6;
    width: min(440px, 72vw);
    max-height: 320px;
    overflow-y: auto;
    overflow-x: hidden; /* mai frecce/scroll orizzontali: i titoli vanno a capo */
    scrollbar-width: thin; /* scrollbar verticale sottile, senza frecce */
    scrollbar-color: color-mix(in srgb, var(--dim) 32%, transparent) transparent;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border);
    border-radius: var(--r-md, 11px);
    box-shadow: var(--shadow-md, 0 4px 16px rgba(20, 22, 28, 0.09));
  }
  .srchnone { padding: 8px 10px; font-size: 11.5px; color: var(--faint); }
  .srchit {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    width: 100%;
    box-sizing: border-box;
    text-align: left;
    border: none;
    background: none;
    cursor: pointer;
    padding: 6px 9px;
    border-radius: 7px;
    font-family: inherit;
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
  }
  .srchit.selrow { background: var(--accent-soft, rgba(43, 74, 120, 0.1)); }
  .srchkind { flex: none; font-size: 9px; padding-top: 3px; }
  .srchtitle {
    flex: 1;
    min-width: 0;
    line-height: 1.35;
    overflow-wrap: anywhere; /* anche i titoli-nomefile senza spazi vanno a capo */
    display: -webkit-box;
    -webkit-line-clamp: 2; /* i titoli lunghi vanno su due righe, poi … */
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .srchyear { flex: none; padding-top: 2px; font-size: 10.5px; color: var(--dim); font-variant-numeric: tabular-nums; }
  /* Density tuning panel, anchored below the HUD (top-right). */
  .tune {
    position: absolute;
    right: 12px;
    top: 50px;
    z-index: 4;
    width: 220px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border);
    border-radius: var(--r-md, 11px);
    box-shadow: var(--shadow-md, 0 4px 16px rgba(20, 22, 28, 0.09));
    font-size: 11.5px;
    color: var(--dim);
  }
  .tune label { display: flex; flex-direction: column; gap: 3px; }
  .tune label b { color: var(--text); font-variant-numeric: tabular-nums; }
  .tune input[type="range"] { width: 100%; accent-color: var(--accent); }
  .tunerow { display: flex; gap: 8px; justify-content: flex-end; }
  .tuneapply {
    border: none; cursor: pointer; background: var(--accent);
    color: var(--on-accent, #fff); border-radius: var(--r-pill, 999px);
    padding: 5px 14px; font-size: 11.5px; font-weight: 600;
  }
  .tunecancel {
    border: 1px solid var(--border); background: none; cursor: pointer;
    color: var(--dim); border-radius: var(--r-pill, 999px); padding: 5px 12px; font-size: 11.5px;
  }
  .card-peer { color: #2e9e63; }
  /* Pinned info card: paper details + its similarity links (below the HUD). */
  .card {
    position: absolute;
    top: 50px;
    right: 12px;
    z-index: 5;
    width: min(300px, calc(100% - 24px));
    max-height: calc(100% - 100px);
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border);
    border-radius: var(--r-md, 11px);
    box-shadow: var(--shadow-md, 0 4px 16px rgba(20, 22, 28, 0.09));
  }
  .card-x {
    position: absolute;
    top: 6px;
    right: 8px;
    border: none;
    background: none;
    color: var(--faint);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 2px 4px;
  }
  .card-x:hover { color: var(--danger); }
  .card-title {
    font-family: var(--serif, Georgia, serif);
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    line-height: 1.35;
    padding-right: 16px;
  }
  .card-meta { margin-top: 3px; font-size: 11px; color: var(--dim); }
  .card-authors { margin-top: 5px; font-size: 11.5px; color: var(--text); opacity: 0.85; line-height: 1.35; }
  .card-venue { margin-top: 2px; font-size: 11px; font-style: italic; color: var(--dim); }
  .card-tags { margin-top: 6px; display: flex; flex-wrap: wrap; gap: 4px; }
  .card-tag {
    font-size: 10px;
    color: var(--dim);
    border: 1px solid var(--border);
    border-radius: var(--r-pill, 999px);
    padding: 1px 7px;
  }
  .card-sec {
    margin-top: 10px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--faint);
  }
  .card-links { margin-top: 4px; overflow-y: auto; min-height: 0; }
  .card-link {
    display: flex;
    gap: 7px;
    align-items: baseline;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    cursor: pointer;
    padding: 4px 2px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text);
  }
  .card-link:hover { background: var(--accent-soft, rgba(43, 74, 120, 0.08)); }
  .card-link-w {
    flex: none;
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    color: var(--accent);
    font-weight: 600;
    min-width: 32px;
  }
  .card-link-t {
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card-none { margin: 4px 2px; font-size: 11.5px; color: var(--faint); }
  .card-explore { display: flex; gap: 6px; margin-top: 4px; }
  .card-explore button {
    flex: 1; border: 1px solid var(--border); background: none; cursor: pointer;
    color: var(--dim); border-radius: var(--r-pill, 999px); padding: 5px 0; font-size: 11.5px;
  }
  .card-explore button:hover { color: var(--accent); border-color: var(--accent); }
  .card-ghosthint { margin: 6px 0 0; font-size: 10.5px; color: var(--faint); }
  .ghostcard { border-style: dashed; }
  .ghostlbl { color: var(--accent); }
  .ghost-in { font-size: 12px; color: #2e9e63; line-height: 1.4; }
  .chipx {
    pointer-events: auto; border: none; background: none; cursor: pointer;
    color: var(--faint); font-size: 12px; padding: 0 2px; line-height: 1;
  }
  .chipx:hover { color: var(--danger); }
  .card-actions { margin-top: 10px; display: flex; align-items: center; gap: 8px; }
  .card-open {
    border: none;
    cursor: pointer;
    background: var(--accent);
    color: var(--on-accent, #fff);
    border-radius: var(--r-pill, 999px);
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 600;
  }
  .card-open:hover { background: var(--accent-strong, var(--accent)); }
  .card-hint { font-size: 10.5px; color: var(--faint); }
  .tip {
    position: absolute;
    z-index: 4;
    pointer-events: none;
    max-width: 240px;
    padding: 8px 11px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border);
    border-radius: var(--r-md, 11px);
    box-shadow: var(--shadow-md, 0 4px 16px rgba(20, 22, 28, 0.09));
  }
  .tip-title {
    font-family: var(--serif, Georgia, serif);
    font-size: 12.5px;
    color: var(--text);
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .tip-meta {
    margin-top: 3px;
    font-size: 11px;
    color: var(--dim);
  }
  .state {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    text-align: center;
    padding: 32px;
  }
  .state h3 {
    margin: 0;
    font-family: var(--serif, Georgia, serif);
    font-size: 19px;
    font-weight: 600;
    color: var(--text);
    max-width: 440px;
  }
  .state p {
    margin: 0;
    color: var(--dim);
    font-size: 13px;
    line-height: 1.55;
    max-width: 420px;
  }
  .cta {
    margin-top: 12px;
    border: none;
    cursor: pointer;
    background: var(--accent);
    color: var(--on-accent, #fff);
    border-radius: var(--r-pill, 999px);
    padding: 9px 22px;
    font-size: 13.5px;
    font-weight: 600;
    box-shadow: var(--shadow-sm, 0 1px 3px rgba(20, 22, 28, 0.08));
  }
  .cta:hover:not(:disabled) {
    background: var(--accent-strong, var(--accent));
  }
  .cta:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .dots {
    display: flex;
    gap: 7px;
    margin-bottom: 8px;
  }
  .dots span {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .dots span:nth-child(2) {
    animation-delay: 0.18s;
  }
  .dots span:nth-child(3) {
    animation-delay: 0.36s;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.25;
      transform: scale(0.8);
    }
    50% {
      opacity: 1;
      transform: scale(1.05);
    }
  }
  .hint {
    position: absolute;
    top: 14px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 2;
    pointer-events: none;
    max-width: min(82%, 520px);
    font-size: 11px;
    color: var(--dim);
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    backdrop-filter: blur(6px);
    border: 1px solid var(--border);
    border-radius: var(--r-pill, 999px);
    padding: 5px 13px;
    text-align: center;
  }
</style>
