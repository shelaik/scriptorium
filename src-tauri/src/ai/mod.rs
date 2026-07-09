//! Optional, local AI features via Ollama or LM Studio. Disabled by default —
//! the user turns it on in Settings, picks a provider and a model. Kept
//! lightweight (truncated input, capped output) so it runs on a modest laptop
//! GPU or slowly on CPU. Network calls go only to the local LLM server the user
//! configured (Ollama or LM Studio).

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// HTTP client with a long timeout — local LLM generation can be slow on CPU.
pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::limited(2))
        .build()
        .context("building AI HTTP client")
}

/// Whether the provider string selects LM Studio (OpenAI-compatible) rather than Ollama.
pub fn is_lmstudio(provider: &str) -> bool {
    let p = provider.trim();
    p.eq_ignore_ascii_case("lmstudio") || p.eq_ignore_ascii_case("lm_studio") || p.eq_ignore_ascii_case("lm studio")
}

/// Human-readable provider label for messages.
pub fn label(provider: &str) -> &'static str {
    if is_lmstudio(provider) {
        "LM Studio"
    } else {
        "Ollama"
    }
}

/// Normalize a base URL: trim whitespace, trailing slashes, and a trailing `/v1`
/// (so the user can paste either `http://host:1234` or `http://host:1234/v1`).
fn base_url(url: &str) -> String {
    let t = url.trim().trim_end_matches('/');
    t.strip_suffix("/v1").unwrap_or(t).trim_end_matches('/').to_string()
}

/// List the models a provider currently serves. Doubles as a reachability check
/// (errors if the server isn't running). `provider` is "ollama" or "lmstudio".
pub async fn list_models(client: &reqwest::Client, provider: &str, url: &str) -> Result<Vec<String>> {
    let base = base_url(url);
    if is_lmstudio(provider) {
        // LM Studio exposes an OpenAI-compatible API.
        let resp = client
            .get(format!("{base}/v1/models"))
            .send()
            .await
            .context("LM Studio non raggiungibile (server locale avviato?)")?;
        if !resp.status().is_success() {
            anyhow::bail!("LM Studio ha risposto HTTP {}", resp.status());
        }
        let body: Value = resp.json().await.context("risposta LM Studio non valida")?;
        let models = body["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    } else {
        let resp = client
            .get(format!("{base}/api/tags"))
            .send()
            .await
            .context("Ollama non raggiungibile (è in esecuzione?)")?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama ha risposto HTTP {}", resp.status());
        }
        let body: Value = resp.json().await.context("risposta Ollama non valida")?;
        let models = body["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }
}

/// Single-shot, non-streaming generation. `num_predict` caps output length.
pub async fn generate(
    client: &reqwest::Client,
    provider: &str,
    url: &str,
    model: &str,
    prompt: &str,
    num_predict: i64,
) -> Result<String> {
    let base = base_url(url);
    if is_lmstudio(provider) {
        let resp = client
            .post(format!("{base}/v1/chat/completions"))
            .json(&json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "max_tokens": num_predict,
                "temperature": 0.2,
                "stream": false
            }))
            .send()
            .await
            .context("richiesta a LM Studio fallita")?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("LM Studio HTTP {status}: {}", truncate(&txt, 300));
        }
        let body: Value = resp.json().await.context("risposta LM Studio non valida")?;
        Ok(body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string())
    } else {
        let resp = client
            .post(format!("{base}/api/generate"))
            .json(&json!({
                "model": model,
                "prompt": prompt,
                "stream": false,
                "options": { "num_predict": num_predict, "temperature": 0.2 }
            }))
            .send()
            .await
            .context("richiesta a Ollama fallita")?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama HTTP {status}: {}", truncate(&txt, 300));
        }
        let body: Value = resp.json().await.context("risposta Ollama non valida")?;
        Ok(body["response"].as_str().unwrap_or("").trim().to_string())
    }
}

