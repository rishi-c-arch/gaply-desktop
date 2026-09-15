//! **The editor layer — §5.7, and what it can honestly be.**
//!
//! §5.7: *"The editor reads all reviewer reports, the disagreement map, the
//! journal fingerprint, and Tier 0/1 findings. It applies severity precedence
//! from the risk table … and produces the editorial posture (§8)."*
//!
//! # TWO OF THE FOUR DECLARED INPUTS DO NOT EXIST
//!
//! Measured against the tree, 15 Sep 2026:
//!
//! * **reviewer reports** — real, four lenses (§4.5 [v8]).
//! * **Tier 0/1 findings** — real, `validate.rs` and the equation engine.
//! * **journal fingerprint** — real, from a crawl.
//! * **the disagreement map** — **nothing produces one.** §5.3 is unbuilt and
//!   the round-table has never run past round one. [`EditorInput`] therefore has
//!   no field for it, and [`EditorialRubric::not_read`] names it in the output
//!   so a reader is not left assuming the editor weighed a disagreement.
//!
//! # WHAT SEVERITY PRECEDENCE HAS TO ORDER
//!
//! `examples/editor_input_audit.rs` over 20 real manuscripts:
//!
//! | | |
//! |---|---:|
//! | BLOCKING | **2**, on 2 of 20 manuscripts |
//! | MAJOR | 126 |
//! | MINOR | 9 |
//! | manuscripts with at least one concern | **20 of 20** |
//!
//! **Both BLOCKING findings are `SmallSampleCausalClaim` from `validate.rs`**,
//! and of the four codes mapped to BLOCKING, two can never fire:
//! `test_claimed_but_absent_from_analysis` needs an `AnalysisRecord` no upload
//! path accepts, and `PRIOR_WORK_EXISTS` is declined (§11 D166). So the rule
//! §5.7 calls the editor's job rests on two deterministic statistical rules, and
//! [`EditorialRubric::precedence_note`] says so on every report rather than
//! leaving the tier looking broader than it is.
//!
//! # THE POSTURE IS A RECOMMENDATION, AND IT IS NOT A PREDICTION
//!
//! §8: *"as a **recommendation**, never as a prediction of what the journal will
//! decide"*. [`Posture`] carries no number and there is no field a probability
//! could be written into. §8's Stage 3 — a calibrated probability — needs an
//! outcome dataset that does not exist, and
//! `the_rubric_carries_no_probability_and_no_percent` is where that stays true.
//!
//! # THE CAUSES LEAD; THE POSTURE IS A LINE BENEATH THEM
//!
//! Measured over 20 real manuscripts: **MAJOR REVISION on 18, REJECT on 2,
//! MINOR REVISION and ACCEPT on none**. That is correct — 20 of 20 carry at
//! least one major concern and the smallest count on any paper is three — and
//! it means a reader who meets the posture first has learned nothing about
//! their manuscript.
//!
//! So [`EditorialRubric::display_order`] states the order for every consumer:
//! **primary causes first, posture after**. A summary that never varies belongs
//! after the thing it summarises, and the exports inherit this rather than each
//! deciding for itself.
//!
//! **The boundary is not moved to create spread.** Redefining MAJOR against
//! MINOR so 20 papers distribute across four postures would be fitting the
//! scale to the sample. The posture becomes informative when the BLOCKING tier
//! widens, and both unreachable blocking codes are tied to declined layers
//! (§11 D165, D166) — so its usefulness is downstream of those, not of this
//! rubric. §8 [v8] carries the reasoning.
//!
//! # THE EDITOR ORDERS; IT DOES NOT RE-DECIDE
//!
//! §5.7: *"The editor is Tier 4 and overrides nothing below it."* It never
//! changes a severity, never drops a concern, and never demotes a blocking
//! finding. [`decide`] is a pure function of counts and the concerns it was
//! handed, and `the_editor_never_changes_a_severity_it_was_given` pins it.

use serde::{Deserialize, Serialize};

use crate::review_lens::{Concern, LensId, ReviewSeverity, ReviewerReport};

/// The four outcomes every journal uses, as the reviewer-criteria document
/// lists them. **A recommendation, never a prediction.**
/// `Ord` is by SEVERITY, most severe first — `Reject` < `Accept` — so a sort
/// or a `BTreeMap` orders postures the way the editor ranks them rather than
/// alphabetically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Posture {
    Reject,
    MajorRevision,
    MinorRevision,
    Accept,
}

impl Posture {
    pub fn as_str(self) -> &'static str {
        match self {
            Posture::Accept => "ACCEPT",
            Posture::MinorRevision => "MINOR REVISION",
            Posture::MajorRevision => "MAJOR REVISION",
            Posture::Reject => "REJECT",
        }
    }
}

