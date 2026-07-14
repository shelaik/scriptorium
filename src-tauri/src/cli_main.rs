//! scriptorium-cli — headless, **read-only** access to the Scriptorium library.
//!
//! The library is a single plain-SQLite file; this tiny binary exposes a handful
//! of convenience queries (JSON / BibTeX output) so scripts — and Claude Code —
//! can drive the library from a terminal without opening the GUI. It NEVER writes:
//! the database is opened `READ_ONLY`, so it is safe to run while Scriptorium is
//! open (WAL allows concurrent readers).
//!
//! It links only `rusqlite` + `serde_json` (no pdfium / onnx / tauri), so it stays
//! small and builds fast. It is gated behind the `cli` cargo feature so the GUI
//! release (`tauri build`) never compiles it:
//!     cargo build --release --bin scriptorium-cli --features cli

use rusqlite::{params, params_from_iter, Connection, OpenFlags, Row};
use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const HELP: &str = r#"scriptorium-cli — headless read-only view of your Scriptorium library

USAGE:
    scriptorium-cli [--db <path>] <command> [args]

COMMANDS:
    query <text> [--limit N]              Full-text-ish search (title/venue/doi/citekey/author). JSON.
    list [--tag NAME] [--unread]          List documents. JSON.
         [--favorite] [--limit N]
    show <id>                             Full record for one document (authors, tags, abstract…). JSON.
    tags                                  All tags with document counts. JSON.
    stats                                 Library counters. JSON.
    bib  [--tag NAME] [--id N]            Export BibTeX (all, or filtered). Raw .bib to stdout.
    notes [--limit N]                     List the Appunti vault (.md files: slug/title/modified). JSON.
    note <slug>                           Print one note's raw Markdown to stdout.
    search-notes <text> [--limit N]       Search the notes (title+body) with a match excerpt. JSON.
    projects [--limit N]                  List the LaTeX projects (folders under projects/). JSON.
    schema                                Print the stable columns/paths this tool reads (the contract).
    version                               Print the CLI version.
    help                                  This message.

OPTIONS:
    --db <path>   Use a specific database file
                  (default: %APPDATA%\com.pdfmanage.app\pdfmanage.db)
                  The Appunti vault and the projects live NEXT to it
                  (<dir>\notes\*.md, <dir>\projects\<slug>\).

Read-only and safe to run while the app is open. Examples:
    scriptorium-cli query "diffusion" --limit 10
    scriptorium-cli list --tag "reinforcement learning" --unread
    scriptorium-cli bib --tag thesis > refs.bib
    scriptorium-cli notes
    scriptorium-cli note "attention-notes" > attention.md
    scriptorium-cli search-notes "gradient"
    scriptorium-cli projects
    scriptorium-cli stats
"#;

const SCHEMA_DOC: &str = r#"Scriptorium library — stable read contract (SQLite)

documents(id, doi, title, abstract, year, venue, language, path, citekey,
          is_read, favorite, added_at, modified_at, summary, notes,
          github_url, page_count, deleted_at)
    A row is "live" when deleted_at IS NULL. A reference-only entry (no PDF file)
    has path LIKE 'ref:%'; a real PDF has an absolute path.
authors(id, family, given)   document_authors(document_id, author_id, position)
tags(id, name, color)        document_tags(document_id, tag_id)
collections(id, name, parent_id, is_smart)  collection_members(collection_id, document_id)
document_references(id, document_id, ref_doi, raw)   -- extracted bibliography
annotations(id, document_id, page, kind, quote, note, created_at)
saved_searches(id, name, source, query, author, year_from, year_to, oa_only, sort,
               seen_ids, last_run_at, auto_run)      -- monitored topics
watch_hits(id, watch_id, external_id, result_json, found_at, state)  -- "Novità" feed
notes(slug, title, body)     -- shadow copy of the .md vault (kept for FTS)
settings(key, value)

Files next to the database (SOURCE OF TRUTH for writing surfaces):
<dir>\notes\*.md               -- the Appunti vault (real Markdown files)
<dir>\notes\assets\            -- images referenced by the notes
<dir>\projects\<slug>\         -- LaTeX projects (real folders; main.tex, refs.bib)

