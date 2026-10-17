//! **The Level 0 outcome for one manuscript: the verdict rule per claim, and
//! coverage as counts.**
//!
//! # The verdict rule
//!
//! [`claim_verdict`] is the one place a claim's verdict is decided. It is
//! deterministic and total over every [`CheckOutcome`], in this order:
//!
//! 1. `Inconsistent` if any check is `Inconsistent`.
//! 2. Else `Concern` if any check is `Concern`.
//! 3. Else `Consistent` if at least one check is `Consistent`.
//! 4. Else `CouldNotCheck`, with every reason.
//!
//! A method that was `NotApplicable`, and a check that came back
//! `CouldNotCheck` (`Uninformative` among them), become `CouldNotCheck`
//! reasons. Neither ever counts toward `Consistent`: a claim no method could
//! judge is not a claim that passed.
//!
//! # Coverage is counts
//!
//! [`StatsConsistencyOutcome`] holds how many results were read, how many
//! attempts were not read and why, and how many claims got each verdict.
//! **No ratio, percentage or score is computed or stored here.** A coverage
//! fraction invites being read as a grade, and its denominator (attempts the
//! reader recognised) is not the number of results a paper reports.
//!
//! Deliberately NOT `Serialize`: every claim keeps its printed unit.

use super::input::{DocumentContext, L0Input};
use super::method::{
    Applicability, CheckNote, CouldNotCheckReason, L0Verdict, MethodCheck, NotApplicableReason,
};
use super::registry::MethodRegistry;
use super::result::{NotRead, Reason, ReportedResult, ResultReading};
use crate::equation::rational::Rational;
use crate::extract::ExtractionResult;

/// What one registered method did with one claim.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckOutcome {
    Ran(MethodCheck),
    /// The method does not judge this input and was not run.
    NotApplicable { method_id: &'static str, reason: NotApplicableReason },
}

impl CheckOutcome {
    pub fn method_id(&self) -> &'static str {
        match self {
            CheckOutcome::Ran(c) => c.method_id,
            CheckOutcome::NotApplicable { method_id, .. } => method_id,
        }
    }
}

/// One claim, every check run on it in registry order, and its verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimOutcome {
    pub result: ReportedResult,
    pub checks: Vec<CheckOutcome>,
    /// From [`claim_verdict`]. A `CouldNotCheck` here holds the FIRST of
    /// `could_not_check`.
    pub verdict: L0Verdict,
    /// Every reason, in registry order, when `verdict` is `CouldNotCheck`;
    /// empty otherwise.
    pub could_not_check: Vec<CouldNotCheckReason>,
}

/// How many claims got each verdict. Counts only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VerdictCounts {
    pub consistent: usize,
    pub inconsistent: usize,
    pub concern: usize,
    pub could_not_check: usize,
}

impl VerdictCounts {
    fn add(&mut self, v: &L0Verdict) {
        match v {
            L0Verdict::Consistent(_) => self.consistent += 1,
            L0Verdict::Inconsistent { .. } => self.inconsistent += 1,
            L0Verdict::Concern(_) => self.concern += 1,
            L0Verdict::CouldNotCheck(_) => self.could_not_check += 1,
        }
    }
    pub fn total(&self) -> usize {
        self.consistent + self.inconsistent + self.concern + self.could_not_check
    }
}

/// Level 0 for one manuscript.
#[derive(Debug, Clone, PartialEq)]
pub struct StatsConsistencyOutcome {
    /// Results read: one per claim in `claims`.
    pub claims_read: usize,
    /// Attempts not read, by reason, in the order each reason first occurs.
    pub not_read_by_reason: Vec<(Reason, usize)>,
    /// Every attempt not read, as the reader recorded it: a location, a reason
    /// and a byte span. No text; a labeller resolves the span against the
    /// document.
    pub not_read: Vec<NotRead>,
    pub verdict_counts: VerdictCounts,
    /// Every claim read, in document order.
    pub claims: Vec<ClaimOutcome>,
    /// What `p_consistency` judged against: [`DocumentContext::alpha`].
    pub alpha_used: Vec<Rational>,
    /// `alpha_used` as the paper printed each value, in the same order:
    /// [`DocumentContext::alpha_printed`].
    pub alpha_printed: Vec<String>,
    /// No threshold is stated, so `alpha_used` is the default .05.
    pub alpha_defaulted: bool,
}

