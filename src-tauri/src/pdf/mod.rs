//! PDF text extraction and thumbnail rendering, backed by pdfium.
//!
//! pdfium is single-threaded internally; the `thread_safe` feature (enabled by
//! default) serializes calls behind a mutex, so a single [`Pdfium`] instance can
//! live in shared app state. Heavy calls should still run off the UI thread
//! (the command layer wraps them in `spawn_blocking`).

use anyhow::{Context, Result};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Result of reading a PDF: concatenated page text and the page count.
#[derive(Debug, Clone)]
pub struct PdfExtract {
    pub text: String,
    #[allow(dead_code)]
    pub page_count: u16,
}

/// Bind to the `pdfium` dynamic library located in `lib_dir`, falling back to a
/// system-installed pdfium if it is not found there.
///
/// pdfium-render's `thread_safe` feature stores the bindings in a process-global
/// `OnceCell`, so this (and [`bind_for_app`]) may only succeed once per process.
#[allow(dead_code)]
pub fn bind(lib_dir: &Path) -> Result<Pdfium> {
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(lib_dir))
        .or_else(|_| Pdfium::bind_to_system_library())
        .context("binding to the pdfium library")?;
    Ok(Pdfium::new(bindings))
}

/// Shared pdfium instance for tests: pdfium's global bindings can only be set
/// once per process, so every test must go through this single binding.
#[cfg(test)]
pub(crate) fn test_pdfium() -> &'static Pdfium {
    use std::sync::OnceLock;
    static PDFIUM: OnceLock<Pdfium> = OnceLock::new();
    PDFIUM.get_or_init(|| {
        bind(&Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries")).expect("binding test pdfium")
    })
}

/// Resolve and bind pdfium for a running app: prefer the bundled resource
/// directory (production), then the executable's own directory (dev, where we
/// copy `pdfium.dll` next to the binary). Each candidate is an *absolute* path,
/// so binding never consults the ambient Windows DLL search order.
pub fn bind_for_app(app: &AppHandle) -> Result<Pdfium> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
        }
    }
    for dir in &candidates {
        let name = Pdfium::pdfium_platform_library_name_at_path(dir);
        if let Ok(bindings) = Pdfium::bind_to_library(name) {
            return Ok(Pdfium::new(bindings));
        }
    }
    // Security: deliberately do NOT fall back to `Pdfium::bind_to_system_library()`,
    // which loads `pdfium.dll` by bare name through the Windows DLL search order
    // (current dir / PATH) and could load an attacker-planted DLL on a broken install.
    // The library ships bundled in the resource dir; its absence is a hard error.
    anyhow::bail!("pdfium non trovato nelle risorse dell'app o accanto all'eseguibile")
}

/// Extract all text from `path`, concatenating pages in order.
pub fn extract_text(pdfium: &Pdfium, path: &Path) -> Result<PdfExtract> {
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("loading PDF {}", path.display()))?;
    let pages = doc.pages();
    let page_count = pages.len();
    let mut text = String::new();
    for page in pages.iter() {
        if let Ok(page_text) = page.text() {
            text.push_str(&page_text.all());
            text.push('\n');
        }
    }
    Ok(PdfExtract {
        text,
        page_count: u16::try_from(page_count).unwrap_or(u16::MAX),
    })
}

/// Extract text page by page (one string per page), for page-attributed chunking.
pub fn extract_pages(pdfium: &Pdfium, path: &Path) -> Result<Vec<String>> {
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("loading PDF {}", path.display()))?;
    let mut out = Vec::new();
    for page in doc.pages().iter() {
        let t = page.text().map(|x| x.all()).unwrap_or_default();
        out.push(t);
    }
    Ok(out)
}

/// A word with its bounding box, normalized to 0..1 in the page's top-left frame.
#[derive(Debug, Clone)]
pub struct PdfWord {
    pub text: String,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Extract words (with positions) inside a normalized region `[rx, ry, rw, rh]`
/// (top-left origin) of page `page_index`. Used to reconstruct a selected table.
pub fn extract_region_words(
    pdfium: &Pdfium,
    path: &Path,
    page_index: u16,
    region: [f32; 4],
) -> Result<Vec<PdfWord>> {
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("loading PDF {}", path.display()))?;
    let page = doc.pages().get(page_index as i32).context("page out of range")?;
    let pw = page.width().value;
    let ph = page.height().value;
    if pw <= 0.0 || ph <= 0.0 {
        return Ok(Vec::new());
    }
    let text = page.text().context("reading page text")?;
    let [rx, ry, rw, rh] = region;

