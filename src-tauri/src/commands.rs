//! Tauri command handlers exposed to the Svelte frontend.

use crate::ai;
use crate::bibtex;
use crate::citation;
use crate::connector;
use crate::discovery;
use crate::embed;
use crate::github;
use crate::import;
use crate::mathocr;
use crate::mdexport;
use crate::metadata;
use crate::notes;
use crate::obsidian;
use crate::pdf;
use crate::projects;
use crate::rag;
use crate::secret;
use crate::table;
use crate::tablestruct;
use crate::term;
use crate::wiki;
use crate::model::{Annotation, Collection, Document, EditableMeta, ImportSummary, Tag};
use crate::AppState;
use zerocopy::IntoBytes;
use base64::prelude::{Engine as _, BASE64_STANDARD};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

fn thumb_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("thumbnails"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_thumbnails"))
}

/// Import one or more PDF files by absolute path. Runs off the UI thread.
#[tauri::command]
pub async fn import_files(app: AppHandle, paths: Vec<String>) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let dir = thumb_dir(&app);
        let mut summary = ImportSummary {
            imported: Vec::new(),
            duplicates: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        for p in paths {
            // Slow work (read, hash, pdfium extract/render) WITHOUT the DB lock, but
            // serialized against other pdfium document operations (startup scan etc.).
            let prepared = {
                let _pdf_guard = state.pdfium_lock.lock();
                import::prepare_import(&state.pdfium, &dir, Path::new(&p))
            };
            let prepared = match prepared {
                Ok(pr) => pr,
                Err(e) => {
                    summary.errors.push(format!("{p}: {e:#}"));
                    continue;
                }
            };
            let text_failed = prepared.text_failed;
            // Short-lived lock just for the commit.
            let outcome = {
                let conn = state.db.lock();
                import::commit_import(&conn, &prepared)
            };
            match outcome {
                Ok(o) if o.imported => {
                    summary.imported.push(o.document_id);
                    if text_failed {
                        summary
                            .warnings
                            .push(format!("{p}: testo non estratto (PDF protetto o corrotto?)"));
                    }
                }
                Ok(o) => summary.duplicates.push(o.document_id),
                Err(e) => summary.errors.push(format!("{p}: {e:#}")),
            }
        }
        Ok::<_, String>(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// List all documents, newest first, with their authors.
#[tauri::command]
pub fn list_documents(
    state: State<'_, AppState>,
    tag_id: Option<i64>,
    collection_id: Option<i64>,
    flag: Option<String>,
) -> Result<Vec<Document>, String> {
    let conn = state.db.lock();
    query_documents(&conn, tag_id, collection_id, flag.as_deref()).map_err(|e| e.to_string())
}

/// Fetch a document's cached thumbnail as a PNG data URL (or `None`).
#[tauri::command]
pub fn get_thumbnail(state: State<'_, AppState>, id: i64) -> Result<Option<String>, String> {
    let path: Option<String> = {
        let conn = state.db.lock();
        conn.query_row(
            "SELECT thumb_path FROM documents WHERE id = ?1",
            params![id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten()
    };
    match path {
        Some(p) if !p.is_empty() => {
            let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
            Ok(Some(format!(
                "data:image/png;base64,{}",
                BASE64_STANDARD.encode(bytes)
            )))
        }
        _ => Ok(None),
    }
}

/// Re-render every PDF document's cover thumbnail at the current resolution
/// ([`pdf::THUMB_WIDTH`]), overwriting the cached PNGs. Lets an existing library —
/// whose covers were rendered at the old lower width — look crisp when the grid is
/// zoomed in. Heavy (one pdfium render per document), so it runs off the async
/// runtime; returns how many covers were regenerated.
#[tauri::command]
pub async fn rebuild_thumbnails(app: AppHandle) -> Result<usize, String> {
    let dir = thumb_dir(&app);
    tokio::task::spawn_blocking(move || -> Result<usize, String> {
        std::fs::create_dir_all(&dir).ok();
        let state = app.state::<AppState>();
        // Snapshot the work list, then render WITHOUT holding the DB lock.
        let rows: Vec<(i64, String, Option<String>, Option<String>)> = {
            let conn = state.db.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT id, path, file_hash, thumb_path FROM documents \
                     WHERE deleted_at IS NULL AND path NOT LIKE 'ref:%'",
                )
                .map_err(|e| e.to_string())?;
            let it = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map_err(|e| e.to_string())?;
            it.filter_map(Result::ok).collect()
        };
        let mut done = 0usize;
        for (id, path, hash, thumb_path) in rows {
            let src = std::path::Path::new(&path);
            if !src.is_file() {
                continue;
            }
            // Overwrite the existing cover file, else key a fresh one by file hash.
            let out = match thumb_path.filter(|t| !t.trim().is_empty()) {
                Some(t) => std::path::PathBuf::from(t),
                None => dir.join(format!("{}.png", hash.unwrap_or_else(|| id.to_string()))),
            };
            if pdf::render_thumbnail(&state.pdfium, src, &out, pdf::THUMB_WIDTH).is_ok() {
                let conn = state.db.lock();
                let _ = conn.execute(
                    "UPDATE documents SET thumb_path = ?1 WHERE id = ?2",
                    params![out.to_string_lossy().to_string(), id],
                );
                done += 1;
            }
        }
        Ok(done)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Result of a metadata-enrichment batch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichSummary {
    pub updated: usize,
    pub no_doi: usize,
    /// DOIs whose Crossref title did not match the PDF (a cited work, not this
    /// document) — skipped instead of overwriting with the wrong paper.
    pub skipped_mismatch: usize,
    pub errors: Vec<String>,
}

/// Enrich every document that has no DOI yet: find a DOI in its text, look it
/// up on Crossref, and write back the bibliographic metadata.
#[tauri::command]
pub async fn enrich_all(app: AppHandle) -> Result<EnrichSummary, String> {
    // Collect candidates up front, then release the DB lock before any network I/O.
    let candidates: Vec<(i64, Option<String>, String)> = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare("SELECT id, doi, COALESCE(fulltext, '') FROM documents WHERE doi IS NULL AND deleted_at IS NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    let email = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty())
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let mut summary = EnrichSummary {
        updated: 0,
        no_doi: 0,
        skipped_mismatch: 0,
        errors: Vec::new(),
    };

    for (id, existing, fulltext) in candidates {
        // Whether the PDF carried *any* DOI at all — used only to classify a
        // non-resolution as "cited-work DOI" vs "no DOI" for the summary.
        let had_doi = existing.is_some() || metadata::extract_doi(&fulltext).is_some();
        // Resolve identity from the paper's OWN title (a scavenged citation DOI
        // can no longer rename the document). Bounded so one slow lookup can't hang.
        let resolved = match tokio::time::timeout(
            std::time::Duration::from_secs(45),
            metadata::resolve_document_meta(&client, &fulltext, existing.as_deref(), email.as_deref()),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                summary.errors.push(format!("id {id}: {e:#}"));
                None
            }
            Err(_) => {
                summary.errors.push(format!("id {id}: timeout"));
                None
            }
        };
        match resolved {
            Some(r) => {
                let state = app.state::<AppState>();
                let mut conn = state.db.lock();
                match apply_resolved(&mut conn, id, &r) {
                    Ok(()) => {
                        // Refresh the stored citekey now that authors/year/title are known.
                        let _ = crate::db::citekey::auto_citekey(&conn, id);
                        summary.updated += 1;
                    }
                    Err(e) => summary.errors.push(format!("id {id}: {e:#}")),
                }
            }
            None if had_doi => summary.skipped_mismatch += 1,
            None => summary.no_doi += 1,
        }
        // Be polite to the online APIs between documents.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    Ok(summary)
}

/// Apply metadata resolved by [`metadata::resolve_document_meta`]: with a DOI use
/// the full upsert (keeps references + caches the record); without one (an arXiv
/// preprint) write the bibliographic fields and leave the DOI null.
fn apply_resolved(conn: &mut Connection, id: i64, r: &metadata::Resolved) -> anyhow::Result<()> {
    match r.doi.as_deref() {
        Some(doi) => metadata::apply_metadata(conn, id, doi, &r.meta),
        None => write_repaired(conn, id, &r.meta),
    }
}

/// Result of a one-shot metadata repair pass.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct RepairSummary {
    /// Documents whose stored title did not match their PDF (mis-enriched).
    pub checked: usize,
    /// Correct metadata recovered from arXiv via the id in the filename.
    pub repaired_arxiv: usize,
    /// Full record re-resolved online (Crossref/arXiv) from the recovered title —
    /// fixes docs mislabelled with a cited work's metadata.
    pub resolved_online: usize,
    /// Title recovered from the PDF's first line (no arXiv id available).
    pub retitled: usize,
    /// Wrong metadata blanked because no title could be recovered.
    pub cleared: usize,
    pub details: Vec<String>,
}

/// Overwrite a mis-enriched document's bibliographic fields and clear its wrong
/// DOI and reference list. `meta.authors` replaces the author list (empty list
/// = authors cleared). The FTS triggers re-index the new title automatically.
fn write_repaired(conn: &mut Connection, id: i64, meta: &metadata::CrossrefMeta) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE documents SET title = ?1, year = ?2, venue = ?3, abstract = ?4, doi = NULL WHERE id = ?5",
        params![meta.title, meta.year, meta.venue, meta.abstract_text, id],
    )?;
    metadata::set_authors(&tx, id, &meta.authors)?;
    tx.execute("DELETE FROM document_references WHERE document_id = ?1", params![id])?;
    tx.commit()?;
    Ok(())
}

/// Update a document's bibliographic fields (title/year/venue/abstract/authors)
/// while PRESERVING its DOI and reference list. Used when an online title search
/// resolves to an arXiv record (no DOI) for a document that may already hold a
/// valid DOI — so repair never removes a good identifier.
fn write_bib_only(conn: &mut Connection, id: i64, meta: &metadata::CrossrefMeta) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE documents SET title = COALESCE(?1, title), year = ?2, venue = ?3, abstract = ?4 WHERE id = ?5",
        params![meta.title, meta.year, meta.venue, meta.abstract_text, id],
    )?;
    metadata::set_authors(&tx, id, &meta.authors)?;
    tx.commit()?;
    Ok(())
}

/// Re-verify documents and fix any whose stored title does not match the PDF —
/// the result of enrichment latching onto a *cited* work's DOI (or a previous
/// imperfect recovery). A document is left untouched only when it has a real DOI
/// AND its title matches the start of the PDF (a confidently-correct record).
/// Everything else is re-derived: arXiv papers from the arXiv record (id taken
/// from the FILENAME, never the body text), others from the PDF's first line.
/// Every recovery must pass the title gate before it is saved, and each network
/// call is bounded by a hard timeout so the pass can never hang. Idempotent.
#[tauri::command]
pub async fn repair_metadata(app: AppHandle) -> Result<RepairSummary, String> {
    // (id, doi, title, path, head) for every on-disk document — gathered off-lock.
    let candidates: Vec<(i64, Option<String>, Option<String>, String, String)> = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, doi, title, path, substr(COALESCE(fulltext,''),1,1500) \
                 FROM documents \
                 WHERE deleted_at IS NULL AND path NOT LIKE 'ref:%'",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    let email = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty())
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let mut sum = RepairSummary::default();

    for (id, doi, title, path, head) in candidates {
        let title_s = title.unwrap_or_default();
        // A confidently-correct record: real DOI + title matching the PDF. Leave it.
        let protected =
            doi.is_some() && !title_s.trim().is_empty() && metadata::title_matches_doc(&title_s, &head);
        if protected {
            continue;
        }
        let fname = std::path::Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = std::path::Path::new(&fname)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_filename_title = !stem.is_empty() && title_s.trim().eq_ignore_ascii_case(stem.trim());

        // (a) arXiv is authoritative for arXiv papers — recover from the id in the
        //     filename, bounded by a hard timeout so a slow request can't hang.
        if let Some(aid) = metadata::arxiv_id_from_filename(&fname) {
            let fetched =
                tokio::time::timeout(std::time::Duration::from_secs(20), metadata::fetch_arxiv(&client, &aid)).await;
            tokio::time::sleep(std::time::Duration::from_millis(800)).await; // be gentle to arXiv
            if let Ok(Ok(Some(meta))) = fetched {
                if meta.title.as_deref().is_some_and(|t| metadata::title_matches_doc(t, &head)) {
                    let changed = doi.is_some() || meta.title.as_deref() != Some(title_s.as_str());
                    let state = app.state::<AppState>();
                    let mut conn = state.db.lock();
                    // The arXiv record (matched by filename id + head) is authoritative:
                    // rewrite title/authors and clear any stale DOI/refs. We can't tell a
                    // genuinely-published DOI from a wrongly-scavenged one here (arXiv
                    // gives no DOI), so we don't freeze a possibly-wrong DOI.
                    match write_repaired(&mut conn, id, &meta) {
                        Ok(()) => {
                            let _ = crate::db::citekey::auto_citekey(&conn, id);
                            if changed {
                                sum.repaired_arxiv += 1;
                                sum.checked += 1;
                                sum.details
                                    .push(format!("id {id}: arXiv {aid} → {}", meta.title.as_deref().unwrap_or("")));
                            }
                        }
                        Err(e) => sum.details.push(format!("id {id}: errore scrittura: {e:#}")),
                    }
                    continue;
                }
            }
        }

        // (b) No arXiv recovery. Never-enriched docs (title == filename) are left
        //     alone — there's nothing wrong to fix and a first-line guess could be
        //     junk. A doc with a *wrong* real title gets a title from the PDF.
        if is_filename_title {
            continue;
        }

        // (b1) Re-resolve the correct record ONLINE from the paper's own title.
        //      This repairs a doc mislabelled with a cited work's metadata when
        //      the real paper IS indexed. We pass the FULL fulltext (so DOI
        //      scanning matches the enrich path) and the STORED DOI (so a still-
        //      valid DOI is re-validated and preserved, not discarded).
        let full: String = {
            let state = app.state::<AppState>();
            let conn = state.db.lock();
            conn.query_row("SELECT COALESCE(fulltext,'') FROM documents WHERE id = ?1", params![id], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| head.clone())
        };
        let online = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            metadata::resolve_document_meta(&client, &full, doi.as_deref(), email.as_deref()),
        )
        .await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await; // be gentle to the APIs
        match online {
            Ok(Ok(Some(r))) => {
                let new_title = r.meta.title.clone().unwrap_or_default();
                let state = app.state::<AppState>();
                let mut conn = state.db.lock();
                // A DOI result replaces the record; an arXiv (no-DOI) result
                // updates the bib fields but PRESERVES any existing DOI+references
                // rather than nulling them.
                let write = match r.doi.as_deref() {
                    Some(d) => metadata::apply_metadata(&mut conn, id, d, &r.meta),
                    None => write_bib_only(&mut conn, id, &r.meta),
                };
                match write {
                    Ok(()) => {
                        let _ = crate::db::citekey::auto_citekey(&conn, id);
                        sum.resolved_online += 1;
                        sum.checked += 1;
                        sum.details.push(format!("id {id}: risolto online → {new_title}"));
                    }
                    Err(e) => sum.details.push(format!("id {id}: errore scrittura: {e:#}")),
                }
                continue;
            }
            // A network error / timeout must NOT fall through to the destructive
            // retitle below — that would corrupt a good record on a mere hiccup.
            Ok(Err(_)) | Err(_) => {
                sum.details.push(format!("id {id}: risoluzione online non riuscita (rete) — lasciato invariato"));
                continue;
            }
            Ok(Ok(None)) => { /* not indexed anywhere — fall through to (b2) */ }
        }

        // (b2) Nothing online. NEVER destroy a record that still has a DOI: a
        //      failed title match can be a merely-garbled head, not a wrong DOI,
        //      and clearing the DOI + references would lose good data. Only docs
        //      with no DOI get a first-line title recovery (nothing to lose).
        if doi.is_some() {
            continue;
        }
        let Some(guess) = metadata::first_line_title(&head) else {
            continue;
        };
        if guess == title_s {
            continue; // already recovered on a previous pass
        }
        let meta = metadata::CrossrefMeta {
            title: Some(guess.clone()),
            ..Default::default()
        };
        let state = app.state::<AppState>();
        let mut conn = state.db.lock();
        match write_repaired(&mut conn, id, &meta) {
            Ok(()) => {
                let _ = crate::db::citekey::auto_citekey(&conn, id);
                sum.retitled += 1;
                sum.checked += 1;
                sum.details.push(format!("id {id}: titolo dal PDF → {guess}"));
            }
            Err(e) => sum.details.push(format!("id {id}: errore scrittura: {e:#}")),
        }
    }

    Ok(sum)
}

// ===== Recupero metadati: bulk sui documenti "magri" + candidati per documento =====

/// Progress event payload emitted during the bulk metadata recovery.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetaRecoverProgress {
    pub done: usize,
    pub total: usize,
    pub updated: usize,
    /// "running" | "done" | "cancelled"
    pub phase: String,
}

/// Result of a bulk metadata-recovery pass.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MetaRecoverSummary {
    pub scanned: usize,
    pub updated: usize,
    /// Of which: recovered via the arXiv id in the filename.
    pub from_arxiv: usize,
    pub unresolved: usize,
    pub errors: Vec<String>,
}

/// Recover metadata for every document with THIN metadata — blank title, no
/// year or no authors, the same notion "Salute libreria" reports — on live,
/// on-disk documents. Wider than `enrich_all` (which only visits `doi IS NULL`
/// docs) and additionally recovers arXiv papers from the id in the FILENAME,
/// the only safe signal for scanned PDFs with no text layer. Precision-first
/// like the whole engine: an online record is applied only when it passes the
/// title gates; anything else is left for the interactive per-document finder.
/// Emits `meta-progress` events; cancellable via `cancel_recover_metadata`.
#[tauri::command]
pub async fn recover_missing_metadata(app: AppHandle) -> Result<MetaRecoverSummary, String> {
    let candidates: Vec<(i64, Option<String>, String, String)> = {
        let state = app.state::<AppState>();
        state.meta_cancel.store(false, Ordering::SeqCst);
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare(
                "SELECT d.id, d.doi, d.path, substr(COALESCE(d.fulltext,''),1,20000)
                 FROM documents d
                 WHERE d.deleted_at IS NULL AND d.path NOT LIKE 'ref:%'
                   AND (d.title IS NULL OR TRIM(d.title) = '' OR d.year IS NULL
                        OR NOT EXISTS (SELECT 1 FROM document_authors da WHERE da.document_id = d.id))
                 ORDER BY d.id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let email = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty())
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;

    let total = candidates.len();
    let mut sum = MetaRecoverSummary { scanned: total, ..Default::default() };
    let emit = |done: usize, updated: usize, phase: &str| {
        let _ = app.emit(
            "meta-progress",
            MetaRecoverProgress { done, total, updated, phase: phase.into() },
        );
    };
    emit(0, 0, "running");

    for (done, (id, existing_doi, path, text)) in candidates.into_iter().enumerate() {
        {
            let state = app.state::<AppState>();
            if state.meta_cancel.load(Ordering::SeqCst) {
                emit(done, sum.updated, "cancelled");
                return Ok(sum);
            }
        }
        let head: String = text.chars().take(1500).collect();
        let fname = std::path::Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        // (a) arXiv id in the FILENAME — authoritative for arXiv papers, and the
        //     only usable signal when the PDF has no text layer. Accepted when
        //     the record's title matches the PDF head, or — with NO text to gate
        //     against — when the filename is essentially just the id (a bare
        //     "2406.09406v2.pdf" names exactly one paper; anything longer could
        //     be a mislabelled save and is left to the interactive finder).
        let mut recovered = false;
        if let Some(aid) = metadata::arxiv_id_from_filename(&fname) {
            let fetched = tokio::time::timeout(
                std::time::Duration::from_secs(20),
                metadata::fetch_arxiv(&client, &aid),
            )
            .await;
            tokio::time::sleep(std::time::Duration::from_millis(400)).await; // be gentle to arXiv
            if let Ok(Ok(Some(meta))) = fetched {
                let gate_text = meta
                    .title
                    .as_deref()
                    .is_some_and(|t| metadata::title_matches_doc(t, &head));
                let gate_bare = head.trim().is_empty() && filename_is_bare_id(&fname, &aid);
                if gate_text || gate_bare {
                    let state = app.state::<AppState>();
                    let mut conn = state.db.lock();
                    match write_repaired(&mut conn, id, &meta) {
                        Ok(()) => {
                            let _ = crate::db::citekey::auto_citekey(&conn, id);
                            sum.updated += 1;
                            sum.from_arxiv += 1;
                            recovered = true;
                        }
                        Err(e) => sum.errors.push(format!("id {id}: {e:#}")),
                    }
                }
            }
        }

        // (b) Full precision-first resolution from the document's own text.
        if !recovered && !text.trim().is_empty() {
            let resolved = tokio::time::timeout(
                std::time::Duration::from_secs(45),
                metadata::resolve_document_meta(&client, &text, existing_doi.as_deref(), email.as_deref()),
            )
            .await;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            match resolved {
                Ok(Ok(Some(r))) => {
                    let state = app.state::<AppState>();
                    let mut conn = state.db.lock();
                    match apply_resolved(&mut conn, id, &r) {
                        Ok(()) => {
                            let _ = crate::db::citekey::auto_citekey(&conn, id);
                            sum.updated += 1;
                            recovered = true;
                        }
                        Err(e) => sum.errors.push(format!("id {id}: {e:#}")),
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => sum.errors.push(format!("id {id}: {e:#}")),
                Err(_) => sum.errors.push(format!("id {id}: timeout")),
            }
        }
        if !recovered {
            sum.unresolved += 1;
        }
        emit(done + 1, sum.updated, "running");
    }
    emit(total, sum.updated, "done");
    Ok(sum)
}

/// Request cancellation of an in-progress bulk metadata recovery.
#[tauri::command]
pub fn cancel_recover_metadata(state: State<'_, AppState>) {
    state.meta_cancel.store(true, Ordering::SeqCst);
}

/// True when `fname`'s stem is essentially just the arXiv id `aid` (allowing an
/// "arxiv" marker or a version suffix) — e.g. "2406.09406v2.pdf",
/// "arxiv_2606_00995.pdf" — so the id can be trusted without a text gate.
fn filename_is_bare_id(fname: &str, aid: &str) -> bool {
    let alnum = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    };
    let stem = std::path::Path::new(fname)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let residue = alnum(&stem).replacen(&alnum(aid), "", 1);
    residue.is_empty() || residue == "arxiv" || residue.len() <= 2
}

/// One candidate identity for a document, offered to the user for confirmation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetaCandidate {
    /// "crossref" | "arxiv" | "openalex"
    pub source: String,
    /// Where this candidate came from, for display (Italian).
    pub origin: String,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
    /// Display author names ("Given Family").
    pub authors: Vec<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    /// Heuristic evidence score (higher = more likely this document).
    pub score: i64,
    /// Passed the same strict title gate automatic enrichment uses.
    pub sure: bool,
    /// Human-readable evidence chips (Italian).
    pub signals: Vec<String>,
    /// Title of another live document that already owns this DOI (likely a duplicate).
    pub duplicate_of: Option<String>,
}

/// Report of the per-document candidate search.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetaProbe {
    pub pdf_title: Option<String>,
    pub filename: String,
    pub candidates: Vec<MetaCandidate>,
}

/// Candidate metadata before scoring, as gathered from one source.
struct RawCandidate {
    source: &'static str,
    origin: String,
    doi: Option<String>,
    arxiv_id: Option<String>,
    title: Option<String>,
    authors: Vec<String>,
    year: Option<i64>,
    venue: Option<String>,
    from_identifier: bool,
}

/// Display author names for a fetched record.
fn display_authors(meta: &metadata::CrossrefMeta) -> Vec<String> {
    meta.authors
        .iter()
        .map(|a| {
            format!("{} {}", a.given.as_deref().unwrap_or(""), a.family.as_deref().unwrap_or(""))
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// EXTENSIVE per-document metadata search: gather candidate identities from
/// every available signal — the stored DOI, every DOI/arXiv id printed in the
/// first pages (a citation's id is a legitimate *candidate* here: the user
/// confirms visually, unlike automatic enrichment which must reject it), the
/// arXiv id in the filename, and title searches (recovered PDF title, cleaned
/// filename, stored title) on Crossref, arXiv and OpenAlex. Each candidate is
/// scored against evidence found in the PDF itself; NOTHING is applied — the
/// user picks one via `apply_meta_candidate`.
#[tauri::command]
pub async fn metadata_candidates(app: AppHandle, id: i64) -> Result<MetaProbe, String> {
    let (existing_doi, stored_title, path, text): (Option<String>, Option<String>, String, String) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.query_row(
            "SELECT doi, title, path, substr(COALESCE(fulltext,''),1,20000) FROM documents WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| e.to_string())?
    };
    let (email, oa_key) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        (
            setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty()),
            secret::get("openalex_key").unwrap_or_default(),
        )
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let timeout = std::time::Duration::from_secs(20);
    let nap = || tokio::time::sleep(std::time::Duration::from_millis(120));

    let head: String = text.chars().take(1500).collect();
    let pdf_title = metadata::first_line_title(&head);
    let fname = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = std::path::Path::new(&fname)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut raw: Vec<RawCandidate> = Vec::new();

    // 1) DOIs: the stored one plus every one printed in the first pages.
    let mut dois: Vec<String> = Vec::new();
    if let Some(d) = existing_doi.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        dois.push(d.to_string());
    }
    for d in metadata::extract_dois(&text, 5) {
        if !dois.iter().any(|x| x.eq_ignore_ascii_case(&d)) {
            dois.push(d);
        }
    }
    for (i, d) in dois.iter().take(5).enumerate() {
        let origin = if i == 0 && existing_doi.as_deref().map(str::trim).is_some_and(|e| e.eq_ignore_ascii_case(d)) {
            "DOI della scheda".to_string()
        } else {
            "DOI stampato nel PDF".to_string()
        };
        if let Ok(Ok(Some(meta))) =
            tokio::time::timeout(timeout, metadata::fetch_crossref(&client, d, email.as_deref())).await
        {
            raw.push(RawCandidate {
                source: "crossref",
                origin,
                doi: Some(d.clone()),
                arxiv_id: None,
                title: meta.title.clone(),
                authors: display_authors(&meta),
                year: meta.year,
                venue: meta.venue.clone(),
                from_identifier: true,
            });
        }
        nap().await;
    }

    // 2) arXiv ids: filename first, then explicit "arXiv:…" mentions in the text.
    let mut aids: Vec<(String, String)> = Vec::new();
    if let Some(aid) = metadata::arxiv_id_from_filename(&fname) {
        aids.push((aid, "arXiv nel nome del file".to_string()));
    }
    for aid in metadata::extract_arxiv_ids(&text, 3) {
        if !aids.iter().any(|(x, _)| x == &aid) {
            aids.push((aid, "arXiv stampato nel PDF".to_string()));
        }
    }
    for (aid, origin) in aids.into_iter().take(3) {
        if let Ok(Ok(Some(meta))) = tokio::time::timeout(timeout, metadata::fetch_arxiv(&client, &aid)).await {
            raw.push(RawCandidate {
                source: "arxiv",
                origin,
                doi: None,
                arxiv_id: Some(aid),
                title: meta.title.clone(),
                authors: display_authors(&meta),
                year: meta.year,
                venue: meta.venue.clone(),
                from_identifier: true,
            });
        }
        nap().await;
    }

    // 3) Title searches on Crossref + arXiv + OpenAlex.
    let norm = |s: &str| -> String {
        metadata::fold_ascii(&s.to_lowercase())
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut queries: Vec<(String, String)> = Vec::new();
    if let Some(g) = pdf_title.as_deref().filter(|g| g.chars().count() >= 8) {
        queries.push(("titolo dal PDF".to_string(), g.to_string()));
    }
    let stem_q = stem.replace(['_', '-', '.'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let stem_alpha_words = stem_q
        .split_whitespace()
        .filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 3)
        .count();
    if stem_q.chars().count() >= 12
        && stem_alpha_words >= 3
        && !queries.iter().any(|(_, q)| norm(q) == norm(&stem_q))
    {
        queries.push(("nome del file".to_string(), stem_q));
    }
    if let Some(t) = stored_title.as_deref().map(str::trim).filter(|t| t.chars().count() >= 8) {
        if !queries.iter().any(|(_, q)| norm(q) == norm(t)) {
            queries.push(("titolo della scheda".to_string(), t.to_string()));
        }
    }
    let oa_filters = discovery::Filters {
        year_from: None,
        year_to: None,
        oa_only: false,
        sort: "relevance".to_string(),
        author: None,
    };
    for (label, q) in queries.iter().take(3) {
        if let Ok(Ok(list)) =
            tokio::time::timeout(timeout, metadata::crossref_search_candidates(&client, q, email.as_deref(), 5)).await
        {
            for (doi, meta) in list {
                raw.push(RawCandidate {
                    source: "crossref",
                    origin: format!("ricerca per {label} (Crossref)"),
                    doi: Some(doi),
                    arxiv_id: None,
                    title: meta.title.clone(),
                    authors: display_authors(&meta),
                    year: meta.year,
                    venue: meta.venue.clone(),
                    from_identifier: false,
                });
            }
        }
        nap().await;
        if let Ok(Ok(list)) = tokio::time::timeout(timeout, metadata::arxiv_search_candidates(&client, q, 5)).await {
            for (aid, meta) in list {
                raw.push(RawCandidate {
                    source: "arxiv",
                    origin: format!("ricerca per {label} (arXiv)"),
                    doi: None,
                    arxiv_id: (!aid.is_empty()).then_some(aid),
                    title: meta.title.clone(),
                    authors: display_authors(&meta),
                    year: meta.year,
                    venue: meta.venue.clone(),
                    from_identifier: false,
                });
            }
        }
        nap().await;
        if let Ok(Ok(list)) =
            tokio::time::timeout(timeout, discovery::search_openalex(&client, q, &oa_filters, &oa_key)).await
        {
            for r in list.into_iter().take(5) {
                raw.push(RawCandidate {
                    source: "openalex",
                    origin: format!("ricerca per {label} (OpenAlex)"),
                    doi: r.doi.clone(),
                    arxiv_id: None,
                    title: r.title.clone(),
                    authors: r.authors.clone(),
                    year: r.year,
                    venue: r.venue.clone(),
                    from_identifier: false,
                });
            }
        }
        nap().await;
    }

    // --- score against evidence in the PDF itself ---
    let head4000: String = text.chars().take(4000).collect();
    let head_l = metadata::fold_ascii(&head4000.to_lowercase());
    let head_tokens: std::collections::HashSet<String> = head_l
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    let hay_l = text.to_lowercase();

    let mut out: Vec<MetaCandidate> = Vec::new();
    for r in raw {
        let Some(t) = r.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) else {
            continue;
        };
        let mut score: i64 = 0;
        let mut signals: Vec<String> = Vec::new();
        let sure = pdf_title.as_deref().is_some_and(|g| metadata::strong_title_match(g, t));
        if sure {
            score += 100;
            signals.push("titolo identico a quello nel PDF".to_string());
        } else if metadata::title_matches_doc(t, &head4000) {
            score += 30;
            signals.push("parole del titolo nella prima pagina".to_string());
        }
        // Author surnames printed near the top — whole tokens ('adam' ≠ 'adamson').
        let fams: Vec<String> = r
            .authors
            .iter()
            .filter_map(|a| parse_author(a).1)
            .filter_map(|f| {
                metadata::fold_ascii(&f.to_lowercase())
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() >= 3)
                    .last()
                    .map(str::to_string)
            })
            .collect();
        if !fams.is_empty() {
            let hits = fams.iter().filter(|f| head_tokens.contains(*f)).count();
            if hits > 0 {
                score += (30 * hits / fams.len()) as i64;
                signals.push(format!("{hits}/{} autori nella prima pagina", fams.len()));
            }
        }
        if let Some(d) = r.doi.as_deref() {
            if hay_l.contains(&d.to_lowercase()) {
                score += 25;
                signals.push("DOI presente nel PDF".to_string());
            }
        }
        if let Some(y) = r.year {
            if head_tokens.contains(&y.to_string()) {
                score += 5;
                signals.push("anno nella prima pagina".to_string());
            }
        }
        if r.from_identifier {
            score += 10;
        }
        out.push(MetaCandidate {
            source: r.source.to_string(),
            origin: r.origin,
            doi: r.doi,
            arxiv_id: r.arxiv_id,
            title: r.title,
            authors: r.authors,
            year: r.year,
            venue: r.venue,
            score,
            sure,
            signals,
            duplicate_of: None,
        });
    }

    // Best first; dedup by identity (DOI, else arXiv id sans version, else title).
    out.sort_by(|a, b| (b.sure, b.score).cmp(&(a.sure, a.score)));
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    out.retain(|c| {
        let key = c
            .doi
            .as_deref()
            .map(|d| format!("doi:{}", d.to_lowercase()))
            .or_else(|| {
                c.arxiv_id
                    .as_deref()
                    .map(|a| format!("arx:{}", a.split('v').next().unwrap_or(a)))
            })
            .unwrap_or_else(|| {
                format!(
                    "t:{}",
                    c.title
                        .as_deref()
                        .map(|t| t.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" "))
                        .unwrap_or_default()
                )
            });
        seen.insert(key)
    });
    out.truncate(12);

    // Flag candidates whose DOI already belongs to another live document.
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        for c in out.iter_mut() {
            if let Some(d) = c.doi.as_deref() {
                let dup: Option<String> = conn
                    .query_row(
                        "SELECT COALESCE(NULLIF(TRIM(title),''),'senza titolo') FROM documents
                         WHERE doi = ?1 AND id <> ?2 AND deleted_at IS NULL",
                        params![d, id],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                if let Some(t) = dup {
                    c.duplicate_of = Some(t);
                    c.signals.push("già in libreria (possibile duplicato)".to_string());
                }
            }
        }
    }

    Ok(MetaProbe { pdf_title, filename: fname, candidates: out })
}

/// A `CrossrefMeta` built from a candidate's own fields (fallback when the
/// record cannot be re-fetched).
fn meta_from_candidate(c: &MetaCandidate) -> metadata::CrossrefMeta {
    metadata::CrossrefMeta {
        title: c.title.clone(),
        venue: c.venue.clone(),
        year: c.year,
        abstract_text: None,
        authors: c
            .authors
            .iter()
            .map(|s| {
                let (given, family) = parse_author(s);
                metadata::Author { given, family }
            })
            .collect(),
        references: Vec::new(),
        raw_json: String::new(),
    }
}

/// Apply a USER-CONFIRMED candidate to document `id`. With a DOI the full
/// Crossref record is re-fetched and applied (references included); with an
/// arXiv id the arXiv record is fetched and applied, clearing any stale
/// DOI/references — the user just told us the paper's identity. With neither,
/// the candidate's own fields are written. The citekey is refreshed afterwards.
#[tauri::command]
pub async fn apply_meta_candidate(app: AppHandle, id: i64, candidate: MetaCandidate) -> Result<(), String> {
    let email = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty())
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let timeout = std::time::Duration::from_secs(20);

    if let Some(doi) = candidate.doi.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        // Refuse a DOI another live document already owns: applying it would
        // silently drop the identifier (apply_metadata's dup-guard) — the user
        // almost certainly has a duplicate to merge instead.
        let dup: Option<String> = {
            let state = app.state::<AppState>();
            let conn = state.db.lock();
            conn.query_row(
                "SELECT COALESCE(NULLIF(TRIM(title),''),'senza titolo') FROM documents
                 WHERE doi = ?1 AND id <> ?2 AND deleted_at IS NULL",
                params![doi, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
        };
        if let Some(t) = dup {
            return Err(format!(
                "Questo DOI appartiene già a «{t}»: probabilmente è un duplicato — meglio unire i documenti"
            ));
        }
        let fetched = tokio::time::timeout(timeout, metadata::fetch_crossref(&client, doi, email.as_deref())).await;
        let meta = match fetched {
            Ok(Ok(Some(m))) => m,
            _ => meta_from_candidate(&candidate),
        };
        let state = app.state::<AppState>();
        let mut conn = state.db.lock();
        metadata::apply_metadata(&mut conn, id, doi, &meta).map_err(|e| e.to_string())?;
        let _ = crate::db::citekey::auto_citekey(&conn, id);
        return Ok(());
    }

    if let Some(aid) = candidate.arxiv_id.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        let fetched = tokio::time::timeout(timeout, metadata::fetch_arxiv(&client, aid)).await;
        let meta = match fetched {
            Ok(Ok(Some(m))) => m,
            _ => meta_from_candidate(&candidate),
        };
        if meta.title.as_deref().map(str::trim).unwrap_or("").is_empty() {
            return Err("Record arXiv non trovato per questo identificativo".to_string());
        }
        let state = app.state::<AppState>();
        let mut conn = state.db.lock();
        write_repaired(&mut conn, id, &meta).map_err(|e| e.to_string())?;
        let _ = crate::db::citekey::auto_citekey(&conn, id);
        return Ok(());
    }

    let meta = meta_from_candidate(&candidate);
    if meta.title.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err("Il candidato non ha né identificativo né titolo".to_string());
    }
    let state = app.state::<AppState>();
    let mut conn = state.db.lock();
    write_repaired(&mut conn, id, &meta).map_err(|e| e.to_string())?;
    let _ = crate::db::citekey::auto_citekey(&conn, id);
    Ok(())
}

#[cfg(test)]
mod meta_recover_tests {
    use super::filename_is_bare_id;

    #[test]
    fn bare_id_filenames() {
        assert!(filename_is_bare_id("2406.09406v2.pdf", "2406.09406v2"));
        assert!(filename_is_bare_id("arxiv_2606_00995.pdf", "2606.00995"));
        assert!(filename_is_bare_id("2512.16301.pdf", "2512.16301"));
        // A descriptive filename that merely CONTAINS an id is not "bare": with
        // no text layer to gate against, it must not be trusted.
        assert!(!filename_is_bare_id("smith_review_2406.09406.pdf", "2406.09406"));
    }
}

/// Read a document's raw PDF bytes for the in-app viewer (efficient binary IPC).
#[tauri::command]
pub fn read_pdf(state: State<'_, AppState>, id: i64) -> Result<tauri::ipc::Response, String> {
    let path: Option<String> = {
        let conn = state.db.lock();
        conn.query_row("SELECT path FROM documents WHERE id = ?1", params![id], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(|e| e.to_string())?
    };
    let path = path.ok_or_else(|| "document not found".to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// List a document's annotations, ordered by page.
#[tauri::command]
pub fn list_annotations(state: State<'_, AppState>, document_id: i64) -> Result<Vec<Annotation>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, page, kind, color, rects_json, quote, note, created_at
             FROM annotations WHERE document_id = ?1 ORDER BY page, id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![document_id], |r| {
            Ok(Annotation {
                id: r.get(0)?,
                page: r.get(1)?,
                kind: r.get(2)?,
                color: r.get(3)?,
                rects_json: r.get(4)?,
                quote: r.get(5)?,
                note: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

/// The annotation kinds the reader can produce (anything else falls back to a highlight).
fn norm_anno_kind(kind: Option<String>) -> &'static str {
    match kind.as_deref() {
        Some("underline") => "underline",
        Some("strikethrough") => "strikethrough",
        Some("note") => "note",
        _ => "highlight",
    }
}

/// Add an annotation (`highlight` by default); returns the new annotation id.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn add_annotation(
    state: State<'_, AppState>,
    document_id: i64,
    page: i64,
    kind: Option<String>,
    color: Option<String>,
    rects_json: String,
    quote: Option<String>,
    note: Option<String>,
) -> Result<i64, String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO annotations (document_id, page, kind, color, rects_json, quote, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![document_id, page, norm_anno_kind(kind), color, rects_json, quote, note],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Update the note text of an annotation.
#[tauri::command]
pub fn update_annotation_note(state: State<'_, AppState>, id: i64, note: Option<String>) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("UPDATE annotations SET note = ?1 WHERE id = ?2", params![note, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete an annotation.
#[tauri::command]
pub fn delete_annotation(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM annotations WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Save just a document's free-text notes (cheap path for autosave from the reader).
/// `notes` is not in the FTS index, so this avoids the author/fulltext rewrite that the
/// full metadata editor does. An empty/blank string clears the notes.
#[tauri::command]
pub fn set_document_notes(state: State<'_, AppState>, id: i64, notes: String) -> Result<(), String> {
    let trimmed = notes.trim();
    let value: Option<&str> = if trimmed.is_empty() { None } else { Some(trimmed) };
    let conn = state.db.lock();
    conn.execute("UPDATE documents SET notes = ?1 WHERE id = ?2", params![value, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct HealthRow {
    pub id: i64,
    pub title: Option<String>,
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct DupGroup {
    pub file_hash: String,
    pub ids: Vec<i64>,
    pub titles: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct LibraryHealth {
    pub total: i64,
    pub missing_file: Vec<HealthRow>,
    pub no_text: Vec<HealthRow>,
    pub no_metadata: Vec<HealthRow>,
    pub no_embedding: Vec<HealthRow>,
    pub no_thumbnail: Vec<HealthRow>,
    pub duplicates: Vec<DupGroup>,
}

/// Read-only "library health" scan: surface rot signals — files that vanished from disk,
/// PDFs with no extracted text (scanned/image-only), thin metadata, missing embeddings or
/// thumbnails, and duplicate files (same hash). Each list is capped to keep the payload sane.
#[tauri::command]
pub async fn library_health(app: AppHandle) -> Result<LibraryHealth, String> {
    const CAP: usize = 300;
    tauri::async_runtime::spawn_blocking(move || -> Result<LibraryHealth, String> {
        let state = app.state::<AppState>();
        let conn = state.db.lock();

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents WHERE deleted_at IS NULL", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;

        struct Row {
            id: i64,
            title: Option<String>,
            path: String,
            thumb: Option<String>,
            year: Option<i64>,
            hash: Option<String>,
            tlen: i64,
            nauth: i64,
            has_vec: i64,
        }
        let mut stmt = conn
            .prepare(
                "SELECT d.id, d.title, d.path, d.thumb_path, d.year, d.file_hash,
                        COALESCE(LENGTH(TRIM(d.fulltext)), 0) AS tlen,
                        (SELECT COUNT(*) FROM document_authors da WHERE da.document_id = d.id) AS nauth,
                        EXISTS(SELECT 1 FROM doc_vec v WHERE v.document_id = d.id) AS has_vec
                 FROM documents d WHERE d.deleted_at IS NULL ORDER BY d.id",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Row> = stmt
            .query_map([], |r| {
                Ok(Row {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    path: r.get(2)?,
                    thumb: r.get(3)?,
                    year: r.get(4)?,
                    hash: r.get(5)?,
                    tlen: r.get(6)?,
                    nauth: r.get(7)?,
                    has_vec: r.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        let mut missing_file = Vec::new();
        let mut no_text = Vec::new();
        let mut no_metadata = Vec::new();
        let mut no_embedding = Vec::new();
        let mut no_thumbnail = Vec::new();
        let mut by_hash: std::collections::HashMap<String, Vec<(i64, String)>> = std::collections::HashMap::new();

        for r in &rows {
            let row = || HealthRow { id: r.id, title: r.title.clone(), path: r.path.clone() };
            if !std::path::Path::new(&r.path).exists() && missing_file.len() < CAP {
                missing_file.push(row());
            }
            if r.tlen == 0 && no_text.len() < CAP {
                no_text.push(row());
            }
            let thin = r.title.as_deref().map(|t| t.trim().is_empty()).unwrap_or(true)
                || r.year.is_none()
                || r.nauth == 0;
            if thin && no_metadata.len() < CAP {
                no_metadata.push(row());
            }
            if r.has_vec == 0 && no_embedding.len() < CAP {
                no_embedding.push(row());
            }
            if r.thumb.as_deref().map(|t| t.trim().is_empty()).unwrap_or(true) && no_thumbnail.len() < CAP {
                no_thumbnail.push(row());
            }
            if let Some(h) = r.hash.as_deref().filter(|h| !h.is_empty()) {
                by_hash
                    .entry(h.to_string())
                    .or_default()
                    .push((r.id, r.title.clone().unwrap_or_default()));
            }
        }
        let mut duplicates: Vec<DupGroup> = by_hash
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(h, v)| DupGroup {
                file_hash: h,
                ids: v.iter().map(|(i, _)| *i).collect(),
                titles: v.into_iter().map(|(_, t)| t).collect(),
            })
            .collect();
        duplicates.sort_by(|a, b| b.ids.len().cmp(&a.ids.len()));

        Ok(LibraryHealth {
            total,
            missing_file,
            no_text,
            no_metadata,
            no_embedding,
            no_thumbnail,
            duplicates,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn embed_cache_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("fastembed_cache"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_fastembed"))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbedStatus {
    pub total: i64,
    pub embedded: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbedSummary {
    pub embedded: usize,
    pub errors: Vec<String>,
}

/// How many documents have a semantic embedding vs the total.
#[tauri::command]
pub fn embedding_status(state: State<'_, AppState>) -> Result<EmbedStatus, String> {
    let conn = state.db.lock();
    let total = conn
        .query_row("SELECT count(*) FROM documents WHERE deleted_at IS NULL", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let embedded = conn
        .query_row(
            "SELECT count(*) FROM doc_vec WHERE document_id IN (SELECT id FROM documents WHERE deleted_at IS NULL)",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(EmbedStatus { total, embedded })
}

/// Progress event payload emitted during embedding generation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbedProgress {
    pub done: usize,
    pub total: usize,
    /// "model" (loading/downloading) | "running" | "done" | "cancelled"
    pub phase: String,
}

/// Generate embeddings for documents missing them, in background batches.
/// Emits `embed-progress` events and can be stopped via `cancel_embeddings`.
/// Resumable: only documents absent from the vector store are processed.
#[tauri::command]
pub async fn generate_embeddings(app: AppHandle) -> Result<EmbedSummary, String> {
    const BATCH: usize = 16;
    let cache = embed_cache_dir(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.cancel_embed.store(false, Ordering::SeqCst);

        let (candidates, note_candidates) = {
            let conn = state.db.lock();
            // Vectors of deleted notes are orphans: sweep them so the map is clean.
            let _ = conn.execute(
                "DELETE FROM note_vec WHERE note_id NOT IN (SELECT id FROM notes)",
                [],
            );
            let candidates: Vec<(i64, Option<String>, Option<String>, String)> = {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, title, abstract, COALESCE(fulltext, '')
                         FROM documents
                         WHERE id NOT IN (SELECT document_id FROM doc_vec) AND deleted_at IS NULL",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                    .map_err(|e| e.to_string())?;
                rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
            };
            // Notes (.md vault) still missing a vector — they join the map too.
            let note_candidates: Vec<(i64, Option<String>, Option<String>)> = {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, title, body FROM notes
                         WHERE id NOT IN (SELECT note_id FROM note_vec)",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .map_err(|e| e.to_string())?;
                rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
            };
            (candidates, note_candidates)
        };

        let total = candidates.len() + note_candidates.len();
        let mut summary = EmbedSummary {
            embedded: 0,
            errors: Vec::new(),
        };
        if total == 0 {
            let _ = app.emit(
                "embed-progress",
                EmbedProgress { done: 0, total: 0, phase: "done".into() },
            );
            return Ok(summary);
        }

        // The first batch lazily loads the model (downloads ~2.3GB on first ever run).
        let _ = app.emit(
            "embed-progress",
            EmbedProgress { done: 0, total, phase: "model".into() },
        );

        for chunk in candidates.chunks(BATCH) {
            if state.cancel_embed.load(Ordering::SeqCst) {
                let _ = app.emit(
                    "embed-progress",
                    EmbedProgress { done: summary.embedded, total, phase: "cancelled".into() },
                );
                return Ok(summary);
            }
            let ids: Vec<i64> = chunk.iter().map(|c| c.0).collect();
            let texts: Vec<String> = chunk
                .iter()
                .map(|(_, title, abstract_, fulltext)| {
                    embed::compose_text(title.as_deref(), abstract_.as_deref(), fulltext)
                })
                .collect();

            match embed::embed_batch(&cache, texts) {
                Ok(vectors) => {
                    if state.cancel_embed.load(Ordering::SeqCst) {
                        let _ = app.emit(
                            "embed-progress",
                            EmbedProgress { done: summary.embedded, total, phase: "cancelled".into() },
                        );
                        return Ok(summary);
                    }
                    let conn = state.db.lock();
                    for (id, v) in ids.iter().zip(vectors.iter()) {
                        match conn.execute(
                            "INSERT INTO doc_vec (document_id, embedding) VALUES (?1, ?2)",
                            params![id, v.as_slice().as_bytes()],
                        ) {
                            Ok(_) => summary.embedded += 1,
                            Err(e) => summary.errors.push(format!("doc {id}: {e}")),
                        }
                    }
                }
                Err(e) => summary.errors.push(format!("batch: {e}")),
            }

            let _ = app.emit(
                "embed-progress",
                EmbedProgress { done: summary.embedded, total, phase: "running".into() },
            );
        }

        // Notes: same batching, into note_vec (same bge-m3 space as the docs).
        for chunk in note_candidates.chunks(BATCH) {
            if state.cancel_embed.load(Ordering::SeqCst) {
                let _ = app.emit(
                    "embed-progress",
                    EmbedProgress { done: summary.embedded, total, phase: "cancelled".into() },
                );
                return Ok(summary);
            }
            let ids: Vec<i64> = chunk.iter().map(|c| c.0).collect();
            let texts: Vec<String> = chunk
                .iter()
                .map(|(_, title, body)| {
                    embed::compose_text(title.as_deref(), None, body.as_deref().unwrap_or(""))
                })
                .collect();
            match embed::embed_batch(&cache, texts) {
                Ok(vectors) => {
                    let conn = state.db.lock();
                    for (id, v) in ids.iter().zip(vectors.iter()) {
                        match conn.execute(
                            "INSERT INTO note_vec (note_id, embedding) VALUES (?1, ?2)",
                            params![id, v.as_slice().as_bytes()],
                        ) {
                            Ok(_) => summary.embedded += 1,
                            Err(e) => summary.errors.push(format!("nota {id}: {e}")),
                        }
                    }
                }
                Err(e) => summary.errors.push(format!("batch note: {e}")),
            }
            let _ = app.emit(
                "embed-progress",
                EmbedProgress { done: summary.embedded, total, phase: "running".into() },
            );
        }

        let _ = app.emit(
            "embed-progress",
            EmbedProgress { done: summary.embedded, total, phase: "done".into() },
        );
        Ok::<_, String>(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Request cancellation of an in-progress embedding job.
#[tauri::command]
pub fn cancel_embeddings(state: State<'_, AppState>) {
    state.cancel_embed.store(true, Ordering::SeqCst);
}

/// Search the library. `mode` is "fulltext", "semantic", or "hybrid".
#[tauri::command]
pub async fn search(app: AppHandle, query: String, mode: String) -> Result<Vec<Document>, String> {
    let cache = embed_cache_dir(&app);
    tauri::async_runtime::spawn_blocking(move || {
        const LIMIT: usize = 50;
        let want_vector = mode == "semantic" || mode == "hybrid";
        let want_text = mode == "fulltext" || mode == "hybrid";

        // Embed the query before touching the DB (avoids holding the lock across work).
        let qvec = if want_vector {
            Some(embed::embed_query(&cache, &query).map_err(|e| e.to_string())?)
        } else {
            None
        };

        let state = app.state::<AppState>();
        let conn = state.db.lock();

        let mut fts_ids: Vec<i64> = Vec::new();
        if want_text {
            let fts = fts_query(&query);
            if !fts.is_empty() {
                let mut stmt = conn
                    .prepare("SELECT rowid FROM doc_fts WHERE doc_fts MATCH ?1 ORDER BY rank LIMIT ?2")
                    .map_err(|e| e.to_string())?;
                fts_ids = stmt
                    .query_map(params![fts, LIMIT as i64], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect();
            }
            // The FTS index covers title/abstract/fulltext only. Also match the
            // user's own notes and annotation notes/quotes (LIKE; collection is
            // small) and append them after the FTS hits, de-duplicated.
            let q = query.trim();
            if q.len() >= 2 {
                let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
                let mut extra: Vec<i64> = Vec::new();
                if let Ok(mut s) = conn.prepare(
                    "SELECT id FROM documents WHERE deleted_at IS NULL AND notes LIKE ?1 ESCAPE '\\'",
                ) {
                    extra.extend(
                        s.query_map(params![like], |r| r.get::<_, i64>(0))
                            .map(|m| m.filter_map(Result::ok).collect::<Vec<_>>())
                            .unwrap_or_default(),
                    );
                }
                if let Ok(mut s) = conn.prepare(
                    "SELECT DISTINCT a.document_id FROM annotations a
                     JOIN documents d ON d.id = a.document_id
                     WHERE d.deleted_at IS NULL
                       AND ((a.note LIKE ?1 ESCAPE '\\') OR (a.quote LIKE ?1 ESCAPE '\\'))",
                ) {
                    extra.extend(
                        s.query_map(params![like], |r| r.get::<_, i64>(0))
                            .map(|m| m.filter_map(Result::ok).collect::<Vec<_>>())
                            .unwrap_or_default(),
                    );
                }
                for id in extra {
                    if !fts_ids.contains(&id) {
                        fts_ids.push(id);
                    }
                }
            }
        }

        let mut vec_ids: Vec<i64> = Vec::new();
        if let Some(qv) = &qvec {
            let mut stmt = conn
                .prepare(
                    "SELECT document_id FROM doc_vec
                     WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance",
                )
                .map_err(|e| e.to_string())?;
            vec_ids = stmt
                .query_map(params![qv.as_slice().as_bytes(), LIMIT as i64], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
        }

        let ranked = rrf_merge(&fts_ids, &vec_ids, LIMIT);
        fetch_documents(&conn, &ranked, false).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Documents most similar to `id` by embedding (excludes itself). Empty if the
/// document hasn't been indexed yet.
#[tauri::command]
pub fn related_documents(state: State<'_, AppState>, id: i64) -> Result<Vec<Document>, String> {
    let conn = state.db.lock();
    let emb: Option<Vec<u8>> = conn
        .query_row(
            "SELECT embedding FROM doc_vec WHERE document_id = ?1",
            params![id],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(emb) = emb else {
        return Ok(Vec::new());
    };
    let ids: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT document_id FROM doc_vec WHERE embedding MATCH ?1 AND k = 13 ORDER BY distance",
            )
            .map_err(|e| e.to_string())?;
        let v: Vec<i64> = stmt
            .query_map(params![emb], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|&x| x != id)
            .take(12)
            .collect();
        v
    };
    fetch_documents(&conn, &ids, false).map_err(|e| e.to_string())
}

// ===== Similarity graph (embedding KNN over the whole library) =====

#[derive(serde::Serialize)]
pub struct GraphNode {
    pub id: i64,
    pub title: Option<String>,
    pub year: Option<i64>,
    /// Color of the document's most-used tag (null if untagged / colorless).
    pub color: Option<String>,
    /// Number of edges incident to this node (0 = isolated).
    pub degree: i64,
    pub unread: bool,
    pub favorite: bool,
    /// Published in a peer-reviewed venue (same derivation as the sidebar facet).
    pub peer_reviewed: bool,
    /// Has a linked GitHub repository.
    pub has_github: bool,
    /// PCA projection of the embedding (normalized −1..1): a semantically
    /// meaningful SEED position for the layout.
    pub px: f32,
    pub py: f32,
    /// Layout position saved from a previous session (world units), if any —
    /// keeps the map stable across restarts.
    pub sx: Option<f32>,
    pub sy: Option<f32>,
    /// Semantic community index (label propagation on the KNN graph), −1 when
    /// the node belongs to no sizeable cluster.
    pub community: i64,
    /// "doc" for papers, "note" for vault appunti (drawn as diamonds; their
    /// `id` is the NEGATED note id so the two id spaces can't collide).
    pub kind: String,
    /// The note's slug ("note" kind only) — lets the frontend open it.
    pub slug: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ClusterInfo {
    pub id: i64,
    /// Short human label from the most characteristic title terms (TF-IDF).
    pub label: String,
    pub size: i64,
}

#[derive(serde::Serialize)]
pub struct GraphEdge {
    pub a: i64,
    pub b: i64,
    /// Cosine similarity of the pair (min_sim..1).
    pub w: f64,
}

#[derive(serde::Serialize)]
pub struct SimilarityGraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Sizeable semantic communities (size ≥ 3), largest first.
    pub clusters: Vec<ClusterInfo>,
    pub embedded: i64,
    pub total: i64,
}

/// Community detection by weighted label propagation, deterministic (fixed node
/// order, ties to the smallest label). Returns doc id → community label (a doc id).
fn label_propagation(
    ids: &std::collections::HashSet<i64>,
    edges: &[GraphEdge],
) -> std::collections::HashMap<i64, i64> {
    use std::collections::HashMap;
    let mut adj: HashMap<i64, Vec<(i64, f64)>> = HashMap::new();
    for e in edges {
        adj.entry(e.a).or_default().push((e.b, e.w));
        adj.entry(e.b).or_default().push((e.a, e.w));
    }
    let mut label: HashMap<i64, i64> = ids.iter().map(|&i| (i, i)).collect();
    let mut order: Vec<i64> = ids.iter().copied().collect();
    order.sort_unstable();
    for _ in 0..12 {
        let mut changed = false;
        for &id in &order {
            let Some(nb) = adj.get(&id) else { continue };
            let mut scores: HashMap<i64, f64> = HashMap::new();
            for (n, w) in nb {
                if let Some(&l) = label.get(n) {
                    *scores.entry(l).or_default() += w;
                }
            }
            // Highest total weight wins; ties break to the smallest label so the
            // outcome is independent of hash iteration order.
            if let Some((&best, _)) = scores.iter().max_by(|a, b| {
                a.1.partial_cmp(b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.0.cmp(a.0))
            }) {
                if label.get(&id) != Some(&best) {
                    label.insert(id, best);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    label
}

/// A short label for a cluster: the 2 most characteristic title tokens (TF-IDF
/// against the whole node set).
fn cluster_label(
    member_titles: &[&str],
    df: &std::collections::HashMap<String, usize>,
    n_docs: usize,
) -> String {
    use std::collections::HashMap;
    let mut tf: HashMap<String, usize> = HashMap::new();
    for t in member_titles {
        for tok in title_tokens(t) {
            *tf.entry(tok).or_default() += 1;
        }
    }
    let mut scored: Vec<(String, f64)> = tf
        .into_iter()
        .map(|(tok, f)| {
            let d = df.get(&tok).copied().unwrap_or(1).max(1) as f64;
            let idf = ((n_docs as f64 + 1.0) / d).ln();
            (tok, f as f64 * idf)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    scored.into_iter().take(2).map(|(t, _)| t).collect::<Vec<_>>().join(" · ")
}

/// Lowercased alphabetic tokens of a title, minus trivial stopwords.
fn title_tokens(title: &str) -> Vec<String> {
    const STOP: [&str; 42] = [
        "the", "and", "for", "with", "from", "into", "via", "using", "based", "towards",
        "toward", "over", "under", "between", "through", "your", "our", "their", "its",
        "are", "can", "not", "all", "when", "what", "how", "why", "who", "this", "that",
        "di", "del", "della", "delle", "dei", "per", "con", "una", "uno", "gli", "les", "das",
    ];
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 2 && !STOP.contains(t) && !t.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// K-nearest-neighbour similarity graph over all embedded, non-deleted
/// documents. Every embedded document becomes a node (isolated ones included);
/// an edge links two documents when one is among the other's `k` nearest
/// neighbours with cosine similarity >= `min_sim`. Read-only; bounded to the
/// 3000 most recent embedded documents to keep the N×KNN pass fast.
#[tauri::command]
pub async fn similarity_graph(
    state: tauri::State<'_, AppState>,
    k: Option<usize>,
    min_sim: Option<f64>,
) -> Result<SimilarityGraphData, String> {
    let k = k.unwrap_or(4).clamp(1, 8);
    let min_sim = min_sim.unwrap_or(0.55).clamp(0.0, 0.95);

    // Docs processed per DB lock acquisition: the O(n×KNN) pass is sliced so
    // concurrent commands can interleave instead of stalling for seconds.
    const CHUNK: usize = 64;

    // Counts + node payloads under one short-lived guard, then release.
    let (total, embedded, docs) = {
        let conn = state.db.lock();
        // Same counts as embedding_status, so the UI can show "N of M embedded".
        let total: i64 = conn
            .query_row("SELECT count(*) FROM documents WHERE deleted_at IS NULL", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let embedded: i64 = conn
            .query_row(
                "SELECT count(*) FROM doc_vec WHERE document_id IN (SELECT id FROM documents WHERE deleted_at IS NULL)",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;

        // Every embedded, non-deleted document (most recent first, capped).
        // (id, title, year, is_read, favorite, peer_reviewed, has_github, embedding)
        #[allow(clippy::type_complexity)]
        let docs: Vec<(i64, Option<String>, Option<i64>, bool, bool, bool, bool, Vec<u8>)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT d.id, d.title, d.year, d.is_read, d.favorite,
                            d.doi, d.venue, d.path, d.github_url, v.embedding
                     FROM documents d JOIN doc_vec v ON v.document_id = d.id
                     WHERE d.deleted_at IS NULL
                     ORDER BY d.id DESC LIMIT 3000",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| {
                    let doi = r.get::<_, Option<String>>(5)?;
                    let venue = r.get::<_, Option<String>>(6)?;
                    let path = r.get::<_, Option<String>>(7)?;
                    let gh = r.get::<_, Option<String>>(8)?;
                    // Same derivation as the sidebar "peer-reviewed" facet.
                    let peer = discovery::classify_pub_status(
                        doi.as_deref(),
                        venue.as_deref(),
                        path.as_deref(),
                    )
                    .as_deref()
                        == Some("published");
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, i64>(3)? != 0,
                        r.get::<_, i64>(4)? != 0,
                        peer,
                        gh.map(|s| !s.trim().is_empty()).unwrap_or(false),
                        r.get::<_, Vec<u8>>(9)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        };
        (total, embedded, docs)
    };
    let node_ids: std::collections::HashSet<i64> = docs.iter().map(|d| d.0).collect();

    // Saved layout positions (previous sessions) — a pure cache, may be empty.
    let saved: std::collections::HashMap<i64, (f32, f32)> = {
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare("SELECT document_id, x, y FROM graph_positions")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, (r.get::<_, f64>(1)? as f32, r.get::<_, f64>(2)? as f32)))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };

    // PCA seed positions from the embeddings (deterministic; off the async thread —
    // ~3000 × 1024-dim vectors take a few hundred ms of pure math).
    let vectors: Vec<Vec<f32>> = docs
        .iter()
        .map(|d| d.7.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
        .collect();
    let pca: Vec<(f32, f32)> = tauri::async_runtime::spawn_blocking(move || embed::pca_2d(&vectors))
        .await
        .map_err(|e| e.to_string())?;

    // KNN per document (k+1 to skip self); edges deduplicated on the unordered
    // pair, keeping the strongest weight. vec0 with distance_metric=cosine
    // returns distance = 1 - cosine similarity. The lock is re-acquired per
    // chunk so other commands aren't starved during the pass.
    let mut edge_map: std::collections::HashMap<(i64, i64), f64> = std::collections::HashMap::new();
    for chunk in docs.chunks(CHUNK) {
        let conn = state.db.lock();
        let mut knn = conn
            .prepare_cached(
                "SELECT document_id, distance FROM doc_vec WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance",
            )
            .map_err(|e| e.to_string())?;
        for (id, _, _, _, _, _, _, emb) in chunk {
            let neighbours: Vec<(i64, f64)> = knn
                .query_map(params![emb, (k + 1) as i64], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
                })
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
            for (nid, dist) in neighbours {
                // Skip self and neighbours outside the node set (deleted docs
                // keep their vector until purged; capped-out docs too).
                if nid == *id || !node_ids.contains(&nid) {
                    continue;
                }
                let sim = 1.0 - dist;
                if sim < min_sim {
                    continue;
                }
                let key = if *id < nid { (*id, nid) } else { (nid, *id) };
                let w = edge_map.entry(key).or_insert(sim);
                if sim > *w {
                    *w = sim;
                }
            }
        }
    }

    // Notes (.md vault) with a vector become satellite nodes: each links to its
    // k nearest DOCUMENTS (id = −note_id so the id spaces can't collide).
    let notes: Vec<(i64, String, Option<String>)> = {
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare("SELECT n.id, n.slug, n.title FROM notes n JOIN note_vec v ON v.note_id = n.id ORDER BY n.id DESC LIMIT 500")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };
    for chunk in notes.chunks(CHUNK) {
        let conn = state.db.lock();
        let mut nvec = conn
            .prepare_cached("SELECT embedding FROM note_vec WHERE note_id = ?1")
            .map_err(|e| e.to_string())?;
        let mut knn = conn
            .prepare_cached(
                "SELECT document_id, distance FROM doc_vec WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance",
            )
            .map_err(|e| e.to_string())?;
        for (nid, _, _) in chunk {
            let Ok(emb) = nvec.query_row(params![nid], |r| r.get::<_, Vec<u8>>(0)) else { continue };
            let neighbours: Vec<(i64, f64)> = knn
                .query_map(params![emb, k as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
            for (did, dist) in neighbours {
                if !node_ids.contains(&did) {
                    continue;
                }
                let sim = 1.0 - dist;
                if sim < min_sim {
                    continue;
                }
                edge_map.insert((-nid, did), sim);
            }
        }
    }

    let mut degree: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut edges: Vec<GraphEdge> = Vec::with_capacity(edge_map.len());
    for (&(a, b), &w) in &edge_map {
        *degree.entry(a).or_default() += 1;
        *degree.entry(b).or_default() += 1;
        edges.push(GraphEdge { a, b, w });
    }
    edges.sort_by(|x, y| (x.a, x.b).cmp(&(y.a, y.b))); // deterministic output

    // Semantic communities + their labels (for cluster coloring and the far-zoom
    // "nebulae"). Only sizeable groups (≥3) get an index; the rest stay −1.
    // Notes participate (they inherit the community of the papers they orbit).
    let mut all_ids = node_ids.clone();
    for (nid, _, _) in &notes {
        all_ids.insert(-nid);
    }
    let labels = label_propagation(&all_ids, &edges);
    let mut groups: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for (&id, &l) in &labels {
        groups.entry(l).or_default().push(id);
    }
    let mut sized: Vec<(i64, Vec<i64>)> = groups.into_iter().filter(|(_, m)| m.len() >= 3).collect();
    sized.sort_by_key(|(l, m)| (std::cmp::Reverse(m.len()), *l));
    let title_of: std::collections::HashMap<i64, &str> = docs
        .iter()
        .map(|d| (d.0, d.1.as_deref().unwrap_or("")))
        .collect();
    let mut df: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for d in &docs {
        let mut seen = std::collections::HashSet::new();
        for tok in title_tokens(d.1.as_deref().unwrap_or("")) {
            if seen.insert(tok.clone()) {
                *df.entry(tok).or_default() += 1;
            }
        }
    }
    let mut community_of: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut clusters: Vec<ClusterInfo> = Vec::new();
    for (ci, (_, members)) in sized.iter().enumerate() {
        let member_titles: Vec<&str> =
            members.iter().filter_map(|id| title_of.get(id).copied()).collect();
        clusters.push(ClusterInfo {
            id: ci as i64,
            label: cluster_label(&member_titles, &df, docs.len()),
            size: members.len() as i64,
        });
        for id in members {
            community_of.insert(*id, ci as i64);
        }
    }

    // Node color = color of the document's most-used tag (chunked like above).
    let mut nodes: Vec<GraphNode> = Vec::with_capacity(docs.len());
    let mut doc_idx = 0usize; // parallel index into `pca` (same order as `docs`)
    for chunk in docs.chunks(CHUNK) {
        let conn = state.db.lock();
        let mut color_stmt = conn
            .prepare_cached(
                "SELECT t.color FROM tags t JOIN document_tags dt ON dt.tag_id = t.id
                 WHERE dt.document_id = ?1
                 ORDER BY (SELECT COUNT(*) FROM document_tags dt2 WHERE dt2.tag_id = t.id) DESC
                 LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        for (id, title, year, is_read, favorite, peer, gh, _) in chunk {
            let color: Option<String> = color_stmt
                .query_row(params![id], |r| r.get::<_, Option<String>>(0))
                .optional()
                .map_err(|e| e.to_string())?
                .flatten();
            let (px, py) = pca.get(doc_idx).copied().unwrap_or((0.0, 0.0));
            doc_idx += 1;
            let pos = saved.get(id);
            nodes.push(GraphNode {
                id: *id,
                title: title.clone(),
                year: *year,
                color,
                degree: degree.get(id).copied().unwrap_or(0),
                unread: !is_read,
                favorite: *favorite,
                peer_reviewed: *peer,
                has_github: *gh,
                px,
                py,
                sx: pos.map(|p| p.0),
                sy: pos.map(|p| p.1),
                community: community_of.get(id).copied().unwrap_or(-1),
                kind: "doc".to_string(),
                slug: None,
            });
        }
    }

    // Note nodes (diamonds in the map). No PCA seed: with no saved position the
    // frontend places them next to their strongest paper; saved positions live
    // in note_graph_positions (id = -note_id, FK sulle note).
    let saved_notes: std::collections::HashMap<i64, (f32, f32)> = {
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare("SELECT note_id, x, y FROM note_graph_positions")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, (r.get::<_, f64>(1)? as f32, r.get::<_, f64>(2)? as f32)))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };
    for (nid, slug, title) in &notes {
        let id = -nid;
        let pos = saved_notes.get(nid);
        nodes.push(GraphNode {
            id,
            title: title.clone(),
            year: None,
            color: None,
            degree: degree.get(&id).copied().unwrap_or(0),
            unread: false,
            favorite: false,
            peer_reviewed: false,
            has_github: false,
            px: 0.0,
            py: 0.0,
            sx: pos.map(|p| p.0),
            sy: pos.map(|p| p.1),
            community: community_of.get(&id).copied().unwrap_or(-1),
            kind: "note".to_string(),
            slug: Some(slug.clone()),
        });
    }

    Ok(SimilarityGraphData { nodes, edges, clusters, embedded, total })
}

#[derive(serde::Deserialize)]
pub struct NodePos {
    pub id: i64,
    pub x: f32,
    pub y: f32,
}

/// Persist the Costellazione's settled layout positions (called by the frontend
/// when the simulation cools). Pure cache: rows for deleted docs are ignored and
/// eventually cascade away.
#[tauri::command]
pub fn save_graph_positions(
    state: State<'_, AppState>,
    positions: Vec<NodePos>,
) -> Result<(), String> {
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut doc_stmt = tx
            .prepare("INSERT OR REPLACE INTO graph_positions (document_id, x, y) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        // id negativi = appunti (id nodo = -note_id), su una tabella dedicata.
        let mut note_stmt = tx
            .prepare("INSERT OR REPLACE INTO note_graph_positions (note_id, x, y) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        for p in positions.iter().take(4000) {
            if !p.x.is_finite() || !p.y.is_finite() {
                continue;
            }
            if p.id < 0 {
                let _ = note_stmt.execute(params![-p.id, p.x as f64, p.y as f64]);
            } else {
                let _ = doc_stmt.execute(params![p.id, p.x as f64, p.y as f64]);
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

// ===== Controllo aggiornamenti: solo un avviso read-only, nessun auto-update =====

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub current: String,
    /// Versione pubblicata sul ramo main di GitHub; None se non raggiungibile
    /// (repository privato, offline, …).
    pub latest: Option<String>,
    pub newer: bool,
    pub url: String,
}

/// Confronta la versione in esecuzione con il package.json pubblicato su
/// GitHub. Non scarica né installa nulla: informa soltanto.
#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    const RAW: &str = "https://raw.githubusercontent.com/shelaik/scriptorium/main/package.json";
    const REPO: &str = "https://github.com/shelaik/scriptorium";
    let current = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .user_agent(concat!("scriptorium/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let latest = match client.get(RAW).send().await {
        Ok(r) if r.status().is_success() => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("version").and_then(|s| s.as_str()).map(String::from)),
        _ => None,
    };
    let newer = latest.as_deref().map(|l| version_newer(l, &current)).unwrap_or(false);
    Ok(UpdateInfo { current, latest, newer, url: REPO.into() })
}

/// true se `a` è più nuova di `b` (componenti numeriche, tollerante a "v" e suffissi).
fn version_newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (va, vb) = (parse(a), parse(b));
    for i in 0..va.len().max(vb.len()) {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

#[cfg(test)]
mod version_tests {
    use super::version_newer;

    #[test]
    fn version_ordering() {
        assert!(version_newer("1.0.0", "0.9.12"));
        assert!(version_newer("0.9.10", "0.9.9")); // numerico, non lessicografico
        assert!(version_newer("0.10.0", "0.9.99"));
        assert!(!version_newer("0.9.12", "0.9.12"));
        assert!(!version_newer("0.9.11", "0.9.12"));
        assert!(version_newer("v1.0.1", "1.0.0")); // prefisso v tollerato
        assert!(version_newer("1.0.0.1", "1.0.0")); // componente extra
        assert!(!version_newer("garbage", "0.1.0")); // non numerico → 0
    }
}

// ===== Progetti LaTeX: folders of .tex/.bib the user owns, compiled via system toolchain =====

/// Root folder of the LaTeX projects (created on demand).
fn projects_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("projects"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_projects"))
}

fn project_root(app: &AppHandle, slug: &str) -> Result<PathBuf, String> {
    // The slug doubles as the folder name: same guard as any relative segment.
    let s = projects::safe_rel(slug).map_err(|e| e.to_string())?;
    if s.contains('/') {
        return Err("nome progetto non valido".into());
    }
    let root = projects_dir(app).join(&s);
    if !root.is_dir() {
        return Err("progetto non trovato".into());
    }
    Ok(root)
}

#[tauri::command]
pub fn list_projects(app: AppHandle) -> Result<Vec<projects::ProjectMeta>, String> {
    Ok(projects::list(&projects_dir(&app)))
}

/// Scaffold a new project: folder + main.tex from a built-in template +
/// refs.bib synced from the whole library. Returns the slug.
#[tauri::command]
pub fn create_project(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    template: Option<String>,
) -> Result<String, String> {
    let root = new_project_root(&app, &name)?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let body = projects::template_body(template.as_deref().unwrap_or("articolo"));
    std::fs::write(root.join("main.tex"), body).map_err(|e| e.to_string())?;
    write_library_bib(&state, &root)?;
    Ok(slug_of(&root))
}

/// Create a project from a downloaded template .zip (Overleaf gallery, IEEE,
/// ACM, …): extract, ensure main.tex, add refs.bib only if the template has none.
#[tauri::command]
pub fn create_project_from_zip(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    zip_path: String,
) -> Result<String, String> {
    let root = new_project_root(&app, &name)?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if let Err(e) = projects::extract_template_zip(Path::new(&zip_path), &root) {
        let _ = std::fs::remove_dir_all(&root); // don't leave a broken half-project
        return Err(e.to_string());
    }
    if !root.join("refs.bib").exists() {
        write_library_bib(&state, &root)?;
    }
    Ok(slug_of(&root))
}

/// Validate the name and return the (not yet existing) project folder.
fn new_project_root(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if name.trim().is_empty() {
        return Err("Dai un nome al progetto".into());
    }
    let slug = wiki::slugify(name);
    let root = projects_dir(app).join(&slug);
    if root.exists() {
        return Err("Esiste già un progetto con questo nome".into());
    }
    Ok(root)
}

fn slug_of(root: &Path) -> String {
    root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

fn write_library_bib(state: &State<'_, AppState>, root: &Path) -> Result<(), String> {
    let bib = {
        let conn = state.db.lock();
        library_bibtex(&conn)?
    };
    std::fs::write(root.join("refs.bib"), bib).map_err(|e| e.to_string())
}

/// The whole (non-deleted) library as BibTeX — the project's refs.bib content.
fn library_bibtex(conn: &Connection) -> Result<String, String> {
    let ids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM documents WHERE deleted_at IS NULL ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };
    let items = load_cite_items(conn, &ids).map_err(|e| e.to_string())?;
    Ok(citation::build(&items, "bibtex"))
}

#[tauri::command]
pub fn project_files(app: AppHandle, slug: String) -> Result<Vec<projects::ProjectFile>, String> {
    Ok(projects::files(&project_root(&app, &slug)?))
}

#[tauri::command]
pub fn read_project_file(app: AppHandle, slug: String, rel: String) -> Result<String, String> {
    let rel = projects::safe_rel(&rel).map_err(|e| e.to_string())?;
    let p = project_root(&app, &slug)?.join(&rel);
    let md = std::fs::metadata(&p).map_err(|e| e.to_string())?;
    if md.len() > 2_000_000 {
        return Err("File troppo grande per l'editor".into());
    }
    std::fs::read_to_string(&p).map_err(|e| format!("Lettura {rel}: {e}"))
}

/// Atomic write (temp + rename), like the notes vault.
#[tauri::command]
pub fn write_project_file(app: AppHandle, slug: String, rel: String, content: String) -> Result<(), String> {
    let rel = projects::safe_rel(&rel).map_err(|e| e.to_string())?;
    let root = project_root(&app, &slug)?;
    let p = root.join(&rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let tmp = root.join(format!("{}.tmp-write", rel.replace('/', "_")));
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// A project file as base64 (the compiled PDF for the in-app preview).
#[tauri::command]
pub fn read_project_file_b64(app: AppHandle, slug: String, rel: String) -> Result<String, String> {
    let rel = projects::safe_rel(&rel).map_err(|e| e.to_string())?;
    let p = project_root(&app, &slug)?.join(&rel);
    let md = std::fs::metadata(&p).map_err(|e| e.to_string())?;
    if md.len() > 40_000_000 {
        return Err("File troppo grande".into());
    }
    let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
    Ok(BASE64_STANDARD.encode(bytes))
}

/// Rewrite refs.bib from the current library; returns the entry count.
#[tauri::command]
pub fn sync_project_bib(app: AppHandle, state: State<'_, AppState>, slug: String) -> Result<usize, String> {
    let root = project_root(&app, &slug)?;
    let bib = {
        let conn = state.db.lock();
        library_bibtex(&conn)?
    };
    let n = bib.lines().filter(|l| l.starts_with('@')).count();
    let tmp = root.join("refs.bib.tmp-write");
    std::fs::write(&tmp, bib).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, root.join("refs.bib")).map_err(|e| e.to_string())?;
    Ok(n)
}

/// Compile main.tex with the system toolchain (Tectonic, then latexmk).
#[tauri::command]
pub async fn compile_project(app: AppHandle, slug: String) -> Result<projects::CompileResult, String> {
    let root = project_root(&app, &slug)?;
    tauri::async_runtime::spawn_blocking(move || projects::compile(&root))
        .await
        .map_err(|e| e.to_string())
}

/// Open the project folder in the file explorer.
#[tauri::command]
pub fn reveal_project_dir(app: AppHandle, slug: String) -> Result<(), String> {
    let root = project_root(&app, &slug)?;
    run_powershell(&format!(
        "Start-Process explorer.exe -ArgumentList '\"{}\"'",
        ps_lit(&root.to_string_lossy())
    ))
}

// ===== RAG engine: "ask your library" (passage index + graph-augmented Q&A) =====

const RAG_CHUNK_CHARS: usize = 1000;
const RAG_OVERLAP_CHARS: usize = 150;
const RAG_MAX_CHUNKS_PER_DOC: usize = 120;
/// Ollama embedding model used for GPU indexing — bge-m3 to match the 1024-dim
/// CPU index, so vectors stay comparable.
const EMBED_OLLAMA_MODEL: &str = "bge-m3";

/// Embed a batch of texts on GPU (Ollama) or CPU (bundled bge-m3), per setting.
fn embed_texts(gpu: bool, ollama_url: &str, cache: &Path, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
    if gpu {
        let client = ai::client().map_err(|e| e.to_string())?;
        tauri::async_runtime::block_on(ai::embed_ollama(&client, ollama_url, EMBED_OLLAMA_MODEL, texts))
            .map_err(|e| format!("{e:#}"))
    } else {
        embed::embed_batch(cache, texts).map_err(|e| e.to_string())
    }
}

/// Embed a single query string, matching the index's embedding provider.
fn embed_query_text(gpu: bool, ollama_url: &str, cache: &Path, text: &str) -> Result<Vec<f32>, String> {
    if gpu {
        let client = ai::client().map_err(|e| e.to_string())?;
        let mut v = tauri::async_runtime::block_on(ai::embed_ollama(
            &client,
            ollama_url,
            EMBED_OLLAMA_MODEL,
            vec![text.to_string()],
        ))
        .map_err(|e| format!("{e:#}"))?;
        v.pop().ok_or_else(|| "Ollama non ha restituito l'embedding".to_string())
    } else {
        embed::embed_query(cache, text).map_err(|e| e.to_string())
    }
}

#[derive(serde::Serialize)]
pub struct RagStatus {
    pub indexed_docs: i64,
    pub total_docs: i64,
    pub chunks: i64,
}

/// Extract + chunk one document: page-attributed from the PDF when possible,
/// else from the stored fulltext (page = None). Pure CPU work (no DB), so the
/// indexing pipeline can run it on a producer thread.
fn chunk_document(
    pdfium: &pdfium_render::prelude::Pdfium,
    path: &str,
    fulltext: &str,
) -> Vec<(String, Option<i64>)> {
    let mut chunks: Vec<(String, Option<i64>)> = Vec::new();
    if !path.trim().is_empty() && std::path::Path::new(path).exists() {
        if let Ok(pages) = pdf::extract_pages(pdfium, std::path::Path::new(path)) {
            for (i, page_text) in pages.iter().enumerate() {
                for c in rag::chunk_text(page_text, RAG_CHUNK_CHARS, RAG_OVERLAP_CHARS, RAG_MAX_CHUNKS_PER_DOC) {
                    chunks.push((c, Some(i as i64 + 1)));
                    if chunks.len() >= RAG_MAX_CHUNKS_PER_DOC {
                        break;
                    }
                }
                if chunks.len() >= RAG_MAX_CHUNKS_PER_DOC {
                    break;
                }
            }
            return chunks;
        }
    }
    for c in rag::chunk_text(fulltext, RAG_CHUNK_CHARS, RAG_OVERLAP_CHARS, RAG_MAX_CHUNKS_PER_DOC) {
        chunks.push((c, None));
    }
    chunks
}

/// How many docs have passage chunks vs. how many are eligible (have fulltext).
#[tauri::command]
pub fn rag_index_status(state: State<'_, AppState>) -> Result<RagStatus, String> {
    let conn = state.db.lock();
    let total_docs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE deleted_at IS NULL AND fulltext IS NOT NULL AND length(trim(fulltext)) > 0",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let indexed_docs: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT dc.document_id) FROM doc_chunks dc
             JOIN documents d ON d.id = dc.document_id WHERE d.deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM doc_chunks", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(RagStatus { indexed_docs, total_docs, chunks })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RagProgress {
    pub done: usize,
    pub total: usize,
    pub phase: String, // "running" | "done" | "cancelled"
}

#[tauri::command]
pub fn cancel_rag_index(state: State<'_, AppState>) {
    state.rag_cancel.store(true, Ordering::SeqCst);
}

/// Drop the whole passage index so the next build re-chunks from scratch (e.g. to
/// pick up page attribution for documents indexed before that existed).
#[tauri::command]
pub fn clear_rag_index(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM chunk_vec", []).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM doc_chunks", []).map_err(|e| e.to_string())?;
    Ok(())
}

/// Chunk + embed every eligible document not yet indexed (incremental, resumable,
/// cancellable). Runs in the background and emits `rag-progress` events.
#[tauri::command]
pub async fn build_rag_index(app: AppHandle) -> Result<usize, String> {
    let cache = embed_cache_dir(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.rag_cancel.store(false, Ordering::SeqCst);

        let (gpu, ollama_url, batch_cfg) = {
            let conn = state.db.lock();
            let c = ai_config(&conn);
            (c.embed_gpu, c.ollama_url, c.embed_batch)
        };
        // Bigger batches keep a strong GPU busy; CPU does better with small ones.
        let batch = if batch_cfg > 0 {
            (batch_cfg as usize).clamp(1, 512)
        } else if gpu {
            64
        } else {
            16
        };

        let docs: Vec<(i64, String, String)> = {
            let conn = state.db.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT id, COALESCE(path, ''), COALESCE(fulltext, '') FROM documents
                     WHERE deleted_at IS NULL AND fulltext IS NOT NULL AND length(trim(fulltext)) > 0
                       AND id NOT IN (SELECT DISTINCT document_id FROM doc_chunks)",
                )
                .map_err(|e| e.to_string())?;
            let v = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
            v
        };

        let total = docs.len();
        let _ = app.emit("rag-progress", RagProgress { done: 0, total, phase: "running".into() });
        if total == 0 {
            let _ = app.emit("rag-progress", RagProgress { done: 0, total, phase: "done".into() });
            return Ok(0);
        }

        // Pipeline: a producer thread extracts + chunks the NEXT document (CPU)
        // while this thread embeds + inserts the current one (GPU/CPU + DB), so the
        // GPU is never left waiting on PDF parsing. A bounded channel backpressures.
        let (tx, rx) = std::sync::mpsc::sync_channel::<(i64, Vec<(String, Option<i64>)>)>(2);
        let app_prod = app.clone();
        let producer = std::thread::spawn(move || {
            let state = app_prod.state::<AppState>();
            for (doc_id, path, fulltext) in docs {
                if state.rag_cancel.load(Ordering::SeqCst) {
                    break;
                }
                let chunks = chunk_document(&state.pdfium, &path, &fulltext);
                if tx.send((doc_id, chunks)).is_err() {
                    break;
                }
            }
        });

        // Consumer (this thread): embed in `batch`-sized groups + insert per doc in
        // a single transaction. A closure lets us use `?`; the producer is joined after.
        let consume = || -> Result<usize, String> {
            let mut done = 0usize;
            while let Ok((doc_id, chunks)) = rx.recv() {
                if state.rag_cancel.load(Ordering::SeqCst) {
                    return Ok(done);
                }
                if chunks.is_empty() {
                    done += 1;
                    let _ = app.emit("rag-progress", RagProgress { done, total, phase: "running".into() });
                    continue;
                }
                let texts: Vec<String> = chunks.iter().map(|(t, _)| t.clone()).collect();
                let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
                for b in texts.chunks(batch) {
                    let mut v = embed_texts(gpu, &ollama_url, &cache, b.to_vec())
                        .map_err(|e| format!("doc {doc_id}: {e}"))?;
                    vectors.append(&mut v);
                }
                {
                    let mut conn = state.db.lock();
                    let txn = conn.transaction().map_err(|e| e.to_string())?;
                    for (ord, ((text, page), vec)) in chunks.iter().zip(vectors.iter()).enumerate() {
                        if txn
                            .execute(
                                "INSERT INTO doc_chunks (document_id, ord, text, page) VALUES (?1, ?2, ?3, ?4)",
                                params![doc_id, ord as i64, text, page],
                            )
                            .is_ok()
                        {
                            let chunk_id = txn.last_insert_rowid();
                            let _ = txn.execute(
                                "INSERT INTO chunk_vec (chunk_id, embedding) VALUES (?1, ?2)",
                                params![chunk_id, vec.as_slice().as_bytes()],
                            );
                        }
                    }
                    txn.commit().map_err(|e| e.to_string())?;
                }
                done += 1;
                let _ = app.emit("rag-progress", RagProgress { done, total, phase: "running".into() });
            }
            Ok(done)
        };
        let outcome = consume();
        drop(rx); // unblock the producer if we stopped early, so join() returns
        let _ = producer.join();

        let phase = if outcome.is_ok() && state.rag_cancel.load(Ordering::SeqCst) {
            "cancelled"
        } else {
            "done"
        };
        let done_n = *outcome.as_ref().unwrap_or(&0);
        let _ = app.emit("rag-progress", RagProgress { done: done_n, total, phase: phase.into() });
        outcome
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
pub struct AskSource {
    pub n: usize,
    pub document_id: i64,
    pub title: String,
    pub ord: i64,
    pub page: Option<i64>,
    pub excerpt: String,
    pub relation: String, // "match" | "citazione" | "simile"
}

#[derive(serde::Serialize)]
pub struct AskResult {
    pub answer: String,
    pub sources: Vec<AskSource>,
}

fn bytes_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// A retrieved passage with provenance (internal).
struct Retrieved {
    document_id: i64,
    title: String,
    ord: i64,
    page: Option<i64>,
    text: String,
    relation: &'static str,
}

/// Best-matching chunk of a single document for the query vector (graph expansion).
fn best_chunk(conn: &Connection, doc_id: i64, qvec: &[f32]) -> Option<(i64, String, Option<i64>)> {
    let mut stmt = conn
        .prepare("SELECT id, ord, text, page FROM doc_chunks WHERE document_id = ?1")
        .ok()?;
    let rows: Vec<(i64, i64, String, Option<i64>)> = stmt
        .query_map(params![doc_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .ok()?
        .filter_map(Result::ok)
        .collect();
    let mut best: Option<(i64, String, Option<i64>, f32)> = None;
    for (cid, ord, text, page) in rows {
        let emb: Option<Vec<u8>> = conn
            .query_row("SELECT embedding FROM chunk_vec WHERE chunk_id = ?1", params![cid], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        if let Some(bytes) = emb {
            let score = rag::cosine(qvec, &bytes_to_f32(&bytes));
            if best.as_ref().map(|b| score > b.3).unwrap_or(true) {
                best = Some((ord, text, page, score));
            }
        }
    }
    best.map(|(ord, text, page, _)| (ord, text, page))
}

/// Ask a question over the library: passage retrieval + graph expansion
/// (citations + similar docs) + one local-LLM answer with citations.
#[tauri::command]
pub async fn ask_library(
    app: AppHandle,
    question: String,
    scope_kind: Option<String>,
    scope_id: Option<i64>,
) -> Result<AskResult, String> {
    if question.trim().is_empty() {
        return Err("Scrivi una domanda".into());
    }
    let cache = embed_cache_dir(&app);
    let (enabled, provider, url, model, gpu, ollama_url) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone(), c.embed_gpu, c.ollama_url.clone())
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }

    // Retrieval (CPU-bound) off the async thread.
    let app2 = app.clone();
    let q2 = question.clone();
    let scope_kind2 = scope_kind.clone();
    let ollama_url2 = ollama_url.clone();
    let retrieved: Vec<Retrieved> = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<Retrieved>, String> {
        let qvec = embed_query_text(gpu, &ollama_url2, &cache, &q2).map_err(|e| e.to_string())?;
        let state = app2.state::<AppState>();
        let conn = state.db.lock();

        // Optional scope: restrict retrieval to a single doc / collection / tag.
        let allowed: Option<std::collections::HashSet<i64>> = match (scope_kind2.as_deref(), scope_id) {
            (Some("doc"), Some(id)) => Some(std::iter::once(id).collect()),
            (Some("collection"), Some(id)) => {
                let mut s = conn
                    .prepare("SELECT document_id FROM collection_members WHERE collection_id = ?1")
                    .map_err(|e| e.to_string())?;
                let set = s
                    .query_map(params![id], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect();
                Some(set)
            }
            (Some("tag"), Some(id)) => {
                let mut s = conn
                    .prepare("SELECT document_id FROM document_tags WHERE tag_id = ?1")
                    .map_err(|e| e.to_string())?;
                let set = s
                    .query_map(params![id], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect();
                Some(set)
            }
            _ => None,
        };
        let scoped = allowed.is_some();

        // Primary: top passages by vector similarity (over-fetch; orphaned/deleted
        // and out-of-scope chunks are skipped, keeping the top 10 that qualify).
        let k = if scoped { 120i64 } else { 30i64 };
        let chunk_ids: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT chunk_id FROM chunk_vec WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance")
                .map_err(|e| e.to_string())?;
            let v: Vec<i64> = stmt
                .query_map(params![qvec.as_slice().as_bytes(), k], |r| r.get::<_, i64>(0))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
            v
        };

        let mut out: Vec<Retrieved> = Vec::new();
        let mut seen_docs: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for cid in &chunk_ids {
            let row = conn
                .query_row(
                    "SELECT dc.document_id, dc.ord, dc.text, dc.page, COALESCE(d.title, 'Senza titolo')
                     FROM doc_chunks dc JOIN documents d ON d.id = dc.document_id
                     WHERE dc.id = ?1 AND d.deleted_at IS NULL",
                    params![cid],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<i64>>(3)?, r.get::<_, String>(4)?)),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some((doc_id, ord, text, page, title)) = row {
                if let Some(set) = &allowed {
                    if !set.contains(&doc_id) {
                        continue;
                    }
                }
                out.push(Retrieved { document_id: doc_id, title, ord, page, text, relation: "match" });
                seen_docs.insert(doc_id);
            }
            if out.len() >= 10 {
                break;
            }
        }

        // Graph expansion: from the single best-matching doc, pull a few neighbours
        // (cited/citing in-library + semantically similar) and add their best chunk.
        // Skipped when a scope is active (stay within the requested subset).
        if let Some(top) = out.first().map(|r| r.document_id).filter(|_| !scoped) {
            let mut neighbours: Vec<(i64, &'static str)> = Vec::new();

            // Citation neighbours (both directions), matched within the library.
            let doi: Option<String> = conn
                .query_row("SELECT doi FROM documents WHERE id = ?1", params![top], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?
                .flatten();
            {
                let mut s = conn
                    .prepare(
                        "SELECT d.id FROM document_references dr
                         JOIN documents d ON LOWER(d.doi) = LOWER(dr.ref_doi)
                         WHERE dr.document_id = ?1 AND d.deleted_at IS NULL LIMIT 4",
                    )
                    .map_err(|e| e.to_string())?;
                for id in s.query_map(params![top], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.flatten() {
                    neighbours.push((id, "citazione"));
                }
            }
            if let Some(d) = doi.as_deref().filter(|s| !s.trim().is_empty()) {
                let mut s = conn
                    .prepare(
                        "SELECT dr.document_id FROM document_references dr
                         JOIN documents d ON d.id = dr.document_id
                         WHERE LOWER(dr.ref_doi) = LOWER(?1) AND d.deleted_at IS NULL LIMIT 4",
                    )
                    .map_err(|e| e.to_string())?;
                for id in s.query_map(params![d], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.flatten() {
                    neighbours.push((id, "citazione"));
                }
            }
            // Semantically similar documents.
            let emb: Option<Vec<u8>> = conn
                .query_row("SELECT embedding FROM doc_vec WHERE document_id = ?1", params![top], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some(e) = emb {
                let mut s = conn
                    .prepare("SELECT document_id FROM doc_vec WHERE embedding MATCH ?1 AND k = 5 ORDER BY distance")
                    .map_err(|e| e.to_string())?;
                for id in s.query_map(params![e], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.flatten() {
                    neighbours.push((id, "simile"));
                }
            }

            let mut added = 0;
            for (nid, rel) in neighbours {
                if added >= 3 || out.len() >= 14 {
                    break;
                }
                if seen_docs.contains(&nid) {
                    continue;
                }
                if let Some((ord, text, page)) = best_chunk(&conn, nid, &qvec) {
                    let title: String = conn
                        .query_row("SELECT COALESCE(title, 'Senza titolo') FROM documents WHERE id = ?1", params![nid], |r| r.get(0))
                        .optional()
                        .map_err(|e| e.to_string())?
                        .unwrap_or_else(|| "Senza titolo".into());
                    out.push(Retrieved { document_id: nid, title, ord, page, text, relation: rel });
                    seen_docs.insert(nid);
                    added += 1;
                }
            }
        }

        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())??;

    if retrieved.is_empty() {
        return Err("Nessun passaggio trovato. Costruisci l'indice (Chiedi alla libreria → Costruisci indice) e verifica che i PDF contengano testo.".into());
    }

    // Build the cited context + sources list.
    let mut context = String::new();
    let mut sources: Vec<AskSource> = Vec::new();
    for (i, r) in retrieved.iter().enumerate() {
        let n = i + 1;
        context.push_str(&format!("[{}] «{}»\n{}\n\n", n, r.title, r.text));
        sources.push(AskSource {
            n,
            document_id: r.document_id,
            title: r.title.clone(),
            ord: r.ord,
            page: r.page,
            excerpt: ai::truncate(r.text.trim(), 500),
            relation: r.relation.to_string(),
        });
    }
    let prompt = format!(
        "Sei un assistente di ricerca. Rispondi alla DOMANDA usando SOLO i passaggi numerati qui sotto, in italiano, in modo chiaro e conciso. Dopo ogni affermazione cita la fonte tra parentesi quadre, es. [1] o [2][3]. Se i passaggi non contengono la risposta, dillo onestamente senza inventare.\n\nDOMANDA: {question}\n\nPASSAGGI:\n{context}\nRISPOSTA (in italiano, con citazioni [n]):"
    );
    let client = ai::client().map_err(|e| e.to_string())?;
    let app3 = app.clone();
    let answer = ai::generate_stream(&client, &provider, &url, &model, &prompt, 700, |t| {
        let _ = app3.emit("ask-token", t);
    })
    .await
    .map_err(|e| format!("{e:#}"))?;
    if answer.trim().is_empty() {
        return Err("Il modello non ha prodotto una risposta".into());
    }
    Ok(AskResult { answer, sources })
}

/// Build an FTS5 MATCH expression from raw user input: alphanumeric tokens as
/// prefix queries, joined with implicit AND.
fn fts_query(raw: &str) -> String {
    raw.split_whitespace()
        .map(|t| t.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|t| !t.is_empty())
        .map(|t| format!("{t}*"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reciprocal Rank Fusion of two ranked id lists.
fn rrf_merge(a: &[i64], b: &[i64], limit: usize) -> Vec<i64> {
    use std::collections::HashMap;
    const K: f64 = 60.0;
    let mut score: HashMap<i64, f64> = HashMap::new();
    for (rank, &id) in a.iter().enumerate() {
        *score.entry(id).or_default() += 1.0 / (K + rank as f64 + 1.0);
    }
    for (rank, &id) in b.iter().enumerate() {
        *score.entry(id).or_default() += 1.0 / (K + rank as f64 + 1.0);
    }
    let mut scored: Vec<(i64, f64)> = score.into_iter().collect();
    scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(limit).map(|(id, _)| id).collect()
}

/// Fetch documents by id, preserving the order of `ids`.
fn fetch_documents(conn: &Connection, ids: &[i64], include_deleted: bool) -> anyhow::Result<Vec<Document>> {
    let sql = if include_deleted {
        "SELECT id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey, last_page, page_count, (summary IS NOT NULL AND TRIM(summary) != ''), is_own FROM documents WHERE id = ?1"
    } else {
        "SELECT id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey, last_page, page_count, (summary IS NOT NULL AND TRIM(summary) != ''), is_own FROM documents WHERE id = ?1 AND deleted_at IS NULL"
    };
    let mut doc_stmt = conn.prepare(sql)?;
    let mut author_stmt = conn.prepare(
        "SELECT a.given, a.family
         FROM authors a JOIN document_authors da ON da.author_id = a.id
         WHERE da.document_id = ?1 ORDER BY da.position",
    )?;
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        let row = doc_stmt
            .query_row(params![id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                    r.get::<_, Option<i64>>(13)?,
                    r.get::<_, i64>(14)?,
                    r.get::<_, i64>(15)?,
                ))
            })
            .optional()?;
        let Some((id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey, last_page, page_count, has_summary, is_own)) = row else {
            continue;
        };
        let pub_status = discovery::classify_pub_status(doi.as_deref(), venue.as_deref(), path.as_deref());
        let paper_url = paper_link_for(doi.as_deref(), path.as_deref());
        let authors: Vec<String> = author_stmt
            .query_map(params![id], |r| {
                let given: Option<String> = r.get(0)?;
                let family: Option<String> = r.get(1)?;
                Ok(format!(
                    "{} {}",
                    given.unwrap_or_default(),
                    family.unwrap_or_default()
                )
                .trim()
                .to_string())
            })?
            .filter_map(Result::ok)
            .filter(|s| !s.is_empty())
            .collect();
        out.push(Document {
            id,
            title,
            year,
            venue,
            doi,
            authors,
            tags: load_tags(conn, id).unwrap_or_default(),
            has_thumb: thumb_path.map(|t| !t.is_empty()).unwrap_or(false),
            has_file: path.as_deref().map(|p| !p.starts_with("ref:")).unwrap_or(false),
            has_summary: has_summary != 0,
            added_at,
            is_read: is_read != 0,
            favorite: favorite != 0,
            github_url,
            pub_status,
            paper_url,
            citekey,
            last_page,
            page_count,
            is_own: is_own != 0,
        });
    }
    Ok(out)
}

// ===== Tags =====

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color, COUNT(d.id) AS cnt \
             FROM tags t \
             LEFT JOIN document_tags dt ON dt.tag_id = t.id \
             LEFT JOIN documents d ON d.id = dt.document_id AND d.deleted_at IS NULL \
             GROUP BY t.id, t.name, t.color \
             ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                count: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

/// Rename and/or recolor a tag (name stays library-unique, case-insensitive).
#[tauri::command]
pub fn update_tag(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    color: Option<String>,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Il nome del tag non può essere vuoto".into());
    }
    let conn = state.db.lock();
    let clash: Option<i64> = conn
        .query_row(
            "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE AND id != ?2",
            params![name, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if clash.is_some() {
        return Err(format!("Esiste già un tag «{name}»"));
    }
    conn.execute("UPDATE tags SET name = ?1, color = ?2 WHERE id = ?3", params![name, color, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Create a tag (or return the existing one with that name), setting its color.
#[tauri::command]
pub fn create_tag(state: State<'_, AppState>, name: String, color: Option<String>) -> Result<Tag, String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO tags (name, color) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET color = COALESCE(excluded.color, tags.color)",
        params![name, color],
    )
    .map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT t.id, t.name, t.color, \
            (SELECT COUNT(*) FROM document_tags dt JOIN documents d \
             ON d.id = dt.document_id AND d.deleted_at IS NULL WHERE dt.tag_id = t.id) AS cnt \
         FROM tags t WHERE t.name = ?1",
        params![name],
        |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                count: r.get(3)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Replace the set of tags on a document.
#[tauri::command]
pub fn set_document_tags(
    state: State<'_, AppState>,
    document_id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), String> {
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM document_tags WHERE document_id = ?1", params![document_id])
        .map_err(|e| e.to_string())?;
    for tid in &tag_ids {
        tx.execute(
            "INSERT OR IGNORE INTO document_tags (document_id, tag_id) VALUES (?1, ?2)",
            params![document_id, tid],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// ===== Manual metadata editing =====

fn opt(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Parse a display author string into (given, family).
/// "Family, Given" → comma split; otherwise the last word is the family name.
fn parse_author(s: &str) -> (Option<String>, Option<String>) {
    let s = s.trim();
    if let Some((fam, giv)) = s.split_once(',') {
        return (opt(giv), opt(fam));
    }
    match s.rsplit_once(' ') {
        Some((giv, fam)) => (opt(giv), opt(fam)),
        None => (None, opt(s)),
    }
}

/// Full editable metadata for a document.
#[tauri::command]
pub fn get_document_meta(state: State<'_, AppState>, id: i64) -> Result<EditableMeta, String> {
    let conn = state.db.lock();
    let (title, year, venue, doi, abstract_text, notes, summary) = conn
        .query_row(
            "SELECT title, year, venue, doi, abstract, notes, summary FROM documents WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT a.given, a.family FROM authors a
             JOIN document_authors da ON da.author_id = a.id
             WHERE da.document_id = ?1 ORDER BY da.position",
        )
        .map_err(|e| e.to_string())?;
    let authors: Vec<String> = stmt
        .query_map(params![id], |r| {
            let g: Option<String> = r.get(0)?;
            let f: Option<String> = r.get(1)?;
            Ok(format!("{} {}", g.unwrap_or_default(), f.unwrap_or_default())
                .trim()
                .to_string())
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|s| !s.is_empty())
        .collect();
    Ok(EditableMeta {
        title,
        authors,
        year,
        venue,
        doi,
        abstract_text,
        notes,
        summary,
    })
}

/// Save manually-edited metadata for a document.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn update_document_metadata(
    state: State<'_, AppState>,
    id: i64,
    title: Option<String>,
    authors: Vec<String>,
    year: Option<i64>,
    venue: Option<String>,
    doi: Option<String>,
    abstract_text: Option<String>,
    notes: Option<String>,
) -> Result<(), String> {
    let doi = doi.and_then(|d| opt(&d));
    let mut conn = state.db.lock();

    if let Some(d) = &doi {
        let dup: Option<i64> = conn
            .query_row(
                "SELECT id FROM documents WHERE doi = ?1 AND id <> ?2",
                params![d, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if dup.is_some() {
            return Err("Questo DOI è già usato da un altro documento".to_string());
        }
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE documents SET title = ?1, year = ?2, venue = ?3, doi = ?4, abstract = ?5, notes = ?6 WHERE id = ?7",
        params![
            title.and_then(|s| opt(&s)),
            year,
            venue.and_then(|s| opt(&s)),
            doi,
            abstract_text.and_then(|s| opt(&s)),
            notes.and_then(|s| opt(&s)),
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM document_authors WHERE document_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    for (pos, a) in authors.iter().enumerate() {
        let (given, family) = parse_author(a);
        if given.is_none() && family.is_none() {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO authors (family, given) VALUES (?1, ?2)",
            params![family, given],
        )
        .map_err(|e| e.to_string())?;
        let aid: i64 = tx
            .query_row(
                "SELECT id FROM authors WHERE family IS ?1 AND given IS ?2",
                params![family, given],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT OR IGNORE INTO document_authors (document_id, author_id, position) VALUES (?1, ?2, ?3)",
            params![id, aid, pos as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    // Keep the stored citekey in sync with the edited author/year/title.
    let _ = crate::db::citekey::auto_citekey(&conn, id);
    Ok(())
}

// ===== Read / favorite / last page / backup =====

/// Set the read flag on a document.
#[tauri::command]
pub fn set_read(state: State<'_, AppState>, id: i64, value: bool) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE documents SET is_read = ?1 WHERE id = ?2",
        params![value as i64, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Set the favorite flag on a document.
#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, id: i64, value: bool) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE documents SET favorite = ?1 WHERE id = ?2",
        params![value as i64, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remember the last viewed page (1-based) of a document, and its total page
/// count (so older documents get a `page_count` the first time they're opened —
/// it powers the reading-progress bar). `pages` is optional/best-effort.
#[tauri::command]
pub fn set_last_page(state: State<'_, AppState>, id: i64, page: i64, pages: Option<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE documents
         SET last_page = ?1, last_opened_at = datetime('now'),
             page_count = COALESCE(?2, page_count)
         WHERE id = ?3",
        params![page, pages.filter(|p| *p > 0), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct OcrSummary {
    /// Pages of the PDF that were OCR'd (<= total_pages; capped for huge files).
    pub pages: usize,
    pub total_pages: usize,
    pub chars: usize,
    /// True when the PDF had more pages than the OCR cap, so some were skipped.
    pub truncated: bool,
}

/// OCR a scanned PDF that has no text layer: rasterize its pages with pdfium and
/// run the built-in Windows OCR engine, then store the recognised text as the
/// document's `fulltext` (the FTS triggers re-index it). Runs off the UI thread.
/// Refuses to overwrite a document that already has extracted text.
#[tauri::command]
pub async fn ocr_document(app: AppHandle, id: i64) -> Result<OcrSummary, String> {
    const MAX_OCR_PAGES: usize = 40;
    tauri::async_runtime::spawn_blocking(move || -> Result<OcrSummary, String> {
        let state = app.state::<AppState>();
        let path = {
            let conn = state.db.lock();
            // Guard: never clobber a document that already has real text.
            let existing: Option<String> = conn
                .query_row("SELECT fulltext FROM documents WHERE id = ?1", params![id], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?
                .flatten();
            if existing.map(|t| !t.trim().is_empty()).unwrap_or(false) {
                return Err("Il documento ha già del testo estratto; OCR annullato per non sovrascriverlo".to_string());
            }
            resolve_existing_path(&conn, id)?
                .ok_or_else(|| "Nessun file PDF su disco per questo documento".to_string())?
        };
        let out = crate::ocr::ocr_pdf(&state.pdfium, std::path::Path::new(&path), MAX_OCR_PAGES)
            .map_err(|e| format!("{e:#}"))?;
        let trimmed = out.text.trim();
        if trimmed.is_empty() {
            return Err("OCR non ha riconosciuto testo in questo PDF".to_string());
        }
        let chars = trimmed.chars().count();
        {
            // Re-check the guard inside the same lock to avoid a TOCTOU overwrite.
            let conn = state.db.lock();
            let n = conn
                .execute(
                    "UPDATE documents SET fulltext = ?1
                     WHERE id = ?2 AND (fulltext IS NULL OR TRIM(fulltext) = '')",
                    params![trimmed, id],
                )
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("Il documento ha già del testo estratto; OCR annullato".to_string());
            }
        }
        Ok(OcrSummary {
            pages: out.pages_ocred,
            total_pages: out.total_pages,
            chars,
            truncated: out.total_pages > out.pages_ocred,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The most recently opened documents (for the "Continue reading" shelf).
#[tauri::command]
pub fn recent_documents(state: State<'_, AppState>, limit: i64) -> Result<Vec<Document>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id FROM documents
             WHERE deleted_at IS NULL AND last_opened_at IS NOT NULL
             ORDER BY last_opened_at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = stmt
        .query_map(params![limit.max(0)], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);
    fetch_documents(&conn, &ids, false).map_err(|e| e.to_string())
}

/// All documents that have an author matching `name` (exact, case-insensitive).
#[tauri::command]
pub fn documents_by_author(state: State<'_, AppState>, name: String) -> Result<Vec<Document>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT da.document_id
             FROM document_authors da
             JOIN authors a ON a.id = da.author_id
             JOIN documents d ON d.id = da.document_id
             WHERE d.deleted_at IS NULL
               AND LOWER(TRIM(COALESCE(a.given,'') || ' ' || COALESCE(a.family,''))) = LOWER(TRIM(?1))
             ORDER BY da.document_id DESC",
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = stmt
        .query_map(params![name], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);
    fetch_documents(&conn, &ids, false).map_err(|e| e.to_string())
}

/// The last viewed page of a document, or None.
#[tauri::command]
pub fn get_last_page(state: State<'_, AppState>, id: i64) -> Result<Option<i64>, String> {
    let conn = state.db.lock();
    conn.query_row(
        "SELECT last_page FROM documents WHERE id = ?1",
        params![id],
        |r| r.get::<_, Option<i64>>(0),
    )
    .optional()
    .map(|o| o.flatten())
    .map_err(|e| e.to_string())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        // Do not follow symlinks during backup (avoids escaping the data dir).
        if ft.is_symlink() {
            continue;
        }
        let to = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Copy the whole library data folder (DB + PDFs + thumbnails) into `dest`.
/// Returns the path of the created backup folder.
#[tauri::command]
pub fn backup_library(app: AppHandle, dest: String) -> Result<String, String> {
    let src = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let target = std::path::Path::new(&dest).join(format!("Scriptorium-backup-{stamp}"));
    copy_dir_all(&src, &target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().to_string())
}

// ===== Add by identifier =====

#[derive(Debug, Clone, serde::Serialize)]
pub struct AddSummary {
    pub added: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

fn create_reference(conn: &mut Connection, rref: &metadata::ResolvedRef) -> anyhow::Result<Option<i64>> {
    // Dedup by synthetic path or by DOI.
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM documents WHERE path = ?1 OR (doi IS NOT NULL AND doi = ?2)",
            params![rref.path_id, rref.doi],
            |r| r.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Ok(None);
    }
    // Detect a GitHub repo in the reference's abstract/title (no fulltext for refs).
    let gh_text = format!(
        "{} {}",
        rref.meta.title.as_deref().unwrap_or(""),
        rref.meta.abstract_text.as_deref().unwrap_or("")
    );
    let github_url = github::first_repo_url(&gh_text);
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO documents (title, year, venue, doi, abstract, path, github_url) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            rref.meta.title,
            rref.meta.year,
            rref.meta.venue,
            rref.doi,
            rref.meta.abstract_text,
            rref.path_id,
            github_url
        ],
    )?;
    let id = tx.last_insert_rowid();
    for (pos, a) in rref.meta.authors.iter().enumerate() {
        if a.given.is_none() && a.family.is_none() {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO authors (family, given) VALUES (?1, ?2)",
            params![a.family, a.given],
        )?;
        let aid: i64 = tx.query_row(
            "SELECT id FROM authors WHERE family IS ?1 AND given IS ?2",
            params![a.family, a.given],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO document_authors (document_id, author_id, position) VALUES (?1, ?2, ?3)",
            params![id, aid, pos as i64],
        )?;
    }
    tx.commit()?;
    // Stored citekey from the reference's metadata (authors are now committed).
    let _ = crate::db::citekey::auto_citekey(conn, id);
    Ok(Some(id))
}

/// Add reference-only items (no PDF) from pasted identifiers: DOI, arXiv, ISBN, PMID.
#[tauri::command]
pub async fn add_by_identifiers(app: AppHandle, identifiers: Vec<String>) -> Result<AddSummary, String> {
    let email = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty())
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let mut summary = AddSummary {
        added: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    for raw in identifiers {
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            continue;
        }
        match metadata::resolve(&client, &raw, email.as_deref()).await {
            Ok(Some(rref)) => {
                let state = app.state::<AppState>();
                let mut conn = state.db.lock();
                match create_reference(&mut conn, &rref) {
                    Ok(Some(_)) => summary.added += 1,
                    Ok(None) => summary.skipped += 1,
                    Err(e) => summary.errors.push(format!("{raw}: {e:#}")),
                }
            }
            Ok(None) => summary.errors.push(format!("{raw}: nessun risultato trovato")),
            Err(e) => summary.errors.push(format!("{raw}: {e:#}")),
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    Ok(summary)
}

/// Convert a parsed BibTeX entry into a reference (None if it has no usable data).
/// Strip TeX control sequences and math/brace punctuation from a bibliography
/// title so an online title search isn't polluted by markup (e.g. `\emph{Attn}$x$`
/// → `Attn x`). Recall-only: precision stays enforced by the Crossref title gate.
fn detex_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
                // control word (\emph, \textbf…) → drop it, leave a word boundary
                while chars.peek().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
                    chars.next();
                }
                out.push(' ');
            } else {
                // control symbol (accent like \" \' \`) → drop it, keep the letter attached
                chars.next();
            }
        } else if c == '{' || c == '}' || c == '$' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bib_to_ref(e: &bibtex::BibEntry) -> Option<metadata::ResolvedRef> {
    let f = &e.fields;
    let get = |k: &str| f.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let title = get("title");
    // Route the DOI field through extract_doi so any wrapper (doi.org, dx.doi.org,
    // "doi:", trailing punctuation) is normalized to a bare lowercase DOI. Fall back
    // to a DOI embedded in the `url` field (e.g. url = {https://doi.org/10.x/y}).
    let doi = get("doi")
        .and_then(|d| metadata::extract_doi(&d))
        // Only mine a DOI from a genuine doi.org link — a DOI-shaped token buried in
        // some other URL (proxy path, etc.) may not be the work's real DOI.
        .or_else(|| {
            get("url")
                .filter(|u| u.to_lowercase().contains("doi.org/"))
                .and_then(|u| metadata::extract_doi(&u))
        })
        .map(|d| d.to_lowercase());
    if title.is_none() && doi.is_none() {
        return None;
    }
    let year = get("year")
        .or_else(|| get("date"))
        .and_then(|y| {
            let digits: String = y.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<i64>().ok()
        });
    let venue = get("journal").or_else(|| get("booktitle")).or_else(|| get("publisher"));
    let authors = f
        .get("author")
        .map(|a| {
            bibtex::split_authors(a)
                .into_iter()
                .map(|(given, family)| metadata::Author { given, family })
                .collect()
        })
        .unwrap_or_default();
    let path_id = match &doi {
        Some(d) => format!("ref:doi:{d}"),
        None => {
            let token = if !e.key.is_empty() {
                e.key.clone()
            } else {
                title.clone().unwrap_or_default()
            };
            format!("ref:bibtex:{}", safe_component(&token))
        }
    };
    Some(metadata::ResolvedRef {
        path_id,
        doi,
        meta: metadata::CrossrefMeta {
            title,
            venue,
            year,
            abstract_text: get("abstract"),
            authors,
            references: Vec::new(),
            raw_json: String::new(),
        },
    })
}

/// Import a BibTeX (.bib) file (e.g. a Zotero/Mendeley export) as reference-only items.
#[tauri::command]
pub fn import_bibtex(app: AppHandle, path: String) -> Result<AddSummary, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Lettura file: {e}"))?;
    let entries = bibtex::parse(&text);
    let state = app.state::<AppState>();
    let mut conn = state.db.lock();
    let mut summary = AddSummary { added: 0, skipped: 0, errors: Vec::new() };
    for e in &entries {
        match bib_to_ref(e) {
            Some(rref) => match create_reference(&mut conn, &rref) {
                Ok(Some(_)) => summary.added += 1,
                Ok(None) => summary.skipped += 1,
                Err(err) => summary.errors.push(format!("{}: {err:#}", e.key)),
            },
            None => summary.skipped += 1,
        }
    }
    Ok(summary)
}

/// Summary of a LaTeX-project (.zip) import.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LatexImportSummary {
    /// New "own work" PDFs added to the library.
    pub imported: usize,
    /// PDFs already present (still marked own + their references re-linked).
    pub duplicates: usize,
    /// PDF files found inside the archive.
    pub pdfs_found: usize,
    /// Total .bib entries parsed across every .bib in the archive.
    pub bib_entries: usize,
    /// Bibliography rows linked into the citation graph (document_references).
    pub references_linked: usize,
    /// Of those, how many lack a DOI (won't match citation-gap analysis yet).
    pub refs_without_doi: usize,
    /// DOIs recovered online (title → Crossref) for entries that had none, so the
    /// gap-finder can see them.
    pub dois_resolved: usize,
    pub errors: Vec<String>,
}

/// RAII cleanup for the extraction temp directory (fires on any return path).
struct TempDirGuard(PathBuf);
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Import a LaTeX project archive (.zip): add its compiled PDF(s) as the user's
/// own work and capture the .bib bibliography as the paper's reference graph.
/// fs + pdfium heavy, so it runs on a blocking thread. Entirely local (no network).
#[tauri::command]
pub async fn import_latex_zip(app: AppHandle, path: String) -> Result<LatexImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || import_latex_zip_inner(&app, &path))
        .await
        .map_err(|e| format!("Task fallito: {e}"))?
}

fn import_latex_zip_inner(app: &AppHandle, zip_path: &str) -> Result<LatexImportSummary, String> {
    use std::io::Read;
    let state = app.state::<AppState>();
    let data_dir = app.path().app_data_dir().map_err(|e| format!("app_data_dir: {e}"))?;
    let papers_dir = data_dir.join("papers");
    std::fs::create_dir_all(&papers_dir).map_err(|e| format!("Creazione papers/: {e}"))?;
    let extract_dir = {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        data_dir.join("tmp").join(format!("latex_{stamp}"))
    };
    std::fs::create_dir_all(&extract_dir).map_err(|e| format!("Creazione cartella temp: {e}"))?;
    let _cleanup = TempDirGuard(extract_dir.clone());

    let mut errors: Vec<String> = Vec::new();

    // --- 1. Open the archive; extract only PDFs (to disk) and .bib files (to memory).
    // Defensive bounds against a malformed/decompression-bomb archive (the user
    // picks the file, so this is belt-and-suspenders, not a security boundary).
    const MAX_ENTRIES: usize = 50_000;
    const MAX_PDF_BYTES: u64 = 300 * 1024 * 1024; // matches import::MAX_IMPORT_BYTES
    const MAX_BIB_BYTES: u64 = 32 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 3 * 1024 * 1024 * 1024;

    let file = std::fs::File::open(zip_path).map_err(|e| format!("Apertura .zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Lettura .zip: {e}"))?;
    let mut pdfs: Vec<(PathBuf, PathBuf)> = Vec::new(); // (abs pdf path on disk, relative dir)
    let mut bibs: Vec<(PathBuf, String)> = Vec::new(); //  (relative dir, .bib content)
    let n_entries = archive.len();
    if n_entries > MAX_ENTRIES {
        errors.push(format!("Archivio con troppe voci ({n_entries}): ne leggo {MAX_ENTRIES}"));
    }
    let mut total_bytes: u64 = 0;
    for i in 0..n_entries.min(MAX_ENTRIES) {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!("Voce zip {i}: {e}"));
                continue;
            }
        };
        if entry.is_dir() {
            continue;
        }
        // enclosed_name() rejects absolute paths and `..` traversal (zip-slip guard);
        // take an owned copy so the entry can then be read mutably.
        let rel = match entry.enclosed_name() {
            Some(r) => r.to_path_buf(),
            None => continue,
        };
        let ext = rel.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase());
        let rel_dir = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        if total_bytes >= MAX_TOTAL_BYTES {
            errors.push("Limite dimensione archivio raggiunto: import interrotto".into());
            break;
        }
        match ext.as_deref() {
            Some("pdf") => {
                // `entry.size()` is the declared uncompressed size; the `.take()` cap
                // below enforces it even if the header lies.
                if entry.size() > MAX_PDF_BYTES {
                    errors.push(format!("PDF troppo grande, saltato: {}", rel.display()));
                    continue;
                }
                let out = extract_dir.join(&rel);
                if let Some(parent) = out.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::File::create(&out) {
                    Ok(mut f) => {
                        match std::io::copy(&mut (&mut entry).take(MAX_PDF_BYTES), &mut f) {
                            Ok(written) => {
                                total_bytes = total_bytes.saturating_add(written);
                                pdfs.push((out, rel_dir));
                            }
                            Err(e) => errors.push(format!("Estrazione {}: {e}", rel.display())),
                        }
                    }
                    Err(e) => errors.push(format!("Scrittura {}: {e}", rel.display())),
                }
            }
            Some("bib") => {
                if entry.size() > MAX_BIB_BYTES {
                    errors.push(format!(".bib troppo grande, saltato: {}", rel.display()));
                    continue;
                }
                let mut buf = Vec::new();
                if (&mut entry).take(MAX_BIB_BYTES).read_to_end(&mut buf).is_ok() {
                    total_bytes = total_bytes.saturating_add(buf.len() as u64);
                    bibs.push((rel_dir, String::from_utf8_lossy(&buf).into_owned()));
                }
            }
            _ => {}
        }
    }

    // --- 2. Parse every .bib file.
    let parsed: Vec<(PathBuf, Vec<bibtex::BibEntry>)> =
        bibs.iter().map(|(dir, c)| (dir.clone(), bibtex::parse(c))).collect();
    let bib_entries: usize = parsed.iter().map(|(_, e)| e.len()).sum();

    // --- 3. Import each PDF as own work: content-addressed copy → prepare → commit.
    let thumb = thumb_dir(app);
    let _ = std::fs::create_dir_all(&thumb);
    let mut imported = 0usize;
    let mut duplicates = 0usize;
    let mut own_papers: Vec<(i64, PathBuf)> = Vec::new();
    for (pdf_path, pdf_dir) in &pdfs {
        let bytes = match std::fs::read(pdf_path) {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("Lettura PDF: {e}"));
                continue;
            }
        };
        let hash = import::sha256_hex(&bytes);
        let dest = papers_dir.join(format!("{hash}.pdf"));
        if !dest.exists() {
            if let Err(e) = std::fs::write(&dest, &bytes) {
                errors.push(format!("Copia PDF nella libreria: {e}"));
                continue;
            }
        }
        let prepared = {
            let _g = state.pdfium_lock.lock();
            match import::prepare_import(&state.pdfium, &thumb, &dest) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!("Analisi PDF: {e:#}"));
                    continue;
                }
            }
        };
        let outcome = {
            let conn = state.db.lock();
            match import::commit_import(&conn, &prepared) {
                Ok(o) => o,
                Err(e) => {
                    errors.push(format!("Salvataggio PDF: {e:#}"));
                    continue;
                }
            }
        };
        {
            let conn = state.db.lock();
            let _ = conn.execute("UPDATE documents SET is_own = 1 WHERE id = ?1", params![outcome.document_id]);
        }
        if outcome.imported {
            imported += 1;
            // Only NEW documents get their bibliography linked, so we never wipe or
            // overwrite the (possibly Crossref-enriched) references of a paper the
            // user already had. A fresh row has no references yet.
            own_papers.push((outcome.document_id, pdf_dir.clone()));
        } else {
            duplicates += 1;
        }
    }

    // --- 4. Link each new paper to its bibliography (the document_references graph).
    //
    // Collect the rows first (no DB, no network); then, if online discovery is on,
    // recover a DOI for the entries that lack one (title → Crossref) so they become
    // visible to the citation-gap finder; only then take the lock and insert. Network
    // I/O never happens while the DB mutex is held.
    struct PendingRef {
        doc_id: i64,
        raw: String,
        doi: Option<String>,
        title: Option<String>,
        resolved: bool, // DOI came from the online recovery (for an accurate count)
    }
    let mut pending: Vec<PendingRef> = Vec::new();
    if !own_papers.is_empty() {
        let single = own_papers.len() == 1;
        for (doc_id, pdf_dir) in &own_papers {
            // Dedupe within a paper: the same citation can be reachable via two
            // overlapping .bib dirs (root + subdir), or via `single` accepting all.
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for (bib_dir, entries) in &parsed {
                // A .bib applies to a PDF sitting in the same dir or a descendant;
                // for a lone paper, accept every .bib in the archive.
                if !(single || pdf_dir.starts_with(bib_dir)) {
                    continue;
                }
                for e in entries {
                    let Some(rref) = bib_to_ref(e) else { continue };
                    let key = match &rref.doi {
                        Some(d) => format!("doi:{d}"),
                        None => format!("t:{}", rref.meta.title.as_deref().unwrap_or_default().to_lowercase()),
                    };
                    if !seen.insert(key) {
                        continue;
                    }
                    pending.push(PendingRef {
                        doc_id: *doc_id,
                        raw: latex_ref_raw(&rref),
                        doi: rref.doi.clone(),
                        title: rref.meta.title.clone(),
                        resolved: false,
                    });
                }
            }
        }
    }

    // Online DOI recovery for the entries still missing one (de-TeX'd title →
    // Crossref DOI, precision-gated inside crossref_search_title). Gated on the
    // discovery opt-in. A per-paper budget keeps it fair across papers, plus a hard
    // total backstop so a pathological archive can't stall the import.
    let mut dois_resolved = 0usize; // counted at insert time below (after dedup)
    let need_resolve = pending.iter().filter(|p| p.doi.is_none() && p.title.is_some()).count();
    if need_resolve > 0 {
        let (online, email) = {
            let conn = state.db.lock();
            let online = setting(&conn, "discovery_enabled").as_deref() == Some("1");
            let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
            (online, email)
        };
        if online {
            if let Ok(client) = metadata::http_client(email.as_deref()) {
                const PER_DOC_MAX: usize = 150;
                const TOTAL_MAX: usize = 600;
                tauri::async_runtime::block_on(async {
                    let mut per_doc: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
                    let mut total = 0usize;
                    for p in pending.iter_mut() {
                        if p.doi.is_some() {
                            continue;
                        }
                        let Some(title) = p.title.clone() else { continue };
                        if total >= TOTAL_MAX {
                            break;
                        }
                        let n = per_doc.entry(p.doc_id).or_insert(0);
                        if *n >= PER_DOC_MAX {
                            continue;
                        }
                        *n += 1;
                        total += 1;
                        let r = tokio::time::timeout(
                            std::time::Duration::from_secs(20),
                            metadata::crossref_search_title(&client, &detex_title(&title), email.as_deref()),
                        )
                        .await;
                        if let Ok(Ok(Some((doi, _)))) = r {
                            p.doi = Some(doi.to_lowercase());
                            p.resolved = true;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                });
            }
        }
    }

    // --- Insert the (possibly DOI-enriched) references. Dedupe a resolved DOI that
    // now collides with another entry of the same paper.
    let mut references_linked = 0usize;
    let mut refs_without_doi = 0usize;
    if !pending.is_empty() {
        let conn = state.db.lock();
        let mut inserted_doi: std::collections::HashSet<(i64, String)> = std::collections::HashSet::new();
        for p in &pending {
            if let Some(d) = &p.doi {
                if !inserted_doi.insert((p.doc_id, d.clone())) {
                    continue;
                }
            }
            if conn
                .execute(
                    "INSERT INTO document_references (document_id, ref_doi, raw) VALUES (?1, ?2, ?3)",
                    params![p.doc_id, p.doi.as_deref(), p.raw],
                )
                .is_ok()
            {
                references_linked += 1;
                if p.doi.is_none() {
                    refs_without_doi += 1;
                } else if p.resolved {
                    dois_resolved += 1;
                }
            }
        }
    }

    if imported > 0 || duplicates > 0 {
        let _ = app.emit("library-changed", ());
    }
    Ok(LatexImportSummary {
        imported,
        duplicates,
        pdfs_found: pdfs.len(),
        bib_entries,
        references_linked,
        refs_without_doi,
        dois_resolved,
        errors,
    })
}

/// Best-effort human citation string for a bibliography entry (stored in `raw`).
fn latex_ref_raw(rref: &metadata::ResolvedRef) -> String {
    let m = &rref.meta;
    let mut parts: Vec<String> = Vec::new();
    if !m.authors.is_empty() {
        let names: Vec<String> = m
            .authors
            .iter()
            .take(6)
            .map(|a| match (&a.given, &a.family) {
                (Some(g), Some(f)) => format!("{g} {f}"),
                (None, Some(f)) => f.clone(),
                (Some(g), None) => g.clone(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty())
            .collect();
        if !names.is_empty() {
            parts.push(names.join(", "));
        }
    }
    if let Some(t) = &m.title {
        parts.push(t.clone());
    }
    if let Some(v) = &m.venue {
        parts.push(v.clone());
    }
    if let Some(y) = m.year {
        parts.push(y.to_string());
    }
    let s = parts.join(". ");
    if s.is_empty() {
        rref.doi.clone().unwrap_or_default()
    } else {
        s
    }
}

/// Collect a candidate OA-PDF URL, skipping blanks and duplicates (the sources
/// often agree on the same link).
fn push_pdf_url(urls: &mut Vec<String>, u: Option<String>) {
    if let Some(u) = u {
        let u = u.trim().to_string();
        if !u.is_empty() && !urls.iter().any(|x| x.eq_ignore_ascii_case(&u)) {
            urls.push(u);
        }
    }
}

/// Try to attach an Open-Access PDF to a reference-only document — robustly:
/// candidate links are gathered from a CHAIN of sources (arXiv id in the path,
/// arXiv DOI, Unpaywall when a contact email is set, OpenAlex, Semantic
/// Scholar, and — with no DOI at all — an OpenAlex title search gated by the
/// strict title match), then downloaded in order until one actually yields a
/// PDF, so a dead link just moves on to the next source.
/// Returns "attached" | "already" | "duplicate" | "not_found".
#[tauri::command]
pub async fn find_pdf(app: AppHandle, id: i64) -> Result<String, String> {
    let (doi, path, title, email) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let row = conn
            .query_row(
                "SELECT doi, path, title FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let (doi, path, title) = row.ok_or_else(|| "documento non trovato".to_string())?;
        let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
        (doi, path, title, email)
    };
    // Already has a real file?
    if path.as_deref().map(|p| !p.starts_with("ref:")).unwrap_or(false) {
        return Ok("already".into());
    }
    let oa_key = secret::get("openalex_key").unwrap_or_default();
    let s2_key = secret::get("s2_key").unwrap_or_default();

    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let timeout = std::time::Duration::from_secs(20);
    let mut urls: Vec<String> = Vec::new();

    // (1) An arXiv reference: the id IS the paper, its PDF link is deterministic.
    if let Some(rest) = path.as_deref().and_then(|p| p.strip_prefix("ref:arxiv:")) {
        push_pdf_url(&mut urls, Some(format!("https://arxiv.org/pdf/{rest}")));
    }
    // (2) An arXiv DOI names the eprint directly.
    if let Some(aid) = doi
        .as_deref()
        .map(|d| d.trim().to_lowercase())
        .and_then(|d| d.strip_prefix("10.48550/arxiv.").map(str::to_string))
        .filter(|a| !a.is_empty())
    {
        push_pdf_url(&mut urls, Some(format!("https://arxiv.org/pdf/{aid}")));
    }
    // (3-5) DOI lookups: Unpaywall (only with the contact email it mandates —
    // no longer a hard requirement), then OpenAlex and Semantic Scholar.
    if let Some(d) = doi.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        if let Some(mail) = email.as_deref() {
            if let Ok(Ok(u)) = tokio::time::timeout(timeout, metadata::unpaywall_pdf(&client, d, mail)).await {
                push_pdf_url(&mut urls, u);
            }
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        }
        if let Ok(u) = tokio::time::timeout(timeout, discovery::openalex_oa_pdf(&client, d, &oa_key)).await {
            push_pdf_url(&mut urls, u);
        }
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        if let Ok(u) = tokio::time::timeout(timeout, discovery::s2_oa_pdf(&client, d, &s2_key)).await {
            push_pdf_url(&mut urls, u);
        }
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }
    // (6) No leads yet: TITLE searches on OpenAlex, arXiv and Semantic Scholar,
    // each accepted ONLY on the strict title match — a near-neighbour must
    // never provide the file. (Looser matches belong to the interactive
    // `pdf_candidates` dialog, where the USER confirms.)
    if urls.is_empty() {
        if let Some(t) = title.as_deref().map(str::trim).filter(|t| t.chars().count() >= 8) {
            let filters = discovery::Filters {
                year_from: None,
                year_to: None,
                oa_only: false,
                sort: "relevance".to_string(),
                author: None,
            };
            if let Ok(Ok(list)) =
                tokio::time::timeout(timeout, discovery::search_openalex(&client, t, &filters, &oa_key)).await
            {
                for r in list.into_iter().take(8) {
                    if r.title.as_deref().is_some_and(|c| metadata::strong_title_match(t, c)) {
                        push_pdf_url(&mut urls, r.oa_pdf_url);
                        // The matched work's DOI unlocks the DOI sources too.
                        if let (Some(d2), Some(mail)) = (r.doi.as_deref(), email.as_deref()) {
                            if let Ok(Ok(u)) =
                                tokio::time::timeout(timeout, metadata::unpaywall_pdf(&client, d2, mail)).await
                            {
                                push_pdf_url(&mut urls, u);
                            }
                        }
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            // arXiv by title — the common case for CS/ML references without a DOI.
            if let Ok(Ok(list)) = tokio::time::timeout(timeout, metadata::arxiv_search_candidates(&client, t, 5)).await
            {
                for (aid, meta) in list {
                    if !aid.is_empty()
                        && meta.title.as_deref().is_some_and(|c| metadata::strong_title_match(t, c))
                    {
                        push_pdf_url(&mut urls, Some(format!("https://arxiv.org/pdf/{aid}")));
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            if let Ok(Ok(list)) =
                tokio::time::timeout(timeout, discovery::search_semantic_scholar(&client, t, &filters, &s2_key)).await
            {
                for r in list.into_iter().take(8) {
                    if r.title.as_deref().is_some_and(|c| metadata::strong_title_match(t, c)) {
                        push_pdf_url(&mut urls, r.oa_pdf_url);
                        break;
                    }
                }
            }
        }
    }
    if urls.is_empty() {
        return Ok("not_found".into());
    }
    download_and_attach(&app, id, &urls).await
}

/// Download the first WORKING candidate URL — a dead OA link (404 host, HTML
/// landing page) just moves on to the next — and attach it to document `id`.
/// Shared by the automatic `find_pdf` and the user-picked `attach_pdf_candidate`.
/// Returns "attached" | "duplicate" | "not_found".
async fn download_and_attach(app: &AppHandle, id: i64, urls: &[String]) -> Result<String, String> {
    let mut saved_path: Option<PathBuf> = None;
    for url in urls {
        match download_pdf(app, url).await {
            Ok(Some(p)) => {
                saved_path = Some(p);
                break;
            }
            Ok(None) | Err(_) => continue,
        }
    }
    let Some(saved) = saved_path else {
        return Ok("not_found".into());
    };

    // Extract text + thumbnail (no DB lock), serialized against other pdfium
    // document work, then attach to the existing row.
    let state = app.state::<AppState>();
    let dir = thumb_dir(app);
    let prepared = {
        let _pdf_guard = state.pdfium_lock.lock();
        import::prepare_import(&state.pdfium, &dir, &saved)
    }
    .map_err(|e| e.to_string())?;
    let conn = state.db.lock();
    // If these exact bytes are already in the library (file_hash isn't UNIQUE),
    // don't create a duplicate; drop the just-downloaded file instead.
    let dup: Option<i64> = conn
        .query_row(
            "SELECT id FROM documents WHERE file_hash = ?1 AND id != ?2 AND deleted_at IS NULL",
            params![prepared.hash, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if dup.is_some() {
        // Content-addressed storage means `saved` may be the very file the
        // existing duplicate already references — so never delete it here.
        return Ok("duplicate".into());
    }
    conn.execute(
        "UPDATE documents SET path = ?1, file_hash = ?2, thumb_path = ?3, fulltext = ?4,
         github_url = COALESCE(?5, github_url) WHERE id = ?6",
        params![prepared.path, prepared.hash, prepared.thumb_path, prepared.fulltext, prepared.github_url, id],
    )
    .map_err(|e| e.to_string())?;
    Ok("attached".into())
}

/// One candidate PDF source for a reference-only document, offered to the user.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdfCandidate {
    /// "arxiv" | "openalex" | "semanticscholar" | "crossref" | "doi"
    pub source: String,
    /// Where it came from, for display (Italian).
    pub origin: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    /// Direct PDF link, when the source exposes one.
    pub pdf_url: Option<String>,
    /// DOI to resolve at attach time (Unpaywall/OpenAlex/S2) when no direct link.
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    /// Landing/abstract page, for the "open and check" button.
    pub landing_url: Option<String>,
    pub score: i64,
    /// Passed the same strict title gate the automatic path uses (or comes
    /// from an identifier of the record itself, whose identity is certain).
    pub sure: bool,
    pub signals: Vec<String>,
}

/// Report of the per-document PDF-candidate search.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PdfProbe {
    pub title: Option<String>,
    pub candidates: Vec<PdfCandidate>,
}

/// Last surname token of each display name — whole tokens, folded.
fn surname_tokens(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter_map(|a| parse_author(a).1)
        .filter_map(|f| {
            metadata::fold_ascii(&f.to_lowercase())
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() >= 3)
                .last()
                .map(str::to_string)
        })
        .collect()
}

/// EXTENSIVE per-document PDF search for a reference-only entry: gather
/// CANDIDATE sources by identifier (arXiv id in the path, DOI → Unpaywall/
/// OpenAlex/Semantic Scholar) and by TITLE on arXiv, OpenAlex, Semantic
/// Scholar and Crossref, scored against the stored title/authors/year.
/// Nothing is downloaded here — the user picks one and `attach_pdf_candidate`
/// does the download+attach. This is the human fallback for when the
/// strictly-gated automatic `find_pdf` comes up empty.
#[tauri::command]
pub async fn pdf_candidates(app: AppHandle, id: i64) -> Result<PdfProbe, String> {
    let (doi, path, title, year, authors, email): (
        Option<String>,
        String,
        Option<String>,
        Option<i64>,
        Vec<String>,
        Option<String>,
    ) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let (doi, path, title, year) = conn
            .query_row(
                "SELECT doi, path, title, year FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "documento non trovato".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT a.given, a.family FROM authors a
                 JOIN document_authors da ON da.author_id = a.id
                 WHERE da.document_id = ?1 ORDER BY da.position",
            )
            .map_err(|e| e.to_string())?;
        let authors: Vec<String> = stmt
            .query_map(params![id], |r| {
                let g: Option<String> = r.get(0)?;
                let f: Option<String> = r.get(1)?;
                Ok(format!("{} {}", g.unwrap_or_default(), f.unwrap_or_default()).trim().to_string())
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|s| !s.is_empty())
            .collect();
        let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
        (doi, path, title, year, authors, email)
    };
    let oa_key = secret::get("openalex_key").unwrap_or_default();
    let s2_key = secret::get("s2_key").unwrap_or_default();
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let timeout = std::time::Duration::from_secs(20);
    let nap = || tokio::time::sleep(std::time::Duration::from_millis(150));

    let mut raw: Vec<PdfCandidate> = Vec::new();

    // Identifier-based candidates: the IDENTITY is certain, only the link can fail.
    if let Some(rest) = path.strip_prefix("ref:arxiv:").filter(|r| !r.is_empty()) {
        raw.push(PdfCandidate {
            source: "arxiv".into(),
            origin: "id arXiv della scheda".into(),
            title: title.clone(),
            authors: authors.clone(),
            year,
            venue: Some("arXiv".into()),
            pdf_url: Some(format!("https://arxiv.org/pdf/{rest}")),
            doi: None,
            arxiv_id: Some(rest.to_string()),
            landing_url: Some(format!("https://arxiv.org/abs/{rest}")),
            score: 120,
            sure: true,
            signals: vec!["identificativo della scheda".into(), "link PDF diretto".into()],
        });
    }
    if let Some(d) = doi.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        let dl = d.to_lowercase();
        if let Some(aid) = dl.strip_prefix("10.48550/arxiv.").filter(|a| !a.is_empty()) {
            raw.push(PdfCandidate {
                source: "arxiv".into(),
                origin: "DOI arXiv della scheda".into(),
                title: title.clone(),
                authors: authors.clone(),
                year,
                venue: Some("arXiv".into()),
                pdf_url: Some(format!("https://arxiv.org/pdf/{aid}")),
                doi: Some(d.to_string()),
                arxiv_id: Some(aid.to_string()),
                landing_url: Some(format!("https://arxiv.org/abs/{aid}")),
                score: 120,
                sure: true,
                signals: vec!["identificativo della scheda".into(), "link PDF diretto".into()],
            });
        }
        let doi_link = |u: Option<String>, origin: &str, source: &str, raw: &mut Vec<PdfCandidate>| {
            if let Some(u) = u.filter(|u| !u.trim().is_empty()) {
                raw.push(PdfCandidate {
                    source: source.into(),
                    origin: origin.into(),
                    title: title.clone(),
                    authors: authors.clone(),
                    year,
                    venue: None,
                    pdf_url: Some(u),
                    doi: Some(d.to_string()),
                    arxiv_id: None,
                    landing_url: Some(format!("https://doi.org/{d}")),
                    score: 115,
                    sure: true,
                    signals: vec!["via il DOI della scheda".into(), "link PDF diretto".into()],
                });
            }
        };
        if let Some(mail) = email.as_deref() {
            if let Ok(Ok(u)) = tokio::time::timeout(timeout, metadata::unpaywall_pdf(&client, d, mail)).await {
                doi_link(u, "Unpaywall (dal DOI)", "doi", &mut raw);
            }
            nap().await;
        }
        if let Ok(u) = tokio::time::timeout(timeout, discovery::openalex_oa_pdf(&client, d, &oa_key)).await {
            doi_link(u, "OpenAlex (dal DOI)", "openalex", &mut raw);
        }
        nap().await;
        if let Ok(u) = tokio::time::timeout(timeout, discovery::s2_oa_pdf(&client, d, &s2_key)).await {
            doi_link(u, "Semantic Scholar (dal DOI)", "semanticscholar", &mut raw);
        }
        nap().await;
    }

    // Title searches → candidates the USER judges (this is where the automatic
    // strict gate is deliberately relaxed into a ranked list).
    if let Some(t) = title.as_deref().map(str::trim).filter(|t| t.chars().count() >= 8) {
        let filters = discovery::Filters {
            year_from: None,
            year_to: None,
            oa_only: false,
            sort: "relevance".to_string(),
            author: None,
        };
        if let Ok(Ok(list)) = tokio::time::timeout(timeout, discovery::search_arxiv(&client, t, &filters)).await {
            for r in list.into_iter().take(6) {
                let aid = r.external_id.clone();
                raw.push(PdfCandidate {
                    source: "arxiv".into(),
                    origin: "ricerca per titolo (arXiv)".into(),
                    title: r.title,
                    authors: r.authors,
                    year: r.year,
                    venue: r.venue,
                    pdf_url: r
                        .oa_pdf_url
                        .or_else(|| (!aid.is_empty()).then(|| format!("https://arxiv.org/pdf/{aid}"))),
                    doi: r.doi,
                    arxiv_id: (!aid.is_empty()).then_some(aid),
                    landing_url: r.url,
                    score: 0,
                    sure: false,
                    signals: Vec::new(),
                });
            }
        }
        nap().await;
        if let Ok(Ok(list)) =
            tokio::time::timeout(timeout, discovery::search_openalex(&client, t, &filters, &oa_key)).await
        {
            for r in list.into_iter().take(6) {
                raw.push(PdfCandidate {
                    source: "openalex".into(),
                    origin: "ricerca per titolo (OpenAlex)".into(),
                    title: r.title,
                    authors: r.authors,
                    year: r.year,
                    venue: r.venue,
                    pdf_url: r.oa_pdf_url,
                    doi: r.doi,
                    arxiv_id: None,
                    landing_url: r.url,
                    score: 0,
                    sure: false,
                    signals: Vec::new(),
                });
            }
        }
        nap().await;
        if let Ok(Ok(list)) =
            tokio::time::timeout(timeout, discovery::search_semantic_scholar(&client, t, &filters, &s2_key)).await
        {
            for r in list.into_iter().take(6) {
                raw.push(PdfCandidate {
                    source: "semanticscholar".into(),
                    origin: "ricerca per titolo (Semantic Scholar)".into(),
                    title: r.title,
                    authors: r.authors,
                    year: r.year,
                    venue: r.venue,
                    pdf_url: r.oa_pdf_url,
                    doi: r.doi,
                    arxiv_id: None,
                    landing_url: r.url,
                    score: 0,
                    sure: false,
                    signals: Vec::new(),
                });
            }
        }
        nap().await;
        if let Ok(Ok(list)) =
            tokio::time::timeout(timeout, metadata::crossref_search_candidates(&client, t, email.as_deref(), 5)).await
        {
            for (cdoi, meta) in list {
                raw.push(PdfCandidate {
                    source: "crossref".into(),
                    origin: "ricerca per titolo (Crossref)".into(),
                    title: meta.title.clone(),
                    authors: display_authors(&meta),
                    year: meta.year,
                    venue: meta.venue.clone(),
                    pdf_url: None,
                    doi: Some(cdoi.clone()),
                    arxiv_id: None,
                    landing_url: Some(format!("https://doi.org/{cdoi}")),
                    score: 0,
                    sure: false,
                    signals: Vec::new(),
                });
            }
        }
    }

    // Score the title-search candidates against the STORED record.
    let stored_t = title.as_deref().unwrap_or("");
    let stored_fams = surname_tokens(&authors);
    for c in raw.iter_mut() {
        if c.sure {
            continue; // identifier-based: already top-ranked
        }
        let ct = c.title.as_deref().unwrap_or("");
        if !stored_t.is_empty() && !ct.is_empty() {
            if metadata::strong_title_match(stored_t, ct) {
                c.sure = true;
                c.score += 100;
                c.signals.push("titolo identico alla scheda".into());
            } else {
                let f = metadata::title_overlap_frac(stored_t, ct);
                if f >= 0.5 {
                    c.score += (40.0 * f) as i64;
                    c.signals.push(format!("titolo simile ({}%)", (f * 100.0) as i32));
                }
            }
        }
        if !stored_fams.is_empty() && !c.authors.is_empty() {
            let cand: std::collections::HashSet<String> = surname_tokens(&c.authors).into_iter().collect();
            let hits = stored_fams.iter().filter(|f| cand.contains(*f)).count();
            if hits > 0 {
                c.score += (30 * hits / stored_fams.len()) as i64;
                c.signals.push(format!("{hits}/{} autori coincidono", stored_fams.len()));
            }
        }
        if let (Some(sy), Some(cy)) = (year, c.year) {
            if sy == cy {
                c.score += 10;
                c.signals.push("stesso anno".into());
            } else if (sy - cy).abs() <= 1 {
                c.score += 4;
            }
        }
        if c.pdf_url.is_some() {
            c.score += 15;
            c.signals.push("link PDF diretto".into());
        }
    }

    // Best first; dedup by identity (arXiv id — an arXiv DOI counts as one —
    // then DOI, then the link itself, then the title).
    raw.sort_by(|a, b| (b.sure, b.score).cmp(&(a.sure, a.score)));
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    raw.retain(|c| {
        let key = if let Some(a) = c.arxiv_id.as_deref() {
            format!("a:{}", a.split('v').next().unwrap_or(a).to_lowercase())
        } else if let Some(d) = c.doi.as_deref() {
            let dl = d.trim().to_lowercase();
            match dl.strip_prefix("10.48550/arxiv.") {
                Some(aid) => format!("a:{}", aid.split('v').next().unwrap_or(aid)),
                None => format!("d:{dl}"),
            }
        } else if let Some(u) = c.pdf_url.as_deref() {
            format!("u:{}", u.to_lowercase())
        } else {
            format!("t:{}", c.title.as_deref().unwrap_or("").to_lowercase())
        };
        seen.insert(key)
    });
    raw.truncate(10);
    Ok(PdfProbe { title, candidates: raw })
}

/// Download and attach a USER-CHOSEN PDF candidate: its direct link first,
/// then the links its identifiers unlock (arXiv id → eprint; DOI → Unpaywall/
/// OpenAlex/Semantic Scholar). Returns "attached" | "already" | "duplicate" |
/// "not_found" — on "not_found" the dialog stays open so the user can try the
/// next candidate.
#[tauri::command]
pub async fn attach_pdf_candidate(app: AppHandle, id: i64, candidate: PdfCandidate) -> Result<String, String> {
    let (path, email): (String, Option<String>) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let path = conn
            .query_row(
                "SELECT path FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "documento non trovato".to_string())?;
        let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
        (path, email)
    };
    if !path.starts_with("ref:") {
        return Ok("already".into());
    }
    let oa_key = secret::get("openalex_key").unwrap_or_default();
    let s2_key = secret::get("s2_key").unwrap_or_default();
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let timeout = std::time::Duration::from_secs(20);

    let mut urls: Vec<String> = Vec::new();
    push_pdf_url(&mut urls, candidate.pdf_url.clone());
    if let Some(aid) = candidate.arxiv_id.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        push_pdf_url(&mut urls, Some(format!("https://arxiv.org/pdf/{aid}")));
    }
    if let Some(d) = candidate.doi.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        if let Some(mail) = email.as_deref() {
            if let Ok(Ok(u)) = tokio::time::timeout(timeout, metadata::unpaywall_pdf(&client, d, mail)).await {
                push_pdf_url(&mut urls, u);
            }
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        }
        if let Ok(u) = tokio::time::timeout(timeout, discovery::openalex_oa_pdf(&client, d, &oa_key)).await {
            push_pdf_url(&mut urls, u);
        }
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        if let Ok(u) = tokio::time::timeout(timeout, discovery::s2_oa_pdf(&client, d, &s2_key)).await {
            push_pdf_url(&mut urls, u);
        }
    }
    if urls.is_empty() {
        return Ok("not_found".into());
    }
    download_and_attach(&app, id, &urls).await
}

/// Attach a PDF downloaded from `url` to an EXISTING reference-only document —
/// unlike [`add_from_url`], which creates a new entry. Same SSRF-guarded
/// download and the same attach path as [`find_pdf`] (hash-dedupe against the
/// rest of the library, GitHub blob links normalized to the raw file). Returns
/// `"attached"` | `"already"` (the doc has a file) | `"duplicate"` | `"not_pdf"`.
#[tauri::command]
pub async fn attach_from_url(app: AppHandle, id: i64, url: String) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL vuoto".into());
    }
    let url = normalize_pdf_url(url);
    let path: Option<String> = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.query_row(
            "SELECT path FROM documents WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "documento non trovato".to_string())?
    };
    if path.as_deref().map(|p| !p.starts_with("ref:")).unwrap_or(false) {
        return Ok("already".into());
    }
    let Some(saved) = download_pdf(&app, &url).await? else {
        return Ok("not_pdf".into());
    };
    let state = app.state::<AppState>();
    let dir = thumb_dir(&app);
    let prepared = {
        let _pdf_guard = state.pdfium_lock.lock();
        import::prepare_import(&state.pdfium, &dir, &saved)
    }
    .map_err(|e| e.to_string())?;
    let conn = state.db.lock();
    // Same bytes already in the library under another document? Don't duplicate
    // (content-addressed storage: never delete `saved`, the twin may reference it).
    let dup: Option<i64> = conn
        .query_row(
            "SELECT id FROM documents WHERE file_hash = ?1 AND id != ?2 AND deleted_at IS NULL",
            params![prepared.hash, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if dup.is_some() {
        return Ok("duplicate".into());
    }
    conn.execute(
        "UPDATE documents SET path = ?1, file_hash = ?2, thumb_path = ?3, fulltext = ?4,
         github_url = COALESCE(?5, github_url), page_count = COALESCE(?6, page_count) WHERE id = ?7",
        params![
            prepared.path,
            prepared.hash,
            prepared.thumb_path,
            prepared.fulltext,
            prepared.github_url,
            (prepared.page_count > 0).then_some(prepared.page_count),
            id
        ],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);
    let _ = app.emit("library-changed", ());
    Ok("attached".into())
}

// ===== Hugging Face: code & models linked to a paper =====

#[derive(serde::Serialize)]
pub struct HfItem {
    id: String,
    likes: i64,
    downloads: i64,
    url: String,
}

#[derive(serde::Serialize)]
pub struct HfResources {
    arxiv_id: Option<String>,
    paper_url: Option<String>,
    models: Vec<HfItem>,
    datasets: Vec<HfItem>,
}

/// Best original link for a document: its DOI (doi.org), else the arXiv abstract page.
fn paper_link_for(doi: Option<&str>, path: Option<&str>) -> Option<String> {
    if let Some(d) = doi {
        if !d.trim().is_empty() {
            return Some(format!("https://doi.org/{}", d.trim()));
        }
    }
    arxiv_id_from(None, path).map(|aid| format!("https://arxiv.org/abs/{aid}"))
}

/// Derive an arXiv id from a document's path (`ref:arxiv:<id>`) or DOI (10.48550/arXiv.<id>).
fn arxiv_id_from(doi: Option<&str>, path: Option<&str>) -> Option<String> {
    if let Some(rest) = path.and_then(|p| p.strip_prefix("ref:arxiv:")) {
        if !rest.trim().is_empty() {
            return Some(rest.trim().to_string());
        }
    }
    if let Some(d) = doi {
        let dl = d.to_lowercase();
        for marker in ["arxiv.", "arxiv:", "arxiv/"] {
            if let Some(idx) = dl.find(marker) {
                let id = d[idx + marker.len()..].trim();
                if !id.is_empty() {
                    return Some(id.to_string());
                }
            }
        }
    }
    // Downloaded arXiv PDFs are saved as `…/papers/arxiv_<id>.pdf`: pull the
    // modern arXiv id (YYMM.NNNNN) out of the path when it mentions arxiv.
    if let Some(p) = path {
        if p.to_lowercase().contains("arxiv") {
            if let Some(id) = arxiv_new_id(p) {
                return Some(id);
            }
        }
    }
    None
}

/// Find a modern arXiv id (4 digits, dot, 4-5 digits) in a string, e.g. `2301.12345`.
fn arxiv_new_id(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i + 5 < b.len() {
        let four = b[i..i + 4].iter().all(u8::is_ascii_digit);
        if four && b[i + 4] == b'.' {
            let mut j = i + 5;
            while j < b.len() && j < i + 10 && b[j].is_ascii_digit() {
                j += 1;
            }
            if j - (i + 5) >= 4 {
                return Some(s[i..j].to_string());
            }
        }
        i += 1;
    }
    None
}

/// List Hugging Face models or datasets that reference an arXiv id (best-effort).
async fn hf_list(client: &reqwest::Client, kind: &str, aid: &str) -> Vec<HfItem> {
    let url = format!(
        "https://huggingface.co/api/{kind}?filter=arxiv:{aid}&sort=likes&direction=-1&limit=25"
    );
    let Ok(resp) = client.get(&url).send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return Vec::new();
    };
    body.as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let prefix = if kind == "datasets" { "datasets/" } else { "" };
            Some(HfItem {
                url: format!("https://huggingface.co/{prefix}{id}"),
                id,
                likes: m["likes"].as_i64().unwrap_or(0),
                downloads: m["downloads"].as_i64().unwrap_or(0),
            })
        })
        .collect()
}

/// Models & datasets on the Hugging Face Hub that cite this document's paper.
#[tauri::command]
pub async fn hf_resources(app: AppHandle, id: i64) -> Result<HfResources, String> {
    let (enabled, doi, path) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let row = conn
            .query_row(
                "SELECT doi, path FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let (doi, path) = row.ok_or_else(|| "documento non trovato".to_string())?;
        (setting(&conn, "discovery_enabled").as_deref() == Some("1"), doi, path)
    };
    if !enabled {
        return Err("La ricerca online è disattivata (abilitala nelle impostazioni)".into());
    }
    let Some(aid) = arxiv_id_from(doi.as_deref(), path.as_deref()) else {
        return Ok(HfResources { arxiv_id: None, paper_url: None, models: Vec::new(), datasets: Vec::new() });
    };
    let client = metadata::http_client(None).map_err(|e| e.to_string())?;
    let models = hf_list(&client, "models", &aid).await;
    let datasets = hf_list(&client, "datasets", &aid).await;
    Ok(HfResources {
        paper_url: Some(format!("https://huggingface.co/papers/{aid}")),
        arxiv_id: Some(aid),
        models,
        datasets,
    })
}

// ===== GitHub: a paper's code repository =====

/// GitHub repos referenced in a document's text, with live metadata.
#[tauri::command]
pub async fn github_repos(app: AppHandle, id: i64) -> Result<Vec<github::GhRepo>, String> {
    let (enabled, text, token) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let row = conn
            .query_row(
                "SELECT COALESCE(fulltext,'') || ' ' || COALESCE(abstract,'') || ' ' || COALESCE(notes,'')
                 FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let text = row.ok_or_else(|| "documento non trovato".to_string())?;
        (
            setting(&conn, "discovery_enabled").as_deref() == Some("1"),
            text,
            secret::get("github_token").unwrap_or_default(),
        )
    };
    if !enabled {
        return Err("La ricerca online è disattivata (abilitala nelle impostazioni)".into());
    }
    let candidates = github::find_repos_in_text(&text, 6);
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let client = metadata::http_client(None).map_err(|e| e.to_string())?;
    let mut repos = Vec::new();
    for (owner, repo) in candidates {
        if let Some(r) = github::fetch_repo(&client, &token, &owner, &repo).await {
            repos.push(r);
        }
    }
    Ok(repos)
}

/// True if `s` is a safe GitHub owner/repo path segment.
fn is_gh_segment(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A repo's README rendered to sanitized HTML.
#[tauri::command]
pub async fn github_readme(app: AppHandle, owner: String, repo: String) -> Result<String, String> {
    let token = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        if setting(&conn, "discovery_enabled").as_deref() != Some("1") {
            return Err("La ricerca online è disattivata (abilitala nelle impostazioni)".into());
        }
        secret::get("github_token").unwrap_or_default()
    };
    if !is_gh_segment(&owner) || !is_gh_segment(&repo) {
        return Err("Nome repository non valido".into());
    }
    let client = metadata::http_client(None).map_err(|e| e.to_string())?;
    github::fetch_readme_html(&client, &token, &owner, &repo)
        .await
        .map_err(|e| e.to_string())
}

// ===== Table extraction from a selected PDF region =====

/// Reconstruct a tabular grid from a normalized region `[x,y,w,h]` (top-left,
/// rotation-0 frame) of page `page` (1-based) of a document.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn extract_table(
    state: State<'_, AppState>,
    id: i64,
    page: u16,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Result<Vec<Vec<String>>, String> {
    let path = {
        let conn = state.db.lock();
        resolve_existing_path(&conn, id)?
    };
    let path = path.ok_or_else(|| "Questo elemento non ha un file PDF".to_string())?;
    let words = pdf::extract_region_words(
        &state.pdfium,
        Path::new(&path),
        page.saturating_sub(1),
        [x, y, w, h],
    )
    .map_err(|e| e.to_string())?;
    Ok(table::reconstruct(&words))
}

/// Extract the text of a normalized region `[x,y,w,h]` (rotation-0 frame) of page
/// `page` (1-based), as readable lines in a small Markdown subset that carries the
/// paper's formatting: `*italic*`, `**bold**`, `<sup>`/`<sub>` for citation
/// markers and chemical formulas. The frontend derives plain text / LaTeX from it.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn extract_region_text(
    state: State<'_, AppState>,
    id: i64,
    page: u16,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Result<String, String> {
    let path = {
        let conn = state.db.lock();
        resolve_existing_path(&conn, id)?
    };
    let path = path.ok_or_else(|| "Questo elemento non ha un file PDF".to_string())?;
    let words = pdf::extract_region_words(
        &state.pdfium,
        Path::new(&path),
        page.saturating_sub(1),
        [x, y, w, h],
    )
    .map_err(|e| e.to_string())?;
    Ok(table::join_text_rich(&words))
}

/// Directory where the formula-OCR (pix2tex) models are cached. Downloaded once
/// on first use, like the fastembed model, so the installer stays lean.
fn mathocr_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("mathocr"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_mathocr"))
}

/// Directory where the table-structure model (TATR) is cached — same first-use
/// download pattern as the formula OCR.
fn tablestruct_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("tablestruct"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pdfmanage_tablestruct"))
}

/// Whether the table-structure model is present, and the MB a first extraction
/// would download otherwise.
#[tauri::command]
pub fn tablestruct_status(app: AppHandle) -> Result<serde_json::Value, String> {
    let dir = tablestruct_dir(&app);
    let missing = tablestruct::missing_bytes(&dir);
    Ok(serde_json::json!({
        "ready": missing == 0,
        "downloadMb": missing.div_ceil(1024 * 1024),
    }))
}

/// Extract a table from a region using the STRUCTURE MODEL (TATR): the model reads
/// the cropped image and predicts rows/columns/spanning cells; the cell text comes
/// from pdfium's word boxes intersected with that grid — byte-exact, no OCR. The
/// image must be the crop of exactly the normalized region `[x,y,w,h]`.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn extract_table_model(
    app: AppHandle,
    image_base64: String,
    id: i64,
    page: u16,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Result<Vec<Vec<String>>, String> {
    let dir = tablestruct_dir(&app);
    if !tablestruct::models_present(&dir) {
        tablestruct::ensure_models(&dir)
            .await
            .map_err(|e| format!("scarico modello tabelle: {e}"))?;
    }
    let b64 = strip_data_url(&image_base64);
    let bytes = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| format!("immagine non valida: {e}"))?;

    // Words of the SAME region, converted to region-relative coordinates.
    let path = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        resolve_existing_path(&conn, id)?
    };
    let path = path.ok_or_else(|| "Questo elemento non ha un file PDF".to_string())?;
    let words = {
        let state = app.state::<AppState>();
        pdf::extract_region_words(&state.pdfium, Path::new(&path), page.saturating_sub(1), [x, y, w, h])
            .map_err(|e| e.to_string())?
    };
    if words.is_empty() {
        return Err(
            "Nessun testo nel PDF in quest'area (pagina scansionata?): il modello struttura \
             ha bisogno del testo del PDF per riempire le celle — usa il motore «Ollama»."
                .to_string(),
        );
    }
    let rw = w.max(f32::MIN_POSITIVE);
    let rh = h.max(f32::MIN_POSITIVE);
    let wboxes: Vec<tablestruct::WordBox> = words
        .into_iter()
        .map(|wd| tablestruct::WordBox {
            text: wd.text,
            x0: (wd.x0 - x) / rw,
            y0: (wd.y0 - y) / rh,
            x1: (wd.x1 - x) / rw,
            y1: (wd.y1 - y) / rh,
        })
        .collect();

    let grid = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<Vec<String>>, String> {
        let dets = tablestruct::recognize(&dir, &bytes).map_err(|e| e.to_string())?;
        Ok(tablestruct::assemble_grid(&dets, &wboxes))
    })
    .await
    .map_err(|e| e.to_string())??;
    if grid.is_empty() {
        return Err(
            "Il modello non ha riconosciuto una struttura di tabella in quest'area: \
             restringi la selezione alla sola tabella, o prova il motore «Nativa»."
                .to_string(),
        );
    }
    Ok(grid)
}

/// Whether the formula→LaTeX models are already present, and if not, how many MB
/// the first recognition will download. Lets the UI warn before the wait.
#[tauri::command]
pub fn mathocr_status(app: AppHandle) -> Result<serde_json::Value, String> {
    let dir = mathocr_dir(&app);
    let ready = mathocr::models_present(&dir);
    let mb = (mathocr::missing_bytes(&dir) as f64 / (1024.0 * 1024.0)).round() as u64;
    Ok(serde_json::json!({ "ready": ready, "downloadMb": mb }))
}

/// Recognize a cropped formula image (base64 PNG from the reader canvas) as
/// LaTeX. Downloads the ~140 MB pix2tex models on first use, then runs fully
/// locally via the ONNX Runtime already linked into the app.
#[tauri::command]
pub async fn formula_to_latex(
    app: AppHandle,
    image_base64: String,
    multi: bool,
) -> Result<String, String> {
    let dir = mathocr_dir(&app);
    // Accept either a bare base64 string or a full `data:image/png;base64,…` URL.
    let b64 = image_base64.rsplit(',').next().unwrap_or(&image_base64).trim();
    let bytes = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| format!("immagine non valida: {e}"))?;
    if !mathocr::models_present(&dir) {
        let client = ai::client().map_err(|e| e.to_string())?;
        mathocr::ensure_models(&dir, &client)
            .await
            .map_err(|e| format!("scarico modelli formula: {e}"))?;
    }
    tauri::async_runtime::spawn_blocking(move || mathocr::recognize(&dir, &bytes, multi))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Take just the base64 payload of a possibly-data-URL image string.
fn strip_data_url(s: &str) -> &str {
    s.rsplit(',').next().unwrap_or(s).trim()
}

/// Clean a vision model's LaTeX: drop ```code fences``` and $…$ / $$…$$ / \[…\] wrappers.
fn clean_vision_latex(s: &str) -> String {
    let mut t = s.trim().to_string();
    if t.starts_with("```") {
        // Drop the opening fence: the ```lang line if multi-line, else the ``` token.
        if let Some(nl) = t.find('\n') {
            t = t[nl + 1..].to_string();
        } else {
            t = t.trim_start_matches('`').to_string();
        }
        if let Some(pos) = t.rfind("```") {
            t = t[..pos].to_string();
        }
        t = t.trim().to_string();
    }
    for (open, close) in [("$$", "$$"), ("\\[", "\\]"), ("$", "$")] {
        if t.len() >= open.len() + close.len() && t.starts_with(open) && t.ends_with(close) {
            let inner = &t[open.len()..t.len() - close.len()];
            // Only unwrap a single wrapping group: if the delimiter reappears inside,
            // this is multiple inline pieces (e.g. "$a$ + $b$") and stripping the outer
            // pair would corrupt it into "a$ + $b". Leave those untouched.
            if !inner.contains(open) && !inner.contains(close) {
                t = inner.trim().to_string();
                break;
            }
        }
    }
    t
}

/// Resolve the AI provider/url and pick the vision model (explicit override or the
/// configured model), erroring if AI is off or no model is available.
fn vision_target(app: &AppHandle, model: String) -> Result<(String, String, String), String> {
    let (enabled, provider, url, default_model) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone())
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    let model = if model.trim().is_empty() { default_model } else { model };
    if model.trim().is_empty() {
        return Err("Nessun modello di visione selezionato".into());
    }
    Ok((provider, url, model))
}

/// Recognize a cropped formula image as LaTeX via a local vision LLM (Ollama /
/// LM Studio) instead of the bundled math-OCR. Needs a vision-capable model.
#[tauri::command]
pub async fn formula_to_latex_ai(
    app: AppHandle,
    image_base64: String,
    model: String,
    multi: bool,
) -> Result<String, String> {
    let (provider, url, model) = vision_target(&app, model)?;
    let b64 = strip_data_url(&image_base64).to_string();
    // Ask explicitly for upright \mathrm on differentials/operators/units, which
    // vision models otherwise emit as italic letters (e.g. `dx` instead of
    // `\mathrm{d}x`), and reproduce faithfully whatever the source typesets upright.
    let upright = "Match the source's upright vs italic exactly: use \\mathrm{...} for anything typeset \
         upright — differentials (write \\mathrm{d}x, not dx), the constants e and i when upright, \
         multi-letter operators, and units. Use \\operatorname{...} for named operators.";
    let prompt = if multi {
        format!(
            "You are an OCR engine for mathematics. Transcribe every equation in this image into LaTeX. \
             If there are multiple stacked equations, output them inside \\begin{{gathered}} … \\end{{gathered}}, \
             one per line separated by \\\\. {upright} Output ONLY the LaTeX for the math — no surrounding $ signs, \
             no \\[ \\], no code fences, no explanation."
        )
    } else {
        format!(
            "You are an OCR engine for mathematics. Transcribe the equation in this image into a single line \
             of LaTeX. {upright} Output ONLY the LaTeX for the math — no surrounding $ signs, no \\[ \\], \
             no code fences, no explanation."
        )
    };
    let client = ai::client().map_err(|e| e.to_string())?;
    let (out, _truncated) = ai::generate_vision(&client, &provider, &url, &model, &prompt, &b64, 512)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let latex = clean_vision_latex(&out);
    if latex.trim().is_empty() {
        return Err("Il modello non ha restituito LaTeX".into());
    }
    Ok(latex)
}

/// Extract a cropped table image into a grid via a local vision LLM (asks for CSV,
/// parses it) instead of the native pdfium text extraction.
#[tauri::command]
pub async fn table_from_image_ai(
    app: AppHandle,
    image_base64: String,
    model: String,
) -> Result<Vec<Vec<String>>, String> {
    let (provider, url, model) = vision_target(&app, model)?;
    let b64 = strip_data_url(&image_base64).to_string();
    let prompt = "You are an OCR engine for tables. Extract the table in this image and output it as CSV: \
        one row per line, fields separated by commas, wrap a field in double quotes if it contains a comma. \
        Preserve the original cell text. Output ONLY the CSV — no explanation, no code fences.";
    let client = ai::client().map_err(|e| e.to_string())?;
    let (out, truncated) = ai::generate_vision(&client, &provider, &url, &model, prompt, &b64, 4096)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if truncated {
        return Err("La tabella è troppo grande per il modello di visione (output troncato). \
                    Usa il motore «Nativa» per tabelle grandi."
            .into());
    }
    let grid = parse_csv(&out);
    if grid.is_empty() {
        return Err("Il modello non ha restituito una tabella".into());
    }
    Ok(grid)
}

/// OCR a cropped text region via a local vision LLM (Ollama / LM Studio) — an
/// alternative to native pdfium extraction, useful for scanned pages.
#[tauri::command]
pub async fn text_from_image_ai(
    app: AppHandle,
    image_base64: String,
    model: String,
) -> Result<String, String> {
    let (provider, url, model) = vision_target(&app, model)?;
    let b64 = strip_data_url(&image_base64).to_string();
    let prompt = "You are an OCR engine. Transcribe ALL the text visible in this image faithfully, \
        preserving reading order and line/paragraph breaks. Do not translate, summarize, comment, \
        or add anything. Output ONLY the transcribed text — no explanation, no code fences.";
    let client = ai::client().map_err(|e| e.to_string())?;
    let (out, _truncated) = ai::generate_vision(&client, &provider, &url, &model, prompt, &b64, 2048)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let text = out.trim().to_string();
    if text.is_empty() {
        return Err("Il modello non ha restituito testo".into());
    }
    Ok(text)
}

/// Free GPU/VRAM by unloading the models the active provider currently holds.
/// Ollama: re-request each loaded model with `keep_alive:0`. LM Studio: `lms unload --all`.
/// Returns the number of Ollama models unloaded (0 for LM Studio, which is fire-and-forget).
#[tauri::command]
pub async fn ai_unload_models(app: AppHandle) -> Result<usize, String> {
    let (provider, url) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.provider.clone(), c.active_url().to_string())
    };
    if ai::is_lmstudio(&provider) {
        run_powershell(&format!("{LMS_RESOLVE} & $lms unload --all; exit 0"))?;
        Ok(0)
    } else {
        let client = ai::client().map_err(|e| e.to_string())?;
        ai::unload_ollama(&client, &url)
            .await
            .map_err(|e| format!("{e:#}"))
    }
}

/// Write arbitrary text to a file (used by "extract text" → Save .txt/.md).
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Write raw bytes (decoded from a base64 or data-URL string) to a file — used to
/// save a cropped page region as a real PNG image.
#[tauri::command]
pub fn write_binary_file(path: String, base64: String) -> Result<(), String> {
    let b64 = strip_data_url(&base64);
    let bytes = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| format!("base64 non valido: {e}"))?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Save a pasted image (as a base64 data-URL) into the notes vault's `assets/`
/// folder, returning the short relative reference (`assets/img-<hash>.<ext>`) to
/// embed in the .md — the editor stays readable instead of holding a base64 wall.
#[tauri::command]
pub fn save_note_asset(app: AppHandle, base64: String) -> Result<String, String> {
    let (mime, payload) = base64
        .strip_prefix("data:")
        .and_then(|u| u.split_once(";base64,"))
        .ok_or("Immagine non riconosciuta (atteso un data-URL base64)")?;
    let ext = notes::ext_for_mime(mime.trim()).ok_or("Formato immagine non supportato")?;
    let bytes = BASE64_STANDARD
        .decode(payload.trim())
        .map_err(|e| format!("base64 non valido: {e}"))?;
    notes::store_asset(&notes_dir(&app), &bytes, ext)
}

/// Copy an image file from disk into the vault's `assets/` (OS drag&drop of an
/// image onto the note editor — Tauri's native drag-drop hands us a path, not
/// bytes) and return the relative reference to embed in the .md.
#[tauri::command]
pub fn import_note_asset(app: AppHandle, path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| notes::mime_for_ext(e).is_some())
        .ok_or("Formato immagine non supportato")?;
    // Stat before read: don't buffer a mislabeled multi-GB file just to reject it.
    let meta = std::fs::metadata(p).map_err(|e| format!("Lettura immagine: {e}"))?;
    if meta.len() > notes::ASSET_MAX_BYTES {
        return Err(format!(
            "Immagine troppo grande ({} MB, max 20)",
            meta.len() / 1_048_576
        ));
    }
    let bytes = std::fs::read(p).map_err(|e| format!("Lettura immagine: {e}"))?;
    notes::store_asset(&notes_dir(&app), &bytes, &ext)
}

/// Write a grid to a file as CSV, Markdown, or XLSX (by `format`).
#[tauri::command]
pub fn export_table(grid: Vec<Vec<String>>, format: String, path: String) -> Result<(), String> {
    match format.as_str() {
        "xlsx" => table::to_xlsx(&grid, &path).map_err(|e| e.to_string()),
        "md" | "markdown" => std::fs::write(&path, table::to_markdown(&grid)).map_err(|e| e.to_string()),
        _ => std::fs::write(&path, table::to_csv(&grid)).map_err(|e| e.to_string()),
    }
}

/// Parse simple CSV text (RFC4180-ish) into a grid, skipping markdown fences.
fn parse_csv(s: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in s.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with("```") {
            continue;
        }
        let mut fields = Vec::new();
        let mut cur = String::new();
        let mut in_q = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_q {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        cur.push('"');
                        chars.next();
                    } else {
                        in_q = false;
                    }
                } else {
                    cur.push(c);
                }
            } else if c == '"' {
                in_q = true;
            } else if c == ',' {
                fields.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        }
        fields.push(cur);
        rows.push(fields.into_iter().map(|f| f.trim().to_string()).collect());
    }
    rows
}

/// Refine a roughly-extracted grid with the local AI (Ollama / LM Studio).
#[tauri::command]
pub async fn ai_clean_table(app: AppHandle, grid: Vec<Vec<String>>) -> Result<Vec<Vec<String>>, String> {
    let (enabled, provider, url, model) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone())
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    let tsv = grid.iter().map(|r| r.join("\t")).collect::<Vec<_>>().join("\n");
    let prompt = format!(
        "Ecco una tabella estratta in modo grezzo da un PDF (celle separate da TAB, righe da newline). \
         Correggi l'allineamento di righe e colonne e restituisci SOLO la tabella pulita in CSV valido, \
         senza spiegazioni, senza blocchi di codice.\n\n{}",
        ai::truncate(&tsv, 6000)
    );
    let client = ai::client().map_err(|e| e.to_string())?;
    let out = ai::generate(&client, &provider, &url, &model, &prompt, 1500)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let cleaned = parse_csv(&out);
    if cleaned.is_empty() {
        return Err("Il modello non ha prodotto una tabella valida".into());
    }
    Ok(cleaned)
}

// ===== Online discovery (arXiv + OpenAlex) =====

fn setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| {
        r.get::<_, String>(0)
    })
    .optional()
    .ok()
    .flatten()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoverySettings {
    pub enabled: bool,
    pub email: String,
    // Secrets live in the OS vault; the renderer only learns whether each is set.
    pub has_openalex_key: bool,
    pub has_ads_token: bool,
    pub has_s2_key: bool,
    pub has_core_key: bool,
    pub has_github_token: bool,
}

#[tauri::command]
pub fn get_discovery_settings(state: State<'_, AppState>) -> Result<DiscoverySettings, String> {
    let conn = state.db.lock();
    Ok(DiscoverySettings {
        enabled: setting(&conn, "discovery_enabled").as_deref() == Some("1"),
        email: setting(&conn, "discovery_email").unwrap_or_default(),
        has_openalex_key: secret::has("openalex_key"),
        has_ads_token: secret::has("ads_token"),
        has_s2_key: secret::has("s2_key"),
        has_core_key: secret::has("core_key"),
        has_github_token: secret::has("github_token"),
    })
}

#[tauri::command]
pub fn set_discovery_settings(
    state: State<'_, AppState>,
    enabled: bool,
    email: String,
) -> Result<(), String> {
    let conn = state.db.lock();
    let put = |k: &str, v: &str| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![k, v],
        )
    };
    put("discovery_enabled", if enabled { "1" } else { "0" }).map_err(|e| e.to_string())?;
    put("discovery_email", &email).map_err(|e| e.to_string())?;
    Ok(())
}

/// Store or clear one API key in the OS credential vault. Empty value = remove.
#[tauri::command]
pub fn set_api_key(name: String, value: String) -> Result<(), String> {
    if !secret::is_known(&name) {
        return Err("chiave sconosciuta".into());
    }
    secret::set(&name, &value)
}

/// One-time migration of plaintext API keys from the settings table into the vault.
/// Never blanks a DB value unless its secret is safely in the vault — so a vault
/// failure can't lose a key (it just retries on the next launch).
pub fn migrate_keys_to_vault(conn: &Connection) {
    if setting(conn, "keys_migrated_v1").as_deref() == Some("1") {
        return;
    }
    let blank = |name: &str| {
        let _ = conn.execute("UPDATE settings SET value = '' WHERE key = ?1", params![name]);
    };
    let mut all_ok = true;
    for &name in secret::KEYS {
        let Some(v) = setting(conn, name) else { continue };
        let v = v.trim().to_string();
        if v.is_empty() || secret::has(name) {
            blank(name); // nothing to migrate, or already in the vault
        } else if secret::set(name, &v).is_ok() {
            blank(name);
        } else {
            all_ok = false; // vault write failed — keep the DB copy and retry later
        }
    }
    if all_ok {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('keys_migrated_v1', '1')",
            [],
        );
    }
}

/// Set a discovered/reference document's metadata + authors (replace).
fn write_result_meta(conn: &mut Connection, id: i64, r: &discovery::SearchResult) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE documents
         SET title = COALESCE(?1, title), year = ?2, venue = ?3,
             doi = COALESCE(?4, doi), abstract = COALESCE(?5, abstract)
         WHERE id = ?6",
        params![r.title, r.year, r.venue, r.doi, r.abstract_text, id],
    )?;
    tx.execute("DELETE FROM document_authors WHERE document_id = ?1", params![id])?;
    for (pos, name) in r.authors.iter().enumerate() {
        let (given, family) = parse_author(name);
        if given.is_none() && family.is_none() {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO authors (family, given) VALUES (?1, ?2)",
            params![family, given],
        )?;
        let aid: i64 = tx.query_row(
            "SELECT id FROM authors WHERE family IS ?1 AND given IS ?2",
            params![family, given],
            |r2| r2.get(0),
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO document_authors (document_id, author_id, position) VALUES (?1, ?2, ?3)",
            params![id, aid, pos as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Max size of a discovery-downloaded PDF (bounded to avoid OOM / disk abuse).
const MAX_PDF_BYTES: usize = 100 * 1024 * 1024; // 100 MB

/// Normalize a discovery field into a single safe filename component (no path separators).
fn safe_component(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect()
}

/// Extract the host from a URL authority (handles `user@`, `:port`, and `[ipv6]`).
fn host_of(authority: &str) -> String {
    let a = authority.rsplit('@').next().unwrap_or(authority);
    if let Some(rest) = a.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("").to_ascii_lowercase();
    }
    a.split(':').next().unwrap_or(a).to_ascii_lowercase()
}

/// SSRF guard: only allow https to public hosts (reject loopback/private/link-local
/// IP literals and localhost). Note: a DNS name resolving to an internal IP is not
/// fully blocked here — that would require resolve-and-pin.
/// Whether an IP literal is a routable public address. Rejects loopback, private,
/// link-local, unique-local, multicast and unspecified — for IPv4, IPv6, and
/// IPv4-mapped IPv6 (e.g. `::ffff:127.0.0.1`).
fn ip_is_public(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast())
        }
        IpAddr::V6(v6) => {
            // Treat an IPv4-mapped address by its embedded IPv4 (resolvers do too).
            if let Some(v4) = v6.to_ipv4_mapped() {
                return ip_is_public(IpAddr::V4(v4));
            }
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return false;
            }
            let seg0 = v6.segments()[0];
            // fc00::/7 (unique-local) or fe80::/10 (link-local).
            if (seg0 & 0xfe00) == 0xfc00 || (seg0 & 0xffc0) == 0xfe80 {
                return false;
            }
            true
        }
    }
}

/// Fetch-time URL safety, parsed with the SAME engine reqwest connects with
/// (`reqwest::Url` / the WHATWG url parser), so the host we validate is byte-identical to
/// the host reqwest actually connects to — no parser-differential bypass via a backslash,
/// userinfo `@`, a trailing dot, IDN, or obfuscated numeric IPs (the url parser
/// canonicalizes `0x..`/decimal forms into a dotted-quad). https on the default port only.
/// IP-literal hosts must be public — reqwest connects to literals directly, skipping the
/// DNS resolver, so they are vetted here; domain hosts are enforced public-only at connect
/// time by [`PublicOnlyResolver`].
fn is_safe_fetch_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    // https only, and refuse an explicit non-default port so the fetch can't be aimed at
    // an arbitrary service port (the url parser strips the default :443, leaving None).
    if parsed.scheme() != "https" || parsed.port().is_some() {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    // host_str is canonical (exactly what reqwest connects to); strip IPv6 brackets.
    let h = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = h.parse::<std::net::IpAddr>() {
        return ip_is_public(ip);
    }
    // Loopback alias names: blocked here too (the resolver would also reject them, but this
    // is the classic SSRF target, so refuse it at the gate as well).
    let lower = h.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return false;
    }
    true
}

/// A reqwest DNS resolver that resolves names off the async runtime and only ever hands
/// back *public* addresses. reqwest connects to exactly what this returns — for the
/// initial host AND every redirect hop — so a name that (re)resolves to an
/// internal/loopback/link-local address yields an empty set and the connect fails. This
/// closes the DNS-rebinding TOCTOU at its root: the resolution that vets the host IS the
/// one reqwest connects with, so there is no independent second lookup left to rebind.
struct PublicOnlyResolver;

impl reqwest::dns::Resolve for PublicOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            // getaddrinfo blocks — keep it off the async worker threads (same pattern the
            // rest of this module uses for blocking work).
            let resolved: Vec<std::net::SocketAddr> = tauri::async_runtime::spawn_blocking(move || {
                use std::net::ToSocketAddrs;
                (host.as_str(), 0u16)
                    .to_socket_addrs()
                    .map(|it| it.collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default();
            let public: Vec<std::net::SocketAddr> =
                resolved.into_iter().filter(|a| ip_is_public(a.ip())).collect();
            let out: Result<reqwest::dns::Addrs, Box<dyn std::error::Error + Send + Sync>> =
                if public.is_empty() {
                    Err("host did not resolve to a permitted public address".into())
                } else {
                    Ok(Box::new(public.into_iter()))
                };
            out
        })
    }
}

/// Per-process counter for unique temp download filenames.
static DL_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

async fn download_pdf(app: &AppHandle, url: &str) -> Result<Option<PathBuf>, String> {
    // SSRF guard: refuse non-https, a non-default port, or an internal IP-literal host.
    if !is_safe_fetch_url(url) {
        return Ok(None);
    }
    // Defense-in-depth on redirects: refuse a hop to a non-https / non-default-port /
    // internal-IP-literal target (domain hops are still vetted by the resolver below).
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 {
            attempt.stop()
        } else if is_safe_fetch_url(attempt.url().as_str()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    // Connect through a resolver that returns ONLY public IPs (and runs getaddrinfo off the
    // async runtime). reqwest connects to exactly what it resolves here — the initial host
    // and every redirect hop — so a DNS-rebinding flip to an internal address resolves to
    // an empty set and the connect fails. This is the real anti-rebinding guarantee.
    let client = reqwest::Client::builder()
        .user_agent("Scriptorium/0.1")
        .timeout(std::time::Duration::from_secs(60))
        .redirect(policy)
        .dns_resolver(PublicOnlyResolver)
        .build()
        .map_err(|e| e.to_string())?;
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !resp.status().is_success() {
        return Ok(None);
    }
    // Re-check after redirects: the final URL must still be a safe public host.
    if !is_safe_fetch_url(resp.url().as_str()) {
        return Ok(None);
    }
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    // Bounded streaming read — never buffer more than MAX_PDF_BYTES.
    let mut resp = resp;
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                if bytes.len() + chunk.len() > MAX_PDF_BYTES {
                    return Ok(None);
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => return Ok(None),
        }
    }
    // Only accept actual PDFs (some OA links are HTML landing pages).
    if !ct.contains("pdf") && !bytes.starts_with(b"%PDF") {
        return Ok(None);
    }
    // Store PDFs content-addressed (papers/{sha256}.pdf), written via a unique
    // temp + atomic rename. This makes storage collision-free: distinct content
    // never overwrites another document's file, and identical content maps to one
    // shared file — so callers can dedupe by hash and NEVER delete a file that
    // another document row still references.
    let dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("papers"))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).ok();
    let hash = import::sha256_hex(&bytes);
    let path = dir.join(format!("{hash}.pdf"));
    // Defense-in-depth: never write outside the papers directory.
    if path.parent() != Some(dir.as_path()) {
        return Err("percorso di salvataggio non valido".into());
    }
    if path.exists() {
        return Ok(Some(path)); // identical content already stored — reuse it
    }
    let seq = DL_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".dl-{}-{}.part", std::process::id(), seq));
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    match std::fs::rename(&tmp, &path) {
        Ok(()) => Ok(Some(path)),
        Err(_) => {
            // A concurrent download may have just created it; fall back to it.
            let _ = std::fs::remove_file(&tmp);
            if path.exists() {
                Ok(Some(path))
            } else {
                Err("impossibile salvare il PDF".into())
            }
        }
    }
}

/// Search arXiv or OpenAlex (requires discovery enabled in settings).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn discover_search(
    app: AppHandle,
    query: String,
    source: String,
    author: Option<String>,
    year_from: Option<i64>,
    year_to: Option<i64>,
    oa_only: bool,
    sort: String,
) -> Result<Vec<discovery::SearchResult>, String> {
    let (enabled, key, ads_token, s2_key, core_key, email) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        (
            setting(&conn, "discovery_enabled").as_deref() == Some("1"),
            secret::get("openalex_key").unwrap_or_default(),
            secret::get("ads_token").unwrap_or_default(),
            secret::get("s2_key").unwrap_or_default(),
            secret::get("core_key").unwrap_or_default(),
            setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty()),
        )
    };
    if !enabled {
        return Err("La ricerca online è disattivata (abilitala nelle impostazioni)".into());
    }
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let filters = discovery::Filters {
        year_from,
        year_to,
        oa_only,
        sort,
        author,
    };
    let mut results = match source.as_str() {
        "arxiv" => discovery::search_arxiv(&client, &query, &filters)
            .await
            .map_err(|e| e.to_string())?,
        "ads" => discovery::search_ads(&client, &query, &filters, &ads_token)
            .await
            .map_err(|e| e.to_string())?,
        "semanticscholar" => discovery::search_semantic_scholar(&client, &query, &filters, &s2_key)
            .await
            .map_err(|e| e.to_string())?,
        "europepmc" => discovery::search_europepmc(&client, &query, &filters)
            .await
            .map_err(|e| e.to_string())?,
        "core" => discovery::search_core(&client, &query, &filters, &core_key)
            .await
            .map_err(|e| e.to_string())?,
        "doaj" => discovery::search_doaj(&client, &query, &filters)
            .await
            .map_err(|e| e.to_string())?,
        "huggingface" => discovery::search_huggingface(&client, &query, &filters)
            .await
            .map_err(|e| e.to_string())?,
        _ => discovery::search_openalex(&client, &query, &filters, &key)
            .await
            .map_err(|e| e.to_string())?,
    };

    let state = app.state::<AppState>();
    let conn = state.db.lock();
    for r in &mut results {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM documents WHERE deleted_at IS NULL AND ((doi IS NOT NULL AND doi = ?1) OR (?2 <> '' AND path LIKE ?3)) LIMIT 1",
                params![r.doi, r.external_id, format!("%{}%", r.external_id)],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        r.in_library = exists.is_some();
        // Surface a GitHub repo link detected in the result's title/abstract.
        if r.github_url.is_none() {
            let t = format!("{} {}", r.title.as_deref().unwrap_or(""), r.abstract_text.as_deref().unwrap_or(""));
            r.github_url = github::first_repo_url(&t);
        }
        r.pub_status = discovery::classify_pub_status(r.doi.as_deref(), r.venue.as_deref(), Some(&source));
    }
    Ok(results)
}

/// "Snowball" citation explorer: for a paper's DOI, fetch from OpenAlex the works
/// it references (backward) and the works that cite it (forward), marking which
/// are already in the library. Each neighbour is a `SearchResult`, so the UI can
/// add it with the existing `discover_add`. Network-gated like `discover_search`.
#[tauri::command]
pub async fn explore_citations(
    app: AppHandle,
    doi: Option<String>,
    openalex_id: Option<String>,
    title: Option<String>,
) -> Result<discovery::CitationNeighbors, String> {
    let doi = doi.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
    let oa_id = openalex_id.map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    let title = title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    // Seed priority: an OpenAlex id names the work exactly (ghost stars always
    // carry one); a DOI is exact too; a title is the gated last resort.
    let wref = if let Some(x) = oa_id.as_deref() {
        discovery::WorkRef::Id(x)
    } else if let Some(d) = doi.as_deref() {
        discovery::WorkRef::Doi(d)
    } else if let Some(t) = title.as_deref() {
        // The strict gate needs a title with some substance: an ultra-short one
        // would only be accepted on exact equality, but the search itself gets
        // too noisy — refuse HONESTLY instead of pretending nothing was given.
        if t.chars().count() < 8 {
            return Err("Senza DOI serve un titolo di almeno 8 caratteri per l'aggancio sicuro su OpenAlex".into());
        }
        discovery::WorkRef::Title(t)
    } else {
        return Err("Serve un DOI, un id OpenAlex o almeno un titolo per esplorare le citazioni".into());
    };
    let (enabled, key, email) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        (
            setting(&conn, "discovery_enabled").as_deref() == Some("1"),
            secret::get("openalex_key").unwrap_or_default(),
            setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty()),
        )
    };
    if !enabled {
        return Err("La ricerca online è disattivata (abilitala nelle impostazioni)".into());
    }
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let mut nb = discovery::openalex_neighbors(&client, wref, &key, 40)
        .await
        .map_err(|e| e.to_string())?;

    let state = app.state::<AppState>();
    let conn = state.db.lock();
    for r in nb.references.iter_mut().chain(nb.citations.iter_mut()) {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM documents WHERE deleted_at IS NULL AND ((doi IS NOT NULL AND doi = ?1) OR (?2 <> '' AND path LIKE ?3)) LIMIT 1",
                params![r.doi, r.external_id, format!("%{}%", r.external_id)],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        r.in_library = exists.is_some();
        r.pub_status = discovery::classify_pub_status(r.doi.as_deref(), r.venue.as_deref(), Some("openalex"));
    }
    Ok(nb)
}

/// Add a search result: download the OA PDF if available, else a metadata-only
/// reference. Returns "added_pdf" | "added_ref" | "duplicate".
#[tauri::command]
pub async fn discover_add(app: AppHandle, result: discovery::SearchResult) -> Result<String, String> {
    // Gate behind the opt-in network setting (consistent with discover_search).
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        if setting(&conn, "discovery_enabled").as_deref() != Some("1") {
            return Err("La ricerca online è disattivata (abilitala nelle impostazioni)".into());
        }
    }
    // Dedup by DOI or external id.
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let dup: Option<i64> = conn
            .query_row(
                "SELECT id FROM documents WHERE deleted_at IS NULL AND ((doi IS NOT NULL AND doi = ?1) OR (?2 <> '' AND path LIKE ?3)) LIMIT 1",
                params![result.doi, result.external_id, format!("%{}%", result.external_id)],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if dup.is_some() {
            return Ok("duplicate".into());
        }
    }

    // Try the OA PDF.
    if let Some(url) = result.oa_pdf_url.clone() {
        if let Ok(Some(saved)) = download_pdf(&app, &url).await {
            let state = app.state::<AppState>();
            let dir = thumb_dir(&app);
            let prepared = {
                let _pdf_guard = state.pdfium_lock.lock();
                import::prepare_import(&state.pdfium, &dir, &saved)
            };
            if let Ok(prepared) = prepared {
                let mut conn = state.db.lock();
                match import::commit_import(&conn, &prepared) {
                    Ok(o) if o.imported => {
                        write_result_meta(&mut conn, o.document_id, &result).map_err(|e| e.to_string())?;
                        return Ok("added_pdf".into());
                    }
                    Ok(_) => return Ok("duplicate".into()),
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
    }

    // Metadata-only reference.
    let state = app.state::<AppState>();
    let mut conn = state.db.lock();
    let path_id = format!("ref:{}:{}", result.source, result.external_id);
    conn.execute("INSERT INTO documents (path) VALUES (?1)", params![path_id])
        .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    write_result_meta(&mut conn, id, &result).map_err(|e| e.to_string())?;
    Ok("added_ref".into())
}

// ===== "Aggancia da URL" + browser connector =====

/// Best-effort single-document enrichment: resolve the paper's identity from its
/// own recovered title (a DOI in the text is trusted only when its Crossref title
/// matches this paper, never a *cited* work's). Never fatal.
async fn enrich_one(app: &AppHandle, id: i64) -> Result<(), String> {
    let (fulltext, existing_doi, email): (String, Option<String>, Option<String>) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let row: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT COALESCE(fulltext,''), doi FROM documents WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let (ft, doi) = row.unwrap_or_default();
        let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
        (ft, doi, email)
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    if let Ok(Ok(Some(r))) = tokio::time::timeout(
        std::time::Duration::from_secs(45),
        metadata::resolve_document_meta(&client, &fulltext, existing_doi.as_deref(), email.as_deref()),
    )
    .await
    {
        let state = app.state::<AppState>();
        let mut conn = state.db.lock();
        if apply_resolved(&mut conn, id, &r).is_ok() {
            let _ = crate::db::citekey::auto_citekey(&conn, id);
        }
    }
    Ok(())
}

/// A readable fallback title derived from a URL's last path segment. Content-
/// addressed storage means the on-disk filename is a hash, so `prepare_import`'s
/// filename-based title would otherwise be that hash — this keeps it human.
fn title_from_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let seg = u
        .path_segments()
        .and_then(|s| s.filter(|x| !x.is_empty()).last().map(|x| x.to_string()));
    let raw = seg.or_else(|| u.host_str().map(|h| h.to_string()))?;
    let base = raw.trim_end_matches(".pdf").trim_end_matches(".PDF").trim();
    if base.is_empty() {
        return None;
    }
    let t: String = base.replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let t: String = t.chars().take(200).collect();
    (!t.is_empty()).then_some(t)
}

/// The arXiv id embedded in an arxiv.org PDF/abstract URL, if any.
fn arxiv_id_from_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    if !u.host_str()?.to_ascii_lowercase().contains("arxiv.org") {
        return None;
    }
    let last = u.path_segments()?.filter(|s| !s.is_empty()).last()?;
    // `arxiv_id_from_filename` strips a trailing extension via `rsplit_once('.')`,
    // which would eat the id's own dot on a bare id ("1706.03762" → "1706"). Give
    // it a real ".pdf" tail so only that is stripped and the id stays intact.
    let stem = last.trim_end_matches(".pdf").trim_end_matches(".PDF");
    metadata::arxiv_id_from_filename(&format!("{stem}.pdf"))
}

/// Best-effort: overwrite a freshly-grabbed arXiv document's metadata from the
/// authoritative arXiv record. The id came from the user-chosen URL, so (unlike
/// enrichment off body text) no title gate is needed. Never fatal.
async fn enrich_from_arxiv(app: &AppHandle, id: i64, aid: &str) -> Result<(), String> {
    let email = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty())
    };
    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    let fetched = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        metadata::fetch_arxiv(&client, aid),
    )
    .await;
    if let Ok(Ok(Some(meta))) = fetched {
        if meta.title.as_deref().map(|t| !t.trim().is_empty()).unwrap_or(false) {
            let state = app.state::<AppState>();
            let mut conn = state.db.lock();
            if write_repaired(&mut conn, id, &meta).is_ok() {
                let _ = crate::db::citekey::auto_citekey(&conn, id);
            }
        }
    }
    Ok(())
}

/// Run pdfium extraction/render on a dedicated LARGE-stack thread. pdfium can
/// recurse deeply parsing complex PDFs and overflow a default (~2 MB) worker or
/// connector thread stack, which surfaces on Windows as a native crash
/// (0xc0000409, stack-cookie failure). A generous stack makes it safe no matter
/// which thread the caller runs on.
fn prepare_import_big_stack(
    pdfium: &pdfium_render::prelude::Pdfium,
    dir: &Path,
    saved: &Path,
) -> Result<import::PreparedImport, String> {
    std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .name("scriptorium-pdf-extract".into())
            .stack_size(64 * 1024 * 1024)
            .spawn_scoped(s, || import::prepare_import(pdfium, dir, saved))
            .map_err(|e| e.to_string())?;
        handle
            .join()
            .map_err(|_| "estrazione PDF interrotta".to_string())?
            .map_err(|e| format!("{e:#}"))
    })
}

/// Normalize URLs that point at an HTML *viewer* of a PDF rather than at the
/// file itself. Currently: GitHub blob pages (`github.com/{o}/{r}/blob/…`) →
/// `raw.githubusercontent.com/{o}/{r}/…`, so both "Aggancia da URL" and the
/// browser connector accept the link you actually have in the address bar
/// (the blob page is HTML and would fail the `%PDF` gate). Path segments are
/// kept percent-encoded as-is; anything unrecognized passes through untouched.
fn normalize_pdf_url(url: &str) -> String {
    if let Ok(u) = reqwest::Url::parse(url) {
        let host = u.host_str().unwrap_or("").to_ascii_lowercase();
        if u.scheme() == "https" && (host == "github.com" || host == "www.github.com") {
            let segs: Vec<&str> = u.path().trim_start_matches('/').split('/').collect();
            // /{owner}/{repo}/blob/{branch}/{path…}
            if segs.len() >= 5 && segs[2] == "blob" {
                return format!(
                    "https://raw.githubusercontent.com/{}/{}/{}",
                    segs[0],
                    segs[1],
                    segs[3..].join("/")
                );
            }
        }
    }
    url.to_string()
}

/// Shared engine for "aggancia da URL": SSRF-guarded download → import pipeline →
/// best-effort metadata enrichment. Returns `"added"` | `"duplicate"` |
/// `"not_pdf"`. Emits `library-changed` when a new document lands. Used by the
/// [`add_from_url`] command and by the browser connector's loopback server.
pub(crate) async fn import_from_url(app: &AppHandle, url: &str) -> Result<&'static str, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL vuoto".into());
    }
    let url = &normalize_pdf_url(url);
    // The SSRF guard returns Ok(None) for non-https / internal / non-PDF / oversize.
    // Storage is content-addressed, so `saved` is shared by identical content and
    // must never be deleted here (an existing document row may reference it).
    let saved = match download_pdf(app, url).await? {
        Some(p) => p,
        None => return Ok("not_pdf"),
    };
    let dir = thumb_dir(app);
    let prepared = {
        let state = app.state::<AppState>();
        // Serialize whole-document pdfium work vs. the startup scan / other imports.
        let _pdf_guard = state.pdfium_lock.lock();
        prepare_import_big_stack(&state.pdfium, &dir, &saved)?
    };
    let outcome = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        import::commit_import(&conn, &prepared)
    }
    .map_err(|e| e.to_string())?;
    if !outcome.imported {
        return Ok("duplicate");
    }
    let id = outcome.document_id;
    // Never leave the raw content-hash filename as the title: seed a readable one
    // from the URL, then improve it from authoritative metadata (arXiv by id from
    // the grabbed URL, else a DOI found in the text → Crossref). All best-effort.
    if let Some(t) = title_from_url(url) {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let _ = conn.execute("UPDATE documents SET title = ?1 WHERE id = ?2", params![t, id]);
    }
    if let Some(aid) = arxiv_id_from_url(url) {
        let _ = enrich_from_arxiv(app, id, &aid).await;
    } else {
        let _ = enrich_one(app, id).await;
    }
    let _ = app.emit("library-changed", ());
    Ok("added")
}

/// Download a PDF from a URL and import it into the library (the in-app path;
/// the bookmarklet uses the connector, which calls the same engine). Returns
/// `"added"` | `"duplicate"` | `"not_pdf"`.
#[tauri::command]
pub async fn add_from_url(app: AppHandle, url: String) -> Result<String, String> {
    import_from_url(&app, &url).await.map(|s| s.to_string())
}

/// Read (or lazily create + persist) the connector's secret token — a 128-bit
/// random hex string embedded only in the user's bookmarklet, so a random web
/// page can't drive imports against the loopback server.
fn connector_token(conn: &Connection) -> String {
    if let Some(t) = setting(conn, "connector_token").filter(|t| t.len() >= 16) {
        return t;
    }
    let tok: String = conn
        .query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))
        .unwrap_or_else(|_| "scriptorium".to_string());
    let _ = conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('connector_token', ?1)",
        params![tok],
    );
    tok
}

/// Read the saved (or default) connector port from settings.
fn connector_pref_port(conn: &Connection) -> u16 {
    setting(conn, "connector_port")
        .and_then(|p| p.trim().parse::<u16>().ok())
        .unwrap_or(connector::DEFAULT_PORT)
}

/// (Re)start the loopback connector if enabled and not already running,
/// persisting the actually-bound port so the bookmarklet stays in sync.
pub(crate) fn start_connector(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.connector.lock().is_some() {
        return; // already running
    }
    let (enabled, token, pref) = {
        let conn = state.db.lock();
        (
            // Opt-in: off unless the user explicitly enabled it (keeps a fresh
            // install 100% offline until they set up the bookmarklet).
            setting(&conn, "connector_enabled").as_deref() == Some("1"),
            connector_token(&conn),
            connector_pref_port(&conn),
        )
    };
    if !enabled {
        return;
    }
    if let Some(handle) = connector::start(app.clone(), pref, token) {
        let bound = handle.port;
        {
            let conn = state.db.lock();
            let _ = conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('connector_port', ?1)",
                params![bound.to_string()],
            );
        }
        state.connector.lock().replace(handle);
    }
}

/// Stop the loopback connector if running (frees the socket).
pub(crate) fn stop_connector(app: &AppHandle) {
    let state = app.state::<AppState>();
    let handle = state.connector.lock().take(); // drop the guard before stopping
    if let Some(h) = handle {
        h.stop();
    }
}

#[derive(serde::Serialize)]
pub struct ConnectorInfo {
    /// Whether the connector is allowed to run (persisted preference).
    pub enabled: bool,
    /// Whether the loopback server is actually bound right now.
    pub running: bool,
    pub port: u16,
    /// The secret token the bookmarklet must present.
    pub token: String,
}

/// Current connector state + the port/token the frontend needs to build the
/// bookmarklet.
#[tauri::command]
pub fn get_connector_info(state: State<'_, AppState>) -> Result<ConnectorInfo, String> {
    let (enabled, token, saved_port) = {
        let conn = state.db.lock();
        (
            setting(&conn, "connector_enabled").as_deref() == Some("1"),
            connector_token(&conn),
            connector_pref_port(&conn),
        )
    };
    let (running, port) = {
        let g = state.connector.lock();
        match g.as_ref() {
            Some(h) => (true, h.port),
            None => (false, saved_port),
        }
    };
    Ok(ConnectorInfo { enabled, running, port, token })
}

/// Enable/disable the connector and start/stop it live. Returns the new state.
#[tauri::command]
pub fn set_connector_enabled(app: AppHandle, enabled: bool) -> Result<ConnectorInfo, String> {
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('connector_enabled', ?1)",
            params![if enabled { "1" } else { "0" }],
        );
    }
    stop_connector(&app);
    if enabled {
        start_connector(&app);
    }
    get_connector_info(app.state::<AppState>())
}

// ===== Saved searches ("monitor a topic") =====

#[derive(serde::Serialize)]
pub struct SavedSearch {
    pub id: i64,
    pub name: String,
    pub source: String,
    pub query: String,
    pub author: Option<String>,
    pub year_from: Option<i64>,
    pub year_to: Option<i64>,
    pub oa_only: bool,
    pub sort: String,
    pub last_run_at: Option<String>,
    /// Whether the on-launch "Novità" sweep re-runs this search automatically.
    pub auto_run: bool,
}

fn row_to_saved(r: &rusqlite::Row) -> rusqlite::Result<SavedSearch> {
    Ok(SavedSearch {
        id: r.get(0)?,
        name: r.get(1)?,
        source: r.get(2)?,
        query: r.get(3)?,
        author: r.get(4)?,
        year_from: r.get(5)?,
        year_to: r.get(6)?,
        oa_only: r.get::<_, i64>(7)? != 0,
        sort: r.get(8)?,
        last_run_at: r.get(10)?,
        auto_run: r.get::<_, i64>(11)? != 0,
    })
}

const SAVED_COLS: &str = "id, name, source, query, author, year_from, year_to, oa_only, sort, seen_ids, last_run_at, auto_run";

#[tauri::command]
pub fn list_saved_searches(state: State<'_, AppState>) -> Result<Vec<SavedSearch>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(&format!("SELECT {SAVED_COLS} FROM saved_searches ORDER BY name COLLATE NOCASE"))
        .map_err(|e| e.to_string())?;
    let v = stmt
        .query_map([], row_to_saved)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(v)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn create_saved_search(
    state: State<'_, AppState>,
    name: String,
    source: String,
    query: String,
    author: Option<String>,
    year_from: Option<i64>,
    year_to: Option<i64>,
    oa_only: bool,
    sort: String,
    seen_ids: Vec<String>,
) -> Result<SavedSearch, String> {
    let conn = state.db.lock();
    // Initialize "seen" with the results the user is already looking at, so the
    // first re-run only flags genuinely new papers.
    let seen = seen_ids.join("\n");
    conn.execute(
        "INSERT INTO saved_searches (name, source, query, author, year_from, year_to, oa_only, sort, seen_ids)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![name, source, query, author, year_from, year_to, oa_only as i64, sort, seen],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(&format!("SELECT {SAVED_COLS} FROM saved_searches WHERE id = ?1"), params![id], row_to_saved)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_saved_search(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    // Clear the feed rows explicitly (the FK CASCADE only fires when foreign_keys
    // is ON — this keeps it correct regardless).
    conn.execute("DELETE FROM watch_hits WHERE watch_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM saved_searches WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Toggle whether a saved search participates in the on-launch Novità sweep.
#[tauri::command]
pub fn set_watch_auto_run(state: State<'_, AppState>, id: i64, auto_run: bool) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE saved_searches SET auto_run = ?1 WHERE id = ?2",
        params![auto_run as i64, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct SavedRun {
    pub name: String,
    pub results: Vec<discovery::SearchResult>,
    /// external_ids surfaced as genuinely-new this run (also persisted to watch_hits).
    pub new_ids: Vec<String>,
}

/// Shared core for both the manual "Run" and the on-launch sweep: fetch a saved
/// search's current results and persist genuinely-new ones into `watch_hits`.
///
/// Novelty = an external_id that is neither in the creation baseline (`seen_ids`)
/// nor already recorded in `watch_hits`. Crucially this NEVER mutates `seen_ids`,
/// so a manual run and the sweep can't cannibalize each other's novelty (the
/// unbounded `INSERT OR IGNORE` into `watch_hits` is the single dedup point).
/// Returns the fetched results and the external_ids freshly surfaced this run.
async fn run_watch_core(app: &AppHandle, s: &SavedSearch) -> Result<(Vec<discovery::SearchResult>, Vec<String>), String> {
    let seen: String = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.query_row("SELECT seen_ids FROM saved_searches WHERE id = ?1", params![s.id], |r| r.get(0))
            .map_err(|e| e.to_string())?
    };

    let results = discover_search(
        app.clone(),
        s.query.clone(),
        s.source.clone(),
        s.author.clone(),
        s.year_from,
        s.year_to,
        s.oa_only,
        s.sort.clone(),
    )
    .await?;

    let baseline: std::collections::HashSet<&str> = seen.lines().collect();
    let mut fresh: Vec<String> = Vec::new();
    {
        let state = app.state::<AppState>();
        let mut cg = state.db.lock();
        let tx = cg.transaction().map_err(|e| e.to_string())?;
        for r in &results {
            // Skip the frozen creation baseline AND anything already in the library:
            // the feed is only for genuinely-new papers to add. Not persisting
            // in-library hits keeps the UNIQUE slot free, so a paper removed later
            // can resurface as a novità on a future sweep.
            if baseline.contains(r.external_id.as_str()) || r.in_library {
                continue;
            }
            let json = serde_json::to_string(r).map_err(|e| e.to_string())?;
            let n = tx
                .execute(
                    "INSERT OR IGNORE INTO watch_hits (watch_id, external_id, result_json)
                     VALUES (?1, ?2, ?3)",
                    params![s.id, r.external_id, json],
                )
                .map_err(|e| e.to_string())?;
            if n > 0 {
                fresh.push(r.external_id.clone());
            }
        }
        tx.execute("UPDATE saved_searches SET last_run_at = datetime('now') WHERE id = ?1", params![s.id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok((results, fresh))
}

/// Re-run a saved search now; returns its results and which are freshly new,
/// persisting the new ones into the Novità feed.
#[tauri::command]
pub async fn run_saved_search(app: AppHandle, id: i64) -> Result<SavedRun, String> {
    let s = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.query_row(&format!("SELECT {SAVED_COLS} FROM saved_searches WHERE id = ?1"), params![id], row_to_saved)
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "ricerca non trovata".to_string())?
    };
    let (results, new_ids) = run_watch_core(&app, &s).await?;
    Ok(SavedRun { name: s.name, results, new_ids })
}

/// On-launch (or manual) sweep of every auto-run saved search whose last run is
/// older than the debounce window. Runs sequentially (each `discover_search`
/// already paces itself politely), silently no-ops when online discovery is off,
/// and emits `novita-changed` with the total count of fresh hits when done.
/// `force` ignores the debounce (used by the manual "cerca ora" button).
/// Returns the total number of freshly-surfaced hits across all swept watches.
pub async fn run_all_watches(app: AppHandle, force: bool) -> usize {
    let enabled = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        setting(&conn, "discovery_enabled").as_deref() == Some("1")
    };
    if !enabled {
        return 0;
    }
    let watches: Vec<SavedSearch> = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let sql = if force {
            format!("SELECT {SAVED_COLS} FROM saved_searches WHERE auto_run = 1 ORDER BY id")
        } else {
            format!(
                "SELECT {SAVED_COLS} FROM saved_searches
                 WHERE auto_run = 1 AND (last_run_at IS NULL OR last_run_at < datetime('now','-12 hours'))
                 ORDER BY id"
            )
        };
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        stmt.query_map([], row_to_saved)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    };
    if watches.is_empty() {
        return 0;
    }
    let mut total_new = 0usize;
    for s in &watches {
        if let Ok((_, fresh)) = run_watch_core(&app, s).await {
            total_new += fresh.len();
        }
    }
    let _ = app.emit("novita-changed", total_new);
    total_new
}

/// Manual trigger for the sweep (Settings / Novità "cerca ora"). Returns the
/// count of hits this invocation actually surfaced (not a racy global diff).
#[tauri::command]
pub async fn sweep_watches_now(app: AppHandle) -> Result<usize, String> {
    Ok(run_all_watches(app, true).await)
}

// ===== Novità feed (watch_hits) =====

#[derive(serde::Serialize)]
pub struct NovitaHit {
    pub hit_id: i64,
    pub found_at: Option<String>,
    pub result: discovery::SearchResult,
}

#[derive(serde::Serialize)]
pub struct NovitaGroup {
    pub watch_id: i64,
    pub watch_name: String,
    pub hits: Vec<NovitaHit>,
}

/// Count of unhandled ('new') feed items — drives the nav badge.
#[tauri::command]
pub fn novita_count(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.db.lock();
    conn.query_row("SELECT COUNT(*) FROM watch_hits WHERE state = 'new'", [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

/// The Novità feed: unhandled hits grouped by their watch, newest first.
#[tauri::command]
pub fn list_novita(state: State<'_, AppState>) -> Result<Vec<NovitaGroup>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT h.id, h.found_at, h.result_json, s.id, s.name
             FROM watch_hits h JOIN saved_searches s ON s.id = h.watch_id
             WHERE h.state = 'new'
             ORDER BY s.name COLLATE NOCASE, s.id, h.found_at DESC, h.id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut groups: Vec<NovitaGroup> = Vec::new();
    for row in rows.filter_map(Result::ok) {
        let (hit_id, found_at, json, watch_id, watch_name) = row;
        let Ok(result) = serde_json::from_str::<discovery::SearchResult>(&json) else {
            continue; // skip a corrupt/legacy row rather than fail the whole feed
        };
        let hit = NovitaHit { hit_id, found_at, result };
        match groups.last_mut() {
            Some(g) if g.watch_id == watch_id => g.hits.push(hit),
            _ => groups.push(NovitaGroup { watch_id, watch_name, hits: vec![hit] }),
        }
    }
    Ok(groups)
}

/// Mark one feed item as dismissed (removed from the badge/feed, not re-surfaced).
#[tauri::command]
pub fn dismiss_hit(state: State<'_, AppState>, hit_id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("UPDATE watch_hits SET state = 'dismissed' WHERE id = ?1", params![hit_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark every unhandled hit of a watch as dismissed ("segna tutti letti").
#[tauri::command]
pub fn dismiss_watch_hits(state: State<'_, AppState>, watch_id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE watch_hits SET state = 'dismissed' WHERE watch_id = ?1 AND state = 'new'",
        params![watch_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Accept a feed item: add it to the library (reuses `discover_add`) and mark the
/// hit as 'added'. Returns the discover_add outcome ("added_pdf"|"added_ref"|"duplicate").
#[tauri::command]
pub async fn accept_hit(app: AppHandle, hit_id: i64) -> Result<String, String> {
    let json: String = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.query_row("SELECT result_json FROM watch_hits WHERE id = ?1", params![hit_id], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "novità non trovata".to_string())?
    };
    let result: discovery::SearchResult = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let outcome = discover_add(app.clone(), result).await?;
    let state = app.state::<AppState>();
    let conn = state.db.lock();
    conn.execute("UPDATE watch_hits SET state = 'added' WHERE id = ?1", params![hit_id])
        .map_err(|e| e.to_string())?;
    Ok(outcome)
}

// ===== AI (Ollama / LM Studio) — optional, opt-in =====

#[derive(Debug, Clone, serde::Serialize)]
pub struct AiSettings {
    pub enabled: bool,
    /// "ollama" or "lmstudio".
    pub provider: String,
    pub ollama_url: String,
    pub lmstudio_url: String,
    pub model: String,
    /// Compute indexing embeddings via Ollama (GPU) instead of the bundled CPU model.
    pub embed_gpu: bool,
    /// Embedding batch size (0 = auto: 64 on GPU, 16 on CPU). Larger = faster on strong GPUs.
    pub embed_batch: i64,
}

/// Resolved AI configuration read from the settings table.
struct AiConfig {
    enabled: bool,
    provider: String,
    ollama_url: String,
    lmstudio_url: String,
    model: String,
    embed_gpu: bool,
    embed_batch: i64,
}

impl AiConfig {
    /// Base URL of the currently-selected provider.
    fn active_url(&self) -> &str {
        if ai::is_lmstudio(&self.provider) {
            &self.lmstudio_url
        } else {
            &self.ollama_url
        }
    }
}

fn normalize_provider(p: &str) -> String {
    if ai::is_lmstudio(p) {
        "lmstudio".to_string()
    } else {
        "ollama".to_string()
    }
}

fn ai_config(conn: &Connection) -> AiConfig {
    AiConfig {
        enabled: setting(conn, "ai_enabled").as_deref() == Some("1"),
        provider: normalize_provider(&setting(conn, "ai_provider").unwrap_or_default()),
        ollama_url: setting(conn, "ollama_url")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://localhost:11434".to_string()),
        lmstudio_url: setting(conn, "lmstudio_url")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://localhost:1234".to_string()),
        model: setting(conn, "ai_model")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "llama3.2:3b".to_string()),
        embed_gpu: setting(conn, "embed_gpu").as_deref() == Some("1"),
        embed_batch: setting(conn, "embed_batch").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0),
    }
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>) -> Result<AiSettings, String> {
    let conn = state.db.lock();
    let c = ai_config(&conn);
    Ok(AiSettings {
        enabled: c.enabled,
        provider: c.provider,
        ollama_url: c.ollama_url,
        lmstudio_url: c.lmstudio_url,
        model: c.model,
        embed_gpu: c.embed_gpu,
        embed_batch: c.embed_batch,
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn set_ai_settings(
    state: State<'_, AppState>,
    enabled: bool,
    provider: String,
    ollama_url: String,
    lmstudio_url: String,
    model: String,
    embed_gpu: bool,
    embed_batch: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    let put = |k: &str, v: &str| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![k, v],
        )
    };
    put("ai_enabled", if enabled { "1" } else { "0" }).map_err(|e| e.to_string())?;
    put("ai_provider", &normalize_provider(&provider)).map_err(|e| e.to_string())?;
    put("ollama_url", ollama_url.trim()).map_err(|e| e.to_string())?;
    put("lmstudio_url", lmstudio_url.trim()).map_err(|e| e.to_string())?;
    put("ai_model", model.trim()).map_err(|e| e.to_string())?;
    put("embed_gpu", if embed_gpu { "1" } else { "0" }).map_err(|e| e.to_string())?;
    put("embed_batch", &embed_batch.clamp(0, 512).to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

/// List the models a provider serves at the given URL — used by Settings to
/// verify connectivity and populate the model picker before saving.
#[tauri::command]
pub async fn ai_list_models(provider: String, url: String) -> Result<Vec<String>, String> {
    let client = ai::client().map_err(|e| e.to_string())?;
    ai::list_models(&client, &provider, &url)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// Live status of the AI feature for the header indicator.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiStatus {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub reachable: bool,
    pub model_available: bool,
    pub detail: String,
}

/// Check whether the currently-configured AI provider is up right now, and
/// whether the chosen model is loaded. Drives the header AI indicator.
#[tauri::command]
pub async fn ai_status(app: AppHandle) -> Result<AiStatus, String> {
    let (enabled, provider, url, model) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone())
    };
    if !enabled {
        return Ok(AiStatus {
            enabled,
            provider,
            model,
            reachable: false,
            model_available: false,
            detail: "AI disattivata".to_string(),
        });
    }
    let lbl = ai::label(&provider);
    let client = ai::client().map_err(|e| e.to_string())?;
    match ai::list_models(&client, &provider, &url).await {
        Ok(models) => {
            let available = !model.is_empty() && models.iter().any(|m| m == &model);
            let detail = if model.is_empty() {
                format!("AI attiva — {lbl} (nessun modello selezionato)")
            } else if available {
                format!("AI attiva — {lbl}: {model}")
            } else {
                format!("{lbl} attivo, ma il modello «{model}» non è caricato")
            };
            Ok(AiStatus {
                enabled,
                provider,
                model,
                reachable: true,
                model_available: available,
                detail,
            })
        }
        Err(e) => Ok(AiStatus {
            enabled,
            provider,
            model,
            reachable: false,
            model_available: false,
            detail: format!("{lbl} non raggiungibile — avvialo. ({e})"),
        }),
    }
}

/// Combined title + (abstract / fulltext) text for a document.
fn fetch_doc_text(conn: &Connection, id: i64) -> Result<(String, String), String> {
    let (title, abs, full) = conn
        .query_row(
            "SELECT COALESCE(title,''), COALESCE(abstract,''), COALESCE(fulltext,'') FROM documents WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;
    let body = if !full.trim().is_empty() { full } else { abs.clone() };
    let text = if !abs.trim().is_empty() && abs != body {
        format!("{abs}\n\n{body}")
    } else {
        body
    };
    Ok((title, text))
}

/// Generate an Italian summary for a document (manual, opt-in). Caches it.
#[tauri::command]
pub async fn summarize_document(app: AppHandle, id: i64) -> Result<String, String> {
    let (enabled, provider, url, model, title, text) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        let (title, text) = fetch_doc_text(&conn, id)?;
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone(), title, text)
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    let text = ai::truncate(&text, 7000);
    if text.trim().is_empty() {
        return Err("Nessun testo disponibile da riassumere per questo documento".into());
    }
    let prompt = format!(
        "Sei un assistente accademico. Riassumi in italiano, in 4-6 frasi chiare e concrete, il seguente articolo. Rispondi solo con il riassunto, senza preamboli.\n\nTitolo: {title}\n\nTesto:\n{text}"
    );
    let client = ai::client().map_err(|e| e.to_string())?;
    let summary = ai::generate(&client, &provider, &url, &model, &prompt, 360)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if summary.is_empty() {
        return Err("Il modello non ha prodotto un riassunto".into());
    }
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.execute("UPDATE documents SET summary = ?1 WHERE id = ?2", params![summary, id])
            .map_err(|e| e.to_string())?;
    }
    Ok(summary)
}

/// Explain / translate / answer a question about a user-selected passage with
/// the local LLM. Streams tokens as `explain-token` events with payload
/// {"token": t, "req": req} — `req` is the caller's correlation id, echoed
/// verbatim (null if absent) so the UI can drop tokens from stale requests.
/// Returns the full text.
#[tauri::command]
pub async fn ai_explain(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    text: String,
    task: String,
    question: Option<String>,
    doc_id: Option<i64>,
    req: Option<String>,
) -> Result<String, String> {
    let (enabled, provider, url, model, doc_title) = {
        let conn = state.db.lock();
        let c = ai_config(&conn);
        // Optional context: the title of the document the passage comes from.
        let doc_title: Option<String> = match doc_id {
            Some(id) => conn
                .query_row("SELECT title FROM documents WHERE id = ?1", params![id], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .optional()
                .map_err(|e| e.to_string())?
                .flatten(),
            None => None,
        };
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone(), doc_title)
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    let text = ai::truncate(&text, 6000);
    if text.trim().is_empty() {
        return Err("Nessun testo selezionato".into());
    }
    let context = doc_title
        .filter(|t| !t.trim().is_empty())
        .map(|t| format!("Dal documento: {t}\n\n"))
        .unwrap_or_default();
    let prompt = match task.as_str() {
        "explain" => format!(
            "{context}Spiega in italiano, in modo chiaro e conciso (massimo 150 parole circa), il seguente passaggio, definendo brevemente i termini tecnici. Rispondi solo con la spiegazione, senza preamboli.\n\nPassaggio:\n{text}"
        ),
        "translate" => format!(
            "{context}Traduci in italiano il seguente passaggio, mantenendo la terminologia tecnica in lingua originale dove appropriato. Rispondi SOLO con la traduzione, senza spiegazioni né preamboli.\n\nPassaggio:\n{text}"
        ),
        "ask" => {
            let q = question.as_deref().map(str::trim).unwrap_or_default();
            if q.is_empty() {
                return Err("Domanda mancante".into());
            }
            format!(
                "{context}Rispondi in italiano alla DOMANDA basandoti sul passaggio qui sotto, in modo chiaro e conciso. Se il passaggio non contiene la risposta, dillo onestamente senza inventare.\n\nDOMANDA: {q}\n\nPassaggio:\n{text}"
            )
        }
        other => return Err(format!("Operazione non supportata: {other}")),
    };
    let client = ai::client().map_err(|e| e.to_string())?;
    let app2 = app.clone();
    let answer = ai::generate_stream(&client, &provider, &url, &model, &prompt, 700, |t| {
        let _ = app2.emit("explain-token", serde_json::json!({ "token": t, "req": req }));
    })
    .await
    .map_err(|e| format!("{e:#}"))?;
    if answer.trim().is_empty() {
        return Err("Il modello non ha prodotto una risposta".into());
    }
    Ok(answer)
}

/// Suggest and assign 3-6 topical tags for a document (manual, opt-in).
#[tauri::command]
pub async fn autotag_document(app: AppHandle, id: i64) -> Result<Vec<String>, String> {
    let (enabled, provider, url, model, title, text) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        let (title, text) = fetch_doc_text(&conn, id)?;
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone(), title, text)
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    let text = ai::truncate(&text, 5000);
    if text.trim().is_empty() {
        return Err("Nessun testo disponibile per generare i tag".into());
    }
    let prompt = format!(
        "Elenca da 3 a 6 parole chiave tematiche per il seguente articolo accademico. Usa termini brevi (1-3 parole), in minuscolo, in inglese. Rispondi SOLO con le parole chiave separate da virgola, senza numerazione né altro.\n\nTitolo: {title}\n\nTesto:\n{text}"
    );
    let client = ai::client().map_err(|e| e.to_string())?;
    let out = ai::generate(&client, &provider, &url, &model, &prompt, 80)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let tags: Vec<String> = out
        .split(|c| c == ',' || c == '\n' || c == ';')
        .map(|s| {
            s.trim()
                .trim_matches(|c: char| c == '-' || c == '•' || c == '*' || c == '.' || c.is_numeric())
                .trim()
                .to_lowercase()
        })
        .filter(|s| !s.is_empty() && s.len() <= 40)
        .take(6)
        .collect();
    if tags.is_empty() {
        return Err("Il modello non ha prodotto tag utilizzabili".into());
    }
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        for t in &tags {
            conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", params![t])
                .map_err(|e| e.to_string())?;
            let tid: i64 = conn
                .query_row("SELECT id FROM tags WHERE name = ?1", params![t], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT OR IGNORE INTO document_tags (document_id, tag_id) VALUES (?1, ?2)",
                params![id, tid],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(tags)
}

// ===== Citations / export =====

fn load_cite_items(conn: &Connection, ids: &[i64]) -> anyhow::Result<Vec<citation::CiteItem>> {
    let mut doc_stmt =
        conn.prepare("SELECT title, year, venue, doi, citekey FROM documents WHERE id = ?1")?;
    let mut auth_stmt = conn.prepare(
        "SELECT given, family FROM authors a
         JOIN document_authors da ON da.author_id = a.id
         WHERE da.document_id = ?1 ORDER BY da.position",
    )?;
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        let row = doc_stmt
            .query_row(params![id], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .optional()?;
        let Some((title, year, venue, doi, citekey)) = row else {
            continue;
        };
        let authors: Vec<(Option<String>, Option<String>)> = auth_stmt
            .query_map(params![id], |r| {
                Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))
            })?
            .filter_map(Result::ok)
            .collect();
        out.push(citation::CiteItem {
            title,
            authors,
            year,
            venue,
            doi,
            citekey,
        });
    }
    Ok(out)
}

/// Citation text for the given documents. `format`: bibtex | ris | csljson | apa | ieee.
#[tauri::command]
pub fn cite_text(state: State<'_, AppState>, ids: Vec<i64>, format: String) -> Result<String, String> {
    let conn = state.db.lock();
    let items = load_cite_items(&conn, &ids).map_err(|e| e.to_string())?;
    Ok(citation::build(&items, &format))
}

/// Write citations for the given documents to a file.
#[tauri::command]
pub fn export_citations(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    format: String,
    path: String,
) -> Result<(), String> {
    let text = {
        let conn = state.db.lock();
        let items = load_cite_items(&conn, &ids).map_err(|e| e.to_string())?;
        citation::build(&items, &format)
    };
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct GapItem {
    pub doi: String,
    pub count: i64,
    pub sample: Option<String>,
}

/// "Library gap-finder": the DOIs your library cites most but doesn't own. Aggregates every
/// stored reference (from Crossref enrichment) whose DOI isn't a document you have, ranked by
/// how many of your papers cite it. Fully offline. `sample` is a representative raw citation
/// string for a human-readable label; the UI can hand the DOI to online discovery to acquire it.
#[tauri::command]
pub fn citation_gaps(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<GapItem>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT LOWER(TRIM(ref_doi)) AS d, COUNT(DISTINCT document_id) AS c, MAX(raw) AS sample
             FROM document_references
             WHERE ref_doi IS NOT NULL AND TRIM(ref_doi) <> ''
               AND LOWER(TRIM(ref_doi)) NOT IN
                   (SELECT LOWER(doi) FROM documents WHERE doi IS NOT NULL AND deleted_at IS NULL)
             GROUP BY d
             ORDER BY c DESC, d
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(GapItem {
                doi: r.get(0)?,
                count: r.get(1)?,
                sample: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
struct RefDoiProgress {
    done: usize,
    total: usize,
    resolved: usize,
}

#[derive(serde::Serialize)]
pub struct BackfillDoiSummary {
    /// Distinct DOI-less citations we looked up online.
    pub scanned: usize,
    /// Citations that got a DOI (precision-gated — never a near-miss).
    pub resolved: usize,
    /// `document_references` rows updated (one citation can be shared by papers).
    pub updated_rows: usize,
    /// DOI-less references (with usable text) still unresolved after the run.
    pub remaining: usize,
}

/// Backfill DOIs onto already-imported references that lack one, so they reach the
/// citation-gap finder. For each distinct DOI-less citation text, a precision-first
/// Crossref bibliographic lookup (gated by `reference_names_title` — the candidate
/// title must be spelled out in the citation, so a wrong DOI is never attached).
/// Online-opt-in gated; runs with the DB unlocked during network I/O; cancellable;
/// emits `refdoi-progress`.
#[tauri::command]
pub async fn resolve_reference_dois(app: AppHandle) -> Result<BackfillDoiSummary, String> {
    // Run on a blocking thread: the inner body uses `block_on` for network I/O, and
    // that must NOT run on the async-runtime worker thread (nested-runtime panic).
    // Mirrors import_latex_zip / generate_embeddings.
    tauri::async_runtime::spawn_blocking(move || resolve_reference_dois_inner(&app))
        .await
        .map_err(|e| format!("Task fallito: {e}"))?
}

fn resolve_reference_dois_inner(app: &AppHandle) -> Result<BackfillDoiSummary, String> {
    let state = app.state::<AppState>();
    state.refdoi_cancel.store(false, std::sync::atomic::Ordering::SeqCst);

    let (online, email) = {
        let conn = state.db.lock();
        let online = setting(&conn, "discovery_enabled").as_deref() == Some("1");
        let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
        (online, email)
    };
    if !online {
        return Err("Abilita prima «Scopri online» nelle Impostazioni.".into());
    }

    // One lookup per DISTINCT citation text, applied to every row that shares it.
    let raws: Vec<String> = {
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT raw FROM document_references
                 WHERE (ref_doi IS NULL OR TRIM(ref_doi) = '')
                   AND raw IS NOT NULL AND TRIM(raw) <> ''",
            )
            .map_err(|e| e.to_string())?;
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .flatten()
            .collect::<Vec<_>>();
        v
    };

    const TOTAL_MAX: usize = 1200; // backstop; the real vault is far smaller
    let target = raws.len().min(TOTAL_MAX); // what we'll actually attempt
    let mut attempted = 0usize;
    let mut resolved_pairs: Vec<(String, String)> = Vec::new(); // (raw, doi)
    if target > 0 {
        if let Ok(client) = metadata::http_client(email.as_deref()) {
            tauri::async_runtime::block_on(async {
                for raw in raws.iter() {
                    if state.refdoi_cancel.load(std::sync::atomic::Ordering::SeqCst) || attempted >= target {
                        break;
                    }
                    attempted += 1;
                    let r = tokio::time::timeout(
                        std::time::Duration::from_secs(20),
                        metadata::crossref_resolve_reference(&client, raw, email.as_deref()),
                    )
                    .await;
                    if let Ok(Ok(Some(doi))) = r {
                        resolved_pairs.push((raw.clone(), doi));
                    }
                    let _ = app.emit(
                        "refdoi-progress",
                        RefDoiProgress { done: attempted, total: target, resolved: resolved_pairs.len() },
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            });
        }
    }

    // Apply: fill in every DOI-less row that shares a resolved citation text.
    let mut updated_rows = 0usize;
    if !resolved_pairs.is_empty() {
        let conn = state.db.lock();
        for (raw, doi) in &resolved_pairs {
            if let Ok(n) = conn.execute(
                "UPDATE document_references SET ref_doi = ?1
                 WHERE raw = ?2 AND (ref_doi IS NULL OR TRIM(ref_doi) = '')",
                params![doi, raw],
            ) {
                updated_rows += n;
            }
        }
    }

    // Distinct citations (matching `scanned`/`resolved`'s unit) still without a DOI.
    let remaining = {
        let conn = state.db.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT raw) FROM document_references
             WHERE (ref_doi IS NULL OR TRIM(ref_doi) = '') AND raw IS NOT NULL AND TRIM(raw) <> ''",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize
    };

    // No `library-changed` emit: only document_references.ref_doi changed — the
    // document set and embeddings (hence the semantic map) are untouched. The gap
    // tab refreshes itself via citationGaps() after the run.
    Ok(BackfillDoiSummary { scanned: attempted, resolved: resolved_pairs.len(), updated_rows, remaining })
}

/// Request cancellation of an in-progress reference-DOI backfill.
#[tauri::command]
pub fn cancel_reference_dois(state: State<'_, AppState>) {
    state.refdoi_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[derive(serde::Serialize)]
pub struct LibraryFacets {
    pub all: i64,
    pub favorite: i64,
    pub unread: i64,
    pub github: i64,
    pub peerreviewed: i64,
    pub own: i64,
}

/// Counts behind the sidebar's library filters, computed over the whole library
/// (independent of the active filter) so each entry can show how many documents
/// it holds. `peerreviewed` mirrors `query_documents`: it's derived from
/// `classify_pub_status`, not a stored column, so it's evaluated per document.
#[tauri::command]
pub fn library_facets(state: State<'_, AppState>) -> Result<LibraryFacets, String> {
    let conn = state.db.lock();
    let mut facets = LibraryFacets { all: 0, favorite: 0, unread: 0, github: 0, peerreviewed: 0, own: 0 };
    let mut stmt = conn
        .prepare(
            "SELECT favorite, is_read, github_url, doi, venue, path, is_own
             FROM documents WHERE deleted_at IS NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (favorite, is_read, github_url, doi, venue, path, is_own) = row.map_err(|e| e.to_string())?;
        facets.all += 1;
        if favorite != 0 {
            facets.favorite += 1;
        }
        if is_read == 0 {
            facets.unread += 1;
        }
        if github_url.is_some() {
            facets.github += 1;
        }
        if is_own != 0 {
            facets.own += 1;
        }
        if discovery::classify_pub_status(doi.as_deref(), venue.as_deref(), path.as_deref()).as_deref()
            == Some("published")
        {
            facets.peerreviewed += 1;
        }
    }
    Ok(facets)
}

// ===== Citation links (references + cited-by within the library) =====

#[derive(serde::Serialize)]
pub struct RefItem {
    raw: Option<String>,
    ref_doi: Option<String>,
    /// Local document id if this reference is in the library.
    in_library: Option<i64>,
    title: Option<String>,
}

#[derive(serde::Serialize)]
pub struct DocBrief {
    id: i64,
    title: Option<String>,
    year: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct CitationLinks {
    references: Vec<RefItem>,
    cited_by: Vec<DocBrief>,
}

/// A document's bibliography (Crossref references) with library cross-links, plus
/// the library documents that cite this one (by DOI).
#[tauri::command]
pub fn citation_links(state: State<'_, AppState>, id: i64) -> Result<CitationLinks, String> {
    let conn = state.db.lock();
    let doi: Option<String> = conn
        .query_row("SELECT doi FROM documents WHERE id = ?1", params![id], |r| {
            r.get::<_, Option<String>>(0)
        })
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    // References of this document.
    let raw_refs: Vec<(Option<String>, Option<String>)> = {
        let mut stmt = conn
            .prepare("SELECT ref_doi, raw FROM document_references WHERE document_id = ?1")
            .map_err(|e| e.to_string())?;
        let v = stmt
            .query_map(params![id], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        v
    };
    let mut references = Vec::with_capacity(raw_refs.len());
    for (ref_doi, raw) in raw_refs {
        let (in_library, title) = match ref_doi.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(d) => conn
                .query_row(
                    "SELECT id, title FROM documents WHERE LOWER(doi) = LOWER(?1) AND deleted_at IS NULL LIMIT 1",
                    params![d],
                    |r| Ok((Some(r.get::<_, i64>(0)?), r.get::<_, Option<String>>(1)?)),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .unwrap_or((None, None)),
            None => (None, None),
        };
        references.push(RefItem { raw, ref_doi, in_library, title });
    }

    // Library documents whose references cite this document's DOI.
    let cited_by: Vec<DocBrief> = match doi.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT d.id, d.title, d.year
                     FROM document_references dr JOIN documents d ON d.id = dr.document_id
                     WHERE LOWER(dr.ref_doi) = LOWER(?1) AND d.deleted_at IS NULL AND d.id != ?2
                     ORDER BY d.year DESC",
                )
                .map_err(|e| e.to_string())?;
            let v = stmt
                .query_map(params![d, id], |r| {
                    Ok(DocBrief { id: r.get(0)?, title: r.get(1)?, year: r.get(2)? })
                })
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
            v
        }
        None => Vec::new(),
    };

    Ok(CitationLinks { references, cited_by })
}

// ===== Obsidian / Markdown vault export =====

#[tauri::command]
pub fn get_obsidian_vault(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock();
    Ok(setting(&conn, "obsidian_vault").unwrap_or_default())
}

#[tauri::command]
pub fn set_obsidian_vault(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('obsidian_vault', ?1)",
        params![path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Gather one document's full data into a renderable note (None if it doesn't exist).
fn load_note_data(conn: &Connection, id: i64) -> anyhow::Result<Option<obsidian::NoteData>> {
    let row = conn
        .query_row(
            "SELECT title, year, venue, doi, abstract, notes, summary, path, added_at, favorite
             FROM documents WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((title, year, venue, doi, abstract_text, notes, summary, path, added, fav)) = row else {
        return Ok(None);
    };

    let mut astmt = conn.prepare(
        "SELECT a.given, a.family FROM authors a
         JOIN document_authors da ON da.author_id = a.id
         WHERE da.document_id = ?1 ORDER BY da.position",
    )?;
    let authors: Vec<String> = astmt
        .query_map(params![id], |r| {
            let g: Option<String> = r.get(0)?;
            let f: Option<String> = r.get(1)?;
            Ok(format!("{} {}", g.unwrap_or_default(), f.unwrap_or_default()).trim().to_string())
        })?
        .filter_map(Result::ok)
        .filter(|s| !s.is_empty())
        .collect();

    let mut tstmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN document_tags dt ON dt.tag_id = t.id
         WHERE dt.document_id = ?1 ORDER BY t.name",
    )?;
    let tags: Vec<String> = tstmt
        .query_map(params![id], |r| r.get::<_, String>(0))?
        .filter_map(Result::ok)
        .collect();

    let mut nstmt = conn.prepare(
        "SELECT page, quote, note FROM annotations WHERE document_id = ?1 ORDER BY page, id",
    )?;
    let annotations: Vec<obsidian::NoteAnnotation> = nstmt
        .query_map(params![id], |r| {
            Ok(obsidian::NoteAnnotation {
                page: r.get(0)?,
                quote: r.get(1)?,
                note: r.get(2)?,
            })
        })?
        .filter_map(Result::ok)
        .collect();

    // A metadata-only reference stores a "ref:source:id" sentinel in `path`.
    let pdf_path = path.filter(|p| !p.starts_with("ref:"));

    Ok(Some(obsidian::NoteData {
        title: title.unwrap_or_else(|| "Senza titolo".to_string()),
        authors,
        year,
        venue,
        doi,
        tags,
        added,
        favorite: fav != 0,
        pdf_path,
        abstract_text,
        summary,
        notes,
        annotations,
    }))
}

/// Export the given documents as Markdown notes into `<vault_dir>/Scriptorium/`.
/// Returns how many notes were written.
#[tauri::command]
pub fn export_obsidian(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    vault_dir: String,
) -> Result<usize, String> {
    let base = PathBuf::from(vault_dir.trim());
    if vault_dir.trim().is_empty() || !base.is_dir() {
        return Err("Cartella del vault non valida".into());
    }
    let out = base.join("Scriptorium");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let conn = state.db.lock();
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut count = 0usize;
    for id in ids {
        let Some(note) = load_note_data(&conn, id).map_err(|e| e.to_string())? else {
            continue;
        };
        // Unique filename per note; disambiguate collisions with the id (looping
        // so even a contrived title can't overwrite another note in this run).
        let base = obsidian::safe_filename(&note.title, id);
        let mut stem = base.clone();
        let mut suffix = 0u32;
        while !used.insert(stem.to_lowercase()) {
            suffix += 1;
            stem = if suffix == 1 {
                format!("{base} ({id})")
            } else {
                format!("{base} ({id}-{suffix})")
            };
        }
        let path = out.join(format!("{stem}.md"));
        // Defense-in-depth: never write outside the export folder.
        if path.parent() != Some(out.as_path()) {
            continue;
        }
        // One unwritable note (e.g. an odd filename) must not abort the whole batch.
        if std::fs::write(&path, obsidian::build_markdown(&note)).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

// ===== Collections =====

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> Result<Vec<Collection>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare("SELECT id, name, is_smart, rule_json FROM collections ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                is_smart: r.get::<_, i64>(2)? != 0,
                rule_json: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(
    state: State<'_, AppState>,
    name: String,
    is_smart: bool,
    rule_json: Option<String>,
) -> Result<Collection, String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT INTO collections (name, is_smart, rule_json) VALUES (?1, ?2, ?3)",
        params![name, is_smart as i64, rule_json],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(Collection {
        id,
        name,
        is_smart,
        rule_json,
    })
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM collections WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn add_to_collection(
    state: State<'_, AppState>,
    collection_id: i64,
    document_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT OR IGNORE INTO collection_members (collection_id, document_id) VALUES (?1, ?2)",
        params![collection_id, document_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn remove_from_collection(
    state: State<'_, AppState>,
    collection_id: i64,
    document_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "DELETE FROM collection_members WHERE collection_id = ?1 AND document_id = ?2",
        params![collection_id, document_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ===== Watched folder =====

#[tauri::command]
pub fn get_watched_folder(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let conn = state.db.lock();
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'watched_folder'",
        [],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_watched_folder(app: AppHandle, path: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let conn = state.db.lock();
        match &path {
            Some(p) => conn
                .execute(
                    "INSERT OR REPLACE INTO settings (key, value) VALUES ('watched_folder', ?1)",
                    params![p],
                )
                .map_err(|e| e.to_string())?,
            None => conn
                .execute("DELETE FROM settings WHERE key = 'watched_folder'", [])
                .map_err(|e| e.to_string())?,
        };
    }
    // Replace the active watcher: drop the old one (stops it), start a new one.
    {
        let mut guard = state.watcher.lock();
        *guard = None;
        if let Some(dir) = &path {
            let w = crate::watch::start(app.clone(), dir).map_err(|e| e.to_string())?;
            *guard = Some(w);
        }
    }
    // Import PDFs already present in the folder (background, deduped).
    if let Some(dir) = &path {
        crate::watch::scan_existing(app.clone(), dir.clone());
    }
    Ok(())
}

// ===== Library hygiene (trash, bulk, duplicates, merge) =====

/// Soft-delete documents (move to Trash).
#[tauri::command]
pub fn delete_documents(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    for id in ids {
        conn.execute(
            "UPDATE documents SET deleted_at = datetime('now') WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Restore documents from the Trash.
#[tauri::command]
pub fn restore_documents(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    for id in ids {
        conn.execute("UPDATE documents SET deleted_at = NULL WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Permanently delete documents (and their vector rows + thumbnail files).
#[tauri::command]
pub fn purge_documents(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    for id in &ids {
        let thumb: Option<String> = conn
            .query_row("SELECT thumb_path FROM documents WHERE id = ?1", params![id], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        if let Some(t) = thumb {
            if !t.is_empty() {
                std::fs::remove_file(&t).ok();
            }
        }
        // doc_vec / chunk_vec are vec0 virtual tables with no FK cascade; remove
        // them explicitly BEFORE deleting the document (chunk_vec must go while the
        // doc_chunks rows still exist to resolve the chunk ids). The rest cascades,
        // and the FTS delete trigger fires on the documents delete.
        conn.execute("DELETE FROM doc_vec WHERE document_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM chunk_vec WHERE chunk_id IN (SELECT id FROM doc_chunks WHERE document_id = ?1)",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// List documents currently in the Trash.
#[tauri::command]
pub fn list_trash(state: State<'_, AppState>) -> Result<Vec<Document>, String> {
    let conn = state.db.lock();
    let ids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM documents WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC")
            .map_err(|e| e.to_string())?;
        let v: Vec<i64> = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        v
    };
    fetch_documents(&conn, &ids, true).map_err(|e| e.to_string())
}

/// Add a single tag to a document (used by bulk tagging).
#[tauri::command]
pub fn add_document_tag(state: State<'_, AppState>, document_id: i64, tag_id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT OR IGNORE INTO document_tags (document_id, tag_id) VALUES (?1, ?2)",
        params![document_id, tag_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Groups of likely-duplicate documents (by DOI, else normalized title+year).
#[tauri::command]
pub fn find_duplicates(state: State<'_, AppState>) -> Result<Vec<Vec<i64>>, String> {
    let conn = state.db.lock();
    let rows: Vec<(i64, Option<String>, Option<String>, Option<i64>)> = {
        let mut stmt = conn
            .prepare("SELECT id, doi, title, year FROM documents WHERE deleted_at IS NULL")
            .map_err(|e| e.to_string())?;
        let v = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        v
    };
    use std::collections::HashMap;
    let mut by_key: HashMap<String, Vec<i64>> = HashMap::new();
    for (id, doi, title, year) in rows {
        let key = if let Some(d) = doi.filter(|s| !s.trim().is_empty()) {
            format!("doi:{}", d.to_lowercase())
        } else if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
            let norm: String = t.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
            if norm.len() < 6 {
                continue;
            }
            format!("ty:{norm}:{}", year.unwrap_or(0))
        } else {
            continue;
        };
        by_key.entry(key).or_default().push(id);
    }
    Ok(by_key.into_values().filter(|g| g.len() > 1).collect())
}

/// Merge `other_ids` into `master_id`: move their tags, collections and
/// annotations to the master, then send the others to Trash.
#[tauri::command]
pub fn merge_documents(state: State<'_, AppState>, master_id: i64, other_ids: Vec<i64>) -> Result<(), String> {
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for oid in &other_ids {
        if *oid == master_id {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO document_tags (document_id, tag_id)
             SELECT ?1, tag_id FROM document_tags WHERE document_id = ?2",
            params![master_id, oid],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT OR IGNORE INTO collection_members (collection_id, document_id)
             SELECT collection_id, ?1 FROM collection_members WHERE document_id = ?2",
            params![master_id, oid],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE annotations SET document_id = ?1 WHERE document_id = ?2",
            params![master_id, oid],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE documents SET deleted_at = datetime('now') WHERE id = ?1",
            params![oid],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// ===== Filter helpers =====

fn load_tags(conn: &Connection, document_id: i64) -> anyhow::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color FROM tags t
         JOIN document_tags dt ON dt.tag_id = t.id
         WHERE dt.document_id = ?1 ORDER BY t.name",
    )?;
    let rows = stmt.query_map(params![document_id], |r| {
        Ok(Tag {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            count: 0, // per-document tag list; the library-wide count isn't needed here
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn ids_with_tag(conn: &Connection, tag_id: i64) -> anyhow::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT document_id FROM document_tags WHERE tag_id = ?1")?;
    let ids: Vec<i64> = stmt
        .query_map(params![tag_id], |r| r.get::<_, i64>(0))?
        .filter_map(Result::ok)
        .collect();
    Ok(ids)
}

fn ids_in_collection(conn: &Connection, collection_id: i64) -> anyhow::Result<Vec<i64>> {
    let coll: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT is_smart, rule_json FROM collections WHERE id = ?1",
            params![collection_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    match coll {
        Some((1, Some(rule))) => eval_smart(conn, &rule),
        Some((1, None)) => Ok(Vec::new()),
        Some(_) => {
            let mut stmt =
                conn.prepare("SELECT document_id FROM collection_members WHERE collection_id = ?1")?;
            let ids: Vec<i64> = stmt
                .query_map(params![collection_id], |r| r.get::<_, i64>(0))?
                .filter_map(Result::ok)
                .collect();
            Ok(ids)
        }
        None => Ok(Vec::new()),
    }
}

/// Evaluate a smart-collection rule to a list of document ids.
/// Supported: {type:"tag",tagId}, {type:"year_gte",value}, {type:"text",query}, {type:"untagged"}.
fn eval_smart(conn: &Connection, rule_json: &str) -> anyhow::Result<Vec<i64>> {
    let v: serde_json::Value = serde_json::from_str(rule_json).unwrap_or(serde_json::Value::Null);
    let kind = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    match kind {
        "tag" => ids_with_tag(conn, v.get("tagId").and_then(|x| x.as_i64()).unwrap_or(-1)),
        "year_gte" => {
            let y = v.get("value").and_then(|x| x.as_i64()).unwrap_or(0);
            let mut stmt = conn.prepare("SELECT id FROM documents WHERE year >= ?1")?;
            let ids: Vec<i64> = stmt
                .query_map(params![y], |r| r.get::<_, i64>(0))?
                .filter_map(Result::ok)
                .collect();
            Ok(ids)
        }
        "untagged" => {
            let mut stmt = conn
                .prepare("SELECT id FROM documents WHERE id NOT IN (SELECT document_id FROM document_tags)")?;
            let ids: Vec<i64> = stmt
                .query_map([], |r| r.get::<_, i64>(0))?
                .filter_map(Result::ok)
                .collect();
            Ok(ids)
        }
        "text" => {
            let fts = fts_query(v.get("query").and_then(|x| x.as_str()).unwrap_or(""));
            if fts.is_empty() {
                return Ok(Vec::new());
            }
            let mut stmt = conn.prepare("SELECT rowid FROM doc_fts WHERE doc_fts MATCH ?1")?;
            let ids: Vec<i64> = stmt
                .query_map(params![fts], |r| r.get::<_, i64>(0))?
                .filter_map(Result::ok)
                .collect();
            Ok(ids)
        }
        _ => Ok(Vec::new()),
    }
}

fn query_documents(
    conn: &Connection,
    tag_id: Option<i64>,
    collection_id: Option<i64>,
    flag: Option<&str>,
) -> anyhow::Result<Vec<Document>> {
    // Resolve an optional allow-list of document ids from the active filter.
    let filter_ids: Option<Vec<i64>> = match (tag_id, collection_id) {
        (Some(tid), _) => Some(ids_with_tag(conn, tid)?),
        (_, Some(cid)) => Some(ids_in_collection(conn, cid)?),
        _ => None,
    };

    let mut conds: Vec<String> = vec!["deleted_at IS NULL".to_string()];
    if let Some(ids) = &filter_ids {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        conds.push(format!(
            "id IN ({})",
            ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
        ));
    }
    match flag {
        Some("favorite") => conds.push("favorite = 1".to_string()),
        Some("unread") => conds.push("is_read = 0".to_string()),
        Some("github") => conds.push("github_url IS NOT NULL".to_string()),
        Some("mywork") => conds.push("is_own = 1".to_string()),
        _ => {}
    }
    let sql = format!(
        "SELECT id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey, last_page, page_count, (summary IS NOT NULL AND TRIM(summary) != ''), is_own
         FROM documents WHERE {} ORDER BY added_at DESC, id DESC",
        conds.join(" AND ")
    );

    let mut stmt = conn.prepare(&sql)?;
    #[allow(clippy::type_complexity)]
    let base: Vec<(i64, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, i64, i64)> =
        stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
                r.get(12)?,
                r.get(13)?,
                r.get(14)?,
                r.get(15)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut author_stmt = conn.prepare(
        "SELECT a.given, a.family
         FROM authors a JOIN document_authors da ON da.author_id = a.id
         WHERE da.document_id = ?1 ORDER BY da.position",
    )?;

    let mut docs = Vec::with_capacity(base.len());
    for (id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey, last_page, page_count, has_summary, is_own) in base {
        let pub_status = discovery::classify_pub_status(doi.as_deref(), venue.as_deref(), path.as_deref());
        let paper_url = paper_link_for(doi.as_deref(), path.as_deref());
        let authors: Vec<String> = author_stmt
            .query_map(params![id], |r| {
                let given: Option<String> = r.get(0)?;
                let family: Option<String> = r.get(1)?;
                Ok(format!(
                    "{} {}",
                    given.unwrap_or_default(),
                    family.unwrap_or_default()
                )
                .trim()
                .to_string())
            })?
            .filter_map(|x| x.ok())
            .filter(|s| !s.is_empty())
            .collect();

        docs.push(Document {
            id,
            title,
            year,
            venue,
            doi,
            authors,
            tags: load_tags(conn, id).unwrap_or_default(),
            has_thumb: thumb_path.map(|t| !t.is_empty()).unwrap_or(false),
            has_file: path.as_deref().map(|p| !p.starts_with("ref:")).unwrap_or(false),
            has_summary: has_summary != 0,
            added_at,
            is_read: is_read != 0,
            favorite: favorite != 0,
            github_url,
            pub_status,
            paper_url,
            citekey,
            last_page,
            page_count,
            is_own: is_own != 0,
        });
    }
    // pub_status is computed, not a column — filter the peer-reviewed view here.
    if flag == Some("peerreviewed") {
        docs.retain(|d| d.pub_status.as_deref() == Some("published"));
    }
    Ok(docs)
}

// ===== Share / desktop integration (open app, clipboard, reveal) =====

/// The on-disk PDF path for a document, but only if it has a real file present
/// (reference-only entries use a synthetic `ref:` path and have no file).
fn resolve_existing_path(conn: &Connection, id: i64) -> Result<Option<String>, String> {
    let path: Option<String> = conn
        .query_row("SELECT path FROM documents WHERE id = ?1", params![id], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(path.filter(|p| !p.starts_with("ref:") && Path::new(p).is_file()))
}

/// Single-quote-escape a string for embedding inside a PowerShell '...' literal.
fn ps_lit(s: &str) -> String {
    s.replace('\'', "''")
}

/// Run a PowerShell script headlessly (no console window). Encoded as UTF-16LE
/// base64 so quoting/special characters in paths and URLs can't break it.
#[cfg(windows)]
fn run_powershell(script: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = BASE64_STANDARD.encode(&utf16);
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-STA", "-EncodedCommand", &encoded])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .status()
        .map_err(|e| format!("PowerShell non disponibile: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Operazione non riuscita".into())
    }
}

#[cfg(not(windows))]
fn run_powershell(_script: &str) -> Result<(), String> {
    Err("Funzione disponibile solo su Windows".into())
}

/// The on-disk PDF path for a document, or `None` for reference-only entries.
#[tauri::command]
pub fn document_path(state: State<'_, AppState>, id: i64) -> Result<Option<String>, String> {
    let conn = state.db.lock();
    resolve_existing_path(&conn, id)
}

/// Copy the given documents' PDF files onto the clipboard as a file drop list
/// (CF_HDROP) so they can be pasted (Ctrl+V) into chat/email apps. Returns how
/// many files were actually placed on the clipboard (ref-only entries skipped).
#[tauri::command]
pub fn copy_pdfs_to_clipboard(state: State<'_, AppState>, ids: Vec<i64>) -> Result<usize, String> {
    let paths: Vec<String> = {
        let conn = state.db.lock();
        let mut out = Vec::new();
        for id in ids {
            if let Some(p) = resolve_existing_path(&conn, id)? {
                out.push(p);
            }
        }
        out
    };
    if paths.is_empty() {
        return Ok(0);
    }
    let adds: String = paths
        .iter()
        .map(|p| format!("$c.Add('{}') | Out-Null;", ps_lit(p)))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $c = New-Object System.Collections.Specialized.StringCollection; {adds} \
         [System.Windows.Forms.Clipboard]::SetFileDropList($c)"
    );
    run_powershell(&script)?;
    Ok(paths.len())
}

/// Open a URL / web compose link with the OS default handler. Restricted to a
/// small set of known-safe schemes.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let allowed = ["http://", "https://", "mailto:", "whatsapp:", "msteams:"];
    if !allowed.iter().any(|s| url.starts_with(s)) {
        return Err("Schema URL non consentito".into());
    }
    // For web links (which may come from untrusted search results / READMEs),
    // refuse internal/LAN targets so a malicious link can't make the browser
    // hit localhost or a private host with the user's cookies (SSRF / LAN-CSRF).
    if let Some(rest) = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")) {
        let authority = rest.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
        let host = host_of(authority);
        let host = host.trim_end_matches('.');
        if host.is_empty() || host == "localhost" || host.ends_with(".localhost") {
            return Err("Indirizzo locale non consentito".into());
        }
        if host.starts_with("0x")
            || (!host.chars().any(|c| c.is_ascii_alphabetic()) && host.parse::<std::net::IpAddr>().is_err())
        {
            return Err("Indirizzo non valido".into());
        }
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if !ip_is_public(ip) {
                return Err("Indirizzo locale non consentito".into());
            }
        }
    }
    run_powershell(&format!("Start-Process '{}'", ps_lit(&url)))
}

/// Reveal a document's PDF in the system file explorer (file selected).
#[tauri::command]
pub fn reveal_pdf(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let path = {
        let conn = state.db.lock();
        resolve_existing_path(&conn, id)?
    };
    let path = path.ok_or_else(|| "Questo elemento non ha un file PDF".to_string())?;
    run_powershell(&format!(
        "Start-Process explorer.exe -ArgumentList '/select,\"{}\"'",
        ps_lit(&path)
    ))
}

/// Open a new Outlook (desktop) email with the given document attached.
/// Errors if Outlook desktop is not installed, so the caller can fall back to webmail.
#[tauri::command]
pub fn share_via_outlook(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let path = {
        let conn = state.db.lock();
        resolve_existing_path(&conn, id)?
    };
    let path = path.ok_or_else(|| "Questo elemento non ha un file PDF".to_string())?;
    let script = format!(
        "$keys = @(\
           'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\OUTLOOK.EXE',\
           'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\OUTLOOK.EXE',\
           'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths\\OUTLOOK.EXE'); \
         $o = $null; \
         foreach ($k in $keys) {{ $v = (Get-ItemProperty -Path $k -ErrorAction SilentlyContinue).'(default)'; if ($v) {{ $o = $v; break }} }} \
         if (-not $o) {{ throw 'Outlook non installato' }} \
         Start-Process -FilePath $o -ArgumentList '/a','\"{}\"'",
        ps_lit(&path)
    );
    run_powershell(&script)
}

// ===== Standalone Markdown notes (a vault of real .md files on disk) =====
//
// Notes live as `<slug>.md` files under app_data_dir/notes/ — the files ARE the
// source of truth (no DB table), so they stay editable from a terminal / any
// editor. Titles are derived from the first line; [[wikilinks]] to other notes
// (by slug or title) and to papers (`[[@citekey]]` / `[[title]]`) are woven at
// read time. See `crate::notes` for the pure helpers.

fn notes_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|d| d.join("notes"))
        .unwrap_or_else(|_| std::env::temp_dir().join("scriptorium-notes"))
}

/// Write a note atomically: temp file + rename, so an interrupted write (disk
/// full, crash) can never leave the source-of-truth .md truncated on disk.
fn write_note_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("Salvataggio nota: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Salvataggio nota: {e}")
    })
}

/// Resolve a note slug to its file path, rejecting anything that isn't a plain
/// single filename component (path-traversal guard).
fn note_path(dir: &Path, slug: &str) -> Result<PathBuf, String> {
    // Reject path separators, Windows drive/ADS colons and `..` traversal. Plain
    // dots are allowed so a hand-made `my.note.md` still opens.
    if slug.is_empty() || slug.contains(['/', '\\', ':']) || slug.contains("..") {
        return Err("Nome nota non valido".into());
    }
    Ok(dir.join(format!("{slug}.md")))
}

#[derive(serde::Serialize)]
pub struct NoteMeta {
    pub slug: String,
    pub title: String,
    pub excerpt: String,
    /// File creation time as epoch milliseconds (formatted by the frontend).
    pub created_at: Option<i64>,
    /// File mtime as epoch milliseconds (formatted by the frontend).
    pub updated_at: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct NoteLink {
    pub slug: String,
    pub title: String,
}

#[derive(serde::Serialize)]
pub struct NoteView {
    pub slug: String,
    pub title: String,
    pub content_md: String,
    pub html: String,
    /// Other notes that link to this one.
    pub backlinks: Vec<NoteLink>,
    /// Absolute path of the .md file on disk.
    pub path: String,
    /// File creation / last-modified time as epoch milliseconds.
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

fn systime_ms(t: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    t.ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

/// Read every `.md` file in the vault into (slug, raw content, created-millis, mtime-millis).
fn read_vault(dir: &Path) -> Vec<(String, String, Option<i64>, Option<i64>)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        let is_md = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|s| s.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }
        let Some(slug) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (created, mtime) = match e.metadata() {
            Ok(m) => (systime_ms(m.created()), systime_ms(m.modified())),
            Err(_) => (None, None),
        };
        out.push((slug, content, created, mtime));
    }
    out
}

/// Upsert a note's searchable text into the `notes` shadow index (FTS follows via
/// triggers). Best-effort — indexing must never block the .md file write.
/// Replace inline base64 image data-URIs with a short marker so the FTS index
/// stores captions and prose — not the hundreds of KB of base64 an embedded figure
/// carries (which would bloat the DB and pollute search with non-word tokens).
fn strip_data_uris(body: &str) -> String {
    let needle = "data:image/";
    if !body.contains(needle) {
        return body.to_string();
    }
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos]);
        out.push_str("data:image");
        // Drop the payload up to the markdown image's `)` or the next whitespace/quote.
        let after = &rest[pos + needle.len()..];
        let end = after
            .find(|c: char| c == ')' || c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn index_note(conn: &Connection, slug: &str, body: &str, updated_at: Option<i64>) {
    let title = notes::note_title(body);
    let indexed = strip_data_uris(body);
    let _ = conn.execute(
        "INSERT INTO notes (slug, title, body, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(slug) DO UPDATE SET title = excluded.title, body = excluded.body, updated_at = excluded.updated_at",
        params![slug, title, indexed, updated_at],
    );
}

fn unindex_note(conn: &Connection, slug: &str) {
    let _ = conn.execute("DELETE FROM notes WHERE slug = ?1", params![slug]);
}

/// Rebuild the notes search index from the vault: upsert every `.md` file and drop
/// index rows whose file no longer exists. Called on startup so edits/deletes made
/// outside the app (the files are the source of truth) are reflected in search.
pub fn reindex_notes(app: &AppHandle) {
    let dir = notes_dir(app);
    // Enumerate the vault ONCE. If the directory can't be read (transient IO
    // error, unmounted path, AV handle lock — or it simply doesn't exist yet),
    // do nothing this launch: an unreadable dir must never be mistaken for an
    // empty one and trigger a prune of the whole index. The .md files are the
    // source of truth, but a failed read is not evidence they're gone.
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let state = app.state::<AppState>();
    let conn = state.db.lock();
    let mut on_disk: std::collections::HashSet<String> = std::collections::HashSet::new();
    for e in entries.flatten() {
        let path = e.path();
        let is_md = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|s| s.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }
        let Some(slug) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
            continue;
        };
        // A present file — even one we momentarily can't read — stays "on disk"
        // so it is never pruned; we only refresh its indexed content when the
        // read succeeds.
        on_disk.insert(slug.clone());
        if let Ok(content) = std::fs::read_to_string(&path) {
            let mtime = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);
            index_note(&conn, &slug, &content, mtime);
        }
    }
    // Drop index rows whose .md file no longer exists (dir was read cleanly).
    let indexed: Vec<String> = match conn.prepare("SELECT slug FROM notes") {
        Ok(mut stmt) => stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    for slug in indexed {
        if !on_disk.contains(&slug) {
            unindex_note(&conn, &slug);
        }
    }
}

/// A note that matched a full-text search.
#[derive(serde::Serialize)]
pub struct NoteHit {
    pub slug: String,
    pub title: String,
    pub snippet: String,
}

/// Full-text search over the standalone notes vault (title + body).
#[tauri::command]
pub fn search_notes(app: AppHandle, query: String) -> Result<Vec<NoteHit>, String> {
    let expr = fts_query(&query);
    if expr.trim().is_empty() {
        return Ok(Vec::new());
    }
    let state = app.state::<AppState>();
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT n.slug, n.title, snippet(notes_fts, -1, '', '', '…', 12)
             FROM notes_fts JOIN notes n ON n.id = notes_fts.rowid
             WHERE notes_fts MATCH ?1 ORDER BY rank LIMIT 20",
        )
        .map_err(|e| e.to_string())?;
    let hits: Vec<NoteHit> = stmt
        .query_map(params![expr], |r| {
            Ok(NoteHit { slug: r.get(0)?, title: r.get(1)?, snippet: r.get(2)? })
        })
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    Ok(hits)
}

/// List all notes (metadata only), newest first.
#[tauri::command]
pub fn list_notes(app: AppHandle) -> Result<Vec<NoteMeta>, String> {
    let dir = notes_dir(&app);
    let mut metas: Vec<NoteMeta> = read_vault(&dir)
        .into_iter()
        .map(|(slug, content, created, mtime)| NoteMeta {
            slug,
            title: notes::note_title(&content),
            excerpt: notes::note_excerpt(&content),
            created_at: created,
            updated_at: mtime,
        })
        .collect();
    // A sensible default order (newest edit first); the frontend re-sorts on demand.
    metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    Ok(metas)
}

fn doc_by_citekey(conn: &Connection, key: &str) -> Option<(i64, Option<String>)> {
    conn.query_row(
        "SELECT id, title FROM documents WHERE deleted_at IS NULL AND citekey = ?1 COLLATE NOCASE LIMIT 1",
        params![key],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .ok()
    .flatten()
}

fn doc_by_title(conn: &Connection, title: &str) -> Option<(i64, Option<String>)> {
    conn.query_row(
        "SELECT id, title FROM documents WHERE deleted_at IS NULL AND title = ?1 COLLATE NOCASE LIMIT 1",
        params![title],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .ok()
    .flatten()
}

/// Load one note: raw body (for editing) + rendered HTML with woven links + backlinks.
#[tauri::command]
pub fn get_note(app: AppHandle, slug: String) -> Result<NoteView, String> {
    let dir = notes_dir(&app);
    let path = note_path(&dir, &slug)?;
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Lettura nota: {e}"))?;
    // One-time migration on open: a legacy note with embedded base64 images gets
    // them extracted to assets/ so the editor shows short references, not a blob
    // wall. Atomic write (temp+rename: an interrupted write can't truncate the
    // note); keeps the original on failure (the data-URI still renders). The FTS
    // row follows so search stays in sync with the rewritten body.
    let migrated = notes::extract_data_images(&content, &dir);
    let content = if migrated != content && write_note_atomic(&path, &migrated).is_ok() {
        let mtime = std::fs::metadata(&path).ok().and_then(|m| systime_ms(m.modified()));
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock();
            index_note(&conn, &slug, &migrated, mtime);
        }
        migrated
    } else {
        content
    };
    let vault = read_vault(&dir);

    // Lookup for note targets: slug (lower) and title (lower) → (slug, title).
    let mut note_by_key: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
    for (s, c, _, _) in &vault {
        let title = notes::note_title(c);
        note_by_key.entry(s.to_lowercase()).or_insert_with(|| (s.clone(), title.clone()));
        note_by_key.entry(title.to_lowercase()).or_insert_with(|| (s.clone(), title.clone()));
    }

    let state = app.state::<AppState>();
    let woven = {
        let conn = state.db.lock();
        notes::weave_note_links(&content, |target| {
            let key = target.to_lowercase();
            if let Some((s, t)) = note_by_key.get(&key) {
                return Some((t.clone(), format!("#note-{s}")));
            }
            if let Some(rest) = target.strip_prefix('@') {
                if let Some((id, title)) = doc_by_citekey(&conn, rest.trim()) {
                    return Some((title.unwrap_or_else(|| target.to_string()), format!("#doc-{id}")));
                }
            }
            if let Some((id, title)) = doc_by_title(&conn, target) {
                return Some((title.unwrap_or_else(|| target.to_string()), format!("#doc-{id}")));
            }
            None
        })
    };
    // Inline assets/ image references back into data-URIs so the preview renders
    // them (the webview can't resolve vault-relative paths). A missing image just
    // shows broken in the preview — only exports hard-fail on it.
    let html = wiki::render_html_live(&notes::inline_assets(&woven, &dir).0);

    // Backlinks: other notes whose body references this note's slug or title.
    let this_title = notes::note_title(&content).to_lowercase();
    let slug_lower = slug.to_lowercase();
    let mut backlinks: Vec<NoteLink> = Vec::new();
    for (s, c, _, _) in &vault {
        if *s == slug {
            continue;
        }
        if notes::link_targets(c).iter().any(|t| t == &slug_lower || t == &this_title) {
            backlinks.push(NoteLink { slug: s.clone(), title: notes::note_title(c) });
        }
    }
    backlinks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    let (created_at, updated_at) = match std::fs::metadata(&path) {
        Ok(m) => (systime_ms(m.created()), systime_ms(m.modified())),
        Err(_) => (None, None),
    };
    Ok(NoteView {
        title: notes::note_title(&content),
        slug,
        content_md: content,
        html,
        backlinks,
        path: path.to_string_lossy().into_owned(),
        created_at,
        updated_at,
    })
}

/// Export a note to a self-contained HTML file, or a LaTeX document (`.tex` plus a
/// sibling `<name>_figures/` folder of the extracted images). `format` is "html" or
/// "latex". Math is preserved (MathML in HTML, native LaTeX in the .tex).
#[tauri::command]
pub fn export_note(app: AppHandle, slug: String, format: String, path: String) -> Result<(), String> {
    let dir = notes_dir(&app);
    let note_p = note_path(&dir, &slug)?;
    let content = std::fs::read_to_string(&note_p).map_err(|e| format!("Lettura nota: {e}"))?;
    // Inline assets/ references first: to_html embeds them, to_latex decodes the
    // data-URIs back into the export's <stem>_figures/ folder. A figure that
    // can't be included fails the export with its name, instead of shipping a
    // silently broken document.
    let (content, skipped) = notes::inline_assets(&content, &dir);
    if !skipped.is_empty() {
        return Err(format!(
            "Immagini non trovate o troppo grandi per l'export: {}",
            skipped.join(", ")
        ));
    }
    let title = notes::note_title(&content);
    let out = std::path::Path::new(&path);
    match format.as_str() {
        "html" => {
            std::fs::write(out, mdexport::to_html(&content, &title)).map_err(|e| e.to_string())
        }
        "latex" | "tex" => {
            let stem = out.file_stem().and_then(|s| s.to_str()).unwrap_or("nota");
            // Sanitize: the dir name is interpolated raw into \includegraphics, so keep
            // only chars that are safe there (%, #, {, } would break compilation).
            let safe: String = stem
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect();
            let safe = if safe.is_empty() { "nota".to_string() } else { safe };
            let fig_dir_name = format!("{safe}_figures");
            let doc = mdexport::to_latex(&content, &fig_dir_name);
            std::fs::write(out, &doc.tex).map_err(|e| e.to_string())?;
            if !doc.images.is_empty() {
                let parent = out.parent().unwrap_or_else(|| std::path::Path::new("."));
                let fig_dir = parent.join(&fig_dir_name);
                std::fs::create_dir_all(&fig_dir).map_err(|e| e.to_string())?;
                for img in &doc.images {
                    std::fs::write(fig_dir.join(&img.filename), &img.bytes).map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }
        other => Err(format!("Formato di esportazione non supportato: {other}")),
    }
}

/// Render draft Markdown to sanitized HTML for the live editor preview (math as KaTeX
/// placeholders the webview fills in, images inline). No DB/weaving — `[[links]]`
/// resolve on save/full preview.
#[tauri::command]
pub fn preview_markdown(app: AppHandle, md: String) -> String {
    // The draft references images as assets/… — inline them so they render live.
    wiki::render_html_live(&notes::inline_assets(&md, &notes_dir(&app)).0)
}

/// The note as a self-contained HTML document (used to print it to PDF).
#[tauri::command]
pub fn note_export_html(app: AppHandle, slug: String) -> Result<String, String> {
    let dir = notes_dir(&app);
    let note_p = note_path(&dir, &slug)?;
    let content = std::fs::read_to_string(&note_p).map_err(|e| format!("Lettura nota: {e}"))?;
    let title = notes::note_title(&content);
    // Self-contained: assets/ image references become data-URIs. A figure that
    // can't be included is an error, not a silently broken PDF.
    let (inlined, skipped) = notes::inline_assets(&content, &dir);
    if !skipped.is_empty() {
        return Err(format!(
            "Immagini non trovate o troppo grandi per l'export: {}",
            skipped.join(", ")
        ));
    }
    Ok(mdexport::to_html(&inlined, &title))
}

/// Rename a note: set a new title (rewriting the body's title line) and rename
/// the .md file to a slug derived from it. Returns the new slug.
#[tauri::command]
pub fn rename_note(app: AppHandle, slug: String, new_title: String) -> Result<String, String> {
    let dir = notes_dir(&app);
    let old_path = note_path(&dir, &slug)?;
    let title = new_title.trim();
    if title.is_empty() {
        return Err("Il titolo non può essere vuoto".into());
    }
    let content = std::fs::read_to_string(&old_path).map_err(|e| format!("Lettura nota: {e}"))?;
    let new_content = notes::set_title_line(&content, title);

    // A unique new slug. Skip disambiguation against the note's OWN file: on a
    // case-insensitive filesystem an all-lowercase slug would otherwise "collide"
    // with the note's own uppercase/accented filename and mint a spurious "-2".
    let base = wiki::slugify(title);
    let mut new_slug = base.clone();
    let mut n = 2;
    while new_slug != slug
        && !new_slug.eq_ignore_ascii_case(&slug)
        && dir.join(format!("{new_slug}.md")).exists()
    {
        new_slug = format!("{base}-{n}");
        n += 1;
    }
    let new_path = dir.join(format!("{new_slug}.md"));
    // Move the file first (a rename preserves the OS creation timestamp, so the
    // "creata" date survives), then rewrite its body. If the write fails the note
    // still exists intact at the new name — nothing is lost.
    if new_slug != slug {
        std::fs::rename(&old_path, &new_path).map_err(|e| format!("Rinomina file: {e}"))?;
    }
    write_note_atomic(&new_path, &new_content)?;
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        if new_slug != slug {
            unindex_note(&conn, &slug);
        }
        index_note(&conn, &new_slug, &new_content, None);
    }
    Ok(new_slug)
}

/// Create a new note from a title; returns its slug. Body seeded with `# title`.
#[tauri::command]
pub fn create_note(app: AppHandle, title: String) -> Result<String, String> {
    let dir = notes_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Creazione cartella note: {e}"))?;
    let title = title.trim();
    let base = wiki::slugify(title); // "pagina" fallback for an empty/symbol title
    let mut slug = base.clone();
    let mut n = 2;
    while dir.join(format!("{slug}.md")).exists() {
        slug = format!("{base}-{n}");
        n += 1;
    }
    let body = if title.is_empty() {
        "# Nuova nota\n\n".to_string()
    } else {
        format!("# {title}\n\n")
    };
    std::fs::write(dir.join(format!("{slug}.md")), &body).map_err(|e| format!("Creazione nota: {e}"))?;
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        index_note(&conn, &slug, &body, None);
    }
    Ok(slug)
}

/// Overwrite a note's body; returns refreshed metadata for the list.
#[tauri::command]
pub fn save_note(app: AppHandle, slug: String, content_md: String) -> Result<NoteMeta, String> {
    let dir = notes_dir(&app);
    let _ = std::fs::create_dir_all(&dir);
    let path = note_path(&dir, &slug)?;
    // Any embedded base64 image becomes a real file in assets/ + a short reference
    // (keeps the .md readable; re-inlined at render/export time).
    let content_md = notes::extract_data_images(&content_md, &dir);
    write_note_atomic(&path, &content_md)?;
    let m = std::fs::metadata(&path).ok();
    let created = m.as_ref().and_then(|m| systime_ms(m.created()));
    let mtime = m.as_ref().and_then(|m| systime_ms(m.modified()));
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        index_note(&conn, &slug, &content_md, mtime);
    }
    Ok(NoteMeta {
        slug,
        title: notes::note_title(&content_md),
        excerpt: notes::note_excerpt(&content_md),
        created_at: created,
        updated_at: mtime,
    })
}

/// Append a Markdown block to an existing note (used by "send selection/abstract/
/// summary to a note"). The file stays the source of truth: read → concat with a
/// blank-line separator → write → re-index. Never overwrites existing content.
#[tauri::command]
pub fn append_to_note(app: AppHandle, slug: String, markdown: String) -> Result<NoteMeta, String> {
    let dir = notes_dir(&app);
    let path = note_path(&dir, &slug)?;
    let existing = std::fs::read_to_string(&path).map_err(|e| format!("Lettura nota: {e}"))?;
    let block = markdown.trim_end_matches(['\n', '\r']);
    let new_content = if existing.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", existing.trim_end_matches(['\n', '\r']))
    };
    // Extracted figures sent "→ Appunti" arrive as base64 — store them in assets/.
    let new_content = notes::extract_data_images(&new_content, &dir);
    write_note_atomic(&path, &new_content)?;
    let m = std::fs::metadata(&path).ok();
    let created = m.as_ref().and_then(|m| systime_ms(m.created()));
    let mtime = m.as_ref().and_then(|m| systime_ms(m.modified()));
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        index_note(&conn, &slug, &new_content, mtime);
    }
    Ok(NoteMeta {
        slug,
        title: notes::note_title(&new_content),
        excerpt: notes::note_excerpt(&new_content),
        created_at: created,
        updated_at: mtime,
    })
}

#[tauri::command]
pub fn delete_note(app: AppHandle, slug: String) -> Result<(), String> {
    let dir = notes_dir(&app);
    let path = note_path(&dir, &slug)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Eliminazione nota: {e}"))?;
    }
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        unindex_note(&conn, &slug);
        // Its vector (if the note was embedded) is now an orphan: sweep.
        let _ = conn.execute(
            "DELETE FROM note_vec WHERE note_id NOT IN (SELECT id FROM notes)",
            [],
        );
    }
    Ok(())
}

/// Open the notes vault folder in the system file explorer.
#[tauri::command]
pub fn reveal_notes_dir(app: AppHandle) -> Result<(), String> {
    let dir = notes_dir(&app);
    let _ = std::fs::create_dir_all(&dir);
    run_powershell(&format!(
        "Start-Process explorer.exe -ArgumentList '\"{}\"'",
        ps_lit(&dir.to_string_lossy())
    ))
}

// ===== Local AI servers: start / stop =====

/// PowerShell snippet that resolves the LM Studio `lms` CLI (PATH or the default
/// install location) into `$lms`, throwing a helpful error if it's missing.
const LMS_RESOLVE: &str = "$lms = (Get-Command lms -ErrorAction SilentlyContinue).Source; \
     if (-not $lms) { $p = Join-Path $env:USERPROFILE '.lmstudio\\bin\\lms.exe'; if (Test-Path $p) { $lms = $p } } \
     if (-not $lms) { $p = Join-Path $env:USERPROFILE '.cache\\lm-studio\\bin\\lms.exe'; if (Test-Path $p) { $lms = $p } } \
     if (-not $lms) { throw 'CLI di LM Studio (lms) non trovata: avvia LM Studio una volta, oppure esegui ''lms bootstrap''' } ";

/// Start the local server for the given provider.
/// Ollama: `ollama serve` (hidden). LM Studio: `lms server start`.
#[tauri::command]
pub fn ai_server_start(provider: String) -> Result<(), String> {
    if ai::is_lmstudio(&provider) {
        // `; exit 0` keeps a genuine "lms not found" error (LMS_RESOLVE throws first)
        // but tolerates the benign "already running" non-zero exit.
        run_powershell(&format!("{LMS_RESOLVE} & $lms server start; exit 0"))
    } else {
        run_powershell(
            "if (-not (Get-Command ollama -ErrorAction SilentlyContinue)) { throw 'Ollama non trovato nel PATH (installalo da ollama.com)' } \
             Start-Process -WindowStyle Hidden -FilePath ollama -ArgumentList 'serve'",
        )
    }
}

/// Stop the local server for the given provider.
/// Ollama: terminate the server/tray processes (best-effort). LM Studio: `lms server stop`.
#[tauri::command]
pub fn ai_server_stop(provider: String) -> Result<(), String> {
    if ai::is_lmstudio(&provider) {
        // Best-effort: `; exit 0` tolerates "server not running" (lms exits non-zero),
        // while LMS_RESOLVE still throws a real error if the CLI is missing.
        run_powershell(&format!("{LMS_RESOLVE} & $lms server stop; exit 0"))
    } else {
        // Best-effort kill; never fail just because nothing was running. `/T` kills
        // the model-runner child process too — without it the runner is orphaned and
        // keeps the model resident in VRAM (the GPU memory would never be freed).
        // A runner ALREADY orphaned (its ollama.exe died without /T) has image name
        // `llama-server.exe` (verified on Ollama 0.30) and no matching parent, so the
        // taskkill lines miss it: sweep it by executable PATH — only processes under
        // an Ollama install dir, never a user's own llama.cpp server.
        run_powershell(
            "taskkill /F /T /IM 'ollama app.exe' 2>$null; taskkill /F /T /IM ollama.exe 2>$null; \
             Get-Process llama-server -ErrorAction SilentlyContinue | \
             Where-Object { $_.Path -like '*\\Ollama\\*' } | \
             Stop-Process -Force -ErrorAction SilentlyContinue; exit 0",
        )
    }
}

// ===== Embedded terminal (PTY) =====

/// Open a shell in a PTY for the in-app terminal. Starts in the watched folder
/// if one is configured, otherwise the user's home directory.
#[derive(serde::Serialize)]
pub struct TermOpened {
    epoch: u64,
    cwd: String,
}

#[tauri::command]
pub fn term_open(
    app: AppHandle,
    state: State<'_, term::TermState>,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
) -> Result<TermOpened, String> {
    // Explicit folder (from "Cambia cartella") wins and is remembered; otherwise
    // fall back to the last terminal folder → watched folder → home directory.
    let chosen = cwd.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let cwd = {
        let app_state = app.state::<AppState>();
        let conn = app_state.db.lock();
        if let Some(c) = chosen {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('terminal_cwd', ?1)",
                params![c],
            );
            c.to_string()
        } else {
            setting(&conn, "terminal_cwd")
                .filter(|p| !p.trim().is_empty())
                .or_else(|| setting(&conn, "watched_folder").filter(|p| !p.trim().is_empty()))
                .or_else(|| std::env::var("USERPROFILE").ok())
                .unwrap_or_default()
        }
    };
    let epoch = term::open(&app, state.inner(), "powershell.exe", &cwd, cols, rows)?;
    Ok(TermOpened { epoch, cwd })
}

#[tauri::command]
pub fn term_write(state: State<'_, term::TermState>, data: String) -> Result<(), String> {
    term::write(state.inner(), &data)
}

#[tauri::command]
pub fn term_resize(state: State<'_, term::TermState>, cols: u16, rows: u16) -> Result<(), String> {
    term::resize(state.inner(), cols, rows)
}

#[tauri::command]
pub fn term_close(state: State<'_, term::TermState>) -> Result<(), String> {
    term::close(state.inner());
    Ok(())
}

// ===== Wiki della libreria: pagine concettuali generate dall'LLM locale =====

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct WikiClaim {
    pub text: String,
    pub page: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct WikiSource {
    pub n: usize,
    pub document_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub claims: Vec<WikiClaim>,
    /// False when the final page does not cite this source (or it had no text).
    pub used: bool,
}

#[derive(serde::Serialize)]
pub struct WikiPageMeta {
    pub slug: String,
    pub concept: String,
    pub title: String,
    pub generated_at: Option<String>,
    pub model: Option<String>,
    pub n_sources: i64,
    /// True when the page's concept matches a tag whose membership changed
    /// since generation (the page is worth regenerating).
    pub stale: bool,
}

#[derive(serde::Serialize)]
pub struct WikiPage {
    pub slug: String,
    pub concept: String,
    pub title: String,
    pub html: String,
    pub sources: Vec<WikiSource>,
    pub generated_at: Option<String>,
    pub model: Option<String>,
}

/// Current member ids (non-deleted) of the tag named like `concept`, if any.
fn tag_member_ids(conn: &Connection, concept: &str) -> Option<Vec<i64>> {
    let tag_id: i64 = conn
        .query_row("SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE", params![concept], |r| r.get(0))
        .optional()
        .ok()??;
    let mut s = conn
        .prepare(
            "SELECT dt.document_id FROM document_tags dt JOIN documents d ON d.id = dt.document_id
             WHERE dt.tag_id = ?1 AND d.deleted_at IS NULL ORDER BY dt.document_id",
        )
        .ok()?;
    let v = s
        .query_map(params![tag_id], |r| r.get::<_, i64>(0))
        .ok()?
        .filter_map(Result::ok)
        .collect();
    Some(v)
}

#[tauri::command]
pub fn wiki_list(state: State<'_, AppState>) -> Result<Vec<WikiPageMeta>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT slug, concept, title, generated_at, model, sources_json, doc_ids
             FROM wiki_pages ORDER BY title COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String, Option<String>, Option<String>, String, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);
    let mut out = Vec::with_capacity(rows.len());
    for (slug, concept, title, generated_at, model, sources_json, doc_ids) in rows {
        let n_sources = serde_json::from_str::<Vec<WikiSource>>(&sources_json)
            .map(|v| v.len() as i64)
            .unwrap_or(0);
        let stale = tag_member_ids(&conn, &concept)
            .map(|cur| {
                cur.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",") != doc_ids
            })
            .unwrap_or(false);
        out.push(WikiPageMeta { slug, concept, title, generated_at, model, n_sources, stale });
    }
    Ok(out)
}

#[tauri::command]
pub fn wiki_get(state: State<'_, AppState>, slug: String) -> Result<WikiPage, String> {
    let conn = state.db.lock();
    let row = conn
        .query_row(
            "SELECT concept, title, content_md, sources_json, generated_at, model
             FROM wiki_pages WHERE slug = ?1",
            params![slug],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((concept, title, content_md, sources_json, generated_at, model)) = row else {
        return Err("Pagina non trovata".into());
    };
    // Cross-links reflect the pages that exist NOW (raw markdown is stored).
    let others: Vec<(String, String)> = {
        let mut s = conn
            .prepare("SELECT concept, slug FROM wiki_pages WHERE slug != ?1")
            .map_err(|e| e.to_string())?;
        let v: Vec<(String, String)> = s
            .query_map(params![slug], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        v
    };
    let md = wiki::link_citations(&content_md);
    let md = wiki::weave_links(&md, &others);
    let html = wiki::render_html_live(&md);
    let sources: Vec<WikiSource> = serde_json::from_str(&sources_json).unwrap_or_default();
    Ok(WikiPage { slug, concept, title, html, sources, generated_at, model })
}

#[tauri::command]
pub fn wiki_delete(state: State<'_, AppState>, slug: String) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM wiki_pages WHERE slug = ?1", params![slug])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn wiki_cancel(state: State<'_, AppState>) {
    state.wiki_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn wiki_emit(app: &AppHandle, concept: &str, phase: &str, done: usize, total: usize) {
    let _ = app.emit(
        "wiki-progress",
        serde_json::json!({ "phase": phase, "done": done, "total": total, "concept": concept }),
    );
}

/// Material gathered for one source document (internal).
struct WikiDocMaterial {
    id: i64,
    title: String,
    year: Option<i64>,
    material: String,
}

/// The top-`k` chunks of one document by cosine similarity to `qvec`.
fn top_chunks(conn: &Connection, doc_id: i64, qvec: &[f32], k: usize) -> Vec<(String, Option<i64>)> {
    let Ok(mut stmt) =
        conn.prepare("SELECT id, text, page FROM doc_chunks WHERE document_id = ?1")
    else {
        return Vec::new();
    };
    let rows: Vec<(i64, String, Option<i64>)> = stmt
        .query_map(params![doc_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map(|it| it.filter_map(Result::ok).collect())
        .unwrap_or_default();
    let mut scored: Vec<(f32, String, Option<i64>)> = Vec::new();
    for (cid, text, page) in rows {
        let emb: Option<Vec<u8>> = conn
            .query_row("SELECT embedding FROM chunk_vec WHERE chunk_id = ?1", params![cid], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        if let Some(bytes) = emb {
            scored.push((rag::cosine(qvec, &bytes_to_f32(&bytes)), text, page));
        }
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(k).map(|(_, t, p)| (t, p)).collect()
}

/// Generate (or regenerate) the wiki page for `concept`: per-paper claim
/// extraction → synthesis → source-coverage repair, all on the local LLM.
/// Sources: `ids` when given (the user picked them explicitly), else the tag
/// with the same name (or `tag_id`), else semantic search. Emits
/// `wiki-progress` events; cancellable via [`wiki_cancel`].
#[tauri::command]
pub async fn wiki_generate(
    app: AppHandle,
    concept: String,
    tag_id: Option<i64>,
    ids: Option<Vec<i64>>,
) -> Result<String, String> {
    let concept = concept.trim().to_string();
    if concept.is_empty() {
        return Err("Scrivi un concetto (o usa il nome di un tag)".into());
    }
    let cache = embed_cache_dir(&app);
    let (enabled, provider, url, model, gpu, ollama_url) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone(), c.embed_gpu, c.ollama_url.clone())
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    {
        let state = app.state::<AppState>();
        state.wiki_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    }
    let cancelled = |app: &AppHandle| {
        app.state::<AppState>().wiki_cancel.load(std::sync::atomic::Ordering::SeqCst)
    };

    // 1) Resolve the source documents and gather their material (CPU-bound).
    let app2 = app.clone();
    let concept2 = concept.clone();
    let ollama2 = ollama_url.clone();
    let explicit_sel = ids.as_ref().map(|l| !l.is_empty()).unwrap_or(false);
    let materials: Vec<WikiDocMaterial> =
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<WikiDocMaterial>, String> {
            let qvec = embed_query_text(gpu, &ollama2, &cache, &concept2).map_err(|e| e.to_string())?;
            let state = app2.state::<AppState>();
            let conn = state.db.lock();
            let mut ids: Vec<i64> = if let Some(list) = ids.filter(|l| !l.is_empty()) {
                // Explicit selection: exactly these documents, in the given order
                // (still filtered to existing, non-deleted rows below via the
                // per-document metadata query).
                list
            } else if let Some(tid) = tag_id {
                let mut s = conn
                    .prepare(
                        "SELECT dt.document_id FROM document_tags dt
                         JOIN documents d ON d.id = dt.document_id
                         WHERE dt.tag_id = ?1 AND d.deleted_at IS NULL ORDER BY d.year, d.id",
                    )
                    .map_err(|e| e.to_string())?;
                let v: Vec<i64> = s
                    .query_map(params![tid], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect();
                v
            } else if let Some(v) = tag_member_ids(&conn, &concept2) {
                v
            } else {
                // Free concept: the most semantically related documents.
                let mut s = conn
                    .prepare(
                        "SELECT v.document_id FROM doc_vec v
                         JOIN documents d ON d.id = v.document_id
                         WHERE v.embedding MATCH ?1 AND k = 8 AND d.deleted_at IS NULL
                         ORDER BY distance",
                    )
                    .map_err(|e| e.to_string())?;
                let v: Vec<i64> = s
                    .query_map(params![qvec.as_slice().as_bytes()], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect();
                v
            };
            ids.truncate(if explicit_sel { 10 } else { 8 }); // LLM-context budget
            if ids.is_empty() {
                return Err(
                    "Nessun documento per questo concetto: usa il nome di un tag esistente o genera l'indice semantico".into(),
                );
            }
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                let Some((title, year, abstract_)) = conn
                    .query_row(
                        "SELECT COALESCE(title,'Senza titolo'), year, abstract FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                        params![id],
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, Option<i64>>(1)?,
                                r.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|e| e.to_string())?
                else {
                    continue;
                };
                let chunks = top_chunks(&conn, id, &qvec, 4);
                let mut material = String::new();
                if let Some(a) = abstract_.as_deref().filter(|a| !a.trim().is_empty()) {
                    material.push_str(&ai::truncate(a, 900));
                    material.push_str("\n\n");
                }
                material.push_str(
                    &chunks
                        .iter()
                        .map(|(t, p)| match p {
                            Some(p) => format!("[p. {p}] {}", ai::truncate(t, 1100)),
                            None => ai::truncate(t, 1100),
                        })
                        .collect::<Vec<_>>()
                        .join("\n---\n"),
                );
                out.push(WikiDocMaterial { id, title, year, material });
            }
            Ok(out)
        })
        .await
        .map_err(|e| e.to_string())??;

    // 2) Per-paper claim extraction.
    let client = ai::client().map_err(|e| e.to_string())?;
    let total = materials.len() + 1;
    let mut sources: Vec<WikiSource> = Vec::with_capacity(materials.len());
    let mut blocks: Vec<String> = Vec::new();
    for (i, m) in materials.iter().enumerate() {
        if cancelled(&app) {
            return Err("Generazione annullata".into());
        }
        wiki_emit(&app, &concept, "estrazione", i, total);
        let mut claims: Vec<(String, Option<i64>)> = Vec::new();
        if !m.material.trim().is_empty() {
            let prompt = wiki::extraction_prompt(&concept, &m.title, m.year, &m.material);
            let out = ai::generate(&client, &provider, &url, &model, &prompt, 420)
                .await
                .map_err(|e| format!("{e:#}"))?;
            if !out.trim().eq_ignore_ascii_case("niente") {
                claims = wiki::parse_claims(&out);
            }
        }
        let n = i + 1;
        if !claims.is_empty() {
            let lines = claims
                .iter()
                .map(|(t, p)| match p {
                    Some(p) => format!("- {t} (p. {p})"),
                    None => format!("- {t}"),
                })
                .collect::<Vec<_>>()
                .join("\n");
            let year = m.year.map(|y| y.to_string()).unwrap_or_else(|| "s.d.".into());
            blocks.push(format!("[{n}] {} ({year})\n{lines}", m.title));
        }
        sources.push(WikiSource {
            n,
            document_id: m.id,
            title: m.title.clone(),
            year: m.year,
            claims: claims.into_iter().map(|(text, page)| WikiClaim { text, page }).collect(),
            used: false, // set after synthesis, from the actual citations
        });
    }
    if blocks.is_empty() {
        return Err(
            "Nessun contenuto pertinente trovato nei documenti. L'indice dei passaggi è costruito? (Chiedi alla libreria → Costruisci indice)".into(),
        );
    }

    // 3) Synthesis.
    if cancelled(&app) {
        return Err("Generazione annullata".into());
    }
    wiki_emit(&app, &concept, "sintesi", materials.len(), total);
    let mut page = ai::generate(&client, &provider, &url, &model, &wiki::synthesis_prompt(&concept, &blocks), 1100)
        .await
        .map_err(|e| format!("{e:#}"))?;
    // Belt & braces: drop a leading H1 the model may add despite instructions.
    if page.trim_start().starts_with("# ") {
        page = page.trim_start().splitn(2, '\n').nth(1).unwrap_or("").to_string();
    }

    // 4) Coverage: every source with claims must be cited, or explicitly parked.
    let must: Vec<(usize, String)> = sources
        .iter()
        .filter(|s| !s.claims.is_empty())
        .map(|s| (s.n, s.title.clone()))
        .collect();
    let missing: Vec<(usize, String)> = {
        let cited = wiki::cited_ns(&page);
        must.iter().filter(|(n, _)| !cited.contains(n)).cloned().collect()
    };
    if !missing.is_empty() && !cancelled(&app) {
        wiki_emit(&app, &concept, "copertura", materials.len(), total);
        let repaired = ai::generate(&client, &provider, &url, &model, &wiki::repair_prompt(&concept, &page, &missing), 1300)
            .await
            .map_err(|e| format!("{e:#}"))?;
        if repaired.len() > 200 && wiki::cited_ns(&repaired).len() >= wiki::cited_ns(&page).len() {
            page = repaired;
        }
    }
    // Whatever is still missing is declared, never silently dropped.
    let cited = wiki::cited_ns(&page);
    let still: Vec<(usize, String)> =
        must.iter().filter(|(n, _)| !cited.contains(n)).cloned().collect();
    if !still.is_empty() && !page.contains("## Fonti non") {
        page.push_str("\n\n## Fonti non integrate\n");
        for (n, t) in &still {
            page.push_str(&format!("- [{n}] {t} — la sintesi non ha utilizzato questa fonte.\n"));
        }
    }
    let cited = wiki::cited_ns(&page);
    for s in &mut sources {
        s.used = !s.claims.is_empty() && cited.contains(&s.n);
    }

    // 5) Store (regeneration replaces the page in place).
    let slug = wiki::slugify(&concept);
    let mut chars = concept.chars();
    let title = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_else(|| concept.clone());
    let doc_ids = materials.iter().map(|m| m.id.to_string()).collect::<Vec<_>>().join(",");
    let sources_json = serde_json::to_string(&sources).map_err(|e| e.to_string())?;
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO wiki_pages (slug, concept, title, content_md, sources_json, doc_ids, model, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(slug) DO UPDATE SET concept = excluded.concept, title = excluded.title,
               content_md = excluded.content_md, sources_json = excluded.sources_json,
               doc_ids = excluded.doc_ids, model = excluded.model, generated_at = datetime('now')",
            params![slug, concept, title, page, sources_json, doc_ids, model],
        )
        .map_err(|e| e.to_string())?;
    }
    wiki_emit(&app, &concept, "done", total, total);
    Ok(slug)
}

// ===== Sintesi sulla selezione: confronto, rassegna, raccolta risultati =====
// Tre workflow che riusano la meccanica della Wiki (estrazione ancorata per
// documento → sintesi con citazioni [n]); condividono l'evento `wiki-progress`
// e la cancellazione `wiki_cancel` (il frontend li serializza comunque).

/// A document prepared for a synthesis call (internal).
struct SynthDoc {
    id: i64,
    title: String,
    year: Option<i64>,
    citekey: Option<String>,
    material: String,
}

#[derive(serde::Serialize)]
pub struct ReviewSource {
    pub n: usize,
    pub document_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub citekey: Option<String>,
}

#[derive(serde::Serialize)]
pub struct AiDocResult {
    /// The synthesized markdown (citations as plain [n]).
    pub md: String,
    /// Sanitized HTML with [n] linked to #src-n for the UI.
    pub html: String,
    pub sources: Vec<ReviewSource>,
}

/// Digit density of a string — crude but effective proxy for "results table".
fn digit_density(s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    s.chars().filter(|c| c.is_ascii_digit()).count() as f32 / s.chars().count() as f32
}

/// Gather title/year/citekey + grounded material for each document.
/// `digit_focus` ranks chunks by numeric density (for result harvesting)
/// instead of taking the opening chunks (contributions/method).
fn synth_docs(conn: &Connection, ids: &[i64], digit_focus: bool) -> Result<Vec<SynthDoc>, String> {
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        let Some((title, year, citekey, abstract_, summary)) = conn
            .query_row(
                "SELECT COALESCE(title,'Senza titolo'), year, citekey, abstract, summary
                 FROM documents WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?
        else {
            continue;
        };
        let mut chunks: Vec<(i64, String, Option<i64>)> = {
            let mut s = conn
                .prepare("SELECT ord, text, page FROM doc_chunks WHERE document_id = ?1 ORDER BY ord")
                .map_err(|e| e.to_string())?;
            let v: Vec<(i64, String, Option<i64>)> = s
                .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .collect();
            v
        };
        if digit_focus {
            chunks.sort_by(|a, b| {
                digit_density(&b.1)
                    .partial_cmp(&digit_density(&a.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        chunks.truncate(3);
        let mut material = String::new();
        if let Some(a) = abstract_.as_deref().filter(|a| !a.trim().is_empty()) {
            material.push_str(&ai::truncate(a, 700));
            material.push_str("\n\n");
        }
        if let Some(s) = summary.as_deref().filter(|s| !s.trim().is_empty()) {
            material.push_str(&ai::truncate(s, 500));
            material.push_str("\n\n");
        }
        material.push_str(
            &chunks
                .iter()
                .map(|(_, t, p)| match p {
                    Some(p) => format!("[p. {p}] {}", ai::truncate(t, 1000)),
                    None => ai::truncate(t, 1000),
                })
                .collect::<Vec<_>>()
                .join("\n---\n"),
        );
        out.push(SynthDoc { id, title, year, citekey, material });
    }
    if out.is_empty() {
        return Err("Nessun documento valido nella selezione".into());
    }
    Ok(out)
}

/// AI config + client, or the standard Italian "AI disabled" error.
fn ai_ready(app: &AppHandle) -> Result<(reqwest::Client, String, String, String), String> {
    let (enabled, provider, url, model) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let c = ai_config(&conn);
        (c.enabled, c.provider.clone(), c.active_url().to_string(), c.model.clone())
    };
    if !enabled {
        return Err("Le funzioni AI sono disattivate (abilitale nelle Impostazioni)".into());
    }
    let client = ai::client().map_err(|e| e.to_string())?;
    Ok((client, provider, url, model))
}

fn synth_sources(docs: &[SynthDoc]) -> Vec<ReviewSource> {
    docs.iter()
        .enumerate()
        .map(|(i, d)| ReviewSource {
            n: i + 1,
            document_id: d.id,
            title: d.title.clone(),
            year: d.year,
            citekey: d.citekey.clone(),
        })
        .collect()
}

fn ai_doc_result(md: String, docs: &[SynthDoc]) -> AiDocResult {
    let html = wiki::render_html_live(&wiki::link_citations(&md));
    AiDocResult { md, html, sources: synth_sources(docs) }
}

/// Structured comparison of 2-3 selected papers (one grounded LLM call).
#[tauri::command]
pub async fn compare_documents(app: AppHandle, ids: Vec<i64>) -> Result<AiDocResult, String> {
    if !(2..=3).contains(&ids.len()) {
        return Err("Seleziona 2 o 3 documenti da confrontare".into());
    }
    let (client, provider, url, model) = ai_ready(&app)?;
    let docs = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        synth_docs(&conn, &ids, false)?
    };
    wiki_emit(&app, "Confronto", "sintesi", 0, 1);
    let blocks = docs
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let year = d.year.map(|y| y.to_string()).unwrap_or_else(|| "s.d.".into());
            format!("[{}] «{}» ({year})\n{}", i + 1, d.title, d.material)
        })
        .collect::<Vec<_>>()
        .join("\n\n=====\n\n");
    let prompt = format!(
        "Confronta in italiano i {n} paper qui sotto, usando SOLO il materiale fornito.\n\
         Struttura richiesta (markdown):\n\
         1. una tabella con colonna «Aspetto» e una colonna per paper (intestazioni [1], [2]…),\n\
            righe: Obiettivo · Approccio/Metodo · Dati o dominio · Risultati chiave · Limiti;\n\
            celle brevi (max ~15 parole); se il materiale non dice nulla scrivi \"—\";\n\
         2. sezione \"## In sintesi\": 2-4 punti su cosa distingue ciascun paper e cosa aggiunge \
            rispetto agli altri, citando [n];\n\
         niente premesse, niente titolo iniziale, non inventare nulla.\n\n{blocks}",
        n = docs.len()
    );
    let md = ai::generate(&client, &provider, &url, &model, &prompt, 900)
        .await
        .map_err(|e| format!("{e:#}"))?;
    wiki_emit(&app, "Confronto", "done", 1, 1);
    Ok(ai_doc_result(md, &docs))
}

/// Mini literature review of the selection (per-paper claims → synthesis),
/// with [n] citations mapped to citekeys for LaTeX/Pandoc export.
#[tauri::command]
pub async fn generate_review(app: AppHandle, ids: Vec<i64>) -> Result<AiDocResult, String> {
    if !(2..=10).contains(&ids.len()) {
        return Err("Servono da 2 a 10 documenti per una rassegna".into());
    }
    let (client, provider, url, model) = ai_ready(&app)?;
    {
        let state = app.state::<AppState>();
        state.wiki_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    }
    let docs = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        synth_docs(&conn, &ids, false)?
    };
    let total = docs.len() + 1;
    let mut blocks = Vec::with_capacity(docs.len());
    for (i, d) in docs.iter().enumerate() {
        if app.state::<AppState>().wiki_cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Generazione annullata".into());
        }
        wiki_emit(&app, "Rassegna", "estrazione", i, total);
        let year = d.year.map(|y| y.to_string()).unwrap_or_else(|| "s.d.".into());
        let prompt = format!(
            "Dal materiale del paper «{}» ({year}) estrai 4-6 affermazioni chiave su: contributo \
             principale, metodo, risultati, limiti. Una per riga con \"- \", italiano, fattuali, \
             SOLO dal materiale.\n\nMATERIALE:\n{}",
            d.title, d.material
        );
        let out = ai::generate(&client, &provider, &url, &model, &prompt, 420)
            .await
            .map_err(|e| format!("{e:#}"))?;
        blocks.push(format!("[{}] {} ({year})\n{}", i + 1, d.title, out.trim()));
    }
    wiki_emit(&app, "Rassegna", "sintesi", docs.len(), total);
    let prompt = format!(
        "Scrivi in italiano una breve rassegna della letteratura (stile \"related work\", 300-500 \
         parole) basata ESCLUSIVAMENTE sulle affermazioni per paper qui sotto.\n\
         Regole: organizza per temi (non paper per paper); confronta gli approcci; evidenzia \
         disaccordi e lacune aperte; cita OGNI paper almeno una volta con [n] subito dopo le \
         affermazioni che ne derivano; niente titolo iniziale, comincia con \"## Panorama\"; \
         chiudi con \"## Lacune aperte\" (2-3 punti). Non inventare nulla.\n\n{}",
        blocks.join("\n\n")
    );
    let mut md = ai::generate(&client, &provider, &url, &model, &prompt, 1100)
        .await
        .map_err(|e| format!("{e:#}"))?;
    // Coverage, same policy as the wiki: nothing dropped silently.
    let cited = wiki::cited_ns(&md);
    let missing: Vec<String> = docs
        .iter()
        .enumerate()
        .filter(|(i, _)| !cited.contains(&(i + 1)))
        .map(|(i, d)| format!("- [{}] {}", i + 1, d.title))
        .collect();
    if !missing.is_empty() {
        md.push_str("\n\n## Fonti non integrate\n");
        md.push_str(&missing.join("\n"));
        md.push('\n');
    }
    wiki_emit(&app, "Rassegna", "done", total, total);
    Ok(ai_doc_result(md, &docs))
}

/// Harvest quantitative results across the selection into one comparable grid
/// (columns: Paper · Metodo · Dataset · Metrica · Valore). Per-paper grounded
/// extraction; merging is deterministic.
#[tauri::command]
pub async fn harvest_results(app: AppHandle, ids: Vec<i64>) -> Result<Vec<Vec<String>>, String> {
    if !(1..=8).contains(&ids.len()) {
        return Err("Seleziona da 1 a 8 documenti".into());
    }
    let (client, provider, url, model) = ai_ready(&app)?;
    {
        let state = app.state::<AppState>();
        state.wiki_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    }
    let docs = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        synth_docs(&conn, &ids, true)?
    };
    let total = docs.len();
    let mut grid: Vec<Vec<String>> = vec![vec![
        "Paper".into(),
        "Metodo".into(),
        "Dataset".into(),
        "Metrica".into(),
        "Valore".into(),
    ]];
    for (i, d) in docs.iter().enumerate() {
        if app.state::<AppState>().wiki_cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Generazione annullata".into());
        }
        wiki_emit(&app, "Risultati", "estrazione", i, total);
        let prompt = format!(
            "Dal materiale del paper «{}» estrai i risultati quantitativi principali (max 8).\n\
             Formato: una riga per risultato, ESATTAMENTE 4 campi separati da \" | \":\n\
             metodo | dataset o benchmark | metrica | valore\n\
             Esempio: AlphaZero | Go | Elo | 5185\n\
             Usa SOLO il materiale (valori testuali, non calcolare nulla); se non ci sono \
             risultati quantitativi rispondi esattamente: NIENTE.\n\nMATERIALE:\n{}",
            d.title, d.material
        );
        let out = ai::generate(&client, &provider, &url, &model, &prompt, 400)
            .await
            .map_err(|e| format!("{e:#}"))?;
        if out.trim().eq_ignore_ascii_case("niente") {
            continue;
        }
        let short = if d.title.chars().count() > 40 {
            format!("[{}] {}…", i + 1, d.title.chars().take(38).collect::<String>())
        } else {
            format!("[{}] {}", i + 1, d.title)
        };
        for line in out.lines() {
            let line = line.trim().trim_start_matches("- ").trim_start_matches('|');
            let cells: Vec<String> =
                line.split('|').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
            if cells.len() == 4 && !cells[0].eq_ignore_ascii_case("metodo") {
                grid.push(vec![
                    short.clone(),
                    cells[0].clone(),
                    cells[1].clone(),
                    cells[2].clone(),
                    cells[3].clone(),
                ]);
            }
        }
    }
    if grid.len() == 1 {
        return Err("Nessun risultato quantitativo trovato nei documenti selezionati".into());
    }
    wiki_emit(&app, "Risultati", "done", total, total);
    Ok(grid)
}

// ===== Percorso di lettura: prerequisiti per capire un paper =====

#[derive(serde::Serialize)]
pub struct PathStep {
    pub document_id: Option<i64>,
    pub title: String,
    pub year: Option<i64>,
    pub reason: String,
    pub in_library: bool,
    pub doi: Option<String>,
}

/// What to read before a paper: its in-library references (declared
/// foundations), semantically close but earlier documents, and its most
/// frequently-cited references you don't own yet. No LLM involved.
#[tauri::command]
pub fn reading_path(state: State<'_, AppState>, id: i64) -> Result<Vec<PathStep>, String> {
    let conn = state.db.lock();
    let (year, _title): (Option<i64>, String) = conn
        .query_row(
            "SELECT year, COALESCE(title,'') FROM documents WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "documento non trovato".to_string())?;

    let mut steps: Vec<PathStep> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::iter::once(id).collect();

    // 1) In-library references: the paper's declared foundations.
    {
        let mut s = conn
            .prepare(
                "SELECT DISTINCT d.id, COALESCE(d.title,'Senza titolo'), d.year, d.doi
                 FROM document_references dr
                 JOIN documents d ON LOWER(d.doi) = LOWER(dr.ref_doi)
                 WHERE dr.document_id = ?1 AND d.deleted_at IS NULL AND d.id != ?1
                 ORDER BY d.year",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(i64, String, Option<i64>, Option<String>)> = s
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        for (did, title, y, doi) in rows.into_iter().take(6) {
            seen.insert(did);
            steps.push(PathStep {
                document_id: Some(did),
                title,
                year: y,
                reason: "Citato direttamente dal paper: è un suo fondamento dichiarato".into(),
                in_library: true,
                doi,
            });
        }
    }

    // 2) Semantically close and earlier documents you already own.
    let emb: Option<Vec<u8>> = conn
        .query_row("SELECT embedding FROM doc_vec WHERE document_id = ?1", params![id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(e) = emb {
        let mut s = conn
            .prepare(
                "SELECT v.document_id FROM doc_vec v JOIN documents d ON d.id = v.document_id
                 WHERE v.embedding MATCH ?1 AND k = 12 AND d.deleted_at IS NULL ORDER BY distance",
            )
            .map_err(|e| e.to_string())?;
        let cand: Vec<i64> = s
            .query_map(params![e], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        let mut added = 0;
        for did in cand {
            if added >= 4 || seen.contains(&did) {
                continue;
            }
            let row = conn
                .query_row(
                    "SELECT COALESCE(title,'Senza titolo'), year, doi FROM documents WHERE id = ?1",
                    params![did],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<String>>(2)?)),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some((title, y, doi)) = row {
                // "earlier" only when both years are known; otherwise still useful.
                if let (Some(ty), Some(cy)) = (year, y) {
                    if cy > ty {
                        continue;
                    }
                }
                seen.insert(did);
                added += 1;
                steps.push(PathStep {
                    document_id: Some(did),
                    title,
                    year: y,
                    reason: "Molto vicino per contenuti e precedente: prepara il contesto".into(),
                    in_library: true,
                    doi,
                });
            }
        }
    }

    // In-library steps in reading order: oldest first.
    steps.sort_by_key(|s| s.year.unwrap_or(i64::MAX));

    // 3) References you don't own, ranked by how often your library cites them.
    {
        let mut s = conn
            .prepare(
                "SELECT dr.ref_doi, MAX(COALESCE(dr.raw,'')),
                        (SELECT COUNT(DISTINCT dr2.document_id) FROM document_references dr2
                         WHERE LOWER(dr2.ref_doi) = LOWER(dr.ref_doi)) AS freq
                 FROM document_references dr
                 WHERE dr.document_id = ?1 AND dr.ref_doi IS NOT NULL AND dr.ref_doi != ''
                   AND NOT EXISTS (SELECT 1 FROM documents d WHERE LOWER(d.doi) = LOWER(dr.ref_doi)
                                   AND d.deleted_at IS NULL)
                 GROUP BY LOWER(dr.ref_doi) ORDER BY freq DESC LIMIT 4",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<(String, String, i64)> = s
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        for (doi, raw, freq) in rows {
            let title = if raw.trim().is_empty() {
                doi.clone()
            } else {
                ai::truncate(raw.trim(), 110)
            };
            steps.push(PathStep {
                document_id: None,
                title,
                year: None,
                reason: if freq > 1 {
                    format!("Riferimento non in libreria, citato da {freq} tuoi documenti")
                } else {
                    "Riferimento del paper non ancora in libreria".into()
                },
                in_library: false,
                doi: Some(doi),
            });
        }
    }

    if steps.is_empty() {
        return Err(
            "Nessun prerequisito trovato: servono i riferimenti (chip «✦ senza metadati» in alto) o l'indice semantico".into(),
        );
    }
    Ok(steps)
}

#[cfg(test)]
mod detex_tests {
    use super::detex_title;

    #[test]
    fn strips_tex_markup_from_titles() {
        assert_eq!(detex_title("\\emph{Attention} Is All You Need"), "Attention Is All You Need");
        // $…$ math and a \log control word become word boundaries.
        assert_eq!(detex_title("A $O(n\\log n)$ Sort"), "A O(n n) Sort");
        // An accent control symbol is dropped but its letter stays attached.
        assert_eq!(detex_title("Sch\\\"olkopf on Kernels"), "Scholkopf on Kernels");
        // A plain title is returned unchanged.
        assert_eq!(detex_title("Plain Title 2024"), "Plain Title 2024");
    }
}

#[cfg(test)]
mod url_normalize_tests {
    use super::normalize_pdf_url;

    #[test]
    fn rewrites_github_blob_to_raw() {
        assert_eq!(
            normalize_pdf_url("https://github.com/deepseek-ai/DeepSpec/blob/main/DSpark_paper.pdf"),
            "https://raw.githubusercontent.com/deepseek-ai/DeepSpec/main/DSpark_paper.pdf"
        );
        // Nested folders and percent-encoded names survive as-is.
        assert_eq!(
            normalize_pdf_url("https://github.com/o/r/blob/dev/docs/My%20Paper.pdf"),
            "https://raw.githubusercontent.com/o/r/dev/docs/My%20Paper.pdf"
        );
    }

    #[test]
    fn leaves_everything_else_untouched() {
        for u in [
            "https://arxiv.org/pdf/2401.00001.pdf",
            "https://github.com/o/r/releases/download/v1/x.pdf", // already a file URL
            "https://github.com/o/r",                            // too few segments
            "https://raw.githubusercontent.com/o/r/main/x.pdf",
            "http://github.com/o/r/blob/main/x.pdf", // non-https: the SSRF gate rejects later
            "not a url",
        ] {
            assert_eq!(normalize_pdf_url(u), u);
        }
    }
}

#[cfg(test)]
mod ssrf_tests {
    use super::is_safe_fetch_url;

    #[test]
    fn accepts_normal_public_https() {
        assert!(is_safe_fetch_url("https://arxiv.org/pdf/2401.00001.pdf"));
        assert!(is_safe_fetch_url("https://export.arxiv.org/pdf/2401.00001"));
        // A domain with a trailing dot is allowed at the gate; the public-only resolver
        // vets its actual IPs at connect time (no pin-key mismatch any more).
        assert!(is_safe_fetch_url("https://example.com./paper.pdf"));
    }

    #[test]
    fn rejects_non_https_and_nonstandard_port() {
        assert!(!is_safe_fetch_url("http://example.com/x.pdf"));
        assert!(!is_safe_fetch_url("ftp://example.com/x.pdf"));
        assert!(!is_safe_fetch_url("https://example.com:8443/x.pdf"));
        assert!(!is_safe_fetch_url("not a url"));
    }

    #[test]
    fn rejects_internal_ip_literals() {
        assert!(!is_safe_fetch_url("https://127.0.0.1/x.pdf"));
        assert!(!is_safe_fetch_url("https://10.0.0.5/x.pdf"));
        assert!(!is_safe_fetch_url("https://192.168.1.1/x.pdf"));
        assert!(!is_safe_fetch_url("https://169.254.169.254/latest/meta-data/"));
        assert!(!is_safe_fetch_url("https://[::1]/x.pdf"));
        assert!(!is_safe_fetch_url("https://localhost/x.pdf"));
    }

    #[test]
    fn rejects_parser_differential_bypasses() {
        // Backslash authority terminator: the WHATWG parser (= what reqwest connects with)
        // resolves the host to the loopback literal before the '\', not the public name
        // after it. The gate must see the SAME loopback host and reject.
        assert!(!is_safe_fetch_url("https://127.0.0.1\\@public.example/x.pdf"));
        assert!(!is_safe_fetch_url("https://10.0.0.1\\@evil.example/x.pdf"));
        // Obfuscated numeric IPv4 forms canonicalize to 127.0.0.1.
        assert!(!is_safe_fetch_url("https://0x7f000001/x.pdf"));
        assert!(!is_safe_fetch_url("https://2130706433/x.pdf"));
        // Trailing-dot loopback FQDN form.
        assert!(!is_safe_fetch_url("https://127.0.0.1./x.pdf"));
    }
}
