//! Table STRUCTURE recognition: an image of a table -> rows / columns / spanning
//! cells, via Microsoft Table Transformer v1.1 (TATR, MIT — DETR-style set
//! prediction, single forward pass, no autoregressive loop). Trained on
//! PubTables-1M: borderless scientific tables with spanning headers — exactly the
//! tables found in papers.
//!
//! The model only provides the GEOMETRY. Cell text comes from pdfium's word boxes
//! intersected with the predicted grid, so on born-digital PDFs the content is
//! byte-exact with zero OCR. The ~110 MB weights download on first use (same
//! pattern as the formula OCR), pinned to an immutable commit.
//!
//! Reference: <https://github.com/microsoft/table-transformer> (MIT) /
//! ONNX export <https://huggingface.co/Xenova/table-transformer-structure-recognition-v1.1-all>.

use anyhow::{anyhow, Context, Result};
use image::RgbImage;
use ndarray::Array;
use once_cell::sync::Lazy;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Value;
use parking_lot::Mutex;
use std::path::Path;

/// Longest-edge target for the model input (official TATR inference uses 1000,
/// NOT the 800 in the HF preprocessor_config — a verified discrepancy).
const INPUT_LONG_EDGE: u32 = 1000;
/// Keep a detection when its best non-"no object" class beats this.
const SCORE_THRESHOLD: f32 = 0.5;
const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

// TATR structure classes.
const CLS_COLUMN: usize = 1;
const CLS_ROW: usize = 2;
const CLS_SPANNING: usize = 5;
const CLS_NO_OBJECT: usize = 6;

pub const MODELS: [crate::dl::ModelFile; 1] = [(
    "tatr-structure-v1.1.onnx",
    "https://huggingface.co/Xenova/table-transformer-structure-recognition-v1.1-all/resolve/76c3afa174a0f36bafa9438934f57dcdb6e486a5/onnx/model.onnx",
    115_819_060,
)];

pub fn models_present(dir: &Path) -> bool {
    crate::dl::all_present(dir, &MODELS)
}

pub fn missing_bytes(dir: &Path) -> u64 {
    crate::dl::missing_bytes(dir, &MODELS)
}

pub async fn ensure_models(dir: &Path) -> Result<()> {
    crate::dl::fetch(dir, &MODELS).await
}

struct Engine {
    session: Session,
    input: String,
}

// One engine per process, built lazily on first recognition and reused.
static ENGINE: Lazy<Mutex<Option<Engine>>> = Lazy::new(|| Mutex::new(None));

fn load_engine(dir: &Path) -> Result<Engine> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let session = Session::builder()
        .map_err(|e| anyhow!(e.to_string()))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow!(e.to_string()))?
        .with_intra_threads(threads)
        .map_err(|e| anyhow!(e.to_string()))?
        .commit_from_file(dir.join(MODELS[0].0))
        .map_err(|e| anyhow!("carico {}: {e}", MODELS[0].0))?;
    let input = session
        .inputs()
        .first()
        .ok_or_else(|| anyhow!("modello tabella senza input"))?
        .name()
        .to_string();
    Ok(Engine { session, input })
}

