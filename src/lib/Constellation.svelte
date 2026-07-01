<script lang="ts">
  import { onMount } from "svelte";

  // ----- Data contract (local structural copy of $lib/api's SimilarityGraph: no coupling) -----
  interface GraphNode {
    id: number;
    title: string | null;
    year: number | null;
    color: string | null;
    degree: number;
    unread: boolean;
    favorite: boolean;
  }
  interface GraphEdge {
    a: number;
    b: number;
    w: number; // cosine similarity 0..1
  }
  interface SimilarityGraph {
    nodes: GraphNode[];
    edges: GraphEdge[];
    embedded: number;
    total: number;
  }

  let {
    graph,
    loading,
    selected = [],
    onOpen,
    onContext,
    onToggleSelect,
    onGenerate,
    onRefresh,
  }: {
    graph: SimilarityGraph | null;
    loading: boolean;
    selected?: number[];
    onOpen: (id: number) => void;
    onContext: (e: MouseEvent, id: number) => void;
    onToggleSelect: (id: number) => void;
    onGenerate: () => void;
    onRefresh: () => void;
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
  const REPEL = 1300;
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
    topLabelIds.clear();
    hoverIdx = -1;
    hoverSet = null;
    dragIdx = -1;
    repCursor = 0;
    tip = null;
    if (!g || g.embedded < 2 || g.nodes.length === 0) {
      alpha = 0;
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
    for (const gn of g.nodes) {
      const prev = old.get(gn.id);
      let x: number;
      let y: number;
      if (prev) {
        x = prev.x;
        y = prev.y;
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
        x,
        y,
        vx: 0,
        vy: 0,
        r: 4.5 + Math.min(gn.degree, 9) * 1.1,
      });
    }
    for (const ge of g.edges) {
      const ai = idToIdx.get(ge.a);
      const bi = idToIdx.get(ge.b);
      if (ai === undefined || bi === undefined || ai === bi) continue;
      edges.push({ ai, bi, w: ge.w, rest: 70 + (1 - ge.w) * 90, k: 0.04 * ge.w });
      let la = adj.get(ge.a);
      if (!la) adj.set(ge.a, (la = []));
      la.push(ge.b);
      let lb = adj.get(ge.b);
      if (!lb) adj.set(ge.b, (lb = []));
      lb.push(ge.a);
    }
    for (const gn of [...g.nodes].sort((a, b) => b.degree - a.degree).slice(0, 12)) topLabelIds.add(gn.id);
    fx = new Float32Array(nodes.length);
    fy = new Float32Array(nodes.length);
    alpha = hadNodes ? 0.25 : 1; // re-heat gently on refresh, fully on first build
    needFit = !hadNodes;
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
    const f = Math.min(12, REPEL / d2);
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
      const ax = fx[i] - nd.x * 0.012; // centering gravity toward the origin
      const ay = fy[i] - nd.y * 0.012;
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
  function draw() {
    if (!ctx || vw === 0 || vh === 0) return;
    const c = ctx;
    c.setTransform(dpr, 0, 0, dpr, 0, 0);
    c.clearRect(0, 0, vw, vh);
    if (nodes.length === 0) return;

    const hovId = hoverIdx >= 0 && hoverIdx < nodes.length ? nodes[hoverIdx].id : -1;
    const focus = hovId >= 0 ? hoverSet : null;

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
      let al = 0.1 + e.w * 0.28;
      if (focus && a.id !== hovId && b.id !== hovId) al *= 0.18;
      c.globalAlpha = al;
      c.lineWidth = e.w > 0.75 ? 1.5 : 1;
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
      c.globalAlpha = focus && !focus.has(n.id) ? 0.18 : 1;
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
      c.fillStyle = n.color || paint.nodeFill;
      c.beginPath();
      c.arc(sx, sy, rs, 0, TAU);
      c.fill();
      c.strokeStyle = paint.nodeStroke;
      c.lineWidth = 1;
      c.stroke();
      if (n.favorite) drawSparkle(c, sx + rs * 0.85, sy - rs * 0.85, Math.max(3.5, rs * 0.36));
    }
    c.globalAlpha = 1;

    // Labels: hovered node + neighbors, plus the 12 highest-degree nodes when zoomed in
    const labelIds = new Set<number>();
    if (hovId >= 0) {
      labelIds.add(hovId);
      for (const nb of adj.get(hovId) ?? []) labelIds.add(nb);
    }
    if (zoom > 0.55) for (const id of topLabelIds) labelIds.add(id);
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
        const la = focus && !focus.has(id) ? 0.3 : 1;
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
        meta: n.year !== null ? `${n.year} · ${conn}` : conn,
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
      setHover(hitTest(e.offsetX, e.offsetY), e.offsetX, e.offsetY);
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
    if (wasClick && idx >= 0 && idx < nodes.length) {
      const id = nodes[idx].id;
      if (e.ctrlKey || e.metaKey) onToggleSelect(id);
      else onOpen(id);
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
    if (hitTest(e.offsetX, e.offsetY) < 0) fitToView(true);
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
      schedule();
    });
    ro.observe(container);
    mo = new MutationObserver(() => {
      readTheme();
      schedule();
    });
    mo.observe(document.body, { attributes: true, attributeFilter: ["data-theme"] });
    canvas.addEventListener("wheel", onWheel, { passive: false });
    armDprWatch();
    return () => {
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
    <div class="chip counts">{countsText}</div>
    <div class="chip legend">◦ da leggere (alone acceso) · ✦ preferito · colore = tag dominante</div>
    <div class="hud">
      <button title="Adatta alla vista" onclick={() => fitToView(true)}>⤢</button>
      <button title="Ingrandisci" onclick={() => zoomAt(vw / 2, vh / 2, 1.3)}>+</button>
      <button title="Riduci" onclick={() => zoomAt(vw / 2, vh / 2, 1 / 1.3)}>−</button>
      <button title="Ricarica il grafo" onclick={onRefresh}>↻</button>
    </div>
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
    bottom: 12px;
    left: 12px;
    font-size: 10.5px;
    color: var(--faint);
  }
  .hud {
    position: absolute;
    right: 12px;
    bottom: 12px;
    z-index: 3;
    display: flex;
    gap: 6px;
  }
  .hud button {
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
  .hud button:hover {
    color: var(--accent);
    border-color: var(--accent-soft2);
  }
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
