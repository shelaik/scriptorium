//! Persistent, library-unique citation keys.
//!
//! A document gets a stored `citekey` (firstauthor + year + first title word,
//! with a letter suffix when two documents would otherwise collide) once it has
//! a known first author. Documents without an author yet keep a NULL key and the
//! exporter falls back to a computed key for them — this avoids persisting a
//! throwaway "anon…" key that would churn the moment metadata is fetched.
//!
//! The stored key is what the citation exporter emits, so a multi-document
//! BibTeX/CSL export is always valid and a paper's key stays stable across
//! exports. Keys are (re)generated whenever a document's bibliographic metadata
//! is created or changed; see the call sites in `commands`. Uniqueness is
//! checked against ALL documents, including soft-deleted ones, so a key held by
//! a trashed paper isn't reused (which would collide if the paper is restored)
//! and live keys never churn when a sibling is trashed. The key is fully derived
//! from metadata (no manual override yet).

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

/// Disambiguation suffix for the n-th holder of a base key:
/// 0 -> "" (bare), 1 -> "b", 2 -> "c", … 24 -> "y", then numeric ("26", "27"…).
/// 'a' is intentionally skipped so the bare key reads as the implicit "a".
fn suffix(n: u32) -> String {
    if n == 0 {
        String::new()
    } else if n < 25 {
        ((b'a' + n as u8) as char).to_string()
    } else {
        (n + 1).to_string()
    }
}

/// First author's family name (by author order) for a document, if any.
fn first_family(conn: &Connection, id: i64) -> Result<Option<String>> {
    let fam = conn
        .query_row(
            "SELECT a.family FROM authors a
             JOIN document_authors da ON da.author_id = a.id
             WHERE da.document_id = ?1 ORDER BY da.position LIMIT 1",
            params![id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(fam)
}

/// (Re)generate a document's stored citekey from its current metadata, choosing
/// the lowest disambiguation suffix not already taken by ANOTHER document
/// (including soft-deleted ones). Writes the column only when the value actually
/// changes, and returns the chosen key — or an empty string (and no write) when
/// the document has no first author yet. Stable: the holder of a key keeps it
/// while its metadata is unchanged, so re-running this never churns keys.
pub fn auto_citekey(conn: &Connection, id: i64) -> Result<String> {
    // Defer until a first author exists; until then the export computes a key.
    let Some(family) = first_family(conn, id)?.map(|f| f.trim().to_string()).filter(|s| !s.is_empty())
    else {
        return Ok(String::new());
    };
    let (title, year): (Option<String>, Option<i64>) = conn.query_row(
        "SELECT title, year FROM documents WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let base = crate::citation::citekey_from_parts(Some(&family), year, title.as_deref());
    let current: Option<String> = conn
        .query_row("SELECT citekey FROM documents WHERE id = ?1", params![id], |r| r.get(0))
        .optional()?
        .flatten();
    let mut n = 0u32;
    let key = loop {
        let cand = format!("{base}{}", suffix(n));
        let taken = conn
            .query_row(
                "SELECT 1 FROM documents WHERE citekey = ?1 AND id <> ?2 LIMIT 1",
                params![cand, id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !taken {
            break cand;
        }
        n += 1;
    };
    if current.as_deref() != Some(key.as_str()) {
        conn.execute("UPDATE documents SET citekey = ?1 WHERE id = ?2", params![key, id])?;
    }
    Ok(key)
}

/// One-time backfill: assign a citekey to every live document that lacks one,
/// in id order for determinism. Idempotent — documents that already have a key
/// are left untouched.
pub fn backfill(conn: &Connection) -> Result<()> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM documents
             WHERE deleted_at IS NULL AND (citekey IS NULL OR TRIM(citekey) = '')
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for id in ids {
        auto_citekey(conn, id)?;
    }
    Ok(())
}
