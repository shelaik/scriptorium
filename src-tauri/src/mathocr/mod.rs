//! Formula OCR: an image of a mathematical equation -> LaTeX.
//!
//! A faithful Rust port of the pix2tex / LaTeX-OCR ONNX pipeline (as packaged by
//! RapidAI/RapidLaTeXOCR): a ViT-with-ResNet-backbone encoder produces a context
//! sequence, and a Transformer decoder autoregressively emits LaTeX tokens.
//!
//! The ONNX Runtime is the very one already linked statically into the binary via
//! `fastembed` (bge-m3), so this adds no runtime and no extra DLL. The ~140 MB of
//! model weights are NOT bundled: they are downloaded once, on first use, into
//! `app_data/mathocr/` — the same "fetch on demand" pattern as the embeddings.
//!
//! Reference: <https://github.com/RapidAI/RapidLaTeXOCR> (Apache-2.0) /
//! <https://github.com/lukas-blecher/LaTeX-OCR> (MIT).

use anyhow::{anyhow, Context, Result};
use image::{GrayImage, Luma};
use ndarray::{Array, IxDyn};
use once_cell::sync::Lazy;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::{TensorElementType, Value, ValueType};
use parking_lot::Mutex;
use regex::Regex;
use std::path::Path;
use tokenizers::Tokenizer;

// --- Decoding / preprocessing constants (from the model's config) ---
const BOS: i64 = 1;
const EOS: i64 = 2;
const MAX_SEQ: usize = 512;
const MAX_W: u32 = 672; // max_dimensions width
const MAX_H: u32 = 192; // max_dimensions height
const PATCH: u32 = 32; // encoder needs H,W divisible by this
const MIN_W: u32 = 32; // min_dimensions width
const MIN_H: u32 = 32; // min_dimensions height
const MEAN: f32 = 0.7931;
const STD: f32 = 0.1738;

/// (local file name, download URL, exact size in bytes for integrity check).
/// The `image_resizer` network predicts the optimal render width so glyph scale
/// matches the training distribution — it is essential to accuracy, not optional.
const MODELS: [(&str, &str, u64); 4] = [
    (
        "encoder.onnx",
        "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/encoder.onnx",
        89_008_136,
    ),
    (
        "decoder.onnx",
        "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/decoder.onnx",
        50_952_726,
    ),
    (
        "image_resizer.onnx",
        "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/image_resizer.onnx",
        38_967_751,
    ),
    (
        "tokenizer.json",
        "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/tokenizer.json",
        24_174,
    ),
];

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
/// takes the app's shared reqwest client. Writes to a `.part` file then renames,
/// so an interrupted download never looks complete.
pub async fn ensure_models(dir: &Path, client: &reqwest::Client) -> Result<()> {
    std::fs::create_dir_all(dir).ok();
    for (name, url, size) in MODELS.iter() {
        let path = dir.join(name);
        if std::fs::metadata(&path).map(|m| m.len() == *size).unwrap_or(false) {
            continue;
        }
        let resp = client
            .get(*url)
            .send()
            .await
            .with_context(|| format!("scarico {name}"))?;
        if !resp.status().is_success() {
            anyhow::bail!("scarico {name}: HTTP {}", resp.status());
        }
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("leggo {name}"))?;
        if *size != 0 && bytes.len() as u64 != *size {
            anyhow::bail!(
                "{name}: dimensione inattesa ({} invece di {size} byte)",
                bytes.len()
            );
        }
        let tmp = dir.join(format!("{name}.part"));
        std::fs::write(&tmp, &bytes).with_context(|| format!("salvo {name}"))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("rinomino {name}"))?;
    }
    Ok(())
}