/// One of §8's *"primary causes"*: a cause names its lens and its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimaryCause {
    pub lens: LensId,
    pub criterion: String,
    pub code: String,
    pub severity: ReviewSeverity,
    /// How many concerns of this code under this criterion.
    pub concerns: usize,
    /// Total occurrences, which is not the same number — one concern can carry
    /// 42 spans (§4.5's grouping).
    pub occurrences: usize,
    /// **What `occurrences` counts.** Causes are ordered by it, so a reader
    /// comparing two causes has to know whether they count the same thing:
    /// `StatisticalTestReported x6` is six STANDARDS asking for one property,
    /// while `MissingEffectSize x42` is 42 places in the manuscript.
    pub occurrence_unit: String,
    /// The first manuscript sentence it rests on, whole.
    pub span: Option<String>,
}

/// §8 Stage 1's rubric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorialRubric {
    pub posture: Posture,
    pub blocking: usize,
    pub major: usize,
    pub minor: usize,
    pub informational: usize,
    /// Ordered, most severe first. Never more than three — §8's example shows
    /// three and a cause list a reader scrolls past is not a cause list.
    pub primary_causes: Vec<PrimaryCause>,
    /// Why the posture is what it is, in the rubric's own terms.
    pub why: String,
    /// What the precedence tier actually rests on. Printed every time.
    pub precedence_note: String,
    /// **The order a consumer must present this in.** Measured: the posture is
    /// MAJOR REVISION on 18 of 20 real manuscripts, so leading with it tells a
    /// reader nothing. Carried on the rubric rather than left to each export,
    /// because four exports deciding separately is four chances to lead with
    /// the uninformative line.
    pub display_order: Vec<String>,
    /// §5.7 inputs the editor did NOT read, named.
    pub not_read: Vec<String>,
    /// Lenses that produced no report because they are declined.
    pub lenses_declined: Vec<String>,
}

/// Everything the editor reads. **No disagreement-map field**, because nothing
/// produces one — see the module header.
pub struct EditorInput<'a> {
    pub reports: &'a [ReviewerReport],
    /// Whether a journal fingerprint was supplied. The editor does not read its
    /// contents — the reporting lens already did — but §8's posture is a
    /// recommendation *for a journal*, and one made without a journal must say
    /// so.
    pub journal: Option<&'a str>,
}

/// **The precedence rule, stated once.**
///
/// §5.7: a fatal flaw is blocking *"regardless of how many minor strengths
/// surround it"*. So the posture is decided by the most severe tier that has
/// any member, and the counts below it cannot soften it.
pub fn decide(input: &EditorInput<'_>) -> EditorialRubric {
    let all: Vec<(&ReviewerReport, &Concern)> = input
        .reports
        .iter()
        .flat_map(|r| r.major_concerns.iter().chain(r.minor_concerns.iter()).map(move |c| (r, c)))
        .collect();

    let count = |s: ReviewSeverity| all.iter().filter(|(_, c)| c.severity == s).count();
    let blocking = count(ReviewSeverity::Blocking);
    let major = count(ReviewSeverity::Major);
    let minor = count(ReviewSeverity::Minor);
    let informational = count(ReviewSeverity::Informational);

    // **Deterministic precedence.** The most severe tier with a member decides,
    // and nothing below it can soften the result.
    let (posture, why) = if blocking > 0 {
        (
            Posture::Reject,
            format!(
                "{blocking} blocking finding(s). §5.7: a fatal flaw is blocking regardless \
                 of how many strengths surround it, so the {major} major and {minor} minor \
                 finding(s) below do not soften this and the editor cannot demote it."
            ),
        )
    } else if major > 0 {
        (
            Posture::MajorRevision,
            format!(
                "no blocking finding, and {major} major concern(s) — each one a reviewer \
                 would expect addressed before acceptance."
            ),
        )
    } else if minor > 0 {
        (
            Posture::MinorRevision,
            format!("no blocking or major concern, and {minor} minor one(s)."),
        )
    } else {
        (
            Posture::Accept,
            "no concern at any severity was raised by the lenses that ran. **This is not a \
             clean bill of health**: it is the absence of a finding from the checks that \
             ran, and every report lists the criteria it could not evaluate."
                .to_string(),
        )
    };

    // Primary causes: group by (lens, criterion, code), most severe first, then
    // by how many occurrences it carries.
    let mut groups: Vec<PrimaryCause> = Vec::new();
    for (r, c) in &all {
        match groups
            .iter_mut()
            .find(|g| g.lens == r.lens && g.criterion == c.criterion && g.code == c.code)
        {
            Some(g) => {
                g.concerns += 1;
                g.occurrences += c.occurrences;
            }
            None => groups.push(PrimaryCause {
                lens: r.lens,
                criterion: c.criterion.clone(),
                code: c.code.clone(),
                severity: c.severity,
                concerns: 1,
                occurrences: c.occurrences,
                occurrence_unit: c.occurrence_unit.clone(),
                span: c.spans.first().cloned(),
            }),
        }
    }
    groups.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then(b.occurrences.cmp(&a.occurrences))
            .then(a.code.cmp(&b.code))
    });
    groups.truncate(3);

    let mut not_read = vec![
        "the disagreement map (§5.7 names it; nothing in the tree produces one — §5.3 is \
         unbuilt and the round-table has never run past round one)"
            .to_string(),
    ];
    if input.journal.is_none() {
        not_read.push(
            "any journal's requirements — no fingerprint was supplied, so this posture is \
             not a recommendation about a journal"
                .to_string(),
        );
    }

    EditorialRubric {
        posture,
        blocking,
        major,
        minor,
        informational,
        primary_causes: groups,
        display_order: DISPLAY_ORDER.iter().map(|s| s.to_string()).collect(),
        why,
        precedence_note: PRECEDENCE_NOTE.to_string(),
        not_read,
        lenses_declined: crate::review_lens::DECLINED_LENSES
            .iter()
            .map(|d| format!("{}: {}", d.id.as_str(), d.reason))
            .collect(),
    }
}

