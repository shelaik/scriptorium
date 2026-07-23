//! Database schema for PDFmanage.
//!
//! One SQLite file holds everything: the relational core (documents, authors,
//! tags, collections, annotations), an FTS5 full-text index kept in sync by
//! triggers, and a `sqlite-vec` `vec0` virtual table for bge-m3 (1024-dim)
//! semantic search. All statements are idempotent (`IF NOT EXISTS`) so
//! `migrate` can run safely on every startup.

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};

const SCHEMA: &str = r#"
-- ===== Relational core =====
CREATE TABLE IF NOT EXISTS documents (
  id            INTEGER PRIMARY KEY,
  doi           TEXT UNIQUE,
  title         TEXT,
  abstract      TEXT,
  fulltext      TEXT,
  year          INTEGER,
  venue         TEXT,                 -- container / journal title
  language      TEXT,                 -- 'en' | 'it' | detected
  path          TEXT NOT NULL UNIQUE, -- absolute file path on disk
  file_hash     TEXT,                 -- sha256 for dedupe
  thumb_path    TEXT,                 -- cached page-0 PNG
  added_at      TEXT DEFAULT (datetime('now')),
  modified_at   TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS authors (
  id      INTEGER PRIMARY KEY,
  family  TEXT,
  given   TEXT,
  UNIQUE(family, given)
);
CREATE TABLE IF NOT EXISTS document_authors (
  document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  author_id   INTEGER NOT NULL REFERENCES authors(id)   ON DELETE CASCADE,
  position    INTEGER,               -- author order (sequence)
  PRIMARY KEY (document_id, author_id)
);

-- References extracted from Crossref message.reference[] / OpenAlex referenced_works
CREATE TABLE IF NOT EXISTS document_references (
  id           INTEGER PRIMARY KEY,
  document_id  INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  ref_doi      TEXT,
  raw          TEXT                  -- unstructured citation text
);

-- Saved online searches ("monitor a topic"): re-run to surface new results.
CREATE TABLE IF NOT EXISTS saved_searches (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  source      TEXT NOT NULL,
  query       TEXT NOT NULL DEFAULT '',
  author      TEXT,
  year_from   INTEGER,
  year_to     INTEGER,
  oa_only     INTEGER NOT NULL DEFAULT 0,
  sort        TEXT NOT NULL DEFAULT 'relevance',
  seen_ids    TEXT NOT NULL DEFAULT '',   -- newline-joined external ids already seen
  created_at  TEXT DEFAULT (datetime('now')),
  last_run_at TEXT
);

-- Persistent "Novità" feed: genuinely-new results surfaced by a saved search
-- since it was created. Fed by both the manual run and the on-launch sweep;
-- UNIQUE(watch_id, external_id) dedups across runs, `state` tracks the user's
-- handling (new -> added | dismissed). Decoupled from saved_searches.seen_ids
-- (the frozen creation baseline) so manual runs and sweeps never cannibalize.
CREATE TABLE IF NOT EXISTS watch_hits (
  id          INTEGER PRIMARY KEY,
  watch_id    INTEGER NOT NULL REFERENCES saved_searches(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  result_json TEXT NOT NULL,
  score       REAL,
  found_at    TEXT DEFAULT (datetime('now')),
  state       TEXT NOT NULL DEFAULT 'new',   -- 'new' | 'added' | 'dismissed'
  UNIQUE(watch_id, external_id)
);

-- ===== Tags =====
CREATE TABLE IF NOT EXISTS tags (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE,
  color TEXT
);
CREATE TABLE IF NOT EXISTS document_tags (
  document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  tag_id      INTEGER NOT NULL REFERENCES tags(id)      ON DELETE CASCADE,
  PRIMARY KEY (document_id, tag_id)
);

-- ===== Collections (manual + rule-based smart) =====
CREATE TABLE IF NOT EXISTS collections (
  id        INTEGER PRIMARY KEY,
  name      TEXT NOT NULL,
  parent_id INTEGER REFERENCES collections(id) ON DELETE CASCADE,
  is_smart  INTEGER NOT NULL DEFAULT 0,   -- 1 = rule-based
  rule_json TEXT                          -- predicate AST for smart collections (NULL if manual)
);
CREATE TABLE IF NOT EXISTS collection_members (
  collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  document_id   INTEGER NOT NULL REFERENCES documents(id)   ON DELETE CASCADE,
  PRIMARY KEY (collection_id, document_id)
);

-- ===== Annotations / highlights =====
CREATE TABLE IF NOT EXISTS annotations (
  id          INTEGER PRIMARY KEY,
  document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  page        INTEGER NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'highlight', -- highlight | note | underline
  color       TEXT,
  rects_json  TEXT NOT NULL,         -- normalized [x,y,w,h] quads on the page
  quote       TEXT,                  -- selected text
  note        TEXT,                  -- user comment
  created_at  TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_annotations_doc ON annotations(document_id, page);

-- ===== Settings / API cache =====
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT
);
CREATE TABLE IF NOT EXISTS api_cache (
  doi           TEXT PRIMARY KEY,
  source        TEXT,                -- 'crossref' | 'openalex'
  response_json TEXT,
  fetched_at    TEXT DEFAULT (datetime('now'))
);

-- ===== FTS5 full-text index (external content over `documents`) =====
-- unicode61 + remove_diacritics 2 handles Italian accents correctly.
CREATE VIRTUAL TABLE IF NOT EXISTS doc_fts USING fts5(
  title, abstract, fulltext,
  content='documents', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);

-- Sync triggers (external-content safe pattern):
-- use BEFORE UPDATE for the 'delete' command so stale tokens are removed
-- using OLD values, then AFTER UPDATE re-inserts the NEW values.
CREATE TRIGGER IF NOT EXISTS documents_ai AFTER INSERT ON documents BEGIN
  INSERT INTO doc_fts(rowid, title, abstract, fulltext)
    VALUES (new.id, new.title, new.abstract, new.fulltext);
END;
CREATE TRIGGER IF NOT EXISTS documents_ad AFTER DELETE ON documents BEGIN
  INSERT INTO doc_fts(doc_fts, rowid, title, abstract, fulltext)
    VALUES ('delete', old.id, old.title, old.abstract, old.fulltext);
END;
-- WHEN guards: only re-index when an FTS column actually changes, so writes
-- to thumb_path / year / venue / doi don't trigger a full fulltext re-tokenize.
-- DROP first so existing databases pick up the guarded definition.
DROP TRIGGER IF EXISTS documents_au;
CREATE TRIGGER documents_au BEFORE UPDATE ON documents
WHEN old.title IS NOT new.title OR old.abstract IS NOT new.abstract OR old.fulltext IS NOT new.fulltext
BEGIN
  INSERT INTO doc_fts(doc_fts, rowid, title, abstract, fulltext)
    VALUES ('delete', old.id, old.title, old.abstract, old.fulltext);
END;
DROP TRIGGER IF EXISTS documents_au2;
CREATE TRIGGER documents_au2 AFTER UPDATE ON documents
WHEN old.title IS NOT new.title OR old.abstract IS NOT new.abstract OR old.fulltext IS NOT new.fulltext
BEGIN
  INSERT INTO doc_fts(rowid, title, abstract, fulltext)
    VALUES (new.id, new.title, new.abstract, new.fulltext);
END;

-- ===== Standalone notes: a rebuildable search index over the .md vault =====
-- The `.md` files remain the source of truth; this shadow table exists only so
-- notes are full-text searchable. It's reconciled from disk on startup and on
-- every note CRUD, so it can always be dropped and rebuilt.
CREATE TABLE IF NOT EXISTS notes (
  id         INTEGER PRIMARY KEY,
  slug       TEXT NOT NULL UNIQUE,
  title      TEXT,
  body       TEXT,
  updated_at INTEGER
);
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
  title, body,
  content='notes', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
  INSERT INTO notes_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
END;
CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
END;
-- WHEN guard (mirrors documents_au): a re-save / startup reindex that doesn't
-- change the text is a no-op, so notes aren't re-tokenized on every launch.
-- DROP first so an existing DB picks up the guarded definition.
DROP TRIGGER IF EXISTS notes_au;
CREATE TRIGGER notes_au AFTER UPDATE ON notes
WHEN old.title IS NOT new.title OR old.body IS NOT new.body
BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
  INSERT INTO notes_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
END;

-- ===== Vector store (sqlite-vec vec0), bge-m3 dense 1024-dim, cosine =====
-- Kept in sync from Rust after embedding (not via trigger). At <200 docs the
-- brute-force KNN is sub-millisecond. Metadata/partition columns intentionally
-- omitted for now (documents are many-to-many with collections).
CREATE VIRTUAL TABLE IF NOT EXISTS doc_vec USING vec0(
  document_id INTEGER PRIMARY KEY,
  embedding   float[1024] distance_metric=cosine
);

-- ===== Vettori degli appunti (.md vault) =====
-- Same bge-m3 space as doc_vec, so notes join the semantic map next to the
-- papers they are about. Filled by the Indice semantico job; purely derived.
CREATE VIRTUAL TABLE IF NOT EXISTS note_vec USING vec0(
  note_id   INTEGER PRIMARY KEY,
  embedding float[1024] distance_metric=cosine
);

-- ===== Costellazione: posizioni dei nodi persistite =====
-- Saved by the frontend when the layout settles, so the map keeps the same shape
-- across sessions (mental-map stability). Purely a cache: safe to drop anytime.
CREATE TABLE IF NOT EXISTS graph_positions (
  document_id INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
  x REAL NOT NULL,
  y REAL NOT NULL
);
-- Posizioni degli APPUNTI nella Costellazione (id nodo = -note_id): tabella
-- separata perché graph_positions ha la FK sui documents.
CREATE TABLE IF NOT EXISTS note_graph_positions (
  note_id INTEGER PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
  x REAL NOT NULL,
  y REAL NOT NULL
);

-- ===== RAG: passage chunks + their embeddings ("ask your library") =====
-- Document-level vectors above are great for "find similar docs"; the engine
-- needs passage-level retrieval to answer questions and cite specific text.
CREATE TABLE IF NOT EXISTS doc_chunks (
  id          INTEGER PRIMARY KEY,
  document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  ord         INTEGER NOT NULL,
  text        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_doc_chunks_doc ON doc_chunks(document_id);

CREATE VIRTUAL TABLE IF NOT EXISTS chunk_vec USING vec0(
  chunk_id  INTEGER PRIMARY KEY,
  embedding float[1024] distance_metric=cosine
);

-- ===== Wiki della libreria: pagine concettuali generate dall'LLM locale =====
-- content_md is stored raw; citation links / [[cross-links]] are woven at read
-- time so they always reflect the pages that exist now.
CREATE TABLE IF NOT EXISTS wiki_pages (
  id            INTEGER PRIMARY KEY,
  slug          TEXT NOT NULL UNIQUE,
  concept       TEXT NOT NULL,
  title         TEXT NOT NULL,
  content_md    TEXT NOT NULL,
  sources_json  TEXT NOT NULL DEFAULT '[]', -- [{n, document_id, title, year, claims:[{text,page}], used}]
  doc_ids       TEXT NOT NULL DEFAULT '',   -- comma-joined ids used at generation (staleness check)
  model         TEXT,
  generated_at  TEXT DEFAULT (datetime('now'))
);
"#;

/// Add a column if the table doesn't already have it (SQLite has no
/// `ADD COLUMN IF NOT EXISTS`).
fn add_column_if_missing(conn: &Connection, table: &str, col: &str, decl: &str) -> Result<()> {
    // `table`/`col` are interpolated into DDL (SQLite can't bind identifiers), so they
    // must be bare identifiers. Every caller passes a compile-time literal; this guard
    // keeps it that way if a future caller is ever tempted to pass dynamic input.
    // (`decl` legitimately contains spaces/keywords like "INTEGER NOT NULL DEFAULT 0".)
    let is_ident = |s: &str| {
        !s.is_empty()
            && !s.chars().next().unwrap().is_ascii_digit()
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    anyhow::ensure!(
        is_ident(table) && is_ident(col),
        "add_column_if_missing: identificatore non valido ({table}.{col})"
    );
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|name| name == col);
    if !exists {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {col} {decl}"), [])?;
    }
    Ok(())
}

/// Apply the full schema. Idempotent: safe to call on every startup.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)
        .context("applying database schema")?;
    // Soft-delete column for the Trash feature (added in a later version).
    add_column_if_missing(conn, "documents", "deleted_at", "TEXT")?;
    // Manager features: free notes, read/favorite flags, last viewed page.
    add_column_if_missing(conn, "documents", "notes", "TEXT")?;
    add_column_if_missing(conn, "documents", "is_read", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(conn, "documents", "favorite", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(conn, "documents", "last_page", "INTEGER")?;
    // When the document was last opened in the reader ("Continue reading" shelf).
    add_column_if_missing(conn, "documents", "last_opened_at", "TEXT")?;
    // AI per-paper: cached summary text.
    add_column_if_missing(conn, "documents", "summary", "TEXT")?;
    // First GitHub repo URL found in the document's text (for the "has code" badge/filter).
    add_column_if_missing(conn, "documents", "github_url", "TEXT")?;
    // Persistent, library-unique citation key (firstauthor+year+word, see db::citekey).
    add_column_if_missing(conn, "documents", "citekey", "TEXT")?;
    // Total page count, for the reading-progress bar (set on import / first open).
    add_column_if_missing(conn, "documents", "page_count", "INTEGER")?;
    // Marks the user's own work (set when importing a LaTeX project .zip), so it
    // can seed "my work" views and citation-gap analysis.
    add_column_if_missing(conn, "documents", "is_own", "INTEGER NOT NULL DEFAULT 0")?;
    // RAG: page number a passage was taken from (NULL for older chunks / non-PDF refs).
    add_column_if_missing(conn, "doc_chunks", "page", "INTEGER")?;
    // Saved search participates in the on-launch "Novità" sweep (default: yes).
    add_column_if_missing(conn, "saved_searches", "auto_run", "INTEGER NOT NULL DEFAULT 1")?;
    // Ricerca «Novità» agganciata a una raccolta (vista Archivio): le novità
    // accettate dal suo feed entrano direttamente nella raccolta.
    add_column_if_missing(conn, "saved_searches", "collection_id", "INTEGER")?;
    backfill_github_urls(conn)?;
    // Assign citekeys to any documents that don't have one yet (cheap no-op once full).
    super::citekey::backfill(conn)?;
    Ok(())
}

/// One-time scan of the existing library to populate `github_url` from each
/// document's text. Guarded by a settings flag so it runs only once; new imports
/// set the column directly.
fn backfill_github_urls(conn: &Connection) -> Result<()> {
    let done: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = 'gh_backfill_v1'", [], |r| r.get(0))
        .optional()?;
    if done.is_some() {
        return Ok(());
    }
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(fulltext,'') || ' ' || COALESCE(abstract,'')
         FROM documents WHERE github_url IS NULL",
    )?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(std::result::Result::ok)
        .collect();
    drop(stmt);
    for (id, text) in rows {
        if let Some(url) = crate::github::first_repo_url(&text) {
            conn.execute("UPDATE documents SET github_url = ?1 WHERE id = ?2", rusqlite::params![url, id])?;
        }
    }
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('gh_backfill_v1', '1')",
        [],
    )?;
    Ok(())
}
