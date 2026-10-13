//! Spider > Extraction > PDF end to end: properties columns, PDF text for
//! word count and Custom Search, links inside the PDF discovered and crawled,
//! and Bulk Export > All PDF Content.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::CustomSearch;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

/// A one-page PDF with properties, text and a link to `link`.
fn pdf(link: &str) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Annots [6 0 R] >>".into(),
        "STREAM".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        format!("<< /Type /Annot /Subtype /Link /Rect [0 0 100 20] /A << /S /URI /URI ({link}) >> >>"),
        "<< /Title (Toad Guide) /Author (Ann Frog) /Subject (Amphibians) /CreationDate (D:20240131093000Z) >>".into(),
    ];
    let content = "BT /F1 12 Tf 20 100 Td (Toads live in quiet ponds all year.) Tj ET";
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        let body = if o == "STREAM" { format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()) } else { o.clone() };
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R /Info 7 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

async fn serve(listener: TcpListener, origin: String) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let origin = origin.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (ctype, body) = match path.as_str() {
                "/guide.pdf" => ("application/pdf", pdf(&format!("{origin}/from-pdf"))),
                "/" => ("text/html", br#"<html><head><title>Home</title></head><body><a href="/guide.pdf">Guide</a></body></html>"#.to_vec()),
                _ => ("text/html", b"<html><head><title>From the PDF</title></head><body>Hi</body></html>".to_vec()),
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pdf_extraction() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener, origin.clone()));
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig {
        threads: 1,
        store_pdf: true,
        extract_pdf_properties: true,
        extract_pdf_links: true,
        custom_search: vec![CustomSearch { name: "Ponds".into(), query: "ponds".into(), ..Default::default() }],
        ..CrawlConfig::default()
    };
    let dir = std::env::temp_dir().join(format!("toad-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
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

    let guide = format!("{origin}/guide.pdf");
    let field = |url: &str, key: &str| store.url_details(url).unwrap().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default();
    assert_eq!(field(&guide, "pdf_author"), "Ann Frog");
    assert_eq!(field(&guide, "pdf_subject"), "Amphibians");
    assert_eq!(field(&guide, "pdf_created"), "2024-01-31 09:30:00");
    assert_eq!(field(&guide, "pdf_pages"), "1");
    assert_eq!(field(&guide, "title_1"), "Toad Guide", "the PDF Title fills Title 1");
    assert_eq!(field(&guide, "word_count"), "7");

    // The link inside the PDF was discovered and crawled.
    assert_eq!(field(&format!("{origin}/from-pdf"), "status_code"), "200");
    let inlinks = store.inlinks(&format!("{origin}/from-pdf")).unwrap();
    assert!(inlinks.rows.iter().any(|r| r[1] == guide && r[12] == "PDF page 1"));

    // Custom Search runs on PDF text.
    assert_eq!(store.count("Custom Search", "Ponds").unwrap(), 1);

    // Internal shows the PDF columns; All PDF Content exports the text.
    let internal = store.rows(&toad_store::RowQuery { tab: "Internal".into(), filter: "PDF".into(), ..Default::default() }).unwrap();
    assert!(internal.columns.iter().any(|c| c == "PDF Author"));
    let out = dir.join("pdf-text");
    assert_eq!(store.export_pdf_text(&out).unwrap(), 1);
    assert!(std::fs::read_to_string(out.join("000001.txt")).unwrap().contains("quiet ponds"));
}
