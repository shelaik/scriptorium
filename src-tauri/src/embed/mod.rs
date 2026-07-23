//! Local text embeddings via fastembed (bge-m3, 1024-dim, multilingual).
//!
//! The model (~2.3GB) is downloaded into the cache dir on first use and reused
//! afterwards. The embedder is initialized lazily and guarded by a mutex
//! because `TextEmbedding::embed` takes `&mut self`.

use anyhow::{anyhow, Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use once_cell::sync::OnceCell;
use std::path::Path;
use std::sync::Mutex;

/// bge-m3 dense embedding dimension — must match the `doc_vec` vec0 table.
pub const EMBED_DIM: usize = 1024;

/// How many characters of fulltext to fold into a document's embedding text.
const FULLTEXT_CHARS: usize = 2000;

static EMBEDDER: OnceCell<Mutex<TextEmbedding>> = OnceCell::new();

/// True se la cache locale contiene già file del modello (nessun download
/// partirebbe). Usato dai chiamanti in background che NON devono mai innescare
/// il download da 2.3GB senza consenso (es. il filtro semantico dello sweep).
pub fn model_cached(cache_dir: &Path) -> bool {
    fn has_onnx(dir: &Path, depth: u8) -> bool {
        if depth > 3 {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return false };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if has_onnx(&p, depth + 1) {
                    return true;
                }
            } else if p.extension().and_then(|x| x.to_str()) == Some("onnx") {
                return true;
            }
        }
        false
    }
    has_onnx(cache_dir, 0)
}

/// Lazily initialize the bge-m3 embedder. The first call downloads the model
/// into `cache_dir`; later calls reuse the cached files.
fn embedder(cache_dir: &Path) -> Result<&'static Mutex<TextEmbedding>> {
    EMBEDDER.get_or_try_init(|| {
        std::fs::create_dir_all(cache_dir).ok();
        let opts = TextInitOptions::new(EmbeddingModel::BGEM3)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(true);
        let model = TextEmbedding::try_new(opts).context("initializing the bge-m3 embedder")?;
        Ok::<_, anyhow::Error>(Mutex::new(model))
    })
}

/// Compose the text used to represent a document in the embedding space:
/// title + abstract + a prefix of the fulltext.
pub fn compose_text(title: Option<&str>, abstract_: Option<&str>, fulltext: &str) -> String {
    let mut s = String::new();
    if let Some(t) = title {
        s.push_str(t);
        s.push('\n');
    }
    if let Some(a) = abstract_ {
        s.push_str(a);
        s.push('\n');
    }
    let prefix: String = fulltext.chars().take(FULLTEXT_CHARS).collect();
    s.push_str(&prefix);
    s
}

/// Embed a batch of texts into 1024-dim dense vectors.
pub fn embed_batch(cache_dir: &Path, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
    let m = embedder(cache_dir)?;
    let mut guard = m.lock().map_err(|_| anyhow!("embedder mutex poisoned"))?;
    let n = texts.len();
    let vectors = guard.embed(texts, None).context("embedding texts")?;
    // Must get exactly one vector per input (else the caller's id/vector zip truncates).
    if vectors.len() != n {
        return Err(anyhow!(
            "embedder returned {} vectors for {n} texts",
            vectors.len()
        ));
    }
    // Guard against a dimension mismatch with the vec0 table.
    if let Some(v) = vectors.first() {
        if v.len() != EMBED_DIM {
            return Err(anyhow!(
                "unexpected embedding dimension {} (expected {EMBED_DIM})",
                v.len()
            ));
        }
    }
    Ok(vectors)
}

/// Embed a single query string.
pub fn embed_query(cache_dir: &Path, text: &str) -> Result<Vec<f32>> {
    embed_batch(cache_dir, vec![text.to_string()])?
        .pop()
        .context("no embedding produced for query")
}

