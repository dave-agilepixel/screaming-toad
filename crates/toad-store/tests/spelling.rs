//! Content > Spelling & Grammar end to end: a local dictionary (no
//! download), a fake LanguageTool server, filters, details, the top-errors
//! panel, Ignore All and the reports.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::SpellingConfig;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\n";
const DIC: &str = "13\nand\nthe\ntoad\nlives\nin\na\npond\nhome\npage\nfrog\nsite\nabout\nfrogs\n";

/// Pages, plus a LanguageTool-style /v2/check that flags "a apple".
async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 16384];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (ctype, body) = match path.as_str() {
                "/v2/check" => {
                    let flagged = req.contains("a+apple") || req.contains("a%20apple");
                    let body = if flagged {
                        r#"{"matches":[{"message":"Use \"an\" instead of \"a\".","offset":0,"length":7,"replacements":[{"value":"an apple"}],"rule":{"id":"EN_A_VS_AN","category":{"id":"MISC"}}}]}"#
                    } else {
                        r#"{"matches":[]}"#
                    };
                    ("application/json", body.to_string())
                }
                "/" => ("text/html", r#"<html lang="en"><head><title>Home page</title></head><body><p>The toad livse in a pnd.</p><a href="/frog">frog</a></body></html>"#.to_string()),
                _ => ("text/html", r#"<html lang="en"><head><title>Frog site</title></head><body><p>a apple about frogs and a pnd.</p></body></html>"#.to_string()),
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn spelling_and_grammar() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let dir = std::env::temp_dir().join(format!("toad-spelling-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A pre-installed "en" dictionary, so nothing is downloaded.
    std::fs::write(dir.join("en.aff"), AFF).unwrap();
    std::fs::write(dir.join("en.dic"), DIC).unwrap();

    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig {
        threads: 1,
        spelling: SpellingConfig {
            spell_check: true,
            grammar_check: true,
            grammar_server: origin.clone(),
            dictionaries_dir: Some(dir.clone()),
            dictionary_words: vec!["apple".into()],
            ..SpellingConfig::default()
        },
        ..CrawlConfig::default()
    };
    let db = dir.join("crawl.toad");
    let mut store = Store::create(&db, &start, &cfg).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let run = tokio::spawn(Crawler::new(cfg, &start).unwrap().run(vec![start.clone()], tx, CrawlControl::default()));
    let mut records = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    run.await.unwrap().unwrap();
    store.insert_batch(&records).unwrap();
    store.analyse().unwrap();

    assert_eq!(store.count("Content", "Spelling Errors").unwrap(), 2, "both pages have 'pnd'");
    assert_eq!(store.count("Content", "Grammar Errors").unwrap(), 1, "/frog has 'a apple'");

    let home = store.language_details(&format!("{origin}/")).unwrap();
    let words: Vec<&str> = home.rows.iter().filter(|r| r[0] == "Spelling").map(|r| r[1].as_str()).collect();
    assert_eq!(words, vec!["livse", "pnd"]);
    let frog = store.language_details(&format!("{origin}/frog")).unwrap();
    assert!(frog.rows.iter().any(|r| r[0] == "Grammar" && r[2] == "an apple" && r[4] == "EN_A_VS_AN"));
    assert!(!frog.rows.iter().any(|r| r[1] == "apple"), "'apple' is in the user's Dictionary");

    let top = store.top_language_errors().unwrap();
    assert_eq!((top.rows[0][1].as_str(), top.rows[0][3].as_str()), ("pnd", "2"), "most common error first");

    // Ignore All 'pnd': gone from this crawl, counts updated, no re-crawl.
    assert_eq!(store.dismiss_language_error("Spelling", "pnd").unwrap(), 2);
    assert_eq!(store.count("Content", "Spelling Errors").unwrap(), 1, "only /'s 'livse' is left");
    assert_eq!(store.dismiss_language_error("Grammar", "EN_A_VS_AN").unwrap(), 1);
    assert_eq!(store.count("Content", "Grammar Errors").unwrap(), 0);

    assert!(store.report("Spelling & Grammar Errors").unwrap().rows.len() >= 1);
    assert!(store.report("Spelling & Grammar Errors Summary").unwrap().rows.len() >= 1);
}
