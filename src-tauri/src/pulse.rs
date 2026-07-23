//! pulse — il bus di attività della «Plancia».
//!
//! Formalizza i processi interni dell'app come nodi di uno schema (import,
//! estrazione, metadati, embedding, RAG, AI, backup…) e trasmette in tempo
//! reale cosa parte, cosa avanza, cosa finisce e — soprattutto — cosa fallisce
//! e perché. La finestra «Plancia» (webview separata, `open_plancia`) ascolta
//! l'evento `pulse` e anima SOLO ciò che sta lavorando davvero: niente
//! telemetria finta, da fermo lo schema è spento.
//!
//! Regole:
//! - `start`/`done` vanno sempre in coppia (done riceve il Result e sceglie
//!   ok/err da solo); `blip` è per eventi puntuali senza durata.
//! - Ogni emissione è best-effort (`let _`): la Plancia non deve MAI poter
//!   rompere o rallentare un job vero. Il payload è piccolo e già formattato.
//! - Un ring buffer (cap. 300) conserva la storia recente così la finestra,
//!   quando si apre, parte già informata (`pulse_snapshot`).

use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Un evento di attività. `node` è uno degli id formalizzati (vedi la mappa
/// nella Plancia): import, cartella, browser, biblio, latex, scoperta,
/// estrazione, metadati, miniature, ocr, formule, tabelle, db, archivio,
/// backup, embed, rag, chiedi, wiki, riassunti, refdoi, terminale.
#[derive(Clone, Serialize)]
pub struct PulseEvent {
    pub id: u64,
    pub node: String,
    /// "start" | "ok" | "err" | "blip" | "progress"
    pub state: String,
    pub label: String,
    pub detail: Option<String>,
    pub done: Option<u64>,
    pub total: Option<u64>,
    /// ms dall'epoch (orologio del backend: un'unica fonte di verità).
    pub ts: u64,
}

#[derive(Default)]
pub struct PulseState {
    buf: parking_lot::Mutex<VecDeque<PulseEvent>>,
    next: AtomicU64,
    /// Specchio del setting `pulse_log_enabled`: quando è on, ogni evento viene
    /// anche accodato (best-effort) a un file JSONL giornaliero in `logs/`.
    log_to_file: AtomicBool,
}

const CAP: usize = 600;
/// File di log giornalieri conservati (i più vecchi vengono potati all'avvio).
const LOG_KEEP: usize = 14;

fn log_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("logs"))
}

/// Data civile YYYY-MM-DD da ms epoch (algoritmo civil_from_days, niente dipendenze).
fn civil_date(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn append_to_file(app: &AppHandle, ev: &PulseEvent) {
    let Some(dir) = log_dir(app) else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("plancia-{}.jsonl", civil_date(ev.ts)));
    let Ok(mut line) = serde_json::to_string(ev) else { return };
    line.push('\n');
    // Una sola write_all per riga: con più thread che accodano, writeln! (due
    // scritture) può interlacciare mezze righe e corrompere il JSONL.
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// All'avvio: carica il flag dal settings e pota i log più vecchi di LOG_KEEP giorni.
pub fn init(app: &AppHandle) {
    let enabled = {
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock();
        conn.query_row(
            "SELECT value FROM settings WHERE key = 'pulse_log_enabled'",
            [],
            |r| r.get::<_, String>(0),
        )
        .map(|v| v == "1")
        .unwrap_or(false)
    };
    if let Some(ps) = app.try_state::<PulseState>() {
        ps.log_to_file.store(enabled, Ordering::Relaxed);
    }
    if let Some(dir) = log_dir(app) {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("plancia-") && n.ends_with(".jsonl"))
                })
                .collect();
            files.sort(); // il nome è la data: ordine lessicografico = cronologico
            while files.len() > LOG_KEEP {
                let old = files.remove(0);
                let _ = std::fs::remove_file(old);
            }
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn push(
    app: &AppHandle,
    node: &str,
    state: &str,
    label: &str,
    detail: Option<String>,
    done: Option<u64>,
    total: Option<u64>,
) {
    let Some(ps) = app.try_state::<PulseState>() else { return };
    let ev = PulseEvent {
        id: ps.next.fetch_add(1, Ordering::Relaxed),
        node: node.into(),
        state: state.into(),
        label: label.into(),
        detail,
        done,
        total,
        ts: now_ms(),
    };
    {
        let mut buf = ps.buf.lock();
        if buf.len() >= CAP {
            buf.pop_front();
        }
        buf.push_back(ev.clone());
    }
    if ps.log_to_file.load(Ordering::Relaxed) {
        append_to_file(app, &ev);
    }
    let _ = app.emit("pulse", ev);
}

