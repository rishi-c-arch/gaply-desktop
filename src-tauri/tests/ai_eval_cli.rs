//! CLI-level guards for the eval harness.
//!
//! These run the real binary, because the behaviour being protected is what
//! happens at the argument boundary — and the failure they exist to prevent is
//! a run that produces a full, plausible report for the wrong thing. D20 already
//! recorded one of those (a fixed default seed file scoring the wrong task's
//! seeds); a `--model` that silently fell back to the bundled 0.5B would be the
//! same class of bug with worse consequences, since the whole point of the
//! bake-off is to attribute numbers to models.
//!
//! Nothing here loads a model: every case must fail during argument handling,
//! which is also why they are fast.

use std::process::Command;

fn eval_bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ai-eval"));
    // Run from the crate root so the default seed paths resolve as they do in
    // normal use.
    c.current_dir(env!("CARGO_MANIFEST_DIR"));
    c
}

fn run(args: &[&str]) -> (bool, String) {
    let out = eval_bin().args(args).output().expect("the eval binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

#[test]
fn an_unknown_model_id_is_refused_and_names_the_candidates() {
    let (ok, out) = run(&["--task", "citation_support", "--model", "llama-3-8b"]);
    assert!(!ok, "an unknown model must not run: {out}");
    assert!(out.contains("unknown --model"), "{out}");
    // Naming what IS available is the difference between a usable error and a
    // wall. Both pinned candidates must appear.
    assert!(out.contains("qwen2.5-1.5b-instruct-q4km"), "{out}");
    assert!(out.contains("qwen2.5-3b-instruct-q4km"), "{out}");
}

#[test]
fn a_model_without_its_weights_directory_is_refused_rather_than_falling_back() {
    // THE dangerous case: silently using the bundled 0.5B here would publish
    // 0.5B numbers in a row labelled 3B.
    let (ok, out) = run(&["--task", "citation_support", "--model", "qwen2.5-3b-instruct-q4km"]);
    assert!(!ok, "{out}");
    assert!(out.contains("--model-dir"), "the error must say what is missing: {out}");
}

#[test]
fn a_model_dir_that_does_not_hold_the_weights_is_refused() {
    let (ok, out) = run(&[
        "--task",
        "citation_support",
        "--model",
        "qwen2.5-1.5b-instruct-q4km",
        "--model-dir",
        "/nonexistent/gaply/models",
    ]);
    assert!(!ok, "{out}");
    assert!(
        out.contains("not_found") || out.contains("generative model file"),
        "the error must name the missing file: {out}"
    );
}

#[test]
fn an_unknown_support_variant_is_refused() {
    let (ok, out) = run(&["--task", "citation_support", "--support-variant", "v3"]);
    assert!(!ok, "{out}");
    // The message must NAME the legal variants, including the D64 experiment
    // one — an error that lists two of three options sends the reader looking
    // for a flag that exists.
    assert!(out.contains("v1, v2, v1c, v1o, v1e or v1n"), "{out}");
}

/// §11 D107. The bare invocation ran the bundled 0.5B against a lexical mock
/// embedder and printed a complete, plausible report. D29's stamp said so and
/// was walked past anyway, so the default now refuses.
#[test]
fn citation_support_refuses_to_run_against_the_mock_by_default() {
    let (ok, out) = run(&["--task", "citation_support"]);
    assert!(!ok, "a mocked citation_support run must not be produced by default: {out}");
    // It must name BOTH substitutions, because either alone invalidates it.
    assert!(out.contains("0.5B"), "the error must say what model would have run: {out}");
    assert!(out.contains("mock"), "the error must say retrieval would be mocked: {out}");
    assert!(out.contains("--embedder-dir"), "{out}");
    assert!(out.contains("--model-dir"), "{out}");
    // And it must offer the way through, or it is a wall.
    assert!(out.contains("--smoke"), "the error must name the escape hatch: {out}");
}

/// The refusal must not swallow a MISTYPED flag — that is the user's slip and
/// is more useful to hear about than a policy default.
#[test]
fn a_wrong_argument_is_reported_before_the_missing_ones() {
    let (ok, out) = run(&["--task", "citation_support", "--model", "llama-3-8b"]);
    assert!(!ok, "{out}");
    assert!(out.contains("unknown --model"), "the typo must win over the policy refusal: {out}");
}

/// Other tasks are untouched: citation_need has no embedder in its pipeline.
#[test]
fn the_refusal_is_scoped_to_citation_support() {
    let (_, out) = run(&["--task", "citation_need", "--model", "llama-3-8b"]);
    assert!(!out.contains("--embedder-dir"), "citation_need was caught by D107's guard: {out}");
}

#[test]
fn the_bakeoff_summary_refuses_to_invent_a_document_from_nothing() {
    // An empty comparison document would read as "the bake-off found nothing",
    // which is a different claim from "no reports were found".
    let (ok, out) = run(&["--bakeoff", "no-such-prefix-xyz"]);
    assert!(!ok, "{out}");
    assert!(out.contains("no reports matching"), "{out}");
}

/// §11 D73. Every `citation_need` case must send EMPTY neighbours, because the
/// product does.
///
/// `job_runner` builds `CitationNeedInput` with `preceding_sentence:
/// String::new()` and `following_sentence: String::new()` — the shipped engine
/// never shows the model a neighbour. `ai-eval` deserialises whatever the case
/// file carries, so a case with neighbours is scored under a configuration that
/// does not ship: the §11 D68 shape, in the eval harness.
///
/// This is a property of the DATA, so it is asserted against the data rather
/// than left to whoever adds the next case. If neighbours are ever wanted, that
/// is a change to `job_runner` with its own measured cell — and this test is
/// what forces the two to move together.
#[test]
fn citation_need_cases_send_the_neighbours_the_product_sends() {
    let path = std::path::Path::new("evals/citation_need.jsonl");
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut checked = 0usize;
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let case: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line {}: {e}", i + 1));
        let id = case["id"].as_str().unwrap_or("<no id>");
        for field in ["preceding_sentence", "following_sentence"] {
            let v = case["input"][field].as_str().unwrap_or_else(|| {
                panic!("case {id}: input.{field} must be present (and empty)")
            });
            assert!(
                v.is_empty(),
                "case {id}: input.{field} is {v:?}, but job_runner sends an empty \
                 string — this case would be scored under a configuration the \
                 product does not run (§11 D73). Clear it, or change job_runner \
                 and measure that as its own cell."
            );
        }
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} cases checked — did the file move?");
}

/// §11 D75. A case labelled from the model's own suggestion cannot score the
/// model, and the case file must make that checkable rather than remembered.
///
/// Three provenances, not interchangeable:
///   cold                 — labelled without seeing the model. The ONLY kind
///                          that yields an unbiased accuracy number.
///   suggested_overridden — the human disagreed. A LOWER BOUND: these cases are
///                          selected precisely for disagreement.
///   suggested_accepted   — the label carries the model's own answer. Scoring
///                          against it is circular.
///
/// This asserts the field is present and one of the three whenever it exists,
/// and that an ACCEPTED case never claims a per-field difference — accepting
/// means agreeing, so a `differs: true` there would mean the record and the
/// keystroke disagree.
#[test]
fn labelled_cases_record_how_they_were_produced() {
    let path = std::path::Path::new("evals/citation_need.jsonl");
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let case: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line {}: {e}", i + 1));
        let id = case["id"].as_str().unwrap_or("<no id>");
        // Legacy seeds predate the field; anything the tool writes must have it.
        let Some(lab) = case.get("labelling") else { continue };
        let prov = lab["provenance"].as_str().unwrap_or_else(|| {
            panic!("case {id}: labelling.provenance missing")
        });
        assert!(
            matches!(prov, "cold" | "suggested_accepted" | "suggested_overridden"),
            "case {id}: unknown provenance {prov:?} — the eval cannot tell whether \
             this case may score the model"
        );
        if prov == "suggested_accepted" {
            if let Some(d) = lab.get("differs") {
                for field in ["needs_citation", "sentence_type", "severity"] {
                    assert_ne!(
                        d[field].as_bool(),
                        Some(true),
                        "case {id}: accepted the suggestion but records a difference in \
                         {field} — the record and the keystroke disagree"
                    );
                }
            }
        }
    }
}

