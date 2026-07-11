//! Formula OCR: an image of a mathematical equation -> LaTeX.
//!
//! Runs Pix2Text MFR 1.5 (MIT, breezedeus/pix2text-mfr-1.5): a DeiT encoder over a
//! fixed 384×384 input produces a context sequence, and a TrOCR decoder
//! autoregressively emits LaTeX tokens. Trained on printed AND handwritten
//! formulas including matrices/multi-line — a large quality step over the previous
//! pix2tex/LaTeX-OCR model (kept the same download-on-first-use plumbing).
//!
//! The ONNX Runtime is the very one already linked statically into the binary via
//! `fastembed` (bge-m3), so this adds no runtime and no extra DLL. The ~114 MB of
//! model weights are NOT bundled: they are downloaded once, on first use, into
//! `app_data/mathocr/` — the same "fetch on demand" pattern as the embeddings.
//!
//! Reference: <https://huggingface.co/breezedeus/pix2text-mfr-1.5> (MIT) /
//! <https://github.com/breezedeus/Pix2Text> (MIT).

use anyhow::{anyhow, Context, Result};
use image::{GrayImage, Luma};
use ndarray::Array;
use once_cell::sync::Lazy;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::{TensorElementType, Value, ValueType};
use parking_lot::Mutex;
use regex::Regex;
use std::path::Path;
use tokenizers::Tokenizer;

// --- Decoding / preprocessing constants (from the model's generation_config) ---
const BOS: i64 = 1; // decoder_start_token_id == bos
const EOS: i64 = 2;
const MAX_SEQ: usize = 512;
const BEAM: usize = 4; // beam width (1 == greedy)
const LENGTH_PENALTY: f32 = 0.7; // <1 mildly favors longer, complete sequences
const MFR_SIZE: u32 = 384; // fixed encoder input side (DeiT, 384×384×3)

/// (local file name, download URL, exact size in bytes for integrity check).
/// URLs are pinned to a commit hash so the artifacts are immutable (HF `main`
/// can move); the resolve endpoint 302-redirects to the CDN, which the download
/// client follows.
const MODELS: [(&str, &str, u64); 3] = [
    (
        "encoder_model.onnx",
        "https://huggingface.co/breezedeus/pix2text-mfr-1.5/resolve/1cef9f0bdcd6a4c63df7de1311fb0894593340cc/encoder_model.onnx",
        87_510_770,
    ),
    (
        "decoder_model.onnx",
        "https://huggingface.co/breezedeus/pix2text-mfr-1.5/resolve/1cef9f0bdcd6a4c63df7de1311fb0894593340cc/decoder_model.onnx",
        32_026_253,
    ),
    (
        "tokenizer.json",
        "https://huggingface.co/breezedeus/pix2text-mfr-1.5/resolve/1cef9f0bdcd6a4c63df7de1311fb0894593340cc/tokenizer.json",
        113_168,
    ),
];

/// Files of the previous engine (RapidLaTeXOCR pix2tex), deleted on the next
/// model download so the ~178 MB don't linger on disk. (`tokenizer.json` is
/// shared by name — the size check simply re-downloads the right one.)
const LEGACY_FILES: [&str; 3] = ["encoder.onnx", "decoder.onnx", "image_resizer.onnx"];

/// True when every model file is present at its expected size.
pub fn models_present(dir: &Path) -> bool {
    MODELS.iter().all(|(name, _, size)| {
        std::fs::metadata(dir.join(name))
            .map(|m| m.len() == *size)
            .unwrap_or(false)
    })
}

/// Total download size (bytes) still missing — for a user-facing message.
pub fn missing_bytes(dir: &Path) -> u64 {
    MODELS
        .iter()
        .filter(|(name, _, size)| {
            std::fs::metadata(dir.join(name))
                .map(|m| m.len() != *size)
                .unwrap_or(true)
        })
        .map(|(_, _, size)| *size)
        .sum()
}

/// Download any missing / incomplete model file. Network IO, so it's async and
/// Serializes concurrent downloads (two overlapping recognitions must not race on
/// the same files) — async mutex, held across the awaits.
static DL_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));
/// Unique temp-file suffix per attempt, so a stray concurrent writer can never
/// clobber another attempt's `.part` file.
static DL_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Download any missing / incomplete model file. Uses its OWN client: no total
/// request deadline (a fixed one would make big files impossible on slow links),
/// only a connect timeout plus a per-chunk stall guard; the body is streamed to a
/// uniquely-named `.part` file (flat memory) and renamed once the size checks out,
/// so an interrupted download never looks complete.
pub async fn ensure_models(dir: &Path, _client: &reqwest::Client) -> Result<()> {
    let _guard = DL_LOCK.lock().await;
    std::fs::create_dir_all(dir).ok();
    // Reclaim the previous engine's weights (best-effort).
    for name in LEGACY_FILES {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .context("client di download")?;
    for (name, url, size) in MODELS.iter() {
        let path = dir.join(name);
        if std::fs::metadata(&path).map(|m| m.len() == *size).unwrap_or(false) {
            continue; // downloaded by us or by the attempt that held the lock first
        }
        let mut resp = client
            .get(*url)
            .send()
            .await
            .with_context(|| format!("scarico {name}"))?;
        if !resp.status().is_success() {
            anyhow::bail!("scarico {name}: HTTP {}", resp.status());
        }
        let seq = DL_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = dir.join(format!("{name}.part{seq}"));
        let write_res: Result<u64> = async {
            use std::io::Write;
            let mut file = std::fs::File::create(&tmp).with_context(|| format!("creo {name}"))?;
            let mut written: u64 = 0;
            loop {
                // Stall guard: a chunk must arrive within 60s (slow is fine, dead is not).
                let chunk = tokio::time::timeout(std::time::Duration::from_secs(60), resp.chunk())
                    .await
                    .map_err(|_| anyhow!("scarico {name}: connessione interrotta (nessun dato per 60s)"))?
                    .with_context(|| format!("leggo {name}"))?;
                let Some(chunk) = chunk else { break };
                file.write_all(&chunk).with_context(|| format!("salvo {name}"))?;
                written += chunk.len() as u64;
            }
            file.flush().ok();
            Ok(written)
        }
        .await;
        let written = match write_res {
            Ok(w) => w,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
        };
        if *size != 0 && written != *size {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("{name}: dimensione inattesa ({written} invece di {size} byte)");
        }
        std::fs::rename(&tmp, &path).with_context(|| format!("rinomino {name}"))?;
    }
    Ok(())
}

/// The loaded ONNX sessions + tokenizer, plus the introspected input/output names
/// so we bind by name (robust to export differences).
struct Engine {
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
    enc_in: String,
    /// decoder input names, discovered by dtype: ids (first int), mask (second
    /// int, optional — MFR's decoder has none), context (the float input).
    dec_ids: String,
    dec_mask: Option<(String, TensorElementType)>,
    dec_ctx: String,
}

// One engine per process, built lazily on first recognition and reused.
static ENGINE: Lazy<Mutex<Option<Engine>>> = Lazy::new(|| Mutex::new(None));

fn build_session(path: &Path) -> Result<Session> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    Session::builder()
        .map_err(|e| anyhow!(e.to_string()))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow!(e.to_string()))?
        .with_intra_threads(threads)
        .map_err(|e| anyhow!(e.to_string()))?
        .commit_from_file(path)
        .map_err(|e| anyhow!("carico {}: {e}", path.display()))
}

