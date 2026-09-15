//! **§9's marked-up manuscript and four exports, scoped to what can be located.**
//!
//! # THE SCOPE IS A MEASUREMENT, NOT A CHOICE
//!
//! §9 opens *"every finding carries a locator into the manuscript"*. Measured
//! over 20 real manuscripts and 137 findings (`examples/anchor_audit.rs`):
//!
//! | | findings | share |
//! |---|---:|---:|
//! | no `Location` at all | **104** | **75.9%** |
//! | resolves, kind repeated (ambiguous) | 5 | 3.6% |
//! | **anchored** | **28** | **20.4%** |
//!
//! **Every finding code is either 100% located or 0% located, and the dividing
//! line is PRESENCE against ABSENCE.** A finding that says *"no data
//! availability statement was found anywhere in the manuscript"* has no
//! coordinates, and no improvement to `Location` gives it any. So:
//!
//! * [`AnnotatedManuscript`] carries **only** findings that anchor. It is a
//!   smaller artefact than §9 describes and a well-defined one.
//! * Absence findings go to the letter and the rubric, where a document-wide
//!   sentence reads correctly. Dropping them would lose 76% of the findings;
//!   giving them an arbitrary margin position would invent a location, which is
//!   the refusal §9 [v5] already made about `.docx` page geometry.
//! * [`ManuscriptAnnotations::unanchored`] carries the count and the reason, so
//!   a reader of the marked-up view is never left thinking it is everything.
//!
//! # FOUR EXPORTS, AND D92'S REFUSAL STANDS
//!
//! [`ExportKind`] is a closed four-variant enum: the annotated manuscript, the
//! reviewer letter, the readiness rubric, the audit trail. §9: *"Everything
//! else is a section of one of these."*
//!
//! **There is no tracked-changes `.docx`.** D92 declined it — page geometry is
//! not in the file, and *"a reconstruction that looks subtly wrong to the person
//! who wrote the paper is worse than no reconstruction."* §9 [v5] records that
//! v4 listed it anyway and gave no argument against D92's reason. It is not in
//! scope unless that argument is made and recorded, and
//! `there_is_no_tracked_changes_docx_export` is where an attempt to add one
//! meets the requirement to make it.
//!
//! # DISPLAY ORDER IS THE EDITOR'S, NOT EACH EXPORT'S
//!
//! Every export that shows the rubric follows [`crate::editor::DISPLAY_ORDER`]:
//! **primary causes lead, posture beneath**. Measured, the posture is MAJOR
//! REVISION on 18 of 20 real manuscripts (§8 [v8]), so an export leading with
//! it opens with the same sentence for almost every researcher. Four exports
//! each choosing an order is four chances to get that wrong.

use serde::{Deserialize, Serialize};

use crate::editor::EditorialRubric;
use crate::extract::{paragraph_at, ExtractionResult, Location};
use crate::review_lens::{Concern, ReviewSeverity, ReviewerReport};

/// §9's four. A closed enum: adding a fifth is a decision, not an addition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    AnnotatedManuscript,
    ReviewerLetter,
    ReadinessRubric,
    AuditTrail,
}

impl ExportKind {
    pub const ALL: [ExportKind; 4] = [
        ExportKind::AnnotatedManuscript,
        ExportKind::ReviewerLetter,
        ExportKind::ReadinessRubric,
        ExportKind::AuditTrail,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ExportKind::AnnotatedManuscript => "annotated_manuscript",
            ExportKind::ReviewerLetter => "reviewer_letter",
            ExportKind::ReadinessRubric => "readiness_rubric",
            ExportKind::AuditTrail => "audit_trail",
        }
    }
}

/// One margin note at a place in the manuscript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    pub location: Location,
    /// The manuscript's own paragraph at that location, whole. Carried so the
    /// note can be shown beside the text it is about without a second lookup,
    /// and so a reader can see the anchor landed where they expect.
    pub anchor_text: String,
    pub severity: ReviewSeverity,
    pub criterion: String,
    pub code: String,
    pub summary: String,
    /// **§9's verification trail**, verbatim from the concern.
    pub how_to_check: Vec<String>,
    pub uncertainty: Option<String>,
}

/// A finding that could not be placed, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unanchored {
    pub code: String,
    pub criterion: String,
    pub severity: ReviewSeverity,
    pub reason: String,
}