/// One detected structure element, box normalized 0..1 to the INPUT IMAGE (which
/// is the user's crop; aspect is preserved, so these coordinates map straight
/// onto the selected PDF region).
#[derive(Debug, Clone)]
pub struct DetBox {
    pub cls: usize,
    /// Detection confidence — kept for diagnostics/future refinement (not read yet).
    #[allow(dead_code)]
    pub prob: f32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Normalize an RGB image to the model tensor: longest edge to 1000 (aspect
/// preserved — no padding, so normalized outputs align with the crop), 1/255 +
/// ImageNet mean/std, CHW.
fn preprocess(rgb: &RgbImage) -> (Vec<f32>, usize, usize) {
    let (w, h) = rgb.dimensions();
    let scale = INPUT_LONG_EDGE as f32 / w.max(h).max(1) as f32;
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    let resized = image::imageops::resize(rgb, nw, nh, image::imageops::FilterType::Lanczos3);
    let (nw, nh) = (nw as usize, nh as usize);
    let mut data = vec![0f32; 3 * nw * nh];
    for (x, y, p) in resized.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        for ch in 0..3 {
            data[ch * nw * nh + y * nw + x] =
                (p.0[ch] as f32 / 255.0 - IMAGENET_MEAN[ch]) / IMAGENET_STD[ch];
        }
    }
    (data, nh, nw)
}

/// Decode DETR outputs (logits `[1,Q,7]`, boxes `[1,Q,4]` cxcywh normalized) into
/// kept detections. Pure math — separated for unit testing.
fn decode(logits: &[f32], boxes: &[f32], queries: usize, classes: usize) -> Vec<DetBox> {
    let mut out = Vec::new();
    for q in 0..queries {
        let row = &logits[q * classes..(q + 1) * classes];
        // Softmax + argmax.
        let maxv = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = row.iter().map(|&v| (v - maxv).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let (cls, prob) = exps
            .iter()
            .enumerate()
            .map(|(i, &e)| (i, e / sum.max(f32::MIN_POSITIVE)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((CLS_NO_OBJECT, 0.0));
        if cls == CLS_NO_OBJECT || prob < SCORE_THRESHOLD {
            continue;
        }
        let b = &boxes[q * 4..q * 4 + 4];
        let (cx, cy, bw, bh) = (b[0], b[1], b[2], b[3]);
        out.push(DetBox {
            cls,
            prob,
            x0: (cx - bw / 2.0).clamp(0.0, 1.0),
            y0: (cy - bh / 2.0).clamp(0.0, 1.0),
            x1: (cx + bw / 2.0).clamp(0.0, 1.0),
            y1: (cy + bh / 2.0).clamp(0.0, 1.0),
        });
    }
    out
}

/// Recognize the table structure in an image (PNG/any). Blocking; the caller
/// wraps it in `spawn_blocking`. `ensure_models` must have succeeded first.
pub fn recognize(dir: &Path, image_bytes: &[u8]) -> Result<Vec<DetBox>> {
    let rgb = image::load_from_memory(image_bytes).context("decodifico immagine")?.to_rgb8();
    let (data, h, w) = preprocess(&rgb);

    let mut guard = ENGINE.lock();
    if guard.is_none() {
        *guard = Some(load_engine(dir)?);
    }
    let eng = guard.as_mut().unwrap();

    let arr = Array::from_shape_vec((1usize, 3usize, h, w), data).context("costruisco tensore")?;
    let inputs = ort::inputs![ eng.input.as_str() => Value::from_array(arr)? ];
    let outputs = eng.session.run(inputs).map_err(|e| anyhow!(e.to_string()))?;

    // Identify outputs by trailing dimension (7 = class logits, 4 = boxes), so the
    // export's tensor names/order don't matter.
    let mut logits: Option<(Vec<usize>, Vec<f32>)> = None;
    let mut boxes: Option<(Vec<usize>, Vec<f32>)> = None;
    for i in 0..outputs.len() {
        let Ok((shape, data)) = outputs[i].try_extract_tensor::<f32>() else { continue };
        let shape: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        match shape.last() {
            Some(7) => logits = Some((shape, data.to_vec())),
            Some(4) => boxes = Some((shape, data.to_vec())),
            _ => {}
        }
    }
    let (lshape, ldata) = logits.ok_or_else(|| anyhow!("output logits mancante"))?;
    let (_bshape, bdata) = boxes.ok_or_else(|| anyhow!("output boxes mancante"))?;
    let queries = lshape.get(1).copied().unwrap_or(0);
    Ok(decode(&ldata, &bdata, queries, 7))
}

/// A word with its box in REGION-relative coordinates (0..1 of the user's crop).
#[derive(Debug, Clone)]
pub struct WordBox {
    pub text: String,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// 1-D interval de-duplication: sort by center, then merge intervals overlapping
/// more than 60% of the smaller one (DETR sometimes emits near-duplicate rows).
fn merge_bands(mut bands: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    bands.sort_by(|a, b| {
        (a.0 + a.1).partial_cmp(&(b.0 + b.1)).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<(f32, f32)> = Vec::new();
    for b in bands {
        if let Some(last) = out.last_mut() {
            let inter = (b.1.min(last.1) - b.0.max(last.0)).max(0.0);
            let min_len = (b.1 - b.0).min(last.1 - last.0).max(f32::MIN_POSITIVE);
            if inter / min_len > 0.6 {
                last.0 = last.0.min(b.0);
                last.1 = last.1.max(b.1);
                continue;
            }
        }
        out.push(b);
    }
    out
}

/// Index of the band containing `c`. A point outside every band is snapped to the
/// nearest one only when it is CLOSE (within half that band's length beyond its
/// edge) — words far outside the table (captions, footnotes caught by a generous
/// selection) are dropped instead of contaminating an edge cell.
fn band_of(bands: &[(f32, f32)], c: f32) -> Option<usize> {
    for (i, (a, b)) in bands.iter().enumerate() {
        if c >= *a && c <= *b {
            return Some(i);
        }
    }
    bands
        .iter()
        .enumerate()
        .map(|(i, (a, b))| {
            let edge_dist = if c < *a { *a - c } else { c - *b };
            (i, edge_dist, (*b - *a).max(f32::MIN_POSITIVE))
        })
        .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal))
        .filter(|(_, dist, len)| *dist <= 0.5 * *len)
        .map(|(i, _, _)| i)
}

/// Assemble the final grid: rows × columns from the detections, each cell filled
/// with the words whose center falls inside it (reading order); spanning cells
/// pull their content into the top-left anchor. Empty rows/columns are dropped.
pub fn assemble_grid(dets: &[DetBox], words: &[WordBox]) -> Vec<Vec<String>> {
    let rows = merge_bands(
        dets.iter().filter(|d| d.cls == CLS_ROW).map(|d| (d.y0, d.y1)).collect(),
    );
    let cols = merge_bands(
        dets.iter().filter(|d| d.cls == CLS_COLUMN).map(|d| (d.x0, d.x1)).collect(),
    );
    if rows.is_empty() || cols.is_empty() {
        return Vec::new();
    }
    let (nr, nc) = (rows.len(), cols.len());

    // Words → (row, col) by center point; words far outside the detected table
    // (e.g. a caption caught by the selection) are dropped, not misfiled.
    let mut cells: Vec<Vec<Vec<&WordBox>>> = vec![vec![Vec::new(); nc]; nr];
    for w in words {
        let cx = (w.x0 + w.x1) / 2.0;
        let cy = (w.y0 + w.y1) / 2.0;
        if let (Some(ri), Some(ci)) = (band_of(&rows, cy), band_of(&cols, cx)) {
            cells[ri][ci].push(w);
        }
    }

    // Spanning cells: absorb every base cell whose area is >50% covered by the
    // span box; content moves to the span's top-left anchor.
    for d in dets.iter().filter(|d| d.cls == CLS_SPANNING) {
        let mut covered: Vec<(usize, usize)> = Vec::new();
        for (ri, r) in rows.iter().enumerate() {
            for (ci, c) in cols.iter().enumerate() {
                let ix = (d.x1.min(c.1) - d.x0.max(c.0)).max(0.0);
                let iy = (d.y1.min(r.1) - d.y0.max(r.0)).max(0.0);
                let cell_area = ((c.1 - c.0) * (r.1 - r.0)).max(f32::MIN_POSITIVE);
                if (ix * iy) / cell_area > 0.5 {
                    covered.push((ri, ci));
                }
            }
        }
        if covered.len() > 1 {
            let anchor = *covered.iter().min().unwrap();
            let mut moved: Vec<&WordBox> = Vec::new();
            for &(ri, ci) in &covered {
                moved.append(&mut cells[ri][ci]);
            }
            cells[anchor.0][anchor.1] = moved;
        }
    }

    // Render each cell in reading order (line clustering by word height).
    let grid: Vec<Vec<String>> = cells
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|mut ws| {
                    ws.sort_by(|a, b| {
                        let ay = (a.y0 + a.y1) / 2.0;
                        let by = (b.y0 + b.y1) / 2.0;
                        let h = ((a.y1 - a.y0).abs()).max(0.001) * 0.6;
                        if (ay - by).abs() > h {
                            ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal)
                        }
                    });
                    ws.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
                })
                .collect()
        })
        .collect();

    // Drop rows/columns that came out entirely empty.
    let grid: Vec<Vec<String>> =
        grid.into_iter().filter(|r| r.iter().any(|c| !c.trim().is_empty())).collect();
    crate::table::trim_empty_columns(grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn det(cls: usize, x0: f32, y0: f32, x1: f32, y1: f32) -> DetBox {
        DetBox { cls, prob: 0.9, x0, y0, x1, y1 }
    }
    fn word(t: &str, x0: f32, y0: f32, x1: f32, y1: f32) -> WordBox {
        WordBox { text: t.into(), x0, y0, x1, y1 }
    }

    #[test]
    fn decode_keeps_confident_non_background() {
        // 2 queries, 7 classes: q0 strongly class 2 (row), q1 strongly no-object.
        let mut logits = vec![0f32; 2 * 7];
        logits[2] = 8.0; // q0 → row
        logits[7 + 6] = 8.0; // q1 → no-object
        let boxes = vec![0.5, 0.3, 0.8, 0.2, /* q1 */ 0.5, 0.5, 0.1, 0.1];
        let dets = decode(&logits, &boxes, 2, 7);
        assert_eq!(dets.len(), 1);
        assert_eq!(dets[0].cls, 2);
        assert!((dets[0].x0 - 0.1).abs() < 1e-4 && (dets[0].y0 - 0.2).abs() < 1e-4);
    }

    #[test]
    fn grid_from_rows_columns_and_words() {
        let dets = vec![
            det(CLS_ROW, 0.0, 0.0, 1.0, 0.45),
            det(CLS_ROW, 0.0, 0.55, 1.0, 1.0),
            det(CLS_COLUMN, 0.0, 0.0, 0.48, 1.0),
            det(CLS_COLUMN, 0.52, 0.0, 1.0, 1.0),
        ];
        let words = vec![
            word("Nome", 0.05, 0.1, 0.2, 0.2),
            word("Valore", 0.6, 0.1, 0.8, 0.2),
            word("alfa", 0.05, 0.7, 0.15, 0.8),
            word("42", 0.6, 0.7, 0.65, 0.8),
        ];
        let g = assemble_grid(&dets, &words);
        assert_eq!(g, vec![vec!["Nome".to_string(), "Valore".into()], vec!["alfa".into(), "42".into()]]);
    }

    #[test]
    fn spanning_cell_pulls_content_to_anchor() {
        let dets = vec![
            det(CLS_ROW, 0.0, 0.0, 1.0, 0.3),
            det(CLS_ROW, 0.0, 0.35, 1.0, 0.65),
            det(CLS_ROW, 0.0, 0.7, 1.0, 1.0),
            det(CLS_COLUMN, 0.0, 0.0, 0.48, 1.0),
            det(CLS_COLUMN, 0.52, 0.0, 1.0, 1.0),
            // Header spanning both columns on the first row.
            det(CLS_SPANNING, 0.0, 0.0, 1.0, 0.3),
        ];
        let words = vec![
            word("Titolo", 0.3, 0.1, 0.45, 0.2),
            word("unico", 0.55, 0.1, 0.7, 0.2),
            word("a", 0.1, 0.45, 0.15, 0.55),
            word("b", 0.6, 0.45, 0.65, 0.55),
            word("c", 0.1, 0.8, 0.15, 0.9),
            word("d", 0.6, 0.8, 0.65, 0.9),
        ];
        let g = assemble_grid(&dets, &words);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0][0], "Titolo unico");
        assert_eq!(g[0][1], "");
        assert_eq!(g[1], vec!["a".to_string(), "b".into()]);
    }

