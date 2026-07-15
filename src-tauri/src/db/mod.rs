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

/// The `-wal`/`-shm` sidecar paths SQLite derives from a database file path
/// (it simply appends the suffix to the full db filename).
fn db_sidecars(base: &Path) -> [std::path::PathBuf; 2] {
    let name = base
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    [
        base.with_file_name(format!("{name}-wal")),
        base.with_file_name(format!("{name}-shm")),
    ]
}

/// Recursively copy `src` into `dst`, overwriting files that already exist and
/// creating directories as needed. Extra files in `dst` are left untouched
/// (merge, not mirror) and symlinks are skipped. Used by [`apply_pending_restore`]
/// to bring back a full backup's papers/notes/projects without deleting anything
/// created since the backup.
///
/// Per-file copy errors are **propagated** (not swallowed): when this copies the
/// pre-restore *undo snapshot* of `notes/`/`projects/`, a silently-incomplete copy
/// would let the subsequent overwrite destroy an in-place edit with no recoverable
/// copy — so the snapshot caller aborts the restore if any file fails. The final
/// merge-back caller, where a partial copy is not catastrophic (the db is already
/// swapped and the undo snapshot exists), deliberately ignores the returned error.
fn copy_dir_merge(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        let to = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_merge(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Apply a restore staged by the `stage_restore` command (Impostazioni → Backup →
/// Ripristina). Runs at startup, **before any SQLite connection is opened**, so
/// the live catalog file can be swapped safely.
///
/// Designed safe-by-construction for the app's single most destructive operation:
/// the live library is only ever replaced by a backup that has been proven sound,
/// and only after a complete, recoverable snapshot of the current state exists. Any
/// failure along the way aborts with the current library fully intact.
///
/// Steps, on finding `restore_pending.txt` in `data_dir`:
///  1. Consume the marker; only proceed if it was actually deleted (a marker we can't
///     remove would otherwise re-apply this destructive restore on every launch).
///  2. Resolve the backup's `pdfmanage.db` (a folder → `<folder>/pdfmanage.db`, or a
///     `.db` file directly). If it's gone, do nothing (leave current data intact).
///  3. **Stage + validate**: copy the backup (and any `-wal`/`-shm`) into a staging
///     file, then prove it opens, passes `integrity_check` and `migrate`s cleanly.
///     A corrupt / truncated / foreign source is rejected here and never reaches the
///     live db, so it can never brick startup — we abort and keep the current library.
///  4. **Checkpoint + snapshot**: fold the live WAL into the main db, then snapshot the
///     current catalog to `backups/pre-restore-<ts>/` (db + sidecars, plus `notes/` and
///     `projects/` for a full restore). This MUST succeed or we abort — the live library
///     is never destroyed without a recoverable copy.
///  5. **Atomic swap**: drop the (checkpointed, empty) live WAL/SHM, then `rename` the
///     validated staging db over `pdfmanage.db` in one step (a mid-copy failure can't
///     corrupt the live db).
///  6. For a full backup folder, merge-restore `papers/`, `notes/`, `projects/` (never
///     deletes anything added since the backup).
///  7. Delete `last_version.txt` so [`backup_on_upgrade`] then makes a pre-migration
///     backup of the restored db before [`open`] migrates it up to the current schema.
pub fn apply_pending_restore(data_dir: &Path, db_path: &Path) {
    let marker = data_dir.join("restore_pending.txt");
    let Ok(content) = std::fs::read_to_string(&marker) else {
        return;
    };
    // Consume the marker FIRST, and only proceed if it was actually removed. A marker we
    // cannot delete (read-only attribute, deny-delete ACL, AV/indexer lock without
    // FILE_SHARE_DELETE) would otherwise re-apply this destructive restore on every
    // launch — silently reverting new work and eventually pruning the undo snapshot.
    // Refusing here means the restore just doesn't run this launch; the live library
    // stays intact and the user can retry.
    if std::fs::remove_file(&marker).is_err() {
        return;
    }
    let mut lines = content.lines();
    let source = match lines.next().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => return,
    };
    let full = lines.next().map(str::trim) == Some("full");
    let src = Path::new(&source);
    let src_db = if src.is_dir() { src.join("pdfmanage.db") } else { src.to_path_buf() };
    if !src_db.is_file() {
        return; // backup vanished between staging and restart — leave current data alone
    }

    // ---- 3) Stage + validate the backup BEFORE touching the live catalog ----
    // Copy it (and any -wal/-shm carrying uncheckpointed frames) into a staging file,
    // then prove it opens, is not corrupt, and migrates to the current schema. Only a
    // validated db is ever swapped in, so a corrupt / half-copied / foreign source can
    // never brick startup — we abort and keep the current library untouched.
    let staging = data_dir.join("pdfmanage.restore-staging.db");
    let stage_sides = db_sidecars(&staging);
    let clear_staging = || {
        let _ = std::fs::remove_file(&staging);
        for s in &stage_sides {
            let _ = std::fs::remove_file(s);
        }
    };
    clear_staging();
    if std::fs::copy(&src_db, &staging).is_err() {
        clear_staging();
        return;
    }
    for (from, to) in db_sidecars(&src_db).iter().zip(stage_sides.iter()) {
        if from.is_file() {
            let _ = std::fs::copy(from, to);
        }
    }
    // register_sqlite_vec() already ran in run(); open the staging db, fold in any WAL by
    // switching to a rollback journal (so the renamed file is standalone and migrate's
    // writes land in the main file), verify integrity, and migrate it up (this also
    // upgrades an older backup, making the later db::open a no-op).
    let validated = (|| -> Result<()> {
        let conn = Connection::open(&staging)?;
        conn.pragma_update(None, "journal_mode", "DELETE")?;
        let check: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if check != "ok" {
            anyhow::bail!("integrity_check failed: {check}");
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::migrate(&conn)?;
        Ok(())
    })();
    if validated.is_err() {
        clear_staging();
        return; // not a sound Scriptorium catalog — keep the current library
    }
    // Staging is now a standalone, integrity-checked, migrated db; drop any leftover
    // journal/sidecars before it is renamed into place.
    for s in &stage_sides {
        let _ = std::fs::remove_file(s);
    }

    // ---- 4) Checkpoint the live db, then snapshot the CURRENT catalog (undo point) ----
    // Checkpointing folds committed-but-uncheckpointed WAL frames into the main db file
    // so the snapshot (and the live db, if we abort) is complete, and leaves the WAL
    // empty so it can't later be replayed onto the restored db.
    if db_path.is_file() {
        if let Ok(conn) = Connection::open(db_path) {
            let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| {
                Ok::<(), rusqlite::Error>(())
            });
        }
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let snap_dir = data_dir.join("backups").join(format!("pre-restore-{secs}"));
    let snapshot = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&snap_dir)?;
        if db_path.is_file() {
            std::fs::copy(db_path, snap_dir.join("pdfmanage.db"))?;
        }
        // Sidecars (belt-and-suspenders: empty after the checkpoint, but carry frames if
        // the checkpoint failed on a locked/corrupt live db).
        for s in db_sidecars(db_path) {
            if let (true, Some(name)) = (s.is_file(), s.file_name()) {
                let _ = std::fs::copy(&s, snap_dir.join(name));
            }
        }
        // A full restore overwrites in-place-edited notes/ and projects/ below, so those
        // must be snapshotted too or edits made since the backup would be unrecoverable.
        // (papers/ are content-hash-addressed, never overwritten, so they need no copy.)
        if full {
            for sub in ["notes", "projects"] {
                let from = data_dir.join(sub);
                if from.is_dir() {
                    copy_dir_merge(&from, &snap_dir.join(sub))?;
                }
            }
        }
        Ok(())
    })();
    if snapshot.is_err() {
        // No recoverable copy of the current library ⇒ refuse to overwrite it.
        clear_staging();
        let _ = std::fs::remove_dir_all(&snap_dir);
        return;
    }

    // ---- 5) Atomic swap ----
    // Atomically replace the live catalog with the validated staging db in ONE step.
    // `rename` (same directory ⇒ same volume) is atomic and never leaves a partial file,
    // so a mid-operation failure cannot corrupt the live db. If it somehow fails we abort
    // with the live db (and its checkpointed WAL) fully intact — we do NOT fall back to a
    // non-atomic copy, which would truncate-then-write the live file and could brick
    // startup. The undo snapshot exists and the marker is consumed, so nothing loops.
    if std::fs::rename(&staging, db_path).is_err() {
        clear_staging();
        return;
    }
    // The old db's WAL/SHM are now stale against the replaced main file (and were emptied
    // by the checkpoint above); drop them so nothing is replayed onto the restored db.
    for s in db_sidecars(db_path) {
        let _ = std::fs::remove_file(s);
    }

    // ---- 6) Full backup folder → bring back the real files (merge, never delete) ----
    if full && src.is_dir() {
        for sub in ["papers", "notes", "projects"] {
            let from = src.join(sub);
            if from.is_dir() {
                let _ = copy_dir_merge(&from, &data_dir.join(sub));
            }
        }
    }

    // ---- 7) Force a pre-migration backup of the restored db before it is migrated up ----
    let _ = std::fs::remove_file(data_dir.join("last_version.txt"));
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
            // pre-restore-<ts> snapshots are directories (db + notes/projects); the
            // per-version pre-<ver>-<ts> backups are files.
            if f.is_dir() {
                let _ = std::fs::remove_dir_all(f);
            } else {
                let _ = std::fs::remove_file(f);
            }
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

    /// Build a real migrated catalog at `path` holding one document titled `title`.
    #[cfg(test)]
    fn make_catalog(path: &Path, title: &str) -> Result<()> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::migrate(&conn)?;
        conn.execute(
            "INSERT INTO documents (title, abstract, fulltext, path) VALUES (?1, '', '', ?2)",
            params![title, format!("/x/{title}.pdf")],
        )?;
        Ok(())
    }

    /// The title of the first document in the catalog at `path` (read-only open).
    #[cfg(test)]
    fn first_title(path: &Path) -> Result<String> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(conn.query_row("SELECT title FROM documents LIMIT 1", [], |r| r.get(0))?)
    }

    /// A staged full-folder restore: the current catalog is validated-in-staging then
    /// atomically swapped for the backup, the old catalog + in-place notes are snapshotted
    /// as an undo point, papers/notes are merged back, and the marker + version marker are
    /// cleared. A second launch (no marker) is a no-op.
    #[test]
    fn apply_pending_restore_validates_snapshots_and_swaps() -> Result<()> {
        register_sqlite_vec();
        let root = std::env::temp_dir().join(format!("restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data = root.join("data");
        let backup = root.join("bk");
        std::fs::create_dir_all(&data)?;
        std::fs::create_dir_all(backup.join("papers"))?;
        std::fs::create_dir_all(backup.join("notes"))?;

        // Current live library: a real catalog, an in-place note, a version marker.
        let db = data.join("pdfmanage.db");
        make_catalog(&db, "CURRENT-DOC")?;
        std::fs::create_dir_all(data.join("notes"))?;
        std::fs::write(data.join("notes").join("ideas.md"), b"current note")?;
        std::fs::write(data.join("last_version.txt"), b"9.9.9")?;

        // Full backup folder to restore (its own catalog + a paper + an edited note).
        make_catalog(&backup.join("pdfmanage.db"), "BACKUP-DOC")?;
        std::fs::write(backup.join("papers").join("x.pdf"), b"PDF")?;
        std::fs::write(backup.join("notes").join("ideas.md"), b"backup note")?;

        std::fs::write(
            data.join("restore_pending.txt"),
            format!("{}\nfull\n", backup.display()),
        )?;

        apply_pending_restore(&data, &db);

        // The live catalog is now the backup, staging is gone, markers cleared, files merged.
        assert_eq!(first_title(&db)?, "BACKUP-DOC", "db should be replaced by the backup");
        assert!(!data.join("pdfmanage.restore-staging.db").exists(), "staging cleaned up");
        assert!(!data.join("restore_pending.txt").exists(), "marker must be consumed");
        assert!(!data.join("last_version.txt").exists(), "version marker cleared to force a pre-migration backup");
        assert_eq!(std::fs::read(data.join("papers").join("x.pdf"))?, b"PDF", "full restore brings back papers");
        assert_eq!(std::fs::read(data.join("notes").join("ideas.md"))?, b"backup note", "full restore merges notes");

        // The undo snapshot: a pre-restore-*/ folder holding the OLD catalog AND old notes.
        let snap = std::fs::read_dir(data.join("backups"))?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("pre-restore-"))
                        .unwrap_or(false)
            })
            .expect("pre-restore snapshot folder must be written");
        assert_eq!(first_title(&snap.join("pdfmanage.db"))?, "CURRENT-DOC", "snapshot holds the old catalog");
        assert_eq!(
            std::fs::read(snap.join("notes").join("ideas.md"))?,
            b"current note",
            "snapshot holds the pre-restore notes (undo point for in-place edits)"
        );

        // Second launch: no marker → no-op, db unchanged.
        apply_pending_restore(&data, &db);
        assert_eq!(first_title(&db)?, "BACKUP-DOC");

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// A corrupt / non-Scriptorium backup is rejected in staging and NEVER replaces the
    /// live catalog: the app cannot be bricked by a bad restore source.
    #[test]
    fn apply_pending_restore_rejects_invalid_backup() -> Result<()> {
        register_sqlite_vec();
        let root = std::env::temp_dir().join(format!("restore-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data = root.join("data");
        std::fs::create_dir_all(&data)?;

        let db = data.join("pdfmanage.db");
        make_catalog(&db, "CURRENT-DOC")?;

        // A "backup" that is not a valid SQLite database.
        let bad = data.join("bad.db");
        std::fs::write(&bad, b"this is definitely not a sqlite database")?;
        std::fs::write(
            data.join("restore_pending.txt"),
            format!("{}\ndb\n", bad.display()),
        )?;

        apply_pending_restore(&data, &db);

        assert_eq!(first_title(&db)?, "CURRENT-DOC", "an invalid backup must NOT replace the live catalog");
        assert!(!data.join("restore_pending.txt").exists(), "marker consumed even on abort");
        assert!(!data.join("pdfmanage.restore-staging.db").exists(), "staging cleaned up on abort");

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}