/// §11 D79, §11 D123. A performance rate printed to a researcher must be
/// measured on THE POPULATION THE PRODUCT JUDGES.
///
/// # WHAT THE OLD GUARD CHECKED, AND WHY IT PASSED ON A NUMBER THAT WAS WRONG
///
/// The version of this test that shipped required `>= 20` cold labelled cases
/// and recomputed recall/precision from the eval report to confirm the printed
/// constants had not drifted. It did both correctly, and it certified "43%
/// precision" on a set drawn almost entirely from ONE PAPER'S FRONT THIRD.
///
/// It counted the labels. It never asked where they came from.
///
/// Measured on the manuscript that exposed this: of 65 judged sentences, the 14
/// labelled ones all sit above "Experimental Configuration". The 39 sentences
/// from there to the ACKNOWLEDGEMENT — Results, ablations, Discussion,
/// Conclusion — hold ZERO labels and produce 31 of the 46 suggestions.
/// Restricted to the audit's own selection the same model measured 25%
/// precision, not 43%.
///
/// So this guard is about COVERAGE, not count. A set of 500 Introduction
/// sentences still fails it.
///
/// # IT IS ARMED BY THE THING IT GUARDS
///
/// Nothing prints such a rate today (§11 D123 removed both constants), so there
/// is nothing to recompute — and a guard that merely returns while disarmed is
/// the shape that failed here before. It therefore reads `audit_report.rs` and
/// arms itself the moment a constant of that shape reappears, whatever it is
/// named.
#[test]
fn a_printed_advisory_rate_needs_a_representative_set() {
    // Per HALF, not in total. The old floor of 20 was met entirely by one.
    const MIN_PER_HALF: usize = 10;

    // WHY A DECLARED FIELD AND NOT THE SECTION TITLE.
    //
    // The first version of this guard classified cases by matching "result",
    // "discussion", "introduction" against `input.section`. On the real set it
    // classified 0 of 42, because papers name their sections whatever they
    // like: the cold cases carry "HEFCSO-BILSTM: A HYBRID", "1.1 Universal
    // Health Coverage and Employer Mandates", "Proposed HEFCSO Algorithm". A
    // keyword proxy standing in for the thing it cannot see is how the 43% was
    // certified in the first place, so the labeller declares it instead — they
    // have read the sentence, and the guard has not.
    const FIELD: &str = "population";
    const OWN_WORK: &str = "own_work";
    const PRIOR_WORK: &str = "prior_work";

    let source =
        std::fs::read_to_string("gaply-core/src/audit_report.rs").expect("audit_report.rs");
    let armed = source
        .lines()
        .any(|l| l.trim_start().starts_with("pub const ADVISORY_") && l.contains("u32"));

    let (mut own_work, mut prior_work, mut undeclared) = (0usize, 0usize, 0usize);
    let mut sections: std::collections::BTreeSet<String> = Default::default();
    for line in std::fs::read_to_string("evals/citation_need.jsonl").expect("case file").lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).expect("case json");
        if v["labelling"]["provenance"].as_str() != Some("cold") {
            continue;
        }
        sections.insert(v["input"]["section"].as_str().unwrap_or("(none)").to_string());
        match v["labelling"][FIELD].as_str() {
            Some(OWN_WORK) => own_work += 1,
            Some(PRIOR_WORK) => prior_work += 1,
            _ => undeclared += 1,
        }
    }

    if !armed {
        // DISARMED, and it says so with the numbers that would decide — so a
        // reader who reinstates a constant learns the cost before the failure.
        eprintln!(
            "no ADVISORY_* rate is printed, so this guard is dormant. Cold cases: {prior_work} \
             prior-work, {own_work} own-work, {undeclared} with no `labelling.{FIELD}`. Before \
             any rate may be printed, {MIN_PER_HALF} of EACH are needed. Sections present: {:?}",
            sections
        );
        return;
    }

    assert_eq!(
        undeclared, 0,
        "a rate is printed to researchers, but {undeclared} cold cases do not declare \
         `labelling.{FIELD}` ({OWN_WORK:?} or {PRIOR_WORK:?}). A rate cannot be certified \
         against a set whose population is unknown — that is §11 D123 exactly. Sections \
         present: {sections:?}"
    );
    assert!(
        own_work >= MIN_PER_HALF && prior_work >= MIN_PER_HALF,
        "a rate is printed to researchers from {prior_work} prior-work and {own_work} own-work \
         cold cases; {MIN_PER_HALF} of each are needed. The audit judges Results, Discussion and \
         Conclusion sentences — mostly the authors' own work, mostly needing no citation — and a \
         set that skips them measures a population the product never sees. Label that half, or \
         print no rate."
    );
}