/// **Primary causes first, posture after.** See the module header for the
/// measurement that decides this.
pub const DISPLAY_ORDER: &[&str] = &[
    "primary_causes",
    "posture",
    "counts",
    "why",
    "precedence_note",
    "not_read",
];

/// What the blocking tier rests on, printed on every rubric.
///
/// Measured over 20 manuscripts: 2 blocking findings, both
/// `SmallSampleCausalClaim`. Of four codes mapped to BLOCKING, two can never
/// fire. A reader who sees `Blocking: 0` should know how narrow that tier is
/// before reading it as reassurance.
pub const PRECEDENCE_NOTE: &str =
    "Precedence is deterministic: the most severe tier with any member decides the posture, \
     and no count below it softens the result. The BLOCKING tier is narrow — across 20 real \
     manuscripts it fired twice, both times on `SmallSampleCausalClaim` from the \
     deterministic statistical rules. Two of the four codes mapped to BLOCKING cannot fire \
     at all: one needs an uploaded analysis file, which no picker accepts, and one is the \
     declined novelty check (§11 D166). `Blocking: 0` therefore means those two rules did \
     not fire, not that nothing fatal is present.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_lens::{CriterionOutcome, CriterionState};

    fn concern(code: &str, sev: ReviewSeverity, occurrences: usize) -> Concern {
        Concern {
            criterion: "Statistical Analysis".into(),
            source: crate::review_lens::LensSource::Specialist(
                crate::review_lens::SourceId::FrequentistStats,
            ),
            code: code.into(),
            severity: sev,
            summary: "s".into(),
            occurrence_unit: "places in the manuscript".into(),
            occurrences,
            spans: vec!["the manuscript's own sentence".into()],
            locations: Vec::new(),
            trail: Vec::new(),
            uncertainty: Some("u".into()),
            required_revision: "r".into(),
        }
    }

    fn report(lens: LensId, major: Vec<Concern>, minor: Vec<Concern>) -> ReviewerReport {
        ReviewerReport {
            lens,
            overall_assessment: String::new(),
            contribution: String::new(),
            strengths: Vec::new(),
            major_concerns: major,
            minor_concerns: minor,
            journal_compliance: Vec::new(),
            required_revisions: Vec::new(),
            criteria: vec![CriterionOutcome {
                criterion: "Statistical Analysis".into(),
                state: CriterionState::Evaluated,
                concerns: 0,
            }],
            not_applicable: None,
            absence_rejected: Vec::new(),
            policy_rejected: Vec::new(),
        }
    }

    fn decide_on(reports: &[ReviewerReport]) -> EditorialRubric {
        decide(&EditorInput { reports, journal: Some("nature-medicine") })
    }

    /// **A blocking finding outranks any number of strengths** — §5.7's rule,
    /// and the one thing the editor exists to do.
    #[test]
    fn one_blocking_finding_outranks_a_hundred_major_ones() {
        let many: Vec<Concern> =
            (0..100).map(|_| concern("MissingEffectSize", ReviewSeverity::Major, 1)).collect();
        let mut with_block = many.clone();
        with_block.push(concern("SmallSampleCausalClaim", ReviewSeverity::Blocking, 1));

        let without = decide_on(&[report(LensId::Statistics, many, vec![])]);
        assert_eq!(without.posture, Posture::MajorRevision);

        let with = decide_on(&[report(LensId::Statistics, with_block, vec![])]);
        assert_eq!(with.posture, Posture::Reject, "{:?}", with.why);
        assert!(with.why.contains("cannot demote"), "{}", with.why);
    }

    /// **The editor orders; it does not re-decide.** §5.7: Tier 4 overrides
    /// nothing below it.
    #[test]
    fn the_editor_never_changes_a_severity_it_was_given() {
        let given = vec![
            concern("MissingEffectSize", ReviewSeverity::Major, 42),
            concern("marginal_significance_language", ReviewSeverity::Minor, 1),
        ];
        let r = decide_on(&[report(LensId::Statistics, given.clone(), vec![])]);
        for c in &r.primary_causes {
            let original = given.iter().find(|g| g.code == c.code).unwrap();
            assert_eq!(c.severity, original.severity, "the editor re-decided {}", c.code);
        }
        // Both were handed in as `major_concerns`; the editor counts by the
        // severity ON the concern, not by which list it arrived in.
        assert_eq!(r.major, 1);
        assert_eq!(r.minor, 1);
    }

    /// **No probability, ever.** §8 Stage 3 needs an outcome dataset that does
    /// not exist, and this is the test that blocks it.
    #[test]
    fn the_rubric_carries_no_probability_and_no_percent() {
        let r = decide_on(&[report(
            LensId::Statistics,
            vec![concern("MissingEffectSize", ReviewSeverity::Major, 3)],
            vec![],
        )]);
        let json = serde_json::to_string(&r).unwrap();
        for banned in ["probability", "likelihood", "percent", "chance of", "odds"] {
            assert!(!json.to_lowercase().contains(banned), "rubric mentions {banned}: {json}");
        }
        // A bare `%` would be a percentage of something.
        assert!(!r.why.contains('%'), "{}", r.why);
    }

    /// **`Accept` is not a clean bill of health**, and must say so — every
    /// lens lists criteria it could not evaluate.
    #[test]
    fn an_accept_posture_says_what_it_is_not() {
        let r = decide_on(&[report(LensId::Statistics, vec![], vec![])]);
        assert_eq!(r.posture, Posture::Accept);
        assert!(r.why.contains("not a clean bill of health"), "{}", r.why);
    }

    /// **The narrowness of the blocking tier travels with every rubric.**
    /// `Blocking: 0` read as "nothing fatal" is the failure this prevents.
    #[test]
    fn every_rubric_states_what_the_blocking_tier_rests_on() {
        let r = decide_on(&[report(LensId::Statistics, vec![], vec![])]);
        assert_eq!(r.blocking, 0);
        assert!(r.precedence_note.contains("cannot fire at all"), "{}", r.precedence_note);
        assert!(
            r.precedence_note.contains("not that nothing fatal is present"),
            "{}",
            r.precedence_note
        );
    }

    /// **An input §5.7 declares and nothing produces is named in the output.**
    #[test]
    fn the_rubric_names_the_disagreement_map_as_unread() {
        let r = decide_on(&[report(LensId::Statistics, vec![], vec![])]);
        assert!(
            r.not_read.iter().any(|x| x.contains("disagreement map")),
            "{:?}",
            r.not_read
        );
    }

    /// A posture with no journal is not a recommendation about a journal.
    #[test]
    fn a_posture_without_a_journal_says_it_is_not_about_one() {
        let reports = [report(LensId::Statistics, vec![], vec![])];
        let r = decide(&EditorInput { reports: &reports, journal: None });
        assert!(
            r.not_read.iter().any(|x| x.contains("not a recommendation about a journal")),
            "{:?}",
            r.not_read
        );
    }

    /// **The causes lead and the posture follows.** Measured: MAJOR REVISION on
    /// 18 of 20 real manuscripts, so a report leading with the posture opens
    /// with the same sentence for almost every researcher. Four exports each
    /// choosing an order is four chances to get this wrong, so the rubric
    /// carries it.
    #[test]
    fn the_rubric_says_the_causes_come_before_the_posture() {
        let r = decide_on(&[report(
            LensId::Statistics,
            vec![concern("MissingEffectSize", ReviewSeverity::Major, 42)],
            vec![],
        )]);
        let causes = r.display_order.iter().position(|x| x == "primary_causes").unwrap();
        let posture = r.display_order.iter().position(|x| x == "posture").unwrap();
        assert!(
            causes < posture,
            "a summary that never varies belongs after the thing it summarises: {:?}",
            r.display_order
        );
    }

    /// **`MinorRevision` is reachable**, and nothing in the 20-manuscript
    /// corpus reached it. The distinction is the one §8 [v8] rests on: the
    /// posture is uninformative because of the CORPUS, not because the rule
    /// cannot return the other values.
    #[test]
    fn minor_revision_and_accept_are_both_reachable() {
        let minor_only = decide_on(&[report(
            LensId::Statistics,
            vec![],
            vec![concern("marginal_significance_language", ReviewSeverity::Minor, 1)],
        )]);
        assert_eq!(minor_only.posture, Posture::MinorRevision, "{:?}", minor_only.why);

        let clean = decide_on(&[report(LensId::Statistics, vec![], vec![])]);
        assert_eq!(clean.posture, Posture::Accept);
    }

    /// **A cause states what its count counts** (§11 D161).
    ///
    /// Measured: on 11 of 20 real manuscripts the top cause was
    /// `StatisticalTestReported x6` — six bound standards each asking for a
    /// named statistical test, which is ONE manuscript property asked six
    /// times. Ordering causes by a bare occurrence count put that ahead of
    /// `MissingEffectSize x42`, which is 42 distinct places. The counts are
    /// still comparable only if the reader knows they count different things,
    /// so every cause carries its unit.
    #[test]
    fn a_cause_says_what_its_occurrence_count_counts() {
        use crate::review_lens::{occurrence_unit, LensSource, SourceId, StateField};
        assert!(
            occurrence_unit(LensSource::Specialist(SourceId::ReportingStandards))
                .contains("not places in the manuscript"),
            "a standards count is per STANDARD, and must say so"
        );
        assert_eq!(
            occurrence_unit(LensSource::Specialist(SourceId::FrequentistStats)),
            "places in the manuscript"
        );
        assert!(occurrence_unit(LensSource::State(StateField::FullText)).contains("phrase"));

        let mut c = concern("MissingEffectSize", ReviewSeverity::Major, 42);
        c.occurrence_unit = "places in the manuscript".into();
        let r = decide_on(&[report(LensId::Statistics, vec![c], vec![])]);
        assert_eq!(r.primary_causes[0].occurrences, 42);
        assert_eq!(r.primary_causes[0].occurrence_unit, "places in the manuscript");
    }

    /// **The blocking tier's reachable count is two, not four**, and the
    /// constant that says so must stay in step with the policy table.
    #[test]
    fn exactly_two_blocking_codes_are_reachable() {
        use crate::review_lens::{severity_rule, REACHABLE_BLOCKING_CODES, SEVERITY_POLICY};
        let blocking: Vec<&str> = SEVERITY_POLICY
            .iter()
            .filter(|r| r.severity == ReviewSeverity::Blocking)
            .map(|r| r.code)
            .collect();
        assert_eq!(blocking.len(), 4, "the table maps four codes to BLOCKING: {blocking:?}");
        assert_eq!(REACHABLE_BLOCKING_CODES.len(), 2);
        for c in REACHABLE_BLOCKING_CODES {
            assert!(blocking.contains(c), "`{c}` is listed reachable but is not BLOCKING");
            assert_eq!(severity_rule(c).unwrap().severity, ReviewSeverity::Blocking);
        }
        // The two unreachable ones are named, so neither can quietly become
        // reachable without this test noticing.
        let unreachable: Vec<&&str> =
            blocking.iter().filter(|c| !REACHABLE_BLOCKING_CODES.contains(c)).collect();
        assert_eq!(
            unreachable.len(),
            2,
            "one needs an upload path, one is declined (§11 D166): {unreachable:?}"
        );
    }

    /// Causes are ordered most severe first, and a cause carrying 42
    /// occurrences outranks one carrying 1 at the same severity.
    #[test]
    fn causes_are_ordered_by_severity_then_by_weight() {
        let r = decide_on(&[report(
            LensId::Statistics,
            vec![
                concern("MissingEffectSize", ReviewSeverity::Major, 3),
                concern("MissingConfidenceInterval", ReviewSeverity::Major, 42),
                concern("TestGroupMismatch", ReviewSeverity::Blocking, 1),
            ],
            vec![],
        )]);
        assert_eq!(r.primary_causes[0].code, "TestGroupMismatch", "{:?}", r.primary_causes);
        assert_eq!(r.primary_causes[1].code, "MissingConfidenceInterval");
        assert_eq!(r.primary_causes[1].occurrences, 42);
        assert!(r.primary_causes.len() <= 3);
    }
}
