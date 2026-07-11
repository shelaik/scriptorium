//! Progetti LaTeX: real folders under `app_data_dir/projects/<slug>/` — plain
//! `.tex`/`.bib` files the user owns (same philosophy as the notes vault), with
//! a `refs.bib` synced from the library and compilation via a SYSTEM-installed
//! toolchain (Tectonic preferred, latexmk fallback). Nothing is bundled: LaTeX
//! distributions weigh gigabytes; without one, the editor + citations + bib
//! sync still work and the user compiles elsewhere.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

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

/// Two-column conference-style paper.
pub const PAPER_TEMPLATE: &str = r#"\documentclass[10pt,twocolumn]{article}
\usepackage[utf8]{inputenc}
\usepackage[T1]{fontenc}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{booktabs}
\usepackage[hidelinks]{hyperref}
\usepackage[margin=2cm]{geometry}

\title{Titolo del paper}
\author{Autore Uno \and Autore Due}
\date{}

\begin{document}
\maketitle

\begin{abstract}
Scrivi qui l'abstract: problema, approccio, risultato principale.
\end{abstract}

\section{Introduzione}
Il contesto e il contributo. Cita dalla tua libreria con «Cita» nell'editor.

\section{Metodo}
\begin{equation}
  \mathcal{L} = -\sum_i y_i \log \hat{y}_i
\end{equation}

\section{Esperimenti}
\begin{table}[t]
  \centering
  \caption{Risultati principali.}
  \begin{tabular}{lcc}
    \toprule
    Metodo & Metrica A & Metrica B \\
    \midrule
    Baseline & 0.72 & 0.65 \\
    Proposto & \textbf{0.81} & \textbf{0.74} \\
    \bottomrule
  \end{tabular}
\end{table}

\section{Conclusioni}

\bibliographystyle{plain}
\bibliography{refs}
\end{document}
"#;

/// Report / thesis-like document with chapters and a table of contents.
pub const REPORT_TEMPLATE: &str = r#"\documentclass[11pt]{report}
\usepackage[utf8]{inputenc}
\usepackage[T1]{fontenc}
\usepackage[italian]{babel}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{booktabs}
\usepackage[hidelinks]{hyperref}

\title{Titolo della relazione}
\author{}
\date{\today}

\begin{document}
\maketitle
\tableofcontents

\chapter{Introduzione}
Scrivi qui. Le citazioni vengono dalla tua libreria («Cita» nell'editor).

\chapter{Stato dell'arte}

\chapter{Metodo}

\chapter{Risultati}

\chapter{Conclusioni}

\bibliographystyle{plain}
\bibliography{refs}
\end{document}
"#;

/// Beamer slide deck.
pub const SLIDES_TEMPLATE: &str = r#"\documentclass{beamer}
\usetheme{Madrid}
\usepackage[utf8]{inputenc}
\usepackage[T1]{fontenc}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{booktabs}

\title{Titolo della presentazione}
\author{}
\date{\today}

\begin{document}

\begin{frame}
  \titlepage
\end{frame}

\begin{frame}{Sommario}
  \tableofcontents
\end{frame}

\section{Introduzione}
\begin{frame}{Introduzione}
  \begin{itemize}
    \item Primo punto
    \item Secondo punto
  \end{itemize}
\end{frame}

\section{Risultati}
\begin{frame}{Risultati}
  \begin{equation*}
    E = mc^2
  \end{equation*}
\end{frame}

\end{document}
"#;

/// Bare-bones document, no packages beyond math.
pub const MINIMAL_TEMPLATE: &str = r#"\documentclass{article}
\usepackage[utf8]{inputenc}
\usepackage{amsmath}

\begin{document}

Scrivi qui.

\end{document}
"#;

/// The built-in templates: (id, body). The frontend shows matching labels.
pub const TEMPLATES: [(&str, &str); 5] = [
    ("articolo", MAIN_TEMPLATE),
    ("paper", PAPER_TEMPLATE),
    ("relazione", REPORT_TEMPLATE),
    ("presentazione", SLIDES_TEMPLATE),
    ("minimale", MINIMAL_TEMPLATE),
];

/// Body of a built-in template ("articolo" if the id is unknown).
pub fn template_body(id: &str) -> &'static str {
    TEMPLATES
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, b)| *b)
        .unwrap_or(MAIN_TEMPLATE)
}

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

