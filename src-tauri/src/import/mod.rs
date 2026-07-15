//! Import pipeline. The slow, connection-free work (read, hash, text
//! extraction, thumbnail render) is done by [`prepare_import`] OFF the DB lock;
//! [`commit_import`] then writes the row under a short-lived lock in a single
//! INSERT (so the FTS5 trigger fires exactly once).

use crate::pdf;
use anyhow::{Context, Result};
use pdfium_render::prelude::Pdfium;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Outcome of committing one file.
pub struct ImportOutcome {
    pub document_id: i64,
    /// `false` when the file already existed (matched by hash or path).
    pub imported: bool,
}

/// A PDF processed without touching the database, ready to commit.
pub struct PreparedImport {
    pub path: String,
    pub hash: String,
    pub title: String,
    pub fulltext: String,
    pub thumb_path: Option<String>,
    /// True if text extraction failed (e.g. encrypted/corrupt PDF).
    pub text_failed: bool,
    /// First GitHub repo URL found in the text, if any.
    pub github_url: Option<String>,
    /// Total page count (0 if it couldn't be read), for the reading-progress bar.
    pub page_count: u16,
}

/// Strip control characters (except newlines/tabs) embedded by some PDFs.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .collect()
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Refuse to load a single file larger than this into memory (DoS / disk guard).
const MAX_IMPORT_BYTES: u64 = 300 * 1024 * 1024; // 300 MB
/// Cap the amount of extracted text stored/indexed per document.
const MAX_FULLTEXT_CHARS: usize = 5_000_000;

/// All the slow, connection-free work for one file. No DB access, so callers
/// can run this without holding the database lock. The thumbnail is named by
/// content hash so it does not depend on the (not-yet-assigned) document id.
pub fn prepare_import(pdfium: &Pdfium, thumb_dir: &Path, src: &Path) -> Result<PreparedImport> {
    let path = src.to_string_lossy().to_string();
    let meta = std::fs::metadata(src).with_context(|| format!("reading {}", src.display()))?;
    if meta.len() > MAX_IMPORT_BYTES {
        anyhow::bail!(
            "PDF troppo grande: {} MB (limite {} MB)",
            meta.len() / (1024 * 1024),
            MAX_IMPORT_BYTES / (1024 * 1024)
        );
    }
    let bytes = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let hash = sha256_hex(&bytes);

    let (mut fulltext, text_failed, page_count) = match pdf::extract_text(pdfium, src) {
        Ok(e) => (sanitize(&e.text), false, e.page_count),
        Err(_) => (String::new(), true, 0),
    };
    // Bound the stored/indexed text (huge PDFs can extract enormous strings).
    if fulltext.chars().count() > MAX_FULLTEXT_CHARS {
        fulltext = fulltext.chars().take(MAX_FULLTEXT_CHARS).collect();
    }

    let title = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    std::fs::create_dir_all(thumb_dir).ok();
    let thumb = thumb_dir.join(format!("{hash}.png"));
    let thumb_path = if pdf::render_thumbnail(pdfium, src, &thumb, pdf::THUMB_WIDTH).is_ok() {
        Some(thumb.to_string_lossy().to_string())
    } else {
        None
    };

    let github_url = crate::github::first_repo_url(&fulltext);

    Ok(PreparedImport {
        path,
        hash,
        title,
        fulltext,
        thumb_path,
        text_failed,
        github_url,
        page_count,
    })
}