/// Project vectors onto their two principal components (PCA via power iteration
/// on the centered data, deterministic start, ~24 iterations per component).
/// Returns one (x, y) per input vector, scaled so the largest |coordinate| is 1 —
/// the Costellazione uses these as semantically meaningful seed positions, so the
/// force layout refines a sensible map instead of untangling a random spiral.
pub fn pca_2d(data: &[Vec<f32>]) -> Vec<(f32, f32)> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    let d = data[0].len();
    if n < 3 || d == 0 {
        return vec![(0.0, 0.0); n];
    }
    let mut mean = vec![0f32; d];
    for v in data {
        for (m, x) in mean.iter_mut().zip(v) {
            *m += x;
        }
    }
    for m in &mut mean {
        *m /= n as f32;
    }
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();

    let mut comps: Vec<Vec<f32>> = Vec::new();
    let mut seed = 0x9E37_79B9u32; // fixed → deterministic layout seeds
    for _ in 0..2 {
        let mut v: Vec<f32> = (0..d)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 8) as f32 / 16_777_216.0) - 0.5
            })
            .collect();
        for _ in 0..24 {
            // w = Xᵀ(Xv) on the centered data — never materializes the covariance.
            let mut w = vec![0f32; d];
            for row in data {
                let mut s = 0f32;
                for i in 0..d {
                    s += (row[i] - mean[i]) * v[i];
                }
                for i in 0..d {
                    w[i] += (row[i] - mean[i]) * s;
                }
            }
            // Deflate: stay orthogonal to the components already found.
            for c in &comps {
                let p = dot(&w, c);
                for i in 0..d {
                    w[i] -= p * c[i];
                }
            }
            let norm = dot(&w, &w).sqrt();
            if norm < 1e-12 {
                break;
            }
            for x in &mut w {
                *x /= norm;
            }
            v = w;
        }
        comps.push(v);
    }

    let mut out: Vec<(f32, f32)> = data
        .iter()
        .map(|row| {
            let (mut px, mut py) = (0f32, 0f32);
            for i in 0..d {
                let c = row[i] - mean[i];
                px += c * comps[0][i];
                py += c * comps[1][i];
            }
            (px, py)
        })
        .collect();
    let mx = out
        .iter()
        .map(|(x, y)| x.abs().max(y.abs()))
        .fold(0f32, f32::max)
        .max(1e-9);
    for p in &mut out {
        p.0 /= mx;
        p.1 /= mx;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pca_separates_two_blobs() {
        // Two tight clusters far apart along one direction: the first component
        // must separate them cleanly, whatever its sign.
        let mut data: Vec<Vec<f32>> = Vec::new();
        for i in 0..10 {
            let jitter = (i as f32) * 0.01;
            data.push(vec![5.0 + jitter, 0.1 * jitter, 0.0, 0.0]);
            data.push(vec![-5.0 - jitter, -0.1 * jitter, 0.0, 0.0]);
        }
        let proj = pca_2d(&data);
        assert_eq!(proj.len(), 20);
        // Alternating rows belong to opposite blobs → opposite signs on x.
        for pair in proj.chunks(2) {
            assert!(
                pair[0].0.signum() != pair[1].0.signum(),
                "blobs not separated: {pair:?}"
            );
        }
        // Normalized: everything within [-1, 1], at least one coordinate at ±1.
        assert!(proj.iter().all(|(x, y)| x.abs() <= 1.001 && y.abs() <= 1.001));
        assert!(proj.iter().any(|(x, _)| x.abs() > 0.99));
    }

    #[test]
    fn pca_degenerate_inputs() {
        assert!(pca_2d(&[]).is_empty());
        assert_eq!(pca_2d(&[vec![1.0, 2.0]]), vec![(0.0, 0.0)]);
        // Identical vectors: no variance → all projections collapse to ~0.
        let same = vec![vec![1.0f32; 8]; 5];
        assert!(pca_2d(&same).iter().all(|(x, y)| x.abs() < 1e-3 && y.abs() < 1e-3));
    }
}
