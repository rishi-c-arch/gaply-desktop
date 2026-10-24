//! **The two exports of a project with reruns (Research OS, Phase 5C-1a;
//! decision 189).**
//!
//! A synthetic project: a summary (run 1) and a t-test (run 2), each run
//! again, run 3 repeating run 1 with the same output hash and run 4 repeating
//! run 2 with another, and run 5 a rerun of run 1 that failed. The different
//! case is made by changing run 4's stored output hash, never by changing an
//! engine. Its report and PROV JSON are pinned byte for byte in
//! `tests/fixtures/research_project/` (`report_reruns.md`,
//! `provenance_reruns.json`), beside the goldens of a project with no rerun,
//! which `research_project_render.rs` pins unchanged.
//!
//! * The report says decision 189's sentences, word for word, and for a
//!   rerun that did not succeed only "Repeats run {n}.".
//! * The PROV JSON records each rerun as its own activity with
//!   `wasInformedBy` the run it repeats, and has that relation only when a
//!   rerun exists.
//! * Decision 143 over both: a value from the data's cells is in neither,
//!   and the provenance file does not hold the project's name.
//! * No rerun sentence passes a verdict.

use gaply_core::contracts::{
    Arch, CapabilityId, ContentHash, Diagnostic, Os, ParamKey, ParamValue, Platform,
    Reproducibility, RunState,
};
use gaply_core::research_project::{
    render_prov_json, render_report_markdown, rerun_sentence, AgentVersion, ArtifactHash,
    ArtifactKind, ArtifactRef, DataFormat, ExportCode, ExportManifest, ExportRun,
    ProjectExportInput, ProjectId, ProjectTitle, ProvEdge, RerunComparison, StoredRun, StoredRunId,
    UsedRole,
};
use gaply_core::stats_desc::DescCode;
use gaply_core::stats_ttest::TTestCode;
use serde_json::Value;

const TITLE: &str = "Rerun TITLESENTz71";

/// Digits that appear nowhere else in either export: a value from the data.
const SENTINEL: i64 = 987_654_321;

fn h(c: char) -> ArtifactHash {
    ArtifactHash::parse(&c.to_string().repeat(64)).unwrap()
}

fn ch(c: char) -> ContentHash {
    ContentHash::from_hex(&c.to_string().repeat(64)).unwrap()
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

fn run(id: u64, cap: &str, state: RunState, rerun_of: Option<u64>) -> StoredRun {
    let t = 1_792_396_351 + 10 * id as i64;
    StoredRun {
        run_id: StoredRunId(id),
        capability_id: CapabilityId::parse(cap).unwrap(),
        capability_version: 1,
        state,
        end_code: None,
        agent: AgentVersion::parse("0.1.0").unwrap(),
        started_at: t,
        ended_at: Some(t + 1),
        saved_at: t + 5,
        rerun_of: rerun_of.map(StoredRunId),
    }
}

fn manifest(output: char) -> ExportManifest {
    ExportManifest {
        engine_version: 1,
        inputs: vec![ch('1')],
        parameters_hash: ch('b'),
        output_hash: ch(output),
        rows_read: 6,
        rows_used: 6,
        elapsed_ms: 3,
        platform: Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        },
        reproducibility: Reproducibility::Bitwise,
    }
}

fn export(r: StoredRun, manifest: Option<ExportManifest>, notes: Vec<ExportCode>) -> ExportRun {
    ExportRun {
        run: r,
        parameters: vec![],
        manifest,
        exclusions: vec![],
        limitations: vec![],
        notes,
        failure: None,
    }
}

fn edges(run: u64, data: char, spec: char, made: &[char]) -> Vec<ProvEdge> {
    let mut e = vec![
        ProvEdge::Used {
            run: StoredRunId(run),
            artifact: h(data),
            role: UsedRole::Data,
        },
        ProvEdge::Used {
            run: StoredRunId(run),
            artifact: h(spec),
            role: UsedRole::Spec,
        },
        ProvEdge::WasAssociatedWith {
            run: StoredRunId(run),
            agent: AgentVersion::parse("0.1.0").unwrap(),
        },
    ];
    for c in made {
        e.push(ProvEdge::WasGeneratedBy {
            artifact: h(*c),
            run: StoredRunId(run),
        });
    }
    e
}

