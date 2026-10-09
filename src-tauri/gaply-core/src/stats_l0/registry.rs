//! **`MethodRegistry`: the closed list of methods, fixed at compile time.**
//!
//! A method is registered only when it has an implementation, golden fixtures,
//! a tolerance rule and edge-case tests. The fixtures are named here, beside
//! the method, and `every_registered_method_names_fixture_files_that_exist`
//! fails the build of the test suite when one is missing.

use super::input::L0Input;
use super::method::VerificationMethod;
use super::p_consistency::PConsistency;

/// One registered method and the fixture files that pin it, as paths relative
/// to `gaply-core/tests/fixtures/`.
pub struct RegisteredMethod {
    pub method: &'static (dyn for<'a> VerificationMethod<L0Input<'a>> + Sync),
    pub fixtures: &'static [&'static str],
}

pub struct MethodRegistry;

impl MethodRegistry {
    /// Every Level 0 method, in the order they run.
    pub const LEVEL_0: &'static [RegisteredMethod] = &[RegisteredMethod {
        method: &PConsistency,
        fixtures: &["stats_l0/p_fixtures.csv", "stats_l0/statcheck_diff_fixtures.csv"],
    }];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats_l0::method::Level;

    #[test]
    fn every_registered_method_names_fixture_files_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut checked = 0;
        for m in MethodRegistry::LEVEL_0 {
            assert!(!m.fixtures.is_empty(), "{} names no fixture", m.method.id());
            for f in m.fixtures {
                assert!(root.join(f).is_file(), "{}: fixture {f} is missing", m.method.id());
                checked += 1;
            }
        }
        // A registry that silently empties passes the loop above; this does not.
        assert!(checked > 0, "no fixtures checked: the registry is empty");
    }

    #[test]
    fn registered_ids_are_unique_and_level_0() {
        let ids: Vec<&str> = MethodRegistry::LEVEL_0.iter().map(|m| m.method.id()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "{ids:?}");
        assert!(MethodRegistry::LEVEL_0.iter().all(|m| m.method.level() == Level::L0));
        assert_eq!(ids, vec!["p_consistency"]);
    }
}
