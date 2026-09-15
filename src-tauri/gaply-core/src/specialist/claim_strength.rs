//! **Claim–evidence strength, and the causal-overclaim check (§4.6).**
//!
//! §4.6: *"The evidence graph traced claim → analysis → result → conclusion,
//! classified `SUPPORTED · PARTIALLY_SUPPORTED · UNSUPPORTED · CONTRADICTED ·
//! UNVERIFIED`. The specific check the reviewer document names: **causal
//! overclaim** — the study supports "associated with", the conclusion says
//! "causes". That is a Tier 2 finding with the sentence and the design that
//! limits it both cited."*
//!
//! # §12 DECLINES THIS NODE, AND IT IS RIGHT ABOUT HALF OF IT
//!
//! §12's Phase-4 table lists *claim–evidence strength* as DECLINED, against
//! **0 of 123 claims linking to a variable, method or dataset**
//! (`claims.rs:277-279` writes `Vec::new()`). That is correct, and it decides
//! the first half: **the claim → analysis → result → conclusion trace cannot be
//! walked**, because the edges do not exist.
//!
//! It does not decide the second half. The causal-overclaim check needs two
//! things and neither is a claim link:
//!
//! 1. a conclusion sentence asserting causation, and
//! 2. the design that limits it,
//!
//! and both are **manuscript text** — the free tier's own layer, read by the
//! same `ExtractionResult` the shipped specialists already read. So the check
//! ships and the trace does not, which is a finer answer than declining the
//! node.
//!
//! # `SUPPORTED` AND `CONTRADICTED` ARE UNREACHABLE, AND §4.6 CONTRADICTS
//! PHASE 1 IN ASKING FOR THEM
//!
//! Phase 1 built the evidence graph and recorded, in this document, why it
//! emits `ClaimCoLocatedWithStatistic` and not `Supports`:
//!
//! > *"`Supports` would be a claim about meaning; extraction can only see
//! > position."*
//!
//! §4.6 then asks the same graph for `SUPPORTED` and `CONTRADICTED`. A statistic
//! in the same paragraph as a claim is evidence that they are near each other.
//! Reading it as support is exactly the inference Phase 1 refused, and doing it
//! here would silently reverse a decision made with its reason written down.
//!
//! Both variants exist, because they are §4.6's wire contract and a Tier-2
//! judge reading the claim and the statistic together will produce them. **No
//! path in this module returns either**, and
//! `support_and_contradiction_are_never_concluded_from_co_location` pins it.
//!
//! # THE INPUT, MEASURED BEFORE THE REPORT SHAPE WAS CHOSEN
//!
//! `examples/claim_evidence_audit.rs`, 20 real manuscripts, 528 claims:
//!
//! * **65 (12.3%)** are traced to a result — the evidence graph's one
//!   claim→result edge, `ClaimCoLocatedWithStatistic`.
//! * **463 (87.7%)** are `UNVERIFIED`.
//! * `SUPPORTED` / `CONTRADICTED`: **0**, and unreachable by construction.
//!
//! A per-claim report over that is 463 rows saying *"we could not check this."*
//! So the shipped output is the causal-overclaim finding only, and
//! [`assess_claims`] stays a measurement surface rather than a report section.
//! **87.7% is not a reason to loosen the tracing** — it is what an extractor
//! that sees position and not meaning can honestly say, and the way to move it
//! is a real claim↔analysis link.
//!
//! **Both halves of the overclaim check are common; the pairing is not.** Of
//! the 20: 15 state a design that admits causation, 2 state an association-only
//! design, 3 state none, and 5 make a causal conclusion — **0 have both
//! halves**. The check has therefore **never fired on real input**: its
//! precision here is undefined, not high, and only
//! `a_causal_conclusion_on_a_cross_sectional_design_is_a_finding` shows it can
//! fire.
//!
//! # WHAT THE CORPUS SAID, BEFORE ANY RULE WAS WRITTEN
//!
//! The first causal-verb list contained `increased`, `reduced`, `improved`,
//! `enhanced`, `produced` and `driven by`. Run over the six manuscripts it
//! called *"Chloride ranged through 30–82 mg L⁻¹ …, increased toward the inlet
//! stations"* a causal claim — 13 such sentences in one manuscript, nearly all
//! descriptions of direction. Those terms are gone, and
//! `examples/causal_design_scan.rs` is where that was measured.
//!
//! Two more came from the same run and are guards rather than list entries:
//!
//! * *"achieving state-of-the-art **results in** terms of 96.42% accuracy"* —
//!   a causal phrase inside an idiom ([`NOT_CAUSAL_IDIOMS`]).
//! * *"Cross-sectional design: **No causal direction can be established**."* —
//!   the manuscript limiting itself, which is the author doing the right thing.
//!   A check that flagged it would punish the honest paper and stay silent on
//!   the one that says nothing ([`DISCLAIMERS`]).