fn elem_ty(vt: &ValueType) -> Option<TensorElementType> {
    match vt {
        ValueType::Tensor { ty, .. } => Some(*ty),
        _ => None,
    }
}

impl Engine {
    fn load(dir: &Path) -> Result<Engine> {
        let encoder = build_session(&dir.join("encoder_model.onnx"))?;
        let decoder = build_session(&dir.join("decoder_model.onnx"))?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("tokenizer: {e}"))?;

        let enc_in = encoder
            .inputs()
            .first()
            .ok_or_else(|| anyhow!("encoder senza input"))?
            .name()
            .to_string();

        // Classify decoder inputs by element type. Model order is preserved, so the
        // first integer input is the token ids and the second (if any) is the mask;
        // the float input is the encoder context.
        let mut dec_ctx: Option<String> = None;
        let mut ints: Vec<(String, TensorElementType)> = Vec::new();
        for o in decoder.inputs() {
            let name = o.name().to_string();
            match elem_ty(o.dtype()) {
                Some(TensorElementType::Float32) | Some(TensorElementType::Float16) => {
                    dec_ctx = Some(name)
                }
                Some(ty) => ints.push((name, ty)),
                None => {}
            }
        }
        let dec_ids = ints
            .first()
            .map(|(n, _)| n.clone())
            .ok_or_else(|| anyhow!("decoder senza input token"))?;
        let dec_mask = ints.get(1).cloned();
        let dec_ctx = dec_ctx.ok_or_else(|| anyhow!("decoder senza input context"))?;

