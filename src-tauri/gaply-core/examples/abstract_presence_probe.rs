//! **Does the section splitter find an abstract, and when it does not, what form
//! did the manuscript write?**
//!
//! `heading_vocab_probe` cannot answer this. It applies `detect_heading`'s own
//! shape gate — `split_whitespace().count() > 5` — before looking, so a RUN-IN
//! heading (`"Abstract- Emotion detection in social media text…"`, the heading
//! glued to the body on one line) is invisible to it. That probe measures the
//! VOCABULARY gap; this one measures the SHAPE gap, and they are different
//! defects with different fixes.
//!
//! Drives the real `split_document` and the real `detect_heading_for_probe`. A
//! probe that reimplemented either would agree with the classifier by
//! construction.

use std::path::Path;

use gaply_core::extract::{docparse, sections, SectionKind};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: abstract_presence_probe <file>...");
        std::process::exit(2);
    }

    let mut parsed = 0usize;
    let mut found = 0usize;
    let mut missed_with_marker = 0usize;
    let mut missed_no_marker = 0usize;
    let mut rows: Vec<String> = Vec::new();

    for p in &paths {
        let Ok(text) = docparse::parse_path(Path::new(p)) else {
            rows.push(format!("PARSE-FAIL          {}", base(p)));
            continue;
        };
        parsed += 1;
        let (_title, secs) = sections::split_document(&text);
        let has_abstract = secs.iter().any(|s| s.kind == SectionKind::Abstract);

        // **Position is what separates a heading from prose.** The first pass
        // took the first line mentioning "abstract" ANYWHERE, and on two files
        // that was body text using the word in its ordinary sense ("the
        // abstract theoretical propositions"), reported as a missed heading.
        // An abstract heading sits at the top; a mention at line 700 of 900 is
        // a word. Every candidate in the head of the document is printed, so
        // the judgement is visible rather than buried in a threshold.
        let lines: Vec<&str> = text.lines().collect();
        let head = (lines.len() / 5).max(40).min(lines.len());
        let marker = lines[..head].iter().position(|l| mentions_abstract(l));
        let marker = marker.map(|i| (i, lines[i].trim()));

        if has_abstract {
            found += 1;
            let h = secs
                .iter()
                .find(|s| s.kind == SectionKind::Abstract)
                .map(|s| s.heading.clone())
                .unwrap_or_default();
            rows.push(format!("FOUND    heading={:<14} {}", trunc(&h, 14), base(p)));
        } else if let Some((idx, m)) = marker {
            missed_with_marker += 1;
            let words = m.split_whitespace().count();
            // WHY the detector said no, in its own terms.
            let why = if words > 5 {
                format!("run-in: {words} words on the heading line")
            } else if sections::detect_heading_for_probe(m).is_none() {
                "short line, phrase not in the lexicon".to_string()
            } else {
                "detector accepts it — the miss is elsewhere".to_string()
            };
            rows.push(format!(
                "MISSED   {:<44} {}  (line {idx} of {}, head={head})",
                why,
                base(p),
                lines.len()
            ));
            rows.push(format!("         line: {:?}", trunc(m, 1400)));
        } else {
            missed_no_marker += 1;
            rows.push(format!(
                "NO-ABSTRACT  no 'abstract' in the first {head} of {} lines     {}",
                lines.len(),
                base(p)
            ));
        }
    }

    for r in &rows {
        println!("{r}");
    }
    println!("\n--- totals over {parsed} parsed file(s) ---");
    println!("  abstract FOUND by the splitter          {found}");
    println!("  MISSED, word present near the top        {missed_with_marker}");
    println!("  no abstract marker at all               {missed_no_marker}");
    assert!(parsed > 0, "nothing parsed — the probe, not the corpus");
}

/// "abstract" as a word, not inside another token. Deliberately permissive: this
/// is a survey, and a gate tuned to the answer would only confirm itself.
fn mentions_abstract(line: &str) -> bool {
    let l = line.to_lowercase();
    let Some(i) = l.find("abstract") else { return false };
    let before_ok = i == 0 || !l.as_bytes()[i - 1].is_ascii_alphanumeric();
    let j = i + "abstract".len();
    let after_ok = j >= l.len() || !l.as_bytes()[j].is_ascii_alphanumeric();
    before_ok && after_ok
}

fn base(p: &str) -> String {
    Path::new(p).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.into())
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n).collect() }
}