/// §9's marked-up manuscript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManuscriptAnnotations {
    pub annotations: Vec<Annotation>,
    /// **Never omitted.** A marked-up view showing 28 of 137 findings and not
    /// saying so reads as the whole review.
    pub unanchored: Vec<Unanchored>,
    /// One sentence a renderer must show. Measured, not phrased for comfort.
    pub coverage_note: String,
}

/// Why a finding has no anchor. Two reasons, and they are not the same.
const NO_LOCATION: &str =
    "this finding is about something ABSENT from the manuscript, and an absence has no \
     coordinates — it is reported in the reviewer letter, where a document-wide statement \
     reads correctly";
const AMBIGUOUS_LOCATION: &str =
    "this finding has a location that resolves, but the manuscript has more than one \
     section of that kind, so the anchor could silently be the wrong one — refused rather \
     than placed";

/// **Build the marked-up manuscript.** Pure: no model, no network.
pub fn annotate(ex: &ExtractionResult, reports: &[ReviewerReport]) -> ManuscriptAnnotations {
    let mut counts = std::collections::BTreeMap::new();
    for s in &ex.sections {
        *counts.entry(s.kind).or_insert(0usize) += 1;
    }

    let mut annotations = Vec::new();
    let mut unanchored = Vec::new();

    for r in reports {
        for c in r.major_concerns.iter().chain(r.minor_concerns.iter()) {
            let mut placed = false;
            for loc in &c.locations {
                // Repeated section kind: refuse. This is the `paragraph_at`
                // ambiguity defect, and placing anyway would commit it here.
                if counts.get(&loc.section).copied().unwrap_or(0) > 1 {
                    continue;
                }
                let Some(text) = paragraph_at(ex, loc) else { continue };
                annotations.push(Annotation {
                    location: loc.clone(),
                    anchor_text: text.to_string(),
                    severity: c.severity,
                    criterion: c.criterion.clone(),
                    code: c.code.clone(),
                    summary: c.summary.clone(),
                    how_to_check: verification_trail(c),
                    uncertainty: c.uncertainty.clone(),
                });
                placed = true;
            }
            if !placed {
                unanchored.push(Unanchored {
                    code: c.code.clone(),
                    criterion: c.criterion.clone(),
                    severity: c.severity,
                    reason: if c.locations.is_empty() { NO_LOCATION } else { AMBIGUOUS_LOCATION }.to_string(),
                });
            }
        }
    }

    // Most severe first, then in document order.
    annotations.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then(format!("{:?}", a.location.section).cmp(&format!("{:?}", b.location.section)))
            .then(a.location.paragraph.cmp(&b.location.paragraph))
    });

    let total = annotations.len() + unanchored.len();
    let coverage_note = format!(
        "This view shows {} of {total} finding(s) — the ones that can be placed in the \
         manuscript. The other {} are about something the manuscript does NOT contain, or \
         about a section that appears more than once; they are in the reviewer letter and \
         the rubric. **An absence has no coordinates**, so a marked-up page can never be \
         the whole review. Measured across 20 real manuscripts, 20.4% of findings anchor.",
        annotations.len(),
        unanchored.len()
    );

    ManuscriptAnnotations { annotations, unanchored, coverage_note }
}

/// §9: *"Every major finding states how a reviewer could check it
/// independently."* Taken from the concern's own trail rather than re-derived,
/// so the letter and the page cannot disagree about how to verify a finding.
fn verification_trail(c: &Concern) -> Vec<String> {
    c.trail.iter().map(|t| format!("{}: {}", t.stage, t.detail)).collect()
}

/// The reviewer letter — §9's second export.
///
/// Carries EVERY concern, anchored or not, because a letter is document-wide by
/// nature and is where the 76% that cannot be placed belong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewerLetter {
    /// Per lens, in the order `lenses()` returns them.
    pub sections: Vec<LetterSection>,
    /// The editor's rubric, rendered in [`crate::editor::DISPLAY_ORDER`].
    pub rubric: EditorialRubric,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LetterSection {
    pub lens: String,
    pub overall_assessment: String,
    pub contribution: String,
    pub strengths: Vec<String>,
    pub major_concerns: Vec<Concern>,
    pub minor_concerns: Vec<Concern>,
    pub journal_compliance: Vec<String>,
    pub required_revisions: Vec<String>,
    /// What this lens could not evaluate. §4.5's addition, carried through: a
    /// letter listing only concerns reads as a complete review.
    pub not_evaluated: Vec<String>,
}

