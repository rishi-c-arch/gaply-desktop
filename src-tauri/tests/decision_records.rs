//! Every `§11 D<n>` cited in the source must resolve to an entry in the plan.
//!
//! # Why this is a test rather than a habit
//!
//! Three times now a commit has shipped a code comment citing a decision record
//! that was never written — `§11 D71`, then `§11 D118` and `D119` together. **A
//! citation that reads as provenance and leads nowhere is worse than no
//! citation**: it tells a reader the reasoning was recorded and sends them to
//! find it, and the cost lands on whoever trusts it most.
//!
//! Each time the fix was the same one-line grep, run AFTER the fact. Running it
//! before is the whole difference, and that is what a test is for. This is the
//! same lesson as every other guard in this project: enforce it, do not remember
//! it.
//!
//! Scope is deliberately narrow — it checks that a cited number EXISTS, not that
//! the entry says what the comment claims. A test cannot read for meaning, and
//! pretending otherwise would be a guard that passes while measuring nothing.

use std::collections::BTreeSet;
use std::path::Path;

/// Walk `.rs` files under a directory, skipping build output.
fn rust_files(root: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "node_modules") {
                continue;
            }
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Every `### D<n>` heading in the plan.
fn recorded_entries(plan: &str) -> BTreeSet<u32> {
    plan.lines()
        .filter_map(|l| l.strip_prefix("### D"))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// Every `§11 D<n>` cited in a source file.
fn cited_in(src: &str) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for (i, _) in src.match_indices("§11 D") {
        let rest = &src[i + "§11 D".len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse() {
            out.insert(n);
        }
    }
    out
}

#[test]
fn every_cited_decision_record_exists() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("repo root");
    let plan_path = repo.join("docs/AI_ENGINE_PLAN.md");
    let plan = std::fs::read_to_string(&plan_path)
        .unwrap_or_else(|e| panic!("{}: {e}", plan_path.display()));
    let recorded = recorded_entries(&plan);
    assert!(recorded.len() > 50, "only {} entries parsed — the heading format changed?", recorded.len());

    let mut files = Vec::new();
    rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("gaply-core/src"), &mut files);
    assert!(!files.is_empty(), "no source files found — the walk is broken");

    let mut dangling: Vec<String> = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        for n in cited_in(&src) {
            if !recorded.contains(&n) {
                dangling.push(format!(
                    "  D{n} cited in {} but no `### D{n}` in docs/AI_ENGINE_PLAN.md",
                    f.strip_prefix(repo).unwrap_or(f).display()
                ));
            }
        }
    }
    dangling.sort();
    dangling.dedup();
    assert!(
        dangling.is_empty(),
        "decision records cited but never written — a citation that reads as \
         provenance and leads nowhere is worse than none:\n{}\n\nWrite the entry, \
         or drop the reference.",
        dangling.join("\n")
    );
}
