//! Database layer: connection setup, the statically-linked `sqlite-vec`
//! registration, and schema migrations.

pub mod migrations;

use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use std::path::Path;

static VEC_REGISTERED: OnceCell<()> = OnceCell::new();

/// Register the statically-linked `sqlite-vec` extension as a SQLite
/// *auto-extension*, so the `vec0` module is available on every connection
/// (including `:memory:`). MUST be called once before any [`Connection`] is
/// opened. Idempotent thanks to the `OnceCell` guard.
///
/// We use `rusqlite::auto_extension::register_auto_extension` (the older
/// `ffi::sqlite3_auto_extension` shape no longer typechecks on rusqlite 0.40).
pub fn register_sqlite_vec() {
    VEC_REGISTERED.get_or_init(|| {
        // SAFETY: `sqlite3_vec_init` has the standard SQLite extension entry
        // point signature; we transmute its function pointer to the
        // `RawAutoExtension` type rusqlite expects.
        unsafe {
            rusqlite::auto_extension::register_auto_extension(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            ))
            .expect("failed to register sqlite-vec auto extension");
        }
    });
}

/// Open (creating if needed) the database at `path`, set pragmas and apply
/// migrations. [`register_sqlite_vec`] must already have been called.
pub fn open(path: &Path) -> Result<Connection> {
    let conn =
        Connection::open(path).with_context(|| format!("opening database at {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrations::migrate(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use zerocopy::IntoBytes;

    /// End-to-end smoke test: registration -> migrations -> relational insert
    /// -> FTS5 trigger population -> vec0 KNN round-trip.
    #[test]
    fn smoke_test_full_stack() -> Result<()> {
        register_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::migrate(&conn)?;

        // Relational insert.
        conn.execute(
            "INSERT INTO documents (title, abstract, fulltext, path) VALUES (?1, ?2, ?3, ?4)",
            params![
                "Attention Is All You Need",
                "We propose the Transformer.",
                "full body discussing neural networks and attention mechanisms",
                "/tmp/transformer.pdf"
            ],
        )?;
        let doc_id = conn.last_insert_rowid();

        // FTS5 must have been populated by the AFTER INSERT trigger.
        let fts_hits: i64 = conn.query_row(
            "SELECT count(*) FROM doc_fts WHERE doc_fts MATCH 'neural'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(fts_hits, 1, "FTS trigger did not index the new document");

        // Vector round-trip: insert a 1024-dim embedding, query nearest.
        let embedding: Vec<f32> = vec![0.05_f32; 1024];
        conn.execute(
            "INSERT INTO doc_vec (document_id, embedding) VALUES (?1, ?2)",
            params![doc_id, embedding.as_slice().as_bytes()],
        )?;
        let nearest: i64 = conn.query_row(
            "SELECT document_id FROM doc_vec WHERE embedding MATCH ?1 AND k = 1",
            params![embedding.as_slice().as_bytes()],
            |r| r.get(0),
        )?;
        assert_eq!(nearest, doc_id, "vec0 KNN did not return the inserted row");

        // Update must keep FTS consistent (BEFORE/AFTER UPDATE trigger pair).
        conn.execute(
            "UPDATE documents SET fulltext = ?1 WHERE id = ?2",
            params!["completely different content about databases", doc_id],
        )?;
        let stale: i64 = conn.query_row(
            "SELECT count(*) FROM doc_fts WHERE doc_fts MATCH 'neural'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(stale, 0, "FTS still matches stale tokens after update");

        Ok(())
    }

    /// The "related papers" feature reads a stored vector back out of vec0 and
    /// re-queries with it — verify that round-trip works.
    #[test]
    fn vec_self_retrieval_roundtrip() -> Result<()> {
        register_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        migrations::migrate(&conn)?;
        conn.execute("INSERT INTO documents (title, path) VALUES ('a', '/a.pdf')", [])?;
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO documents (title, path) VALUES ('b', '/b.pdf')", [])?;
        let b = conn.last_insert_rowid();

        let va: Vec<f32> = (0..1024).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
        let vb: Vec<f32> = (0..1024).map(|i| if i == 0 { 0.9 } else { 0.05 }).collect();
        conn.execute(
            "INSERT INTO doc_vec (document_id, embedding) VALUES (?1, ?2)",
            params![a, va.as_slice().as_bytes()],
        )?;
        conn.execute(
            "INSERT INTO doc_vec (document_id, embedding) VALUES (?1, ?2)",
            params![b, vb.as_slice().as_bytes()],
        )?;

        // Read a's vector back out and confirm it's the raw 4096-byte blob.
        let emb: Vec<u8> = conn.query_row(
            "SELECT embedding FROM doc_vec WHERE document_id = ?1",
            params![a],
            |r| r.get(0),
        )?;
        assert_eq!(emb.len(), 1024 * 4, "embedding should round-trip as raw f32 bytes");

        // Re-query KNN with the retrieved bytes.
        let mut stmt = conn
            .prepare("SELECT document_id FROM doc_vec WHERE embedding MATCH ?1 AND k = 2 ORDER BY distance")?;
        let ids: Vec<i64> = stmt
            .query_map(params![emb], |r| r.get::<_, i64>(0))?
            .filter_map(Result::ok)
            .collect();
        assert!(ids.contains(&a) && ids.contains(&b), "KNN should find both docs");
        Ok(())
    }
}
