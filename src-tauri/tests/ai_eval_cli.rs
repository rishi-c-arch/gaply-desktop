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
    // THE SET'S MIX MUST MATCH THE POPULATION'S, NOT MERELY CLEAR A FLOOR.
    //
    // A per-half floor is a proxy, and this test exists because a proxy is what
    // certified 43%. If a paper's judged sentences are ~45% prior-work / ~55%
    // own-work and the labelled set is 78/22, ten of each clears every floor and
    // the printed number still over-claims — it is weighted toward the half
    // where "does this need a citation?" is a real question, and away from the
    // half that is mostly the authors' own work and mostly needs none.
    //
    // So: compare SHARES, within a tolerance, and say what to label.
    // A proportion over three cases is noise, so each stratum carries a floor.
    // This is a FLOOR, never a certificate: clearing it says the estimate can
    // be computed, not that it was reported weighted (§11 D126).
    const MIN_PER_STRATUM: usize = 10;
    // Below this, the population itself is not known well enough to compare
    // against: too much of the document sits in sections nobody has classified.
    const MAX_UNDECLARED_SHARE: f64 = 25.0;

    const OWN_WORK: &str = "own_work";
    const PRIOR_WORK: &str = "prior_work";

    // ONE INSTRUMENT, BOTH SIDES (§11 D125).
    //
    // The population's mix can only ever be section-based — classifying all 347
    // judged sentences by hand IS the labelling job, so there is no per-sentence
    // population to compare against. The sample is therefore read the same way:
    // a case's half is looked up from (document, section) in this map, never
    // stored on the case. A per-sentence override would make the sample
    // sentence-level while the population stayed section-level, and a comparison
    // between two different instruments measures the instruments.
    //
    // The cost is honest and bounded: a prior-work claim sitting inside a
    // Results section counts as own-work on BOTH sides. The stratum is
    // "sentences in sections of this kind", and the population carries exactly
    // the same contamination as the sample drawn from it.
    let mix: serde_json::Value = std::fs::read_to_string("evals/population_mix.json")
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let half_of = |doc: &str, section: &str| -> Option<String> {
        mix.get(doc)?["sections"][section]["population"].as_str().map(str::to_string)
    };

    let source =
        std::fs::read_to_string("gaply-core/src/audit_report.rs").expect("audit_report.rs");
    let armed = source
        .lines()
        .any(|l| l.trim_start().starts_with("pub const ADVISORY_") && l.contains("u32"));

    // ---- the SAMPLE: what was labelled, and from which documents ----
    let (mut own_work, mut prior_work, mut undeclared) = (0usize, 0usize, 0usize);
    let mut documents: std::collections::BTreeSet<String> = Default::default();
    let mut undeclared_sections: std::collections::BTreeSet<String> = Default::default();
    for line in std::fs::read_to_string("evals/citation_need.jsonl").expect("case file").lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).expect("case json");
        if v["labelling"]["provenance"].as_str() != Some("cold") {
            continue;
        }
        let Some(doc) = v["labelling"]["document"].as_str() else {
            undeclared += 1;
            continue;
        };
        documents.insert(doc.to_string());
        let section = v["input"]["section"].as_str().unwrap_or_default();
        match half_of(doc, section).as_deref() {
            Some(OWN_WORK) => own_work += 1,
            Some(PRIOR_WORK) => prior_work += 1,
            _ => {
                undeclared += 1;
                undeclared_sections.insert(format!("{doc} :: {section}"));
            }
        }
    }
    let labelled = own_work + prior_work;
    let set_own_share = if labelled == 0 { 0.0 } else { 100.0 * own_work as f64 / labelled as f64 };

    // ---- the POPULATION: what the audit actually judges on those documents ----
    //
    // Measured by `label-cn` from a real pre-pass and committed as
    // `evals/population_mix.json`; classified by the labeller's `--population`
    // declaration, per section. Sections nobody declared are counted as UNKNOWN
    // rather than folded into a half, because guessing there is the same proxy
    // this test refuses.
    let (mut pop_own, mut pop_prior, mut pop_unknown) = (0usize, 0usize, 0usize);
    let mut missing_docs: Vec<String> = Vec::new();
    for doc in &documents {
        let Some(entry) = mix.get(doc) else {
            missing_docs.push(doc.clone());
            continue;
        };
        for (_name, sec) in entry["sections"].as_object().into_iter().flatten() {
            let n = sec["planned"].as_u64().unwrap_or(0) as usize;
            match sec["population"].as_str() {
                Some(OWN_WORK) => pop_own += n,
                Some(PRIOR_WORK) => pop_prior += n,
                _ => pop_unknown += n,
            }
        }
    }
    let pop_total = pop_own + pop_prior + pop_unknown;
    let pop_known = pop_own + pop_prior;
    let pop_own_share = if pop_known == 0 { 0.0 } else { 100.0 * pop_own as f64 / pop_known as f64 };
    let undeclared_share =
        if pop_total == 0 { 100.0 } else { 100.0 * pop_unknown as f64 / pop_total as f64 };

    // How many own-work cases would bring the sample's share to the
    // population's, holding the prior-work cases fixed. This is the number the
    // next person needs; "you cannot print a rate" is not actionable.
    // UNDER STRATIFICATION THE SAMPLE NEED NOT MATCH THE POPULATION'S MIX.
    //
    // Re-weighting is exactly what removes that requirement — demanding both a
    // matched mix and correct weights would ask the stratification to do
    // nothing, and would send the labeller after 44 cases to earn a number that
    // 15 can support. What must hold instead is a floor per stratum (a
    // proportion over three cases is noise) and that the weights were applied.
    let short_own = MIN_PER_STRATUM.saturating_sub(own_work);
    let short_prior = MIN_PER_STRATUM.saturating_sub(prior_work);
    let advice = if pop_known == 0 {
        "the population's mix is not known yet — declare sections with \
         `label-cn <doc> --section <name> --population <half>`"
            .to_string()
    } else if short_own + short_prior > 0 {
        format!(
            "label {short_own} more own-work and {short_prior} more prior-work case(s) to reach \
             the floor of {MIN_PER_STRATUM} per stratum; the mix need NOT match — the weights \
             carry the population in"
        )
    } else {
        "both strata are above the floor; any printed rate must be the WEIGHTED estimate"
            .to_string()
    };
    let state = format!(
        "sample: {labelled} cold cases ({prior_work} prior-work, {own_work} own-work; \
         {undeclared} undeclared). population across {:?}: {pop_prior} prior-work, \
         {pop_own} own-work = weights {:.0}/{:.0}, {pop_unknown} of {pop_total} judged \
         sentences unclassified ({undeclared_share:.0}%). {advice}.",
        documents,
        100.0 - pop_own_share,
        pop_own_share,
    );

    if !armed {
        // DISARMED, and it still reports the numbers that would decide, so the
        // reader who reinstates a constant learns the cost before the failure.
        eprintln!("no ADVISORY_* rate is printed, so this guard is dormant. {state}");
        return;
    }

    assert!(
        undeclared == 0 && missing_docs.is_empty(),
        "a rate is printed, but {undeclared} cold case(s) sit in sections with no declared \
         half {undeclared_sections:?}, and {missing} document(s) have no measured population \
         in evals/population_mix.json ({missing_docs:?}). A rate cannot be certified against a \
         set whose population is unknown. Declare with \
         `label-cn <doc> --section <name> --population <half>`. {state}",
        missing = missing_docs.len(),
    );
    assert!(
        undeclared_share <= MAX_UNDECLARED_SHARE,
        "a rate is printed, but {undeclared_share:.0}% of the judged sentences sit in sections \
         nobody has classified as prior-work or own-work (limit {MAX_UNDECLARED_SHARE:.0}%), so \
         the population's mix is not known well enough to compare against. Declare those \
         sections with `label-cn <doc> --section <name> --population <half>`. {state}"
    );
    assert!(
        own_work >= MIN_PER_STRATUM && prior_work >= MIN_PER_STRATUM,
        "a rate is printed from {prior_work} prior-work and {own_work} own-work cold cases; \
         {MIN_PER_STRATUM} of each are needed before a per-stratum proportion means \
         anything. {state}"
    );
    // ---- THE WEIGHTING MUST ACTUALLY HAVE BEEN APPLIED (§11 D126) ----
    //
    // A per-stratum floor is not enough, and neither is a mix comparison. A
    // stratified estimate that is COMPUTED and then REPORTED UNWEIGHTED looks
    // rigorous, keeps every stratum populated, clears every floor — and prints
    // the pooled number, which is the §11 D123 over-claim wearing better
    // clothes. So the printed figure is recomputed here from the per-stratum
    // counts and the MEASURED population weights, and must match.
    //
    // The sample no longer has to match the population's mix: re-weighting is
    // what removes that requirement, and demanding both would be asking the
    // stratification to do nothing. What replaces it is this identity check.
    // Score the shipped prompt's eval report PER STRATUM, against cold labels
    // only (§11 D75) — a suggested-accepted label carries the model's own
    // answer and cannot score it.
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
    let shipped = app_lib::ai::tasks::citation_need::PROMPT_VERSION;
    let report = std::fs::read_dir("evals/reports")
        .expect("reports dir")
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(|v| v["promptVersion"].as_str() == Some(shipped))
        .max_by_key(|v| v["results"].as_array().map(|a| a.len()).unwrap_or(0));
    let report = report.unwrap_or_else(|| {
        panic!(
            "a rate is printed but no eval report exists for the SHIPPED prompt {shipped}. \
             The number cannot be recomputed, so it cannot be checked. {state}"
        )
    });

    let mut counts: std::collections::HashMap<&str, (u32, u32, u32, u32)> = Default::default();
    for r in report["results"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let Some(case) = cases.get(r["id"].as_str().unwrap_or_default()) else { continue };
        if case["labelling"]["provenance"].as_str() != Some("cold") {
            continue;
        }
        let doc = case["labelling"]["document"].as_str().unwrap_or_default();
        let section = case["input"]["section"].as_str().unwrap_or_default();
        let Some(half) = half_of(doc, section) else { continue };
        let key = if half == OWN_WORK { OWN_WORK } else { PRIOR_WORK };
        let c = counts.entry(key).or_default();
        match (
            case["expected"]["needs_citation"].as_bool(),
            r["scored"]["got"]["needs_citation"].as_bool(),
        ) {
            (Some(true), Some(true)) => c.0 += 1,
            (Some(false), Some(true)) => c.1 += 1,
            (Some(true), Some(false)) => c.2 += 1,
            (Some(false), Some(false)) => c.3 += 1,
            _ => {}
        }
    }
    let build = |name: &str, population: usize| {
        let (tp, fp, fn_, tn) = counts.get(name).copied().unwrap_or_default();
        app_lib::ai::eval_strata::Stratum {
            name: name.to_string(),
            population,
            tp,
            fp,
            fn_,
            tn,
        }
    };
    let strata = vec![build(PRIOR_WORK, pop_prior), build(OWN_WORK, pop_own)];
    let weighted = app_lib::ai::eval_strata::stratified_precision_pct(&strata);
    let pooled = app_lib::ai::eval_strata::pooled_precision_pct(&strata);
    let shares = app_lib::ai::eval_strata::population_shares(&strata);
    // Named, not positional: "68/32" tells the reader nothing about which half
    // carried which weight, and the whole failure is about which half dominates.
    let weights =
        shares.iter().map(|(n, p)| format!("{n} {p}%")).collect::<Vec<_>>().join(", ");
    let printed = printed_precision_pct(&source);

    let (Some(weighted), Some(printed)) = (weighted, printed) else {
        panic!(
            "a rate is printed but it cannot be recomputed: weighted={weighted:?}, \
             printed={printed:?}. Every stratum with population needs a sample, and the \
             printed constant must be parseable from audit_report.rs. {state}"
        )
    };
    assert!(
        printed.abs_diff(weighted) <= 1,
        "THE PRINTED RATE DOES NOT CARRY ITS WEIGHTS. printed {printed}%, recomputed \
         {weighted}% from {own} own-work at {own_p}% and {prior} prior-work at {prior_p}% \
         weighted {weights}{pooled_note}. A stratified estimate reported unweighted passes \
         every floor and is the same over-claim as the 43% (§11 D123). Publish the weighted \
         figure, or do not publish one.",
        own = strata[1].sample(),
        prior = strata[0].sample(),
        own_p = per_stratum_precision(&strata[1]),
        prior_p = per_stratum_precision(&strata[0]),
        pooled_note = match pooled {
            Some(p) if p != weighted => format!(" (pooled would read {p}%)"),
            _ => String::new(),
        },
    );
}

/// The rate `audit_report.rs` actually prints, parsed from the constant.
fn printed_precision_pct(source: &str) -> Option<u32> {
    source
        .lines()
        .find(|l| l.trim_start().starts_with("pub const ADVISORY_PRECISION_PCT"))
        .and_then(|l| l.rsplit_once('='))
        .and_then(|(_, v)| v.trim().trim_end_matches(';').parse().ok())
}

fn per_stratum_precision(s: &app_lib::ai::eval_strata::Stratum) -> String {
    match s.tp + s.fp {
        0 => "n/a".to_string(),
        d => format!("{}", (100 * s.tp) / d),
    }
}