/// The verdict rule. See the module docs.
pub fn claim_verdict(checks: &[CheckOutcome]) -> (L0Verdict, Vec<CouldNotCheckReason>) {
    let ran = || {
        checks.iter().filter_map(|c| match c {
            CheckOutcome::Ran(m) => Some(m.outcome),
            CheckOutcome::NotApplicable { .. } => None,
        })
    };

    // Only p_consistency assesses whether the conclusion changes; another
    // method's `None` must not erase its answer.
    let mut inconsistent: Option<Option<bool>> = None;
    for v in ran() {
        if let L0Verdict::Inconsistent { conclusion_changes } = v {
            inconsistent = Some(match (inconsistent.flatten(), conclusion_changes) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), _) | (_, Some(false)) => Some(false),
                (None, None) => None,
            });
        }
    }
    if let Some(conclusion_changes) = inconsistent {
        return (L0Verdict::Inconsistent { conclusion_changes }, Vec::new());
    }
    if let Some(v) = ran().find(|v| matches!(v, L0Verdict::Concern(_))) {
        return (v, Vec::new());
    }
    if ran().any(|v| matches!(v, L0Verdict::Consistent(_))) {
        // The first note, in registry order: a Consistent that rests on a
        // reading the manuscript states is reported as resting on it.
        let note: Option<CheckNote> = ran().find_map(|v| match v {
            L0Verdict::Consistent(n) => n,
            _ => None,
        });
        return (L0Verdict::Consistent(note), Vec::new());
    }

    let mut reasons: Vec<CouldNotCheckReason> = checks
        .iter()
        .filter_map(|c| match c {
            CheckOutcome::Ran(m) => match m.outcome {
                L0Verdict::CouldNotCheck(r) => Some(r),
                _ => None,
            },
            CheckOutcome::NotApplicable { reason, .. } => Some(CouldNotCheckReason::NotApplicable(*reason)),
        })
        .collect();
    if reasons.is_empty() {
        reasons.push(CouldNotCheckReason::NoMethodRan);
    }
    (L0Verdict::CouldNotCheck(reasons[0]), reasons)
}