        Ok(Engine {
            encoder,
            decoder,
            tokenizer,
            enc_in,
            dec_ids,
            dec_mask,
            dec_ctx,
        })
    }

    /// Run the encoder on a normalized [1,3,384,384] image; return the context
    /// tensor (`last_hidden_state`) as (data, shape). The graph declares dynamic
    /// dims, so the fixed size is enforced here, not by ONNX.
    fn encode(&mut self, pixels: Vec<f32>) -> Result<(Vec<f32>, Vec<usize>)> {
        let side = MFR_SIZE as usize;
        let arr = Array::from_shape_vec((1usize, 3usize, side, side), pixels)
            .context("costruisco tensore immagine")?;
        let inputs = ort::inputs![ self.enc_in.as_str() => Value::from_array(arr)? ];
        let outputs = self.encoder.run(inputs).map_err(|e| anyhow!(e.to_string()))?;
        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!(e.to_string()))?;
        let shape: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        Ok((data.to_vec(), shape))
    }

    /// Beam-search autoregressive decode. Keeps the `BEAM` most-probable partial
    /// sequences and returns the best finished one by length-normalized
    /// log-probability, plus whether it actually finished (reached EOS). An
    /// unfinished winner means the transcription overflowed MAX_SEQ — in practice
    /// the crop holds more than one line and the model spiraled into a verbose
    /// `\begin{array}…` it never closes; callers use the flag to rescue that case.
    /// All active beams share the current length, so they are batched into a
    /// single decoder call per step (≈ as fast as greedy). With `BEAM = 1` this
    /// reduces to plain greedy decoding.
    fn decode(&mut self, ctx: &[f32], ctx_shape: &[usize]) -> Result<(Vec<i64>, bool)> {
        let s = *ctx_shape.get(1).unwrap_or(&0);
        let d = *ctx_shape.get(2).unwrap_or(&0);
        // (tokens, cumulative log-probability)
        let mut active: Vec<(Vec<i64>, f32)> = vec![(vec![BOS], 0.0)];
        let mut finished: Vec<(Vec<i64>, f32)> = Vec::new();

        for _ in 0..MAX_SEQ {
            if active.is_empty() {
                break;
            }
            let na = active.len();
            let len = active[0].0.len();

            // Batched inputs: ids [na,len], mask [na,len] (all valid), ctx [na,S,D].
            let mut ids_flat = Vec::with_capacity(na * len);
            for (toks, _) in &active {
                ids_flat.extend(toks.iter().copied());
            }
            let ids = Array::from_shape_vec((na, len), ids_flat)
                .context("costruisco tensore token")?;
            let mut ctx_flat = Vec::with_capacity(na * s * d);
            for _ in 0..na {
                ctx_flat.extend_from_slice(ctx);
            }
            let ctx_arr = Array::from_shape_vec((na, s, d), ctx_flat)
                .context("costruisco tensore context")?;

            let mut inp = ort::inputs![ self.dec_ids.as_str() => Value::from_array(ids)? ];
            if let Some((mask_name, mask_ty)) = &self.dec_mask {
                match mask_ty {
                    TensorElementType::Bool => inp.push((
                        mask_name.as_str().into(),
                        Value::from_array(Array::from_elem((na, len), true))?.into(),
                    )),
                    _ => inp.push((
                        mask_name.as_str().into(),
                        Value::from_array(Array::from_elem((na, len), 1i64))?.into(),
                    )),
                }
            }
            inp.push((self.dec_ctx.as_str().into(), Value::from_array(ctx_arr)?.into()));

            let outputs = self.decoder.run(inp).map_err(|e| anyhow!(e.to_string()))?;
            let (shape, logits) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!(e.to_string()))?;
            let vocab = *shape.last().unwrap_or(&0) as usize;
            if vocab == 0 {
                break;
            }
            let l = shape[shape.len() - 2] as usize;

            // Expand every active beam by its top-BEAM next tokens.
            let mut cands: Vec<(Vec<i64>, f32)> = Vec::with_capacity(na * BEAM);
            for (bi, (toks, score)) in active.iter().enumerate() {
                let row = &logits[(bi * l + (l - 1)) * vocab..(bi * l + l) * vocab];
                for (tok, lp) in topk_logsoftmax(row, BEAM) {
                    let mut nt = toks.clone();
                    nt.push(tok);
                    cands.push((nt, score + lp));
                }
            }
            // Best-first: route EOS-terminated candidates to `finished`, keep up to
            // BEAM still-open ones active.
            cands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let mut next_active: Vec<(Vec<i64>, f32)> = Vec::with_capacity(BEAM);
            for (toks, score) in cands {
                if toks.last() == Some(&EOS) {
                    finished.push((toks, score));
                } else if next_active.len() < BEAM {
                    next_active.push((toks, score));
                }
            }
            active = next_active;
            if finished.len() >= BEAM {
                break;
            }
        }

        // Prefer a completed (EOS-terminated) hypothesis; fall back to the best
        // still-open beam only if none finished. Otherwise a confident but
        // unterminated continuation could win and yield truncated LaTeX.
        let norm = |t: &(Vec<i64>, f32)| {
            let n = (t.0.len().saturating_sub(1)).max(1) as f32; // exclude BOS
            t.1 / n.powf(LENGTH_PENALTY)
        };
        let terminated = !finished.is_empty();
        let pool = if terminated { finished } else { active };
        let best = pool
            .into_iter()
            .max_by(|a, b| norm(a).partial_cmp(&norm(b)).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| anyhow!("decoder senza sequenze"))?;
        Ok((best.0, terminated))
    }
}