Query the DB directly with any SQLite client; this CLI is just convenience + JSON.
Open READ ONLY (e.g. sqlite3 'file:...?mode=ro') to be safe while the app runs.
"#;

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    // Pull an optional `--db <path>` from anywhere in the argument list.
    let mut db_override: Option<String> = None;
    let mut args: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == "--db" && i + 1 < raw.len() {
            db_override = Some(raw[i + 1].clone());
            i += 2;
        } else {
            args.push(raw[i].clone());
            i += 1;
        }
    }

    let cmd = args.first().map(String::as_str).unwrap_or("");
    if cmd.is_empty() || cmd == "help" || cmd == "--help" || cmd == "-h" {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if cmd == "schema" {
        print!("{SCHEMA_DOC}");
        return ExitCode::SUCCESS;
    }
    if cmd == "version" || cmd == "--version" || cmd == "-V" {
        println!("scriptorium-cli {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let path = match db_override {
        Some(p) => PathBuf::from(p),
        None => match default_db_path() {
            Some(p) => p,
            None => {
                eprintln!("scriptorium-cli: cannot locate the database (%APPDATA% unset); pass --db <path>");
                return ExitCode::FAILURE;
            }
        },
    };
    if !path.exists() {
        eprintln!("scriptorium-cli: no database at {}", path.display());
        return ExitCode::FAILURE;
    }
    let conn = match open_ro(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("scriptorium-cli: cannot open {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let rest = &args[1..];
    let result = match cmd {
        "query" => cmd_query(&conn, rest),
        "list" => cmd_list(&conn, rest),
        "show" => cmd_show(&conn, rest),
        "tags" => cmd_tags(&conn),
        "stats" => cmd_stats(&conn),
        "bib" => cmd_bib(&conn, rest),
        "notes" => cmd_notes(&path, rest),
        "note" => cmd_note(&path, rest),
        "search-notes" => cmd_search_notes(&conn, rest),
        "projects" => cmd_projects(&path, rest),
        other => Err(format!("unknown command '{other}'. Try `scriptorium-cli help`.")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("scriptorium-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

fn default_db_path() -> Option<PathBuf> {
    let appdata = env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("com.pdfmanage.app");
    p.push("pdfmanage.db");
    Some(p)
}

fn open_ro(path: &PathBuf) -> rusqlite::Result<Connection> {
    // READ_ONLY == mode=ro: never writes, safe next to the running app (WAL).
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

// ---- flag helpers ----------------------------------------------------------

fn flag_val(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
fn flag_present(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}
/// First argument that is neither a flag nor a flag's value.
fn positional(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a.starts_with("--") {
            if matches!(a.as_str(), "--limit" | "--tag" | "--id") {
                i += 2; // skip the flag and its value
            } else {
                i += 1;
            }
        } else {
            return Some(a.clone());
        }
    }
    None
}
fn limit_of(args: &[String], default: i64) -> i64 {
    flag_val(args, "--limit")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|n| *n > 0 && *n <= 100_000)
        .unwrap_or(default)
}

// ---- shared document projection --------------------------------------------

const DOC_SELECT: &str = "SELECT d.id, d.title, d.year, d.venue, d.doi, d.citekey, \
    d.is_read, d.favorite, (d.path NOT LIKE 'ref:%') AS has_pdf, \
    (SELECT GROUP_CONCAT(TRIM(COALESCE(a.given,'')||' '||COALESCE(a.family,'')), '; ') \
       FROM document_authors da JOIN authors a ON a.id = da.author_id \
       WHERE da.document_id = d.id ORDER BY da.position) AS authors \
    FROM documents d ";

fn doc_row_json(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "title": r.get::<_, Option<String>>(1)?,
        "year": r.get::<_, Option<i64>>(2)?,
        "venue": r.get::<_, Option<String>>(3)?,
        "doi": r.get::<_, Option<String>>(4)?,
        "citekey": r.get::<_, Option<String>>(5)?,
        "read": r.get::<_, i64>(6)? != 0,
        "favorite": r.get::<_, i64>(7)? != 0,
        "has_pdf": r.get::<_, i64>(8)? != 0,
        "authors": r.get::<_, Option<String>>(9)?,
    }))
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_else(|_| "null".into()));
}
fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---- commands --------------------------------------------------------------

fn cmd_query(conn: &Connection, args: &[String]) -> Result<(), String> {
    let text = positional(args).ok_or("query: missing search text")?;
    let limit = limit_of(args, 50);
    let like = format!("%{}%", text);
    let sql = format!(
        "{DOC_SELECT} WHERE d.deleted_at IS NULL AND ( \
            d.title LIKE ?1 OR d.venue LIKE ?1 OR d.doi LIKE ?1 OR d.citekey LIKE ?1 \
            OR EXISTS (SELECT 1 FROM document_authors da JOIN authors a ON a.id = da.author_id \
                       WHERE da.document_id = d.id AND (a.family LIKE ?1 OR a.given LIKE ?1)) ) \
         ORDER BY d.year DESC, d.title LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql).map_err(e2s)?;
    let rows: Vec<Value> = stmt
        .query_map(params![like], doc_row_json)
        .map_err(e2s)?
        .filter_map(Result::ok)
        .collect();
    print_json(&Value::Array(rows));
    Ok(())
}

fn cmd_list(conn: &Connection, args: &[String]) -> Result<(), String> {
    let limit = limit_of(args, 200);
    let mut clauses = vec!["d.deleted_at IS NULL".to_string()];
    if flag_present(args, "--unread") {
        clauses.push("d.is_read = 0".into());
    }
    if flag_present(args, "--favorite") {
        clauses.push("d.favorite = 1".into());
    }
    let tag = flag_val(args, "--tag");
    if tag.is_some() {
        clauses.push(
            "EXISTS (SELECT 1 FROM document_tags dt JOIN tags t ON t.id = dt.tag_id \
             WHERE dt.document_id = d.id AND t.name = ?1 COLLATE NOCASE)"
                .into(),
        );
    }
    let sql = format!(
        "{DOC_SELECT} WHERE {} ORDER BY d.added_at DESC LIMIT {limit}",
        clauses.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql).map_err(e2s)?;
    let binds: Vec<String> = tag.into_iter().collect();
    let rows: Vec<Value> = stmt
        .query_map(params_from_iter(binds.iter()), doc_row_json)
        .map_err(e2s)?
        .filter_map(Result::ok)
        .collect();
    print_json(&Value::Array(rows));
    Ok(())
}

fn cmd_show(conn: &Connection, args: &[String]) -> Result<(), String> {
    let id: i64 = positional(args)
        .and_then(|s| s.parse().ok())
        .ok_or("show: missing or invalid document id")?;
    let base = conn
        .query_row(
            "SELECT id, title, year, venue, doi, citekey, is_read, favorite, language, \
                    page_count, abstract, summary, notes, github_url, added_at, \
                    (path NOT LIKE 'ref:%') AS has_pdf \
             FROM documents WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, Option<String>>(1)?,
                    "year": r.get::<_, Option<i64>>(2)?,
                    "venue": r.get::<_, Option<String>>(3)?,
                    "doi": r.get::<_, Option<String>>(4)?,
                    "citekey": r.get::<_, Option<String>>(5)?,
                    "read": r.get::<_, i64>(6)? != 0,
                    "favorite": r.get::<_, i64>(7)? != 0,
                    "language": r.get::<_, Option<String>>(8)?,
                    "page_count": r.get::<_, Option<i64>>(9)?,
                    "abstract": r.get::<_, Option<String>>(10)?,
                    "summary": r.get::<_, Option<String>>(11)?,
                    "notes": r.get::<_, Option<String>>(12)?,
                    "github_url": r.get::<_, Option<String>>(13)?,
                    "added_at": r.get::<_, Option<String>>(14)?,
                    "has_pdf": r.get::<_, i64>(15)? != 0,
                }))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => format!("show: no live document with id {id}"),
            other => other.to_string(),
        })?;

    let authors = collect_strings(
        conn,
        "SELECT TRIM(COALESCE(a.given,'')||' '||COALESCE(a.family,'')) \
         FROM document_authors da JOIN authors a ON a.id = da.author_id \
         WHERE da.document_id = ?1 ORDER BY da.position",
        id,
    )?;
    let tags = collect_strings(
        conn,
        "SELECT t.name FROM document_tags dt JOIN tags t ON t.id = dt.tag_id \
         WHERE dt.document_id = ?1 ORDER BY t.name",
        id,
    )?;
    let n_refs: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_references WHERE document_id = ?1", params![id], |r| r.get(0))
        .unwrap_or(0);
    let n_annots: i64 = conn
        .query_row("SELECT COUNT(*) FROM annotations WHERE document_id = ?1", params![id], |r| r.get(0))
        .unwrap_or(0);

    let mut out = base;
    out["authors"] = json!(authors);
    out["tags"] = json!(tags);
    out["n_references"] = json!(n_refs);
    out["n_annotations"] = json!(n_annots);
    print_json(&out);
    Ok(())
}

