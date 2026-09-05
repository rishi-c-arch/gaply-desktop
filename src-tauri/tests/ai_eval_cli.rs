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

/// §11 D79. The advisory figures printed to researchers must match a REAL eval
/// of the prompt that actually ships.
///
/// `audit_report` prints `ADVISORY_RECALL_PCT` / `ADVISORY_PRECISION_PCT` in the
/// report so the advisory claim is checkable (§11 D78). Nothing stopped those
/// constants drifting from the engine: for one commit the default was v3 —
/// measured at **0% recall** — while the report already advertised v4's 82%.
///
/// So this does not check that a report EXISTS. It finds the report for the
/// SHIPPED prompt version, recomputes recall and precision from its per-case
/// results against the cold labels, and asserts the constants match. Changing
/// the prompt, the default variant, or either number without a matching eval run
/// fails here — which is the only way a number printed to a user stays true.
#[test]
fn advisory_figures_match_a_real_eval_of_the_shipped_prompt() {
    use app_lib::ai::tasks::citation_need::PROMPT_VERSION;
    let shipped = PROMPT_VERSION;

    let cases: std::collections::HashMap<String, serde_json::Value> =
        std::fs::read_to_string("evals/citation_need.jsonl")
            .expect("case file")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).expect("case json");
                (v["id"].as_str().unwrap_or_default().to_string(), v)
            })
            .collect();

    // Recompute from a report's per-case results, over COLD labels only
    // (§11 D75): a suggested-accepted label carries the model's own answer and
    // cannot score it. Defined before selection so EVERY candidate can be
    // measured, not only the one that wins.
    let measure = |v: &serde_json::Value| -> (u32, u32, u32, u32, u32) {
        let (mut tp, mut fp, mut fn_) = (0u32, 0u32, 0u32);
        for r in v["results"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let id = r["id"].as_str().unwrap_or_default();
            let Some(case) = cases.get(id) else { continue };
            let Some(lab) = case.get("labelling") else { continue };
            if lab["provenance"].as_str() != Some("cold") {
                continue;
            }
            match (case["expected"]["needs_citation"].as_bool(), r["scored"]["got"]["needs_citation"].as_bool()) {
                (Some(true), Some(true)) => tp += 1,
                (Some(true), Some(false)) => fn_ += 1,
                (Some(false), Some(true)) => fp += 1,
                _ => {}
            }
        }
        let recall = (tp * 100).checked_div(tp + fn_).unwrap_or(0);
        let precision = (tp * 100).checked_div(tp + fp).unwrap_or(0);
        (tp, fp, fn_, recall, precision)
    };

    // WHICH report speaks for the printed numbers (§11 D83).
    //
    // This used to take the FIRST `read_dir` entry matching the prompt version.
    // `read_dir` order is filesystem order, not a decision — so a second report
    // for the shipped prompt (a diagnostic run, a re-measure, a five-case
    // reproduction) could silently become the authority for two numbers printed
    // to researchers, with nothing in the test or the constants changing. A
    // guard that can quietly change what it validates is worse than none,
    // because it still reads as one.
    //
    // `date` cannot decide it: the field is a caller-supplied TAG, by design
    // ("reproducible without a clock in the engine"), and the values on disk are
    // things like `bo-3b` and `labelled-v4`. So recency is not available, and
    // inventing a clock to get it would override that decision for a tiebreak.
    //
    // Instead the order is: **the report that scores the MOST cold labels wins**,
    // ties broken by filename descending so the order is total. That is not
    // arbitrary — the fullest measurement is the one entitled to authorise a
    // published number, and it excludes a narrow diagnostic run by what the run
    // IS rather than by what it is called.
    let cold_scored = |v: &serde_json::Value| -> usize {
        v["results"]
            .as_array()
            .map(|rs| {
                rs.iter()
                    .filter(|r| {
                        let id = r["id"].as_str().unwrap_or_default();
                        cases
                            .get(id)
                            .and_then(|c| c.get("labelling"))
                            .and_then(|l| l["provenance"].as_str())
                            == Some("cold")
                            && r["scored"]["got"]["needs_citation"].is_boolean()
                    })
                    .count()
            })
            .unwrap_or(0)
    };

    let mut candidates: Vec<(usize, String, serde_json::Value)> = Vec::new();
    for e in std::fs::read_dir("evals/reports").expect("reports dir").flatten() {
        let Ok(raw) = std::fs::read_to_string(e.path()) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { continue };
        if v["promptVersion"].as_str() == Some(shipped) && v.get("results").is_some() {
            let name = e.file_name().to_string_lossy().into_owned();
            candidates.push((cold_scored(&v), name, v));
        }
    }
    // Descending on both keys, so the winner is the same on every machine.
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    let tied = candidates.clone();
    let (n_cold, chosen, report) = candidates.into_iter().next().unwrap_or_else(|| {
        panic!(
            "no eval report found for the SHIPPED prompt {shipped}. The report prints \
             ADVISORY_RECALL_PCT/ADVISORY_PRECISION_PCT to researchers; they may not \
             describe a prompt nobody has measured. Run: cargo run --release --bin \
             ai-eval -- --task citation_need --cases evals/citation_need.jsonl --prompt \
             <variant>"
        )
    });

    // A report that scores almost nothing cannot authorise a published number,
    // and computing a percentage from it would be the failure that looks like a
    // result. Refuse rather than divide.
    const MIN_COLD_CASES: usize = 20;
    assert!(
        n_cold >= MIN_COLD_CASES,
        "the fullest report for the SHIPPED prompt {shipped} is {chosen}, which scores only \
         {n_cold} cold labelled cases (minimum {MIN_COLD_CASES}). ADVISORY_RECALL_PCT / \
         ADVISORY_PRECISION_PCT are printed to researchers and cannot rest on that few."
    );

    // A TIE AT THE TOP IS AMBIGUITY, NOT A COIN FLIP (§11 D83).
    //
    // Coverage plus filename is a total order, so SOME report always wins — but
    // if two equally full measurements of the shipped prompt disagree, picking
    // one by name is exactly the silent rebinding this is meant to end. Two
    // full runs that disagree (a different model, a changed case file) is a
    // question for a human, so it fails and names them both.
    let rivals: Vec<&(usize, String, serde_json::Value)> =
        tied.iter().filter(|(n, name, v)| *n == n_cold && *name != chosen && measure(v).3 != measure(&report).3).collect();
    assert!(
        rivals.is_empty(),
        "{} other report(s) measure the SHIPPED prompt {shipped} just as fully as {chosen} \
         and disagree with it on recall: {}. Two equally complete measurements cannot both \
         authorise the number printed to researchers — reconcile them, or remove the one \
         that is not the measurement of record.",
        rivals.len(),
        rivals.iter().map(|(_, n, v)| format!("{n} (recall {}%)", measure(v).3)).collect::<Vec<_>>().join(", ")
    );

    let (tp, fp, fn_, recall, precision) = measure(&report);
    assert!(tp + fn_ > 0, "no cold true cases scored — the set cannot support a recall figure");

    // Integer division and rounding can differ by a point; more than that means
    // the constants describe a different run.
    let recall_const = gaply_core::audit_report::ADVISORY_RECALL_PCT;
    let precision_const = gaply_core::audit_report::ADVISORY_PRECISION_PCT;
    assert!(
        recall.abs_diff(recall_const) <= 1,
        "the report tells researchers recall is {recall_const}%, but the eval of the \
         SHIPPED prompt {shipped} measures {recall}% (tp {tp}, fn {fn_}) in {chosen}, over \
         {n_cold} cold cases. Re-measure or correct the constant — this number is printed \
         to users."
    );
    assert!(
        precision.abs_diff(precision_const) <= 1,
        "the report tells researchers precision is {precision_const}%, but the eval of the \
         SHIPPED prompt {shipped} measures {precision}% (tp {tp}, fp {fp}) in {chosen}, over \
         {n_cold} cold cases. Re-measure or correct the constant — this number is printed \
         to users."
    );
}
