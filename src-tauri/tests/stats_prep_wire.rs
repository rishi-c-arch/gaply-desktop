//! **A prepared slot's view, from the Rust side (Research OS, Phase 8M-3,
//! decision 424).** It serialises to exactly the bytes of
//! `src/screens/statistics/fixtures/datasetViewPrepared.json`, deep-compared on
//! the TypeScript side with a copy typed `satisfies DatasetView`
//! (`describeTypes.vitest.ts`), so a renamed or missing key fails `tsc`. A file's
//! own table is `datasetView.json`, in `stats_desc_wire.rs`, unedited: neither
//! new field is written there.

use std::path::PathBuf;

use app_lib::stats_desc_commands::{ColumnView, DatasetView, ProfileView};
use gaply_core::contracts::{ContentHash, InputRef, Preparation, SemanticRole};
use gaply_core::stats_desc::{Alias, ColumnKind, ColumnProfile};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::ScoreCounts;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src/screens/statistics/fixtures/datasetViewPrepared.json")
}

fn hash(hex: &str) -> ContentHash {
    ContentHash::from_hex(hex).unwrap()
}

fn column(n: u32, header: &str, distinct: u64) -> ColumnProfile {
    ColumnProfile {
        alias: Alias(ColumnAlias(n)),
        header: header.into(),
        kind: ColumnKind::Numeric,
        n: 8,
        missing: 1,
        distinct,
        levels: None,
        suspect_missing_codes: Vec::new(),
        non_finite: 0,
    }
}

#[test]
fn a_prepared_view_serialises_to_exactly_its_fixtures_bytes() {
    let view = DatasetView {
        slot: 3,
        display_name: "survey.csv".into(),
        profile: ProfileView {
            rows: 9,
            columns: vec![
                ColumnView {
                    profile: column(1, "item", 3),
                    prepared: false,
                    score_name: None,
                },
                ColumnView {
                    profile: column(2, "C2", 4),
                    prepared: true,
                    score_name: Some("wellbeing".into()),
                },
            ],
        },
        input_ref: InputRef {
            content_hash: hash("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            rows: 9,
            columns: 2,
            semantic_role: SemanticRole::Table,
        },
        file_findings: Vec::new(),
        preparation: Some(Preparation {
            source_hash: hash("fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"),
            recipe_hash: hash("5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a"),
            source_columns: 1,
            capability_version: 1,
            engine_version: 1,
        }),
        score_counts: vec![ScoreCounts {
            column: Alias(ColumnAlias(2)),
            scored_from_every_item: 6,
            scored_from_fewer: 2,
            not_scored_too_few: 1,
            not_scored_overflow: 0,
        }],
    };
    let want = std::fs::read_to_string(fixture()).expect("the fixture");
    let got = serde_json::to_string_pretty(&view).unwrap() + "\n";
    assert_eq!(got, want, "the view and its fixture differ");
}