use serde::{Deserialize, Serialize};

use crate::agent_graph::{Cluster, EvidencePolicy, EvidenceSource, TrustTier};
use crate::epistemic::EpistemicStatus;
use crate::extract::{sentence, ExtractionResult, Location, SectionKind};
use crate::report::FindingSeverity;

use super::{Finding, Specialist, SpecialistInput};

/// §4.6's five strengths. **Never a number** — there is no score field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceStrength {
    /// A judge read the claim and the analysis and found the second carries the
    /// first. **Not reachable from co-location** — see the module header.
    Supported,
    /// A statistic sits with the claim. Whether it bears on the claim is not
    /// decided here.
    PartiallySupported,
    /// The design the manuscript describes cannot carry the claim it makes.
    Unsupported,
    /// A judge found the analysis contradicts the claim. **Not reachable.**
    Contradicted,
    /// Nothing decisive. The default.
    Unverified,
}

impl EvidenceStrength {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceStrength::Supported => "SUPPORTED",
            EvidenceStrength::PartiallySupported => "PARTIALLY_SUPPORTED",
            EvidenceStrength::Unsupported => "UNSUPPORTED",
            EvidenceStrength::Contradicted => "CONTRADICTED",
            EvidenceStrength::Unverified => "UNVERIFIED",
        }
    }
}

/// Phrases that assert causation.
///
/// **What is NOT here is the measurement.** `increased`, `reduced`, `improved`,
/// `enhanced`, `produced`, `driven by` were in the first list and describe
/// direction, not causation; they produced 13 false positives on one manuscript.
pub const CAUSAL_PHRASES: &[&str] = &[
    "causes",
    "caused",
    "causing",
    "cause of",
    "causal effect",
    "causally",
    "leads to",
    "lead to",
    "led to",
    "results in",
    "resulted in",
    "induces",
    "induced",
    "gives rise to",
    "brings about",
    "attributable to",
    // **"the result of" needs a copula.** Bare, it matched "The result of the
    // physicochemical characterization of the nanoemulsion was as follows" —
    // a heading for a table of measurements.
    "is the result of",
    "was the result of",
    "are the result of",
    "were the result of",
];

/// **A hyphen before `induced` makes it a noun modifier, not an assertion.**
///
/// Measured on `formulation-and-evaluation-of-thermosensitive-nanoemulsion-b.docx`:
/// of seven sentences the causal list matched, **six were `X-induced` compounds
/// naming a model system** — *"LPS-induced neuroinflammation"*,
/// *"Amyloid-β (Aβ)-induced Alzheimer's disease models"*, *"MPTP-induced
/// Parkinson's disease model"*, *"oleic-acid-induced disruption"*,
/// *"osmotic-induced ciliary dysfunction"*, *"gel-induced enhancement"*. None
/// asserts that this study established a cause; each names the thing being
/// studied.
///
/// They produced no finding only because that manuscript is experimental and
/// the design gate held. On an observational paper every one would have been a
/// causal overclaim finding. This is the `rema`**`in `**`unexplored` family:
/// a match inside a compound is not a match.
fn is_noun_modifier(sentence_lower: &str, phrase: &str) -> bool {
    if !matches!(phrase, "induced" | "induces") {
        return false;
    }
    // TRUE when EVERY occurrence is hyphen-prefixed: one bare `induced`
    // elsewhere in the sentence is a real assertion and must still count.
    let mut any = false;
    for (i, _) in sentence_lower.match_indices(phrase) {
        any = true;
        if !sentence_lower[..i].ends_with('-') {
            return false;
        }
    }
    any
}

