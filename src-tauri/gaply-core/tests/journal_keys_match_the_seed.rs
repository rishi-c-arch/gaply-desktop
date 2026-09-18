//! **The runtime's journal keys and the bundled snapshot's journal keys must be
//! the same set.**
//!
//! # The defect, measured 18 Sep 2026
//!
//! Two committed files name the same ten journals, and five of the ten were
//! spelled differently in each:
//!
//! ```text
//! config/journal-crawl.json   gaply-core/data/journal-seed.json
//! nature-comms                nature-communications
//! stat-med                    statistics-in-medicine
//! front-pubhealth             frontiers-public-health
//! jhp-sage                    j-health-psychology
//! bmc-pubhealth               bmc-public-health
//! ```
//!
//! `journal_profiles` enumerates the CONFIG and looks each key up with
//! `fingerprint_for` → `requirements_for`, whose SQL is `WHERE journal_key = ?1`
//! with no normalisation. `load_bundled_seed` writes rows under the SEED's keys.
//! So five journals' requirements sat in the database, correct and complete, and
//! no surface ever asked for them — 83 requirement rows, of which 68 were
//! Frontiers in Public Health. The picker filters on `requirement_count > 0`, so
//! those five silently became "not crawled — structural checks only".
//!
//! # Why neither file's own tests could see it
//!
//! `journal_store::seed_tests` asserts the seed writes what the seed says
//! (`requirements_for(&db, "nature-medicine").len() == 39`), which is true and
//! says nothing about the config. Nothing read both files. This is the
//! spec-and-implementation-agree shape with the artefacts reduced to two: each
//! is internally correct, and the defect exists only in the composition.
//!
//! # Why this test lives in `gaply_core` and reads across the crate boundary
//!
//! Deliberate, and the reason is mechanical rather than aesthetic: **both CI
//! workflows run `cargo test -p gaply_core` and nothing else**
//! (`.github/workflows/clean-checkout.yml`, `windows-build-check.yml`). A guard
//! in the app crate would pass under a local `cargo test --workspace` and never
//! run remotely — which is the failure mode this repo already met from the other
//! side when a package-scoped green was read as a suite result.
//!
//! `config/journal-crawl.json` therefore belongs to the app crate and is read
//! here by a relative `include_str!`. `gaply_core` itself does not read it; only
//! this test does, so the library stays portable and Tauri-free.
//!
//! # The vacuous-pass half
//!
//! Both key sets are asserted NON-EMPTY before they are compared. Either file
//! could be renamed, restructured, or have its array emptied, and set equality
//! over two empty sets is `true` — a guard whose input can silently become empty
//! passes forever. The `journal_tables_have_writers` sibling makes the same
//! assertion for the same reason.

use std::collections::BTreeSet;

/// The runtime's list — what `journal_profiles` enumerates
/// (`src-tauri/src/commands.rs`, `CrawlBudget::from_json`).
const CRAWL_CONFIG: &str = include_str!("../../config/journal-crawl.json");

/// The shipped snapshot — what `journal_store::load_bundled_seed` writes.
const SEED: &str = include_str!("../data/journal-seed.json");

fn keys(json: &str, array: &str, field: &str, what: &str) -> BTreeSet<String> {
    let v: serde_json::Value = serde_json::from_str(json)
        .unwrap_or_else(|e| panic!("{what} is not valid JSON: {e}"));
    let rows = v[array]
        .as_array()
        .unwrap_or_else(|| panic!("{what} has no `{array}` array — this guard is now blind"));
    rows.iter()
        .map(|r| {
            r[field]
                .as_str()
                .unwrap_or_else(|| panic!("{what}: a `{array}` row has no string `{field}`: {r}"))
                .to_string()
        })
        .collect()
}

#[test]
fn the_runtime_and_the_bundled_seed_name_the_same_journals() {
    let config = keys(CRAWL_CONFIG, "profiled_journals", "key", "config/journal-crawl.json");
    let seed = keys(SEED, "fingerprints", "journal_key", "data/journal-seed.json");

    // POSITIVE COUNTS FIRST. Set equality over two empty sets is true, so a
    // renamed array or an emptied list would make this guard pass forever.
    assert!(
        !config.is_empty(),
        "config/journal-crawl.json listed no profiled journals — the guard is silently inert"
    );
    assert!(
        !seed.is_empty(),
        "data/journal-seed.json carried no fingerprints — the guard is silently inert"
    );

    let only_config: Vec<&String> = config.difference(&seed).collect();
    let only_seed: Vec<&String> = seed.difference(&config).collect();

    assert!(
        only_config.is_empty() && only_seed.is_empty(),
        "the runtime's journal keys and the bundled snapshot's do not match.\n\n\
         `journal_profiles` looks up the CONFIG keys with `WHERE journal_key = ?1`; \
         `load_bundled_seed` writes the SEED keys. A key in only one list is a journal \
         whose requirements are stored and unreachable, and the picker renders it as \
         'Not crawled — structural checks only'.\n\n\
         in config/journal-crawl.json but never written by the seed: {only_config:?}\n\
         written by the seed but never looked up by the runtime:   {only_seed:?}\n\n\
         Fix by ALIGNING the two files — the seed's spelling is the de-facto key in the \
         data, so config is normally the side that moves. Do not add an alias table: the \
         key is read at three call sites (journal_profiles, run_publishready's \
         journal_key, journal_fingerprint) and a second mapping is a second thing to keep \
         correct."
    );
}