/// The loaded ONNX sessions + tokenizer, plus the introspected input/output names
/// so we bind by name (robust to export differences).
struct Engine {
    encoder: Session,
    decoder: Session,
    resizer: Session,
    tokenizer: Tokenizer,
    enc_in: String,
    res_in: String,
    /// decoder input names, discovered by dtype: ids (first int), mask (second
    /// int, optional), context (the float input).
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
        let encoder = build_session(&dir.join("encoder.onnx"))?;
        let decoder = build_session(&dir.join("decoder.onnx"))?;
        let resizer = build_session(&dir.join("image_resizer.onnx"))?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("tokenizer: {e}"))?;

        let enc_in = encoder
            .inputs()
            .first()
            .ok_or_else(|| anyhow!("encoder senza input"))?
            .name()
            .to_string();
        let res_in = resizer
            .inputs()
            .first()
            .ok_or_else(|| anyhow!("resizer senza input"))?
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
            resizer,
            tokenizer,
            enc_in,
            res_in,
            dec_ids,
            dec_mask,
            dec_ctx,
        })
    }

    /// Run the image_resizer on a normalized tensor; return the predicted
    /// render width = (argmax + 1) * 32.
    fn resize_width(&mut self, img: &[f32], h: usize, w: usize) -> Result<u32> {
        let arr = Array::from_shape_vec((1usize, 1usize, h, w), img.to_vec())
            .context("costruisco tensore resizer")?;
        let inputs = ort::inputs![ self.res_in.as_str() => Value::from_array(arr)? ];
        let outputs = self.resizer.run(inputs).map_err(|e| anyhow!(e.to_string()))?;
        let (_shape, logits) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!(e.to_string()))?;
        let idx = argmax(logits) as u32;
        Ok((idx + 1) * PATCH)
    }

    /// Reproduce pix2tex's `loop_image_resizer`: iteratively resize the base image
    /// to the width the resizer predicts, until it stabilizes; return the final
    /// normalized tensor (data, H, W) that then feeds the encoder.
    fn resize_loop(&mut self, base: &GrayImage) -> Result<(Vec<f32>, usize, usize)> {
        let mut r: f32 = 1.0;
        let mut w: u32 = base.width().max(1);
        let mut h: f32 = base.height() as f32;
        let mut last: Option<(Vec<f32>, usize, usize)> = None;
        for _ in 0..10 {
            h = (h * r).floor();
            let hh = (h as u32).max(1);
            let filt = if r > 1.0 {
                image::imageops::FilterType::Triangle // PIL BILINEAR
            } else {
                image::imageops::FilterType::Lanczos3
            };
            let resized = image::imageops::resize(base, w.max(1), hh, filt);
            let pad_img = pad(&minmax_size(&resized));
            let pw = pad_img.width();
            let (data, th, tw) = to_tensor(&pad_img);
            let w_new = self.resize_width(&data, th, tw)?;
            last = Some((data, th, tw));
            if w_new == pw {
                break;
            }
            r = w_new as f32 / pw as f32;
            w = w_new;
        }
        last.ok_or_else(|| anyhow!("resizer non ha prodotto output"))
    }

    /// Run the encoder; return the context tensor as (data, shape).
    fn encode(&mut self, img: Vec<f32>, h: usize, w: usize) -> Result<(Vec<f32>, Vec<usize>)> {
        let arr = Array::from_shape_vec((1usize, 1usize, h, w), img)
            .context("costruisco tensore immagine")?;
        let inputs = ort::inputs![ self.enc_in.as_str() => Value::from_array(arr)? ];
        let outputs = self.encoder.run(inputs).map_err(|e| anyhow!(e.to_string()))?;
        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!(e.to_string()))?;
        let shape: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        Ok((data.to_vec(), shape))
    }

    /// Greedy (argmax) autoregressive decode. Temperature in the reference config
    /// is ~1e-5, i.e. effectively argmax, so this is a faithful, deterministic
    /// equivalent with no RNG.
    fn decode(&mut self, ctx: &[f32], ctx_shape: &[usize]) -> Result<Vec<i64>> {
        let mut out: Vec<i64> = vec![BOS];
        for _ in 0..MAX_SEQ {
            let start = out.len().saturating_sub(MAX_SEQ);
            let cur = &out[start..];
            let len = cur.len();

            let ids = Array::from_shape_vec((1usize, len), cur.to_vec())
                .context("costruisco tensore token")?;
            let ctx_arr = Array::from_shape_vec(IxDyn(ctx_shape), ctx.to_vec())
                .context("costruisco tensore context")?;

            let mut inp = ort::inputs![ self.dec_ids.as_str() => Value::from_array(ids)? ];
            if let Some((mask_name, mask_ty)) = &self.dec_mask {
                // A padding mask of "all valid" — dtype must match the model's input.
                match mask_ty {
                    TensorElementType::Bool => inp.push((
                        mask_name.as_str().into(),
                        Value::from_array(Array::from_elem((1usize, len), true))?.into(),
                    )),
                    _ => inp.push((
                        mask_name.as_str().into(),
                        Value::from_array(Array::from_elem((1usize, len), 1i64))?.into(),
                    )),
                }
            }
            inp.push((self.dec_ctx.as_str().into(), Value::from_array(ctx_arr)?.into()));

            let outputs = self.decoder.run(inp).map_err(|e| anyhow!(e.to_string()))?;
            let (shape, logits) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!(e.to_string()))?;
            // logits: [1, L, V] — take the last position's row.
            let vocab = *shape.last().unwrap_or(&0) as usize;
            if vocab == 0 {
                break;
            }
            let l = shape[shape.len() - 2] as usize;
            let row = &logits[(l - 1) * vocab..l * vocab];
            let next = argmax(row);
            if next == EOS {
                break;
            }
            out.push(next);
        }
        Ok(out)
    }
}

