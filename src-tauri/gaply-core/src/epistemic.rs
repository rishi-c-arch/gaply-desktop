//! **What a finding ASSERTS about a claim — §9's `EpistemicStatus`, and the
//! rule for reading a manuscript's decimals.**
//!
//! # Why this is a new type and not a rename of something shipping
//!
//! The architecture document and the code carry three DIFFERENT axes that are
//! easy to mistake for one, and §11 D156 records the decision that keeps them
//! apart:
//!
//! | axis | question it answers | where it lives |
//! |---|---|---|
//! | **trust** | how much weight does this carry, and what overrides what | [`crate::report::CertaintyTier`] — SHIPPING, 72 references, a serde wire contract the frontend reads |
//! | **verdict** | did a recomputation match | [`crate::stats_verdict::Verdict`] — binary by construction, and that is load-bearing |
//! | **epistemic status** | what is being asserted about the claim | HERE — nothing shipping expressed it |
//!
//! `Verdict` is `Match | Mismatch` and `stats_verdict.rs` depends on it staying
//! binary: the lane separation there is enforced BY CONSTRUCTION, and adding
//! `Unverified` to it would put an un-decided result inside the type that
//! carries `MathematicallyCertain`. The two states this module exists for —
//! [`EpistemicStatus::Unverified`] and
//! [`EpistemicStatus::RequiresAuthorConfirmation`] — are exactly the two that
//! type cannot hold.
//!
//! **§4.4's five-tier table deliberately did NOT become a type.** See §11 D156.
//!
//! # DETECTED IS NOT PROVEN WRONG
//!
//! §9: *"there may be a protocol reason, and the product does not auto-correct
//! scientifically ambiguous things."* [`EpistemicStatus::Detected`] says the
//! engine found a disagreement, not that the author is wrong.
//!
//! # The decimal rule — why every arithmetic finding carries two readings
//!
//! **A manuscript's written decimals are ambiguous by construction.** `0.108`
//! may be the exact value the author used, or `0.10843…` displayed to three
//! places. Which one was meant is NOT RECOVERABLE FROM THE TEXT, and the
//! difference changes the answer: under one reading a chain is arithmetically
//! false, under the other it is fine.
//!
//! Picking one silently would be the engine deciding something it cannot know
//! — the §6b.3 failure, in the place it is easiest to commit. So every
//! arithmetic claim is evaluated under BOTH readings ([`DecimalReading`]) and
//! the outcome says which readings agree:
//!
//! * both readings say it holds → **no finding**;
//! * both say it fails → a finding, [`EpistemicStatus::Detected`];
//! * they disagree → a finding that SAYS SO,
//!   [`EpistemicStatus::RequiresAuthorConfirmation`], with both numbers;
//! * not computable → [`EpistemicStatus::Unverified`], never a guess.
//!
//! One case is settled before the readings are consulted at all: when the
//! reported value is what the computed value ROUNDS TO at the precision the
//! author displayed, there is no discrepancy to explain. Slovin's formula in
//! `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` computes 623.35613… and is
//! reported as `623.36`; that is display rounding, and it produces no finding.

use serde::{Deserialize, Serialize};

/// What a finding asserts about the claim it names. §9's vocabulary, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicStatus {
    /// A disagreement was found. **Not a claim that the author is wrong** —
    /// §9's `DETECTED ≠ PROVEN WRONG`.
    Detected,
    /// Evidence points the claim's way without settling it.
    Supported,
    /// Established. For Tier 0 this means exact arithmetic agreement.
    Confirmed,
    /// Established to be false.
    Contradicted,
    /// The record is insufficient to decide, and the engine says so rather
    /// than guessing (§6b.3).
    Unverified,
    /// The answer depends on something only the author knows. The engine
    /// states every reading and its result, and asks.
    RequiresAuthorConfirmation,
}

impl EpistemicStatus {
    /// Does this status put something in front of the author?
    ///
    /// [`EpistemicStatus::Confirmed`] and [`EpistemicStatus::Supported`] do
    /// not: a report that lists every check that passed buries the ones that
    /// did not.
    pub fn is_finding(&self) -> bool {
        matches!(
            self,
            EpistemicStatus::Detected
                | EpistemicStatus::Contradicted
                | EpistemicStatus::RequiresAuthorConfirmation
        )
    }