/// Runs 1 and 2, their reruns 3 (identical) and 4 (different: its stored
/// output hash changed), and run 5, a rerun of run 1 that failed. A note
/// carrying a value from the data rides on run 3.
fn project() -> ProjectExportInput {
    let note = ExportCode::of(
        &Diagnostic::warning(DescCode::SuspectMissingCode)
            .with(ParamKey::Column, ParamValue::Int(1))
            .int(ParamKey::Value, SENTINEL)
            .int(ParamKey::Count, 2),
    );
    let mut failed = export(
        run(5, "stats.describe", RunState::Failed, Some(1)),
        None,
        vec![],
    );
    failed.failure = Some(ExportCode::of(&Diagnostic::error(
        TTestCode::BothGroupsConstant,
    )));
    let mut all_edges = Vec::new();
    all_edges.extend(edges(1, '1', '2', &['3', '4']));
    all_edges.extend(edges(2, '1', '5', &['6', '7']));
    all_edges.extend(edges(3, '1', '2', &['8', '4']));
    all_edges.extend(edges(4, '1', '5', &['a', 'c']));
    all_edges.extend(edges(5, '1', '2', &['d']));
    ProjectExportInput {
        title: ProjectTitle::from_typed(TITLE).unwrap(),
        project: ProjectId::parse("0123456789abcdef0123456789abcdef").unwrap(),
        created_at: 1_792_396_000,
        created_by: AgentVersion::parse("0.1.0").unwrap(),
        artifacts: vec![
            art('1', ArtifactKind::DataFile, DataFormat::Csv),
            art('2', ArtifactKind::OperationSpec, DataFormat::Json),
            art('3', ArtifactKind::ResultBundle, DataFormat::Json),
            art('4', ArtifactKind::RunManifest, DataFormat::Json),
            art('5', ArtifactKind::OperationSpec, DataFormat::Json),
            art('6', ArtifactKind::ResultBundle, DataFormat::Json),
            art('7', ArtifactKind::RunManifest, DataFormat::Json),
            art('8', ArtifactKind::ResultBundle, DataFormat::Json),
            art('a', ArtifactKind::ResultBundle, DataFormat::Json),
            art('c', ArtifactKind::RunManifest, DataFormat::Json),
            art('d', ArtifactKind::RunFailure, DataFormat::Json),
        ],
        runs: vec![
            export(
                run(1, "stats.describe", RunState::Succeeded, None),
                Some(manifest('e')),
                vec![],
            ),
            export(
                run(2, "stats.analyze.independent_t", RunState::Succeeded, None),
                Some(manifest('f')),
                vec![],
            ),
            export(
                run(3, "stats.describe", RunState::Succeeded, Some(1)),
                Some(manifest('e')),
                vec![note],
            ),
            // The different case: run 4's stored output hash is not run 2's.
            export(
                run(
                    4,
                    "stats.analyze.independent_t",
                    RunState::Succeeded,
                    Some(2),
                ),
                Some(manifest('9')),
                vec![],
            ),
            failed,
        ],
        edges: all_edges,
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/research_project")
        .join(name)
}

fn golden(name: &str, got: &[u8]) {
    let p = fixture(name);
    if std::env::var_os("GAPLY_WRITE_RENDER_GOLDEN").is_some() {
        std::fs::write(&p, got).unwrap();
    }
    let want = std::fs::read(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        String::from_utf8_lossy(got),
        String::from_utf8_lossy(&want),
        "{name}: the render and its golden file differ"
    );
}

/// The report's line about run `n`'s rerun, if it has one.
fn repeats_line(report: &str, n: u64) -> Option<String> {
    let section = report.split(&format!("\n## Run {n}:")).nth(1)?;
    let section = section.split("\n## Run ").next().unwrap();
    section
        .lines()
        .find(|l| l.starts_with("- Repeats run "))
        .map(str::to_string)
}

#[test]
fn the_report_says_decision_189s_sentences_for_each_rerun() {
    let report = render_report_markdown(&project());
    assert_eq!(repeats_line(&report, 1), None);
    assert_eq!(repeats_line(&report, 2), None);
    assert_eq!(
        repeats_line(&report, 3).as_deref(),
        Some("- Repeats run 1. Its result is identical, byte for byte, to run 1's.")
    );
    assert_eq!(
        repeats_line(&report, 4).as_deref(),
        Some("- Repeats run 2. Its result is not identical, byte for byte, to run 2's. Both are kept; Gaply does not judge which is right.")
    );
    assert_eq!(
        repeats_line(&report, 5).as_deref(),
        Some("- Repeats run 1."),
        "a rerun that did not succeed only says what it repeats"
    );
}

#[test]
fn the_prov_json_informs_each_rerun_by_the_run_it_repeats_and_only_then() {
    let prov: Value = serde_json::from_slice(&render_prov_json(&project()).unwrap()).unwrap();
    let informed = prov["wasInformedBy"]
        .as_object()
        .expect("a project with a rerun has wasInformedBy");
    let pairs: Vec<(String, String)> = informed
        .values()
        .map(|v| {
            (
                v["prov:informed"].as_str().unwrap().to_string(),
                v["prov:informant"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [
            ("gaply:run-3".to_string(), "gaply:run-1".to_string()),
            ("gaply:run-4".to_string(), "gaply:run-2".to_string()),
            ("gaply:run-5".to_string(), "gaply:run-1".to_string()),
        ]
    );
    // Each rerun is an activity of its own.
    for n in 1..=5 {
        assert!(
            prov["activity"][format!("gaply:run-{n}")].is_object(),
            "run {n}"
        );
    }
    // Without a rerun, no such relation.
    let mut none = project();
    none.runs.retain(|r| r.run.rerun_of.is_none());
    none.edges.retain(|e| match e {
        ProvEdge::Used { run, .. }
        | ProvEdge::WasGeneratedBy { run, .. }
        | ProvEdge::WasAssociatedWith { run, .. } => run.0 <= 2,
        ProvEdge::WasDerivedFrom { via, .. } => via.0 <= 2,
    });
    let prov: Value = serde_json::from_slice(&render_prov_json(&none).unwrap()).unwrap();
    assert!(prov.get("wasInformedBy").is_none());
}

#[test]
fn the_report_and_the_prov_json_of_a_project_with_reruns_match_their_golden_files() {
    let p = project();
    golden("report_reruns.md", render_report_markdown(&p).as_bytes());
    golden("provenance_reruns.json", &render_prov_json(&p).unwrap());
}

/// Decision 143 over both new goldens: no value from the data in either,
/// and the project's name only as the report's title.
#[test]
fn neither_rerun_golden_holds_a_value_from_the_data_and_the_prov_has_no_name() {
    let report = std::fs::read_to_string(fixture("report_reruns.md")).unwrap();
    let prov = std::fs::read_to_string(fixture("provenance_reruns.json")).unwrap();
    let digits = SENTINEL.to_string();
    for (name, text) in [("report", &report), ("provenance", &prov)] {
        assert!(
            !text.contains(&digits),
            "the {name} holds a value from the data"
        );
        assert!(!text.contains(", value "), "the {name} names a value");
    }
    assert!(
        report.contains("`stats.desc.suspect_missing_code` (column 1, count 2)"),
        "the note is there, by its column and count"
    );
    assert_eq!(report.matches("TITLESENTz71").count(), 1, "the title, once");
    assert!(report.starts_with("# Rerun TITLESENTz71\n"));
    assert!(
        !prov.contains("TITLESENTz71") && !prov.contains("Rerun TITLE"),
        "the provenance file does not hold the project's name"
    );
}

/// No rerun sentence passes a verdict, for any comparison and any run.
#[test]
fn no_rerun_sentence_passes_a_verdict() {
    let verdicts = [
        "reproduced",
        "reproducible",
        "verified",
        "correct",
        "failed to reproduce",
        "wrong",
        "valid",
        "confirmed",
    ];
    let mut said = Vec::new();
    for n in [1, 2, 37] {
        for c in [
            None,
            Some(RerunComparison::Identical),
            Some(RerunComparison::Different),
            Some(RerunComparison::NotCompared),
        ] {
            said.push(rerun_sentence(n, c));
        }
    }
    let report = render_report_markdown(&project());
    said.extend((1..=5).filter_map(|n| repeats_line(&report, n)));
    assert!(said.len() > 12);
    for s in &said {
        let lower = s.to_lowercase();
        for v in verdicts {
            assert!(!lower.contains(v), "{s:?} says {v:?}");
        }
    }
}
