//! Visualisations: crawl tree parents (shortest crawl path), the directory
//! tree with a virtual folder, and the anchor text word cloud.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

fn page(path: &str) -> String {
    let body = match path {
        "/" => r#"<a href="/blog/">Toad blog</a> <a href="/shop/item">Toad shop item</a> <a href="/about">About toads</a>"#,
        "/blog/" => r#"<a href="/blog/a">Toad post A</a> <a href="/blog/b">Toad post B</a>"#,
        "/blog/a" => r#"<a href="/blog/b">More toad posts</a>"#,
        _ => "",
    };
    format!("<html><head><title>{path}</title></head><body>{body}</body></html>")
}

#[tokio::test(flavor = "multi_thread")]
async fn trees_and_words() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let path = String::from_utf8_lossy(&buf[..n]).split_whitespace().nth(1).unwrap_or("/").to_string();
                let body = page(&path);
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(body.as_bytes()).await;
            });
        }
    });
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig::default();
    let db = std::env::temp_dir().join(format!("toad-viz-{}.toad", std::process::id()));
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

    // Crawl tree: /blog/b is linked from /blog/ (depth 1) and /blog/a (depth 2); its parent is /blog/.
    let tree = store.crawl_tree(1000).unwrap();
    let find = |g: &toad_store::viz::VizGraph, url: &str| g.nodes.iter().find(|n| n.url == url).cloned().unwrap_or_else(|| panic!("{url}"));
    let blog = find(&tree, &format!("{origin}/blog/"));
    assert_eq!(find(&tree, &format!("{origin}/blog/b")).parent, Some(blog.id));
    assert_eq!(find(&tree, &format!("{origin}/")).parent, None);
    assert_eq!(tree.nodes.iter().filter(|n| n.parent.is_none()).count(), 1);

    // Directory tree: /shop/ was never crawled, so it's a virtual folder.
    let dirs = store.directory_tree(1000).unwrap();
    let shop = find(&dirs, &format!("{origin}/shop/"));
    assert!(shop.virtual_node);
    assert_eq!(find(&dirs, &format!("{origin}/shop/item")).parent, Some(shop.id));
    let blog = find(&dirs, &format!("{origin}/blog/"));
    assert!(!blog.virtual_node, "a crawled folder page is the folder node");
    assert_eq!(find(&dirs, &format!("{origin}/blog/a")).parent, Some(blog.id));
    let root = find(&dirs, &format!("{origin}/"));
    assert_eq!(blog.parent, Some(root.id));
    assert_eq!(shop.parent, Some(root.id));

    // Inlink anchor text word cloud.
    let words = store.anchor_words(&format!("{origin}/blog/b"), 10).unwrap();
    assert!(words.iter().any(|(w, n)| w == "toad" && *n == 2), "{words:?}");

    // Limits.
    let small = store.crawl_tree(2).unwrap();
    assert!(small.truncated && small.nodes.len() == 2);
}
