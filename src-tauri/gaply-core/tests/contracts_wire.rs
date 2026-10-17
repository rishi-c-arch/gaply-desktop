//! **The contracts on the wire (Research OS, Phase 2A).** Every contract
//! type round-trips through serde and refuses an unknown field; the checked
//! newtypes refuse what does not fit them; the RunState edge table is
//! exactly the D-entry's; and the code lists of `RunCode` and `DescCode`,
//! and the words of the run states and events, are pinned.

use std::collections::BTreeMap;
use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::{WorkspaceCode, WorkspaceEventKind, WorkspacePhase};
use gaply_core::contracts::{
    transition, Arch, CapabilityId, ContentHash, Diagnostic, Exclusion, InputRef, OperationId,
    OperationSpec, Os, ParamKey, ParamValue, Platform, Reproducibility, ResourceEstimate,
    ResultBundle, RunEvent, RunId, RunManifest, RunRecord, RunState, SchemaVersion, SemanticRole,
    Severity, ValidationReport, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_desc::{Alias, ColumnRequest, DescCode, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::ColumnAlias;

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(v: &T) -> Value {
    let json = serde_json::to_string(v).expect("serialises");
    let back: T =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{json} does not read back: {e}"));
    assert_eq!(&back, v, "{json}");
    serde_json::from_str(&json).expect("json")
}

/// `v`'s JSON with one more field must be refused.
fn refuses_extra_field<T: Serialize + DeserializeOwned + Debug>(v: &T) {
    let mut json = serde_json::to_value(v).expect("serialises");
    json.as_object_mut()
        .expect("an object")
        .insert("extraField".into(), Value::from(1));
    assert!(
        serde_json::from_value::<T>(json.clone()).is_err(),
        "accepted an unknown field: {json}"
    );
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn hash(byte: u8) -> ContentHash {
    ContentHash::of_bytes(&[byte])
}

fn params() -> DescribeParams {
    DescribeParams {
        columns: vec![
            ColumnRequest {
                alias: Alias(ColumnAlias(1)),
                treat_as: TreatAs::Numeric,
            },
            ColumnRequest {
                alias: Alias(ColumnAlias(12)),
                treat_as: TreatAs::Categorical,
            },
        ],
        missing_codes: vec![-99.0, 999.0],
    }
}

fn input() -> InputRef {
    InputRef {
        content_hash: hash(1),
        rows: 40,
        columns: 3,
        semantic_role: SemanticRole::Table,
    }
}

fn spec() -> OperationSpec<DescribeParams> {
    OperationSpec {
        schema_version: SchemaVersion,
        operation_id: OperationId(7),
        workspace_id: WorkspaceId::Statistics,
        capability_id: CapabilityId::parse("stats.describe").expect("id"),
        capability_version: 1,
        inputs: vec![input()],
        parameters: params(),
        random_seed: Some(20261017),
    }
}

fn diagnostic() -> Diagnostic<DescCode> {
    Diagnostic::warning(DescCode::SuspectMissingCode)
        .int(ParamKey::Column, 3)
        .int(ParamKey::Value, -99)
        .with(ParamKey::From, ParamValue::State(RunState::Queued))
        .with(ParamKey::Event, ParamValue::Event(RunEvent::Start))
        .with(ParamKey::Found, ParamValue::Code(DescCode::NoValues))
}

fn manifest() -> RunManifest<DescCode> {
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: CapabilityId::parse("stats.describe").expect("id"),
        capability_version: 1,
        engine_version: 1,
        inputs: vec![hash(1)],
        parameters_hash: hash(2),
        seed: None,
        rows_read: 40,
        rows_used: 40,
        exclusions: vec![Exclusion {
            code: DescCode::MissingCell,
            count: 4,
            params: BTreeMap::from([(ParamKey::Column, ParamValue::Int(1))]),
        }],
        platform: Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        },
        elapsed_ms: 3,
        output_hash: hash(3),
        reproducibility: Reproducibility::WithinTolerance { relative: 1e-12 },
    }
}

#[test]
fn every_contract_type_round_trips_and_structs_refuse_an_unknown_field() {
    let spec_json = round_trip(&spec());
    assert_eq!(
        keys(&spec_json),
        [
            "capabilityId",
            "capabilityVersion",
            "inputs",
            "operationId",
            "parameters",
            "randomSeed",
            "schemaVersion",
            "workspaceId"
        ]
    );
    assert_eq!(spec_json["schemaVersion"], 1);
    assert_eq!(spec_json["workspaceId"], "statistics");
    assert_eq!(spec_json["parameters"]["columns"][1]["alias"], "C12");
    assert_eq!(
        spec_json["parameters"]["columns"][1]["treatAs"],
        "categorical"
    );
    refuses_extra_field(&spec());
    refuses_extra_field(&params());
    refuses_extra_field(&params().columns[0]);

    assert_eq!(
        keys(&round_trip(&input())),
        ["columns", "contentHash", "rows", "semanticRole"]
    );
    refuses_extra_field(&input());

    let d = round_trip(&diagnostic());
    assert_eq!(d["code"], "stats.desc.suspect_missing_code");
    assert_eq!(d["params"]["from"], "queued");
    assert_eq!(d["params"]["event"], "start");
    assert_eq!(d["params"]["found"], "stats.desc.no_values");
    assert_eq!(d["params"]["value"], -99);
    refuses_extra_field(&diagnostic());

    let estimate = ResourceEstimate {
        rows: 40,
        columns: 2,
        cells: 80,
    };
    round_trip(&estimate);
    refuses_extra_field(&estimate);
    let report = ValidationReport::new(vec![], vec![], vec![], vec![diagnostic()], estimate);
    assert_eq!(report.status, ValidationStatus::ValidWithWarnings);
    assert_eq!(
        keys(&round_trip(&report)),
        [
            "errors",
            "estimatedResources",
            "missingInformation",
            "schemaVersion",
            "status",
            "warnings"
        ]
    );
    refuses_extra_field(&report);

    let record = RunRecord {
        schema_version: SchemaVersion,
        run_id: RunId(3),
        operation_id: OperationId(7),
        state: RunState::Running,
        started_ms: 10,
        ended_ms: None,
    };
    assert_eq!(
        keys(&round_trip(&record)),
        [
            "endedMs",
            "operationId",
            "runId",
            "schemaVersion",
            "startedMs",
            "state"
        ]
    );
    refuses_extra_field(&record);

    let m = round_trip(&manifest());
    assert_eq!(
        keys(&m),
        [
            "capabilityId",
            "capabilityVersion",
            "elapsedMs",
            "engineVersion",
            "exclusions",
            "inputs",
            "outputHash",
            "parametersHash",
            "platform",
            "reproducibility",
            "rowsRead",
            "rowsUsed",
            "schemaVersion",
            "seed"
        ]
    );
    assert_eq!(m["reproducibility"]["within_tolerance"]["relative"], 1e-12);
    refuses_extra_field(&manifest());
    refuses_extra_field(&manifest().exclusions[0]);
    refuses_extra_field(&manifest().platform);
    for r in [
        Reproducibility::Bitwise,
        Reproducibility::NotReproducible,
        Reproducibility::WithinTolerance { relative: 0.5 },
    ] {
        round_trip(&r);
    }
    let tolerance_with_extra =
        serde_json::json!({"within_tolerance": {"relative": 1e-12, "extra": 1}});
    assert!(serde_json::from_value::<Reproducibility>(tolerance_with_extra).is_err());

    // The envelope is generic: any typed result inside, any closed codes.
    let bundle: ResultBundle<Vec<u32>, RunCode> = ResultBundle {
        schema_version: SchemaVersion,
        run_id: RunId(3),
        status: RunState::Succeeded,
        primary_outputs: vec![1, 2, 3],
        diagnostics: vec![Diagnostic::note(RunCode::Cancelled)],
        limitations: vec![],
        manifest: manifest_with_codes(),
    };
    assert_eq!(
        keys(&round_trip(&bundle)),
        [
            "diagnostics",
            "limitations",
            "manifest",
            "primaryOutputs",
            "runId",
            "schemaVersion",
            "status"
        ]
    );
    refuses_extra_field(&bundle);

    for v in WorkspaceId::ALL {
        round_trip(v);
    }
    for v in [Severity::Error, Severity::Warning, Severity::Note] {
        round_trip(&v);
    }
    for v in [Os::Macos, Os::Windows, Os::Linux, Os::Other] {
        round_trip(&v);
    }
    for v in [Arch::Aarch64, Arch::X86_64, Arch::Other] {
        round_trip(&v);
    }
    for v in [
        ValidationStatus::Valid,
        ValidationStatus::ValidWithWarnings,
        ValidationStatus::NeedsUserInput,
        ValidationStatus::Unsupported,
        ValidationStatus::Invalid,
    ] {
        round_trip(&v);
    }
    round_trip(&SemanticRole::Table);
    round_trip(&SchemaVersion);
    round_trip(&OperationId(u64::MAX));
    round_trip(&RunId(0));
    round_trip(&hash(9));
    round_trip(&CapabilityId::parse("stats.describe").expect("id"));
    round_trip(&Platform::current());
    for s in RunState::ALL {
        round_trip(s);
    }
    for e in RunEvent::ALL {
        round_trip(e);
    }
    for c in RunCode::ALL {
        round_trip(c);
    }
    for c in DescCode::ALL {
        round_trip(c);
    }
    for t in [TreatAs::Numeric, TreatAs::Categorical] {
        round_trip(&t);
    }
    round_trip(&Alias(ColumnAlias(128)));
}

fn manifest_with_codes() -> RunManifest<RunCode> {
    let m = manifest();
    RunManifest {
        schema_version: m.schema_version,
        capability_id: m.capability_id,
        capability_version: m.capability_version,
        engine_version: m.engine_version,
        inputs: m.inputs,
        parameters_hash: m.parameters_hash,
        seed: m.seed,
        rows_read: m.rows_read,
        rows_used: m.rows_used,
        exclusions: vec![],
        platform: m.platform,
        elapsed_ms: m.elapsed_ms,
        output_hash: m.output_hash,
        reproducibility: m.reproducibility,
    }
}

#[test]
fn the_checked_newtypes_refuse_what_does_not_fit_them() {
    let read = |json: &str| serde_json::from_str::<OperationSpec<DescribeParams>>(json);
    let good = serde_json::to_value(spec()).expect("json");
    let with = |key: &str, v: Value| {
        let mut j = good.clone();
        j[key] = v;
        read(&j.to_string())
    };
    assert!(read(&good.to_string()).is_ok());
    assert!(
        with("schemaVersion", Value::from(2)).is_err(),
        "schema version 2"
    );
    assert!(
        with("schemaVersion", Value::from(0)).is_err(),
        "schema version 0"
    );
    assert!(
        with("workspaceId", Value::from("geometry")).is_err(),
        "an unknown workspace"
    );
    // Phase 3: Compute is a workspace id, so the spec reads; the app refuses
    // it with `workspace.wrong_workspace` (src-tauri/tests/workspace_session.rs)
    // and `stats_desc::validate` with `unknown_capability`.
    assert!(
        with("workspaceId", Value::from("compute")).is_ok(),
        "a known workspace reads"
    );

    for id in ["stats.describe", "stats.desc_summary", "stats.a.b.c"] {
        assert!(CapabilityId::parse(id).is_some(), "{id}");
        assert!(with("capabilityId", Value::from(id)).is_ok(), "{id}");
    }
    for id in [
        "/Users/someone/thesis/data.csv",
        "C:\\data\\x.csv",
        "data.csv",
        "my data.describe",
        "Stats.describe",
        "stats.Describe",
        "stats.describe2",
        "stats.describe-v2",
        "stats",
        "stats.",
        ".describe",
        "stats..describe",
        "stats.a.b.c.d",
        "compute.solve",
        "stats.describe ",
        "",
    ] {
        assert!(CapabilityId::parse(id).is_none(), "accepted {id:?}");
        assert!(
            with("capabilityId", Value::from(id)).is_err(),
            "accepted {id:?} on the wire"
        );
    }
    assert!(
        CapabilityId::parse(&format!("stats.{}", "a".repeat(33))).is_none(),
        "a 33-letter word"
    );
    assert_eq!(
        CapabilityId::parse("stats.describe")
            .expect("id")
            .workspace(),
        WorkspaceId::Statistics
    );

    let hex = hash(1).as_hex().to_string();
    assert_eq!(hex.len(), 64);
    assert!(ContentHash::from_hex(&hex).is_some());
    for bad in [
        hex.to_uppercase(),
        hex[..63].to_string(),
        format!("{hex}0"),
        format!("{}g", &hex[..63]),
        "/tmp/x".to_string(),
    ] {
        assert!(ContentHash::from_hex(&bad).is_none(), "{bad}");
        assert!(
            serde_json::from_value::<ContentHash>(Value::from(bad.clone())).is_err(),
            "{bad}"
        );
    }
    assert_eq!(
        format!("{:?}", hash(1)),
        "ContentHash(..)",
        "a content hash is never printed"
    );
    assert!(
        !format!("{:?}", manifest()).contains(&hex),
        "a manifest's Debug prints no hash"
    );

    for good in ["C1", "C9", "C10", "C128", "C4294967295"] {
        assert!(
            serde_json::from_value::<Alias>(Value::from(good)).is_ok(),
            "{good}"
        );
    }
    for bad in [
        "C0",
        "C01",
        "c1",
        "C",
        "1",
        "Cx",
        "C-1",
        "C 1",
        "C4294967296",
        "age",
    ] {
        assert!(
            serde_json::from_value::<Alias>(Value::from(bad)).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_refused_word_is_not_echoed() {
    for bad in [
        "stats.desc.SENTINEL_9f2c",
        "run.SENTINEL_9f2c",
        "SENTINEL_9f2c",
    ] {
        let e = serde_json::from_value::<DescCode>(Value::from(bad))
            .expect_err("refused")
            .to_string();
        assert!(!e.contains("SENTINEL"), "{e}");
        let e = serde_json::from_value::<RunState>(Value::from(bad))
            .expect_err("refused")
            .to_string();
        assert!(!e.contains("SENTINEL"), "{e}");
    }
    let e = serde_json::from_value::<CapabilityId>(Value::from("stats.SENTINEL"))
        .expect_err("refused")
        .to_string();
    assert!(!e.contains("SENTINEL"), "{e}");
    let e = serde_json::from_value::<Alias>(Value::from("SENTINEL"))
        .expect_err("refused")
        .to_string();
    assert!(!e.contains("SENTINEL"), "{e}");
}

/// The D-entry's table: blueprint §8.5, and nothing else.
const EDGES: [(RunState, RunEvent, RunState); 18] = {
    use RunEvent as E;
    use RunState as S;
    [
        (S::Draft, E::BeginValidation, S::Validating),
        (S::Validating, E::RequestInput, S::NeedsUserInput),
        (S::Validating, E::Refuse, S::Refused),
        (S::Validating, E::PassValidation, S::Validated),
        (
            S::Validated,
            E::RequireConfirmation,
            S::AwaitingConfirmation,
        ),
        (S::Validated, E::Enqueue, S::Queued),
        (S::AwaitingConfirmation, E::Confirm, S::Queued),
        (S::AwaitingConfirmation, E::Cancel, S::Cancelled),
        (S::AwaitingConfirmation, E::Invalidate, S::Invalidated),
        (S::Queued, E::Start, S::Running),
        (S::Queued, E::Cancel, S::Cancelled),
        (S::Queued, E::Refuse, S::Refused),
        (S::Running, E::Succeed, S::Succeeded),
        (S::Running, E::Fail, S::Failed),
        (S::Running, E::Cancel, S::Cancelled),
        (S::Running, E::TimeOut, S::TimedOut),
        (S::Running, E::ExceedResources, S::ResourceLimit),
        (S::Running, E::FailNumerically, S::NumericalFailure),
    ]
};

const TERMINAL: [RunState; 9] = [
    RunState::NeedsUserInput,
    RunState::Refused,
    RunState::Invalidated,
    RunState::Succeeded,
    RunState::Failed,
    RunState::Cancelled,
    RunState::TimedOut,
    RunState::ResourceLimit,
    RunState::NumericalFailure,
];

#[test]
fn every_listed_edge_succeeds_and_every_other_pair_is_refused() {
    assert_eq!(RunState::ALL.len(), 15);
    assert_eq!(RunEvent::ALL.len(), 15);
    let (mut allowed, mut refused) = (0, 0);
    for &from in RunState::ALL {
        for &event in RunEvent::ALL {
            let listed = EDGES
                .iter()
                .find(|(f, e, _)| *f == from && *e == event)
                .map(|(_, _, to)| *to);
            match (transition(from, event), listed) {
                (Ok(to), Some(want)) => {
                    assert_eq!(to, want, "{from:?} --{event:?}-->");
                    allowed += 1;
                }
                (Err(d), None) => {
                    let code = if TERMINAL.contains(&from) {
                        RunCode::AlreadyTerminal
                    } else {
                        RunCode::IllegalTransition
                    };
                    assert_eq!(d.code, code, "{from:?} --{event:?}");
                    assert_eq!(d.severity, Severity::Error);
                    assert_eq!(
                        d.params.get(&ParamKey::From),
                        Some(&ParamValue::State(from))
                    );
                    assert_eq!(
                        d.params.get(&ParamKey::Event),
                        Some(&ParamValue::Event(event))
                    );
                    refused += 1;
                }
                (got, want) => panic!("{from:?} --{event:?}: got {got:?}, the table says {want:?}"),
            }
        }
    }
    assert_eq!((allowed, refused), (18, 15 * 15 - 18));

    for &s in RunState::ALL {
        assert_eq!(s.is_terminal(), TERMINAL.contains(&s), "{s:?}");
        let leaves = EDGES.iter().any(|(f, _, _)| *f == s);
        assert_eq!(
            leaves,
            !s.is_terminal(),
            "{s:?}: a terminal state has no way out, every other has one"
        );
    }

    // Every state of §8.5 is reachable from Draft through the table.
    let mut reached = vec![RunState::Draft];
    let mut i = 0;
    while i < reached.len() {
        for (f, _, to) in EDGES {
            if f == reached[i] && !reached.contains(&to) {
                reached.push(to);
            }
        }
        i += 1;
    }
    assert_eq!(
        reached.len(),
        15,
        "unreachable: {:?}",
        RunState::ALL
            .iter()
            .filter(|s| !reached.contains(s))
            .collect::<Vec<_>>()
    );

    // A run that is cancelled is never reported as completed.
    let running = transition(
        transition(RunState::Queued, RunEvent::Start).expect("start"),
        RunEvent::Cancel,
    )
    .expect("cancel");
    assert_eq!(running, RunState::Cancelled);
    assert!(transition(running, RunEvent::Succeed).is_err());
}

#[test]
fn the_code_and_word_lists_are_pinned() {
    assert_eq!(
        RunCode::ALL.iter().map(|c| c.wire()).collect::<Vec<_>>(),
        [
            "run.unknown_run",
            "run.run_busy",
            "run.unknown_input",
            "run.not_finished",
            "run.no_result",
            "run.cancelled",
            "run.timed_out",
            "run.resource_limit",
            "run.illegal_transition",
            "run.already_terminal",
        ]
    );
    assert_eq!(
        DescCode::ALL.iter().map(|c| c.wire()).collect::<Vec<_>>(),
        [
            "stats.desc.unknown_capability",
            "stats.desc.capability_version_mismatch",
            "stats.desc.input_count",
            "stats.desc.input_mismatch",
            "stats.desc.no_columns_selected",
            "stats.desc.unknown_column",
            "stats.desc.duplicate_column",
            "stats.desc.numeric_on_text",
            "stats.desc.non_finite_cell",
            "stats.desc.missing_code_not_finite",
            "stats.desc.suspect_missing_code",
            "stats.desc.no_values",
            "stats.desc.single_value",
            "stats.desc.overflow",
            "stats.desc.underflow",
            "stats.desc.arithmetic_refused",
            "stats.desc.missing_cell",
            "stats.desc.missing_code",
        ]
    );
    assert_eq!(
        RunState::ALL.iter().map(|c| c.wire()).collect::<Vec<_>>(),
        [
            "draft",
            "validating",
            "needs_user_input",
            "awaiting_confirmation",
            "refused",
            "validated",
            "invalidated",
            "queued",
            "running",
            "succeeded",
            "failed",
            "cancelled",
            "timed_out",
            "resource_limit",
            "numerical_failure",
        ]
    );
    assert_eq!(
        RunEvent::ALL.iter().map(|c| c.wire()).collect::<Vec<_>>(),
        [
            "begin_validation",
            "request_input",
            "refuse",
            "pass_validation",
            "require_confirmation",
            "enqueue",
            "confirm",
            "invalidate",
            "cancel",
            "start",
            "succeed",
            "fail",
            "time_out",
            "exceed_resources",
            "fail_numerically",
        ]
    );
    assert_eq!(RunCode::NAMESPACE, "run.");
    assert_eq!(DescCode::NAMESPACE, "stats.desc.");
    assert!(DescCode::NAMESPACE.starts_with(WorkspaceId::Statistics.namespace()));

    // Each word is in exactly one set, so an untagged parameter reads back
    // as what it was.
    let mut all: Vec<&str> = RunCode::ALL.iter().map(|c| c.wire()).collect();
    all.extend(DescCode::ALL.iter().map(|c| c.wire()));
    all.extend(RunState::ALL.iter().map(|c| c.wire()));
    all.extend(RunEvent::ALL.iter().map(|c| c.wire()));
    // Phase 3's parameter words: workspace ids, phases and events.
    let workspaces: Vec<String> = WorkspaceId::ALL
        .iter()
        .map(|w| {
            serde_json::to_value(w)
                .expect("json")
                .as_str()
                .expect("a word")
                .to_string()
        })
        .collect();
    all.extend(workspaces.iter().map(String::as_str));
    all.extend(WorkspacePhase::ALL.iter().map(|c| c.wire()));
    all.extend(WorkspaceEventKind::ALL.iter().map(|c| c.wire()));
    all.extend(WorkspaceCode::ALL.iter().map(|c| c.wire()));
    let n = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), n, "a word is in two sets");
    for c in RunCode::ALL {
        assert!(
            c.wire().starts_with(RunCode::NAMESPACE)
                && serde_json::to_value(c).expect("json") == c.wire()
        );
    }
    for c in DescCode::ALL {
        assert!(
            c.wire().starts_with(DescCode::NAMESPACE)
                && serde_json::to_value(c).expect("json") == c.wire()
        );
    }
}

#[test]
fn a_validation_status_follows_the_most_serious_finding() {
    let e = ResourceEstimate {
        rows: 1,
        columns: 1,
        cells: 1,
    };
    let d = |c| Diagnostic::error(c);
    assert_eq!(
        ValidationReport::<DescCode>::new(vec![], vec![], vec![], vec![], e).status,
        ValidationStatus::Valid
    );
    assert_eq!(
        ValidationReport::new(
            vec![],
            vec![],
            vec![],
            vec![d(DescCode::SuspectMissingCode)],
            e
        )
        .status,
        ValidationStatus::ValidWithWarnings
    );
    assert_eq!(
        ValidationReport::new(
            vec![],
            vec![],
            vec![d(DescCode::NoColumnsSelected)],
            vec![],
            e
        )
        .status,
        ValidationStatus::NeedsUserInput
    );
    assert_eq!(
        ValidationReport::new(
            vec![],
            vec![d(DescCode::UnknownColumn)],
            vec![d(DescCode::NoColumnsSelected)],
            vec![],
            e
        )
        .status,
        ValidationStatus::Invalid
    );
    let r = ValidationReport::new(
        vec![d(DescCode::UnknownCapability)],
        vec![d(DescCode::UnknownColumn)],
        vec![],
        vec![],
        e,
    );
    assert_eq!(r.status, ValidationStatus::Unsupported);
    assert_eq!(
        r.errors.len(),
        2,
        "unsupported findings are reported among the errors"
    );
    assert!(!r.may_run());
}