/// Un lavoro con durata è partito.
pub fn start(app: &AppHandle, node: &str, label: &str) {
    push(app, node, "start", label, None, None, None);
}

/// Chiude il lavoro aperto con `start` in base al Result (ok oppure err col perché).
pub fn done<T, E: std::fmt::Display>(app: &AppHandle, node: &str, label: &str, r: &Result<T, E>) {
    match r {
        Ok(_) => push(app, node, "ok", label, None, None, None),
        Err(e) => push(app, node, "err", label, Some(e.to_string()), None, None),
    }
}

/// Variante di `done` con dettaglio in caso di successo (es. conteggi).
pub fn ok(app: &AppHandle, node: &str, label: &str, detail: &str) {
    push(app, node, "ok", label, Some(detail.to_string()), None, None);
}

/// Errore esplicito (quando non c'è un Result a portata di mano).
pub fn err(app: &AppHandle, node: &str, label: &str, detail: &str) {
    push(app, node, "err", label, Some(detail.to_string()), None, None);
}

/// Evento puntuale senza durata (un file arrivato, un documento toccato…).
pub fn blip(app: &AppHandle, node: &str, label: &str) {
    push(app, node, "blip", label, None, None, None);
}

/// Problema NON terminale dentro un lavoro in corso (es. un file su cento
/// fallito): visibile nel registro e sul nodo, ma NON chiude la coppia
/// start/ok — il job prosegue e chiuderà da solo.
pub fn warn(app: &AppHandle, node: &str, label: &str, detail: &str) {
    push(app, node, "warn", label, Some(detail.to_string()), None, None);
}

/// Come [`done`], ma un annullamento chiesto dall'utente non è un'avaria:
/// se il messaggio d'errore è un «annullato/interrotto», chiude con ok.
pub fn done_user<T>(app: &AppHandle, node: &str, label: &str, r: &Result<T, String>) {
    match r {
        Ok(_) => push(app, node, "ok", label, None, None, None),
        Err(e) if e.to_lowercase().contains("annullat") || e.to_lowercase().contains("interrott") => {
            push(app, node, "ok", label, Some(e.clone()), None, None)
        }
        Err(e) => push(app, node, "err", label, Some(e.clone()), None, None),
    }
}

/// Avanzamento di un lavoro in corso.
pub fn progress(app: &AppHandle, node: &str, label: &str, done_n: u64, total: u64) {
    push(app, node, "progress", label, None, Some(done_n), Some(total));
}

// ---- comandi per la finestra Plancia ---------------------------------------

/// I «gate»: cosa spiega perché un nodo è spento (non è un errore: è OFF).
#[derive(Serialize)]
pub struct PulseGates {
    pub discovery: bool,
    pub ai_enabled: bool,
    pub ai_provider: String,
    pub ai_model: String,
    pub watched_folder: Option<String>,
    pub connector: bool,
    pub mathocr_ready: bool,
    pub tatr_ready: bool,
}

