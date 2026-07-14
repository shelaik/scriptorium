//! Database layer: connection setup, the statically-linked `sqlite-vec`
//! registration, and schema migrations.

pub mod citekey;
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

/// Pre-migration safety net: the FIRST time a new app version starts, copy the
/// database (plus any leftover WAL/SHM from an unclean shutdown) into
/// `backups/` BEFORE the new version's migrations touch it. Keeps the newest
/// 5 backups. Must run before [`open`]; best-effort — never blocks startup.
pub fn backup_on_upgrade(data_dir: &Path, db_path: &Path, version: &str) {
    let marker = data_dir.join("last_version.txt");
    let prev = std::fs::read_to_string(&marker).ok();
    if prev.as_deref().map(str::trim) == Some(version) {
        return; // stessa versione già avviata: nessuna migrazione nuova in arrivo
    }
    // No DB yet (first ever launch) → nothing to back up; advancing the marker is
    // correct. If a DB exists, only advance the marker once the .db copy actually
    // succeeded, so a failed backup (full disk, AV/OneDrive lock, unwritable dir)
    // is retried next launch instead of silently running migrations with no net.
    let mut backed_up = true;
    if db_path.is_file() {
        backed_up = false;
        let dir = data_dir.join("backups");
        let _ = std::fs::create_dir_all(&dir);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let base = format!("pre-{version}-{secs}");
        if std::fs::copy(db_path, dir.join(format!("{base}.db"))).is_ok() {
            backed_up = true;
            for ext in ["-wal", "-shm"] {
                let side = db_path.with_file_name(format!(
                    "{}{}",
                    db_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    ext
                ));
                if side.is_file() {
                    let _ = std::fs::copy(&side, dir.join(format!("{base}.db{ext}")));
                }
            }
            prune_backups(&dir, 5);
        }
    }
    if backed_up {
        let _ = std::fs::write(&marker, version);
    }
}

/// Keep only the newest `keep` backup groups (a group = the files sharing the
/// same `pre-…` stem, i.e. .db + optional .db-wal/.db-shm).
fn prune_backups(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    // (stem, mtime, full paths) per group
    let mut groups: std::collections::HashMap<String, (std::time::SystemTime, Vec<std::path::PathBuf>)> =
        std::collections::HashMap::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("pre-") {
            continue;
        }
        let stem = name.split(".db").next().unwrap_or(&name).to_string();
        let mtime = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        let g = groups.entry(stem).or_insert((mtime, Vec::new()));
        if mtime > g.0 {
            g.0 = mtime;
        }
        g.1.push(e.path());
    }
    if groups.len() <= keep {
        return;
    }
    let mut ordered: Vec<_> = groups.into_values().collect();
    ordered.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    for (_, files) in ordered.into_iter().skip(keep) {
        for f in files {
            let _ = std::fs::remove_file(f);
        }
    }
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

    /// Version bump → one backup; same version again → no new backup.
    #[test]
    fn backup_runs_once_per_version() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("dbbk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let db = dir.join("pdfmanage.db");
        std::fs::write(&db, b"fake-db-bytes")?;

        backup_on_upgrade(&dir, &db, "1.2.3");
        let count = || {
            std::fs::read_dir(dir.join("backups"))
                .map(|it| it.flatten().count())
                .unwrap_or(0)
        };
        assert_eq!(count(), 1, "prima apertura della versione: un backup");
        backup_on_upgrade(&dir, &db, "1.2.3");
        assert_eq!(count(), 1, "stessa versione: nessun backup in più");
        backup_on_upgrade(&dir, &db, "1.2.4");
        assert_eq!(count(), 2, "versione nuova: nuovo backup");
        assert_eq!(std::fs::read_to_string(dir.join("last_version.txt"))?, "1.2.4");

        let _ = std::fs::remove_dir_all(&dir);
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
