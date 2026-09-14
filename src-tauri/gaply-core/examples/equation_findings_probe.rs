//! **The deliverable: Tier-0 findings on real manuscripts, no model involved.**
//!
//! Drives the whole path from the user's entry point — the file on disk —
//! through `docparse`, the linear parser and the Tier-0 checker. Nothing here
//! loads a model, opens a socket or touches the database; the purity of the
//! engine itself is enforced by `tests/equation_is_llm_free.rs`.
use gaply_core::epistemic::EpistemicStatus;
use gaply_core::equation::check::{check_equation, ArithmeticFinding, BoundValues};
use gaply_core::equation::linear::parse_equation;
use gaply_core::extract::docparse;
use std::path::Path;

fn main() {
    let mut reportable = 0usize;
    let mut checked = 0usize;
    let mut confirmed = 0usize;

    for a in std::env::args().skip(1) {
        let p = Path::new(&a);
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or(&a);
        let bytes = std::fs::read(p).expect("read");
        let text = if p.extension().map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false) {
            docparse::parse_docx(&bytes)
        } else {
            docparse::parse_pdf_bytes(&bytes)
        }
        .expect("parse");

        println!("########## {name}");
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || !t.contains('=') {
                continue;
            }
            let Ok(eq) = parse_equation(t) else { continue };
            for f in check_equation(&eq, &BoundValues::new()) {
                checked += 1;
                if f.status == EpistemicStatus::Confirmed {
                    confirmed += 1;
                }
                if !f.is_reportable() {
                    continue;
                }
                reportable += 1;
                print(&f);
            }
        }
    }
    println!(
        "\nclaims checked={checked}  confirmed silently={confirmed}  REPORTABLE FINDINGS={reportable}"
    );
}

fn print(f: &ArithmeticFinding) {
    println!("\n  ── {} ──", f.status.label().to_uppercase());
    println!("  line   : {}", f.source_line);
    println!("  claim  : #{} of the chain", f.claim_index);
    println!("  left   : {}   = {}", f.left.text, f.left.value.as_deref().unwrap_or("—"));
    println!("  right  : {}   = {}", f.right.text, f.right.value.as_deref().unwrap_or("—"));
    for r in &f.readings {
        println!(
            "  {:<38} {}   left {}  right {}",
            r.reading.label(),
            if r.holds { "HOLDS " } else { "FAILS " },
            r.left,
            r.right
        );
    }
    println!("  finding: {}", f.message);
    println!("  trail  :");
    for t in &f.trail {
        println!("           - {t}");
    }
}
