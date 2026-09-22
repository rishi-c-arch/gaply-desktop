//! **§11 D205's pool is §11 D204's pool, or its comparison is between two sets.**
//!
//! `evals/grrb/scientific_extraction_ctx.jsonl` is
//! `evals/grrb/scientific_extraction.jsonl` with each paragraph's place in its
//! document joined on. Every figure in §11 D205 is a delta against §11 D204 on
//! the rows both payloads could answer, so the two files must carry the SAME
//! paragraphs with the SAME labels. An edit to either — a relabelled row, a
//! re-exported context, a paragraph reworded — leaves both files parsing, leaves
//! `score_ctx.py` printing, and silently makes the comparison one between two
//! different pools.
//!
//! **The second half pins the privacy boundary.** §11 D205 reports that the
//! four-field context payload passes the proxy's validator on 144 of 160 and
//! that those 16 exclusions are a STRICT SUPERSET of §11 D204's 12 — which is
//! the whole reason the two runs are comparable without relaxing anything. The
//! limits are read out of `gaply-proxy/app/validation.py` rather than copied, so
//! widening `MAX_FIELD_CHARS` or `MAX_SENTENCES_PER_FIELD` reddens this test.
//!
//! **WHAT THIS TEST CANNOT SEE, stated here because a green run will otherwise
//! be read as more than it is.** It re-implements the validator's two per-field
//! rules in Rust from constants it reads; it does not execute
//! `validate_structured`. A change to that function's LOGIC — a new rule, a
//! different sentence pattern — is invisible to it. Nothing in CI runs the
//! proxy's own Python tests either: none of the five workflows invokes `pytest`
//! (checked 22 Sep 2026), so this is the only automatic guard on those limits,
//! and it guards their VALUES, not their behaviour.

use std::path::PathBuf;

fn grrb_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../evals/grrb")
}

fn read_jsonl(name: &str) -> Vec<serde_json::Value> {
    let p = grrb_dir().join(name);
    let s = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("jsonl row"))
        .collect()
}

/// `MAX_FIELD_CHARS = 2000` -> `2000`, from the proxy's own source.
fn proxy_limit(src: &str, name: &str) -> usize {
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with(name))
        .unwrap_or_else(|| panic!("{name} not found in gaply-proxy/app/validation.py"));
    let rhs = line.split('=').nth(1).expect("assignment");
    rhs.split('#')
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("{name} is not an integer: {e}"))
}

/// The validator's `_SENTENCE = re.compile(r"[.!?]+(?:\s|$)")`, counted the same
/// way `re.findall` counts it: non-overlapping, left to right.
fn sentences(s: &str) -> usize {
    regex::Regex::new(r"[.!?]+(?:\s|$)").unwrap().find_iter(s).count()
}

#[test]
fn the_context_file_is_the_same_160_paragraphs_with_the_same_labels() {
    let base = read_jsonl("scientific_extraction.jsonl");
    let ctx = read_jsonl("scientific_extraction_ctx.jsonl");

    // A POSITIVE count first: an enumeration that silently becomes empty would
    // otherwise satisfy every assertion below forever.
    assert_eq!(base.len(), 160, "the fixed set is 160 paragraphs (§11 D196)");
    assert_eq!(ctx.len(), 160, "the context file must carry every row of it");

    let positives = ctx.iter().filter(|r| r["expected"]["finding"] == true).count();
    assert_eq!(
        positives, 26,
        "26 positives after the §11 D198 corrections — a different number means \
         the labels moved, and every §11 D196/D197/D204/D205 figure is on the old ones"
    );

    for (b, c) in base.iter().zip(ctx.iter()) {
        let id = b["id"].as_str().expect("id");
        assert_eq!(c["id"].as_str(), Some(id), "row order must match, so the pools line up");
        assert_eq!(
            b["input"]["text"], c["input"]["text"],
            "{id}: the paragraph text differs between the fixed set and the context file"
        );
        assert_eq!(
            b["expected"]["finding"], c["expected"]["finding"],
            "{id}: the label differs between the fixed set and the context file"
        );
        assert!(
            c["context"]["heading"].is_string(),
            "{id}: carries no section heading — the context file exists to supply one"
        );
        assert!(
            c["context"]["prev"].is_string() && c["context"]["next"].is_string(),
            "{id}: carries no neighbours"
        );
    }
}

#[test]
fn the_context_payload_passes_on_144_rows_and_excludes_a_superset_of_d204s_12() {
    let src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../gaply-proxy/app/validation.py");
    let src = std::fs::read_to_string(&src_path).unwrap_or_else(|e| {
        panic!("cannot read {} — §11 D205's pool is defined by it: {e}", src_path.display())
    });
    let max_field = proxy_limit(&src, "MAX_FIELD_CHARS");
    let max_sentences = proxy_limit(&src, "MAX_SENTENCES_PER_FIELD");
    let prose_min = proxy_limit(&src, "PROSE_MIN_FIELD_CHARS");
    let max_total = proxy_limit(&src, "MAX_TOTAL_CHARS");

    // The longest of the two instructions actually sent (variant A: framing +
    // question). The worst case, so the total is not understated.
    const INSTRUCTION_CHARS: usize = 765;

    let refused = |f: &str| -> bool {
        f.chars().count() > max_field
            || (sentences(f) > max_sentences && f.chars().count() > prose_min)
    };

    let ctx = read_jsonl("scientific_extraction_ctx.jsonl");
    let mut ctx_excluded: Vec<String> = Vec::new();
    let mut base_excluded: Vec<String> = Vec::new();
    let mut worst_total = 0usize;

    for r in &ctx {
        let id = r["id"].as_str().unwrap().to_string();
        let text = r["input"]["text"].as_str().unwrap_or("");
        let heading = r["context"]["heading"].as_str().unwrap_or("");
        let prev = r["context"]["prev"].as_str().unwrap_or("");
        let next = r["context"]["next"].as_str().unwrap_or("");

        let total = heading.chars().count()
            + prev.chars().count()
            + text.chars().count()
            + next.chars().count()
            + INSTRUCTION_CHARS;
        worst_total = worst_total.max(total);

        if total > max_total || [heading, prev, text, next].iter().any(|f| refused(f)) {
            ctx_excluded.push(id.clone());
        }
        // The §11 D204 payload: the paragraph alone.
        if text.chars().count() + INSTRUCTION_CHARS > max_total || refused(text) {
            base_excluded.push(id);
        }
    }

    assert_eq!(
        base_excluded.len(),
        12,
        "§11 D204 recorded 12 exclusions on the paragraph-only payload; got {:?}",
        base_excluded
    );
    assert_eq!(
        ctx_excluded.len(),
        16,
        "§11 D205 ran on 144 of 160; got {} excluded: {:?}",
        ctx_excluded.len(),
        ctx_excluded
    );
    for id in &base_excluded {
        assert!(
            ctx_excluded.contains(id),
            "{id} is excluded from the §11 D204 pool but not the context pool — the \
             exclusions are no longer a superset, so the two runs are on different rows"
        );
    }
    assert!(
        worst_total <= max_total,
        "no row may be refused on the TOTAL limit: §11 D205 records that all four \
         extra exclusions are long NEIGHBOURS, not oversized payloads"
    );
    assert!(
        worst_total > max_total / 2,
        "worst total is {worst_total} against a {max_total} limit — implausibly small, \
         which is what an empty or truncated context file looks like here"
    );
}
