//! **What the four commands that recorded a path now render in tracing
//! (Research OS decision 8, Phase 2B).** `detect_ai`, `run_stats_preview`,
//! `run_stats_verify` and `run_stats_chat` each recorded the file path in
//! their span (`path="…"`, through `Debug`) until Phase 2B. This runs each
//! one on files in a folder named `SPANPATH_SENTINEL_p55`, with
//! `SPANFILE_SENTINEL_…` in their names, and reads every span and event the
//! process produces: neither sentinel may appear, and each command's span
//! must have been seen, so the check is not vacuous.
//!
//! Each runs without a model or the network: `detect_ai` uses the heuristic
//! model it hard-codes, and `run_stats_chat` is asked to write analysis code,
//! which its firewall refuses before the proxy is probed or the keychain
//! read (asserted first, so the test cannot reach either).
//!
//! The source-side guard is `no_path_in_tracing.rs`. Alone in its binary on
//! purpose: its subscriber is the process's global default, so the chat
//! command's `spawn_blocking` thread is captured too.

mod l1_support;

use std::sync::{Arc, Mutex};

use app_lib::commands;
use gaply_core::stats_verdict::{
    AnalysisSpec, ColumnRoles, ReportedStatistic, TestKind, Tolerance,
};

const PATH: &str = "SPANPATH_SENTINEL_p55";
const FILE: &str = "SPANFILE_SENTINEL";
const QUESTION: &str = "Write the python code for my regression analysis";

#[test]
fn the_four_commands_record_no_path_or_file_name_in_tracing() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();
    assert!(
        gaply_core::chat_agent::is_ghostwriting(QUESTION),
        "the chat must stop at its firewall, before any network"
    );

    let root = std::env::temp_dir().join(format!("gaply-spans-{}", std::process::id()));
    let folder = root.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}_data.csv"));
    std::fs::write(
        &data,
        "score,group\n1.5,a\n2.5,b\n3.5,a\n4.5,b\n5.5,a\n6.5,b\n",
    )
    .unwrap();
    let manuscript = folder.join(format!("{FILE}_ms.txt"));
    std::fs::write(
        &manuscript,
        "Introduction\n\nWe compared two groups (t(4) = 1.2, p = .29).\n\nMethods\n\nA t-test.\n",
    )
    .unwrap();
    let (data_s, ms_s) = (
        data.to_string_lossy().into_owned(),
        manuscript.to_string_lossy().into_owned(),
    );
    let spec = AnalysisSpec {
        test_kind: TestKind::TTestWelch,
        roles: ColumnRoles::Grouped {
            value: "score".into(),
            group: "group".into(),
        },
        reported: ReportedStatistic::default(),
        tolerance: Tolerance::default(),
    };

    let ai = commands::detect_ai(ms_s.clone());
    let preview = commands::run_stats_preview(data_s.clone(), Some(ms_s.clone()));
    let verify = commands::run_stats_verify(data_s.clone(), spec.clone(), Some(ms_s.clone()));
    let chat = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(commands::run_stats_chat(
            data_s.clone(),
            spec,
            Some(ms_s.clone()),
            QUESTION.into(),
            None,
            None,
        ));
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        ai.is_ok() && preview.is_ok() && verify.is_ok() && chat.is_ok(),
        "each command ran"
    );

    let traced = lines.lock().unwrap().clone();
    for name in [
        "detect_ai",
        "run_stats_preview",
        "run_stats_verify",
        "run_stats_chat",
    ] {
        assert!(
            traced
                .iter()
                .any(|l| l.starts_with(&format!("span {name} "))),
            "{name}'s span was captured: {traced:?}"
        );
    }
    let leaks: Vec<&String> = traced
        .iter()
        .filter(|l| l.contains(PATH) || l.contains(FILE))
        .collect();
    println!("tracing lines: {}", traced.len());
    assert!(
        leaks.is_empty(),
        "a path or file name in tracing:\n{leaks:#?}"
    );
}