/// Run every method in [`MethodRegistry::LEVEL_0`] on every result read, and
/// count. `ex` is the document `reading` was read from.
pub fn check_reading(ex: &ExtractionResult, reading: ResultReading) -> StatsConsistencyOutcome {
    let context = DocumentContext::read(ex, &reading);
    let (alpha_used, alpha_defaulted) = context.alpha();
    let alpha_printed = context.alpha_printed();

    let ResultReading { results, not_read, .. } = reading;
    let mut not_read_by_reason: Vec<(Reason, usize)> = Vec::new();
    for n in &not_read {
        match not_read_by_reason.iter_mut().find(|(r, _)| *r == n.reason) {
            Some((_, count)) => *count += 1,
            None => not_read_by_reason.push((n.reason, 1)),
        }
    }

    let mut verdict_counts = VerdictCounts::default();
    let claims: Vec<ClaimOutcome> = results
        .into_iter()
        .map(|result| {
            let input = L0Input { result: &result, context: &context };
            let checks: Vec<CheckOutcome> = MethodRegistry::LEVEL_0
                .iter()
                .map(|m| match m.method.applicability(&input) {
                    Applicability::Applicable => CheckOutcome::Ran(m.method.run(&input)),
                    Applicability::NotApplicable(reason) => {
                        CheckOutcome::NotApplicable { method_id: m.method.id(), reason }
                    }
                })
                .collect();
            let (verdict, could_not_check) = claim_verdict(&checks);
            verdict_counts.add(&verdict);
            ClaimOutcome { result, checks, verdict, could_not_check }
        })
        .collect();

    StatsConsistencyOutcome {
        claims_read: claims.len(),
        not_read_by_reason,
        not_read,
        verdict_counts,
        claims,
        alpha_used,
        alpha_printed,
        alpha_defaulted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l0::method::{ConcernReason, Provenance, ToleranceRule};

    fn ran(outcome: L0Verdict) -> CheckOutcome {
        CheckOutcome::Ran(MethodCheck {
            method_id: "m",
            provenance: Provenance::GaplyArithmetic,
            engine_version: "test",
            inputs_digest: String::new(),
            computed: Vec::new(),
            tolerance: ToleranceRule::RoundingOnly,
            outcome,
        })
    }
    fn na(reason: NotApplicableReason) -> CheckOutcome {
        CheckOutcome::NotApplicable { method_id: "m", reason }
    }

    const OK: L0Verdict = L0Verdict::Consistent(None);
    const BAD: L0Verdict = L0Verdict::Inconsistent { conclusion_changes: None };
    const CONCERN: L0Verdict = L0Verdict::Concern(ConcernReason::OneTailedUnstated);
    const INVALID: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::InvalidInput);
    const UNINFORMATIVE: L0Verdict = L0Verdict::CouldNotCheck(CouldNotCheckReason::Uninformative);

    #[test]
    fn the_rule_orders_inconsistent_then_concern_then_consistent() {
        assert_eq!(claim_verdict(&[ran(OK), ran(CONCERN), ran(BAD)]).0, BAD);
        assert_eq!(claim_verdict(&[ran(OK), ran(CONCERN), ran(INVALID)]).0, CONCERN);
        assert_eq!(claim_verdict(&[ran(INVALID), ran(OK), na(NotApplicableReason::DfNotFixedByN)]), (OK, vec![]));
        // An Inconsistent outranks a Consistent that came first.
        assert_eq!(claim_verdict(&[ran(OK), ran(BAD)]).0, BAD);
    }

    #[test]
    fn not_applicable_and_uninformative_never_count_toward_consistent() {
        // Every method not applicable: the case the evaluation's mutation M2
        // showed this test did not reach (the list below mixes in a
        // `CouldNotCheck`, which a rule that passed all-NotApplicable lists
        // as Consistent would still refuse).
        let (v, reasons) = claim_verdict(&[
            na(NotApplicableReason::NoEstimateAndInterval),
            na(NotApplicableReason::DfNotFixedByN),
            na(NotApplicableReason::OtherInputKind),
        ]);
        assert_eq!(v, L0Verdict::CouldNotCheck(CouldNotCheckReason::NotApplicable(NotApplicableReason::NoEstimateAndInterval)));
        assert_eq!(reasons.len(), 3);
        let (v, reasons) = claim_verdict(&[
            na(NotApplicableReason::NoEstimateAndInterval),
            ran(UNINFORMATIVE),
            na(NotApplicableReason::DfNotFixedByN),
        ]);
        assert_eq!(v, L0Verdict::CouldNotCheck(CouldNotCheckReason::NotApplicable(NotApplicableReason::NoEstimateAndInterval)));
        assert_eq!(
            reasons,
            vec![
                CouldNotCheckReason::NotApplicable(NotApplicableReason::NoEstimateAndInterval),
                CouldNotCheckReason::Uninformative,
                CouldNotCheckReason::NotApplicable(NotApplicableReason::DfNotFixedByN),
            ]
        );
    }

    #[test]
    fn the_rule_is_total_on_an_empty_check_list() {
        assert_eq!(
            claim_verdict(&[]),
            (L0Verdict::CouldNotCheck(CouldNotCheckReason::NoMethodRan), vec![CouldNotCheckReason::NoMethodRan])
        );
    }

    #[test]
    fn a_consistent_keeps_the_first_note_it_rests_on() {
        let stated = L0Verdict::Consistent(Some(CheckNote::OneTailedStated));
        assert_eq!(claim_verdict(&[ran(OK), ran(stated)]).0, stated);
    }

    #[test]
    fn conclusion_changes_is_not_erased_by_a_method_that_does_not_assess_it() {
        let changes = L0Verdict::Inconsistent { conclusion_changes: Some(true) };
        let stays = L0Verdict::Inconsistent { conclusion_changes: Some(false) };
        assert_eq!(claim_verdict(&[ran(changes), ran(BAD)]).0, changes);
        assert_eq!(claim_verdict(&[ran(BAD), ran(stays)]).0, stays);
        assert_eq!(claim_verdict(&[ran(stays), ran(changes)]).0, changes);
        assert_eq!(claim_verdict(&[ran(BAD), ran(BAD)]).0, BAD);
    }

    /// Every verdict the rule can return, over every combination of the
    /// outcome kinds a check can have, is the one the stated order gives.
    #[test]
    fn the_rule_agrees_with_its_statement_over_every_pair_and_triple() {
        let kinds = [ran(OK), ran(BAD), ran(CONCERN), ran(INVALID), ran(UNINFORMATIVE), na(NotApplicableReason::OtherInputKind)];
        let mut combos: Vec<Vec<CheckOutcome>> = Vec::new();
        for a in &kinds {
            for b in &kinds {
                combos.push(vec![a.clone(), b.clone()]);
                for c in &kinds {
                    combos.push(vec![a.clone(), b.clone(), c.clone()]);
                }
            }
        }
        assert_eq!(combos.len(), 36 + 216);
        for checks in &combos {
            let outs: Vec<L0Verdict> = checks
                .iter()
                .filter_map(|c| if let CheckOutcome::Ran(m) = c { Some(m.outcome) } else { None })
                .collect();
            let (v, reasons) = claim_verdict(checks);
            let expected_kind = if outs.iter().any(|o| matches!(o, L0Verdict::Inconsistent { .. })) {
                "inconsistent"
            } else if outs.iter().any(|o| matches!(o, L0Verdict::Concern(_))) {
                "concern"
            } else if outs.iter().any(|o| matches!(o, L0Verdict::Consistent(_))) {
                "consistent"
            } else {
                "could_not_check"
            };
            let kind = match v {
                L0Verdict::Inconsistent { .. } => "inconsistent",
                L0Verdict::Concern(_) => "concern",
                L0Verdict::Consistent(_) => "consistent",
                L0Verdict::CouldNotCheck(r) => {
                    assert_eq!(Some(&r), reasons.first(), "{checks:?}");
                    assert_eq!(reasons.len(), checks.len(), "one reason per check: {checks:?}");
                    "could_not_check"
                }
            };
            assert_eq!(kind, expected_kind, "{checks:?}");
            assert_eq!(reasons.is_empty(), kind != "could_not_check", "{checks:?}");
        }
    }

    fn outcome_of(text: &str) -> StatsConsistencyOutcome {
        use crate::stats_l0::result::{read_results, TableStructure};
        let ex = crate::extract::extract_from_text(text);
        let reading = read_results(&ex, TableStructure::NotAvailable);
        check_reading(&ex, reading)
    }

    #[test]
    fn counts_add_up_and_coverage_is_counts_only() {
        let o = outcome_of(
            "A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045. Scores fell, t(28) = 2.10, p = .001. \
             A third, t = 3.1, p = .004. Alone p = .03 here.\n",
        );
        assert_eq!(o.claims_read, 2, "{:?}", o.claims.iter().map(|c| &c.result.raw).collect::<Vec<_>>());
        assert_eq!(o.claims_read, o.claims.len());
        assert_eq!(o.verdict_counts.total(), o.claims_read);
        assert_eq!(o.verdict_counts.consistent, 1);
        assert_eq!(o.verdict_counts.inconsistent, 1);
        let not_read: usize = o.not_read_by_reason.iter().map(|(_, n)| n).sum();
        assert_eq!(
            o.not_read_by_reason,
            vec![(Reason::MissingDegreesOfFreedom, 1), (Reason::PWithoutStatistic, 1)],
            "{not_read}"
        );
        assert_eq!(o.not_read.len(), not_read, "the list and the counts describe the same attempts");
        assert!(o.alpha_defaulted);
        assert_eq!(o.alpha_used, vec![Rational::new(5, 100).unwrap()]);
        // Every claim ran every registered method, in registry order.
        for c in &o.claims {
            let ids: Vec<&str> = c.checks.iter().map(CheckOutcome::method_id).collect();
            let registry: Vec<&str> = MethodRegistry::LEVEL_0.iter().map(|m| m.method.id()).collect();
            assert_eq!(ids, registry);
        }
    }

    #[test]
    fn a_stated_alpha_is_the_one_used_and_is_not_defaulted() {
        let o = outcome_of(
            "A Title\n\nMethods\nSignificance was set at p < .01.\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n",
        );
        assert!(!o.alpha_defaulted);
        assert_eq!(o.alpha_used, vec![Rational::new(1, 100).unwrap()]);
    }

    #[test]
    fn a_document_with_no_result_has_zero_claims_and_zero_verdicts() {
        let o = outcome_of("A Title\n\nResults\nNothing was tested here.\n");
        assert_eq!(o.claims_read, 0);
        assert_eq!(o.verdict_counts, VerdictCounts::default());
        assert!(o.not_read_by_reason.is_empty());
    }
}