// **`responsible for` was in this list and is not.** Measured over the six
// manuscripts it matched three sentences, and none of the three asserts
// causation about the study's own result: two describe a LIMITATION — *"the
// chemical identity of the toxicant responsible for the acute zebrafish
// response was not established"* — and one describes the experiment's aim. A
// phrase whose every real occurrence is non-assertive costs recall it never
// had and buys false positives.

/// Phrases with which a manuscript LIMITS its own causal reading. A sentence
/// carrying one is never a finding.
pub const DISCLAIMERS: &[&str] = &[
    "no causal",
    "not causal",
    "cannot establish caus",
    "can not establish caus",
    "does not imply caus",
    "do not imply caus",
    "cannot be inferred",
    "causality cannot",
    "precludes causal",
    "limits causal",
    "mere association",
    "to causal inference",
    "no causation",
];

/// Idioms containing a causal phrase that assert nothing causal.
pub const NOT_CAUSAL_IDIOMS: &[&str] =
    &["results in terms of", "results in table", "results in figure", "results in fig"];

/// Design language that admits only association.
pub const OBSERVATIONAL_MARKERS: &[&str] = &[
    "cross-sectional",
    "cross sectional",
    "observational study",
    "observational design",
    "correlational",
    "questionnaire survey",
    "retrospective",
    "cohort study",
    "case-control",
    "case control",
    "secondary data analysis",
];

// **`self-reported` was in this list and is not, and it is the sharper of the
// two removals** because it was PLAUSIBLE AND DISPLAYED. It describes how the
// DATA was collected, not how the STUDY was designed. On `R PAPER .docx` it
// made `admits_only_association` true and produced, as *"the design that limits
// it"*, the sentence *"The ISEAR dataset [22] consists of 7,666 self-reported
// emotional statements."* — a dataset description offered as a study design. A
// causal conclusion in that paper would have been flagged against it, and the
// row would have read like a real finding. With it gone R PAPER names no design
// at all and the specialist DECLINES, which is the true answer.
//
// `secondary data` became `secondary data analysis` for the same reason: the
// bare phrase names a data source, the full one names a design.

/// Design language that admits a causal reading.
///
/// **Scanned over the WHOLE manuscript, which is deliberately the suppressing
/// direction**: one mention of a control group anywhere — including in a
/// literature review discussing somebody else's trial — silences the check.
/// A missed overclaim is a gap; a fabricated one tells a researcher their
/// controlled experiment is a design error.
pub const EXPERIMENTAL_MARKERS: &[&str] = &[
    "randomised",
    "randomized",
    "randomly assigned",
    "randomly allocated",
    "controlled trial",
    "control group",
    "treatment group",
    "intervention group",
    "placebo",
    "double-blind",
    "single-blind",
    "experimental group",
    "in-vitro",
    "in vitro",
    "microcosm",
];

/// One claim, with what can honestly be said about the evidence under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAssessment {
    /// The manuscript sentence, whole.
    pub claim_span: String,
    pub location: Location,
    pub strength: EvidenceStrength,
    /// How many statistics the extractor found in the same paragraph. **A
    /// count of proximity, not of support.**
    pub statistics_co_located: usize,
    pub uncertainty: String,
}

