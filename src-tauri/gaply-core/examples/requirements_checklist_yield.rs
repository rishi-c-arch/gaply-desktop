//! **What would wiring `checklist_from_requirements` actually put on screen?**
//!
//! It has zero production callers. The shipping path
//! (`checklist_from_guidelines`) does three keyword matches on RAG chunks and
//! gives Nature Medicine ZERO journal items, while `journal_requirements` holds
//! 39 for that journal. Before wiring anything, this runs the unwired function
//! against the really-crawled tables and prints the rows, so the decision rests
//! on what a researcher would read rather than on a row count.
use gaply_core::extract::{self, docparse};
use gaply_core::journal_store::requirements_for;
use gaply_core::report::checklist_from_requirements;
use gaply_core::Database;

fn main() {
    let mut a = std::env::args().skip(1);
    let db_path = a.next().expect("db");
    let manuscript = a.next().expect("manuscript");
    let db = Database::open(std::path::Path::new(&db_path)).expect("open");
    let text = docparse::parse_path(std::path::Path::new(&manuscript)).expect("parse");
    let ex = extract::extract_from_text(&text);
    let words = text.split_whitespace().count();

    for key in ["nature-medicine", "bmj", "plos-medicine", "frontiers-public-health"] {
        let reqs = requirements_for(&db, key).unwrap_or_default();
        let fp = gaply_core::journal_fingerprint::fingerprint_for(&db, key).ok();
        let bindings: Vec<gaply_core::journal_standards::StandardBinding> = fp
            .as_ref()
            .map(|f| {
                f.standards
                    .iter()
                    .filter_map(|s| {
                        Some(gaply_core::journal_standards::StandardBinding {
                            standard: gaply_core::journal_standards::Standard::parse(&s.standard)?,
                            design: s.design.clone(),
                            source_span: s.source_span.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // **Design-independent only: NO bindings.** The reporting-standards
        // section loops over `bindings`; passing none emits no row that depends
        // on a study design, which is what D177's declined gate requires.
        // The real wiring: full requirements + bindings, then the caller's
        // design-independent filter — not a crippled call.
        let items = gaply_core::report::design_independent(
            checklist_from_requirements(&ex, &text, words, &reqs, &[]),
        );
        let _ = &bindings;
        let pass = items.iter().filter(|i| i.passed).count();
        let un = items.iter().filter(|i| i.unevaluable).count();
        println!(
            "\n######## {key}   {} requirement(s), {} binding(s) -> {} checklist item(s) ({pass} pass, {} fail, {un} undecided)",
            reqs.len(), bindings.len(), items.len(), items.len() - pass - un
        );
        let hedged = items.iter().filter(|i| i.requirement.contains("if your study is")).count();
        let applies = items.iter().filter(|i| i.requirement.contains(" applies to ")).count();
        println!("  of which: {hedged} hedged on an undetermined design, {applies} 'applies to' rows, {un} undecided");
        for i in items.iter().take(16) {
            let m = if i.unevaluable { "????" } else if i.passed { "PASS" } else { "FAIL" };
            println!("  [{m}] {}", i.requirement.chars().take(86).collect::<String>());
            println!("         {}", i.detail.chars().take(104).collect::<String>());
        }
        if items.len() > 12 { println!("  … {} more", items.len() - 12); }
    }
}
