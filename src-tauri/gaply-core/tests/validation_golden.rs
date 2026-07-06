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
    assert!(report.outcome(RuleId::SmallSampleCausalClaim).passed, "n = 96 is not small");

    // Overall verdict fails because of the missing effect sizes.
    assert!(!report.passed);
    assert!(report.flags.iter().all(|f| f.rule == RuleId::MissingEffectSize));
}
