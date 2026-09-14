//! Does the local-name collapse reach the OUTPUT, or is it only in the markup?
//! Counts characters `parse_docx` emits that no run in the document produced.
use gaply_core::extract::docparse;
fn main() {
    for a in std::env::args().skip(1) {
        let bytes = std::fs::read(&a).expect("read");
        let t = docparse::parse_docx(&bytes).expect("parse");
        let name = std::path::Path::new(&a).file_name().unwrap().to_string_lossy();
        let tabs = t.matches('\t').count();
        let tab_lines: Vec<&str> =
            t.lines().filter(|l| l.contains('\t')).take(3).collect();
        println!("{name:<48} tab_chars={tabs}");
        for l in tab_lines {
            println!("    {:?}", l.chars().take(90).collect::<String>());
        }
    }
}
