//! Progetti LaTeX: real folders under `app_data_dir/projects/<slug>/` — plain
//! `.tex`/`.bib` files the user owns (same philosophy as the notes vault), with
//! a `refs.bib` synced from the library and compilation via a SYSTEM-installed
//! toolchain (Tectonic preferred, latexmk fallback). Nothing is bundled: LaTeX
//! distributions weigh gigabytes; without one, the editor + citations + bib
//! sync still work and the user compiles elsewhere.

use anyhow::{anyhow, Result};
use std::path::Path;

/// Starter document: minimal, Italian-friendly, wired to `refs.bib` so the
/// library citations work out of the box.
pub const MAIN_TEMPLATE: &str = r#"\documentclass[11pt]{article}
\usepackage[utf8]{inputenc}
\usepackage[T1]{fontenc}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{booktabs}
\usepackage[hidelinks]{hyperref}

\title{Titolo del lavoro}
\author{}
\date{\today}

\begin{document}
\maketitle

\begin{abstract}
Scrivi qui l'abstract.
\end{abstract}

\section{Introduzione}
Scrivi qui. Le citazioni vengono dalla tua libreria: premi «Cita» nell'editor
per inserire \verb|\cite{...}| con le citekey dei tuoi paper.

\bibliographystyle{plain}
\bibliography{refs}
\end{document}
"#;

/// A project's identity, derived from its folder.
#[derive(serde::Serialize)]
pub struct ProjectMeta {
    pub slug: String,
    pub name: String,
    /// mtime of the most recently modified file, epoch ms.
    pub updated_at: Option<i64>,
}

/// One file inside a project (relative path, forward slashes).
#[derive(serde::Serialize)]
pub struct ProjectFile {
    pub rel: String,
    pub size: u64,
}

/// Reject anything that could escape the project folder. Relative, forward
/// slashes only, no `..`, no drive letters, no leading separator.
pub fn safe_rel(rel: &str) -> Result<String> {
    let r = rel.replace('\\', "/");
    if r.is_empty()
        || r.starts_with('/')
        || r.contains(':')
        || r.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(anyhow!("percorso non valido: {rel}"));
    }
    Ok(r)
}

/// List the projects (folders) under `dir`, newest activity first.
pub fn list(dir: &Path) -> Vec<ProjectMeta> {
    let mut out: Vec<ProjectMeta> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let slug = e.file_name().to_string_lossy().to_string();
        if !p.join("main.tex").exists() {
            continue; // not a project folder
        }
        let updated_at = newest_mtime(&p);
        out.push(ProjectMeta { name: slug.replace('-', " "), slug, updated_at });
    }
    out.sort_by_key(|m| std::cmp::Reverse(m.updated_at));
    out
}

fn newest_mtime(dir: &Path) -> Option<i64> {
    let mut best: Option<std::time::SystemTime> = None;
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        if let Ok(md) = e.metadata() {
            if md.is_file() {
                if let Ok(t) = md.modified() {
                    if best.map(|b| t > b).unwrap_or(true) {
                        best = Some(t);
                    }
                }
            }
        }
    }
    best.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

/// The files worth showing in the sidebar: sources and figures, not build junk.
pub fn files(root: &Path) -> Vec<ProjectFile> {
    let mut out = Vec::new();
    collect_files(root, root, &mut out, 0);
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<ProjectFile>, depth: u8) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !name.starts_with('.') && name != "_build" {
                collect_files(root, &p, out, depth + 1);
            }
            continue;
        }
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        const SHOW: [&str; 12] =
            ["tex", "bib", "sty", "cls", "bst", "pdf", "png", "jpg", "jpeg", "svg", "txt", "md"];
        // Build byproducts (aux/log/…) stay hidden; the compiled PDF is shown.
        if !SHOW.contains(&ext.as_str()) {
            continue;
        }
        if let Ok(rel) = p.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(ProjectFile { rel, size });
        }
    }
}

/// Outcome of a compilation attempt.
#[derive(serde::Serialize)]
pub struct CompileResult {
    pub ok: bool,
    /// Which tool ran ("tectonic" / "latexmk"), empty if none was found.
    pub tool: String,
    /// Tail of the combined output (the part with the error, when it fails).
    pub log: String,
    /// Relative path of the produced PDF when ok.
    pub pdf_rel: Option<String>,
}

/// Run the system LaTeX toolchain on `main.tex` in `root`. Blocking (call from
/// `spawn_blocking`); enforces a wall-clock timeout by killing the process.
pub fn compile(root: &Path) -> CompileResult {
    // Preferred: Tectonic — single binary, fetches packages on demand, runs
    // bibtex/reruns automatically. Fallback: latexmk (TeX Live / MiKTeX).
    let attempts: [(&str, Vec<&str>); 2] = [
        ("tectonic", vec!["main.tex"]),
        ("latexmk", vec!["-pdf", "-interaction=nonstopmode", "-halt-on-error", "main.tex"]),
    ];
    for (tool, args) in attempts {
        match run_with_timeout(tool, &args, root, std::time::Duration::from_secs(300)) {
            Ok((status_ok, output)) => {
                let pdf = root.join("main.pdf");
                let ok = status_ok && pdf.exists();
                return CompileResult {
                    ok,
                    tool: tool.to_string(),
                    log: tail(&output, 4000),
                    pdf_rel: if ok { Some("main.pdf".into()) } else { None },
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return CompileResult {
                    ok: false,
                    tool: tool.to_string(),
                    log: format!("avvio {tool}: {e}"),
                    pdf_rel: None,
                }
            }
        }
    }
    CompileResult {
        ok: false,
        tool: String::new(),
        log: "Nessun compilatore LaTeX trovato. Installa Tectonic (consigliato, un solo \
              eseguibile che scarica i pacchetti da solo):\n  winget install Tectonic.Tectonic\n\
              oppure una distribuzione TeX con latexmk (MiKTeX / TeX Live). Poi riprova."
            .to_string(),
        pdf_rel: None,
    }
}

/// Spawn `tool` hidden, capture stdout+stderr, kill after `limit`.
fn run_with_timeout(
    tool: &str,
    args: &[&str],
    cwd: &Path,
    limit: std::time::Duration,
) -> std::io::Result<(bool, String)> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(tool);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn()?;
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    // Drain pipes on threads so a chatty compiler can't deadlock on full buffers.
    let out_h = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let err_h = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            let mut log = out_h.join().unwrap_or_default();
            log.push_str(&err_h.join().unwrap_or_default());
            log.push_str("\n[interrotto: superato il tempo massimo di compilazione]");
            return Ok((false, log));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    };
    let mut log = out_h.join().unwrap_or_default();
    log.push_str(&err_h.join().unwrap_or_default());
    Ok((status.success(), log))
}

fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let cut = s.len() - max;
    let start = s
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= cut)
        .unwrap_or(0);
    format!("…{}", &s[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_paths_are_guarded() {
        assert!(safe_rel("main.tex").is_ok());
        assert!(safe_rel("figures/plot.png").is_ok());
        assert_eq!(safe_rel("a\\b.tex").unwrap(), "a/b.tex");
        assert!(safe_rel("../evil").is_err());
        assert!(safe_rel("a/../b").is_err());
        assert!(safe_rel("/abs").is_err());
        assert!(safe_rel("C:/x").is_err());
        assert!(safe_rel("").is_err());
        assert!(safe_rel("a//b").is_err());
    }

    #[test]
    fn log_tail_respects_utf8() {
        let s = "è".repeat(3000);
        let t = tail(&s, 100);
        assert!(t.chars().count() <= 60);
        assert!(t.starts_with('…'));
    }
}
