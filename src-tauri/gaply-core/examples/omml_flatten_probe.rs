//! What `docparse::parse_docx` actually does to an `m:oMath` element.
//!
//! §6b.1 asserts math is "flattened or dropped". Those are different failures
//! with different consequences, and the difference is measurable. Run against a
//! `.docx` known to carry OMML and read the window around the equation.
use gaply_core::extract::docparse;

fn main() {
    let path = std::env::args().nth(1).expect("usage: omml_flatten_probe <docx>");
    let needle = std::env::args().nth(2).unwrap_or_else(|| "Slovin".into());
    let bytes = std::fs::read(&path).expect("read");
    let text = docparse::parse_docx(&bytes).expect("parse");
    let i = text.find(&needle).unwrap_or_else(|| panic!("needle {needle:?} not in parsed text"));
    let lo = i.saturating_sub(400);
    let hi = (i + 900).min(text.len());
    let win = &text[lo..hi];
    println!("--- parse_docx window around {needle:?} ---");
    for line in win.lines() {
        let t = line.trim();
        if !t.is_empty() {
            println!("| {t}");
        }
    }
}
