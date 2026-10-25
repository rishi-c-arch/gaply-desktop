//! **Neither export holds a value from the user's data (Research OS, Phase
//! 5B-3a; decision 143).**
//!
//! A run's notes are its validation's warnings, and the missing-value warning
//! (`stats.desc.suspect_missing_code`, `stats.ttest.suspect_missing_code`)
//! names the value it found in the column's cells as its `value` parameter.
//! Decision 133 says each export holds fingerprints and none of the data, and
//! decision 134 keeps the ticked codes' values out for the same reason, so
//! the note is shown by its column's position and its cell count, never by
//! the value. The value stays inside the project's own artifacts.
//!
//! The test puts a sentinel in that field of a note, a failure and an
//! exclusion, for both capabilities, renders both exports, and looks for the
//! sentinel's digits anywhere in their bytes.

use gaply_core::contracts::{
    CapabilityId, ContentHash, Diagnostic, Exclusion, ParamKey, ParamValue, Platform,
    Reproducibility, RunState,
};
use gaply_core::research_project::{
    render_prov_json, render_report_markdown, AgentVersion, ArtifactHash, ArtifactKind,
    ArtifactRef, DataFormat, ExportCode, ExportCount, ExportManifest, ExportRun,
    ProjectExportInput, ProjectId, ProjectTitle, ProvEdge, StoredRun, StoredRunId, UsedRole,
};
use gaply_core::stats_desc::DescCode;
use gaply_core::stats_ttest::TTestCode;

/// Digits that appear nowhere else in either export.
const SENTINEL: i64 = 987_654_321;

fn h(c: char) -> ArtifactHash {
    ArtifactHash::parse(&c.to_string().repeat(64)).unwrap()
}

fn art(c: char, kind: ArtifactKind, format: DataFormat) -> ArtifactRef {
    ArtifactRef {
        hash: h(c),
        bytes: 100,
        kind,
        format,
        schema_version: 1,
        damage: None,
    }
}

fn run(id: u64, cap: &str, state: RunState) -> StoredRun {
    StoredRun {
        run_id: StoredRunId(id),
        capability_id: CapabilityId::parse(cap).unwrap(),
        capability_version: 1,
        state,
        end_code: None,
        agent: AgentVersion::parse("0.1.0").unwrap(),
        started_at: 1_792_396_351,
        ended_at: Some(1_792_396_352),
        saved_at: 1_792_396_400,
        rerun_of: None,
    }
}

fn manifest() -> ExportManifest {
    ExportManifest {
        engine_version: 1,
        inputs: vec![ContentHash::from_hex(&"1".repeat(64)).unwrap()],
        parameters_hash: ContentHash::from_hex(&"b".repeat(64)).unwrap(),
        output_hash: ContentHash::from_hex(&"c".repeat(64)).unwrap(),
        rows_read: 6,
        rows_used: 6,
        elapsed_ms: 3,
        platform: Platform::current(),
        reproducibility: Reproducibility::Bitwise,
    }
}

fn input() -> ProjectExportInput {
    let desc_note = Diagnostic::warning(DescCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(2))
        .int(ParamKey::Value, SENTINEL)
        .int(ParamKey::Count, 3);
    let ttest_note = Diagnostic::warning(TTestCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(1))
        .int(ParamKey::Value, -SENTINEL)
        .int(ParamKey::Count, 2);
    // No code carries a value as a failure or an exclusion today; the rule
    // holds there too, so a later one cannot.
    let failure = Diagnostic::error(TTestCode::BothGroupsConstant).int(ParamKey::Value, SENTINEL);
    let excluded = Exclusion {
        code: DescCode::MissingCell,
        count: 4,
        params: [
            (ParamKey::Column, ParamValue::Int(2)),
            (ParamKey::Value, ParamValue::Int(SENTINEL)),
        ]
        .into_iter()
        .collect(),
    };
    ProjectExportInput {
        title: ProjectTitle::from_typed("p").unwrap(),
        project: ProjectId::parse("0123456789abcdef0123456789abcdef").unwrap(),
        created_at: 1_792_396_000,
        created_by: AgentVersion::parse("0.1.0").unwrap(),
        artifacts: vec![
            art('1', ArtifactKind::DataFile, DataFormat::Csv),
            art('2', ArtifactKind::OperationSpec, DataFormat::Json),
            art('3', ArtifactKind::ResultBundle, DataFormat::Json),
            art('4', ArtifactKind::OperationSpec, DataFormat::Json),
            art('5', ArtifactKind::RunFailure, DataFormat::Json),
        ],
        runs: vec![
            ExportRun {
                run: run(1, "stats.describe", RunState::Succeeded),
                parameters: vec![],
                manifest: Some(manifest()),
                exclusions: vec![ExportCount::of(&excluded)],
                limitations: vec![],
                notes: vec![ExportCode::of(&desc_note)],
                failure: None,
            },
            ExportRun {
                run: run(2, "stats.analyze.independent_t", RunState::Failed),
                parameters: vec![],
                manifest: None,
                exclusions: vec![],
                limitations: vec![],
                notes: vec![ExportCode::of(&ttest_note)],
                failure: Some(ExportCode::of(&failure)),
            },
        ],
        edges: vec![
            ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('1'),
                role: UsedRole::Data,
            },
            ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('2'),
                role: UsedRole::Spec,
            },
            ProvEdge::WasGeneratedBy {
                artifact: h('3'),
                run: StoredRunId(1),
            },
            ProvEdge::Used {
                run: StoredRunId(2),
                artifact: h('1'),
                role: UsedRole::Data,
            },
            ProvEdge::Used {
                run: StoredRunId(2),
                artifact: h('4'),
                role: UsedRole::Spec,
            },
            ProvEdge::WasGeneratedBy {
                artifact: h('5'),
                run: StoredRunId(2),
            },
        ],
    }
}

#[test]
fn a_value_found_in_the_data_reaches_neither_export() {
    let input = input();
    let report = render_report_markdown(&input);
    let prov = String::from_utf8(render_prov_json(&input).expect("acyclic")).expect("utf-8");
    let digits = SENTINEL.to_string();
    for (name, text) in [("report", &report), ("provenance", &prov)] {
        assert!(
            !text.contains(&digits),
            "the {name} holds a value from the data's cells"
        );
        assert!(!text.contains(", value "), "the {name} names a value");
    }
    // The note is still there, by its column's position and its cell count.
    assert!(
        report.contains("`stats.desc.suspect_missing_code` (column 2, count 3)"),
        "{report}"
    );
    assert!(
        report.contains("`stats.ttest.suspect_missing_code` (column 1, count 2)"),
        "{report}"
    );
    assert!(
        report.contains("`stats.desc.missing_cell` (column 2): 4"),
        "{report}"
    );
}
