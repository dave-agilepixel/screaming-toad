//! Notifications: the crawl-complete email, sent to a mock SMTP server.

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use toad_apis::notify::{compose, send, CrawlSummary, NotificationSettings, SmtpSecurity};
use toad_store::{IssueRow, IssueType, Priority};

/// A minimal SMTP server: records the envelope and message.
async fn smtp(listener: TcpListener, got: Arc<Mutex<Vec<String>>>) {
    let Ok((sock, _)) = listener.accept().await else { return };
    let (r, mut w) = sock.into_split();
    let mut lines = BufReader::new(r).lines();
    w.write_all(b"220 mock ESMTP\r\n").await.unwrap();
    let mut in_data = false;
    while let Ok(Some(line)) = lines.next_line().await {
        got.lock().unwrap().push(line.clone());
        if in_data {
            if line == "." {
                in_data = false;
                w.write_all(b"250 queued\r\n").await.unwrap();
            }
            continue;
        }
        let cmd = line.to_ascii_uppercase();
        let reply: &[u8] = if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
            b"250-mock\r\n250 8BITMIME\r\n"
        } else if cmd.starts_with("DATA") {
            in_data = true;
            b"354 go\r\n"
        } else if cmd.starts_with("QUIT") {
            w.write_all(b"221 bye\r\n").await.unwrap();
            break;
        } else {
            b"250 ok\r\n"
        };
        w.write_all(reply).await.unwrap();
    }
}

fn summary() -> CrawlSummary {
    CrawlSummary {
        name: "Weekly".into(),
        start_url: "https://www.example.com/".into(),
        urls: 1234,
        issues: vec![IssueRow {
            tab: "Response Codes".into(),
            filter: "Internal Client Error (4XX)".into(),
            issue_type: IssueType::Issue,
            priority: Priority::High,
            urls: 7,
            pct_of_total: 0.6,
            description: String::new(),
        }],
        changes: vec![("Response Codes: Internal Client Error (4XX)".into(), 7, 3)],
        files: vec!["C:/exports/internal_all.csv".into()],
    }
}

#[test]
fn composes_summary() {
    let (subject, text, html) = compose(&summary());
    assert_eq!(subject, "Screaming Toad: crawl of www.example.com complete (1234 URLs)");
    assert!(text.contains("Issues: 1"));
    assert!(text.contains("Response Codes: Internal Client Error (4XX): 7 (was 3, +4)"), "{text}");
    assert!(html.contains("<td>+4</td>"));
    assert!(html.contains("internal_all.csv"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_through_smtp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let got = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(smtp(listener, got.clone()));
    let settings = NotificationSettings {
        smtp_host: "127.0.0.1".into(),
        smtp_port: port,
        security: SmtpSecurity::None,
        from: "toad@example.com".into(),
        default_recipients: vec!["seo@example.com".into()],
        ..Default::default()
    };
    let (subject, text, html) = compose(&summary());
    send(&settings, None, &[], &subject, &text, &html).await.unwrap();
    let lines = got.lock().unwrap().join("\n");
    assert!(lines.contains("MAIL FROM:<toad@example.com>"), "{lines}");
    assert!(lines.contains("RCPT TO:<seo@example.com>"));
    assert!(lines.contains("Subject: Screaming Toad: crawl of www.example.com complete (1234 URLs)"));
    assert!(lines.contains("multipart/alternative"));
}

#[tokio::test]
async fn needs_settings_and_recipients() {
    let none = NotificationSettings::default();
    assert!(send(&none, None, &[], "s", "t", "h").await.unwrap_err().to_string().contains("set up email"));
    let no_to = NotificationSettings { smtp_host: "localhost".into(), ..Default::default() };
    assert!(send(&no_to, None, &[], "s", "t", "h").await.unwrap_err().to_string().contains("no email address"));
}

/// Writes to the real OS credential store, so it only runs when asked
/// (`cargo test -p toad-apis -- --ignored`).
#[test]
#[ignore]
fn keystore_round_trip() {
    let key = format!("toad_test_{}", std::process::id());
    toad_apis::keystore::set(&key, "s3cret").unwrap();
    assert_eq!(toad_apis::keystore::get(&key).as_deref(), Some("s3cret"));
    toad_apis::keystore::set(&key, "").unwrap();
    assert_eq!(toad_apis::keystore::get(&key), None);
}
