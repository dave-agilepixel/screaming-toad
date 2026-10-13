//! Content > Embeddings with the built-in model (`toad-embed`): download
//! on first use into ~/.toad/models, then embed pages on this machine.

use crate::{ApiRunner, StoreLock};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use toad_embed::{Embedder, ModelInfo};
use toad_store::api::EMBED;

/// ~/.toad, or $TOAD_HOME when set.
pub fn toad_home() -> PathBuf {
    if let Some(h) = std::env::var_os("TOAD_HOME") {
        return PathBuf::from(h);
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    home.join(".toad")
}

pub fn model(id: &str) -> anyhow::Result<&'static ModelInfo> {
    ModelInfo::find(id).ok_or_else(|| {
        anyhow::anyhow!("unknown built-in model '{id}' (choose {})", toad_embed::MODELS.iter().map(|m| m.id).collect::<Vec<_>>().join(", "))
    })
}

pub fn model_dir(info: &ModelInfo) -> PathBuf {
    toad_embed::model_dir(&toad_home(), info)
}

/// Download a model's files (skipping ones already there). `progress`
/// gets (bytes done, total bytes if known) for the whole model.
pub async fn download(client: &reqwest::Client, info: &ModelInfo, dir: &Path, progress: &(dyn Fn(u64, Option<u64>) + Send + Sync)) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    tokio::fs::create_dir_all(dir).await?;
    let total_guess = info.size_mb as u64 * 1_000_000;
    let mut done = 0u64;
    for (name, url) in info.files() {
        let path = dir.join(name);
        if path.metadata().is_ok_and(|m| m.len() > 0) {
            done += path.metadata().map(|m| m.len()).unwrap_or(0);
            continue;
        }
        let mut resp = client.get(&url).send().await?;
        anyhow::ensure!(resp.status().is_success(), "downloading {url}: HTTP {}", resp.status());
        let part = dir.join(format!("{name}.part"));
        let mut file = tokio::fs::File::create(&part).await?;
        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk).await?;
            done += chunk.len() as u64;
            progress(done, Some(total_guess.max(done)));
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&part, &path).await?;
    }
    progress(done, Some(done));
    Ok(())
}

/// Embed every target page with the built-in model, in batches of 48
/// pages embedded in parallel.
pub async fn run(runner: &ApiRunner, store: &dyn StoreLock, todo: Vec<String>) -> anyhow::Result<()> {
    let e = &runner.cfg.apis.embeddings;
    let info = model(&e.model)?;
    let dir = model_dir(info);
    if !info.is_downloaded(&dir) {
        let r = runner;
        download(&runner.client, info, &dir, &|done, total| r.report("embed_download", done / 1_000_000, total.unwrap_or(0) / 1_000_000)).await?;
    }
    let embedder = {
        let (info, dir) = (*info, dir.clone());
        Arc::new(tokio::task::spawn_blocking(move || Embedder::load(&info, &dir)).await??)
    };
    let total = todo.len() as u64;
    let max_chars = runner.cfg.apis.ai.max_input_chars;
    let mut done = 0u64;
    runner.report(EMBED, 0, total);
    for batch in todo.chunks(48) {
        let mut texts = Vec::new();
        let mut failed = Vec::new();
        for url in batch {
            match crate::ai::page_content(store, url, e.content, max_chars) {
                Ok(t) if !t.trim().is_empty() => texts.push((url.clone(), t)),
                Ok(_) => failed.push((url.clone(), "no text to embed".to_string())),
                Err(err) => failed.push((url.clone(), format!("{err:#}"))),
            }
        }
        let emb = embedder.clone();
        let results: Vec<(String, anyhow::Result<Vec<f32>>)> = tokio::task::spawn_blocking(move || {
            let (urls, bodies): (Vec<String>, Vec<String>) = texts.into_iter().unzip();
            urls.into_iter().zip(emb.embed_many(&bodies)).collect()
        })
        .await?;
        {
            let mut s = store.lock_store()?;
            for (u, r) in results {
                match r {
                    Ok(v) => s.put_embedding(&u, &v)?,
                    Err(err) => failed.push((u, format!("{err:#}"))),
                }
            }
        }
        crate::save(store, EMBED, &[], &failed)?;
        done += batch.len() as u64;
        runner.report(EMBED, done, total);
    }
    Ok(())
}
