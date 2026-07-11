//! Shared download-on-first-use plumbing for ONNX models (formula OCR, table
//! structure): pinned immutable URLs, exact-size integrity checks, streaming to a
//! uniquely-named `.part` file (flat memory), a per-chunk stall guard instead of a
//! total deadline (big files must survive slow links), and one global async lock
//! so concurrent recognitions never race on the same files.

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use std::path::Path;

/// (local file name, download URL, exact size in bytes for the integrity check).
pub type ModelFile = (&'static str, &'static str, u64);

/// Serializes concurrent model downloads app-wide (they are rare and disk/network
/// bound — one at a time is right). Async mutex: held across awaits.
static DL_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));
/// Unique temp-file suffix per attempt, so a stray concurrent writer can never
/// clobber another attempt's `.part` file.
static DL_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// True when every file is present at its expected size.
pub fn all_present(dir: &Path, files: &[ModelFile]) -> bool {
    files.iter().all(|(name, _, size)| {
        std::fs::metadata(dir.join(name)).map(|m| m.len() == *size).unwrap_or(false)
    })
}

/// Total bytes still missing — for a user-facing "first use downloads N MB" note.
pub fn missing_bytes(dir: &Path, files: &[ModelFile]) -> u64 {
    files
        .iter()
        .filter(|(name, _, size)| {
            std::fs::metadata(dir.join(name)).map(|m| m.len() != *size).unwrap_or(true)
        })
        .map(|(_, _, size)| *size)
        .sum()
}

/// Download any missing / incomplete file in `files` into `dir`. Uses its own
/// client: no total request deadline, only a connect timeout plus a 60s per-chunk
/// stall guard; the body streams to a `.part{seq}` file renamed only after the
/// exact-size check, so an interrupted download never looks complete.
pub async fn fetch(dir: &Path, files: &[ModelFile]) -> Result<()> {
    let _guard = DL_LOCK.lock().await;
    std::fs::create_dir_all(dir).ok();
    // Reclaim temp files a crashed attempt left behind (~100 MB each). They are
    // ours by construction, and nothing can be writing them while we hold the lock.
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().contains(".part") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .context("client di download")?;
    for (name, url, size) in files.iter() {
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
