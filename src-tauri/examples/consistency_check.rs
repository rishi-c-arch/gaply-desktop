//! Run the deterministic consistency checks against a manuscript (§11 D94).
//!
//! NO model, NO audit, NO job, NO network — it parses the file, runs the
//! pre-pass and prints what the pre-flight gate would say. Seconds, not hours.
//!
//! ```text
//! cargo run --release --example consistency_check -- "/path/to/manuscript.docx"
//! ```
fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: consistency_check <manuscript.pdf|.docx|.txt|.md>");
        std::process::exit(2);
    };
    let p = std::path::Path::new(&path);
    let blocks = match gaply_core::extract::docparse::parse_path_paged(p) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not read {path}: {e}");
            std::process::exit(1);
        }
    };
    let report = gaply_core::ai_engine::audit_prepass::prepass_blocks(&blocks);
    let c = gaply_core::consistency::check_consistency(&blocks, &report);

    println!("{path}");
    println!(
        "  {} blocks · {} sentences planned · {} reference entries",
        blocks.len(),
        report.planned.len(),
        report.bibliography.len()
    );
    println!(
        "  {} structural, {} cosmetic{}\n",
        c.structural,
        c.cosmetic,
        if c.blocks_audit() { "  — an audit would ask you to acknowledge these" } else { "" }
    );
    if c.findings.is_empty() {
        println!("  Nothing found.");
        return;
    }
    for f in &c.findings {
        let tag = match f.severity {
            gaply_core::consistency::Severity::Structural => "STRUCTURAL",
            gaply_core::consistency::Severity::Cosmetic => "cosmetic  ",
        };
        println!("  [{tag}] {}", f.kind);
        println!("      {}", f.message);
        if let Some(a) = &f.action {
            println!("      -> {a}");
        }
        println!();
    }
}
