//! Golden-file test: extraction output for a known manuscript must match a
//! frozen expected result. Regenerate with `UPDATE_GOLDEN=1 cargo test -p
//! gaply_core --test extraction_golden` after an intentional change, then
//! review the diff before committing.
//!
//! # [REGENERATED 15 Sep 2026 — §11 D169]
//!
//! `extract::Location` gained `section_index`, because a `SectionKind` is not a
//! unique address: over 20 real manuscripts, 283 of 869 statistical claims
//! resolved to a paragraph that did not contain them.
//!
//! **"Review the diff" above was done structurally, not by eye, and this is the
//! record of it:** 31 differences, every one of them an ADDED `section_index`
//! key. No value changed, no array changed length, no type changed. That is
//! what an additive field must look like, and a walk of both trees is the only
//! thing that can say so about a 6 KB JSON document — a visual scan of 31
//! scattered insertions cannot.

use gaply_core::extract::{extract_from_text_with, ExtractOptions};

const MANUSCRIPT: &str = include_str!("fixtures/manuscript.txt");

#[test]
fn extraction_matches_golden_file() {
    // WITH the scientific layer: it is opt-in for callers, and the golden file
    // is where its output stays pinned. A golden that only covered the base
    // structure would let the Stage-1 extractors drift unwatched.
    let result = extract_from_text_with(MANUSCRIPT, ExtractOptions::with_scientific());
    let actual = serde_json::to_value(&result).unwrap();

    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/expected.json");

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        let pretty = serde_json::to_string_pretty(&actual).unwrap();
        std::fs::write(golden_path, pretty + "\n").unwrap();
        eprintln!("golden file updated: {golden_path}");
        return;
    }

    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/expected.json")).unwrap();
    assert_eq!(actual, expected, "extraction output drifted from golden file");
}
