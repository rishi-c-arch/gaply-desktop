//! **A doc comment that names a test is claiming that test exists.**
//!
//! # The sibling of `decision_records.rs`, for a citation nothing checked
//!
//! `tests/decision_records.rs` catches a cited D-number with no entry, and did
//! so twice on 16 Sep 2026. **Test names in doc comments have the same failure
//! mode and had no guard.** Found 17 Sep 2026, in `pipeline.rs`:
//!
//! > *"the golden test `the_report_is_byte_identical_with_a_research_state_derived`
//! > pins that adding it changed no output"*
//!
//! No such test exists. The claim WAS pinned — by
//! `the_report_is_byte_identical_to_the_pre_research_state_capture`, which
//! compares against a capture taken at `e5eb963`, before `ResearchState`
//! existed — but the pointer was stale.
//!
//! **A stale pointer is worse than a missing one.** A reader who greps for the
//! named test and finds nothing cannot tell whether the claim is UNPINNED (and
//! the invariant is unguarded) or the NAME is wrong (and it is guarded
//! elsewhere). One of those is an emergency and the other is a typo, and the
//! evidence looks identical.
//!
//! # Keyed on FRAMING, not on the shape of the identifier
//!
//! The first attempt matched identifiers that look like test names — a
//! verb-phrase prefix, twelve characters or more. **It was wrong on 6 of 9**:
//! `no_heldout_evaluation_named` is a finding code, `any_ingested` is a struct
//! field, `every_rule_has_a_declared_tier` is a test that does exist.
//!
//! What works is the sentence around the citation. A doc comment saying *"the
//! test `x`"*, *"`x` pins"*, *"pinned by `x`"* is making a claim about `x`
//! being a test; a bare backticked identifier is not. **Measured over the whole
//! crate: 12 framed citations, 3 flagged, 2 genuine defects and 1 legitimate
//! `mod`** — which is why `mod` counts as a definition here too.
//!
//! # What it cannot catch
//!
//! 1. A test that exists but does NOT assert what the comment says it does.
//!    Only reading it can tell you that.
//! 2. A citation with no framing — `see \`foo\`` is not matched, deliberately,
//!    because the false-positive rate on bare identifiers is 67%.
//! 3. A test in another crate. The scan covers this workspace's Rust sources.

use std::path::Path;

/// Roots scanned for both citations and definitions.
const ROOTS: &[&str] = &["src", "tests", "../src", "../tests"];

/// Phrases that make a backticked identifier a claim about a TEST.
/// Order matters only for readability; any match counts.
const FRAMINGS: &[&str] = &[
    "test `",
    "tests `",
    "pinned by `",
    "guard `",
];

/// Framings where the identifier comes FIRST: `` `x` pins ``.
const TRAILING: &[&str] = &[
    "` pins",
    "` asserts",
    "` is the test",
    "` catches",
    "` keeps passing",
];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Every `fn` and `mod` name defined anywhere in the workspace's Rust sources.
fn defined_names(files: &[std::path::PathBuf]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for f in files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        for line in src.lines() {
            let l = line.trim_start();
            for kw in ["fn ", "mod "] {
                if let Some(rest) = l.strip_prefix(kw).or_else(|| {
                    l.strip_prefix("pub ").and_then(|r| r.strip_prefix(kw))
                }) {
                    let name: String =
                        rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    if !name.is_empty() {
                        out.insert(name);
                    }
                }
            }
        }
    }
    out
}

/// Identifiers cited in a way that claims they are tests, with their site.
fn framed_citations(files: &[std::path::PathBuf]) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    for f in files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let rel = f.to_string_lossy().replace('\\', "/");
        for (i, line) in src.lines().enumerate() {
            if !line.trim_start().starts_with("//") {
                continue;
            }
            for frame in FRAMINGS {
                let mut from = 0usize;
                while let Some(j) = line[from..].find(frame) {
                    let start = from + j + frame.len();
                    let name: String = line[start..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    from = start.max(from + 1);
                    if name.len() >= 12 {
                        out.push((name, rel.clone(), i + 1));
                    }
                }
            }
            for frame in TRAILING {
                if let Some(j) = line.find(frame) {
                    let before = &line[..j];
                    let name: String = before
                        .chars()
                        .rev()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if name.len() >= 12 && before.ends_with(&name) {
                        out.push((name, rel.clone(), i + 1));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn every_doc_comment_that_names_a_test_names_one_that_exists() {
    let mut files = Vec::new();
    for r in ROOTS {
        rust_files(Path::new(r), &mut files);
    }
    assert!(
        files.len() > 50,
        "only {} rust files found — the scan is not reaching the crate and would pass \
         vacuously",
        files.len()
    );

    let defined = defined_names(&files);
    assert!(
        defined.len() > 200,
        "only {} definitions parsed — the parser has drifted and this would pass vacuously",
        defined.len()
    );

    // THE GUARD MUST NOT CATCH ITSELF. This file's own header QUOTES the
    // dangling citation it was written to catch, as the worked example — and
    // quoting a defect is not committing one. Exempting the file by name rather
    // than rephrasing the header keeps the example verbatim, which is the part
    // that makes the entry legible.
    let cited: Vec<_> = framed_citations(&files)
        .into_iter()
        .filter(|(_, file, _)| !file.ends_with("tests/cited_tests_exist.rs"))
        .collect();
    assert!(
        !cited.is_empty(),
        "no framed citations found at all — the FRAMINGS list no longer matches how this \
         codebase writes, and the guard is silently inert"
    );

    let missing: Vec<String> = cited
        .iter()
        .filter(|(name, _, _)| !defined.contains(name))
        .map(|(name, file, line)| format!("  {file}:{line}  names `{name}` — no such fn or mod"))
        .collect();

    assert!(
        missing.is_empty(),
        "a doc comment names a test that does not exist.\n\n\
         A STALE POINTER IS WORSE THAN A MISSING ONE: a reader who greps and finds nothing \
         cannot tell whether the invariant is unguarded or the name is simply wrong.\n\n\
         Fix the name, or write the test, or drop the claim.\n\n{}\n\n\
         ({} framed citation(s) checked against {} definitions)",
        missing.join("\n"),
        cited.len(),
        defined.len()
    );
}
