//! **A decision record that names SLM-1 or SLM-2 must say WHICH WEIGHTS, or say
//! it is unmeasured.**
//!
//! # The defect
//!
//! §11 D196 and D197 reported numbers for "SLM1" that came from the bundled
//! **stock** `Qwen2.5-0.5B-Instruct-Q4_K_M`. SLM-1 is a LoRA over
//! Qwen2.5-**7B**-Instruct (§11 D199) and was not in those runs at all. §11 D198
//! then inherited the same rows and carried the false label three entries
//! further — found by this scan before it was committed.
//!
//! Worse than a wrong number: `models/mod.rs:972` DECLINES SLM-2's whole lane on
//! a probe that names `qwen3:4b`, the stock base, because no LoRA path existed to
//! test the tuned model with. **A decline is harder to reopen than a number**,
//! because nobody re-runs what is recorded as settled.
//!
//! # What it requires
//!
//! An entry mentioning SLM-1/SLM-2 must also contain a concrete weights
//! identifier — a parameter count, a base repo, an Ollama tag — or an explicit
//! statement that the thing is unmeasured. It is deliberately satisfiable by
//! honesty as well as by precision: "SLM-2 is unmeasured" passes, and should.
//!
//! It does NOT check that a claim is true. It checks that the claim is
//! ANSWERABLE: a reader can tell which weights produced the number.
//!
//! Lives in `gaply_core` because both CI workflows run `-p gaply_core`, and reads
//! the plan by relative path — `cargo test -p gaply_core` runs with the package
//! root as the working directory, and the doc is committed.

const PLAN: &str = "../../docs/AI_ENGINE_PLAN.md";

/// A concrete weights identifier, or an admission of ignorance. Either resolves
/// "which model produced this?"; nothing else does.
const DISAMBIGUATORS: [&str; 9] = [
    "Qwen2.5-7B",
    "Qwen3-4B",
    "qwen3:4b",
    "Qwen2.5-0.5B",
    "qwen2.5-0.5b",
    "gaply-slm-models",
    "unmeasured",
    "never been measured",
    "never measured",
];

fn mentions_an_slm(body: &str) -> bool {
    for pat in ["SLM-1", "SLM-2", "SLM1", "SLM2"] {
        if body.contains(pat) {
            return true;
        }
    }
    false
}

#[test]
fn every_record_naming_an_slm_says_which_weights_or_says_unmeasured() {
    let src = std::fs::read_to_string(PLAN)
        .unwrap_or_else(|e| panic!("{PLAN}: {e} — has the plan moved?"));

    // Split on the entry headings so each record is judged on its own text.
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("### D") {
            if let Some(e) = current.take() {
                entries.push(e);
            }
            let id = rest.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("");
            current = Some((format!("D{id}"), String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(e) = current.take() {
        entries.push(e);
    }

    assert!(
        entries.len() > 100,
        "only {} decision records parsed from {PLAN} — the heading parser has drifted \
         and this guard would pass vacuously",
        entries.len()
    );

    let naming: Vec<&(String, String)> =
        entries.iter().filter(|(_, b)| mentions_an_slm(b)).collect();
    assert!(
        !naming.is_empty(),
        "no record mentions SLM-1 or SLM-2 at all — either the names changed or the \
         scan is inert; both need a human"
    );

    let bare: Vec<&str> = naming
        .iter()
        .filter(|(_, b)| !DISAMBIGUATORS.iter().any(|d| b.contains(d)))
        .map(|(id, _)| id.as_str())
        .collect();

    assert!(
        bare.is_empty(),
        "these records name SLM-1 or SLM-2 without saying which weights: {bare:?}\n  \
         Add a concrete identifier ({}), or say plainly that it is unmeasured.\n  \
         §11 D196-D198 reported \"SLM1\" figures produced by the bundled stock 0.5B \
         while SLM-1 is a LoRA over the 7B, and a reader had no way to tell.",
        DISAMBIGUATORS[..6].join(", ")
    );
}
