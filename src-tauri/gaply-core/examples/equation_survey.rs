//! **The gating measurement for §6b: does any real manuscript carry
//! machine-readable mathematics, and what survives `docparse` today?**
//!
//! §6b.1 names equation extraction as the gating item for the whole
//! mathematical-verification subsystem, on the stated premise that
//! `docparse.rs` "extracts `<w:t>` text only — no `m:oMath` handling. Math in
//! an uploaded `.docx` is flattened or dropped". This probe measures that
//! claim rather than reading it.
//!
//! Run:
//! ```text
//! cargo run -p gaply_core --release --example equation_survey -- <file>...
//! ```
//!
//! # The known-good case
//!
//! A batch that reports "no equations anywhere" is indistinguishable from a
//! batch whose reader is broken (CLAUDE.md, the negative-control rule), so the
//! corpus MUST include a file known to carry `m:oMath`. If the row for that
//! file reports zero, the instrument is broken and no other row means anything.
//! The probe refuses to print a summary without at least one positive row.

use std::path::Path;

use gaply_core::extract::docparse;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: equation_survey <manuscript>...");
        std::process::exit(2);
    }

    let mut positive_rows = 0usize;
    for a in &args {
        let p = Path::new(a);
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or(a);
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                // A missing input is a LOUD failure, never an empty result.
                println!("{name:<50} READ FAILED: {e}");
                continue;
            }
        };

        let is_docx = p.extension().map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false);

        // What the raw container carries.
        let carriers = if is_docx {
            match raw_docx_math(&bytes) {
                Ok(c) => c,
                Err(e) => {
                    println!("{name:<50} ZIP/XML FAILED: {e}");
                    continue;
                }
            }
        } else {
            RawMath::default()
        };

        // What docparse yields today.
        let parsed = if is_docx {
            docparse::parse_docx(&bytes)
        } else {
            docparse::parse_pdf_bytes(&bytes)
        };
        let (chars, math_glyphs, sample) = match &parsed {
            Ok(t) => (t.chars().count(), count_math_glyphs(t), first_math_line(t)),
            Err(e) => {
                println!("{name:<50} PARSE FAILED: {e}");
                continue;
            }
        };

        if carriers.omath > 0 {
            positive_rows += 1;
        }
        println!(
            "{name:<50} kind={:<4} m:oMath={:<4} m:t={:<5} w:object={:<3} \
             parsed_chars={:<7} math_glyphs={:<4}",
            if is_docx { "docx" } else { "pdf" },
            carriers.omath,
            carriers.mt,
            carriers.object,
            chars,
            math_glyphs,
        );
        if let Some(s) = sample {
            println!("{:>52}first math-ish line: {s}", "");
        }
    }

    println!();
    if positive_rows == 0 {
        println!(
            "NO POSITIVE ROW. Either no input carries OMML, or the reader is broken — \
             these are indistinguishable from this output. Add a known-OMML file."
        );
        std::process::exit(1);
    }
    println!("{positive_rows} input(s) carried m:oMath — the reader is demonstrably live.");
}

#[derive(Default)]
struct RawMath {
    omath: usize,
    mt: usize,
    object: usize,
}

/// Count math carriers in the raw `word/document.xml`, independent of
/// `docparse`, so the probe is not measuring the thing it is checking.
fn raw_docx_math(bytes: &[u8]) -> Result<RawMath, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let mut xml = String::new();
    {
        use std::io::Read;
        zip.by_name("word/document.xml")
            .map_err(|e| e.to_string())?
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
    }
    Ok(RawMath {
        omath: xml.matches("<m:oMath>").count() + xml.matches("<m:oMath ").count(),
        mt: xml.matches("<m:t>").count() + xml.matches("<m:t ").count(),
        object: xml.matches("<w:object").count(),
    })
}

/// Characters that only appear in mathematics, so "does the text layer carry
/// math at all" is answerable without a parser.
fn count_math_glyphs(t: &str) -> usize {
    t.chars()
        .filter(|c| {
            matches!(c,
                '√' | '∑' | '∫' | '≤' | '≥' | '≠' | '±' | '×' | '÷' | '∞' | '∂' | '∈'
                | 'α' | 'β' | 'γ' | 'δ' | 'ε' | 'θ' | 'λ' | 'μ' | 'σ' | 'τ' | 'φ' | 'χ' | 'ω'
                | '²' | '³' | '¹')
        })
        .count()
}

fn first_math_line(t: &str) -> Option<String> {
    t.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && count_math_glyphs(l) > 0 && l.contains('='))
        .map(|l| l.chars().take(110).collect())
}
