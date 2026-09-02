//! HTML renderer for the shared [`Block`] vocabulary.
//!
//! The second renderer `report_compose`'s docs predicted would be cheap, and it
//! is: turning a line of text at a level into a tag is the whole job. It makes
//! no content decisions — ordering, grouping and every user-facing string
//! belong to the composer, and a renderer that decided any of them would be the
//! layering violation those docs exist to prevent.
//!
//! # Escaping is not optional here
//!
//! The blocks carry manuscript sentences and passages from cited PDFs — text
//! this app never wrote and cannot vouch for. A quote containing `<script>` is
//! not hypothetical in a corpus of arbitrary papers, and an exported report is
//! a file people open in a browser. Every interpolated value goes through
//! [`escape`], and `escaping_is_applied_to_every_text_bearing_block` walks the
//! whole vocabulary so a variant added later cannot quietly skip it.

use crate::report_compose::Block;

/// HTML-escape a string. `&` first, or the other replacements get double-escaped.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Print styles live in the document so the file is self-contained: a report
/// mailed to a supervisor must not depend on a stylesheet it cannot reach.
const STYLE: &str = "\
body{font:14px/1.6 -apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;\
max-width:46rem;margin:2rem auto;padding:0 1rem;color:#1a1a1a}\
.cover{border-bottom:2px solid #1a1a1a;padding-bottom:1rem;margin-bottom:2rem}\
.cover h1{margin:0 0 .25rem;font-size:1.75rem}\
.cover .sub{color:#555;font-size:1.05rem;margin-bottom:.75rem}\
.cover dl{display:grid;grid-template-columns:auto 1fr;gap:.15rem .75rem;margin:0;font-size:.9rem}\
.cover dt{color:#555}.cover dd{margin:0}\
h2{margin-top:2rem;border-bottom:1px solid #ddd;padding-bottom:.25rem}\
h3{margin-top:1.5rem;font-size:1.05rem}\
h4{margin-top:1.25rem;font-size:.95rem;color:#333}\
.note{background:#f6f6f4;border-left:3px solid #999;padding:.6rem .8rem;margin:1rem 0;\
color:#444;font-size:.92rem}\
ul{margin:.4rem 0;padding-left:1.25rem}li{margin:.2rem 0}\
li.nested{margin-left:1rem}\
.pagebreak{page-break-after:always;height:0}\
@media print{body{margin:0;max-width:none}}";

