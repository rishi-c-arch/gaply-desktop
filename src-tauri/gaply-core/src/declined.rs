//! **What Gaply deliberately does not do, and why — in the user's words.**
//!
//! # A blank is a worse answer than a refusal
//!
//! Every lane below was declined on a measurement, and each decline is recorded
//! at length in `docs/AI_ENGINE_PLAN.md`. None of that reached a user. The
//! reviewer panel rendered `letter.novelty.assessment || '—'`, so a researcher
//! met an em-dash where a decision belonged — and an em-dash reads as *"Gaply
//! looked and found nothing"*, which is a claim about their manuscript rather
//! than about this product.
//!
//! **The two readings are not close.** "No novelty concerns" is reassurance a
//! paper has not earned; "Gaply does not assess novelty" is a limit a reader can
//! route around by asking someone who does.
//!
//! # These are the SAME claims as the engineering records, shortened
//!
//! Each `reason` is the user-facing form of a decline that already exists —
//! `chat_scope::CORRECTION_BLOCKER`, `chat_scope::CHALLENGE_BLOCKER`, the
//! `novelty` module header, `red_team`'s RT4 fixture. **Shortened, never
//! softened**: each carries the number that justified the decline, because a
//! decline without its measurement is an opinion.
//!
//! # Why a table and not a field on each lane
//!
//! A declined lane has no runtime object to hang a message on — that is what
//! being declined means. A reader asking *"where is novelty?"* is asking about
//! something that never runs, so the answer has to live somewhere that does.

/// One lane Gaply does not run, and what a user is told instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DeclinedLane {
    /// What the user would have looked for.
    pub lane: &'static str,
    /// One sentence, carrying the measurement. Shown verbatim.
    pub reason: &'static str,
    /// Where the full decision lives, for anyone who wants it.
    pub record: &'static str,
}

/// **Every lane declined on measurement.** Order is the order a panel shows.
pub const DECLINED_LANES: &[DeclinedLane] = &[
    DeclinedLane {
        lane: "Novelty",
        reason: "Gaply does not assess novelty. Across 20 real manuscripts, authors made \
                 2 claims about their own contribution in total — 0.1 per paper — so this \
                 check would report nothing on almost every submission.",
        record: "§11 D166",
    },
    DeclinedLane {
        lane: "Table arithmetic",
        reason: "Gaply does not check whether a table's numbers add up. Of 20 manuscripts \
                 only 2 contained a table with a totals row, and in both of those the \
                 obvious check was wrong about a correct paper — it fires on rounding, on \
                 subtotals, and on percentages taken of different bases.",
        record: "§11 D167",
    },
    DeclinedLane {
        lane: "Structured scientific claims",
        reason: "Gaply does not extract your claims, methods and datasets as structured \
                 objects. The extractor that would do it scored 5.9% against a 50% \
                 no-skill baseline, and anything built on it would inherit that.",
        record: "§11 D165",
    },
    DeclinedLane {
        lane: "Chat: suggested corrections",
        reason: "Chat can explain a finding and show the evidence behind it. It cannot \
                 suggest a fix — nothing in Gaply produces one, and the advice a finding \
                 carries is the reviewer criterion's generic text, not a correction written \
                 for your manuscript.",
        record: "§10",
    },
    DeclinedLane {
        lane: "Chat: challenging a finding",
        reason: "Chat cannot re-run the analysis when you disagree with a finding. \
                 Recording the disagreement without re-evaluating anything would leave the \
                 finding visibly unchanged, which is worse than saying so plainly.",
        record: "§10",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry must carry its measurement. A decline without a number is an
    /// opinion, and this panel is the one place a user meets it.
    #[test]
    fn every_decline_names_its_record_and_says_something() {
        assert!(!DECLINED_LANES.is_empty());
        for d in DECLINED_LANES {
            assert!(d.record.contains('§'), "no record: {d:?}");
            assert!(d.reason.len() > 60, "too short to be a reason: {d:?}");
            assert!(!d.lane.is_empty());
        }
    }

    /// **The em-dash is what this replaces.** A reason that does not say what
    /// Gaply does NOT do reads as a finding about the manuscript — the exact
    /// misreading the blank produced.
    #[test]
    fn every_reason_states_a_limit_of_the_product_not_of_the_paper() {
        for d in DECLINED_LANES {
            let l = d.reason.to_lowercase();
            assert!(
                l.contains("gaply does not") || l.contains("cannot") || l.contains("chat can"),
                "a reason must name the product's limit, not the paper's state: {d:?}"
            );
        }
    }

    /// The two chat lanes must agree with the blockers they summarise; if one is
    /// edited and the other is not, the user and the engineer read different
    /// declines.
    #[test]
    fn the_chat_declines_match_the_blockers_they_shorten() {
        let correction = DECLINED_LANES.iter().find(|d| d.lane.contains("corrections")).unwrap();
        let challenge = DECLINED_LANES.iter().find(|d| d.lane.contains("challenging")).unwrap();
        // Both blockers rest on "nothing produces one" / "re-analysis is unbuilt".
        assert!(crate::chat_scope::CORRECTION_BLOCKER.contains("nothing produces one"));
        assert!(correction.reason.contains("nothing in Gaply produces one"));
        assert!(crate::chat_scope::CHALLENGE_BLOCKER.contains("visibly not be re-evaluated"));
        assert!(challenge.reason.contains("visibly unchanged"));
    }
}