    /// The wording shown to a reader. Never a bare enum name.
    pub fn label(&self) -> &'static str {
        match self {
            EpistemicStatus::Detected => "detected",
            EpistemicStatus::Supported => "supported",
            EpistemicStatus::Confirmed => "confirmed",
            EpistemicStatus::Contradicted => "contradicted",
            EpistemicStatus::Unverified => "unverified",
            EpistemicStatus::RequiresAuthorConfirmation => "requires author confirmation",
        }
    }
}

/// How to read the decimals a manuscript wrote.
///
/// Neither reading is the right one in general — that is the whole point. See
/// the module header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecimalReading {
    /// The digits ARE the values. `0.108` is exactly `108/1000`.
    ///
    /// This is what the author literally wrote, so an arithmetic claim that
    /// fails here fails as written, whatever was meant.
    AsWritten,
    /// The digits are a ROUNDING. `0.108` stands for any value in
    /// `[0.1075, 0.1085)`, and the claim is judged over the whole interval.
    ///
    /// A claim that can hold for SOME assignment in those intervals is not
    /// arithmetically refuted; one that can hold for NONE is, and that refusal
    /// is certain regardless of what the author meant.
    AsRounded,
}

impl DecimalReading {
    pub const BOTH: [DecimalReading; 2] = [DecimalReading::AsWritten, DecimalReading::AsRounded];

    pub fn label(&self) -> &'static str {
        match self {
            DecimalReading::AsWritten => "reading the decimals as exact",
            DecimalReading::AsRounded => "reading the decimals as rounded",
        }
    }
}

/// The result of judging one arithmetic claim under both readings.
///
/// Constructed only by [`Agreement::of`], so the mapping from two readings to a
/// status cannot be spelled differently in two places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Agreement {
    pub as_written_holds: bool,
    pub as_rounded_holds: bool,
}

impl Agreement {
    pub fn of(as_written_holds: bool, as_rounded_holds: bool) -> Agreement {
        Agreement { as_written_holds, as_rounded_holds }
    }

    /// The rule, in one place.
    ///
    /// Note the asymmetry, which is not an oversight: `as_written` true and
    /// `as_rounded` false is impossible in practice — the rounded reading
    /// contains the written values in its intervals, so anything that holds
    /// exactly also holds somewhere in the intervals. It is mapped to
    /// [`EpistemicStatus::RequiresAuthorConfirmation`] rather than to
    /// `unreachable!()`, because a Tier-0 engine should not panic on a case its
    /// author believed impossible.
    pub fn status(&self) -> EpistemicStatus {
        match (self.as_written_holds, self.as_rounded_holds) {
            (true, true) => EpistemicStatus::Confirmed,
            (false, false) => EpistemicStatus::Detected,
            _ => EpistemicStatus::RequiresAuthorConfirmation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_readings_agreeing_that_it_holds_is_not_a_finding() {
        let a = Agreement::of(true, true);
        assert_eq!(a.status(), EpistemicStatus::Confirmed);
        assert!(!a.status().is_finding());
    }

    #[test]
    fn both_readings_agreeing_that_it_fails_is_a_finding() {
        let a = Agreement::of(false, false);
        assert_eq!(a.status(), EpistemicStatus::Detected);
        assert!(a.status().is_finding());
    }

    /// The 21.9% case: false as written, permissible once the inputs are read
    /// as rounded. The engine cannot know which the author meant, so it asks.
    #[test]
    fn readings_that_disagree_become_a_question_for_the_author() {
        let a = Agreement::of(false, true);
        assert_eq!(a.status(), EpistemicStatus::RequiresAuthorConfirmation);
        assert!(a.status().is_finding(), "it must still reach the author");
    }

    #[test]
    fn the_impossible_direction_is_a_question_not_a_panic() {
        assert_eq!(
            Agreement::of(true, false).status(),
            EpistemicStatus::RequiresAuthorConfirmation
        );
    }

    #[test]
    fn detected_is_not_contradicted_and_neither_is_silent() {
        assert_ne!(EpistemicStatus::Detected, EpistemicStatus::Contradicted);
        assert!(EpistemicStatus::Detected.is_finding());
        assert!(!EpistemicStatus::Unverified.is_finding());
        assert!(!EpistemicStatus::Confirmed.is_finding());
    }

    #[test]
    fn the_wire_names_are_snake_case_and_stable() {
        let j = serde_json::to_string(&EpistemicStatus::RequiresAuthorConfirmation).unwrap();
        assert_eq!(j, "\"requires_author_confirmation\"");
        assert_eq!(
            serde_json::to_string(&DecimalReading::AsWritten).unwrap(),
            "\"as_written\""
        );
    }
}
