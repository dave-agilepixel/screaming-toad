//! Throughput of the built-in model: `cargo run --release -p toad-embed --example profile`.
use std::time::Instant;
fn main() -> anyhow::Result<()> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))?;
    let info = toad_embed::ModelInfo::find("bge-small-en-v1.5").unwrap();
    let dir = toad_embed::model_dir(&std::path::Path::new(&home).join(".toad"), info);
    let e = toad_embed::Embedder::load(info, &dir)?;
    // 40 pages of 600 words each (3 chunks per page).
    let page = "Toads are amphibians that live on land and return to water to breed every spring. ".repeat(40);
    let pages: Vec<String> = (0..40).map(|i| format!("Page {i}. {page}")).collect();
    let t = Instant::now();
    let r = e.embed_many(&pages);
    let secs = t.elapsed().as_secs_f64();
    assert!(r.iter().all(|v| v.is_ok()));
    println!("{} pages of ~600 words in {secs:.1}s: {:.1} pages/s", pages.len(), pages.len() as f64 / secs);
    Ok(())
}
