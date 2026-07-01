//! Browser connector: a tiny loopback HTTP server that lets a one-click
//! bookmarklet in the browser hand a PDF URL to Scriptorium.
//!
//! Security posture — this never does more than the in-app "Aggancia da URL":
//! - Bound to `127.0.0.1` only, so it is unreachable from the LAN.
//! - Every request (except the mandatory CORS/PNA preflight) must carry a
//!   per-install secret token in the `X-Scriptorium-Token` header — kept OUT of
//!   the URL so it can't leak through the Resource Timing API / logs / Referer.
//!   A random web page doesn't have the token, so it can't drive imports.
//! - The `Host` header must be loopback (defense-in-depth vs DNS-rebinding).
//! - Unauthenticated requests get a bare `403` with no app identity and no CORS
//!   headers (so a page can't read the body to fingerprint the install).
//! - The download itself goes through the same SSRF-guarded pipeline
//!   ([`crate::commands::import_from_url`] → `download_pdf`): https public hosts
//!   only, `%PDF`-gated, size-capped. Each `/add` runs on its own thread so a
//!   slow download can't wedge the accept loop.

use std::io::Cursor;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Response, Server};

/// The header the bookmarklet presents its secret token in.
const TOKEN_HEADER: &str = "X-Scriptorium-Token";

/// Default port for the connector. If taken, [`bind`] scans a few above it and
/// the actually-bound port is persisted so the generated bookmarklet matches.
pub const DEFAULT_PORT: u16 = 8765;

/// A running connector. Dropping it (or calling [`ConnectorHandle::stop`]) frees
/// the loopback socket and lets the server thread exit.
pub struct ConnectorHandle {
    pub port: u16,
    server: Arc<Server>,
}

impl ConnectorHandle {
    pub fn stop(&self) {
        // Unblocks the `incoming_requests()` loop so the serve thread returns.
        self.server.unblock();
    }
}

fn hdr(k: &str, v: &str) -> Header {
    // Names/values here are all static ASCII, so this never fails.
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("static header")
}

/// Add the CORS + Private-Network-Access headers a bookmarklet fetch from an
/// `https://` page to `http://127.0.0.1` needs (only used on *authorized* /
/// preflight responses, never on denials).
fn cors(resp: Response<Cursor<Vec<u8>>>) -> Response<Cursor<Vec<u8>>> {
    resp.with_header(hdr("Access-Control-Allow-Origin", "*"))
        .with_header(hdr("Access-Control-Allow-Methods", "GET, OPTIONS"))
        .with_header(hdr("Access-Control-Allow-Headers", "*"))
        // Chrome's Private Network Access: allow a public page to reach loopback.
        .with_header(hdr("Access-Control-Allow-Private-Network", "true"))
        .with_header(hdr("Content-Type", "application/json; charset=utf-8"))
}

fn json(status: u16, body: &str) -> Response<Cursor<Vec<u8>>> {
    cors(Response::from_string(body.to_string()).with_status_code(status))
}

/// A bare denial: no CORS headers (so a cross-origin page can't read it) and no
/// app identity — minimizing what an unauthorized caller can learn.
fn deny(status: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(String::new()).with_status_code(status)
}

/// The value of the first header matching `name` (case-insensitive).
fn header_value(req: &tiny_http::Request, name: &str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| format!("{}", h.field).eq_ignore_ascii_case(name))
        .map(|h| format!("{}", h.value))
}

/// Constant-time string comparison for the secret token.
fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Bind loopback on the preferred port, else scan a small range above it.
fn bind(preferred: u16) -> Option<(Server, u16)> {
    let end = preferred.saturating_add(12);
    for port in preferred..=end {
        if let Ok(s) = Server::http(("127.0.0.1", port)) {
            return Some((s, port));
        }
    }
    None
}

/// Start the connector. Returns `None` if no loopback port could be bound.
pub fn start(app: AppHandle, preferred: u16, token: String) -> Option<ConnectorHandle> {
    let (server, port) = bind(preferred)?;
    let server = Arc::new(server);
    let srv = server.clone();
    let token = Arc::new(token);
    std::thread::spawn(move || {
        for request in srv.incoming_requests() {
            handle(&app, &token, request);
        }
    });
    Some(ConnectorHandle { port, server })
}

fn handle(app: &AppHandle, token: &Arc<String>, request: tiny_http::Request) {
    // Preflight (CORS + Private Network Access) — must be answered without a
    // token (preflights never carry custom headers). Reveals nothing but that
    // a CORS server exists, which the PNA handshake requires anyway.
    if request.method() == &Method::Options {
        let _ = request.respond(json(204, ""));
        return;
    }

    // --- Authenticate every real request FIRST (before routing) ---
    // Defense-in-depth vs DNS-rebinding: the Host must be loopback. (A rebinding
    // fetch to evil.com→127.0.0.1 still sends `Host: evil.com`; JS can't forge
    // the Host header.)
    let host_ok = header_value(&request, "Host")
        .map(|h| {
            let h = h.to_ascii_lowercase();
            h.starts_with("127.0.0.1") || h.starts_with("localhost")
        })
        .unwrap_or(false);
    let token_ok = header_value(&request, TOKEN_HEADER)
        .map(|t| ct_eq(&t, token))
        .unwrap_or(false);
    if !host_ok || !token_ok {
        let _ = request.respond(deny(403));
        return;
    }

    // Parse path + the (non-secret) `url` param with the same URL engine reqwest
    // uses (percent-decodes what the bookmarklet encoded with encodeURIComponent).
    let raw = request.url().to_string(); // e.g. "/add?url=.."
    let parsed = reqwest::Url::parse(&format!("http://127.0.0.1{raw}")).ok();
    let path = parsed.as_ref().map(|u| u.path().to_string()).unwrap_or_default();
    let mut target = String::new();
    if let Some(u) = &parsed {
        for (k, v) in u.query_pairs() {
            if k.as_ref() == "url" {
                target = v.into_owned();
            }
        }
    }

    if path == "/ping" {
        // Authenticated liveness check (no app identity for anyone else).
        let _ = request.respond(json(200, r#"{"ok":true}"#));
        return;
    }
    if path != "/add" {
        let _ = request.respond(json(404, r#"{"ok":false,"error":"not_found"}"#));
        return;
    }
    if target.trim().is_empty() {
        let _ = request.respond(json(400, r#"{"ok":false,"error":"missing_url"}"#));
        return;
    }

    // Run the (slow) download+import on its own thread so it can't wedge the
    // accept loop; reuse the exact in-app engine (SSRF-guarded → import → enrich).
    let app = app.clone();
    std::thread::spawn(move || {
        let status = match tauri::async_runtime::block_on(crate::commands::import_from_url(&app, &target)) {
            Ok(s) => s,
            Err(_) => "error",
        };
        // Nudge the UI (toast + refresh already happen via `library-changed`).
        let _ = app.emit("connector-added", status);
        let body = format!(r#"{{"ok":true,"status":"{status}"}}"#);
        let _ = request.respond(json(200, &body));
    });
}
