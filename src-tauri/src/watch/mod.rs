//! Watched-folder import: a `notify` watcher that imports PDFs added to a
//! chosen directory and tells the frontend to refresh.

use crate::{import, AppState};
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Paths currently being imported, to coalesce the flurry of events a single
/// file write produces.
static INFLIGHT: Lazy<Mutex<HashSet<PathBuf>>> = Lazy::new(Default::default);

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

/// Import a single watched file after a short settle delay, retrying the read
/// while the file may still be locked by the writer. Notifies the UI on success.
fn import_watched(app: &AppHandle, path: PathBuf) {
    std::thread::sleep(Duration::from_millis(800));

    let thumb_dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("thumbnails"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_thumbnails"));
    let state = app.state::<AppState>();

    // Retry prepare a few times (sharing violation while still being written).
    let mut prepared = None;
    for _ in 0..4 {
        match import::prepare_import(&state.pdfium, &thumb_dir, &path) {
            Ok(p) => {
                prepared = Some(p);
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(700)),
        }
    }

    if let Some(prepared) = prepared {
        let outcome = {
            let conn = state.db.lock();
            import::commit_import(&conn, &prepared)
        };
        if matches!(outcome, Ok(o) if o.imported) {
            let _ = app.emit("library-changed", ());
        }
    }

    INFLIGHT.lock().unwrap().remove(&path);
}

/// Import every PDF already present in `dir` that isn't in the library yet.
/// Runs in a background thread and emits `library-changed` as files land, so
/// the UI fills in progressively. Already-imported files are skipped by hash.
pub fn scan_existing(app: AppHandle, dir: String) {
    std::thread::spawn(move || {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let thumb_dir = app
            .path()
            .app_data_dir()
            .map(|d| d.join("thumbnails"))
            .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_thumbnails"));

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_pdf(&path) {
                continue;
            }
            // Don't race with the live watcher for the same file.
            if !INFLIGHT.lock().unwrap().insert(path.clone()) {
                continue;
            }
            let state = app.state::<AppState>();
            if let Ok(prepared) = import::prepare_import(&state.pdfium, &thumb_dir, &path) {
                let outcome = {
                    let conn = state.db.lock();
                    import::commit_import(&conn, &prepared)
                };
                if matches!(outcome, Ok(o) if o.imported) {
                    let _ = app.emit("library-changed", ());
                }
            }
            INFLIGHT.lock().unwrap().remove(&path);
        }
    });
}

/// Start watching `dir` for new PDFs. Dropping the returned watcher stops it.
pub fn start(app: AppHandle, dir: &str) -> Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };
        if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            return;
        }
        for path in event.paths {
            if !is_pdf(&path) {
                continue;
            }
            // Coalesce duplicate events for the same path.
            if !INFLIGHT.lock().unwrap().insert(path.clone()) {
                continue;
            }
            let app = app.clone();
            std::thread::spawn(move || import_watched(&app, path));
        }
    })
    .context("creating filesystem watcher")?;

    watcher
        .watch(Path::new(dir), RecursiveMode::NonRecursive)
        .with_context(|| format!("watching {dir}"))?;
    Ok(watcher)
}
