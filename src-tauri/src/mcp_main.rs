//! scriptorium-mcp — a local, **read-only** MCP (Model Context Protocol) server
//! over the Scriptorium library, for Claude Desktop / Claude Code and any MCP
//! client. Twin of `scriptorium-cli`: same SQLite file opened READ_ONLY (safe
//! while the app runs — WAL allows concurrent readers), notes and LaTeX
//! projects read from the real files next to the database. It NEVER writes.
//!
//! Transport: MCP **stdio** — one JSON-RPC 2.0 message per line on stdin/stdout
//! (logs go to stderr; stdout carries protocol messages ONLY). The client
//! spawns this binary on demand, so there is nothing to keep running.
//!
//! Gated behind the `mcp` cargo feature so the GUI release never compiles it:
//!     cargo build --release --bin scriptorium-mcp --features mcp
//!
//! Register with Claude Code:
//!     claude mcp add scriptorium -- "<path>\scriptorium-mcp.exe"

use rusqlite::{params, params_from_iter, Connection, OpenFlags, Row};
use serde_json::{json, Value};
use std::env;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const PROTOCOL_FALLBACK: &str = "2024-11-05";

fn main() -> ExitCode {
    // Optional `--db <path>` anywhere in the argument list.
    let raw: Vec<String> = env::args().skip(1).collect();
    let mut db_override: Option<String> = None;
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == "--db" && i + 1 < raw.len() {
            db_override = Some(raw[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    let db_path = match db_override.map(PathBuf::from).or_else(default_db_path) {
        Some(p) => p,
        None => {
            eprintln!("scriptorium-mcp: cannot locate the database (%APPDATA% unset); pass --db <path>");
            return ExitCode::FAILURE;
        }
    };

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            eprintln!("scriptorium-mcp: skipping unparseable line");
            continue;
        };
        // Notifications (no id) never get a response.
        let Some(id) = msg.get("id").filter(|v| !v.is_null()).cloned() else {
            continue;
        };
        let method = msg["method"].as_str().unwrap_or("");
        let reply = match method {
            "initialize" => {
                let requested = msg["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL_FALLBACK);
                Ok(json!({
                    "protocolVersion": requested,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "scriptorium-mcp", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Read-only access to a local Scriptorium research library (papers, BibTeX, Markdown notes, LaTeX projects). Safe to call while the app is open. Text results are JSON."
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_defs() })),
            "tools/call" => handle_call(&db_path, &msg["params"]),
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        let out = match reply {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            }
        };
        let Ok(s) = serde_json::to_string(&out) else { continue };
        if writeln!(stdout, "{s}").is_err() {
            break; // client went away
        }
        let _ = stdout.flush();
    }
    ExitCode::SUCCESS
}

// ---- MCP plumbing -----------------------------------------------------------

/// Tool declarations (names, English descriptions for any client, JSON Schemas).
fn tool_defs() -> Value {
    json!([
        {
            "name": "search_library",
            "description": "Search the paper library by text (title, venue, DOI, citekey, author). Returns matching documents as JSON.",
            "inputSchema": { "type": "object", "properties": {
                "query": { "type": "string", "description": "Search text" },
                "limit": { "type": "integer", "description": "Max results (default 50)" }
            }, "required": ["query"] }
        },
        {
            "name": "list_documents",
            "description": "List documents in the library, optionally filtered by tag / unread / favorite. Newest first.",
            "inputSchema": { "type": "object", "properties": {
                "tag": { "type": "string", "description": "Only documents with this tag (case-insensitive)" },
                "unread": { "type": "boolean", "description": "Only unread documents" },
                "favorite": { "type": "boolean", "description": "Only favorites" },
                "limit": { "type": "integer", "description": "Max results (default 200)" }
            } }
        },
        {
            "name": "get_document",
            "description": "Full record of one document by id: authors, tags, abstract, AI summary, notes, counts.",
            "inputSchema": { "type": "object", "properties": {
                "id": { "type": "integer", "description": "Document id (from search/list)" }
            }, "required": ["id"] }
        },
        {
            "name": "get_bibtex",
            "description": "BibTeX entries for the whole library, one tag, or one document id. Returns raw .bib text.",
            "inputSchema": { "type": "object", "properties": {
                "tag": { "type": "string", "description": "Only documents with this tag" },
                "id": { "type": "integer", "description": "Only this document id" }
            } }
        },
        {
            "name": "list_notes",
            "description": "List the Markdown notes vault (slug, title, modified epoch, size), newest first.",
            "inputSchema": { "type": "object", "properties": {
                "limit": { "type": "integer", "description": "Max results (default 500)" }
            } }
        },
        {
            "name": "get_note",
            "description": "Raw Markdown of one note by slug (see list_notes).",
            "inputSchema": { "type": "object", "properties": {
                "slug": { "type": "string", "description": "Note slug (filename without .md)" }
            }, "required": ["slug"] }
        },
        {
            "name": "search_notes",
            "description": "Search the notes (title + body); each hit includes a short excerpt around the first match.",
            "inputSchema": { "type": "object", "properties": {
                "query": { "type": "string", "description": "Search text" },
                "limit": { "type": "integer", "description": "Max results (default 30)" }
            }, "required": ["query"] }
        },
        {
            "name": "list_projects",
            "description": "List the LaTeX projects (real folders): slug, path, whether main.tex / refs.bib / compiled PDF exist.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "library_stats",
            "description": "Library counters: documents, PDFs vs reference-only, unread, favorites, tags, notes-adjacent tables.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

/// Run one tool. Tool-level failures come back as a normal result with
/// isError=true (protocol errors are reserved for unknown tools/params).
fn handle_call(db: &PathBuf, params: &Value) -> Result<Value, (i64, String)> {
    let name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];
    let outcome: Result<String, String> = match name {
        "search_library" => tool_search_library(db, args),
        "list_documents" => tool_list_documents(db, args),
        "get_document" => tool_get_document(db, args),
        "get_bibtex" => tool_get_bibtex(db, args),
        "list_notes" => tool_list_notes(db, args),
        "get_note" => tool_get_note(db, args),
        "search_notes" => tool_search_notes(db, args),
        "list_projects" => tool_list_projects(db),
        "library_stats" => tool_library_stats(db),
        other => return Err((-32602, format!("unknown tool: {other}"))),
    };
    Ok(match outcome {
        Ok(text) => json!({ "content": [ { "type": "text", "text": text } ], "isError": false }),
        Err(e) => json!({ "content": [ { "type": "text", "text": e } ], "isError": true }),
    })
}

// ---- shared helpers (twin of cli_main.rs, returning strings) ----------------

fn default_db_path() -> Option<PathBuf> {
    let appdata = env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("com.pdfmanage.app");
    p.push("pdfmanage.db");
    Some(p)
}

/// Fresh READ_ONLY connection per call: never writes, safe next to the running
/// app (WAL), and robust to the database appearing after the server started.
fn open_ro(path: &PathBuf) -> Result<Connection, String> {
    if !path.exists() {
        return Err(format!("no Scriptorium database at {} (open the app once first)", path.display()));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(e2s)?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(e2s)?;
    Ok(conn)
}

fn data_dir_of(db: &PathBuf) -> PathBuf {
    db.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| "null".into())
}

fn arg_limit(args: &Value, default: i64) -> i64 {
    args["limit"].as_i64().filter(|n| *n > 0 && *n <= 100_000).unwrap_or(default)
}

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

// ---- tools -------------------------------------------------------------------

fn tool_search_library(db: &PathBuf, args: &Value) -> Result<String, String> {
    let text = args["query"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or("query is required")?;
    let limit = arg_limit(args, 50);
    let conn = open_ro(db)?;
    let like = format!("%{text}%");
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
    Ok(pretty(&Value::Array(rows)))
}

fn tool_list_documents(db: &PathBuf, args: &Value) -> Result<String, String> {
    let limit = arg_limit(args, 200);
    let conn = open_ro(db)?;
    let mut clauses = vec!["d.deleted_at IS NULL".to_string()];
    if args["unread"].as_bool() == Some(true) {
        clauses.push("d.is_read = 0".into());
    }
    if args["favorite"].as_bool() == Some(true) {
        clauses.push("d.favorite = 1".into());
    }
    let tag = args["tag"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
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
    Ok(pretty(&Value::Array(rows)))
}

fn tool_get_document(db: &PathBuf, args: &Value) -> Result<String, String> {
    let id = args["id"].as_i64().ok_or("id is required")?;
    let conn = open_ro(db)?;
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
            rusqlite::Error::QueryReturnedNoRows => format!("no live document with id {id}"),
            other => other.to_string(),
        })?;
    let strings = |sql: &str| -> Result<Vec<String>, String> {
        let mut stmt = conn.prepare(sql).map_err(e2s)?;
        let rows: Vec<String> = stmt
            .query_map(params![id], |r| r.get::<_, String>(0))
            .map_err(e2s)?
            .filter_map(Result::ok)
            .filter(|s| !s.trim().is_empty())
            .collect();
        Ok(rows)
    };
    let authors = strings(
        "SELECT TRIM(COALESCE(a.given,'')||' '||COALESCE(a.family,'')) \
         FROM document_authors da JOIN authors a ON a.id = da.author_id \
         WHERE da.document_id = ?1 ORDER BY da.position",
    )?;
    let tags = strings(
        "SELECT t.name FROM document_tags dt JOIN tags t ON t.id = dt.tag_id \
         WHERE dt.document_id = ?1 ORDER BY t.name",
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
    Ok(pretty(&out))
}

fn tool_get_bibtex(db: &PathBuf, args: &Value) -> Result<String, String> {
    let conn = open_ro(db)?;
    let mut clauses = vec!["d.deleted_at IS NULL".to_string()];
    let mut binds: Vec<String> = Vec::new();
    if let Some(n) = args["id"].as_i64() {
        clauses.push(format!("d.id = {n}"));
    }
    if let Some(tag) = args["tag"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        clauses.push(
            "EXISTS (SELECT 1 FROM document_tags dt JOIN tags t ON t.id = dt.tag_id \
             WHERE dt.document_id = d.id AND t.name = ?1 COLLATE NOCASE)"
                .into(),
        );
        binds.push(tag.to_string());
    }
    let sql = format!(
        "SELECT d.id, d.title, d.year, d.venue, d.doi, d.citekey \
         FROM documents d WHERE {} ORDER BY d.year DESC, d.id",
        clauses.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql).map_err(e2s)?;
    let docs: Vec<(i64, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>)> = stmt
        .query_map(params_from_iter(binds.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .map_err(e2s)?
        .filter_map(Result::ok)
        .collect();
    let mut out = String::new();
    for (id, title, year, venue, doi, citekey) in &docs {
        let authors = author_pairs(&conn, *id)?;
        out.push_str(&bib_entry(*id, title, *year, venue, doi, citekey, &authors));
        out.push('\n');
    }
    Ok(out)
}

fn tool_list_notes(db: &PathBuf, args: &Value) -> Result<String, String> {
    let limit = arg_limit(args, 500) as usize;
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
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
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
    Ok(pretty(&Value::Array(out)))
}

fn tool_get_note(db: &PathBuf, args: &Value) -> Result<String, String> {
    let slug = args["slug"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or("slug is required")?;
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") || slug.contains(':') {
        return Err("invalid slug".into());
    }
    let p = data_dir_of(db).join("notes").join(format!("{slug}.md"));
    std::fs::read_to_string(&p).map_err(|_| format!("no note '{slug}' at {}", p.display()))
}

/// Case-insensitive char-wise find (1:1 lowercase map keeps indices aligned).
fn find_ci(hay: &[char], needle: &str) -> Option<usize> {
    let low = |c: char| c.to_lowercase().next().unwrap_or(c);
    let n: Vec<char> = needle.chars().map(low).collect();
    if n.is_empty() || hay.len() < n.len() {
        return None;
    }
    let h: Vec<char> = hay.iter().map(|c| low(*c)).collect();
    h.windows(n.len()).position(|w| w == n.as_slice())
}

fn tool_search_notes(db: &PathBuf, args: &Value) -> Result<String, String> {
    let text = args["query"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or("query is required")?;
    let limit = arg_limit(args, 30);
    let conn = open_ro(db)?;
    let like = format!("%{text}%");
    let sql = format!("SELECT slug, title, COALESCE(body,'') FROM notes WHERE title LIKE ?1 OR body LIKE ?1 LIMIT {limit}");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("{e} (requires a database created by Scriptorium >= 0.8.7)"))?;
    let rows: Vec<Value> = stmt
        .query_map(params![like], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?))
        })
        .map_err(e2s)?
        .filter_map(Result::ok)
        .map(|(slug, title, body)| {
            let chars: Vec<char> = body.chars().collect();
            let excerpt = find_ci(&chars, text).map(|i| {
                let start = i.saturating_sub(90);
                let end = (i + text.chars().count() + 90).min(chars.len());
                let s: String = chars[start..end].iter().collect();
                s.split_whitespace().collect::<Vec<_>>().join(" ")
            });
            json!({ "slug": slug, "title": title, "excerpt": excerpt })
        })
        .collect();
    Ok(pretty(&Value::Array(rows)))
}

fn tool_list_projects(db: &PathBuf) -> Result<String, String> {
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
            let modified = ent
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            rows.push((
                modified,
                json!({
                    "slug": slug,
                    "path": p.to_string_lossy(),
                    "modified_epoch": modified,
                    "has_main_tex": p.join("main.tex").is_file(),
                    "has_refs_bib": p.join("refs.bib").is_file(),
                    "has_pdf": p.join("main.pdf").is_file(),
                }),
            ));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let out: Vec<Value> = rows.into_iter().map(|(_, v)| v).collect();
    Ok(pretty(&Value::Array(out)))
}

fn tool_library_stats(db: &PathBuf) -> Result<String, String> {
    let conn = open_ro(db)?;
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
    Ok(pretty(&out))
}

// ---- BibTeX rendering (twin of cli_main.rs) ----------------------------------

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

fn bib_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' | '}' => {}
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
