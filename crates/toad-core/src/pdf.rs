//! Spider > Extraction > PDF: text, document properties and links.
//!
//! Parsing uses `pdf-extract` (which re-exports `lopdf`). Malformed PDFs can
//! make PDF libraries panic, so parsing runs inside `catch_unwind`: a bad PDF
//! becomes an error on its URL, never a crashed crawl.

use crate::model::{Link, LinkKind};
use pdf_extract::{Document, Object};
use serde::{Deserialize, Serialize};
use url::Url;

/// PDF document properties (the Info dictionary and page count).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PdfInfo {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
    pub keywords: Option<String>,
    /// `YYYY-MM-DD HH:MM:SS` when the PDF date parses, otherwise as written.
    pub created: Option<String>,
    pub modified: Option<String>,
    pub pages: u32,
}

pub struct ParsedPdf {
    pub info: PdfInfo,
    pub text: String,
    /// URI link annotations, with "PDF page N" as the link position.
    pub links: Vec<Link>,
}

pub fn parse(bytes: &[u8], url: &Url) -> anyhow::Result<ParsedPdf> {
    let bytes = bytes.to_vec();
    let url = url.clone();
    std::panic::catch_unwind(move || parse_inner(&bytes, &url)).unwrap_or_else(|_| Err(anyhow::anyhow!("the PDF parser failed on this file")))
}

fn parse_inner(bytes: &[u8], base: &Url) -> anyhow::Result<ParsedPdf> {
    let doc = Document::load_mem(bytes)?;
    let mut info = PdfInfo { pages: doc.get_pages().len() as u32, ..Default::default() };
    if let Some(dict) = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| match o {
            Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        })
        .and_then(|o| o.as_dict().ok())
    {
        let get = |key: &[u8]| dict.get(key).ok().and_then(text_of).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        info.title = get(b"Title");
        info.subject = get(b"Subject");
        info.author = get(b"Author");
        info.keywords = get(b"Keywords");
        info.created = get(b"CreationDate").map(|d| pdf_date(&d));
        info.modified = get(b"ModDate").map(|d| pdf_date(&d));
    }

    let mut links = Vec::new();
    for (n, (_, page_id)) in doc.get_pages().into_iter().enumerate() {
        let Ok(annots) = doc.get_page_annotations(page_id) else { continue };
        for a in annots {
            let is_link = a.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) == Some(b"Link".as_slice());
            let uri = a
                .get(b"A")
                .ok()
                .and_then(|o| match o {
                    Object::Reference(id) => doc.get_object(*id).ok(),
                    other => Some(other),
                })
                .and_then(|o| o.as_dict().ok())
                .and_then(|d| d.get(b"URI").ok())
                .and_then(text_of);
            if let (true, Some(uri)) = (is_link, uri) {
                if let Some(to) = base.join(uri.trim()).ok().filter(|u| matches!(u.scheme(), "http" | "https")) {
                    links.push(Link {
                        kind: LinkKind::Hyperlink,
                        to: to.to_string(),
                        href: uri.trim().to_string(),
                        anchor: String::new(),
                        alt: None,
                        follow: true,
                        rel: String::new(),
                        target: String::new(),
                        path_type: "Absolute".into(),
                        link_path: String::new(),
                        position: format!("PDF page {}", n + 1),
                        hreflang: None,
                        origin: String::new(),
                    });
                }
            }
        }
    }

    let text = pdf_extract::extract_text_from_mem(bytes).unwrap_or_default();
    Ok(ParsedPdf { info, text, links })
}

/// A PDF text string: UTF-16BE with a BOM, otherwise PDFDocEncoding (read as Latin-1).
fn text_of(o: &Object) -> Option<String> {
    let bytes = match o {
        Object::String(b, _) => b,
        _ => return None,
    };
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Some(s.to_string());
    }
    Some(bytes.iter().map(|&b| b as char).collect())
}

/// `D:20240131093000+01'00'` → `2024-01-31 09:30:00`.
fn pdf_date(raw: &str) -> String {
    let d = raw.trim().trim_start_matches("D:");
    let digits: String = d.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 {
        return raw.trim().to_string();
    }
    let part = |i: usize, j: usize| digits.get(i..j).unwrap_or("00");
    format!("{}-{}-{} {}:{}:{}", part(0, 4), part(4, 6), part(6, 8), part(8, 10), part(10, 12), part(12, 14))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal one-page PDF with Info, text and a URI link annotation.
    fn sample() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Annots [6 0 R] >>",
            "STREAM",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            "<< /Type /Annot /Subtype /Link /Rect [0 0 100 20] /A << /S /URI /URI (https://example.com/linked) >> >>",
            "<< /Title (Toad Guide) /Author (Ann Frog) /Subject (Amphibians) /Keywords (toad, pond) /CreationDate (D:20240131093000Z) >>",
        ];
        let content = "BT /F1 12 Tf 20 100 Td (Toads live in ponds.) Tj ET";
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            let body = if *o == "STREAM" { format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()) } else { o.to_string() };
            pdf.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for o in offsets {
            pdf.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        pdf.extend(format!("trailer\n<< /Size {} /Root 1 0 R /Info 7 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
        pdf
    }

    #[test]
    fn reads_properties_text_and_links() {
        let doc = parse(&sample(), &Url::parse("https://example.com/guide.pdf").unwrap()).unwrap();
        assert_eq!(doc.info.title.as_deref(), Some("Toad Guide"));
        assert_eq!(doc.info.author.as_deref(), Some("Ann Frog"));
        assert_eq!(doc.info.subject.as_deref(), Some("Amphibians"));
        assert_eq!(doc.info.keywords.as_deref(), Some("toad, pond"));
        assert_eq!(doc.info.created.as_deref(), Some("2024-01-31 09:30:00"));
        assert_eq!(doc.info.pages, 1);
        assert!(doc.text.contains("Toads live in ponds"), "{:?}", doc.text);
        assert_eq!(doc.links.len(), 1);
        assert_eq!((doc.links[0].to.as_str(), doc.links[0].position.as_str()), ("https://example.com/linked", "PDF page 1"));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse(b"%PDF-1.4 this is not really a pdf", &Url::parse("https://x.com/a.pdf").unwrap()).is_err());
    }
}
