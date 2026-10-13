//! The built-in embedding model, for real: downloads BGE Small (~134 MB)
//! into ~/.toad/models (or $TOAD_HOME) on first run, so it only runs when
//! asked: `cargo test -p toad-apis --test local_embed -- --ignored --nocapture`.

use std::time::Instant;
use toad_apis::local_embed::{download, model, model_dir};
use toad_embed::Embedder;

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn bge_small_end_to_end() {
    let info = model("bge-small-en-v1.5").unwrap();
    let dir = model_dir(info);
    let started = Instant::now();
    download(&reqwest::Client::new(), info, &dir, &|done, total| {
        if done % 20_000_000 < 70_000 {
            eprintln!("downloaded {} / {} MB", done / 1_000_000, total.unwrap_or(0) / 1_000_000);
        }
    })
    .await
    .unwrap();
    eprintln!("model ready in {:.1}s", started.elapsed().as_secs_f64());
    assert!(info.is_downloaded(&dir));

    let t = Instant::now();
    let e = Embedder::load(info, &dir).unwrap();
    eprintln!("loaded in {:.2}s", t.elapsed().as_secs_f64());

    let toads = e.embed("Common toads live in damp gardens and hunt slugs and beetles at night near ponds.").unwrap();
    let natterjack = e.embed("Natterjack toads breed in shallow warm pools on sand dunes and heathland.").unwrap();
    let tax = e.embed("Small businesses must file their quarterly VAT return and pay corporation tax on time.").unwrap();
    assert_eq!(toads.len(), 384);
    assert!((cosine(&toads, &toads) - 1.0).abs() < 1e-4, "vectors are normalised");
    let same_topic = cosine(&toads, &natterjack);
    let other_topic = cosine(&toads, &tax);
    eprintln!("toads~natterjack {same_topic:.3}, toads~tax {other_topic:.3}");
    assert!(same_topic > other_topic + 0.1, "same topic should be clearly closer");

    eprintln!("threads: {}, avx: {}", candle_core::utils::get_num_threads(), candle_core::utils::with_avx());
    for words in [100usize, 200, 300] {
        let text = "Toads are amphibians that live on land and return to water to breed. ".repeat(words / 13 + 1);
        let t = Instant::now();
        for _ in 0..3 {
            e.embed(&text.split_whitespace().take(words).collect::<Vec<_>>().join(" ")).unwrap();
        }
        eprintln!("{words}-word chunk: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0 / 3.0);
    }
    // A long page: chunked, then averaged.
    let long = "Toads are amphibians that live on land and return to water to breed. ".repeat(400);
    let t = Instant::now();
    let v = e.embed(&long).unwrap();
    eprintln!("long page ({} words) embedded in {:.2}s", long.split_whitespace().count(), t.elapsed().as_secs_f64());
    assert!(cosine(&v, &toads) > other_topic);
}

/// A crawl with Content > Embeddings on the built-in model: two pages about
/// toads are Semantically Similar; a page about tax is Low Relevance.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn crawl_with_built_in_embeddings() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
    use toad_store::{RowQuery, Store};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let path = String::from_utf8_lossy(&buf[..n]).split_whitespace().nth(1).unwrap_or("/").to_string();
                let body = match path.as_str() {
                    "/" => "<p>Welcome to our wildlife pond guide.</p><a href='/a'>a</a> <a href='/b'>b</a> <a href='/c'>c</a>",
                    "/a" => "<p>Common toads hibernate under logs and return each spring to the same pond to breed, laying long strings of spawn.</p>",
                    "/b" => "<p>Toads migrate back to their ancestral breeding ponds in early spring, where females lay spawn in long strings among pond weeds.</p>",
                    _ => "<p>Our accountants can file your quarterly VAT return, payroll and corporation tax so your small business stays compliant.</p>",
                };
                let html = format!("<html><head><title>{path}</title></head><body>{body}</body></html>");
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", html.len());
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(html.as_bytes()).await;
            });
        }
    });
    let mut cfg = CrawlConfig::default();
    cfg.apis.embeddings.enabled = true; // Built-in, BGE Small by default.
    // Paraphrased pages score about 0.78 with BGE Small; unrelated ones about 0.45.
    cfg.apis.embeddings.similarity_threshold = 0.75;
    cfg.apis.embeddings.low_relevance_threshold = 0.75;
    let start = url::Url::parse(&format!("{origin}/")).unwrap();
    let db = std::env::temp_dir().join(format!("toad-local-embed-{}.toad", std::process::id()));
    let store = std::sync::Mutex::new(Store::create(&db, &start, &cfg).unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let run = tokio::spawn(Crawler::new(cfg.clone(), &start).unwrap().run(vec![start.clone()], tx, CrawlControl::default()));
    let mut records = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    run.await.unwrap().unwrap();
    store.lock().unwrap().insert_batch(&records).unwrap();
    let runner = toad_apis::ApiRunner::new(cfg, toad_apis::Secrets::default());
    let errors = runner.fetch_per_url(&store).await;
    assert!(errors.is_empty(), "{errors:?}");
    let s = store.into_inner().unwrap();
    let rows = |filter: &str| s.rows(&RowQuery { tab: "Content".into(), filter: filter.into(), ..Default::default() }).unwrap();
    let similar: Vec<String> = rows("Semantically Similar").rows.iter().map(|r| r[0].clone()).collect();
    let all = rows("All");
    let col = all.columns.iter().position(|c| c == "Semantic Similarity Score").unwrap();
    for r in &all.rows {
        eprintln!("{}  similarity {}  relevance {}", r[0], r[col], r[col + 2]);
    }
    assert!(similar.contains(&format!("{origin}/a")) && similar.contains(&format!("{origin}/b")), "{similar:?}");
    assert!(!similar.contains(&format!("{origin}/c")));
    let low: Vec<String> = rows("Low Relevance Content").rows.iter().map(|r| r[0].clone()).collect();
    assert!(low.contains(&format!("{origin}/c")), "{low:?}");
}
