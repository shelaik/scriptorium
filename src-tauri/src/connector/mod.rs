//! Browser connector: a tiny loopback HTTP server that lets a one-click
//! bookmarklet in the browser hand a PDF URL to Scriptorium.
//!
//! Security posture — this never does more than the in-app "Aggancia da URL":
//! - Bound to `127.0.0.1` only, so it is unreachable from the LAN.
//! - Every request (except the mandatory CORS/PNA preflight and the static
//!   `/grab` page below) must carry a per-install secret token in the
//!   `X-Scriptorium-Token` header — kept OUT of the URL so it can't leak
//!   through the Resource Timing API / logs / Referer. A random web page
//!   doesn't have the token, so it can't drive imports.
//! - The `Host` header must be loopback (defense-in-depth vs DNS-rebinding).
//! - Unauthenticated requests get a bare `403` with no app identity and no CORS
//!   headers (so a page can't read the body to fingerprint the install).
//! - The download itself goes through the same SSRF-guarded pipeline
//!   ([`crate::commands::import_from_url`] → `download_pdf`): https public hosts
//!   only, `%PDF`-gated, size-capped. Each `/add` runs on its own thread so a
//!   slow download can't wedge the accept loop.
//!
//! CSP fallback (`/grab`): sites with a strict `connect-src` (e.g. github.com)
//! block the bookmarklet's `fetch` to loopback, so the bookmarklet falls back
//! to `window.open("…/grab#u=<url>&t=<token>")` — top-level navigation is not
//! subject to `connect-src`. The URL and token travel in the **fragment**,
//! which never leaves the browser (not sent on the wire, no Referer), and the
//! page immediately strips it via `history.replaceState`. `/grab` itself is a
//! static, unauthenticated, CORS-less HTML page (nothing to read cross-origin;
//! `X-Frame-Options: DENY` prevents embedding); the actual import still goes
//! through the token-gated `/add` via a same-origin fetch from that page, so
//! the authentication model is unchanged.

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

/// The static `/grab` page: reads `#u=<url>&t=<token>` from the fragment,
/// strips it from the address bar, then performs the authenticated same-origin
/// `/add` call and shows the outcome. Served without CORS and non-embeddable.
const GRAB_PAGE: &str = r##"<!doctype html><html lang="it"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="referrer" content="no-referrer">
<title>Scriptorium — aggancio PDF</title>
<style>
  body{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;
    background:#f6f2e9;color:#2c2e35;font:15px/1.5 system-ui,sans-serif}
  .card{background:#fffdf8;border:1px solid #e2dccd;border-radius:16px;
    box-shadow:0 22px 60px rgba(20,22,28,.18);padding:34px 40px;max-width:480px;text-align:center}
  h1{font:600 20px/1.3 Georgia,'Times New Roman',serif;margin:0 0 6px;color:#2b4a78}
  p{margin:6px 0;color:#63666e;font-size:13px}
  .st{font-size:17px;font-weight:600;color:#2c2e35;margin:14px 0}
  .ok{color:#1f7a45}.err{color:#b0322a}
  .spin{display:inline-block;width:18px;height:18px;border:2.5px solid #d6e0ef;
    border-top-color:#2b4a78;border-radius:50%;animation:r .8s linear infinite;vertical-align:-4px;margin-right:8px}
  @keyframes r{to{transform:rotate(360deg)}}
  .u{font-size:11.5px;color:#8c8f97;word-break:break-all;margin-top:14px}
</style></head><body><div class="card">
<h1>Scriptorium</h1>
<div class="st" id="st"><span class="spin"></span>Scarico e importo il PDF…</div>
<p>Il sito di partenza blocca le richieste dirette al connettore (CSP), quindi l'aggancio continua qui. A PDF importato puoi chiudere questa scheda.</p>
<div class="u" id="u"></div>
<script>
(function(){
  var h = location.hash.slice(1);
  history.replaceState(null, "", location.pathname); // niente token nella barra o nello storico di sessione
  var q = {};
  h.split("&").forEach(function(kv){ var i = kv.indexOf("="); if (i > 0) q[kv.slice(0, i)] = decodeURIComponent(kv.slice(i + 1)); });
  var st = document.getElementById("st"), u = document.getElementById("u");
  function done(msg, cls, close){
    st.className = "st " + cls; st.textContent = msg;
    if (close) setTimeout(function(){ window.close(); }, 2500);
  }
  if (!q.u || !q.t) { done("Link non valido: riprova dal bookmarklet.", "err"); return; }
  u.textContent = q.u;
  var L = { added: "PDF agganciato ✓", duplicate: "Già in libreria", not_pdf: "Il link non è un PDF diretto", error: "Errore durante il download" };
  fetch("/add?url=" + encodeURIComponent(q.u), { headers: { "X-Scriptorium-Token": q.t } })
    .then(function(r){ return r.json(); })
    .then(function(j){
      var ok = j.status === "added" || j.status === "duplicate";
      done(L[j.status] || j.status, ok ? "ok" : "err", j.status === "added");
    })
    .catch(function(){ done("Scriptorium non risponde: l'app è chiusa?", "err"); });
})();
</script></div></body></html>"##;

fn grab_page() -> Response<Cursor<Vec<u8>>> {
    Response::from_string(GRAB_PAGE.to_string())
        .with_status_code(200)
        .with_header(hdr("Content-Type", "text/html; charset=utf-8"))
        .with_header(hdr("X-Frame-Options", "DENY"))
        .with_header(hdr("Referrer-Policy", "no-referrer"))
        .with_header(hdr("Cache-Control", "no-store"))
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

    // Defense-in-depth vs DNS-rebinding: the Host must be loopback. (A rebinding
    // fetch to evil.com→127.0.0.1 still sends `Host: evil.com`; JS can't forge
    // the Host header.) Enforced for EVERY route, /grab included — otherwise a
    // rebound origin could read the static page and fingerprint the install.
    let host_ok = header_value(&request, "Host")
        .map(|h| {
            let h = h.to_ascii_lowercase();
            h.starts_with("127.0.0.1") || h.starts_with("localhost")
        })
        .unwrap_or(false);
    if !host_ok {
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

    // The CSP-fallback landing page: static, tokenless (credentials arrive in the
    // URL fragment, which the browser never sends to us — the page itself makes
    // the authenticated /add call). Everything else still requires the token.
    if request.method() == &Method::Get && path == "/grab" {
        let _ = request.respond(grab_page());
        return;
    }

    let token_ok = header_value(&request, TOKEN_HEADER)
        .map(|t| ct_eq(&t, token))
        .unwrap_or(false);
    if !token_ok {
        let _ = request.respond(deny(403));
        return;
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
        crate::pulse::start(&app, "browser", "Import dal browser");
        let status = match tauri::async_runtime::block_on(crate::commands::import_from_url(&app, &target)) {
            Ok(s) => {
                crate::pulse::ok(&app, "browser", "Import dal browser", s);
                s
            }
            Err(e) => {
                crate::pulse::err(&app, "browser", "Import dal browser", &e.to_string());
                "error"
            }
        };
        // Nudge the UI (toast + refresh already happen via `library-changed`).
        let _ = app.emit("connector-added", status);
        let body = format!(r#"{{"ok":true,"status":"{status}"}}"#);
        let _ = request.respond(json(200, &body));
    });
}