    #[test]
    fn near_duplicate_rows_are_merged() {
        let bands = merge_bands(vec![(0.0, 0.4), (0.05, 0.42), (0.6, 1.0)]);
        assert_eq!(bands.len(), 2);
    }

    /// Spike: end-to-end structure recognition on a REAL paper table. Env-gated:
    /// TATR_DIR (model dir), TATR_TEST_PDF, TATR_TEST_PAGE (0-based). Without
    /// TATR_TEST_REGION ("x,y,w,h" normalized) it just dumps the rendered page to
    /// TATR_TEST_OUT so the region can be picked by eye; with it, it crops, runs
    /// the model + pdfium words, and prints the assembled grid.
    #[test]
    fn spike_table_struct() -> anyhow::Result<()> {
        use pdfium_render::prelude::*;
        let Ok(pdf) = std::env::var("TATR_TEST_PDF") else { return Ok(()) };
        let page_i: u16 = std::env::var("TATR_TEST_PAGE").ok().and_then(|p| p.parse().ok()).unwrap_or(0);
        let out = std::env::var("TATR_TEST_OUT").unwrap_or_else(|_| "tatr_page.png".into());

        let pdfium = crate::pdf::test_pdfium();
        let doc = pdfium.load_pdf_from_file(std::path::Path::new(&pdf), None)?;
        let page = doc.pages().get(page_i as i32).expect("page");
        let img = page
            .render_with_config(&PdfRenderConfig::new().set_target_width(2000))
            .expect("render")
            .as_image()
            .expect("bitmap to image")
            .to_rgb8();

        let Ok(region) = std::env::var("TATR_TEST_REGION") else {
            img.save(&out)?;
            eprintln!("[tatr] page dumped to {out} ({}x{}) — set TATR_TEST_REGION", img.width(), img.height());
            return Ok(());
        };
        let r: Vec<f32> = region.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let [rx, ry, rw, rh] = [r[0], r[1], r[2], r[3]];
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let crop = image::imageops::crop_imm(
            &img,
            (rx * iw) as u32,
            (ry * ih) as u32,
            ((rw * iw) as u32).max(1),
            ((rh * ih) as u32).max(1),
        )
        .to_image();
        crop.save(&out)?;
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(crop)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;

        let dir = std::path::PathBuf::from(std::env::var("TATR_DIR").expect("TATR_DIR"));
        let dets = recognize(&dir, &png)?;
        let (nr, nc, ns) = (
            dets.iter().filter(|d| d.cls == CLS_ROW).count(),
            dets.iter().filter(|d| d.cls == CLS_COLUMN).count(),
            dets.iter().filter(|d| d.cls == CLS_SPANNING).count(),
        );
        eprintln!("[tatr] detections: {nr} rows, {nc} cols, {ns} spanning");

        let words = crate::pdf::extract_region_words(pdfium, std::path::Path::new(&pdf), page_i, [rx, ry, rw, rh])?;
        let wboxes: Vec<WordBox> = words
            .into_iter()
            .map(|w| WordBox {
                text: w.text,
                x0: (w.x0 - rx) / rw,
                y0: (w.y0 - ry) / rh,
                x1: (w.x1 - rx) / rw,
                y1: (w.y1 - ry) / rh,
            })
            .collect();
        eprintln!("[tatr] {} words in region", wboxes.len());
        let grid = assemble_grid(&dets, &wboxes);
        eprintln!("[tatr] GRID =\n{}", crate::table::to_markdown(&grid));
        Ok(())
    }
}