/// Return the id of an existing document matching this hash or absolute path.
fn find_existing(conn: &Connection, hash: &str, path: &str) -> Result<Option<i64>> {
    let id = conn
        .query_row(
            "SELECT id FROM documents WHERE file_hash = ?1 OR path = ?2 LIMIT 1",
            params![hash, path],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    Ok(id)
}

/// Commit a prepared import under the DB lock. Single INSERT (incl. thumb_path)
/// so the FTS5 insert trigger fires once and no follow-up UPDATE is needed.
/// `restore_trashed`: true only for a DELIBERATE manual re-import (drag-in / file
/// picker). Passive watched-folder rescans pass false — they run over the whole folder
/// on every launch (and on AV/OneDrive Modify events), so auto-restoring there would
/// silently defeat the Trash and resurrect merge_documents' soft-deleted duplicates.
pub fn commit_import(conn: &Connection, p: &PreparedImport, restore_trashed: bool) -> Result<ImportOutcome> {
    if let Some(existing) = find_existing(conn, &p.hash, &p.path)? {
        // A manual re-import of a paper whose twin is in the Trash should bring it back
        // (with its tags/notes/annotations) instead of reporting an invisible "duplicate".
        if restore_trashed {
            // 1) Trashed twin at the SAME path (re-dropping the exact file): restore in
            //    place. The UPDATE leaves path unchanged, so no UNIQUE(path) risk. Refresh
            //    the file-derived facts (the bytes at this path may have changed since it
            //    was trashed): hash, cover, fulltext (re-indexes FTS via the update
            //    trigger) and page count. Title is left as-is so a manual edit survives.
            let trashed_same_path: Option<i64> = conn
                .query_row(
                    "SELECT id FROM documents WHERE path = ?1 AND deleted_at IS NOT NULL LIMIT 1",
                    params![p.path],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(tid) = trashed_same_path {
                conn.execute(
                    "UPDATE documents SET deleted_at = NULL, file_hash = ?2, thumb_path = ?3,
                     fulltext = ?4, page_count = COALESCE(?5, page_count) WHERE id = ?1",
                    params![
                        tid,
                        p.hash,
                        p.thumb_path,
                        p.fulltext,
                        (p.page_count > 0).then_some(p.page_count),
                    ],
                )?;
                return Ok(ImportOutcome {
                    document_id: tid,
                    imported: true,
                });
            }
            // 2) A trashed twin holds the SAME bytes at a DIFFERENT path (re-dragged from a
            //    new location): restore it and repoint it here. Skip if a LIVE row already
            //    holds these bytes or this path — that is a genuine duplicate. With no such
            //    live row and no trashed row at p.path (branch 1), this path is provably
            //    free, so repointing cannot hit UNIQUE(path).
            let live_dup: bool = conn
                .query_row(
                    "SELECT 1 FROM documents WHERE (file_hash = ?1 OR path = ?2) AND deleted_at IS NULL LIMIT 1",
                    params![p.hash, p.path],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if !live_dup {
                let trashed_by_hash: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM documents WHERE file_hash = ?1 AND deleted_at IS NOT NULL LIMIT 1",
                        params![p.hash],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(tid) = trashed_by_hash {
                    conn.execute(
                        "UPDATE documents SET deleted_at = NULL, path = ?2, file_hash = ?3,
                         thumb_path = ?4, fulltext = ?5, page_count = COALESCE(?6, page_count) WHERE id = ?1",
                        params![
                            tid,
                            p.path,
                            p.hash,
                            p.thumb_path,
                            p.fulltext,
                            (p.page_count > 0).then_some(p.page_count),
                        ],
                    )?;
                    return Ok(ImportOutcome {
                        document_id: tid,
                        imported: true,
                    });
                }
            }
        }
        return Ok(ImportOutcome {
            document_id: existing,
            imported: false,
        });
    }
    conn.execute(
        "INSERT INTO documents (title, fulltext, path, file_hash, thumb_path, github_url, page_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            p.title, p.fulltext, p.path, p.hash, p.thumb_path, p.github_url,
            (p.page_count > 0).then_some(p.page_count),
        ],
    )
    .context("inserting document row")?;
    // No citekey yet: a freshly-imported PDF has no authors until enrichment,
    // and we don't persist authorless "anon…" keys (see db::citekey).
    Ok(ImportOutcome {
        document_id: conn.last_insert_rowid(),
        imported: true,
    })
}

/// Convenience: prepare + commit a single file (used in tests / simple paths).
#[allow(dead_code)]
pub fn import_file(
    conn: &Connection,
    pdfium: &Pdfium,
    thumb_dir: &Path,
    src: &Path,
) -> Result<ImportOutcome> {
    let prepared = prepare_import(pdfium, thumb_dir, src)?;
    commit_import(conn, &prepared, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::path::PathBuf;

    fn manifest(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
    }

    #[test]
    fn imports_pdf_with_text_thumbnail_and_dedupe() -> Result<()> {
        db::register_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        db::migrations::migrate(&conn)?;
        let pdfium = pdf::test_pdfium();
        let thumb_dir = std::env::temp_dir().join("pdfmanage_test_thumbs");

        let pdf_path = manifest("tests/fixtures/sample.pdf");
        let first = import_file(&conn, pdfium, &thumb_dir, &pdf_path)?;
        assert!(first.imported, "first import should be new");

        let count: i64 = conn.query_row("SELECT count(*) FROM documents", [], |r| r.get(0))?;
        assert_eq!(count, 1);
        let fts: i64 = conn.query_row(
            "SELECT count(*) FROM doc_fts WHERE doc_fts MATCH 'lorem'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(fts, 1, "fulltext should be searchable");

        let thumb: Option<String> = conn.query_row(
            "SELECT thumb_path FROM documents WHERE id = ?1",
            params![first.document_id],
            |r| r.get(0),
        )?;
        let thumb = thumb.expect("thumb_path set");
        assert!(std::fs::metadata(&thumb)?.len() > 0, "thumbnail rendered");

        let second = import_file(&conn, pdfium, &thumb_dir, &pdf_path)?;
        assert!(!second.imported, "re-import should be a duplicate");
        assert_eq!(second.document_id, first.document_id);

        std::fs::remove_dir_all(&thumb_dir).ok();
        Ok(())
    }
}
