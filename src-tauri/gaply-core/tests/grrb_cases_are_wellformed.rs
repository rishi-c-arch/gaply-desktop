//! **The benchmark's cases and its committed baseline must not drift apart.**
//!
//! # Why this lives in `gaply-core` when the benchmark does not
//!
//! `grrb` and `grrb-gate` are bins in the APP crate, and both CI workflows run
//! `-p gaply_core`. So the benchmark, the pre-commit gate, the prose guard and
//! the D-number citation guard are now four instruments in the same blind spot
//! (§11 D194, §11 D195): each gates a developer's `cargo test --workspace` and
//! nothing remote. This test is deliberately placed where CI can see it, and it
//! reads the case files by relative path — `cargo test -p gaply_core` runs with
//! the package root as the working directory, so `../evals/grrb` is
//! `src-tauri/evals/grrb`, and a CI checkout has those files because they are
//! committed.
//!
//! # What it catches that the gate cannot
//!
//! `grrb-gate` BLOCKS when the case set changes, which is correct — two reports
//! over different cases have different denominators. But blocking is a local,
//! pre-commit event, and a case added without re-baselining reaches the remote
//! as a committed baseline that describes a set which no longer exists. From
//! then on every gate run blocks, and the failure looks like the gate being
//! broken rather than the baseline being stale.
//!
//! So: the baseline's case ids must equal the case files' ids, checked where CI
//! can fail on it.

use std::collections::BTreeSet;

const FAMILIES: [&str; 7] = [
    "mathematical",
    "statistical",
    "manuscript_consistency",
    "literature",
    "journal",
    "adversarial",
    "scientific_extraction",
];
const DIR: &str = "../evals/grrb";
const BASELINE: &str = "../evals/reports/grrb-baseline.json";

/// §6c.1 asks for fifty. The floor is asserted rather than assumed, because a
/// benchmark that quietly shrinks still reports percentages.
const MIN_TOTAL: usize = 50;
/// "at least five per family" — Prompt 4, verbatim.
const MIN_PER_FAMILY: usize = 5;

#[test]
fn every_grrb_case_is_wellformed_and_the_baseline_describes_this_set() {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut total = 0usize;

    for fam in FAMILIES {
        let path = format!("{DIR}/{fam}.jsonl");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{path}: {e} — has the benchmark moved?"));
        let mut n = 0usize;
        for (i, line) in src.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let at = format!("{path}:{}", i + 1);
            let v: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("{at}: {e}"));

            let id = v["id"].as_str().unwrap_or_else(|| panic!("{at}: no id"));
            assert!(
                ids.insert(id.to_string()),
                "{at}: duplicate case id {id:?} — two cases with one id make the \
                 baseline's set ambiguous"
            );
            assert_eq!(
                v["family"].as_str(),
                Some(fam),
                "{at}: filed under {fam}.jsonl but declares a different family"
            );
            // Prompt 4: "Every case has a provenance note." A case whose origin
            // is unrecorded cannot be re-adjudicated, and a benchmark nobody can
            // re-adjudicate is a set of numbers.
            for field in ["provenance", "note", "stratum"] {
                let s = v[field].as_str().unwrap_or("");
                assert!(!s.trim().is_empty(), "{at}: {field} is empty");
            }
            assert!(
                v["expected"]["finding"].is_boolean(),
                "{at}: expected.finding must be a bool"
            );
            assert!(v["input"]["kind"].is_string(), "{at}: input.kind must be a string");
            n += 1;
        }
        assert!(
            n >= MIN_PER_FAMILY,
            "{fam}: {n} cases, and Prompt 4 asks for at least {MIN_PER_FAMILY} per family"
        );
        total += n;
    }
    assert!(total >= MIN_TOTAL, "{total} cases; §6c.1 asks for {MIN_TOTAL}");

    // The drift this test exists for.
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(BASELINE).unwrap_or_else(|e| panic!("{BASELINE}: {e}")),
    )
    .unwrap_or_else(|e| panic!("{BASELINE}: {e}"));
    let scored: BTreeSet<String> = report["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{BASELINE}: no cases array"))
        .iter()
        .map(|c| c["id"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(!scored.is_empty(), "{BASELINE} scored nothing — it would pass vacuously");

    let missing: Vec<&String> = ids.difference(&scored).collect();
    let extra: Vec<&String> = scored.difference(&ids).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "the committed baseline does not describe the current case set.\n  \
         in the cases but not the baseline: {missing:?}\n  \
         in the baseline but not the cases: {extra:?}\n  \
         Re-baseline with: (cd src-tauri && cargo run --bin grrb -- \
         evals/reports/grrb-baseline.json)"
    );
}
