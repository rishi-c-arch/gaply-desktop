//! **`MethodRegistry`: the closed lists of methods, fixed at compile time.**
//!
//! A method is registered only when it has an implementation, golden fixtures,
//! a tolerance rule and edge-case tests. The fixtures are named here, beside
//! the method, and `every_registered_method_names_fixture_files_that_exist`
//! fails the build of the test suite when one is missing.
//!
//! Two lists, one per input type. [`MethodRegistry::LEVEL_0`] runs on one
//! reported result and its document ([`L0Input`]): it is the list the
//! manuscript path runs. [`MethodRegistry::SUMMARY`] runs on a mean, a
//! percentage or a count ([`SummaryInput`]), which Phase 1 cannot read from a
//! manuscript; nothing on the manuscript path runs it.

use super::df_matches_n::DfMatchesN;
use super::estimate_in_interval::EstimateInInterval;
use super::grim::{GrimMean, GrimPercent};
use super::input::L0Input;
use super::method::VerificationMethod;
use super::p_consistency::PConsistency;
use super::percent_matches_count::PercentMatchesCount;
use super::summary::SummaryInput;

/// One registered method and the fixture files that pin it, as paths relative
/// to `gaply-core/tests/fixtures/`.
pub struct RegisteredMethod {
    pub method: &'static (dyn for<'a> VerificationMethod<L0Input<'a>> + Sync),
    pub fixtures: &'static [&'static str],
}

/// The same, for a method on [`SummaryInput`].
pub struct RegisteredSummaryMethod {
    pub method: &'static (dyn VerificationMethod<SummaryInput> + Sync),
    pub fixtures: &'static [&'static str],
}

pub struct MethodRegistry;

impl MethodRegistry {
    /// Every Level 0 method on a reported result, in the order they run.
    pub const LEVEL_0: &'static [RegisteredMethod] = &[
        RegisteredMethod {
            method: &PConsistency,
            fixtures: &["stats_l0/p_fixtures.csv", "stats_l0/statcheck_diff_fixtures.csv"],
        },
        RegisteredMethod { method: &EstimateInInterval, fixtures: &["stats_l0/estimate_interval_fixtures.csv"] },
        RegisteredMethod { method: &DfMatchesN, fixtures: &["stats_l0/df_n_fixtures.csv"] },
    ];

    /// Every Level 0 method on a summary statistic, in the order they run.
    pub const SUMMARY: &'static [RegisteredSummaryMethod] = &[
        RegisteredSummaryMethod { method: &PercentMatchesCount, fixtures: &["stats_l0/percent_count_fixtures.csv"] },
        RegisteredSummaryMethod { method: &GrimMean, fixtures: &["stats_l0/grim_fixtures.csv"] },
        RegisteredSummaryMethod { method: &GrimPercent, fixtures: &["stats_l0/grim_fixtures.csv"] },
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l0::method::{Level, Provenance};

    /// (id, level, provenance, fixtures) over BOTH lists.
    fn all() -> Vec<(&'static str, Level, Provenance, &'static [&'static str])> {
        let a = MethodRegistry::LEVEL_0
            .iter()
            .map(|m| (m.method.id(), m.method.level(), m.method.provenance(), m.fixtures));
        let b = MethodRegistry::SUMMARY
            .iter()
            .map(|m| (m.method.id(), m.method.level(), m.method.provenance(), m.fixtures));
        a.chain(b).collect()
    }

    #[test]
    fn every_registered_method_names_fixture_files_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut checked = 0;
        for (id, _, _, fixtures) in all() {
            assert!(!fixtures.is_empty(), "{id} names no fixture");
            for f in fixtures {
                assert!(root.join(f).is_file(), "{id}: fixture {f} is missing");
                checked += 1;
            }
        }
        // A registry that silently empties passes the loop above; this does not.
        assert_eq!(checked, 7, "fixtures checked across both lists: 2 + 1 + 1, then 1 + 1 + 1");
    }

    #[test]
    fn registered_ids_are_unique_and_level_0() {
        let ids: Vec<&str> = all().iter().map(|m| m.0).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "{ids:?}");
        assert!(all().iter().all(|m| m.1 == Level::L0));
        assert_eq!(
            ids,
            vec!["p_consistency", "estimate_in_interval", "df_matches_n", "percent_matches_count", "grim_mean", "grim_percent"]
        );
    }

    #[test]
    fn every_method_is_labelled_and_a_published_one_names_its_reference() {
        let mut published = Vec::new();
        for (id, _, provenance, _) in all() {
            if let Provenance::Published { reference } = provenance {
                assert!(reference.contains("20"), "{id}: reference names no year: {reference:?}");
                published.push(id);
            }
        }
        assert_eq!(published, vec!["p_consistency", "grim_mean", "grim_percent"]);
    }
}