/// Render blocks to a standalone HTML document.
pub fn render_html(blocks: &[Block], document_title: &str) -> String {
    let mut s = String::with_capacity(4096);
    s.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    s.push_str(&format!("<title>{}</title>\n", escape(document_title)));
    s.push_str(&format!("<style>{STYLE}</style>\n</head>\n<body>\n"));

    // Bullets are consecutive runs in a flat block list; group them into one
    // <ul> so the output is a list rather than a series of one-item lists.
    let mut in_list = false;
    let close_list = |s: &mut String, in_list: &mut bool| {
        if *in_list {
            s.push_str("</ul>\n");
            *in_list = false;
        }
    };

    for b in blocks {
        match b {
            Block::Bullet { text, indent } => {
                if !in_list {
                    s.push_str("<ul>\n");
                    in_list = true;
                }
                let class = if *indent > 0 { " class=\"nested\"" } else { "" };
                s.push_str(&format!("<li{class}>{}</li>\n", escape(text)));
            }
            other => {
                close_list(&mut s, &mut in_list);
                match other {
                    Block::Cover { title, subtitle, meta } => {
                        s.push_str("<header class=\"cover\">\n");
                        s.push_str(&format!("<h1>{}</h1>\n", escape(title)));
                        s.push_str(&format!("<div class=\"sub\">{}</div>\n", escape(subtitle)));
                        s.push_str("<dl>\n");
                        for (k, v) in meta {
                            s.push_str(&format!(
                                "<dt>{}</dt><dd>{}</dd>\n",
                                escape(k),
                                escape(v)
                            ));
                        }
                        s.push_str("</dl>\n</header>\n");
                    }
                    Block::Heading { text, level } => {
                        // level 1 → h2: the cover's <h1> is the document title,
                        // and two competing h1s is a document with no outline.
                        let tag = match level {
                            1 => "h2",
                            2 => "h3",
                            _ => "h4",
                        };
                        s.push_str(&format!("<{tag}>{}</{tag}>\n", escape(text)));
                    }
                    Block::Paragraph { text } => {
                        s.push_str(&format!("<p>{}</p>\n", escape(text)));
                    }
                    Block::Note { text } => {
                        s.push_str(&format!("<div class=\"note\">{}</div>\n", escape(text)));
                    }
                    Block::PageBreak => s.push_str("<div class=\"pagebreak\"></div>\n"),
                    Block::Bullet { .. } => unreachable!("handled above"),
                }
            }
        }
    }
    close_list(&mut s, &mut in_list);
    s.push_str("</body>\n</html>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_is_applied_to_every_text_bearing_block() {
        // The blocks carry manuscript sentences and passages from arbitrary
        // PDFs. An exported report is opened in a browser, so unescaped `<` is
        // a real hazard rather than a theoretical one. Every variant is walked
        // here so one added later cannot quietly skip it.
        let hostile = "<script>alert('x')</script> & \"quoted\"";
        let blocks = vec![
            Block::Cover {
                title: hostile.into(),
                subtitle: hostile.into(),
                meta: vec![(hostile.into(), hostile.into())],
            },
            Block::Heading { text: hostile.into(), level: 1 },
            Block::Paragraph { text: hostile.into() },
            Block::Bullet { text: hostile.into(), indent: 0 },
            Block::Note { text: hostile.into() },
            Block::PageBreak,
        ];
        let html = render_html(&blocks, hostile);
        assert!(!html.contains("<script>"), "unescaped script tag:\n{html}");
        assert!(!html.contains("alert('x')"), "unescaped quote:\n{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&amp;"));
        // `&` must not be double-escaped.
        assert!(!html.contains("&amp;amp;"), "double-escaped:\n{html}");
    }

    #[test]
    fn consecutive_bullets_become_one_list() {
        let blocks = vec![
            Block::Bullet { text: "one".into(), indent: 0 },
            Block::Bullet { text: "two".into(), indent: 0 },
            Block::Paragraph { text: "after".into() },
            Block::Bullet { text: "three".into(), indent: 0 },
        ];
        let html = render_html(&blocks, "t");
        assert_eq!(html.matches("<ul>").count(), 2, "{html}");
        assert_eq!(html.matches("</ul>").count(), 2, "{html}");
        // The list closes before the paragraph, not after it.
        let ul_close = html.find("</ul>").unwrap();
        let p = html.find("<p>after</p>").unwrap();
        assert!(ul_close < p, "{html}");
    }

    #[test]
    fn the_document_is_self_contained_and_well_formed() {
        // A report mailed to a supervisor must not depend on a stylesheet it
        // cannot reach.
        let html = render_html(&[Block::Paragraph { text: "x".into() }], "Report");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<style>"));
        assert!(!html.contains("<link"), "external stylesheet: {html}");
        assert!(html.trim_end().ends_with("</html>"));
        assert!(html.contains("<title>Report</title>"));
    }

    #[test]
    fn heading_levels_map_below_the_covers_h1() {
        // Two competing <h1>s is a document with no outline.
        let blocks = vec![
            Block::Cover { title: "T".into(), subtitle: "S".into(), meta: vec![] },
            Block::Heading { text: "Section".into(), level: 1 },
            Block::Heading { text: "Finding".into(), level: 3 },
        ];
        let html = render_html(&blocks, "t");
        assert_eq!(html.matches("<h1>").count(), 1, "{html}");
        assert!(html.contains("<h2>Section</h2>"), "{html}");
        assert!(html.contains("<h4>Finding</h4>"), "{html}");
    }

    #[test]
    fn the_full_audit_report_renders_end_to_end() {
        // The composer and this renderer meeting for real, not on fixtures.
        use crate::audit_report::{compose_audit, AuditReportModel, ReportEvidence, ReportItem};
        let m = AuditReportModel {
            manuscript_name: "R PAPER .pdf".into(),
            generated_on: "3 September 2026".into(),
            model_id: "qwen2.5-3b".into(),
            prompt_version: "v1.4".into(),
            total_sentences: 114,
            checked: 84,
            supported: vec![ReportItem {
                seq: 1,
                page: Some(2),
                sentence: "A claim.".into(),
                verdict: Some("strong".into()),
                explanation: Some("Because the passage says so.".into()),
                evidence: vec![ReportEvidence {
                    chunk_id: "c1".into(),
                    page: Some(4),
                    quote: "The passage.".into(),
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let html = render_html(&compose_audit(&m), "Thesis citation audit");
        assert!(html.contains("Thesis citation audit"));
        assert!(html.contains("R PAPER .pdf"));
        assert!(html.contains("The passage."));
        // D18 survives the render: the quote is in the document before the prose.
        let quote = html.find("The passage.").unwrap();
        let prose = html.find("Because the passage says so.").unwrap();
        assert!(quote < prose, "prose preceded its evidence in the HTML");
    }
}
