<script lang="ts">
  // ============================================================================
  // PLANCIA — il monitor di sistema di Scriptorium (finestra separata).
  //
  // Regola d'onestà: OGNI luce corrisponde a un processo reale (evento `pulse`
  // dal backend, o un evento di progresso già esistente). Da fermo: spento.
  //
  // Modello auto-risanante: gli eventi di progresso ESTERNI non incrementano
  // contatori (nessun accoppiamento fragile) — accendono il nodo con una
  // scadenza (extUntil); se il segnale cessa, la luce muore da sola. Solo le
  // coppie start/ok del bus `pulse` muovono il contatore `busy`.
  // ============================================================================
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { save } from "@tauri-apps/plugin-dialog";

  // ---- modello dati -----------------------------------------------------------
  interface PulseEvent {
    id: number;
    node: string;
    state: "start" | "ok" | "err" | "warn" | "blip" | "progress";
    label: string;
    detail: string | null;
    done: number | null;
    total: number | null;
    ts: number;
  }
  interface Gates {
    discovery: boolean;
    ai_enabled: boolean;
    ai_provider: string;
    ai_model: string;
    watched_folder: string | null;
    connector: boolean;
    mathocr_ready: boolean;
    tatr_ready: boolean;
  }
  interface Stats {
    docs: number;
    trash: number;
    refs_only: number;
    embedded: number;
    notes: number;
    rag_docs: number;
    rag_chunks: number;
    db_mb: number;
    backup_age_days: number | null;
    projects: number;
  }

  interface NodeState {
    busy: number;            // coppie start/ok del bus pulse
    busySince: number;       // ts backend del primo start della serie
    extUntil: number;        // luce a scadenza da progressi esterni (ms locali)
    blipUntil: number;
    err: string | null;      // ultima avaria (fino a presa visione o nuovo ok)
    errAt: number;
    label: string;           // etichetta dell'ultimo lavoro visto
    prog: { done: number; total: number } | null;
    lastOkAt: number;
    lastOkDetail: string | null;
    lastDur: number;         // durata (ms) dell'ultimo lavoro concluso
    nStart: number;
    nOk: number;
    nErr: number;
    /// id del più recente ok/err applicato: uno start con id MINORE è già stato
    /// chiuso — se lo incontriamo dopo (replay/race) non deve riaccendere busy.
    lastCloseId: number;
  }

  // ---- la mappa formale dei processi interni ---------------------------------
  interface NodeDef { id: string; x: number; y: number; w: number; h: number; label: string; sub: string; gate?: string }
  const NODES: NodeDef[] = [
    { id: "import",   x: 60,  y: 130, w: 170, h: 54, label: "IMPORT",      sub: "file · trascina" },
    { id: "cartella", x: 60,  y: 230, w: 170, h: 54, label: "CARTELLA",    sub: "sorvegliata", gate: "watched" },
    { id: "browser",  x: 60,  y: 330, w: 170, h: 54, label: "BROWSER",     sub: "connettore", gate: "connector" },
    { id: "biblio",   x: 60,  y: 430, w: 170, h: 54, label: "BIBLIOTECHE", sub: "Zotero · RIS · CSL" },
    { id: "latex",    x: 60,  y: 530, w: 170, h: 54, label: "LATEX",       sub: "progetti · compile" },
    { id: "scoperta", x: 60,  y: 660, w: 170, h: 54, label: "SCOPERTA",    sub: "novità · online", gate: "rete" },
    { id: "estrazione", x: 400, y: 180, w: 180, h: 54, label: "ESTRAZIONE", sub: "pdfium" },
    // NB: metadati SENZA gate "rete" — l'arricchimento usa il suo client e
    // funziona anche con la Scoperta disattivata (il gate qui mentirebbe).
    { id: "metadati",   x: 400, y: 330, w: 180, h: 54, label: "METADATI",  sub: "Crossref · OpenAlex" },
    { id: "miniature",  x: 400, y: 480, w: 180, h: 54, label: "MINIATURE", sub: "copertine" },
    { id: "ocr",        x: 400, y: 580, w: 180, h: 54, label: "OCR",       sub: "scansioni" },
    { id: "formule",    x: 400, y: 680, w: 180, h: 54, label: "FORMULE",   sub: "math-OCR", gate: "mathocr" },
    { id: "tabelle",    x: 400, y: 780, w: 180, h: 54, label: "TABELLE",   sub: "TATR", gate: "tatr" },
    { id: "db",        x: 760, y: 300, w: 210, h: 84, label: "DATABASE",  sub: "SQLite · FTS5 · vec" },
    { id: "archivio",  x: 760, y: 470, w: 210, h: 54, label: "ARCHIVIO",  sub: "papers · note · progetti" },
    { id: "backup",    x: 760, y: 590, w: 210, h: 54, label: "BACKUP",    sub: "copie · ripristino" },
    { id: "terminale", x: 760, y: 780, w: 210, h: 54, label: "TERMINALE", sub: "PowerShell" },
    { id: "embed",     x: 1160, y: 120, w: 190, h: 54, label: "EMBEDDING", sub: "bge-m3 locale" },
    { id: "rag",       x: 1160, y: 240, w: 190, h: 54, label: "INDICE RAG", sub: "passaggi per «Chiedi»" },
    { id: "chiedi",    x: 1160, y: 360, w: 190, h: 54, label: "CHIEDI",    sub: "domande alla libreria", gate: "ai" },
    { id: "wiki",      x: 1160, y: 480, w: 190, h: 54, label: "WIKI",      sub: "sintesi per concetto", gate: "ai" },
    { id: "riassunti", x: 1160, y: 600, w: 190, h: 54, label: "RIASSUNTI", sub: "AI locale", gate: "ai" },
    { id: "refdoi",    x: 1160, y: 720, w: 190, h: 54, label: "DOI RIF.",  sub: "backfill riferimenti", gate: "rete" },
  ];

  // Cosa fa ogni sottosistema (pannello dettaglio).
  const INFO: Record<string, string> = {
    import: "Importa PDF scelti a mano o trascinati: lettura, hash anti-doppione, estrazione testo, copertina, commit nel database.",
    cartella: "Sorveglia la cartella scelta nelle Impostazioni: ogni PDF che vi appare viene importato da solo (senza mai ripescare ciò che hai cestinato).",
    browser: "Il connettore browser: il bookmarklet manda l'URL del PDF aperto e Scriptorium lo scarica e importa con lo stesso motore (protetto anti-SSRF).",
    biblio: "Import da gestori bibliografici (Zotero, Mendeley, EndNote, JabRef…): legge .bib/.ris/CSL-JSON, aggancia i PDF, converte le keyword in tag, deduplica per DOI e contenuto.",
    latex: "I progetti LaTeX: import degli .zip (PDF + bibliografia nel grafo) e compilazione con Tectonic/latexmk.",
    scoperta: "Il lato online: ricerche su arXiv/OpenAlex/ADS…, lo sweep «Novità» delle ricerche salvate, l'aggiunta di paper trovati.",
    estrazione: "Il motore pdfium: estrae testo e rende le pagine. Serializzato: un documento alla volta, per stabilità.",
    metadati: "Identità dei documenti: DOI dal testo, titolo/autori/anno da Crossref e OpenAlex, con i filtri di precisione (mai etichettare col paper sbagliato).",
    miniature: "Le copertine della griglia, rese da pdfium e salvate in cache.",
    ocr: "Riconoscimento testo per i PDF scansionati (immagini → testo cercabile).",
    formule: "Math-OCR locale (pix2tex via ONNX): un ritaglio di formula diventa LaTeX. I modelli si scaricano al primo uso.",
    tabelle: "Riconoscimento struttura tabelle (TATR): un ritaglio diventa righe e colonne vere.",
    db: "Il cuore: SQLite con ricerca full-text (FTS5) e vettoriale (sqlite-vec). Ogni luce qui è una modifica alla libreria.",
    archivio: "I file veri su disco: papers/, note .md, progetti LaTeX. Vivono in %APPDATA%, separati dal programma.",
    backup: "Copie di sicurezza complete e ripristino (validato, atomico, con copia pre-ripristino).",
    terminale: "La PowerShell integrata, aperta nella cartella dei PDF.",
    embed: "Vettori semantici bge-m3 (locale, ONNX): alimentano Correlati, ricerca per significato e Costellazione.",
    rag: "L'indice a passaggi per «Chiedi alla libreria»: spezza i documenti e li vettorizza per il recupero citato.",
    chiedi: "Domande alla tua libreria: recupero dei passaggi pertinenti + risposta del modello locale con citazioni [n].",
    wiki: "Genera pagine di sintesi per concetto dai tuoi paper, con fonti.",
    riassunti: "Riassunti e confronti AI dei singoli documenti (modello locale via Ollama/LM Studio).",
    refdoi: "Backfill dei DOI dei riferimenti citati: ricerca su Crossref con il filtro di precisione (mai un DOI sbagliato).",
  };

  type Edge = { from: string; to: string; when: string[] };
  const EDGES: Edge[] = [
    { from: "import",   to: "estrazione", when: ["import"] },
    { from: "cartella", to: "estrazione", when: ["cartella"] },
    { from: "browser",  to: "estrazione", when: ["browser"] },
    { from: "biblio",   to: "estrazione", when: ["biblio"] },
    { from: "latex",    to: "estrazione", when: ["latex"] },
    { from: "scoperta", to: "db",         when: ["scoperta"] },
    { from: "estrazione", to: "db",        when: ["import", "cartella", "browser", "biblio", "latex", "estrazione"] },
    { from: "estrazione", to: "miniature", when: ["miniature"] },
    { from: "estrazione", to: "ocr",       when: ["ocr"] },
    { from: "estrazione", to: "formule",   when: ["formule"] },
    { from: "estrazione", to: "tabelle",   when: ["tabelle"] },
    { from: "metadati",  to: "db",        when: ["metadati"] },
    { from: "miniature", to: "db",        when: ["miniature"] },
    { from: "ocr",       to: "db",        when: ["ocr"] },
    { from: "db",       to: "archivio",   when: ["backup", "archivio"] },
    { from: "db",       to: "backup",     when: ["backup"] },
    { from: "db",       to: "embed",      when: ["embed"] },
    { from: "embed",    to: "rag",        when: ["rag"] },
    { from: "rag",      to: "chiedi",     when: ["chiedi"] },
    { from: "db",       to: "chiedi",     when: ["chiedi"] },
    { from: "db",       to: "wiki",       when: ["wiki"] },
    { from: "db",       to: "riassunti",  when: ["riassunti"] },
    { from: "db",       to: "refdoi",     when: ["refdoi"] },
  ];

  const EXT_TTL = 30_000; // luce a scadenza per i progressi esterni

  // ---- stato reattivo ---------------------------------------------------------
  function freshNode(): NodeState {
    return {
      busy: 0, busySince: 0, extUntil: 0, blipUntil: 0, err: null, errAt: 0,
      label: "", prog: null, lastOkAt: 0, lastOkDetail: null, lastDur: 0,
      nStart: 0, nOk: 0, nErr: 0, lastCloseId: -1,
    };
  }
  let nodes = $state<Record<string, NodeState>>(Object.fromEntries(NODES.map((n) => [n.id, freshNode()])));
  let gates = $state<Gates | null>(null);
  let stats = $state<Stats | null>(null);
  let packets = $state<{ key: string; d: string }[]>([]);
  const sessionStart = Date.now();
  let log = $state<PulseEvent[]>([]);
  let alert = $state<{ node: string; label: string; detail: string } | null>(null);
  let selected = $state<string | null>(null);      // nodo aperto nel pannello
  let logFilter = $state<"tutti" | "errori">("tutti");
  let logToFile = $state(false);
  let exportMsg = $state("");
  let nowTick = $state(Date.now());
  let clockOffset = 0;

  const MAXLOG = 300;
  const seen = new Set<number>();

  function nodeDef(id: string): NodeDef | undefined {
    return NODES.find((n) => n.id === id);
  }

  function apply(ev: PulseEvent, live: boolean) {
    if (seen.has(ev.id)) return;
    seen.add(ev.id);
    if (seen.size > 1500) pruneSeen();
    const st = nodes[ev.node];
    if (!st) return; // nodo non mappato: mai inventare
    st.label = ev.label;
    switch (ev.state) {
      case "start":
        st.nStart += 1;
        // Guarigione indipendente dall'ordine: se questo start è più VECCHIO
        // dell'ultima chiusura vista (replay dello snapshot, evento perso e
        // riconsegnato dopo), il suo lavoro è già finito — non riaccendere.
        if (ev.id < st.lastCloseId) break;
        if (st.busy === 0) st.busySince = ev.ts;
        st.busy += 1;
        st.err = null;
        break;
      case "progress":
        if (ev.done != null && ev.total != null) st.prog = { done: ev.done, total: ev.total };
        if (live) st.extUntil = Date.now() + EXT_TTL;
        break;
      case "ok":
        st.lastCloseId = Math.max(st.lastCloseId, ev.id);
        if (st.busy > 0 && st.busySince) st.lastDur = Math.max(0, ev.ts - st.busySince);
        st.busy = Math.max(0, st.busy - 1);
        if (st.busy === 0) { st.prog = null; st.extUntil = 0; st.busySince = 0; }
        st.err = null;
        st.lastOkAt = ev.ts;
        st.lastOkDetail = ev.detail;
        st.nOk += 1;
        break;
      case "err":
        st.lastCloseId = Math.max(st.lastCloseId, ev.id);
        if (st.busy > 0 && st.busySince) st.lastDur = Math.max(0, ev.ts - st.busySince);
        st.busy = Math.max(0, st.busy - 1);
        if (st.busy === 0) { st.prog = null; st.extUntil = 0; st.busySince = 0; }
        st.err = ev.detail ?? "errore";
        st.errAt = ev.ts;
        st.nErr += 1;
        if (live) alert = { node: ev.node, label: ev.label, detail: ev.detail ?? "" };
        break;
      case "warn":
        // Problema non terminale dentro un job: conta e mostra, NON chiude la coppia.
        st.nErr += 1;
        st.err = ev.detail ?? ev.label;
        st.errAt = ev.ts;
        if (live) alert = { node: ev.node, label: ev.label, detail: ev.detail ?? "" };
        break;
      case "blip":
        // Solo dal vivo: i blip di ore fa non devono accendere lo schema all'apertura.
        if (live) st.blipUntil = Date.now() + 1600;
        break;
    }
    // Coreografia: un impulso che VIAGGIA lungo la traccia per ogni evento
    // concluso dal vivo — un pacchetto = un fatto reale, mai un loop decorativo.
    if (live && (ev.state === "ok" || ev.state === "blip")) spawnPackets(ev.node);
    log.push(ev);
    if (log.length > MAXLOG) log.splice(0, log.length - MAXLOG);
  }

  function spawnPackets(node: string) {
    if (packets.length >= 10) return; // budget: la scena resta fluida
    const inbound = EDGES.filter((e) => e.to === node).slice(0, 2);
    for (const e of inbound) {
      const key = node + "-" + Math.random().toString(36).slice(2);
      packets.push({ key, d: edgePath(e) });
      setTimeout(() => (packets = packets.filter((p) => p.key !== key)), 1200);
    }
  }

  function pruneSeen() {
    let max = -1;
    for (const id of seen) if (id > max) max = id;
    for (const id of [...seen]) if (id < max - 700) seen.delete(id);
  }

  // Progressi già esistenti nell'app → luce a scadenza, MAI contatori.
  function extProgress(node: string, done: number, total: number, phase?: string) {
    const st = nodes[node];
    if (!st) return;
    if (phase === "done" || phase === "cancelled") {
      st.prog = null;
      st.extUntil = 0;
      return;
    }
    st.prog = { done, total };
    st.extUntil = Date.now() + EXT_TTL;
  }
  function blipNode(node: string, label?: string) {
    const st = nodes[node];
    if (!st) return;
    st.blipUntil = Date.now() + 1600;
    if (label) st.label = label;
  }

  // ---- derive -----------------------------------------------------------------
  const SOURCES = ["import", "cartella", "browser", "biblio", "latex"];
  function isActive(id: string): boolean {
    const st = nodes[id];
    if (st.busy > 0 || st.extUntil > nowTick || st.blipUntil > nowTick) return true;
    // L'estrazione gira DENTRO le pipeline sorgente (stesso lock pdfium):
    // quando una sorgente lavora, lo stadio estrazione sta lavorando davvero.
    if (id === "estrazione") return SOURCES.some((s) => nodes[s].busy > 0);
    return false;
  }
  function gateOff(n: NodeDef): string | null {
    if (!gates || !n.gate) return null;
    switch (n.gate) {
      case "rete":      return gates.discovery ? null : "online disattivato";
      case "ai":        return gates.ai_enabled ? null : "AI disattivata";
      case "watched":   return gates.watched_folder ? null : "nessuna cartella";
      case "connector": return gates.connector ? null : "connettore spento";
      case "mathocr":   return gates.mathocr_ready ? null : "modelli da scaricare";
      case "tatr":      return gates.tatr_ready ? null : "modelli da scaricare";
      default: return null;
    }
  }
  function nodeClass(n: NodeDef): string {
    const st = nodes[n.id];
    // Un warn durante un job non spegne la luce: nodo acceso con spia ambra;
    // l'AVARIA piena (rossa) è per un nodo fermo col suo ultimo esito fallito.
    if (isActive(n.id)) return st.err ? "on warnlit" : "on";
    if (st.err) return "err";
    if (gateOff(n)) return "off";
    return "idle";
  }
  function edgeActive(e: Edge): boolean {
    return e.when.some((id) => isActive(id));
  }
  function edgePath(e: Edge): string {
    const a = nodeDef(e.from)!;
    const b = nodeDef(e.to)!;
    const ax = a.x + a.w, ay = a.y + a.h / 2;
    const bx = b.x,        by = b.y + b.h / 2;
    if (ax > b.x + b.w) {
      const ax2 = a.x, mid = (ax2 + (b.x + b.w)) / 2;
      return `M ${ax2} ${ay} L ${mid} ${ay} L ${mid} ${by} L ${b.x + b.w} ${by}`;
    }
    const mid = (ax + bx) / 2;
    if (Math.abs(ay - by) < 4) return `M ${ax} ${ay} L ${bx} ${by}`;
    return `M ${ax} ${ay} L ${mid} ${ay} L ${mid} ${by} L ${bx} ${by}`;
  }

  function fmtTime(ts: number): string {
    // ts è nell'orologio del backend; locale = backend − offset (offset = backend − locale).
    return new Date(ts - clockOffset).toLocaleTimeString("it-IT", { hour12: false });
  }
  function fmtDur(ms: number): string {
    if (ms < 1000) return "<1s";
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ${s % 60}s`;
    return `${Math.floor(m / 60)}h ${m % 60}m`;
  }
  /** Da quanto è in lavorazione: adesso-in-orologio-backend = nowTick + offset. */
  function elapsed(st: NodeState): string {
    if (!st.busySince) return "";
    return fmtDur(Math.max(0, nowTick + clockOffset - st.busySince));
  }

  const activeCount = $derived(NODES.filter((n) => isActive(n.id)).length);
  const errCount = $derived(log.filter((e) => e.state === "err").length);
  const uptime = $derived(fmtDur(Math.max(0, nowTick - sessionStart)));
  const eventsPerMin = $derived(log.filter((e) => e.ts > nowTick + clockOffset - 60_000).length);
  /** DIAGNOSTICA: col pannello aperto, restano vivi il nodo e i suoi vicini. */
  const related = $derived.by(() => {
    if (!selected) return null;
    const s = new Set([selected]);
    for (const e of EDGES) {
      if (e.from === selected) s.add(e.to);
      if (e.to === selected) s.add(e.from);
    }
    return s;
  });

  const fmtN = (n: number) => n.toLocaleString("it-IT");
  /** Il readout VERO del nodo in quiete (stile «CURRENT TEMP — 19°C», ma onesto). */
  function readout(n: NodeDef): string {
    if (!stats) return n.sub;
    switch (n.id) {
      case "db":       return `${fmtN(stats.docs)} doc · ${fmtN(stats.trash)} cestino · ${stats.db_mb} MB`;
      case "embed":    return `${fmtN(stats.embedded)}/${fmtN(stats.docs)} vettorizzati`;
      case "rag":      return stats.rag_docs > 0 ? `${fmtN(stats.rag_docs)} doc · ${fmtN(stats.rag_chunks)} passaggi` : "indice da costruire";
      case "archivio": return `${fmtN(stats.notes)} appunti · ${fmtN(stats.projects)} progetti`;
      case "backup":   return stats.backup_age_days == null ? "nessun backup" : stats.backup_age_days === 0 ? "ultimo: oggi" : `ultimo: ${stats.backup_age_days} g fa`;
      case "biblio":   return stats.refs_only > 0 ? `${fmtN(stats.refs_only)} voci solo-riferimento` : n.sub;
      case "cartella": {
        const wf = gates?.watched_folder;
        return wf ? (wf.split(/[\\/]/).pop() ?? n.sub) : n.sub;
      }
      case "chiedi":
      case "wiki":
      case "riassunti": {
        const m = gates?.ai_enabled ? gates.ai_model : "";
        return m ? m.slice(0, 22) : n.sub;
      }
      default: return n.sub;
    }
  }

  /** Gomiti del percorso: i punti di giunzione dove mettere i pallini. */
  function edgeElbows(e: Edge): [number, number][] {
    const a = nodeDef(e.from)!;
    const b = nodeDef(e.to)!;
    const ay = a.y + a.h / 2, by = b.y + b.h / 2;
    if (Math.abs(ay - by) < 4) return [];
    const ax = a.x + a.w, bx = b.x;
    const mid = ax > b.x + b.w ? (a.x + (b.x + b.w)) / 2 : (ax + bx) / 2;
    return [[mid, ay], [mid, by]];
  }

  // Titolo del pannello con effetto «decodifica» (200ms, poi testo pieno).
  let ptitle = $state("");
  const GLYPHS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#/·";
  $effect(() => {
    if (!selected) { ptitle = ""; return; }
    const target = nodeDef(selected)?.label ?? selected;
    let step = 0;
    const iv = setInterval(() => {
      step += 1;
      const solved = Math.floor((step / 7) * target.length);
      ptitle = target
        .split("")
        .map((c, i) => (i < solved || c === " " ? c : GLYPHS[Math.floor(Math.random() * GLYPHS.length)]))
        .join("");
      if (step >= 7) { ptitle = target; clearInterval(iv); }
    }, 30);
    return () => clearInterval(iv);
  });
  const visibleLog = $derived(
    (selected ? log.filter((e) => e.node === selected) : log)
      .filter((e) => (logFilter === "errori" ? e.state === "err" || e.state === "warn" : true)),
  );

  function ackErr(id: string) {
    const st = nodes[id];
    if (st?.err) st.err = null;
    if (alert?.node === id) alert = null;
  }

  async function exportLog() {
    exportMsg = "";
    try {
      const path = await save({
        title: "Salva il registro della Plancia",
        defaultPath: "plancia-registro.txt",
        filters: [{ name: "Testo", extensions: ["txt"] }],
      });
      if (!path) return;
      const n = await invoke<number>("pulse_export", { path });
      exportMsg = `salvati ${n} eventi`;
      setTimeout(() => (exportMsg = ""), 4000);
    } catch (e) {
      exportMsg = "errore: " + e;
    }
  }

  // ---- ciclo di vita ----------------------------------------------------------
  let unsubs: UnlistenFn[] = [];
  let tickI: ReturnType<typeof setInterval> | null = null;
  let gatesI: ReturnType<typeof setInterval> | null = null;

  async function refreshSnapshot(first: boolean) {
    try {
      const snap = await invoke<{ events: PulseEvent[]; gates: Gates; stats: Stats; now: number }>("pulse_snapshot");
      if (first) clockOffset = snap.now - Date.now();
      gates = snap.gates;
      stats = snap.stats;
      for (const ev of snap.events) apply(ev, false);
    } catch (e) {
      if (first) console.error("pulse_snapshot", e);
    }
    try {
      const ls = await invoke<{ enabled: boolean; dir: string }>("pulse_log_status");
      logToFile = ls.enabled;
    } catch { /* comando assente: ignora */ }
  }

  onMount(async () => {
    // Prima i listener, POI la snapshot: un evento nel mezzo non va perso
    // (il dedupe per id + lastCloseId rendono innocuo l'ordine di arrivo).
    unsubs.push(await listen<PulseEvent>("pulse", (e) => apply(e.payload, true)));
    unsubs.push(await listen<{ done: number; total: number; phase: string }>("embed-progress", (e) =>
      extProgress("embed", e.payload.done, e.payload.total, e.payload.phase)));
    unsubs.push(await listen<{ done: number; total: number; phase: string }>("rag-progress", (e) =>
      extProgress("rag", e.payload.done, e.payload.total, e.payload.phase)));
    unsubs.push(await listen<{ done: number; total: number; updated: number; phase: string }>("meta-progress", (e) =>
      extProgress("metadati", e.payload.done, e.payload.total, e.payload.phase)));
    unsubs.push(await listen<{ done: number; total: number; resolved: number }>("refdoi-progress", (e) =>
      extProgress("refdoi", e.payload.done, e.payload.total)));
    unsubs.push(await listen<{ phase: string; done: number; total: number }>("wiki-progress", (e) => {
      // Lo stesso evento serve Wiki E Confronto/Rassegna: instradalo verso il
      // nodo il cui lavoro è davvero aperto (start pulse pendente).
      const target = nodes.wiki.busy > 0 ? "wiki" : nodes.riassunti.busy > 0 ? "riassunti" : null;
      if (target) extProgress(target, e.payload.done, e.payload.total, e.payload.phase);
    }));
    unsubs.push(await listen("library-changed", () => blipNode("db", "Libreria aggiornata")));
    unsubs.push(await listen<number>("novita-changed", () => blipNode("scoperta")));
    unsubs.push(await listen<string>("connector-added", () => blipNode("browser")));
    unsubs.push(await listen<string>("ask-token", () => blipNode("chiedi")));
    unsubs.push(await listen("term-output", () => blipNode("terminale")));

    await refreshSnapshot(true);
    tickI = setInterval(() => (nowTick = Date.now()), 500);
    gatesI = setInterval(() => void refreshSnapshot(false), 15000);
  });

  onDestroy(() => {
    unsubs.forEach((u) => u());
    if (tickI) clearInterval(tickI);
    if (gatesI) clearInterval(gatesI);
  });
</script>

<svelte:head>
  <title>Plancia — Scriptorium</title>
</svelte:head>

<div class="plancia" class:alarm={alert != null}>
  <div class="vignette"></div>
  <div class="scan"></div>
  <header>
    <div class="brand">
      <span class="t1">PLANCIA</span><span class="t2">// SCRIPTORIUM</span>
      <span class="t3">SESSIONE {uptime} · {eventsPerMin} EV/MIN</span>
    </div>
    <div class="gates">
      {#if gates}
        <span class="chip {gates.discovery ? 'g-on' : 'g-off'}">RETE {gates.discovery ? "ONLINE" : "OFF"}</span>
        <span class="chip {gates.ai_enabled ? 'g-on' : 'g-off'}">AI {gates.ai_enabled ? (gates.ai_model || gates.ai_provider || "ON").toUpperCase() : "OFF"}</span>
        <span class="chip {gates.watched_folder ? 'g-on' : 'g-off'}">CARTELLA {gates.watched_folder ? "●" : "—"}</span>
        <span class="chip {gates.connector ? 'g-on' : 'g-off'}">CONNETTORE {gates.connector ? "●" : "—"}</span>
      {/if}
      {#if errCount > 0}
        <span class="chip g-err">{errCount} ERRORI</span>
      {/if}
      {#if logToFile}
        <span class="chip g-on" title="Il registro viene scritto anche su file (Impostazioni → Manutenzione)">LOG SU FILE ●</span>
      {/if}
      <span class="chip {activeCount > 0 ? 'g-run' : 'g-idle'}">{activeCount > 0 ? `${activeCount} PROCESSI ATTIVI` : "SISTEMA IN QUIETE"}</span>
      <button class="chip g-btn" onclick={exportLog} title="Salva il registro della sessione in un file di testo">SALVA REGISTRO…</button>
      {#if exportMsg}<span class="chip g-idle">{exportMsg}</span>{/if}
    </div>
  </header>

  {#if alert}
    <button class="alert glitch" onclick={() => (alert = null)} title="Chiudi">
      <span class="a1">{nodeDef(alert.node)?.label ?? alert.node} — ERRORE</span>
      <span class="a2">{alert.label}{alert.detail ? " · " + alert.detail : ""}</span>
    </button>
  {/if}

  <div class="mainrow">
    <svg viewBox="0 0 1640 980" preserveAspectRatio="xMidYMid meet" class:diag={selected != null}>
      <defs>
        <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
          <path d="M 40 0 L 0 0 0 40" fill="none" stroke="rgba(0,190,230,0.05)" stroke-width="1" />
        </pattern>
      </defs>
      <rect x="0" y="0" width="1640" height="980" fill="url(#grid)" />

      <!-- cornice: angoli a staffa + righello tacche (statici, densità da blueprint) -->
      <g class="frame">
        <path d="M14 44 V14 H44 M1596 14 H1626 V44 M1626 936 V966 H1596 M44 966 H14 V936" />
        {#each Array.from({ length: 38 }, (_, i) => i) as i (i)}
          <line x1={70 + i * 40} y1="14" x2={70 + i * 40} y2={i % 5 === 0 ? 26 : 20} />
        {/each}
        {#each Array.from({ length: 22 }, (_, i) => i) as i (i)}
          <line x1="14" y1={60 + i * 40} x2={i % 5 === 0 ? 26 : 20} y2={60 + i * 40} />
        {/each}
      </g>

      <text class="zone" x="60"   y="100">SORGENTI</text>
      <text class="zone" x="400"  y="150">ELABORAZIONE</text>
      <text class="zone" x="760"  y="270">NUCLEO</text>
      <text class="zone" x="1160" y="90">INTELLIGENZA</text>

      {#each EDGES as e (e.from + ">" + e.to)}
        {@const act = edgeActive(e)}
        {@const rel = !selected || e.from === selected || e.to === selected}
        {@const broken = nodes[e.from].err != null || nodes[e.to].err != null}
        <path class="trace" class:rel d={edgePath(e)} class:errline={broken} />
        {#each edgeElbows(e) as [jx, jy], k (k)}
          <circle class="jdot" class:jact={act} cx={jx} cy={jy} r="2.6" />
        {/each}
        {#if act}
          <path class="flow" d={edgePath(e)} />
        {/if}
      {/each}

      <!-- impulsi-pacchetto: uno per evento reale concluso -->
      {#each packets as p (p.key)}
        <circle class="packet" r="3.4">
          <animateMotion dur="0.95s" path={p.d} fill="freeze" />
        </circle>
      {/each}

      <!-- sottosistemi del nucleo: esagoni FTS5 / VEC con conteggi veri -->
      {#if stats}
        <g class="hexes">
          <line x1="970" y1="322" x2="1006" y2="308" />
          <line x1="970" y1="362" x2="1006" y2="376" />
          <g class="hex">
            <polygon points="1024,296 1038,304 1038,320 1024,328 1010,320 1010,304" />
            <text class="hexlab" x="1048" y="309">FTS5</text>
            <text class="hexval" x="1048" y="321">{fmtN(stats.docs)}</text>
          </g>
          <g class="hex">
            <polygon points="1024,364 1038,372 1038,388 1024,396 1010,388 1010,372" />
            <text class="hexlab" x="1048" y="377">VEC</text>
            <text class="hexval" x="1048" y="389">{fmtN(stats.embedded)}</text>
          </g>
        </g>
      {/if}

      {#each NODES as n (n.id)}
        {@const st = nodes[n.id]}
        {@const cls = nodeClass(n)}
        {@const off = gateOff(n)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <g
          class="node {cls}"
          class:sel={selected === n.id}
          class:rel={!related || related.has(n.id)}
          onclick={() => (selected = selected === n.id ? null : n.id)}
        >
          <rect x={n.x} y={n.y} width={n.w} height={n.h} rx="6" />
          <rect class="pin" x={n.x - 4} y={n.y + n.h / 2 - 7} width="4" height="14" rx="1" />
          <line class="stub" x1={n.x + n.w} y1={n.y + n.h / 2 - 6} x2={n.x + n.w + 6} y2={n.y + n.h / 2 - 6} />
          <line class="stub" x1={n.x + n.w} y1={n.y + n.h / 2 + 6} x2={n.x + n.w + 6} y2={n.y + n.h / 2 + 6} />
          {#if n.gate}
            <text class="gatebadge" class:gb-off={off != null} x={n.x + 2} y={n.y - 5}>{off ? "OFFLINE" : "ONLINE"}</text>
          {/if}
          <text class="nlabel" x={n.x + 14} y={n.y + 23}>{n.label}</text>
          <text class="nsub" x={n.x + 14} y={n.y + 41}>
            {#if st.busy > 0 || st.extUntil > nowTick}
              {#if st.prog}{st.prog.done}/{st.prog.total}{:else}in lavorazione{/if}{#if st.busySince} · {elapsed(st)}{/if}{#if st.err} · con errori{/if}
            {:else if st.err}
              AVARIA
            {:else if off}
              OFFLINE — {off}
            {:else}
              {readout(n)}
            {/if}
          </text>
          {#if cls.startsWith("on") && !st.err}
            <circle class="led" cx={n.x + n.w - 14} cy={n.y + 14} r="4" />
          {/if}
          {#if st.err}
            <circle class="led-err" cx={n.x + n.w - 14} cy={n.y + 14} r="4" />
          {/if}
        </g>
      {/each}
    </svg>

    {#if selected}
      {@const n = nodeDef(selected)!}
      {@const st = nodes[selected]}
      {@const off = gateOff(n)}
      <aside class="panel">
        <div class="phead">
          <span class="ptitle">{ptitle || n.label}</span>
          <button class="pclose" onclick={() => (selected = null)}>✕</button>
        </div>
        <p class="pdesc">{INFO[n.id] ?? n.sub}</p>

        <div class="pstate">
          {#if st.err}
            <div class="prow perr">AVARIA · {fmtTime(st.errAt)}</div>
            <div class="perrmsg">{st.label}{st.err ? " — " + st.err : ""}</div>
            <button class="pack" onclick={() => ackErr(n.id)}>PRESA VISIONE</button>
          {:else if st.busy > 0 || st.extUntil > nowTick}
            <div class="prow pon">IN LAVORAZIONE{st.busySince ? ` · da ${elapsed(st)}` : ""}</div>
            <div class="plabel">{st.label}{st.prog ? ` — ${st.prog.done}/${st.prog.total}` : ""}</div>
          {:else if off}
            <div class="prow poff">OFFLINE — {off}</div>
          {:else}
            <div class="prow pidle">IN QUIETE</div>
            {#if st.lastOkAt}
              <div class="plabel">ultimo esito · {fmtTime(st.lastOkAt)}{st.lastDur ? ` (${fmtDur(st.lastDur)})` : ""}{st.lastOkDetail ? ` — ${st.lastOkDetail}` : ""}</div>
            {/if}
          {/if}
        </div>

        <div class="pstats">
          <span>avvii <b>{st.nStart}</b></span>
          <span>ok <b class="okc">{st.nOk}</b></span>
          <span>errori <b class="errc">{st.nErr}</b></span>
          {#if st.lastDur}<span>ultima durata <b>{fmtDur(st.lastDur)}</b></span>{/if}
        </div>

        <div class="phist-head">STORICO DEL NODO (sessione)</div>
        <div class="phist">
          {#each log.filter((e) => e.node === n.id).slice(-40).reverse() as ev (ev.id)}
            <div class="lrow {ev.state}">
              <span class="lt">{fmtTime(ev.ts)}</span>
              <span class="ll">{ev.state === "ok" ? "✓" : ev.state === "err" ? "✗" : ev.state === "start" ? "▶" : "·"} {ev.label}{ev.state === "progress" && ev.done != null ? ` ${ev.done}/${ev.total}` : ""}{ev.detail ? " — " + ev.detail : ""}</span>
            </div>
          {:else}
            <div class="lrow idle-msg"><span class="ll">Nessun evento in questa sessione.</span></div>
          {/each}
        </div>
      </aside>
    {/if}
  </div>

  <footer>
    <div class="loghead">
      <span>REGISTRO ATTIVITÀ{selected ? ` — ${nodeDef(selected)?.label}` : ""}</span>
      <span class="lfilters">
        <button class="lf" class:active={logFilter === "tutti"} onclick={() => (logFilter = "tutti")}>TUTTI</button>
        <button class="lf" class:active={logFilter === "errori"} onclick={() => (logFilter = "errori")}>ERRORI</button>
      </span>
    </div>
    <div class="logbody">
      {#each visibleLog.slice(-80).reverse() as ev (ev.id)}
        <div class="lrow {ev.state}">
          <span class="lt">{fmtTime(ev.ts)}</span>
          <span class="ln">{nodeDef(ev.node)?.label ?? ev.node}</span>
          <span class="ll">{ev.label}{ev.state === "progress" && ev.done != null ? ` ${ev.done}/${ev.total}` : ""}{ev.detail ? " — " + ev.detail : ""}</span>
        </div>
      {:else}
        <div class="lrow idle-msg"><span class="ll">{logFilter === "errori" ? "Nessun errore registrato. Ottimo segno." : "Nessuna attività registrata da quando l'app è aperta. Lo schema si accende quando qualcosa lavora davvero."}</span></div>
      {/each}
    </div>
  </footer>
</div>

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: #04080d;
  }
  .plancia {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(1200px 600px at 70% -10%, rgba(0, 80, 110, 0.18), transparent 60%),
      #04080d;
    color: #9fdcec;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    user-select: none;
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 16px 6px;
    border-bottom: 1px solid rgba(0, 200, 240, 0.14);
    flex: none;
  }
  .brand { white-space: nowrap; }
  .brand .t1 {
    font-size: 17px;
    letter-spacing: 0.35em;
    color: #4fe3ff;
    text-shadow: 0 0 12px rgba(79, 227, 255, 0.5);
  }
  .brand .t2 {
    margin-left: 10px;
    font-size: 11px;
    letter-spacing: 0.25em;
    color: rgba(159, 220, 236, 0.45);
  }
  .brand .t3 {
    margin-left: 14px;
    font-size: 9px;
    letter-spacing: 0.2em;
    color: rgba(159, 220, 236, 0.35);
  }

  /* ---- scena: ambiente, allarme, diagnostica ---- */
  .vignette {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 4;
    opacity: 0;
    transition: opacity 0.35s ease;
    background: radial-gradient(ellipse at center, transparent 55%, rgba(255, 20, 35, 0.22) 100%);
  }
  .alarm .vignette { opacity: 1; animation: alarmBreath 1.6s ease-in-out infinite; }
  @keyframes alarmBreath { 50% { opacity: 0.55; } }

  .scan {
    position: absolute;
    left: 0;
    right: 0;
    top: -6%;
    height: 5%;
    pointer-events: none;
    z-index: 3;
    background: linear-gradient(to bottom, transparent, rgba(120, 230, 255, 0.035), transparent);
    animation: scanSweep 16s linear infinite;
  }
  @keyframes scanSweep { to { transform: translateY(2300%); } }

  .glitch { animation: alertIn 0.18s ease-out, glitchJitter 0.5s steps(2) 3; }
  @keyframes glitchJitter {
    20% { transform: translateX(calc(-50% + 3px)) skewX(1.5deg); }
    40% { transform: translateX(calc(-50% - 2px)); clip-path: inset(0 0 34% 0); }
    60% { transform: translateX(calc(-50% + 1px)) skewX(-1deg); clip-path: inset(42% 0 0 0); }
    80% { transform: translateX(-50%); clip-path: none; }
  }

  svg.diag .node:not(.rel) { opacity: 0.22; }
  svg.diag .trace:not(.rel) { opacity: 0.35; }
  svg.diag .zone, svg.diag .hexes { opacity: 0.3; }

  .frame path, .frame line {
    stroke: rgba(0, 200, 240, 0.22);
    stroke-width: 1.4;
    fill: none;
  }
  .frame line { stroke: rgba(0, 200, 240, 0.13); stroke-width: 1; }

  .jdot { fill: rgba(0, 190, 230, 0.25); }
  .jdot.jact {
    fill: #37e0ff;
    animation: ledBlink 0.8s ease-in-out infinite;
    filter: drop-shadow(0 0 3px rgba(55, 224, 255, 0.8));
  }

  .packet {
    fill: #aef4ff;
    filter: drop-shadow(0 0 6px rgba(120, 235, 255, 1));
  }

  .stub { stroke: rgba(0, 190, 230, 0.3); stroke-width: 1.2; }
  .gatebadge {
    font-size: 8px;
    letter-spacing: 0.28em;
    fill: rgba(110, 240, 192, 0.75);
  }
  .gatebadge.gb-off { fill: rgba(140, 160, 175, 0.5); }

  .hexes line { stroke: rgba(0, 190, 230, 0.25); stroke-width: 1.2; }
  .hex polygon {
    fill: rgba(6, 22, 32, 0.9);
    stroke: rgba(0, 190, 230, 0.45);
    stroke-width: 1.2;
  }
  .hexlab { font-size: 9px; letter-spacing: 0.2em; fill: rgba(159, 220, 236, 0.6); }
  .hexval { font-size: 10px; fill: #7be9ff; }

  .errline {
    stroke: rgba(255, 43, 58, 0.4);
    animation: errFlicker 1.1s steps(3) infinite;
  }
  @keyframes errFlicker { 50% { stroke: rgba(255, 43, 58, 0.12); } }
  .gates { display: flex; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
  .chip {
    font-size: 10px;
    letter-spacing: 0.12em;
    padding: 3px 8px;
    border-radius: 3px;
    border: 1px solid rgba(0, 200, 240, 0.25);
    white-space: nowrap;
    background: none;
    font-family: inherit;
  }
  .g-on   { color: #6ef0c0; border-color: rgba(110, 240, 192, 0.4); }
  .g-off  { color: rgba(159, 220, 236, 0.4); border-style: dashed; }
  .g-run  { color: #ffd166; border-color: rgba(255, 209, 102, 0.5); animation: chipPulse 1.6s ease-in-out infinite; }
  .g-idle { color: rgba(159, 220, 236, 0.55); }
  .g-err  { color: #ff5964; border-color: rgba(255, 89, 100, 0.5); }
  .g-btn  { color: #4fe3ff; cursor: pointer; }
  .g-btn:hover { background: rgba(0, 200, 240, 0.12); }
  @keyframes chipPulse { 50% { box-shadow: 0 0 10px rgba(255, 209, 102, 0.35); } }

  .alert {
    position: absolute;
    top: 54px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 5;
    background: rgba(120, 8, 16, 0.92);
    border: 1px solid #ff2b3a;
    box-shadow: 0 0 26px rgba(255, 43, 58, 0.55), inset 0 0 18px rgba(255, 43, 58, 0.25);
    color: #ffd9dc;
    padding: 10px 22px;
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    max-width: 70%;
    animation: alertIn 0.18s ease-out;
  }
  .alert .a1 {
    display: block;
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.2em;
    color: #ff5964;
  }
  .alert .a2 {
    display: block;
    font-size: 11px;
    margin-top: 3px;
    opacity: 0.9;
    overflow-wrap: anywhere;
  }
  @keyframes alertIn { from { opacity: 0; transform: translateX(-50%) translateY(-8px); } }

  .mainrow {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  svg { flex: 1; width: 100%; min-height: 0; }

  .zone {
    font-size: 11px;
    letter-spacing: 0.4em;
    fill: rgba(0, 200, 240, 0.35);
  }

  .trace { fill: none; stroke: rgba(0, 190, 230, 0.16); stroke-width: 1.6; }
  .flow {
    fill: none;
    stroke: #37e0ff;
    stroke-width: 2;
    stroke-dasharray: 4 14;
    filter: drop-shadow(0 0 4px rgba(55, 224, 255, 0.8));
    animation: flowMove 0.8s linear infinite;
  }
  @keyframes flowMove { to { stroke-dashoffset: -18; } }

  .node { cursor: pointer; }
  .node rect:not(.pin) {
    fill: rgba(6, 22, 32, 0.85);
    stroke: rgba(0, 190, 230, 0.35);
    stroke-width: 1.4;
  }
  .node.sel rect:not(.pin) { stroke-width: 2.4; }
  .node .pin { fill: rgba(0, 190, 230, 0.4); }
  .node .nlabel {
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.14em;
    fill: #bfeefb;
  }
  .node .nsub {
    font-size: 10px;
    letter-spacing: 0.06em;
    fill: rgba(159, 220, 236, 0.55);
  }

  .node.off rect:not(.pin) {
    stroke-dasharray: 5 4;
    stroke: rgba(120, 150, 165, 0.35);
    fill: rgba(8, 14, 18, 0.6);
  }
  .node.off .nlabel { fill: rgba(159, 190, 200, 0.45); }
  .node.off .nsub   { fill: rgba(159, 190, 200, 0.4); }

  .node.on rect:not(.pin) {
    stroke: #37e0ff;
    fill: rgba(9, 45, 62, 0.9);
    filter: drop-shadow(0 0 9px rgba(55, 224, 255, 0.45));
  }
  .node.on .nlabel { fill: #eaffff; }
  .node.on .nsub   { fill: #7be9ff; }
  .node.on .pin    { fill: #37e0ff; }
  .led {
    fill: #37e0ff;
    animation: ledBlink 0.9s ease-in-out infinite;
    filter: drop-shadow(0 0 5px rgba(55, 224, 255, 0.9));
  }
  @keyframes ledBlink { 50% { opacity: 0.25; } }

  .node.warnlit rect:not(.pin) {
    stroke: #ffd166;
    filter: drop-shadow(0 0 9px rgba(255, 209, 102, 0.4));
  }

  .node.err rect:not(.pin) {
    stroke: #ff2b3a;
    fill: rgba(70, 8, 14, 0.9);
    filter: drop-shadow(0 0 10px rgba(255, 43, 58, 0.5));
  }
  .node.err .nlabel { fill: #ffb9be; }
  .node.err .nsub   { fill: #ff5964; font-weight: 700; letter-spacing: 0.2em; }
  .led-err {
    fill: #ff2b3a;
    animation: ledBlink 0.5s ease-in-out infinite;
    filter: drop-shadow(0 0 6px rgba(255, 43, 58, 0.9));
  }

  /* ---- pannello dettaglio nodo ---- */
  .panel {
    width: 340px;
    flex: none;
    border-left: 1px solid rgba(0, 200, 240, 0.14);
    background: rgba(4, 12, 18, 0.92);
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    min-height: 0;
  }
  .phead { display: flex; align-items: center; justify-content: space-between; }
  .ptitle { font-size: 15px; font-weight: 700; letter-spacing: 0.2em; color: #4fe3ff; }
  .pclose {
    background: none; border: none; color: rgba(159, 220, 236, 0.6);
    cursor: pointer; font-family: inherit; font-size: 13px;
  }
  .pclose:hover { color: #4fe3ff; }
  .pdesc { font-size: 11px; line-height: 1.5; color: rgba(159, 220, 236, 0.75); margin: 8px 0 10px; }
  .pstate { margin-bottom: 10px; }
  .prow { font-size: 11px; letter-spacing: 0.15em; font-weight: 700; }
  .pon   { color: #ffd166; }
  .poff  { color: rgba(159, 190, 200, 0.5); }
  .pidle { color: rgba(159, 220, 236, 0.55); }
  .perr  { color: #ff5964; }
  .plabel { font-size: 11px; color: rgba(159, 220, 236, 0.8); margin-top: 4px; overflow-wrap: anywhere; }
  .perrmsg { font-size: 11px; color: #ffb9be; margin-top: 4px; overflow-wrap: anywhere; }
  .pack {
    margin-top: 8px;
    background: rgba(120, 8, 16, 0.6);
    border: 1px solid #ff2b3a;
    color: #ffd9dc;
    font-family: inherit;
    font-size: 10px;
    letter-spacing: 0.2em;
    padding: 4px 10px;
    border-radius: 3px;
    cursor: pointer;
  }
  .pack:hover { background: rgba(160, 10, 20, 0.7); }
  .pstats {
    display: flex; gap: 12px; flex-wrap: wrap;
    font-size: 10px; color: rgba(159, 220, 236, 0.6);
    border-top: 1px solid rgba(0, 200, 240, 0.12);
    border-bottom: 1px solid rgba(0, 200, 240, 0.12);
    padding: 7px 0;
    margin-bottom: 8px;
  }
  .pstats b { color: #bfeefb; }
  .pstats .okc { color: #6ef0c0; }
  .pstats .errc { color: #ff5964; }
  .phist-head { font-size: 9px; letter-spacing: 0.3em; color: rgba(0, 200, 240, 0.4); margin-bottom: 4px; }
  .phist { overflow-y: auto; min-height: 0; flex: 1; }

  footer {
    flex: none;
    border-top: 1px solid rgba(0, 200, 240, 0.14);
    padding: 6px 16px 10px;
    height: 168px;
    display: flex;
    flex-direction: column;
  }
  .loghead {
    font-size: 10px;
    letter-spacing: 0.4em;
    color: rgba(0, 200, 240, 0.4);
    margin-bottom: 4px;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .lfilters { display: flex; gap: 6px; }
  .lf {
    background: none;
    border: 1px solid rgba(0, 200, 240, 0.2);
    color: rgba(159, 220, 236, 0.5);
    font-family: inherit;
    font-size: 9px;
    letter-spacing: 0.2em;
    padding: 2px 8px;
    border-radius: 3px;
    cursor: pointer;
  }
  .lf.active { color: #4fe3ff; border-color: rgba(79, 227, 255, 0.6); }
  .logbody { overflow-y: auto; min-height: 0; flex: 1; }
  .lrow {
    display: flex;
    gap: 12px;
    font-size: 11px;
    line-height: 1.65;
    color: rgba(159, 220, 236, 0.6);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lrow .lt { color: rgba(159, 220, 236, 0.35); flex: none; }
  .lrow .ln { color: #4fe3ff; flex: none; min-width: 105px; letter-spacing: 0.08em; }
  .lrow .ll { overflow: hidden; text-overflow: ellipsis; }
  .lrow.err { color: #ff8b94; }
  .lrow.err .ln { color: #ff5964; }
  .lrow.warn { color: #ffd9a0; }
  .lrow.warn .ln { color: #ffd166; }
  .lrow.ok .ln { color: #6ef0c0; }
  .lrow.start .ln { color: #ffd166; }
  .lrow.idle-msg { color: rgba(159, 220, 236, 0.35); font-style: italic; white-space: normal; }
</style>
