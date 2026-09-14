//! Does the linear parser read the equations these manuscripts actually
//! contain — as `docparse` delivers them, not as transcribed into a test?
//!
//! A hand-written fixture inherits the author's premise and can only confirm
//! it. This drives the parser from the user's entry point: the real file.
use gaply_core::equation::expr::Bindings;
use gaply_core::equation::linear::{parse_equation, ParseError};
use gaply_core::extract::docparse;
use std::path::Path;

fn main() {
    let (mut ok, mut refused, mut failed) = (0usize, 0usize, 0usize);
    let (mut prose_lines, mut prose_parsed) = (0usize, 0usize);
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
            if t.is_empty() || t.len() > 300 || !t.contains('=') {
                continue;
            }
            let digits = t.chars().filter(char::is_ascii_digit).count();
            let ops = t.matches(['/', '×', '*', '^', '−', '+']).count();
            let equation_shaped = digits >= 2 && ops >= 1;
            if !equation_shaped {
                // NEGATIVE CONTROL: every other line with an `=` in it is prose
                // as far as this engine is concerned. Longest-prefix parsing
                // makes MORE things parseable, so the question that matters is
                // how often it manufactures an equation out of a sentence.
                prose_lines += 1;
                if let Ok(eq) = parse_equation(t) {
                    prose_parsed += 1;
                    println!("  PROSE-PARSED  {t}");
                    println!("                -> {:?}", eq.sides.iter().map(|s| &s.text).collect::<Vec<_>>());
                }
                continue;
            }
            match parse_equation(t) {
                Ok(eq) => {
                    ok += 1;
                    let vals: Vec<String> = eq
                        .sides
                        .iter()
                        .map(|s| match s.expr.eval(&Bindings::new()) {
                            Ok(v) => v.to_string(),
                            Err(_) => "·".into(),
                        })
                        .collect();
                    println!("  OK    {t}");
                    println!("        sides={} unit={:?} values=[{}]",
                        eq.sides.len(), eq.sides[0].unit, vals.join(" | "));
                }
                Err(ParseError::NotAnEquation) | Err(ParseError::TooLong) => refused += 1,
                Err(e) => {
                    failed += 1;
                    println!("  PARSE-FAIL ({e})  {t}");
                }
            }
        }
    }
    println!(
        "\nequation-shaped: parsed={ok}  refused={refused}  parse-failed={failed}\n\
         prose lines with an `=`: {prose_lines}, of which parsed as equations: {prose_parsed}"
    );
    if ok == 0 {
        println!("NO LINE PARSED — the instrument is broken or the corpus has no equations.");
        std::process::exit(1);
    }
}
