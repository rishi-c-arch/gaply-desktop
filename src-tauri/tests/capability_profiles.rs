//! **The registry's resource profiles are the app's constants (Research OS,
//! Phase 4A).** `gaply_core::stats_registry` may not read the app crate, so
//! each limit is a literal there; this pins every literal to the constant it
//! copies, so the two cannot drift:
//!
//! * `max_rows` = `supplementary::MAX_ROWS`, `max_columns` = `MAX_COLS` and
//!   `max_input_bytes` = `MAX_TOTAL_CHARS` (cell text, UTF-8 bytes) for
//!   `stats.verify.*`;
//! * for the methods on the user's data (`stats.describe` and the three
//!   t-tests), since Phase 8F-2 (decision 288): `max_rows` and `max_columns`
//!   = every memory tier's `IngestLimits::for_tier` rows and columns, the
//!   same on every tier, and `max_input_bytes` none, since it is set by the
//!   computer;
//! * `ceiling_ms` = `runs::RUN_CEILING_MS` for `stats.describe` and
//!   `stats.analyze.independent_t` (Phase 4B-1: the same run manager's
//!   ceiling and check point), and none for Level 0 and Level 1, which have
//!   no time ceiling;
//! * the table formats = `stats_l1_ingest::TABLE_EXTENSIONS`.
//!
//! Level 0's manuscript cap is pinned to `import_guard::MAX_BYTES` in
//! `gaply-core/tests/capability_registry.rs`.

use app_lib::memory_tier::MemoryTier;
use app_lib::supplementary::IngestLimits;
use app_lib::{runs, stats_l1_ingest, supplementary};
use gaply_core::contracts::capability::{Cancel, FileFormat};
use gaply_core::stats_registry::{Kind, Registry};

fn word(f: FileFormat) -> String {
    serde_json::to_value(f)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn every_profile_literal_is_the_app_constant_it_copies() {
    let t = Registry::statistics();
    let mut tables = 0;
    for e in t.entries() {
        let p = e.descriptor.resource_profile;
        let id = e.descriptor.id.as_str();
        match e.kind {
            Kind::Verify(_) => {
                assert_eq!(p.max_rows, Some(supplementary::MAX_ROWS as u64), "{id}");
                assert_eq!(p.max_columns, Some(supplementary::MAX_COLS as u64), "{id}");
                assert_eq!(
                    p.max_input_bytes,
                    Some(supplementary::MAX_TOTAL_CHARS as u64),
                    "{id}"
                );
            }
            Kind::Describe
            | Kind::AnalyzeIndependentT
            | Kind::AnalyzeOneSampleT
            | Kind::AnalyzePairedT => {
                for tier in [MemoryTier::Gb8, MemoryTier::Gb16, MemoryTier::Gb32] {
                    let l = IngestLimits::for_tier(tier);
                    assert_eq!(p.max_rows, Some(l.max_rows as u64), "{id} {tier:?}");
                    assert_eq!(p.max_columns, Some(l.max_cols as u64), "{id} {tier:?}");
                }
                assert_eq!(p.max_input_bytes, None, "{id}: set by the computer");
            }
            Kind::Check => {}
        }
        match e.kind {
            Kind::Verify(_)
            | Kind::Describe
            | Kind::AnalyzeIndependentT
            | Kind::AnalyzeOneSampleT
            | Kind::AnalyzePairedT => {
                tables += 1;
                let mut formats: Vec<String> = e
                    .descriptor
                    .supported_formats
                    .iter()
                    .map(|f| word(*f))
                    .collect();
                let mut accepted: Vec<String> = stats_l1_ingest::TABLE_EXTENSIONS
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                formats.sort();
                accepted.sort();
                assert_eq!(
                    formats,
                    accepted,
                    "{}: the ingest's table formats",
                    e.descriptor.id.as_str()
                );
            }
            Kind::Check => {
                assert_eq!(
                    (p.max_rows, p.max_columns, p.ceiling_ms),
                    (None, None, None)
                );
                assert_eq!(p.cancel, Cancel::None);
            }
        }
        match e.kind {
            Kind::Describe
            | Kind::AnalyzeIndependentT
            | Kind::AnalyzeOneSampleT
            | Kind::AnalyzePairedT => {
                assert_eq!(p.ceiling_ms, Some(runs::RUN_CEILING_MS));
                assert_eq!(
                    p.cancel,
                    Cancel::Cooperative,
                    "the run manager's check point"
                );
            }
            Kind::Verify(_) => {
                assert_eq!(p.ceiling_ms, None, "Level 1 has no time ceiling");
                assert_eq!(p.cancel, Cancel::None, "compute is one call");
            }
            Kind::Check => {}
        }
        assert!(!p.uses_gpu && !p.temp_disk, "{}", e.descriptor.id.as_str());
    }
    assert_eq!(
        tables, 11,
        "seven Level 1 capabilities, the descriptive summary and the three t-tests (Phase 8D-1)"
    );
}
