//! Built-in embeddings for Content > Embeddings: a small BERT-family
//! sentence-embedding model run on the CPU with candle (pure Rust, no
//! native libraries), so page text never leaves the machine.
//!
//! Models are downloaded once into a folder (see [`ModelInfo::files`]);
//! this crate only loads and runs them. Long pages are split into chunks
//! the model can read (up to 512 tokens), each chunk is embedded, and the
//! page vector is the normalised average of its chunks.

use anyhow::Context;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use std::path::{Path, PathBuf};
use tokenizers::{PaddingParams, Tokenizer, TruncationParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pooling {
    /// The [CLS] token's vector (BGE).
    Cls,
    /// The mean of all token vectors (E5, MiniLM).
    Mean,
}

/// A model Toad can download and run.
#[derive(Debug, Clone, Copy)]
pub struct ModelInfo {
    /// The name used in the configuration.
    pub id: &'static str,
    pub label: &'static str,
    /// Hugging Face repository.
    pub repo: &'static str,
    pub dims: usize,
    pub pooling: Pooling,
    /// Text put before each passage (E5 models expect "passage: ").
    pub prefix: &'static str,
    /// Approximate download size in MB.
    pub size_mb: u32,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "bge-small-en-v1.5",
        label: "BGE Small (English, recommended)",
        repo: "BAAI/bge-small-en-v1.5",
        dims: 384,
        pooling: Pooling::Cls,
        prefix: "",
        size_mb: 134,
    },
    ModelInfo {
        id: "all-MiniLM-L6-v2",
        label: "MiniLM L6 (English, fastest)",
        repo: "sentence-transformers/all-MiniLM-L6-v2",
        dims: 384,
        pooling: Pooling::Mean,
        prefix: "",
        size_mb: 91,
    },
    ModelInfo {
        id: "multilingual-e5-small",
        label: "E5 Small (multilingual, ~100 languages)",
        repo: "intfloat/multilingual-e5-small",
        dims: 384,
        pooling: Pooling::Mean,
        prefix: "passage: ",
        size_mb: 471,
    },
];

pub const DEFAULT_MODEL: &str = "bge-small-en-v1.5";

/// The files a model needs, all from the repository root.
pub const FILES: &[&str] = &["config.json", "tokenizer.json", "model.safetensors"];

impl ModelInfo {
    pub fn find(id: &str) -> Option<&'static ModelInfo> {
        MODELS.iter().find(|m| m.id.eq_ignore_ascii_case(id.trim()))
    }

    /// (file name, download URL) for each file.
    pub fn files(&self) -> Vec<(&'static str, String)> {
        FILES.iter().map(|f| (*f, format!("https://huggingface.co/{}/resolve/main/{f}", self.repo))).collect()
    }

    pub fn is_downloaded(&self, dir: &Path) -> bool {
        FILES.iter().all(|f| dir.join(f).metadata().is_ok_and(|m| m.len() > 0))
    }
}

/// Call at the very start of `main`, before any other thread exists.
/// Candle splits each matrix multiply across `RAYON_NUM_THREADS` threads;
/// for a model this small, one page per core ([`Embedder::embed_many`])
/// with single-threaded maths inside is about 50% faster. A value the user
/// set is left alone.
pub fn configure_threads() {
    if std::env::var_os("RAYON_NUM_THREADS").is_none() {
        // Safe: called before any other thread starts.
        std::env::set_var("RAYON_NUM_THREADS", "1");
    }
}

/// Words per chunk (about 270 tokens of English). Attention cost grows with
/// the square of the chunk length, so moderate chunks are much faster than
/// filling the model's 512-token window, with little loss for topic
/// similarity.
const CHUNK_WORDS: usize = 200;
/// Chunks embedded per page; longer pages are represented by their first
/// 800 words, where the topic is set out.
pub const MAX_CHUNKS: usize = 4;

pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    info: ModelInfo,
    device: Device,
}