/// **The design the manuscript describes, as a pair of term lists it matched.**
///
/// Both halves are returned, whole, so a reader can see what the verdict rested
/// on rather than being told one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignReading {
    pub observational: Vec<String>,
    pub experimental: Vec<String>,
    /// The sentence that first stated an observational design, whole. This is
    /// §4.6's *"the design that limits it"*, and a finding without it would be
    /// an assertion the reader cannot check.
    pub limiting_span: Option<String>,
    pub limiting_location: Option<Location>,
}

impl DesignReading {
    /// Only association is admitted: an observational marker is present and no
    /// experimental one is.
    pub fn admits_only_association(&self) -> bool {
        !self.observational.is_empty() && self.experimental.is_empty()
    }
}

/// Read the design out of the manuscript.
pub fn read_design(r: &ExtractionResult) -> DesignReading {
    let mut reading = DesignReading {
        observational: Vec::new(),
        experimental: Vec::new(),
        limiting_span: None,
        limiting_location: None,
    };
    for section in &r.sections {
        if section.kind == SectionKind::References {
            continue;
        }
        for (p_idx, para) in section.paragraphs.iter().enumerate() {
            let lower = para.to_lowercase();
            for m in EXPERIMENTAL_MARKERS.iter().filter(|m| lower.contains(**m)) {
                if !reading.experimental.iter().any(|x| x == m) {
                    reading.experimental.push((*m).to_string());
                }
            }
            for m in OBSERVATIONAL_MARKERS.iter().filter(|m| lower.contains(**m)) {
                if !reading.observational.iter().any(|x| x == m) {
                    reading.observational.push((*m).to_string());
                }
                if reading.limiting_span.is_none() {
                    // The SENTENCE, not the paragraph: the paragraph is what the
                    // matcher saw, the sentence is what a reader checks.
                    let loc = Location { section: section.kind, paragraph: p_idx, section_index: None };
                    if let Some(s) = sentence::sentences_in(para)
                        .into_iter()
                        .find(|s| s.to_lowercase().contains(*m))
                    {
                        reading.limiting_span = Some(s.trim().to_string());
                        reading.limiting_location = Some(loc);
                    }
                }
            }
        }
    }
    reading
}

/// Sections in which a sentence is a conclusion about the work, rather than a
/// description of somebody else's.
fn is_concluding(kind: SectionKind) -> bool {
    matches!(kind, SectionKind::Abstract | SectionKind::Discussion | SectionKind::Conclusion)
}

/// **Assess every concluding sentence.** Pure: no model, no network, no clock.
pub fn assess_claims(r: &ExtractionResult) -> Vec<ClaimAssessment> {
    let design = read_design(r);
    let mut out = Vec::new();

    for section in r.sections.iter().filter(|s| is_concluding(s.kind)) {
        for (p_idx, para) in section.paragraphs.iter().enumerate() {
            let loc = Location { section: section.kind, paragraph: p_idx, section_index: None };
            let stats_here =
                r.statistics.iter().filter(|s| s.location == loc).count();
            for sent in sentence::sentences_in(para) {
                let lower = sent.to_lowercase();
                if DISCLAIMERS.iter().any(|d| lower.contains(*d))
                    || NOT_CAUSAL_IDIOMS.iter().any(|d| lower.contains(*d))
                {
                    continue;
                }
                let Some(_) = CAUSAL_PHRASES
                    .iter()
                    .find(|c| lower.contains(**c) && !is_noun_modifier(&lower, c))
                else {
                    continue;
                };
                let strength = if design.admits_only_association() {
                    EvidenceStrength::Unsupported
                } else if stats_here > 0 {
                    EvidenceStrength::PartiallySupported
                } else {
                    EvidenceStrength::Unverified
                };
                out.push(ClaimAssessment {
                    claim_span: sent.trim().to_string(),
                    location: loc.clone(),
                    strength,
                    statistics_co_located: stats_here,
                    uncertainty: uncertainty_for(strength),
                });
            }
        }
    }
    out
}

