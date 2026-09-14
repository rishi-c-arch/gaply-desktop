//! What units does the corpus actually carry, and in what forms?
//!
//! `Side::unit` is populated by the parser and so far unused. Before deciding
//! what a dimensional checker can compare, measure what it would have to
//! represent — a unit system that cannot express "as CaCO₃" will either refuse
//! everything real or silently drop the qualifier.
use gaply_core::equation::linear::parse_equation;
use gaply_core::extract::{docparse, omml};
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let mut forms: BTreeMap<String, usize> = BTreeMap::new();
    let mut inline: BTreeMap<String, usize> = BTreeMap::new();
    for a in std::env::args().skip(1) {
        let p = Path::new(&a);
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or(&a);
        let bytes = std::fs::read(p).expect("read");
        let is_docx = p.extension().map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false);
        let text = if is_docx {
            docparse::parse_docx(&bytes)
        } else {
            docparse::parse_pdf_bytes(&bytes)
        }
        .expect("parse");
        let mut eqs = if is_docx {
            omml::equations_in_docx(&bytes)
                .map(|(e, _)| e)
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.structural)
                .collect::<Vec<_>>()
                .into_iter()
        } else {
            Vec::new().into_iter()
        };
        println!("########## {name}");
        for line in text.lines() {
            let t = line.trim();
            let t = if t == docparse::EQUATION_PLACEHOLDER {
                match eqs.next() {
                    Some(e) => e.linear,
                    None => continue,
                }
            } else {
                t.to_string()
            };
            let Ok(eq) = parse_equation(&t) else { continue };
            for s in &eq.sides {
                if let Some(u) = &s.unit {
                    *forms.entry(u.clone()).or_default() += 1;
                    println!("  {:<34} <- {}", format!("[{u}]"), s.text);
                }
            }
            // Units written INSIDE a declaration's prose, which the `Side::unit`
            // rule never sees: `Vt = total volume of the reaction (mL)`.
            for cap in t.split(';') {
                if let Some(o) = cap.rfind('(') {
                    let inner = &cap[o + 1..];
                    if let Some(c) = inner.find(')') {
                        let u = inner[..c].trim();
                        if !u.is_empty()
                            && u.len() <= 18
                            && u.chars().any(|c| c.is_alphabetic())
                            && cap.contains('=')
                        {
                            *inline.entry(u.to_string()).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    println!("\n=== unit annotations on an equation side ===");
    for (u, n) in &forms {
        println!("  {n:>3}  {u}");
    }
    println!("\n=== parenthesised units inside declaration prose ===");
    for (u, n) in &inline {
        println!("  {n:>3}  {u}");
    }
}
