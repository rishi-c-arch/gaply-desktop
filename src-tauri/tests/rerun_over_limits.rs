//! **A prepared run's rerun over this computer's limits, end to end (Research
//! OS, Phase 8T-1a, decision 492; decision 439's answer 4).** The 8 GB tier's
//! largest table, 100,000 x 50 one-digit cells (exactly 5,000,000, so it opens
//! on every tier), opened with the 8 GB tier's limits; a score of all fifty
//! items made at the 16 GB tier's cap, so the prepared table holds 5,100,000
//! cells; a summary of the score, saved; then the project's rerun, on THIS
//! computer's limits, which `rerun_open` reads inline (pinned at
//! `prep_guards.rs`), so no branch can be forced:
//!
//! * on an 8 GB computer the rebuilt recipe passes the cap, and the rerun is
//!   refused as `project.rerun_over_limits`, nothing started;
//! * on 16 GB and 32 GB it runs, saves, and is Identical.
//!
//! The test asserts the branch its computer's tier gives and prints which,
//! with its time. Its waits have a limit of their own, above the worst time
//! measured in a debug build, not the 20 s of `World::wait`. It tests code
//! 8M-4 wrote (`rerun_recipe_code`), so it passed on its first run: its red is
//! the mutation of that function. Never ignored. Synthetic data only.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::memory_tier::{this_computer, MemoryTier};
use app_lib::project_commands::{self as pc, RerunProgress, SaveTarget};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use app_lib::supplementary::IngestLimits;
use gaply_core::contracts::{CapabilityId, OperationId, RunId, RunState};
use gaply_core::research_project::{Caps, RerunComparison};
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

use project_support::*;

const ROWS: usize = 100_000;
const COLUMNS: u32 = 50;

/// Each wait's own limit: above the worst whole-test time measured in a debug
/// build on this Mac (see the report), with room for a loaded machine.
const WAIT: Duration = Duration::from_secs(300);

/// Caps that take a 10 MB data file: SMALL's 10 MiB project cap is too tight
/// for the file, its spec, bundle and manifest together.
const ROOMY: Caps = Caps {
    project: 64 * 1024 * 1024,
    all_projects: 256 * 1024 * 1024,
    db_allowance: 64 * 1024,
};

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// 100,000 rows of 50 one-digit cells, 1 to 5, from a fixed generator.
fn the_table() -> Vec<u8> {
    let header: Vec<String> = (1..=COLUMNS).map(|c| format!("q{c}")).collect();
    let mut out = Vec::with_capacity(10_000_200);
    out.extend_from_slice(header.join(",").as_bytes());
    out.push(b'\n');
    let mut x: u32 = 12_345;
    for _ in 0..ROWS {
        for c in 0..COLUMNS {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            out.push(b'1' + ((x >> 16) % 5) as u8);
            out.push(if c + 1 == COLUMNS { b'\n' } else { b',' });
        }
    }
    out
}

/// The mean of all fifty items, column 51.
fn the_recipe() -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: COLUMNS,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(COLUMNS + 1),
            name: "all fifty".into(),
            items: (1..=COLUMNS).map(alias).collect(),
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Mean),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

fn wait_terminal(w: &World, run: u64) -> RunState {
    let e = w.engine();
    let start = Instant::now();
    loop {
        let state = e
            .runs
            .snapshot(RunId(run))
            .map(|s| s.record.state)
            .expect("the run is held");
        if state.is_terminal() {
            return state;
        }
        assert!(start.elapsed() < WAIT, "the summary never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_prepared_run_reruns_on_this_computers_limits_over_limits_on_8_gb_identical_above() {
    let began = Instant::now();
    let w = World::with("8t1a-over-limits", ROOMY, None);
    let e = w.engine();

    let table = the_table();
    assert_eq!(table.len(), 10_000_191, "the CSV's size");
    let path = w.dir.write("wide.csv", &table);
    drop(table);
    let opened = desc::open_with(
        &e.datasets,
        &path,
        None,
        IngestLimits::for_tier(MemoryTier::Gb8),
    )
    .expect("100,000 x 50 opens on the 8 GB tier's limits");
    assert_eq!(opened.profile.rows, ROWS as u64);
    assert_eq!(opened.profile.columns.len(), COLUMNS as usize);
    let slot = opened.slot;

    let cap_16 = IngestLimits::for_tier(MemoryTier::Gb16).max_cells as u64;
    let made = prep::make(&e.runs, &e.datasets, slot, &the_recipe(), cap_16)
        .map_err(|_| "the make")
        .unwrap();
    assert!(made.dataset.is_some(), "{:?}", made.errors);
    let after_make = began.elapsed();

    // A summary of the score, its spec as a screen builds one on this slot.
    let held = e.datasets.get(slot).expect("the slot");
    let mut spec = stats_desc::spec(
        OperationId(1),
        held.dataset(),
        DescribeParams {
            columns: vec![ColumnRequest {
                alias: alias(COLUMNS + 1),
                treat_as: TreatAs::Numeric,
            }],
            missing_codes: Vec::new(),
        },
    );
    spec.preparation = held.preparation();
    drop(held);
    let run = desc::run(&e.runs, &e.datasets, slot, spec)
        .map(|v| v.run_id)
        .unwrap_or_else(|_| panic!("the summary starts"));
    assert_eq!(wait_terminal(&w, run), RunState::Succeeded);
    let id = pc::save_run(
        &w.root(),
        &w.ws,
        run,
        SaveTarget::NewProject {
            name: "Over limits".into(),
        },
    )
    .expect("the prepared summary saves")
    .project_id;
    let after_save = began.elapsed();

    let tier = this_computer().tier;
    match tier {
        MemoryTier::Gb8 => {
            let refused = pc::rerun(&w.root(), &w.ws, 1).map(|_| ()).unwrap_err();
            assert_eq!(code(&refused), "project.rerun_over_limits");
            assert!(
                pc::current(&w.ws).unwrap().rerun.is_none(),
                "nothing started"
            );
            assert_eq!(pc::runs(&w.root(), &w.ws, &id).unwrap().len(), 1);
        }
        MemoryTier::Gb16 | MemoryTier::Gb32 => {
            pc::rerun(&w.root(), &w.ws, 1)
                .unwrap_or_else(|e| panic!("the rerun starts: {}", code(&e)));
            let start = Instant::now();
            let r = loop {
                if let Some(r) = pc::current(&w.ws).unwrap().rerun {
                    if r.status != RerunProgress::Going {
                        break r;
                    }
                }
                assert!(start.elapsed() < WAIT, "the rerun never settled");
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(r.status, RerunProgress::Saved);
            assert_eq!(r.stored_run_id, Some(2));
            let view = pc::runs(&w.root(), &w.ws, &id).unwrap();
            let again = view.iter().find(|r| r.stored_run_id == 2).unwrap();
            assert_eq!(again.rerun_of, Some(1));
            assert_eq!(again.comparison, Some(RerunComparison::Identical));
        }
    }
    println!(
        "rerun_over_limits: tier {tier:?}, branch {}; open and make {:.1} s, run and save {:.1} s, whole {:.1} s",
        if tier == MemoryTier::Gb8 {
            "project.rerun_over_limits"
        } else {
            "Identical"
        },
        after_make.as_secs_f64(),
        (after_save - after_make).as_secs_f64(),
        began.elapsed().as_secs_f64(),
    );
}