/// Numeri VERI per i readout dei nodi (stile «CURRENT TEMP — 19°C», ma onesti).
#[derive(Serialize, Default)]
pub struct PulseStats {
    pub docs: i64,
    pub trash: i64,
    pub refs_only: i64,
    pub embedded: i64,
    pub notes: i64,
    pub rag_docs: i64,
    pub rag_chunks: i64,
    pub db_mb: u64,
    pub backup_age_days: Option<i64>,
    pub projects: i64,
}

#[derive(Serialize)]
pub struct PulseSnapshot {
    pub events: Vec<PulseEvent>,
    pub gates: PulseGates,
    pub stats: PulseStats,
    pub now: u64,
}

#[tauri::command]
pub async fn pulse_snapshot(app: AppHandle) -> Result<PulseSnapshot, String> {
    // Sul thread blocking: prende il lock del DB e tocca il filesystem, e la
    // Plancia lo polla ogni 15s — mai sul main thread (freeze UI se il DB è occupato).
    tauri::async_runtime::spawn_blocking(move || pulse_snapshot_inner(&app))
        .await
        .map_err(|e| e.to_string())?
}

fn pulse_snapshot_inner(app: &AppHandle) -> Result<PulseSnapshot, String> {
    let state = app.state::<crate::AppState>();
    let (discovery, ai_enabled, ai_provider, ai_model, watched_folder, mut stats) = {
        let conn = state.db.lock();
        let get = |k: &str| -> Option<String> {
            conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| r.get(0))
                .ok()
        };
        let count = |sql: &str| -> i64 {
            conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
        };
        let stats = PulseStats {
            docs: count("SELECT COUNT(*) FROM documents WHERE deleted_at IS NULL"),
            trash: count("SELECT COUNT(*) FROM documents WHERE deleted_at IS NOT NULL"),
            refs_only: count("SELECT COUNT(*) FROM documents WHERE deleted_at IS NULL AND path LIKE 'ref:%'"),
            embedded: count("SELECT COUNT(*) FROM doc_vec"),
            notes: count("SELECT COUNT(*) FROM notes"),
            rag_docs: count("SELECT COUNT(DISTINCT document_id) FROM doc_chunks"),
            rag_chunks: count("SELECT COUNT(*) FROM doc_chunks"),
            ..Default::default()
        };
        (
            get("discovery_enabled").as_deref() == Some("1"),
            get("ai_enabled").as_deref() == Some("1"),
            get("ai_provider").unwrap_or_default(),
            get("ai_model").unwrap_or_default(),
            get("watched_folder"),
            stats,
        )
    };
    let connector = state.connector.lock().is_some();
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let mathocr_ready = crate::mathocr::models_present(&data_dir.join("mathocr"));
    let tatr_ready = crate::tablestruct::models_present(&data_dir.join("tablestruct"));
    // Numeri dal filesystem (fuori dal lock DB): dimensione DB, età backup, progetti.
    stats.db_mb = std::fs::metadata(data_dir.join("pdfmanage.db"))
        .map(|m| m.len() / (1024 * 1024))
        .unwrap_or(0);
    stats.backup_age_days = std::fs::read_dir(data_dir.join("backups"))
        .ok()
        .and_then(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                .max()
        })
        .and_then(|newest| newest.elapsed().ok())
        .map(|age| (age.as_secs() / 86_400) as i64);
    stats.projects = std::fs::read_dir(data_dir.join("projects"))
        .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count() as i64)
        .unwrap_or(0);
    let events: Vec<PulseEvent> = {
        let ps = app.state::<PulseState>();
        let buf = ps.buf.lock();
        buf.iter().cloned().collect()
    };
    Ok(PulseSnapshot {
        events,
        gates: PulseGates {
            discovery,
            ai_enabled,
            ai_provider,
            ai_model,
            watched_folder,
            connector,
            mathocr_ready,
            tatr_ready,
        },
        stats,
        now: now_ms(),
    })
}

#[derive(Serialize)]
pub struct PulseLogStatus {
    pub enabled: bool,
    pub dir: String,
}