pub fn letter(reports: &[ReviewerReport], rubric: &EditorialRubric) -> ReviewerLetter {
    ReviewerLetter {
        sections: reports
            .iter()
            .map(|r| LetterSection {
                lens: r.lens.as_str().to_string(),
                overall_assessment: r.overall_assessment.clone(),
                contribution: r.contribution.clone(),
                strengths: r.strengths.clone(),
                major_concerns: r.major_concerns.clone(),
                minor_concerns: r.minor_concerns.clone(),
                journal_compliance: r.journal_compliance.clone(),
                required_revisions: r.required_revisions.clone(),
                not_evaluated: r
                    .criteria
                    .iter()
                    .filter(|c| !matches!(c.state, crate::review_lens::CriterionState::Evaluated))
                    .map(|c| format!("{}: {:?}", c.criterion, c.state))
                    .collect(),
            })
            .collect(),
        rubric: rubric.clone(),
    }
}

/// The audit trail — §9's fourth export. *"JSON: every finding, its layers,
/// versions, evidence and status."*
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTrail {
    pub schema_version: u32,
    pub reports: Vec<ReviewerReport>,
    pub rubric: EditorialRubric,
    pub annotations: ManuscriptAnnotations,
    /// Everything the run did NOT read or could not decide, gathered from the
    /// reports so the trail is auditable without re-deriving it.
    pub not_read: Vec<String>,
}

pub const AUDIT_TRAIL_SCHEMA: u32 = 1;