fn cmd_tags(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name, COUNT(dt.document_id) \
             FROM tags t \
             LEFT JOIN document_tags dt ON dt.tag_id = t.id \
             LEFT JOIN documents d ON d.id = dt.document_id AND d.deleted_at IS NULL \
             GROUP BY t.id ORDER BY 2 DESC, t.name COLLATE NOCASE",
        )
        .map_err(e2s)?;
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({ "name": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
        })
        .map_err(e2s)?
        .filter_map(Result::ok)
        .collect();
    print_json(&Value::Array(rows));
    Ok(())
}

fn cmd_stats(conn: &Connection) -> Result<(), String> {
    let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    let live = "FROM documents WHERE deleted_at IS NULL";
    let out = json!({
        "documents": one(&format!("SELECT COUNT(*) {live}")),
        "with_pdf": one(&format!("SELECT COUNT(*) {live} AND path NOT LIKE 'ref:%'")),
        "references_only": one(&format!("SELECT COUNT(*) {live} AND path LIKE 'ref:%'")),
        "unread": one(&format!("SELECT COUNT(*) {live} AND is_read = 0")),
        "favorite": one(&format!("SELECT COUNT(*) {live} AND favorite = 1")),
        "with_doi": one(&format!("SELECT COUNT(*) {live} AND doi IS NOT NULL")),
        "tags": one("SELECT COUNT(*) FROM tags"),
        "collections": one("SELECT COUNT(*) FROM collections"),
        "authors": one("SELECT COUNT(*) FROM authors"),
        "annotations": one("SELECT COUNT(*) FROM annotations"),
        "saved_searches": one("SELECT COUNT(*) FROM saved_searches"),
        "novita_new": one("SELECT COUNT(*) FROM watch_hits WHERE state = 'new'"),
    });
    print_json(&out);
    Ok(())
}

