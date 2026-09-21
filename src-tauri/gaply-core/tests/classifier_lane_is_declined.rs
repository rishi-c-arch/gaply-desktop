//! **A prompt that tells the model its answer must not reach a model.**
//!
//! # The defect (§11 D203)
//!
//! `ai_detect::CLASSIFY_INSTRUCTION` opens *"This passage from a document was
//! flagged as carrying AI-associated statistical signals"* and
//! `classify_output_schema` offers only machine categories. It states the
//! conclusion as a premise and then provides no way to disagree with it.
//! Measured: **9 of 10 genuine pre-2020 OpenAlex abstracts, each with a DOI,
//! were assigned a machine category.**
//!
//! # Why this guard pins a PREMISE instead of the prompt's wording
//!
//! The obvious guard — "no instruction may contain a presupposition" — would
//! demand the prompt be rewritten, and **the rewrite was measured and is
//! worse**: a neutral framing plus a `human` option clears 8 of 10 machine
//! GENERATED and 7 of 10 PARAPHRASED abstracts as human, and
//! generated-vs-paraphrased falls from 9/20 to 4/20 against a 10/20 no-skill
//! line. The verdict tracks the prompt in both versions. So the lane is
//! UNEVALUABLE and the honest state is a decline, not a better prompt.
//!
//! CLAUDE.md's rule for a guard whose defect is currently unreachable is to
//! keep it and **pin the premise that makes it unreachable, so the day that
//! premise changes the test goes red.** The premise is that every shipped call
//! site passes `None` for the classifier. This asserts exactly that.
//!
//! Re-enabling the classifier therefore fails this test BY DESIGN, and the
//! failure says what new evidence is required first.
//!
//! Lives in `gaply_core` because both CI workflows run `-p gaply_core`; it
//! reads the APP crate's source by relative path, the way
//! `run_artefacts_name_their_model.rs` reads `../evals`.

const AICHECK: &str = "../src/aicheck.rs";

#[test]
fn every_shipped_call_site_declines_the_classifier() {
    let src = std::fs::read_to_string(AICHECK)
        .unwrap_or_else(|e| panic!("{AICHECK}: {e} — has the AI-Check flow moved?"));

    let calls: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && l.contains("classify_passages("))
        .collect();

    // POSITIVE COUNT FIRST. If the call moves or is renamed, this scan finds
    // nothing and would otherwise pass forever while the lane silently ships.
    assert!(
        !calls.is_empty(),
        "no `classify_passages(` call found in {AICHECK} — the guard is inert. \
         The AI-Check flow has moved; re-point this scan before trusting it."
    );

    for call in &calls {
        assert!(
            call.contains("classify_passages(None"),
            "a shipped call site passes a classifier: {call}\n\n\
             The AI-Check three-way lane is DECLINED (§11 D203). Its prompt asserts \
             the passage was already flagged and offers no human option — 9 of 10 real \
             pre-2020 abstracts were called machine. Correcting the prompt was measured \
             and is WORSE: it clears 8 of 10 machine-GENERATED abstracts as human and \
             drops generated-vs-paraphrased from 9/20 to 4/20 (no-skill 10/20).\n\n\
             Re-enabling needs evidence that a model's verdict DEPENDS on the text — a \
             per-class distribution differing from the marginal — measured BEFORE any \
             prompt is tuned. A better prompt is not that evidence."
        );
    }
}

#[test]
fn the_declined_prompt_still_carries_the_defect_it_was_measured_for() {
    // The prompt is kept as the instrument (the `novelty.rs` precedent: deleting
    // it would make the measurement unrepeatable). This pins that it is still the
    // artefact §11 D203 measured, so the decline above cannot quietly come to
    // describe some other prompt.
    let payload = gaply_core::ai_detect::classify_payload("A passage.");
    let instruction = payload["instruction"].as_str().expect("instruction is a string");
    assert!(
        instruction.contains("was flagged as carrying"),
        "the classification instruction no longer contains the presupposition §11 D203 \
         measured. If it was rewritten, the decline's numbers describe a prompt that no \
         longer exists — re-measure, or remove this test and say why."
    );
    // THE ASSERTION BELOW LOOKS BACKWARDS ON PURPOSE. Read §11 D203 before
    // changing it: `human` is absent from the shipped schema because adding it
    // was MEASURED and made the lane worse, not because anyone forgot.
    let categories = payload["output_schema"]["properties"]["category"]["enum"]
        .as_array()
        .expect("category enum");
    assert!(
        !categories.iter().any(|c| c == "human"),
        "the schema now offers `human`.\n\n\
         THIS IS NOT AN OVERSIGHT BEING CORRECTED — see §11 D203. Adding `human` \
         to this enum, with a neutral instruction, was measured on the same 30 items \
         and REVERSED the bias instead of removing it: human text called machine fell \
         9/10 -> 1/10, but 8 of 10 machine-GENERATED and 7 of 10 PARAPHRASED abstracts \
         were then cleared as `human`, and generated-vs-paraphrased fell 9/20 -> 4/20 \
         against a 10/20 no-skill line. For a research-integrity tool, clearing AI text \
         is the worse error: a false accusation invites a look, a false clearance ends one.\n\n\
         If you are shipping `human`, §11 D203's decline must be revisited with new \
         evidence — a model whose per-class verdict distribution differs from its \
         marginal, measured BEFORE any prompt is tuned — not silently widened here."
    );
}