/// Stato del log-su-file della Plancia (per Impostazioni → Manutenzione).
#[tauri::command]
pub fn pulse_log_status(app: AppHandle) -> PulseLogStatus {
    let enabled = app
        .try_state::<PulseState>()
        .map(|ps| ps.log_to_file.load(Ordering::Relaxed))
        .unwrap_or(false);
    let dir = log_dir(&app).map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    PulseLogStatus { enabled, dir }
}

/// Attiva/disattiva il log-su-file (persistito nel settings, effetto immediato).
#[tauri::command]
pub fn set_pulse_log(app: AppHandle, enabled: bool) -> Result<PulseLogStatus, String> {
    // Prima le precondizioni (cartella scrivibile), POI la persistenza e il flag:
    // se qualcosa fallisce qui, lo stato non è cambiato e la checkbox resta vera.
    if enabled {
        let dir = log_dir(&app).ok_or("cartella dati non disponibile")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    {
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('pulse_log_enabled', ?1)",
            [if enabled { "1" } else { "0" }],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(ps) = app.try_state::<PulseState>() {
        ps.log_to_file.store(enabled, Ordering::Relaxed);
    }
    if enabled {
        blip(&app, "db", "Log della Plancia su file: attivato");
    }
    Ok(pulse_log_status(app))
}

/// Esporta il buffer corrente (max 300 eventi) come testo leggibile nel file scelto.
#[tauri::command]
pub fn pulse_export(app: AppHandle, path: String) -> Result<usize, String> {
    let events: Vec<PulseEvent> = {
        let ps = app.state::<PulseState>();
        let buf = ps.buf.lock();
        buf.iter().cloned().collect()
    };
    let mut out = String::with_capacity(events.len() * 96);
    out.push_str("# Scriptorium — registro Plancia (sessione corrente)\n");
    for ev in &events {
        let ms = ev.ts % 86_400_000;
        let (h, m, s) = (ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60);
        out.push_str(&format!(
            "{} {h:02}:{m:02}:{s:02}Z  {:<10} {:<8} {}{}{}\n",
            civil_date(ev.ts),
            ev.node,
            ev.state,
            ev.label,
            match (ev.done, ev.total) {
                (Some(d), Some(t)) => format!(" [{d}/{t}]"),
                _ => String::new(),
            },
            ev.detail.as_deref().map(|d| format!(" — {d}")).unwrap_or_default(),
        ));
    }
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(events.len())
}

/// Apre la cartella dei log nel file explorer (argomento passato diretto, niente shell).
#[tauri::command]
pub fn pulse_reveal_logs(app: AppHandle) -> Result<(), String> {
    let dir = log_dir(&app).ok_or("cartella dati non disponibile")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Percorso qualificato: mai risolvere "explorer.exe" dalla search path (CWE-427).
    let explorer = std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("explorer.exe"))
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows\explorer.exe"));
    std::process::Command::new(explorer)
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Apre (o riporta in primo piano) la finestra Plancia.
#[tauri::command]
pub async fn open_plancia(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("plancia") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    // NB: "plancia" senza estensione — il file emesso è plancia.html ma il router
    // di SvelteKit fa match sul percorso /plancia (con .html mostrerebbe il 404).
    tauri::WebviewWindowBuilder::new(
        &app,
        "plancia",
        tauri::WebviewUrl::App("plancia".into()),
    )
    .title("Plancia — Scriptorium")
    .inner_size(1240.0, 800.0)
    .min_inner_size(900.0, 600.0)
    .build()
    .map_or_else(
        |e| {
            // Doppio click ravvicinato: se intanto la finestra è nata, non è un errore.
            if let Some(w) = app.get_webview_window("plancia") {
                let _ = w.set_focus();
                Ok(())
            } else {
                Err(e.to_string())
            }
        },
        |_| Ok(()),
    )
}