fn uncertainty_for(s: EvidenceStrength) -> String {
    match s {
        EvidenceStrength::Unsupported =>
            "The design was read from term matches over the whole manuscript. A design \
             stated in words none of the lists match reads here as absent."
                .into(),
        EvidenceStrength::PartiallySupported =>
            "A statistic sits in the same paragraph. Co-location is proximity, not support \
             — whether that statistic bears on this claim is not decided here."
                .into(),
        _ => "No statistic sits with this claim and the design does not restrict it to \
              association. Nothing here decides whether the evidence carries it."
            .into(),
    }
}

/// **The causal-overclaim specialist.** Tier 2 per §4.4 — it reasons about
/// whether causal language is earned, which is evidence reasoning rather than a
/// checklist. `model: None` all the same: the reasoning is a rule over two term
/// lists and both spans are quoted.
pub struct ClaimStrengthSpecialist;

impl Specialist for ClaimStrengthSpecialist {
    fn id(&self) -> &'static str {
        "claim_evidence_strength"
    }
    fn cluster(&self) -> Cluster {
        Cluster::MethodologicalSoundness
    }
    fn trust_tier(&self) -> TrustTier {
        TrustTier::EvidenceReasoning
    }
    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy {
            min_sources: 1,
            permitted_sources: vec![EvidenceSource::ManuscriptSpan],
            citation_required: true,
            uncertainty_required: true,
        }
    }
    /// Declines when the manuscript states no design at all, because the check
    /// is *"the design supports association and the conclusion says causes"* and
    /// with no design read there is no first half. Ran-and-found-nothing and
    /// never-ran are different answers.
    fn applies_to(&self, input: &SpecialistInput<'_>) -> Option<String> {
        let d = read_design(input.extraction);
        if d.observational.is_empty() && d.experimental.is_empty() {
            return Some(
                "the manuscript names no study design this check recognises, so there is \
                 nothing to weigh a causal claim against — the check needs the design, not \
                 just the claim"
                    .into(),
            );
        }
        None
    }
    fn assess(&self, input: &SpecialistInput<'_>) -> Vec<Finding> {
        let r = input.extraction;
        let design = read_design(r);
        if !design.admits_only_association() {
            return Vec::new();
        }
        let Some(limiting) = design.limiting_span.clone() else {
            return Vec::new();
        };

        assess_claims(r)
            .into_iter()
            .filter(|a| a.strength == EvidenceStrength::Unsupported)
            .map(|a| Finding {
                specialist: "claim_evidence_strength".into(),
                code: "causal_claim_from_associational_design".into(),
                severity: FindingSeverity::Major,
                status: EpistemicStatus::Detected,
                summary: format!(
                    "A concluding sentence asserts causation while the manuscript \
                     describes a design that supports association only ({}). The design \
                     sentence is quoted below beside the claim.",
                    design.observational.join(", ")
                ),
                // **BOTH spans, which is what §4.6 asks for.** A causal
                // overclaim finding showing only the claim asks the reader
                // to take the design on trust.
                span: Some(format!(
                    "CLAIM: {}\nDESIGN THAT LIMITS IT: {}",
                    a.claim_span, limiting
                )),
                location: Some(a.location.clone()),
                evidence: vec![EvidenceSource::ManuscriptSpan],
                uncertainty: Some(format!(
                    "{} It also reads the design over the whole manuscript, so a single \
                     mention of a control group — including one describing another \
                     study — silences this check entirely.",
                    a.uncertainty
            )),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(text: &str) -> ExtractionResult {
        crate::extract::extract_from_text(text)
    }
    fn run_on(text: &str) -> super::super::SpecialistReport {
        let r = ex(text);
        let input = SpecialistInput { extraction: &r, science: None, analysis: None };
        super::super::run(&ClaimStrengthSpecialist, &input)
    }

    const OVERCLAIM: &str = "Abstract\n\nWe report findings from a cross-sectional survey \
        of 412 firms.\n\nDiscussion\n\nOrganisational capacity causes higher insurance \
        provision among firms.\n";

    /// **This check must never read the claim extractor's output.**
    ///
    /// §4.6 scopes it to *"every major claim"*. Measured, that scoping
    /// classifies **0 of 528 claims** across 20 manuscripts: 8.9% are whole
    /// sentences, 2.1% are whole and in a concluding section, and none of those
    /// assert causation. The check ships because it reads concluding SENTENCES
    /// and the design field instead — so a future edit reaching for
    /// `input.science` would be re-scoping it onto an input with no yield, and
    /// this is where that gets caught.
    #[test]
    fn the_check_produces_the_same_findings_with_and_without_the_scientific_layer() {
        let base = crate::extract::extract_from_text(OVERCLAIM);
        let enriched = crate::extract::extract_from_text_with(
            OVERCLAIM,
            crate::extract::ExtractOptions::with_scientific(),
        );
        assert!(enriched.scientific.is_some(), "precondition: the layer is populated");

        let without = super::super::run(
            &ClaimStrengthSpecialist,
            &SpecialistInput { extraction: &base, science: None, analysis: None },
        );
        let with = super::super::run(
            &ClaimStrengthSpecialist,
            &SpecialistInput {
                extraction: &enriched,
                science: enriched.scientific.as_deref(),
                analysis: None,
            },
        );
        assert_eq!(without.admitted, with.admitted, "the declined layer changes nothing");
        assert_eq!(without.admitted.len(), 1, "and the check still fires: {without:?}");
    }

    /// **The positive control.** The check had never fired on the six real
    /// manuscripts — every one of them was correctly silent — so a case it must
    /// fire on is the only thing that shows it fires at all.
    #[test]
    fn a_causal_conclusion_on_a_cross_sectional_design_is_a_finding() {
        let report = run_on(OVERCLAIM);
        assert_eq!(report.admitted.len(), 1, "{report:?}");
        let f = &report.admitted[0];
        assert_eq!(f.code, "causal_claim_from_associational_design");
        let span = f.span.as_deref().unwrap();
        assert!(span.contains("causes higher insurance provision"), "{span}");
        assert!(
            span.contains("cross-sectional survey"),
            "§4.6 asks for the sentence AND the design that limits it: {span}"
        );
    }

    /// **`X-induced` names a model system; it does not assert a cause.**
    ///
    /// Six of the seven causal matches on one real manuscript were these. They
    /// produced no finding only because that paper is experimental — on an
    /// observational one, every sentence below would have been reported as a
    /// causal overclaim.
    #[test]
    fn a_hyphenated_induced_compound_is_not_a_causal_claim() {
        for sentence in [
            "Lipopolysaccharide (LPS)-induced neuroinflammation: Assessment of reductions \
             in pro-inflammatory cytokines in brain homogenates.",
            "MPTP-induced Parkinson's disease model: Assessment of dopaminergic neuron \
             protection in the substantia nigra.",
            "Adjusted to 280-320 mOsm/kg to prevent osmotic-induced ciliary dysfunction.",
        ] {
            let r = ex(&format!(
                "Abstract\n\nA cross-sectional survey of 412 firms.\n\nDiscussion\n\n{sentence}\n"
            ));
            let claims = assess_claims(&r);
            assert!(
                claims.is_empty(),
                "a noun modifier is not an assertion, and this manuscript is \
                 observational so it WOULD have been reported: {sentence:?} -> {claims:?}"
            );
        }
    }

    /// And a bare `induced` in the same sentence still counts — the rule is
    /// about the compound, not about the word.
    #[test]
    fn a_bare_induced_beside_a_hyphenated_one_still_asserts_a_cause() {
        let r = ex(
            "Abstract\n\nA cross-sectional survey.\n\nDiscussion\n\nLPS-induced \
             neuroinflammation was measured, and capacity induced higher provision \
             across firms.\n",
        );
        assert_eq!(assess_claims(&r).len(), 1, "{:?}", assess_claims(&r));
    }

    /// *"The result of the physicochemical characterization … was as follows"*
    /// is a heading for a table, not a causal claim. `the result of` now needs
    /// a copula.
    #[test]
    fn the_result_of_without_a_copula_is_not_a_causal_claim() {
        let r = ex(
            "Abstract\n\nA cross-sectional survey.\n\nDiscussion\n\nThe result of the \
             physicochemical characterization of the nanoemulsion was as follows: droplet \
             size 156.4 nm.\n",
        );
        assert!(assess_claims(&r).is_empty(), "{:?}", assess_claims(&r));

        let real = ex(
            "Abstract\n\nA cross-sectional survey.\n\nDiscussion\n\nThe higher provision \
             rate was the result of organisational capacity.\n",
        );
        assert_eq!(assess_claims(&real).len(), 1, "{:?}", assess_claims(&real));
    }

    /// **The discriminating negative.** `Revised Health Economics Paper
    /// FINAL (1).docx` is cross-sectional and writes *"Cross-sectional design:
    /// No causal direction can be established."* A check that flagged the paper
    /// which states its own limit would punish the honest manuscript.
    #[test]
    fn a_manuscript_that_disclaims_causation_is_not_flagged_for_saying_so() {
        let report = run_on(
            "Abstract\n\nA cross-sectional survey of 412 firms.\n\nDiscussion\n\n\
             Cross-sectional design: No causal direction can be established.\n",
        );
        assert!(report.admitted.is_empty(), "{report:?}");
    }

    /// The same causal sentence in an experimental paper is earned.
    #[test]
    fn a_causal_conclusion_backed_by_an_experimental_design_is_not_a_finding() {
        let report = run_on(
            "Methods\n\nFirms were randomly assigned to a treatment group and a control \
             group.\n\nDiscussion\n\nOrganisational capacity causes higher insurance \
             provision among firms.\n",
        );
        assert!(report.admitted.is_empty(), "{report:?}");
    }

    /// Measured on `R PAPER .docx`: *"achieving state-of-the-art results in
    /// terms of 96.42% accuracy"* is the manuscript's only causal-phrase match.
    #[test]
    fn a_causal_phrase_inside_an_idiom_is_not_a_causal_claim() {
        let report = run_on(
            "Abstract\n\nA cross-sectional survey of users.\n\nDiscussion\n\nThe model \
             achieved state-of-the-art results in terms of 96.42% accuracy.\n",
        );
        assert!(report.admitted.is_empty(), "{report:?}");
    }

    /// Ran-and-found-nothing is not never-ran. A manuscript naming no design at
    /// all has no first half to the check.
    #[test]
    fn a_manuscript_with_no_stated_design_declines_rather_than_passing() {
        let report = run_on(
            "Introduction\n\nThis chapter reviews the literature.\n\nDiscussion\n\n\
             Nutrient loading causes eutrophication.\n",
        );
        assert!(report.admitted.is_empty());
        assert!(report.not_applicable.is_some(), "{report:?}");
    }

    /// **Phase 1 refused to emit `Supports` because extraction can only see
    /// position. §4.6 asks this node for `SUPPORTED` anyway.** Reading
    /// co-location as support would reverse that decision silently, so no path
    /// returns it — nor `CONTRADICTED`, which needs the same reading.
    #[test]
    fn support_and_contradiction_are_never_concluded_from_co_location() {
        let r = ex("Abstract\n\nA cross-sectional survey.\n\nDiscussion\n\nCapacity \
                    causes provision (p < 0.001, n = 412).\n");
        for a in assess_claims(&r) {
            assert!(
                !matches!(
                    a.strength,
                    EvidenceStrength::Supported | EvidenceStrength::Contradicted
                ),
                "a statistic beside a claim is proximity, not meaning: {a:?}"
            );
        }
    }
}