    let mut words: Vec<PdfWord> = Vec::new();
    // Current word accumulator.
    let mut cur = String::new();
    let (mut wx0, mut wy0, mut wx1, mut wy1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut prev_x1 = f32::NAN;
    let mut prev_cy = f32::NAN;

    let flush = |cur: &mut String, words: &mut Vec<PdfWord>, x0: f32, y0: f32, x1: f32, y1: f32| {
        let t = cur.trim();
        if !t.is_empty() && x1 > x0 {
            words.push(PdfWord { text: t.to_string(), x0, y0, x1, y1 });
        }
        cur.clear();
    };

    for ch in text.chars().iter() {
        let c = ch.unicode_char().unwrap_or(' ');
        // Char bounds in PDF points (origin bottom-left, y upward).
        let Ok(b) = ch.loose_bounds() else { continue };
        let cx0 = b.left().value / pw;
        let cx1 = b.right().value / pw;
        // Flip Y to a top-left origin.
        let cy0 = (ph - b.top().value) / ph;
        let cy1 = (ph - b.bottom().value) / ph;
        let ccx = (cx0 + cx1) / 2.0;
        let ccy = (cy0 + cy1) / 2.0;
        let inside = ccx >= rx && ccx <= rx + rw && ccy >= ry && ccy <= ry + rh;

        if c.is_whitespace() || !inside {
            flush(&mut cur, &mut words, wx0, wy0, wx1, wy1);
            wx0 = f32::MAX; wy0 = f32::MAX; wx1 = f32::MIN; wy1 = f32::MIN;
            prev_x1 = f32::NAN;
            prev_cy = f32::NAN;
            continue;
        }
        // Break the word on a new line or a wide horizontal gap (column boundary).
        let h = (cy1 - cy0).max(0.001);
        if !prev_x1.is_nan()
            && ((cx0 - prev_x1) > 0.4 * h || (ccy - prev_cy).abs() > 0.6 * h)
        {
            flush(&mut cur, &mut words, wx0, wy0, wx1, wy1);
            wx0 = f32::MAX; wy0 = f32::MAX; wx1 = f32::MIN; wy1 = f32::MIN;
        }
        cur.push(c);
        wx0 = wx0.min(cx0);
        wy0 = wy0.min(cy0);
        wx1 = wx1.max(cx1);
        wy1 = wy1.max(cy1);
        prev_x1 = cx1;
        prev_cy = ccy;
    }
    flush(&mut cur, &mut words, wx0, wy0, wx1, wy1);
    Ok(words)
}

/// Render the first page of `path` to a PNG at `out`, fitting within `width`px.
pub fn render_thumbnail(pdfium: &Pdfium, path: &Path, out: &Path, width: u16) -> Result<()> {
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("loading PDF {}", path.display()))?;
    let page = doc.pages().get(0).context("PDF has no pages")?;
    let config = PdfRenderConfig::new()
        .set_target_width(width.into())
        .set_maximum_height(width.saturating_mul(2).into());
    let img = page
        .render_with_config(&config)
        .context("rendering page to bitmap")?
        .as_image()
        .context("converting rendered bitmap to image")?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    img.save(out)
        .with_context(|| format!("saving thumbnail {}", out.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.pdf")
    }

    #[test]
    fn extracts_text_and_renders_thumbnail() -> Result<()> {
        let pdfium = test_pdfium();
        let pdf = fixture();

        let extract = extract_text(pdfium, &pdf)?;
        println!("page_count = {}", extract.page_count);
        println!("text = {:?}", extract.text);
        assert!(extract.page_count >= 1, "expected at least one page");
        assert!(
            !extract.text.trim().is_empty(),
            "expected non-empty extracted text"
        );

        let out = std::env::temp_dir().join("pdfmanage_thumb_test.png");
        render_thumbnail(pdfium, &pdf, &out, 200)?;
        let size = std::fs::metadata(&out)?.len();
        println!("thumbnail bytes = {}", size);
        assert!(size > 0, "thumbnail file is empty");
        std::fs::remove_file(&out).ok();
        Ok(())
    }
}
