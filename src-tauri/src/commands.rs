//! Tauri command handlers exposed to the Svelte frontend.

use crate::ai;
use crate::bibtex;
use crate::citation;
use crate::discovery;
use crate::embed;
use crate::github;
use crate::import;
use crate::metadata;
use crate::obsidian;
use crate::pdf;
use crate::rag;
use crate::secret;
use crate::table;
use crate::term;
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
            // Slow work (read, hash, pdfium extract/render) WITHOUT the DB lock.
            let prepared = match import::prepare_import(&state.pdfium, &dir, Path::new(&p)) {
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

/// Result of a metadata-enrichment batch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichSummary {
    pub updated: usize,
    pub no_doi: usize,
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
        errors: Vec::new(),
    };

    for (id, existing, fulltext) in candidates {
        let doi = existing.or_else(|| metadata::extract_doi(&fulltext));
        let Some(doi) = doi else {
            summary.no_doi += 1;
            continue;
        };
        match metadata::fetch_crossref(&client, &doi, email.as_deref()).await {
            Ok(Some(meta)) => {
                let state = app.state::<AppState>();
                let mut conn = state.db.lock();
                match metadata::apply_metadata(&mut conn, id, &doi, &meta) {
                    Ok(()) => {
                        // Refresh the stored citekey now that authors/year/title are known.
                        let _ = crate::db::citekey::auto_citekey(&conn, id);
                        summary.updated += 1;
                    }
                    Err(e) => summary.errors.push(format!("{doi}: {e:#}")),
                }
            }
            Ok(None) => summary.no_doi += 1,
            Err(e) => summary.errors.push(format!("{doi}: {e:#}")),
        }
        // Be polite to the Crossref API between requests.
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }

    Ok(summary)
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

        let candidates: Vec<(i64, Option<String>, Option<String>, String)> = {
            let conn = state.db.lock();
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

        let total = candidates.len();
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
        "SELECT id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey FROM documents WHERE id = ?1"
    } else {
        "SELECT id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey FROM documents WHERE id = ?1 AND deleted_at IS NULL"
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
                ))
            })
            .optional()?;
        let Some((id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey)) = row else {
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
            added_at,
            is_read: is_read != 0,
            favorite: favorite != 0,
            github_url,
            pub_status,
            paper_url,
            citekey,
        });
    }
    Ok(out)
}

// ===== Tags =====

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
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
        "SELECT id, name, color FROM tags WHERE name = ?1",
        params![name],
        |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
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