fn cmd_bib(conn: &Connection, args: &[String]) -> Result<(), String> {
    let mut clauses = vec!["d.deleted_at IS NULL".to_string()];
    let mut binds: Vec<String> = Vec::new();
    if let Some(ids) = flag_val(args, "--id") {
        let n: i64 = ids.parse().map_err(|_| "bib: --id expects a number")?;
        clauses.push(format!("d.id = {n}"));
    }
    if let Some(tag) = flag_val(args, "--tag") {
        clauses.push(
            "EXISTS (SELECT 1 FROM document_tags dt JOIN tags t ON t.id = dt.tag_id \
             WHERE dt.document_id = d.id AND t.name = ?1 COLLATE NOCASE)"
                .into(),
        );
        binds.push(tag);
    }
    let sql = format!(
        "SELECT d.id, d.title, d.year, d.venue, d.doi, d.citekey \
         FROM documents d WHERE {} ORDER BY d.year DESC, d.id",
        clauses.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql).map_err(e2s)?;
    let docs: Vec<(i64, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>)> = stmt
        .query_map(params_from_iter(binds.iter()), |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(e2s)?
        .filter_map(Result::ok)
        .collect();

    let mut out = String::new();
    for (id, title, year, venue, doi, citekey) in &docs {
        let authors = author_pairs(conn, *id)?;
        out.push_str(&bib_entry(*id, title, *year, venue, doi, citekey, &authors));
        out.push('\n');
    }
    print!("{out}");
    Ok(())
}

// ---- Appunti vault + LaTeX projects (read from the FILES, next to the DB) ---

/// The app-data directory: the vault and the projects live next to the DB file.
fn data_dir_of(db: &PathBuf) -> PathBuf {
    db.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}

fn epoch_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// List the .md notes vault: slug, title (first non-empty line, `#` stripped),
/// modification time (epoch seconds, newest first) and size.
fn cmd_notes(db: &PathBuf, args: &[String]) -> Result<(), String> {
    let limit = limit_of(args, 500) as usize;
    let dir = data_dir_of(db).join("notes");
    let mut rows: Vec<(u64, Value)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.filter_map(Result::ok) {
            let p = ent.path();
            let is_md = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_md || !p.is_file() {
                continue;
            }
            let Some(slug) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
                continue;
            };
            let meta = ent.metadata().ok();
            let modified = meta.as_ref().map(epoch_of).unwrap_or(0);
            let bytes = meta.map(|m| m.len()).unwrap_or(0);
            let title = std::fs::read_to_string(&p)
                .ok()
                .and_then(|body| {
                    body.lines()
                        .find(|l| !l.trim().is_empty())
                        .map(|l| l.trim_start_matches('#').trim().to_string())
                })
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| slug.clone());
            rows.push((
                modified,
                json!({ "slug": slug, "title": title, "modified_epoch": modified, "bytes": bytes }),
            ));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let out: Vec<Value> = rows.into_iter().take(limit).map(|(_, v)| v).collect();
    print_json(&Value::Array(out));
    Ok(())
}

