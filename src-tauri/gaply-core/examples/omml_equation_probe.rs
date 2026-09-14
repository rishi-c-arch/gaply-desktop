//! Read equations out of a REAL .docx and check them. The OMML fixtures in
//! `extract/omml.rs` are reconstructions of Word's markup; this is Word's.
use gaply_core::epistemic::EpistemicStatus;
use gaply_core::equation::check::check_equation;
use gaply_core::equation::expr::Bindings;
use gaply_core::equation::linear::parse_equation;
use gaply_core::extract::omml;

fn main() {
    let mut total = 0usize;
    let mut reportable = 0usize;
    for a in std::env::args().skip(1) {
        let bytes = std::fs::read(&a).expect("read");
        let name = std::path::Path::new(&a).file_name().unwrap().to_string_lossy();
        let (eqs, errs) = omml::equations_in_docx(&bytes).expect("docx");
        println!("########## {name}  equations={} unsupported={}", eqs.len(), errs.len());
        for e in &eqs {
            total += 1;
            println!("  ¶{:<5} {}", e.paragraph, e.linear);
            match parse_equation(&e.linear) {
                Ok(eq) => {
                    for c in check_equation(&eq, &Bindings::new()) {
                        let mark = if c.is_reportable() { "  >>> " } else { "      " };
                        if c.is_reportable() {
                            reportable += 1;
                        }
                        if c.status != EpistemicStatus::Unverified || c.is_reportable() {
                            println!("{mark}#{} {:<28} {}", c.claim_index, c.status.label(), c.message);
                        }
                    }
                }
                Err(err) => println!("        (not an equation: {err})"),
            }
        }
        for (p, err) in &errs {
            println!("  ¶{p:<5} REFUSED: {err}");
        }
    }
    println!("\nequations read={total}  reportable findings={reportable}");
}