/// Remember the last viewed page (1-based) of a document.
#[tauri::command]
pub fn set_last_page(state: State<'_, AppState>, id: i64, page: i64) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE documents SET last_page = ?1, last_opened_at = datetime('now') WHERE id = ?2",
        params![page, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
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
fn bib_to_ref(e: &bibtex::BibEntry) -> Option<metadata::ResolvedRef> {
    let f = &e.fields;
    let get = |k: &str| f.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let title = get("title");
    // Route the DOI field through extract_doi so any wrapper (doi.org, dx.doi.org,
    // "doi:", trailing punctuation) is normalized to a bare lowercase DOI.
    let doi = get("doi").and_then(|d| metadata::extract_doi(&d)).map(|d| d.to_lowercase());
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

/// Try to attach an Open-Access PDF to a reference-only document.
/// Returns "attached" | "already" | "not_found".
#[tauri::command]
pub async fn find_pdf(app: AppHandle, id: i64) -> Result<String, String> {
    let (doi, path, email) = {
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
        let email = setting(&conn, "discovery_email").filter(|s| !s.trim().is_empty());
        (doi, path, email)
    };
    // Already has a real file?
    if path.as_deref().map(|p| !p.starts_with("ref:")).unwrap_or(false) {
        return Ok("already".into());
    }

    let client = metadata::http_client(email.as_deref()).map_err(|e| e.to_string())?;
    // Prefer an arXiv eprint if the reference came from arXiv; else ask Unpaywall.
    let mut url: Option<String> = None;
    if let Some(rest) = path.as_deref().and_then(|p| p.strip_prefix("ref:arxiv:")) {
        url = Some(format!("https://arxiv.org/pdf/{rest}"));
    }
    if url.is_none() {
        let Some(doi) = doi.as_deref() else {
            return Ok("not_found".into());
        };
        let mail = email
            .clone()
            .ok_or_else(|| "Per «Trova PDF» imposta un'email nelle Impostazioni (richiesta da Unpaywall)".to_string())?;
        url = metadata::unpaywall_pdf(&client, doi, &mail).await.map_err(|e| e.to_string())?;
    }
    let Some(url) = url else {
        return Ok("not_found".into());
    };

    let Some(saved) = download_pdf(&app, &url, &format!("doc_{id}")).await? else {
        return Ok("not_found".into());
    };

    // Extract text + thumbnail (no lock), then attach to the existing row.
    let state = app.state::<AppState>();
    let dir = thumb_dir(&app);
    let prepared = import::prepare_import(&state.pdfium, &dir, &saved).map_err(|e| e.to_string())?;
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
        std::fs::remove_file(&saved).ok();
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

/// Extract the plain text of a normalized region `[x,y,w,h]` (rotation-0 frame)
/// of page `page` (1-based), as readable lines.
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
    Ok(table::join_text(&words))
}

/// Write arbitrary text to a file (used by "extract text" → Save .txt/.md).
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
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

async fn download_pdf(
    app: &AppHandle,
    url: &str,
    stem: &str,
) -> Result<Option<PathBuf>, String> {
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
    let dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("papers"))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).ok();
    // Sanitize the caller's stem to a single filename token (no separators / no `..`).
    let path = dir.join(format!("{}.pdf", safe_component(stem)));
    // Defense-in-depth: never write outside the papers directory.
    if path.parent() != Some(dir.as_path()) {
        return Err("percorso di salvataggio non valido".into());
    }
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(Some(path))
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
                "SELECT 1 FROM documents WHERE deleted_at IS NULL AND ((doi IS NOT NULL AND doi = ?1) OR path LIKE ?2) LIMIT 1",
                params![r.doi, format!("%{}%", r.external_id)],
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
                "SELECT id FROM documents WHERE deleted_at IS NULL AND ((doi IS NOT NULL AND doi = ?1) OR path LIKE ?2) LIMIT 1",
                params![result.doi, format!("%{}%", result.external_id)],
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
        let stem = format!("{}_{}", safe_component(&result.source), safe_component(&result.external_id));
        if let Ok(Some(saved)) = download_pdf(&app, &url, &stem).await {
            let state = app.state::<AppState>();
            let dir = thumb_dir(&app);
            if let Ok(prepared) = import::prepare_import(&state.pdfium, &dir, &saved) {
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
    })
}

const SAVED_COLS: &str = "id, name, source, query, author, year_from, year_to, oa_only, sort, seen_ids, last_run_at";

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
    conn.execute("DELETE FROM saved_searches WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct SavedRun {
    pub name: String,
    pub results: Vec<discovery::SearchResult>,
    /// external_ids that are new since the last run.
    pub new_ids: Vec<String>,
}

/// Re-run a saved search; returns its results and which are new since last time,
/// then records the current results as seen.
#[tauri::command]
pub async fn run_saved_search(app: AppHandle, id: i64) -> Result<SavedRun, String> {
    let (s, seen) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        let s = conn
            .query_row(&format!("SELECT {SAVED_COLS} FROM saved_searches WHERE id = ?1"), params![id], row_to_saved)
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "ricerca non trovata".to_string())?;
        let seen: String = conn
            .query_row("SELECT seen_ids FROM saved_searches WHERE id = ?1", params![id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        (s, seen)
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

    let seen_set: std::collections::HashSet<&str> = seen.lines().collect();
    let new_ids: Vec<String> = results
        .iter()
        .map(|r| r.external_id.as_str())
        .filter(|x| !seen_set.contains(x))
        .map(str::to_string)
        .collect();

    // Merge the current ids into "seen" and stamp the run.
    let mut all: std::collections::BTreeSet<String> = seen.lines().map(str::to_string).collect();
    for r in &results {
        all.insert(r.external_id.clone());
    }
    let merged = all.into_iter().collect::<Vec<_>>().join("\n");
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock();
        conn.execute(
            "UPDATE saved_searches SET seen_ids = ?1, last_run_at = datetime('now') WHERE id = ?2",
            params![merged, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(SavedRun { name: s.name, results, new_ids })
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
                   (SELECT LOWER(doi) FROM documents WHERE doi IS NOT NULL)
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

#[derive(serde::Serialize)]
pub struct LibraryFacets {
    pub all: i64,
    pub favorite: i64,
    pub unread: i64,
    pub github: i64,
    pub peerreviewed: i64,
}

/// Counts behind the sidebar's library filters, computed over the whole library
/// (independent of the active filter) so each entry can show how many documents
/// it holds. `peerreviewed` mirrors `query_documents`: it's derived from
/// `classify_pub_status`, not a stored column, so it's evaluated per document.
#[tauri::command]
pub fn library_facets(state: State<'_, AppState>) -> Result<LibraryFacets, String> {
    let conn = state.db.lock();
    let mut facets = LibraryFacets { all: 0, favorite: 0, unread: 0, github: 0, peerreviewed: 0 };
    let mut stmt = conn
        .prepare(
            "SELECT favorite, is_read, github_url, doi, venue, path
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
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (favorite, is_read, github_url, doi, venue, path) = row.map_err(|e| e.to_string())?;
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
        _ => {}
    }
    let sql = format!(
        "SELECT id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey
         FROM documents WHERE {} ORDER BY added_at DESC, id DESC",
        conds.join(" AND ")
    );

    let mut stmt = conn.prepare(&sql)?;
    #[allow(clippy::type_complexity)]
    let base: Vec<(i64, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64, Option<String>, Option<String>, Option<String>)> =
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
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut author_stmt = conn.prepare(
        "SELECT a.given, a.family
         FROM authors a JOIN document_authors da ON da.author_id = a.id
         WHERE da.document_id = ?1 ORDER BY da.position",
    )?;

    let mut docs = Vec::with_capacity(base.len());
    for (id, title, year, venue, doi, thumb_path, added_at, is_read, favorite, github_url, path, citekey) in base {
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
            added_at,
            is_read: is_read != 0,
            favorite: favorite != 0,
            github_url,
            pub_status,
            paper_url,
            citekey,
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

// ===== Local AI servers: start / stop =====

/// PowerShell snippet that resolves the LM Studio `lms` CLI (PATH or the default
/// install location) into `$lms`, throwing a helpful error if it's missing.
const LMS_RESOLVE: &str = "$lms = (Get-Command lms -ErrorAction SilentlyContinue).Source; \
     if (-not $lms) { $p = Join-Path $env:USERPROFILE '.lmstudio\\bin\\lms.exe'; if (Test-Path $p) { $lms = $p } } \
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
        // Best-effort kill; never fail just because nothing was running.
        run_powershell(
            "taskkill /F /IM 'ollama app.exe' 2>$null; taskkill /F /IM ollama.exe 2>$null; exit 0",
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