/// Print one note's raw Markdown (the .md file is the source of truth).
fn cmd_note(db: &PathBuf, args: &[String]) -> Result<(), String> {
    let slug = positional(args).ok_or("note: missing slug (see `scriptorium-cli notes`)")?;
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") || slug.contains(':') {
        return Err("note: invalid slug".into());
    }
    let p = data_dir_of(db).join("notes").join(format!("{slug}.md"));
    let body = std::fs::read_to_string(&p).map_err(|_| format!("note: no note '{slug}' at {}", p.display()))?;
    print!("{body}");
    Ok(())
}

/// Case-insensitive char-wise find (1:1 lowercase map keeps indices aligned —
/// byte offsets from a lowercased COPY would corrupt multi-byte text).
fn find_ci(hay: &[char], needle: &str) -> Option<usize> {
    let low = |c: char| c.to_lowercase().next().unwrap_or(c);
    let n: Vec<char> = needle.chars().map(low).collect();
    if n.is_empty() || hay.len() < n.len() {
        return None;
    }
    let h: Vec<char> = hay.iter().map(|c| low(*c)).collect();
    h.windows(n.len()).position(|w| w == n.as_slice())
}

/// Search the notes' shadow table (title + body) and return a short excerpt
/// around the first body match.
fn cmd_search_notes(conn: &Connection, args: &[String]) -> Result<(), String> {
    let text = positional(args).ok_or("search-notes: missing search text")?;
    let limit = limit_of(args, 30);
    let like = format!("%{}%", text);
    let sql = format!("SELECT slug, title, COALESCE(body,'') FROM notes WHERE title LIKE ?1 OR body LIKE ?1 LIMIT {limit}");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("search-notes: {e} (serve un database creato da Scriptorium ≥ 0.8.7)"))?;
    let rows: Vec<Value> = stmt
        .query_map(params![like], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?))
        })
        .map_err(e2s)?
        .filter_map(Result::ok)
        .map(|(slug, title, body)| {
            let chars: Vec<char> = body.chars().collect();
            let excerpt = find_ci(&chars, &text).map(|i| {
                let start = i.saturating_sub(90);
                let end = (i + text.chars().count() + 90).min(chars.len());
                let s: String = chars[start..end].iter().collect();
                s.split_whitespace().collect::<Vec<_>>().join(" ")
            });
            json!({ "slug": slug, "title": title, "excerpt": excerpt })
        })
        .collect();
    print_json(&Value::Array(rows));
    Ok(())
}