/// Top-`k` (token id, log-probability) pairs from a logits row, via log-softmax.
fn topk_logsoftmax(row: &[f32], k: usize) -> Vec<(i64, f32)> {
    let maxv = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let sum: f32 = row.iter().map(|&x| (x - maxv).exp()).sum();
    let lse = maxv + sum.ln();
    let k = k.min(row.len()).max(1);
    let mut idx: Vec<usize> = (0..row.len()).collect();
    idx.select_nth_unstable_by(k - 1, |&a, &b| {
        row[b].partial_cmp(&row[a]).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut top: Vec<usize> = idx[..k].to_vec();
    top.sort_by(|&a, &b| row[b].partial_cmp(&row[a]).unwrap_or(std::cmp::Ordering::Equal));
    top.into_iter().map(|i| (i as i64, row[i] - lse)).collect()
}


// ---------------------------------------------------------------------------
// Image preprocessing (port of pix2tex PreProcess)
// ---------------------------------------------------------------------------

/// Flatten any alpha onto white and return an 8-bit grayscale image.
fn to_gray_on_white(bytes: &[u8]) -> Result<GrayImage> {
    let img = image::load_from_memory(bytes).context("decodifico immagine")?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut gray = GrayImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let [r, g, b, a] = p.0;
        let af = a as f32 / 255.0;
        // composite over white
        let rf = r as f32 * af + 255.0 * (1.0 - af);
        let gf = g as f32 * af + 255.0 * (1.0 - af);
        let bf = b as f32 * af + 255.0 * (1.0 - af);
        let lum = (0.299 * rf + 0.587 * gf + 0.114 * bf).round().clamp(0.0, 255.0) as u8;
        gray.put_pixel(x, y, Luma([lum]));
    }
    Ok(gray)
}

/// Contrast-stretch to full range, orient so the text is dark on light, and crop
/// to the ink bounding box (with the polarity detected BEFORE any inversion, so
/// night-mode selections come out right too).
fn crop_ink(gray: &GrayImage) -> GrayImage {
    let (w, h) = gray.dimensions();
    let n = (w * h) as usize;
    // Contrast stretch: (v - min) / (max - min) * 255.
    let mut mn = 255u8;
    let mut mx = 0u8;
    for p in gray.pixels() {
        let v = p.0[0];
        mn = mn.min(v);
        mx = mx.max(v);
    }
    let range = (mx.saturating_sub(mn)).max(1) as f32;
    let mut data = vec![0u8; n];
    let mut sum = 0f64;
    for (i, p) in gray.pixels().enumerate() {
        let v = (((p.0[0].saturating_sub(mn)) as f32) / range * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        data[i] = v;
        sum += v as f64;
    }
    let mean = sum / n as f64;
    let dark_bg = mean <= 128.0;
    // Ink = dark pixels (light bg) or light pixels (dark bg), computed BEFORE any
    // inversion, to match the reference's mask.
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let v = data[(y * w + x) as usize];
            let ink = if dark_bg { v > 128 } else { v < 128 };
            if ink {
                any = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if dark_bg {
        for v in data.iter_mut() {
            *v = 255 - *v;
        }
    }
    let (a, b, cw, ch) = if any {
        (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
    } else {
        (0, 0, w, h)
    };
    let mut canvas = GrayImage::new(cw, ch);
    for yy in 0..ch {
        for xx in 0..cw {
            let v = data[((b + yy) * w + (a + xx)) as usize];
            canvas.put_pixel(xx, yy, Luma([v]));
        }
    }
    canvas
}

/// MFR preprocessing: ink-crop plus a small white margin, then resize STRAIGHT to
/// the fixed 384×384 (aspect-destroying — that is what the model's own
/// preprocessor does and was trained with; square-padding instead shrinks wide
/// formulas and loses thin marks like `\bar{}`), replicate to 3 channels and
/// normalize `(v/255 − 0.5)/0.5` per the preprocessor_config.
fn preprocess_384(gray: &GrayImage) -> Vec<f32> {
    let ink = crop_ink(gray);
    let (w, h) = ink.dimensions();
    // A margin ~4% of each side keeps strokes off the border after resizing.
    let mx = (w / 25).max(4);
    let my = (h / 25).max(4);
    let mut framed = GrayImage::from_pixel(w + 2 * mx, h + 2 * my, Luma([255]));
    image::imageops::overlay(&mut framed, &ink, mx as i64, my as i64);
    let resized = image::imageops::resize(
        &framed,
        MFR_SIZE,
        MFR_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    // [1,3,384,384]: channel-major, grayscale replicated on the three channels.
    let plane: Vec<f32> = resized
        .pixels()
        .map(|p| (p.0[0] as f32 / 255.0 - 0.5) / 0.5)
        .collect();
    let mut out = Vec::with_capacity(plane.len() * 3);
    for _ in 0..3 {
        out.extend_from_slice(&plane);
    }
    out
}

/// Runs of consecutive non-blank rows ("ink bands"), via a contrast-relative ink
/// test with background-polarity detection (like `crop_ink()`), so dark-themed pages
/// segment too. Shared by `segment_bands` and the bisection rescue.
fn ink_bands(gray: &GrayImage) -> Vec<(u32, u32)> {
    let (w, h) = gray.dimensions();
    if h == 0 || w == 0 {
        return Vec::new();
    }
    // Contrast-relative ink test.
    let (mut mn, mut mx) = (255u8, 0u8);
    for p in gray.pixels() {
        let v = p.0[0];
        mn = mn.min(v);
        mx = mx.max(v);
    }
    let range = (mx.saturating_sub(mn)).max(1) as f32;
    let stretch = |v: u8| ((v.saturating_sub(mn)) as f32 / range * 255.0) as u8;
    let mean: f64 = gray.pixels().map(|p| stretch(p.0[0]) as f64).sum::<f64>() / (w as f64 * h as f64);
    let dark_bg = mean <= 128.0;
    let is_ink = |v: u8| {
        let s = stretch(v);
        if dark_bg {
            s > 115
        } else {
            s < 140
        }
    };
    let blank_limit = (w / 100).max(1); // a row with <~1% ink counts as blank
    let mut bands: Vec<(u32, u32)> = Vec::new();
    let mut start: Option<u32> = None;
    for y in 0..h {
        let mut c = 0u32;
        for x in 0..w {
            if is_ink(gray.get_pixel(x, y).0[0]) {
                c += 1;
            }
        }
        if c > blank_limit {
            if start.is_none() {
                start = Some(y);
            }
        } else if let Some(st) = start.take() {
            bands.push((st, y - 1));
        }
    }
    if let Some(st) = start {
        bands.push((st, h - 1));
    }
    bands
}

/// Split a multi-line selection into separate formula bands by horizontal
/// projection. Conservative: only breaks at blank gaps taller than a fraction of
/// the tallest ink band, so a single formula's internal gaps (around fraction
/// bars, sub/superscripts) never split it. Returns the whole image unchanged when
/// there is only one line.
fn segment_bands(gray: &GrayImage) -> Vec<GrayImage> {
    let (w, h) = gray.dimensions();
    if h < 8 || w == 0 {
        return vec![gray.clone()];
    }
    let bands = ink_bands(gray);
    if bands.len() <= 1 {
        return vec![gray.clone()];
    }
    // Merge bands separated by small gaps (parts of one formula). Scale the gap
    // threshold from the tallest band so it tracks the text size.
    let max_h = bands.iter().map(|(a, b)| b - a + 1).max().unwrap_or(1);
    let gap_thresh = (((max_h as f32) * 0.6).round() as u32).max(6);
    let mut merged: Vec<(u32, u32)> = vec![bands[0]];
    for &(a, b) in &bands[1..] {
        let last = merged.last_mut().unwrap();
        if a - last.1 - 1 < gap_thresh {
            last.1 = b;
        } else {
            merged.push((a, b));
        }
    }
    if merged.len() <= 1 {
        return vec![gray.clone()];
    }
    // Crop each band full-width, with a little vertical breathing room.
    let pad_v = (max_h / 8).max(2);
    merged
        .into_iter()
        .map(|(a, b)| {
            let y0 = a.saturating_sub(pad_v);
            let y1 = (b + pad_v).min(h - 1);
            image::imageops::crop_imm(gray, 0, y0, w, y1 - y0 + 1).to_image()
        })
        .collect()
}

/// Cut the image in two at the WIDEST blank gap between ink bands (min 4px), for
/// the bisection rescue. In a stacked-lines crop the inter-line gap is wider than
/// any gap inside one formula (fraction bars, sum limits), so the widest gap is a
/// natural, threshold-free line boundary. None when there is nothing to split.
fn split_at_widest_gap(gray: &GrayImage) -> Option<(GrayImage, GrayImage)> {
    let (w, h) = gray.dimensions();
    let bands = ink_bands(gray);
    if bands.len() < 2 {
        return None;
    }
    let (mut cut, mut widest) = (0u32, 0u32);
    for pair in bands.windows(2) {
        let gap = pair[1].0 - pair[0].1 - 1;
        if gap > widest {
            widest = gap;
            cut = pair[0].1 + 1 + gap / 2;
        }
    }
    if widest < 4 {
        return None;
    }
    let top = image::imageops::crop_imm(gray, 0, 0, w, cut).to_image();
    let bottom = image::imageops::crop_imm(gray, 0, cut, w, h - cut).to_image();
    Some((top, bottom))
}

/// Structural sanity of a LaTeX transcription: balanced braces (honoring `\{`)
/// and as many `\begin` as `\end`. Any CORRECT transcription passes — unlike
/// "does it render in engine X", this is renderer-independent — so it can safely
/// gate the rescue: a failure here means the model emitted garbage (typically an
/// unclosed `\begin{array}…` on a stacked-lines crop).
fn latex_is_sane(s: &str) -> bool {
    let b = s.as_bytes();
    let mut depth: i64 = 0;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => {
                i += 2; // skip the escaped char (`\{`, `\}`, `\\`)
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    depth == 0 && s.matches("\\begin").count() == s.matches("\\end").count()
}

// ---------------------------------------------------------------------------
// Post-processing (collapse the tokenizer's spacing to compact LaTeX)
// ---------------------------------------------------------------------------

static RE_NOLET_LET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([\W_^\d])\s+?([a-zA-Z])").unwrap());
static RE_LET_NOLET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([a-zA-Z])\s+?([\W_^\d])").unwrap());
static RE_NOLET_NOLET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([\W_^\d])\s+?([\W_^\d])").unwrap());
// Math text-mode commands whose braced argument holds letters/operators with no
// meaningful spaces: `\mathrm{d}` (differentials), `\operatorname{sign}`,
// `\mathbf{v}` etc. Reference pix2tex collapses the spacing inside these up front
// and treats them as atomic — without it, `\mathrm { d }` keeps spurious spaces
// and a multi-letter `\operatorname{s i g n}` never rejoins (the generic passes
// below never collapse a space between two letters), so the command around a
// differential ends up looking mangled. `\text{…}` is deliberately excluded: its
// spaces are literal, and the generic passes already leave letter-letter spaces
// untouched, so it needs no protection.
static RE_MATH_CMD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\\(?:operatorname|mathrm|mathbf|mathbb|mathcal|mathsf|mathtt|mathfrak)\s*\*?\s*\{[^{}]*\}").unwrap()
});
// A space that MUST be kept: it terminates a control word (`\mu`, `\Sigma`) right
// before a letter — dropping it would fuse them into an undefined command
// (`\mu m` → `\mum`). Everything else inside a math command is safe to compact.
static RE_KEEP_SPACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\\[A-Za-z]+) +([A-Za-z])").unwrap());

/// Compact the spacing inside a matched `\mathrm{…}`-style command: drop every
/// space except the one that terminates a control word before a letter.
fn compact_math_arg(m: &str) -> String {
    // Protect required spaces with a sentinel (SOH, never in LaTeX), strip the rest,
    // then restore. `\u{1}` can't appear in OCR output, so this is unambiguous.
    RE_KEEP_SPACE
        .replace_all(m, "$1\u{1}$2")
        .replace(' ', "")
        .replace('\u{1}', " ")
}

// Integral commands (\int, \iint, \oint, \iiint, …): the gate for the differential
// heuristic — a bare `dx` is virtually always a differential inside an integral.
// The `(?:[^A-Za-z]|$)` requires a non-letter (or end) right after `int`, so this
// matches `\int_`, `\int f`, `\iint\,` … but NOT `\intercal`/`\intertext` (where a
// letter follows) — those would otherwise fire the rewrite with no real integral.
// A bare `\b` can't be used: `\int_` has `_` (a word char) after `t`, so no boundary.
static RE_HAS_INTEGRAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"\\[io]*int(?:[^A-Za-z]|$)").unwrap());
// A differential `d<var>` in a differential position: at the start, or right after
// whitespace or a spacing/opening delimiter (` `, `\,`→`,`, `\;`→`;`, `\!`→`!`, `(`).
// Deliberately NOT after `^`/`_` (an integral limit like `\int_0^d`), a letter (a
// word), a backslash (a command like `\det`), or `{` (a derivative denominator) —
// those are where a `d` is not a differential. `\b` after the variable rejects `dxy`.
static RE_DIFFERENTIAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(^|[\s(,;!])d\s?([A-Za-z])\b").unwrap());

/// Upright differentials: inside an integral, rewrite `dx` → `\mathrm{d}x` so the
/// differential renders roman (the bundled model transcribes it italic). Opt-in for
/// the local engine; conservative by construction (see the regexes) and idempotent
/// (`\mathrm{d}x`'s `d` sits after `{`, which is excluded). Never touches the vision
/// engine, which is prompted for `\mathrm` instead.
fn upright_differentials(s: &str) -> String {
    if !RE_HAS_INTEGRAL.is_match(s) {
        return s.to_string();
    }
    RE_DIFFERENTIAL.replace_all(s, "${1}\\mathrm{d}${2}").into_owned()
}

fn post_process(s: &str) -> String {
    let s = s
        .replace('Ġ', " ")
        .replace("[BOS]", "")
        .replace("[EOS]", "")
        .replace("[PAD]", "");
    // Collapse the inner spacing of `\mathrm{…}` & math friends atomically so the
    // generic passes below can't leave stray spaces — keeps e.g. `\mathrm { d } x`
    // as `\mathrm{d}x` and `\operatorname{s i g n}` as `\operatorname{sign}`
    // (pix2tex reference step).
    let mut s = RE_MATH_CMD
        .replace_all(&s, |c: &regex::Captures| compact_math_arg(&c[0]))
        .into_owned();
    // Iterate the three space-collapsing passes until stable (pix2tex).
    for _ in 0..8 {
        let a = RE_NOLET_LET.replace_all(&s, "$1$2").into_owned();
        let b = RE_LET_NOLET.replace_all(&a, "$1$2").into_owned();
        let c = RE_NOLET_NOLET.replace_all(&b, "$1$2").into_owned();
        if c == s {
            break;
        }
        s = c;
    }
    // Mark differentials upright inside integrals (dx → \mathrm{d}x). After the
    // space passes so `d x` is already `d x` and offsets are settled.
    upright_differentials(s.trim())
}

/// Recognize a single grayscale formula image with the already-loaded engine.
/// The boolean is the decoder's EOS flag: `false` means the transcription
/// overflowed (typically a multi-line crop) and the text is unreliable.
fn recognize_gray(eng: &mut Engine, gray: &GrayImage) -> Result<(String, bool)> {
    let pixels = preprocess_384(gray);
    let (ctx, ctx_shape) = eng.encode(pixels)?;
    let (ids, terminated) = eng.decode(&ctx, &ctx_shape)?;
    // Skip the leading BOS; the tokenizer drops the rest of the special tokens.
    let toks: Vec<u32> = ids.iter().skip(1).map(|&x| x as u32).collect();
    let raw = eng
        .tokenizer
        .decode(&toks, true)
        .map_err(|e| anyhow!("decode tokenizer: {e}"))?;
    Ok((post_process(&raw), terminated))
}

/// Recognize `gray`, rescuing a bad transcription by bisection: when the decode
/// overflows OR the LaTeX is structurally broken (unbalanced braces / unmatched
/// `\begin` — the model's tell on stacked-line crops, where it spirals into an
/// unclosed `\begin{array}…`), cut at the widest blank gap and recurse on the two
/// halves. Good lines land in `lines`.
///
/// `budget` is the shared decode allowance for the whole rescue tree — it strictly
/// decreases on every call, so the recursion terminates and the worst-case latency
/// is bounded (each garbage decode runs the full 512-step beam loop, the slow kind).
/// With equal inter-line gaps the widest-gap cut peels ONE line per split, so fully
/// resolving N lines costs ~2N-1 decodes; when the budget runs out mid-tree the
/// enclosing split is rejected and that region keeps its whole-crop text instead —
/// content is never dropped.
///
/// Returns `(clean, complete)`: whether any pushed line passed the sanity gate, and
/// whether this subtree fully covered its crop (false only on budget exhaustion).
/// A split is accepted only if BOTH halves are complete AND at least one is clean —
/// otherwise (e.g. a hard single-line formula whose widest gap is internal, like a
/// sum's limits: both halves garbage) the whole-crop text is kept, never fragments.
fn rescue_lines(
    eng: &mut Engine,
    gray: &GrayImage,
    budget: &mut u32,
    lines: &mut Vec<String>,
) -> Result<(bool, bool)> {
    if *budget == 0 {
        return Ok((false, false)); // no allowance to even look at this region
    }
    *budget -= 1;
    let (latex, terminated) = recognize_gray(eng, gray)?;
    let ok = terminated && latex_is_sane(&latex);
    if !ok {
        if let Some((top, bottom)) = split_at_widest_gap(gray) {
            let mut sub: Vec<String> = Vec::new();
            let (a_clean, a_complete) = rescue_lines(eng, &top, budget, &mut sub)?;
            let (b_clean, b_complete) = if a_complete {
                rescue_lines(eng, &bottom, budget, &mut sub)?
            } else {
                (false, false) // don't burn budget on the bottom of a rejected split
            };
            if a_complete && b_complete && (a_clean || b_clean) {
                lines.append(&mut sub);
                return Ok((a_clean || b_clean, true));
            }
            // Split rejected (budget ran out, or both halves were garbage too):
            // fall through to the whole-crop text.
        }
    }
    // Sane result — or nothing left to split: keep the text (the user can edit it).
    if !latex.trim().is_empty() {
        lines.push(latex);
    }
    Ok((ok, true))
}

/// Stack recognized lines: one stays plain, more become a `gathered` block
/// (independent equations, each centered on its own line — an `&`-less `aligned`
/// would silently not align at all).
fn join_lines(mut lines: Vec<String>) -> Result<String> {
    if lines.is_empty() {
        anyhow::bail!("nessuna formula riconosciuta");
    }
    if lines.len() == 1 {
        return Ok(lines.pop().unwrap());
    }
    let body = lines.join(" \\\\\n");
    Ok(format!("\\begin{{gathered}}\n{body}\n\\end{{gathered}}"))
}

/// Recognize a formula image (PNG/any) from in-memory bytes, using the models
/// under `dir`. Pure CPU; call from a blocking context. `ensure_models` must have
/// succeeded first. When `multi` is set, a multi-line selection is segmented into
/// separate equations up front; either way, a garbage transcription (overflowed or
/// structurally broken LaTeX — what the model produces on stacked-line crops) is
/// rescued by bisecting the crop at its widest blank gap and recognizing the
/// halves, so a multi-line selection yields a clean `gathered` block instead.
pub fn recognize(dir: &Path, image_bytes: &[u8], multi: bool) -> Result<String> {
    let gray = to_gray_on_white(image_bytes)?;

    let mut guard = ENGINE.lock();
    if guard.is_none() {
        *guard = Some(Engine::load(dir)?);
    }
    let eng = guard.as_mut().unwrap();

    // Decode budget: 9 fully resolves a 4-line stack under one-line-per-split
    // peeling (2N-1) with margin; beyond that the rescue degrades gracefully
    // (good head lines + one whole-tail chunk) instead of running unbounded.
    const RESCUE_BUDGET: u32 = 9;
    let mut lines: Vec<String> = Vec::new();
    if multi {
        // Explicit multi-line mode: conservative segmentation first, then each band
        // still gets the bisection rescue (a band may itself hold merged lines).
        // The budget is shared across bands but every band is guaranteed at least
        // its own plain decode (the pre-rescue behavior), so none is ever skipped.
        let bands = segment_bands(&gray);
        let mut budget = RESCUE_BUDGET.max(3 * bands.len() as u32);
        for band in bands {
            budget = budget.max(1);
            rescue_lines(eng, &band, &mut budget, &mut lines)?;
        }
    } else {
        let mut budget = RESCUE_BUDGET;
        rescue_lines(eng, &gray, &mut budget, &mut lines)?;
    }
    join_lines(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_process_compacts_spacing() {
        assert_eq!(post_process("\\frac { 1 } { 2 }"), "\\frac{1}{2}");
        assert_eq!(post_process("x ^ { 2 } + y"), "x^{2}+y");
    }

    #[test]
    fn post_process_preserves_mathrm() {
        // The command survives and its inner spaces are compacted (differential `d`).
        assert_eq!(post_process("\\int f \\mathrm { d } x"), "\\int f\\mathrm{d}x");
        // A leading \mathrm must never be dropped or split from its backslash.
        assert_eq!(post_process("\\mathrm { d } y"), "\\mathrm{d}y");
        // Multi-letter operator names rejoin (the generic passes never would).
        assert_eq!(post_process("\\operatorname { s i g n } ( x )"), "\\operatorname{sign}(x)");
        // \mathbb / \mathbf single letters stay intact.
        assert_eq!(post_process("\\mathbb { R } ^ { n }"), "\\mathbb{R}^{n}");
        // A control word inside the arg keeps its terminating space: `\mu m`
        // (micrometers) must NOT fuse into the undefined `\mum`.
        assert_eq!(post_process("\\mathrm { \\mu m }"), "\\mathrm{\\mu m}");
        assert_eq!(post_process("\\mathbf { \\Sigma x }"), "\\mathbf{\\Sigma x}");
    }

    #[test]
    fn differentials_upright_in_integrals() {
        // Inside an integral, `dx` becomes an upright differential.
        assert_eq!(
            post_process("\\int f ( x ) \\, d x"),
            "\\int f(x)\\,\\mathrm{d}x"
        );
        // Several differentials in a double integral.
        assert_eq!(
            post_process("\\iint g \\, d x \\, d y"),
            "\\iint g\\,\\mathrm{d}x\\,\\mathrm{d}y"
        );
        // Idempotent: running again doesn't double-wrap.
        assert_eq!(post_process("\\int f\\,\\mathrm{d}x"), "\\int f\\,\\mathrm{d}x");
    }

    #[test]
    fn differentials_left_alone_when_unsafe() {
        // No integral in the formula → never touch a bare `d` (letter-letter spaces
        // are not collapsed by the generic passes, matching existing pix2tex output).
        assert_eq!(post_process("a d x + b"), "a d x+b");
        // A `d` that is an integral limit (superscript) must NOT become a differential.
        assert_eq!(post_process("\\int _ 0 ^ d x"), "\\int_0^d x");
        // `\det` and other `\d…` commands are safe (the `d` follows a backslash).
        assert_eq!(post_process("\\int \\det ( A ) \\, d t"), "\\int\\det(A)\\,\\mathrm{d}t");
        // `\intercal` (transpose) must NOT satisfy the integral gate: a `(dx)`
        // product with no real integral stays untouched.
        assert_eq!(post_process("A ^ { \\intercal } ( d x )"), "A^{\\intercal}(d x)");
        // But a real `\int_` subscript still enables the rewrite.
        assert_eq!(post_process("\\int _ 0 ^ 1 f \\, d x"), "\\int_0^1f\\,\\mathrm{d}x");
    }

    /// The bisection rescue's building blocks. Two display lines whose tight gap the
    /// conservative segmentation merges must still split at the WIDEST gap; internal
    /// micro-gaps (fraction bar, sum limits) must never be the cut point.
    #[test]
    fn widest_gap_split() {
        // Sparse ink (every 4th column) like real glyphs — a solid block would tip
        // the mean over the dark-background polarity heuristic.
        let block = |img: &mut GrayImage, y0: u32, y1: u32| {
            for y in y0..=y1 {
                for x in (0..img.width()).step_by(4) {
                    img.put_pixel(x, y, Luma([0]));
                }
            }
        };
        // Two 60px lines, 12px gap: conservative segmentation merges (12 < 0.6×60),
        // the rescue split cuts inside the gap (rows 60..71).
        let mut two = GrayImage::from_pixel(300, 132, Luma([255]));
        block(&mut two, 0, 59);
        block(&mut two, 72, 131);
        assert_eq!(segment_bands(&two).len(), 1, "conservative merges tight lines");
        let (top, bottom) = split_at_widest_gap(&two).expect("splits at the 12px gap");
        assert!((60..=72).contains(&top.height()), "cut inside the gap: {}", top.height());
        assert_eq!(top.height() + bottom.height(), 132);
        // A formula with an internal 5px gap (sum limits) and a 14px line gap: the
        // widest gap wins, so the cut lands in the line gap, not inside the formula.
        let mut mixed = GrayImage::from_pixel(300, 159, Luma([255]));
        block(&mut mixed, 0, 39); // formula body
        block(&mut mixed, 45, 65); // its limits, 5px internal gap
        block(&mut mixed, 80, 158); // next line, 14px gap
        let (t, _) = split_at_widest_gap(&mixed).expect("splits at the line gap");
        assert!((66..=80).contains(&t.height()), "cut in the 14px gap: {}", t.height());
        // Nothing to split → None (single band; micro-gaps below 4px).
        let mut one = GrayImage::from_pixel(300, 60, Luma([255]));
        block(&mut one, 0, 27);
        block(&mut one, 30, 59); // 2px gap: below the 4px floor
        assert!(split_at_widest_gap(&one).is_none(), "micro-gaps never split");
    }

    /// The structural gate that triggers the rescue: correct LaTeX passes, the
    /// model's stacked-lines garbage (unbalanced braces, unclosed \begin) fails.
    #[test]
    fn latex_sanity() {
        assert!(latex_is_sane(r"\frac{x^{2}}{a^{2}}-\frac{y^{2}}{b^{2}}=1"));
        assert!(latex_is_sane(r"\begin{gathered} a=b \\ c=d \end{gathered}"));
        assert!(latex_is_sane(r"\left\{x\right\}")); // escaped braces don't count
        assert!(latex_is_sane(r"\{a\}+\|b\|"));
        // The real garbage observed on a stacked two-line crop:
        assert!(!latex_is_sane(r"\begin{array}{c}{{\displaystyle\frac{}{}\!\!\!\!\begin"));
        assert!(!latex_is_sane(r"\frac{a}{b")); // unbalanced braces
        assert!(!latex_is_sane(r"a}b{")); // premature close
        assert!(!latex_is_sane(r"\begin{array}{c}x\\y")); // begin without end
    }

    /// Spike: end-to-end recognition against a real image. Runs only when
    /// MATHOCR_DIR (models) and MATHOCR_TEST_IMG are set; otherwise a no-op.
    /// Prints model I/O specs and the recognized LaTeX (run with --nocapture).
    #[test]
    fn spike_recognize() {
        let dir = match std::env::var("MATHOCR_DIR") {
            Ok(d) => std::path::PathBuf::from(d),
            Err(_) => return,
        };
        let img = match std::env::var("MATHOCR_TEST_IMG") {
            Ok(d) => d,
            Err(_) => return,
        };
        if !models_present(&dir) {
            eprintln!("[spike] models missing in {}, skipping", dir.display());
            return;
        }
        let bytes = std::fs::read(&img).expect("read test image");
        let mut eng = Engine::load(&dir).expect("load engine");
        for i in eng.encoder.inputs() {
            eprintln!("[spike] ENC IN  {} : {:?}", i.name(), i.dtype());
        }
        for o in eng.encoder.outputs() {
            eprintln!("[spike] ENC OUT {} : {:?}", o.name(), o.dtype());
        }
        for i in eng.decoder.inputs() {
            eprintln!("[spike] DEC IN  {} : {:?}", i.name(), i.dtype());
        }
        for o in eng.decoder.outputs() {
            eprintln!("[spike] DEC OUT {} : {:?}", o.name(), o.dtype());
        }
        let gray = to_gray_on_white(&bytes).expect("gray");
        let pixels = preprocess_384(&gray);
        let (ctx, shape) = eng.encode(pixels).expect("encode");
        eprintln!("[spike] context shape {:?}", shape);
        let (ids, terminated) = eng.decode(&ctx, &shape).expect("decode");
        let toks: Vec<u32> = ids.iter().skip(1).map(|&x| x as u32).collect();
        let raw = eng.tokenizer.decode(&toks, true).expect("detok");
        eprintln!("[spike] terminated = {terminated}");
        eprintln!("[spike] raw   = {raw}");
        eprintln!("[spike] LATEX = {}", post_process(&raw));
    }

    /// Spike: the full single-mode entry point (`recognize`, multi=false) against a
    /// real image — exercises the truncation rescue on stacked-line crops. Env-gated
    /// like the other spikes.
    #[test]
    fn spike_recognize_single() {
        let dir = match std::env::var("MATHOCR_DIR") {
            Ok(d) => std::path::PathBuf::from(d),
            Err(_) => return,
        };
        let img = match std::env::var("MATHOCR_TEST_IMG") {
            Ok(d) => d,
            Err(_) => return,
        };
        if !models_present(&dir) {
            return;
        }
        let bytes = std::fs::read(&img).expect("read test image");
        let out = recognize(&dir, &bytes, false).expect("recognize single");
        eprintln!("[spike-single] LATEX =\n{out}");
    }

    /// Spike: multi-formula. Stacks MATHOCR_TEST_IMG over MATHOCR_TEST_IMG2 with a
    /// gap and checks that segmentation + `recognize(multi=true)` returns both as
    /// an `aligned` block. No-op unless the env vars and models are present.
    #[test]
    fn spike_multi() {
        let dir = match std::env::var("MATHOCR_DIR") {
            Ok(d) => std::path::PathBuf::from(d),
            Err(_) => return,
        };
        let (Ok(a), Ok(b)) = (
            std::env::var("MATHOCR_TEST_IMG"),
            std::env::var("MATHOCR_TEST_IMG2"),
        ) else {
            return;
        };
        if !models_present(&dir) {
            return;
        }
        let g1 = to_gray_on_white(&std::fs::read(&a).unwrap()).unwrap();
        let g2 = to_gray_on_white(&std::fs::read(&b).unwrap()).unwrap();
        let gap = 40u32;
        let w = g1.width().max(g2.width());
        let h = g1.height() + gap + g2.height();
        let mut canvas = GrayImage::from_pixel(w, h, Luma([255]));
        image::imageops::overlay(&mut canvas, &g1, 0, 0);
        image::imageops::overlay(&mut canvas, &g2, 0, (g1.height() + gap) as i64);
        eprintln!("[spike-multi] bands detected: {}", segment_bands(&canvas).len());
        let mut buf = Vec::new();
        image::DynamicImage::ImageLuma8(canvas)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let out = recognize(&dir, &buf, true).expect("recognize multi");
        eprintln!("[spike-multi] LATEX =\n{out}");
    }
}
