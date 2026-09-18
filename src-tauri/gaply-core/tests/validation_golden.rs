//! End-to-end wiring test: feed the Extraction Agent's golden-file output
//! (Prompt 5's `expected.json`) straight into the Validation Agent and
//! confirm the deterministic verdicts.

use gaply_core::extract::{ExtractionResult, SectionKind};
use gaply_core::validate::{validate, RuleId};

const GOLDEN_EXTRACTION: &str = include_str!("fixtures/expected.json");

#[test]
fn validates_real_extraction_output() {
    let extraction: ExtractionResult =
        serde_json::from_str(GOLDEN_EXTRACTION).expect("golden extraction should deserialize");

    let report = validate(&extraction);

    // The fixture reports p-values in the Abstract and Results without any
    // effect size → missing-effect-size must fire in both places.
    let mes = report.flags_for(RuleId::MissingEffectSize);
    assert!(
        mes.iter().any(|f| f.location.section == SectionKind::Abstract),
        "expected missing-effect-size flag in Abstract"
    );
    assert!(
        mes.iter().any(|f| f.location.section == SectionKind::Results),
        "expected missing-effect-size flag in Results"
    );

    // The fixture DOES report CIs alongside its primary p-values, uses a
    // t-test only for two groups, makes no overclaim, and has n = 96.
    assert!(report.outcome(RuleId::MissingConfidenceInterval).passed, "CIs are present");
    assert!(report.outcome(RuleId::TestGroupMismatch).passed, "t-test used for two groups");
    assert!(report.outcome(RuleId::PValueOverclaim).passed, "no overclaiming language");
    // **`SmallSampleCausalClaim` is DECLINED (§11 D178), so it has no outcome.**
    // This line used to read `outcome(SmallSampleCausalClaim).passed` with the
    // comment "n = 96 is not small" — a PASS, meaning "we checked the sample
    // size and it was fine". That is the claim the decline withdraws: the rule
    // read any paragraph-level `n`, not the study's, at a measured precision of
    // 0 of 15 over 20 manuscripts. Asserting its ABSENCE is what keeps a future
    // reinstatement from passing this test silently.
    assert!(
        report.checks.iter().all(|c| c.rule != RuleId::SmallSampleCausalClaim),
        "a declined rule must report no outcome — an outcome says the check ran"
    );

    // Overall verdict fails because of the missing effect sizes.
    assert!(!report.passed);
    assert!(report.flags.iter().all(|f| f.rule == RuleId::MissingEffectSize));
}