/// Extract a downloaded template .zip into a fresh project folder `root`.
/// Strips a single shared top-level folder (the usual shape of gallery
/// downloads), guards against zip-slip, and makes sure a `main.tex` exists
/// (renaming the single root `\documentclass` file if needed).
pub fn extract_template_zip(zip_path: &Path, root: &Path) -> Result<()> {
    use std::io::Read;
    const MAX_ENTRIES: usize = 2000;
    const MAX_FILE_BYTES: u64 = 60 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 300 * 1024 * 1024;

    let file = std::fs::File::open(zip_path).map_err(|e| anyhow!("apertura .zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| anyhow!("lettura .zip: {e}"))?;
    let n = archive.len().min(MAX_ENTRIES);

    // A shared top-level folder is stripped so files land at the project root.
    let mut prefix: Option<String> = None;
    let mut uniform = true;
    for i in 0..n {
        let Ok(entry) = archive.by_index(i) else { continue };
        let Some(rel) = entry.enclosed_name() else { continue };
        let Some(first) = rel.components().next() else { continue };
        let first = first.as_os_str().to_string_lossy().to_string();
        if first == "__MACOSX" {
            continue;
        }
        match &prefix {
            None => prefix = Some(first),
            Some(p) if *p != first => {
                uniform = false;
                break;
            }
            _ => {}
        }
    }
    let strip = if uniform { prefix } else { None };

    let mut total: u64 = 0;
    let mut wrote_any = false;
    for i in 0..n {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.is_dir() {
            continue;
        }
        // enclosed_name() rejects absolute paths and `..` traversal (zip-slip guard).
        let Some(rel) = entry.enclosed_name().map(|r| r.to_path_buf()) else { continue };
        let rel = match &strip {
            Some(p) => match rel.strip_prefix(p) {
                Ok(r) => r.to_path_buf(),
                Err(_) => rel,
            },
            None => rel,
        };
        if rel.as_os_str().is_empty()
            || rel.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            || rel.starts_with("__MACOSX")
        {
            continue;
        }
        if entry.size() > MAX_FILE_BYTES {
            continue; // no single template file is this big; likely junk
        }
        if total >= MAX_TOTAL_BYTES {
            break;
        }
        let out = root.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut f = std::fs::File::create(&out).map_err(|e| anyhow!("{}: {e}", rel.display()))?;
        let written = std::io::copy(&mut (&mut entry).take(MAX_FILE_BYTES), &mut f)
            .map_err(|e| anyhow!("{}: {e}", rel.display()))?;
        total += written;
        wrote_any = true;
    }
    if !wrote_any {
        return Err(anyhow!("lo .zip non contiene file utilizzabili"));
    }
    ensure_main_tex(root)
}

/// Make sure `root/main.tex` exists: when the template names its document
/// differently, rename the root-level `\documentclass` file. Ambiguity
/// (several candidates) is an error asking the user to pick by hand.
fn ensure_main_tex(root: &Path) -> Result<()> {
    if root.join("main.tex").exists() {
        return Ok(());
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("tex") {
                let head: String = std::fs::read_to_string(&p)
                    .unwrap_or_default()
                    .chars()
                    .take(64 * 1024)
                    .collect();
                if head.contains("\\documentclass") {
                    candidates.push(p);
                }
            }
        }
    }
    match candidates.len() {
        1 => std::fs::rename(&candidates[0], root.join("main.tex"))
            .map_err(|e| anyhow!("rinomina in main.tex: {e}")),
        0 => Err(anyhow!(
            "nessun file .tex con \\documentclass alla radice dello .zip: \
             rinomina il documento principale in main.tex e riprova"
        )),
        _ => Err(anyhow!(
            "più file .tex principali nello .zip ({}): rinomina quello giusto in main.tex",
            candidates
                .iter()
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Outcome of a compilation attempt.
#[derive(serde::Serialize)]
pub struct CompileResult {
    /// A PDF was produced by THIS run (fresh mtime) — show the preview.
    pub ok: bool,
    /// The tool also exited cleanly. `ok && !clean` = PDF produced but with
    /// errors/warnings (e.g. bibtex on a document with no \cite yet) — like
    /// Overleaf, we show the result AND the log.
    pub clean: bool,
    /// Which tool ran ("tectonic" / "texify" / "latexmk"), empty if none found.
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
    // bibtex/reruns automatically. Then texify (MiKTeX's native driver: handles
    // bibtex/reruns and needs NO Perl — latexmk on MiKTeX dies without Perl).
    // Last, latexmk (TeX Live, where texify does not exist).
    let attempts: [(&str, Vec<&str>); 3] = [
        ("tectonic", vec!["main.tex"]),
        ("texify", vec!["--pdf", "--batch", "main.tex"]),
        ("latexmk", vec!["-pdf", "-interaction=nonstopmode", "-halt-on-error", "main.tex"]),
    ];
    for (tool, args) in attempts {
        // A stale main.pdf from an earlier run must not count as success:
        // only a PDF (re)written by THIS run does.
        let started = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
        match run_with_timeout(tool, &args, root, std::time::Duration::from_secs(300)) {
            Ok((status_ok, output)) => {
                let pdf = root.join("main.pdf");
                let fresh = pdf
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|t| t >= started)
                    .unwrap_or(false);
                let mut log = tail(&output, 4000);
                if !status_ok {
                    // The drivers (texify in particular) often just say "did not
                    // succeed"; the actual TeX error lives in main.log.
                    if let Some(snip) = log_errors(&root.join("main.log")) {
                        log.push_str("\n--- errori da main.log ---\n");
                        log.push_str(&snip);
                    }
                }
                return CompileResult {
                    ok: fresh,
                    clean: status_ok,
                    tool: tool.to_string(),
                    log,
                    pdf_rel: if fresh { Some("main.pdf".into()) } else { None },
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return CompileResult {
                    ok: false,
                    clean: false,
                    tool: tool.to_string(),
                    log: format!("avvio {tool}: {e}"),
                    pdf_rel: None,
                }
            }
        }
    }
    CompileResult {
        ok: false,
        clean: false,
        tool: String::new(),
        log: "Nessun compilatore LaTeX trovato. Installa Tectonic (consigliato, un solo \
              eseguibile che scarica i pacchetti da solo):\n  winget install Tectonic.Tectonic\n\
              oppure una distribuzione TeX (MiKTeX / TeX Live). Poi riprova."
            .to_string(),
        pdf_rel: None,
    }
}

/// The "! …" error lines from a TeX log (each with 2 lines of context), capped.
/// None when the log is missing/unreadable or has no error markers.
fn log_errors(log_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(log_path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() && out.len() < 2500 {
        if lines[i].starts_with('!') {
            for l in lines.iter().skip(i).take(3) {
                out.push_str(l);
                out.push('\n');
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
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
    fn template_zip_strips_prefix_and_renames_main() {
        let dir = std::env::temp_dir().join(format!("tpzip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A gallery-style zip: one shared top folder, doc not named main.tex.
        let zip_path = dir.join("t.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut w = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            use std::io::Write;
            w.start_file("MyTemplate/paper.tex", opts).unwrap();
            w.write_all(b"\\documentclass{article}\\begin{document}x\\end{document}").unwrap();
            w.start_file("MyTemplate/style.cls", opts).unwrap();
            w.write_all(b"% cls").unwrap();
            w.start_file("__MACOSX/junk", opts).unwrap();
            w.write_all(b"j").unwrap();
            w.finish().unwrap();
        }
        let root = dir.join("proj");
        std::fs::create_dir_all(&root).unwrap();
        extract_template_zip(&zip_path, &root).unwrap();
        assert!(root.join("main.tex").exists(), "paper.tex renamed to main.tex");
        assert!(root.join("style.cls").exists(), "prefix MyTemplate/ stripped");
        assert!(!root.join("__MACOSX").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Env-gated spike: scaffold every built-in template as create_project does
    /// (main.tex + empty refs.bib) and compile it with the real system toolchain.
    /// Run: $env:TEXPROJ_TEST_DIR="<dir>"; cargo test --lib spike_compile_templates -- --nocapture
    #[test]
    fn spike_compile_templates() {
        let Some(dir) = std::env::var_os("TEXPROJ_TEST_DIR") else { return };
        for (id, body) in TEMPLATES {
            let root = Path::new(&dir).join(id);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("main.tex"), body).unwrap();
            std::fs::write(root.join("refs.bib"), "").unwrap();
            let res = compile(&root);
            println!("== {id}: ok={} tool={}", res.ok, res.tool);
            if !res.ok {
                println!("{}", res.log);
            }
            assert!(res.ok, "template {id} non compila");
        }
    }

    #[test]
    fn log_tail_respects_utf8() {
        let s = "è".repeat(3000);
        let t = tail(&s, 100);
        assert!(t.chars().count() <= 60);
        assert!(t.starts_with('…'));
    }
}
