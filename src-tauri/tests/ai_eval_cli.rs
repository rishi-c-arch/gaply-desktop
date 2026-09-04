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
    assert!(out.contains("v1, v2 or v1c"), "{out}");
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