/// List the LaTeX projects: real folders under projects/, newest first.
fn cmd_projects(db: &PathBuf, args: &[String]) -> Result<(), String> {
    let limit = limit_of(args, 200) as usize;
    let dir = data_dir_of(db).join("projects");
    let mut rows: Vec<(u64, Value)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.filter_map(Result::ok) {
            let p = ent.path();
            if !p.is_dir() {
                continue;
            }
            let Some(slug) = p.file_name().and_then(|s| s.to_str()).map(str::to_string) else {
                continue;
            };
            let modified = ent.metadata().ok().map(|m| epoch_of(&m)).unwrap_or(0);
            let has_main = p.join("main.tex").is_file();
            let has_bib = p.join("refs.bib").is_file();
            let has_pdf = p.join("main.pdf").is_file();
            rows.push((
                modified,
                json!({
                    "slug": slug,
                    "path": p.to_string_lossy(),
                    "modified_epoch": modified,
                    "has_main_tex": has_main,
                    "has_refs_bib": has_bib,
                    "has_pdf": has_pdf,
                }),
            ));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let out: Vec<Value> = rows.into_iter().take(limit).map(|(_, v)| v).collect();
    print_json(&Value::Array(out));
    Ok(())
}

// ---- small helpers ---------------------------------------------------------

fn collect_strings(conn: &Connection, sql: &str, id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(e2s)?;
    let rows: Vec<String> = stmt
        .query_map(params![id], |r| r.get::<_, String>(0))
        .map_err(e2s)?
        .filter_map(Result::ok)
        .filter(|s| !s.trim().is_empty())
        .collect();
    Ok(rows)
}

fn author_pairs(conn: &Connection, id: i64) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(a.family,''), COALESCE(a.given,'') \
             FROM document_authors da JOIN authors a ON a.id = da.author_id \
             WHERE da.document_id = ?1 ORDER BY da.position",
        )
        .map_err(e2s)?;
    let rows = stmt
        .query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(e2s)?
        .filter_map(Result::ok)
        .collect();
    Ok(rows)
}

/// Escape the small set of characters that break a `{…}` BibTeX field value.
fn bib_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' | '}' => {} // drop stray braces to keep the field balanced
            '&' | '%' | '$' | '#' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn alnum_lower(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Use the persisted citekey when present; otherwise a stable-ish fallback.
fn make_key(id: i64, citekey: &Option<String>, year: Option<i64>, first_family: Option<&str>, title: &Option<String>) -> String {
    if let Some(k) = citekey {
        if !k.trim().is_empty() {
            return k.clone();
        }
    }
    let fam = first_family.map(alnum_lower).filter(|s| !s.is_empty());
    let yr = year.map(|y| y.to_string());
    let word = title
        .as_deref()
        .and_then(|t| t.split_whitespace().find(|w| w.chars().filter(|c| c.is_alphanumeric()).count() > 3))
        .map(alnum_lower);
    match (fam, yr, word) {
        (Some(f), Some(y), Some(w)) => format!("{f}{y}{w}"),
        (Some(f), Some(y), None) => format!("{f}{y}"),
        (Some(f), None, _) => f,
        _ => format!("doc{id}"),
    }
}

fn bib_entry(
    id: i64,
    title: &Option<String>,
    year: Option<i64>,
    venue: &Option<String>,
    doi: &Option<String>,
    citekey: &Option<String>,
    authors: &[(String, String)],
) -> String {
    let first_family = authors.first().map(|(f, _)| f.as_str());
    let key = make_key(id, citekey, year, first_family, title);
    let kind = if venue.as_deref().map(|v| !v.trim().is_empty()).unwrap_or(false) {
        "article"
    } else {
        "misc"
    };
    let author_str = authors
        .iter()
        .map(|(fam, giv)| {
            let fam = fam.trim();
            let giv = giv.trim();
            if giv.is_empty() {
                bib_field(fam)
            } else {
                format!("{}, {}", bib_field(fam), bib_field(giv))
            }
        })
        .collect::<Vec<_>>()
        .join(" and ");

    let mut e = format!("@{kind}{{{key},\n");
    if let Some(t) = title {
        e.push_str(&format!("  title = {{{}}},\n", bib_field(t)));
    }
    if !author_str.is_empty() {
        e.push_str(&format!("  author = {{{author_str}}},\n"));
    }
    if let Some(y) = year {
        e.push_str(&format!("  year = {{{y}}},\n"));
    }
    if let Some(v) = venue {
        if !v.trim().is_empty() {
            e.push_str(&format!("  journal = {{{}}},\n", bib_field(v)));
        }
    }
    if let Some(d) = doi {
        if !d.trim().is_empty() {
            e.push_str(&format!("  doi = {{{}}},\n", bib_field(d)));
        }
    }
    e.push_str("}\n");
    e
}
