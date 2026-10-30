//! **This computer's memory tier and the methods' limits for it (Research
//! OS, Phase 8F-1, decisions 281 and 284).**
//!
//! * the tier at every boundary of decision 281, from injected readings:
//!   0, 1, 12 GiB - 1, 12 GiB, 24 GiB - 1, 24 GiB, `u64::MAX`, and unknown;
//!   unknown and 0 are the 8 GB tier with `memory_known` false;
//! * this machine's reading is known on macOS and Linux, and its tier is
//!   printed;
//! * decision 281's table, pinned number by number, and the rules it keeps:
//!   no tier above 100,000 rows, every file cap under decision 107's 100 MiB
//!   project cap, the columns within the dataset's own width;
//! * `stats_desc_limits`'s view for each tier is its wire fixture, byte for
//!   byte, and so is `stats_desc_open_progress`'s view.
//!
//! No limit here reaches a user in 8F-1: the methods' open and the rerun
//! keep `IngestLimits::METHODS` (`tests/ingest_limits_guard.rs`).

use std::path::PathBuf;

use app_lib::memory_tier::{tier_for, MemoryReading, MemoryTier, MethodLimits};
use app_lib::stats_desc_commands::{limits_view, OpenProgressView};
use app_lib::supplementary::{IngestLimits, MAX_COLS};
use gaply_core::research_project::PROJECT_CAP_BYTES;
use gaply_core::stats_l1::dataset::MAX_COLUMNS;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

fn known(tier: MemoryTier) -> MemoryReading {
    MemoryReading {
        tier,
        memory_known: true,
    }
}

#[test]
fn the_tier_follows_total_memory_at_every_boundary() {
    let unknown = MemoryReading {
        tier: MemoryTier::Gb8,
        memory_known: false,
    };
    for (reading, want) in [
        (None, unknown),
        (Some(0), unknown),
        (Some(1), known(MemoryTier::Gb8)),
        (Some(8 * GIB), known(MemoryTier::Gb8)),
        (Some(12 * GIB - 1), known(MemoryTier::Gb8)),
        (Some(12 * GIB), known(MemoryTier::Gb16)),
        (Some(16 * GIB), known(MemoryTier::Gb16)),
        (Some(24 * GIB - 1), known(MemoryTier::Gb16)),
        (Some(24 * GIB), known(MemoryTier::Gb32)),
        (Some(64 * GIB), known(MemoryTier::Gb32)),
        (Some(u64::MAX), known(MemoryTier::Gb32)),
    ] {
        assert_eq!(tier_for(reading), want, "a reading of {reading:?} bytes");
    }
    assert_eq!(MemoryTier::Gb8.word(), "8gb");
    assert_eq!(MemoryTier::Gb16.word(), "16gb");
    assert_eq!(MemoryTier::Gb32.word(), "32gb");
}

#[test]
fn this_computers_reading_is_known_on_macos_and_linux_and_its_tier_is_printed() {
    let total = app_lib::memory_tier::total_memory_bytes();
    let reading = app_lib::memory_tier::this_computer();
    println!(
        "this computer: total memory read = {}, tier {}, memory_known {}",
        total.is_some(),
        reading.tier.word(),
        reading.memory_known
    );
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        assert!(total.is_some_and(|t| t > 0), "total memory is read here");
        assert!(reading.memory_known);
    }
    assert_eq!(
        reading,
        tier_for(total),
        "the kept reading is the tier of the reading"
    );
}

#[test]
fn each_tier_has_decision_281s_limits() {
    // (tier, rows, columns, cells, file bytes, cell text, workbook inflated, slot budget)
    let table = [
        (
            MemoryTier::Gb8,
            100_000,
            128,
            5_000_000,
            64 * MIB,
            64 * MIB,
            96 * MIB,
            10_000_000,
        ),
        (
            MemoryTier::Gb16,
            100_000,
            128,
            12_800_000,
            96 * MIB,
            96 * MIB,
            256 * MIB,
            51_200_000,
        ),
        (
            MemoryTier::Gb32,
            100_000,
            128,
            12_800_000,
            96 * MIB,
            96 * MIB,
            256 * MIB,
            51_200_000,
        ),
    ];
    for (tier, rows, cols, cells, file, text, workbook, budget) in table {
        let m = MethodLimits::for_tier(tier);
        let l = m.ingest;
        let w = tier.word();
        assert_eq!(l, IngestLimits::for_tier(tier), "{w}");
        assert_eq!(l.max_rows, rows, "{w}");
        assert_eq!(l.max_cols, cols, "{w}");
        assert_eq!(l.max_cells, cells, "{w}");
        assert_eq!(l.max_file_size_bytes, file, "{w}");
        assert_eq!(l.max_total_chars as u64, text, "{w}");
        assert_eq!(l.max_workbook_uncompressed_bytes, workbook, "{w}");
        assert_eq!(m.slot_budget_cells, budget, "{w}");

        // Decision 242.3: no tier above 100,000 rows.
        assert!(l.max_rows <= 100_000, "{w}");
        // Decision 107: every file that opens can be saved.
        assert!(l.max_file_size_bytes < PROJECT_CAP_BYTES, "{w}");
        // The dataset's own width is the ceiling, and every tier keeps 128.
        assert_eq!(l.max_cols, MAX_COLS, "{w}");
        assert!(l.max_cols <= MAX_COLUMNS, "{w}");
        // The cell cap never exceeds the rows times the columns.
        assert!(l.max_cells <= l.max_rows * l.max_cols, "{w}");
        // The slot budget holds at least one dataset at the cell cap.
        assert!(m.slot_budget_cells >= l.max_cells as u64, "{w}");
    }
    // On 16 and 32 GB the rows and columns bind first, and the slot count:
    // the budget is four datasets at the cap.
    for tier in [MemoryTier::Gb16, MemoryTier::Gb32] {
        let m = MethodLimits::for_tier(tier);
        assert_eq!(m.ingest.max_cells, m.ingest.max_rows * m.ingest.max_cols);
        assert_eq!(
            m.slot_budget_cells,
            app_lib::runs::MAX_SLOTS as u64 * m.ingest.max_cells as u64
        );
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics/fixtures")
}

fn assert_bytes<T: serde::Serialize>(name: &str, view: &T) {
    let want =
        std::fs::read_to_string(fixtures().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

#[test]
fn each_tiers_limits_view_is_its_wire_fixture() {
    // The 8 GB fixture is the unknown reading, the case decision 281 sets
    // apart; the other two are read tiers.
    assert_bytes(
        "methodLimitsView8gb.json",
        &limits_view(MemoryReading {
            tier: MemoryTier::Gb8,
            memory_known: false,
        }),
    );
    assert_bytes(
        "methodLimitsView16gb.json",
        &limits_view(known(MemoryTier::Gb16)),
    );
    assert_bytes(
        "methodLimitsView32gb.json",
        &limits_view(known(MemoryTier::Gb32)),
    );
}

#[test]
fn the_open_progress_view_is_its_wire_fixture() {
    assert_bytes(
        "openProgressView.json",
        &OpenProgressView { rows_read: 4000 },
    );
}
