//! OCR fallback for scanned PDFs that have no embedded text layer.
//!
//! Uses the OCR engine built into Windows (`Windows.Media.Ocr`) — there is no
//! bundled engine or model; it relies on the OCR language packs already present
//! in the OS (English ships by default). Pages are rasterized with the same
//! pdfium instance used elsewhere, converted to a `SoftwareBitmap`, and fed to
//! the engine. Best-effort: pages that fail are skipped.

use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;

/// The recognised text plus how much of the PDF was covered, so the caller can
/// tell the user when a long document was capped.
pub struct OcrOutput {
    pub text: String,
    pub total_pages: usize,
    pub pages_ocred: usize,
}

#[cfg(windows)]
pub fn ocr_pdf(pdfium: &Pdfium, path: &Path, max_pages: usize) -> Result<OcrOutput> {
    use anyhow::Context;
    // Balance COM init/uninit on this (possibly pooled) thread; RPC_E_CHANGED_MODE
    // means an apartment already exists, which is fine for WinRT.
    let _com = ComGuard::new();
    let engine = create_engine()
        .context("motore OCR di Windows non disponibile (installa un language pack OCR dalle impostazioni di Windows)")?;
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("loading PDF {}", path.display()))?;
    let total_pages = doc.pages().len() as usize;
    // Render fairly large so small scanned type stays legible to the OCR engine.
    let config = PdfRenderConfig::new().set_target_width(1654).set_maximum_height(2339);
    let mut out = String::new();
    let mut pages_ocred = 0usize;
    for (i, page) in doc.pages().iter().enumerate() {
        if i >= max_pages {
            break;
        }
        pages_ocred += 1;
        let Ok(bmp) = page.render_with_config(&config) else { continue };
        let Ok(img) = bmp.as_image() else { continue };
        if let Ok(text) = ocr_image(&engine, &img.into_rgba8()) {
            let text = text.trim();
            if !text.is_empty() {
                out.push_str(text);
                out.push_str("\n\n");
            }
        }
    }
    Ok(OcrOutput { text: out, total_pages, pages_ocred })
}

/// RAII guard that initialises COM (MTA) for the current thread and uninitialises
/// it on drop only when this guard actually performed the initialisation.
#[cfg(windows)]
struct ComGuard {
    uninit: bool,
}

#[cfg(windows)]
impl ComGuard {
    fn new() -> Self {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        // S_OK / S_FALSE increment the init count (must balance with CoUninitialize);
        // RPC_E_CHANGED_MODE (Err) means the thread already has a different apartment
        // — usable by WinRT, and we must NOT uninitialise it.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self { uninit: hr.is_ok() }
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninit {
            use windows::Win32::System::Com::CoUninitialize;
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(windows)]
fn create_engine() -> Result<windows::Media::Ocr::OcrEngine> {
    use windows::Globalization::Language;
    use windows::Media::Ocr::OcrEngine;
    // Prefer the user's installed languages; fall back to English.
    if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
        return Ok(engine);
    }
    let lang = Language::CreateLanguage(&windows::core::HSTRING::from("en"))?;
    Ok(OcrEngine::TryCreateFromLanguage(&lang)?)
}

#[cfg(windows)]
fn ocr_image(engine: &windows::Media::Ocr::OcrEngine, rgba: &image::RgbaImage) -> Result<String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Security::Cryptography::CryptographicBuffer;
    let (w, h) = rgba.dimensions();
    // SoftwareBitmap Bgra8 wants BGRA byte order; pdfium gives us RGBA.
    let mut bytes = rgba.as_raw().clone();
    for px in bytes.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let buffer = CryptographicBuffer::CreateFromByteArray(&bytes)?;
    let bmp = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32)?;
    let result = engine.RecognizeAsync(&bmp)?.get()?;
    Ok(result.Text()?.to_string())
}

#[cfg(not(windows))]
pub fn ocr_pdf(_pdfium: &Pdfium, _path: &Path, _max_pages: usize) -> Result<OcrOutput> {
    anyhow::bail!("OCR è disponibile solo su Windows")
}