/// Single-shot multimodal generation: sends `prompt` together with one PNG image
/// (`image_b64`, bare base64, no data-URL prefix) to a vision-capable model.
/// Ollama takes an `images` array; LM Studio takes an OpenAI `image_url` part.
/// Returns `(text, truncated)` where `truncated` is true if generation stopped
/// because it hit `num_predict` (so the caller can warn about a cut-off result).
pub async fn generate_vision(
    client: &reqwest::Client,
    provider: &str,
    url: &str,
    model: &str,
    prompt: &str,
    image_b64: &str,
    num_predict: i64,
) -> Result<(String, bool)> {
    let base = base_url(url);
    if is_lmstudio(provider) {
        let data_url = format!("data:image/png;base64,{image_b64}");
        let resp = client
            .post(format!("{base}/v1/chat/completions"))
            .json(&json!({
                "model": model,
                "messages": [{
                    "role": "user",
                    "content": [
                        { "type": "text", "text": prompt },
                        { "type": "image_url", "image_url": { "url": data_url } }
                    ]
                }],
                "max_tokens": num_predict,
                "temperature": 0.1,
                "stream": false
            }))
            .send()
            .await
            .context("richiesta vision a LM Studio fallita")?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("LM Studio HTTP {status}: {}", truncate(&txt, 300));
        }
        let body: Value = resp.json().await.context("risposta LM Studio non valida")?;
        let truncated = body["choices"][0]["finish_reason"].as_str() == Some("length");
        let text = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        Ok((text, truncated))
    } else {
        let resp = client
            .post(format!("{base}/api/generate"))
            .json(&json!({
                "model": model,
                "prompt": prompt,
                "images": [image_b64],
                "stream": false,
                "options": { "num_predict": num_predict, "temperature": 0.1 }
            }))
            .send()
            .await
            .context("richiesta vision a Ollama fallita")?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama HTTP {status}: {}", truncate(&txt, 300));
        }
        let body: Value = resp.json().await.context("risposta Ollama non valida")?;
        let truncated = body["done_reason"].as_str() == Some("length");
        let text = body["response"].as_str().unwrap_or("").trim().to_string();
        Ok((text, truncated))
    }
}

/// Streaming generation: invokes `on_token` with each text delta as it arrives
/// and returns the full text. Lets the RAG answer appear progressively.
pub async fn generate_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    provider: &str,
    url: &str,
    model: &str,
    prompt: &str,
    num_predict: i64,
    mut on_token: F,
) -> Result<String> {
    let base = base_url(url);
    let mut full = String::new();
    let mut buf: Vec<u8> = Vec::new();
    let lmstudio = is_lmstudio(provider);

    let mut resp = if lmstudio {
        client
            .post(format!("{base}/v1/chat/completions"))
            .json(&json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "max_tokens": num_predict,
                "temperature": 0.2,
                "stream": true
            }))
            .send()
            .await
            .context("richiesta a LM Studio fallita")?
    } else {
        client
            .post(format!("{base}/api/generate"))
            .json(&json!({
                "model": model,
                "prompt": prompt,
                "stream": true,
                "options": { "num_predict": num_predict, "temperature": 0.2 }
            }))
            .send()
            .await
            .context("richiesta a Ollama fallita")?
    };

    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        anyhow::bail!("{} HTTP {status}: {}", label(provider), truncate(&txt, 300));
    }

    // Both providers emit newline-delimited records (Ollama NDJSON, LM Studio SSE).
    let mut handle_line = |line: &str, full: &mut String, on_token: &mut F| {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let json_str = if lmstudio {
            match line.strip_prefix("data:") {
                Some(d) => d.trim(),
                None => return,
            }
        } else {
            line
        };
        if json_str.is_empty() || json_str == "[DONE]" {
            return;
        }
        if let Ok(v) = serde_json::from_str::<Value>(json_str) {
            let delta = if lmstudio {
                v["choices"][0]["delta"]["content"].as_str()
            } else {
                v["response"].as_str()
            };
            if let Some(t) = delta {
                if !t.is_empty() {
                    full.push_str(t);
                    on_token(t);
                }
            }
        }
    };

    while let Some(bytes) = resp.chunk().await.context("stream interrotto")? {
        buf.extend_from_slice(&bytes);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let s = String::from_utf8_lossy(&line[..line.len() - 1]);
            handle_line(&s, &mut full, &mut on_token);
        }
    }
    // Flush any trailing partial line (no terminating newline).
    if !buf.is_empty() {
        let s = String::from_utf8_lossy(&buf);
        handle_line(&s, &mut full, &mut on_token);
    }

    Ok(full.trim().to_string())
}

/// Compute embeddings for a batch of texts via Ollama (GPU-accelerated). Returns
/// one vector per input. Use bge-m3 to stay 1024-dim and compatible with the
/// CPU-built index.
pub async fn embed_ollama(
    client: &reqwest::Client,
    url: &str,
    model: &str,
    texts: Vec<String>,
) -> Result<Vec<Vec<f32>>> {
    let base = base_url(url);
    let resp = client
        .post(format!("{base}/api/embed"))
        .json(&json!({ "model": model, "input": texts, "keep_alive": "30m" }))
        .send()
        .await
        .context("richiesta embeddings a Ollama fallita")?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama embeddings HTTP {status}: {}", truncate(&txt, 300));
    }
    let body: Value = resp.json().await.context("risposta embeddings Ollama non valida")?;
    let arr = body["embeddings"]
        .as_array()
        .context("Ollama: campo 'embeddings' mancante (modello scaricato? `ollama pull bge-m3`)")?;
    let mut out = Vec::with_capacity(arr.len());
    for e in arr {
        let v: Vec<f32> = e
            .as_array()
            .context("Ollama: embedding non valido")?
            .iter()
            .filter_map(|x| x.as_f64().map(|f| f as f32))
            .collect();
        out.push(v);
    }
    Ok(out)
}

/// Truncate to roughly `max_chars`, respecting char boundaries.
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}