impl Embedder {
    /// Load a downloaded model from `dir`.
    pub fn load(info: &ModelInfo, dir: &Path) -> anyhow::Result<Self> {
        anyhow::ensure!(info.is_downloaded(dir), "the {} model isn't downloaded", info.label);
        let device = Device::Cpu;
        let config: Config = serde_json::from_str(&std::fs::read_to_string(dir.join("config.json"))?).context("reading the model's config.json")?;
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow::anyhow!("reading tokenizer.json: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams { max_length: 512, ..Default::default() }))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tokenizer.with_padding(Some(PaddingParams::default()));
        // SAFETY: the file is mapped read-only and not modified while loaded.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[dir.join("model.safetensors")], DType::F32, &device)? };
        let model = BertModel::load(vb, &config).context("loading the model weights")?;
        Ok(Self { model, tokenizer, info: *info, device })
    }

    pub fn info(&self) -> &ModelInfo {
        &self.info
    }

    /// Embed several texts in one batch: one normalised vector each.
    fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        let inputs: Vec<String> = texts.iter().map(|t| format!("{}{t}", self.info.prefix)).collect();
        let enc = self.tokenizer.encode_batch(inputs, true).map_err(|e| anyhow::anyhow!("tokenizing: {e}"))?;
        let ids: Vec<Tensor> = enc.iter().map(|e| Tensor::new(e.get_ids(), &self.device)).collect::<Result<_, _>>()?;
        let masks: Vec<Tensor> = enc.iter().map(|e| Tensor::new(e.get_attention_mask(), &self.device)).collect::<Result<_, _>>()?;
        let ids = Tensor::stack(&ids, 0)?;
        let mask = Tensor::stack(&masks, 0)?;
        let types = ids.zeros_like()?;
        // (batch, tokens, hidden)
        let hidden = self.model.forward(&ids, &types, Some(&mask))?;
        let pooled = match self.info.pooling {
            Pooling::Cls => hidden.narrow(1, 0, 1)?.squeeze(1)?,
            Pooling::Mean => {
                let m = mask.to_dtype(DType::F32)?.unsqueeze(2)?;
                let summed = hidden.broadcast_mul(&m)?.sum(1)?;
                let counts = m.sum(1)?.clamp(1e-9, f64::MAX)?;
                summed.broadcast_div(&counts)?
            }
        };
        let norm = pooled.sqr()?.sum_keepdim(1)?.sqrt()?.clamp(1e-12, f64::MAX)?;
        let normed = pooled.broadcast_div(&norm)?;
        Ok(normed.to_vec2::<f32>()?)
    }

    /// Many pages at once, in parallel across the CPU's cores (one page per
    /// core is far faster than one page across all cores for a model this
    /// small).
    pub fn embed_many(&self, texts: &[String]) -> Vec<anyhow::Result<Vec<f32>>> {
        use rayon::prelude::*;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
            Ok(pool) => pool.install(|| texts.par_iter().map(|t| self.embed(t)).collect()),
            Err(_) => texts.iter().map(|t| self.embed(t)).collect(),
        }
    }

    /// One vector for a whole page: the normalised mean of its chunks.
    pub fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let chunks = chunk(text, CHUNK_WORDS, MAX_CHUNKS);
        anyhow::ensure!(!chunks.is_empty(), "no text to embed");
        let vecs = self.embed_batch(&chunks)?;
        Ok(average(&vecs))
    }
}

/// Split text into chunks of at most `words` words, keeping at most `max`.
pub fn chunk(text: &str, words: usize, max: usize) -> Vec<String> {
    let all: Vec<&str> = text.split_whitespace().collect();
    all.chunks(words.max(1)).take(max).map(|c| c.join(" ")).collect()
}

/// The normalised mean of unit vectors.
pub fn average(vecs: &[Vec<f32>]) -> Vec<f32> {
    let dims = vecs.first().map_or(0, Vec::len);
    let mut out = vec![0f32; dims];
    for v in vecs {
        for (o, x) in out.iter_mut().zip(v) {
            *o += x;
        }
    }
    let norm = out.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
    out.iter().map(|x| x / norm).collect()
}

/// Where models live: `<base>/models/<id>`.
pub fn model_dir(base: &Path, info: &ModelInfo) -> PathBuf {
    base.join("models").join(info.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_and_averages() {
        let text = (0..700).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
        let c = chunk(&text, 300, 8);
        assert_eq!(c.len(), 3);
        assert_eq!(c[2].split_whitespace().count(), 100);
        assert_eq!(chunk(&text, 300, 2).len(), 2);
        assert!(chunk("   ", 300, 8).is_empty());
        let a = average(&[vec![1.0, 0.0], vec![0.0, 1.0]]);
        assert!((a[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn catalogue() {
        let m = ModelInfo::find("BGE-small-en-v1.5").unwrap();
        assert_eq!(m.files()[2].1, "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/model.safetensors");
        assert!(ModelInfo::find(DEFAULT_MODEL).is_some());
        assert!(ModelInfo::find("nope").is_none());
    }
}