fn argmax(row: &[f32]) -> i64 {
    let mut best = 0usize;
    let mut best_v = f32::NEG_INFINITY;
    for (i, &v) in row.iter().enumerate() {
        if v > best_v {
            best_v = v;
            best = i;
        }
    }
    best as i64
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

/// Faithful port of pix2tex `pad`: contrast-stretch to full range, orient so the
/// text is dark on light, crop to the ink bounding box, then pad (top-left) to the
/// next multiple of `PATCH` on a white canvas.
fn pad(gray: &GrayImage) -> GrayImage {
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
    let pw = cw.div_ceil(PATCH).max(1) * PATCH;
    let ph = ch.div_ceil(PATCH).max(1) * PATCH;
    let mut canvas = GrayImage::from_pixel(pw, ph, Luma([255]));
    for yy in 0..ch {
        for xx in 0..cw {
            let v = data[((b + yy) * w + (a + xx)) as usize];
            canvas.put_pixel(xx, yy, Luma([v]));
        }
    }
    canvas
}

/// Faithful port of pix2tex `minmax_size`: downscale (BILINEAR) so both sides fit
/// within (MAX_W, MAX_H) preserving aspect; then pad up to the minimum size.
fn minmax_size(img: &GrayImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let ratio = (w as f32 / MAX_W as f32).max(h as f32 / MAX_H as f32);
    let scaled = if ratio > 1.0 {
        let nw = ((w as f32 / ratio).floor() as u32).max(1);
        let nh = ((h as f32 / ratio).floor() as u32).max(1);
        image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    let (sw, sh) = scaled.dimensions();
    let pw = sw.max(MIN_W);
    let ph = sh.max(MIN_H);
    if pw == sw && ph == sh {
        return scaled;
    }
    let mut canvas = GrayImage::from_pixel(pw, ph, Luma([255]));
    image::imageops::overlay(&mut canvas, &scaled, 0, 0);
    canvas
}

/// Normalize a grayscale image to the model's (1,1,H,W) tensor.
/// `(v/255 - MEAN)/STD` is identical to the reference's `(v - MEAN*255)/(STD*255)`.
fn to_tensor(gray: &GrayImage) -> (Vec<f32>, usize, usize) {
    let (w, h) = gray.dimensions();
    let (w, h) = (w as usize, h as usize);
    let mut data = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let v = gray.get_pixel(x as u32, y as u32).0[0] as f32 / 255.0;
            data.push((v - MEAN) / STD);
        }
    }
    (data, h, w)
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

fn post_process(s: &str) -> String {
    let mut s = s
        .replace('Ġ', " ")
        .replace("[BOS]", "")
        .replace("[EOS]", "")
        .replace("[PAD]", "");
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
    s.trim().to_string()
}

/// Recognize a formula image (PNG/any) already on disk-loaded bytes, using the
/// models under `dir`. Pure CPU; call from a blocking context. `ensure_models`
/// must have succeeded first.
pub fn recognize(dir: &Path, image_bytes: &[u8]) -> Result<String> {
    let gray = to_gray_on_white(image_bytes)?;
    let base = minmax_size(&pad(&gray));

    let mut guard = ENGINE.lock();
    if guard.is_none() {
        *guard = Some(Engine::load(dir)?);
    }
    let eng = guard.as_mut().unwrap();

    let (data, h, w) = eng.resize_loop(&base)?;
    let (ctx, ctx_shape) = eng.encode(data, h, w)?;
    let ids = eng.decode(&ctx, &ctx_shape)?;

    // Skip the leading BOS; the tokenizer drops the rest of the special tokens.
    let toks: Vec<u32> = ids.iter().skip(1).map(|&x| x as u32).collect();
    let raw = eng
        .tokenizer
        .decode(&toks, true)
        .map_err(|e| anyhow!("decode tokenizer: {e}"))?;
    let latex = post_process(&raw);
    if latex.is_empty() {
        anyhow::bail!("nessuna formula riconosciuta");
    }
    Ok(latex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_process_compacts_spacing() {
        assert_eq!(post_process("\\frac { 1 } { 2 }"), "\\frac{1}{2}");
        assert_eq!(post_process("x ^ { 2 } + y"), "x^{2}+y");
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
        for i in eng.resizer.inputs() {
            eprintln!("[spike] RES IN  {} : {:?}", i.name(), i.dtype());
        }
        for o in eng.resizer.outputs() {
            eprintln!("[spike] RES OUT {} : {:?}", o.name(), o.dtype());
        }
        let gray = to_gray_on_white(&bytes).expect("gray");
        let base = minmax_size(&pad(&gray));
        let (data, h, w) = eng.resize_loop(&base).expect("resize_loop");
        eprintln!("[spike] resized to {w}x{h}");
        let (ctx, shape) = eng.encode(data, h, w).expect("encode");
        eprintln!("[spike] context shape {:?}", shape);
        let ids = eng.decode(&ctx, &shape).expect("decode");
        let toks: Vec<u32> = ids.iter().skip(1).map(|&x| x as u32).collect();
        let raw = eng.tokenizer.decode(&toks, true).expect("detok");
        eprintln!("[spike] raw   = {raw}");
        eprintln!("[spike] LATEX = {}", post_process(&raw));
    }
}