pub fn audit_trail(
    reports: &[ReviewerReport],
    rubric: &EditorialRubric,
    annotations: &ManuscriptAnnotations,
) -> AuditTrail {
    let mut not_read = rubric.not_read.clone();
    for r in reports {
        for a in &r.absence_rejected {
            not_read.push(format!("{} / {}: {}", a.criterion, a.code, a.reason));
        }
        for p in &r.policy_rejected {
            not_read.push(p.clone());
        }
        if let Some(na) = &r.not_applicable {
            not_read.push(format!("{}: {na}", r.lens.as_str()));
        }
    }
    AuditTrail {
        schema_version: AUDIT_TRAIL_SCHEMA,
        reports: reports.to_vec(),
        rubric: rubric.clone(),
        annotations: annotations.clone(),
        not_read,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{decide, EditorInput};
    use crate::review_lens::{lenses, review, LensInput};
    use crate::specialist::{self, SpecialistInput};

    const PAPER: &str = "A Title\n\n1. Background\n\nEmployer mandates matter.\n\n\
                         5. Results\n\nProvision was 36.5% (t = 2.1, p = 0.04).\n";

    fn run() -> (ExtractionResult, Vec<ReviewerReport>, EditorialRubric) {
        let ex = crate::extract::extract_from_text(PAPER);
        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let specs: Vec<_> =
            specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &sin)).collect();
        let validity = crate::validate::validate(&ex);
        let standards = vec![crate::report::evaluate(
            crate::journal_standards::Standard::Strobe,
            &ex,
            PAPER,
        )];
        let input = LensInput {
            extraction: &ex,
            full_text: Some(PAPER),
            this_year: 2026,
            specialists: Some(&specs),
            validity: Some(&validity),
            standards: Some(&standards),
            checklist: None,
            novelty: None,
        };
        let reports: Vec<ReviewerReport> = lenses().iter().map(|l| review(l, &input)).collect();
        let rubric = decide(&EditorInput { reports: &reports, journal: None });
        (ex, reports, rubric)
    }

    /// **There are four exports and no fifth.** §9: *"Everything else is a
    /// section of one of these."*
    #[test]
    fn there_are_exactly_four_exports() {
        assert_eq!(ExportKind::ALL.len(), 4);
    }

    /// **D92's refusal stands.** A tracked-changes `.docx` is the artefact D92
    /// declined on stated grounds, and §9 [v5] records that v4 listed it with
    /// no argument against that reason. This test is where an attempt to add
    /// one meets the requirement to make the argument first.
    #[test]
    fn there_is_no_tracked_changes_docx_export() {
        for k in ExportKind::ALL {
            let n = k.as_str();
            assert!(
                !n.contains("tracked") && !n.contains("docx") && !n.contains("redline"),
                "`{n}` looks like the artefact D92 declined: page geometry is not in the \
                 file, and a reconstruction that looks subtly wrong to the person who wrote \
                 the paper is worse than no reconstruction. Adding it needs an argument \
                 against D92's reason, recorded — not a new enum variant"
            );
        }
    }

    /// **The marked-up view never claims to be the whole review.** Measured,
    /// 20.4% of findings anchor; a page showing those and saying nothing about
    /// the rest reads as everything.
    #[test]
    fn the_annotated_manuscript_states_what_it_does_not_show() {
        let (ex, reports, _) = run();
        let a = annotate(&ex, &reports);
        assert!(!a.unanchored.is_empty(), "this paper has absence findings: {a:?}");
        assert!(a.coverage_note.contains("An absence has no coordinates"), "{}", a.coverage_note);
        assert!(
            a.coverage_note.contains(&format!("{} of", a.annotations.len())),
            "the note states the actual fraction: {}",
            a.coverage_note
        );
    }

    /// **An absence is never given a position.** The whole scoping decision.
    #[test]
    fn an_absence_finding_is_never_annotated_onto_the_page() {
        let (ex, reports, _) = run();
        let a = annotate(&ex, &reports);
        for code in ["no_data_availability_statement", "StatisticalTestReported"] {
            assert!(
                a.annotations.iter().all(|x| x.code != code),
                "`{code}` is about something the manuscript does NOT contain"
            );
            assert!(
                a.unanchored.iter().any(|x| x.code == code),
                "and it must appear in `unanchored` rather than vanish"
            );
        }
    }

    /// Every annotation quotes the manuscript text it sits on, so a reader can
    /// see the anchor landed where they expect.
    #[test]
    fn every_annotation_carries_the_paragraph_it_anchors_to() {
        let (ex, reports, _) = run();
        let a = annotate(&ex, &reports);
        for an in &a.annotations {
            assert!(!an.anchor_text.trim().is_empty(), "{an:?}");
            assert_eq!(paragraph_at(&ex, &an.location), Some(an.anchor_text.as_str()));
            assert!(!an.how_to_check.is_empty(), "§9 asks for a verification trail: {an:?}");
        }
    }

    /// **Nothing is lost between the page and the letter.** Every concern is in
    /// exactly one of the two, and the audit trail has all of them.
    #[test]
    fn every_concern_reaches_the_page_or_the_letter_and_none_is_dropped() {
        let (ex, reports, rubric) = run();
        let a = annotate(&ex, &reports);
        let total: usize =
            reports.iter().map(|r| r.major_concerns.len() + r.minor_concerns.len()).sum();
        assert_eq!(
            a.annotations.len() + a.unanchored.len(),
            total,
            "a finding that is neither annotated nor listed unanchored has vanished"
        );

        let l = letter(&reports, &rubric);
        let in_letter: usize =
            l.sections.iter().map(|s| s.major_concerns.len() + s.minor_concerns.len()).sum();
        assert_eq!(in_letter, total, "the letter carries every concern, anchored or not");
    }

    /// The audit trail gathers what the run could not read or decide, so the
    /// trail is auditable without re-deriving it.
    #[test]
    fn the_audit_trail_carries_what_was_not_read() {
        let (ex, reports, rubric) = run();
        let a = annotate(&ex, &reports);
        let t = audit_trail(&reports, &rubric, &a);
        assert_eq!(t.schema_version, AUDIT_TRAIL_SCHEMA);
        assert!(
            t.not_read.iter().any(|x| x.contains("disagreement map")),
            "{:?}",
            t.not_read
        );
        // Round-trips, because an audit trail that cannot be read back is a log.
        let json = serde_json::to_string(&t).unwrap();
        let back: AuditTrail = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rubric.posture, t.rubric.posture);
    }

    /// The fixture must carry unanchored concerns at more than one severity,
    /// or a deletion that drops just one severity passes unnoticed — which is
    /// what happened the first time this suite's deletion tests were run.
    #[test]
    fn the_fixture_exercises_unanchored_concerns_at_major_severity() {
        let (ex, reports, _) = run();
        let a = annotate(&ex, &reports);
        assert!(
            a.unanchored.iter().any(|u| u.severity == ReviewSeverity::Major),
            "no unanchored MAJOR concern, so a severity-selective drop would be invisible: {:?}",
            a.unanchored
        );
    }

    /// Annotations are ordered most severe first — the same precedence the
    /// editor applies, so the page and the rubric agree about what matters.
    #[test]
    fn annotations_are_ordered_most_severe_first() {
        let (ex, reports, _) = run();
        let a = annotate(&ex, &reports);
        let sev: Vec<ReviewSeverity> = a.annotations.iter().map(|x| x.severity).collect();
        let mut sorted = sev.clone();
        sorted.sort();
        assert_eq!(sev, sorted, "{sev:?}");
    }
}
