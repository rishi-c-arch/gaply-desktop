//! Startup guarantees for the generative engine, plus the env-gated real-model
//! proof of the whole path.

#![cfg(test)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::ai::generative::{estimate_ram, BundledGenerativeLoader, BUNDLED_GEN_MODEL_ID};
use crate::ai::model_manager::{GenState, ModelManager};
use crate::ai::task::{run_task, EchoTask, EvidenceChunk, TaskContext};

/// Constructing the manager must NOT load weights, and must not reach the
/// network. Asserted structurally: the manager reports NotLoaded, and the
/// module that owns the lifecycle contains no network identifier at all.
#[test]
fn startup_performs_no_generative_load_and_no_network() {
    // Resolving a path reads a directory entry; it does not map weights.
    let m = match BundledGenerativeLoader::resolve() {
        Some(loader) => ModelManager::new(Arc::new(loader)),
        None => {
            let m = ModelManager::new(Arc::new(crate::ai::model_manager::NullLoader));
            m.set_not_installed();
            m
        }
    };
    assert!(
        matches!(m.state(), GenState::NotLoaded | GenState::NotInstalled),
        "constructing the manager loaded the model: {:?}",
        m.state()
    );
    assert_eq!(m.in_flight(), 0);

    // Source-level guard: the generative lifecycle must never acquire anything
    // over the network. Model acquisition is ai_model_install's job alone (R4).
    for (name, src) in [
        ("model_manager.rs", include_str!("model_manager.rs")),
        ("generative.rs", include_str!("generative.rs")),
        ("task.rs", include_str!("task.rs")),
    ] {
        let body = src.split("mod tests").next().unwrap();
        for forbidden in ["reqwest", "TcpStream", "http://", "https://", "model_install::install"] {
            assert!(
                !body.contains(forbidden),
                "{name} references {forbidden:?} — the generative path must never touch the network"
            );
        }
    }
}

/// RAM must be derived from the file, never from the process.
#[test]
fn ram_is_never_process_rss() {
    for (name, src) in [
        ("generative.rs", include_str!("generative.rs")),
        ("model_manager.rs", include_str!("model_manager.rs")),
    ] {
        let body = src.split("mod tests").next().unwrap();
        // API identifiers only — the prose in these files says "never RSS", so
        // matching the word itself would flag the very comment that forbids it.
        for forbidden in
            ["getrusage", "/proc/self", "task_basic_info", "mach_task_self", "sysinfo::"]
        {
            assert!(
                !body.contains(forbidden),
                "{name} reads process memory via {forbidden} — RAM must come from GGUF metadata"
            );
        }
    }
}

fn gen_enabled() -> bool {
    std::env::var("GAPLY_TEST_GEN_MODEL").is_ok() || BundledGenerativeLoader::resolve().is_some()
}

/// The whole path with the REAL bundled model: prompt -> generate -> extract ->
/// parse -> validate -> typed result. `#[ignore]` because it maps ~400 MB of
/// weights and decodes on CPU.
#[tokio::test]
#[ignore = "needs the bundled GGUF; run with --ignored"]
async fn echo_task_end_to_end_with_the_real_model() {
    if !gen_enabled() {
        eprintln!("SKIP: no generative model resolves (set GAPLY_TEST_GEN_MODEL)");
        return;
    }
    let Some(loader) = BundledGenerativeLoader::resolve() else {
        eprintln!("SKIP: loader did not resolve");
        return;
    };
    let gguf = loader.gguf_path().to_path_buf();
    let ram = estimate_ram(&gguf).expect("gguf metadata must parse");
    let mb = |b: u64| b as f64 / 1024.0 / 1024.0;

    println!("\n=== PHASE 3 — REAL MODEL, END TO END ===");
    println!("model id : {BUNDLED_GEN_MODEL_ID}");
    println!("file     : {}", gguf.display());
    println!(
        "RAM      : weights {:.0} MB + kv {:.0} MB = {:.0} MB  (from GGUF metadata, never RSS)",
        mb(ram.weights_bytes),
        mb(ram.kv_cache_bytes),
        mb(ram.total_bytes)
    );

    let m = ModelManager::new(Arc::new(loader));
    assert_eq!(m.state(), GenState::NotLoaded, "manager loaded before it was asked to");

    let ctx = TaskContext::new(vec![EvidenceChunk {
        chunk_id: "c1".into(),
        page: Some(1),
        section: Some("Results".into()),
        text: "This is a development evidence chunk.".into(),
    }]);
    let task = EchoTask { phrase: "citation intelligence".into(), evidence: ctx.render_evidence() };

    let load_start = std::time::Instant::now();
    let run = run_task(&m, &task, &ctx, Arc::new(AtomicBool::new(false)), None)
        .await
        .unwrap_or_else(|e| panic!("EchoTask failed end to end: {e}"));
    let total_ms = load_start.elapsed().as_millis();

    println!("load+gen : {total_ms} ms total (first call includes the load)");
    println!("generate : {} ms, {} tokens", run.elapsed_ms, run.tokens);
    println!("retried  : {}", run.retried);
    println!("output   : {}", serde_json::to_string(&run.output).unwrap());
    println!("state    : {:?}, in flight {}", m.state(), m.in_flight());

    assert!(!run.output.echo.trim().is_empty(), "echo came back empty");
    assert_eq!(run.prompt_version, "echo-v1");
    assert_eq!(run.model_id, BUNDLED_GEN_MODEL_ID);
    // If it referenced a chunk at all, the validator already proved it was one
    // we supplied — an invented id would have failed the run.
    if let Some(id) = &run.output.chunk_id {
        assert_eq!(id, "c1");
    }
    assert_eq!(m.in_flight(), 0, "the run leaked its lease");
    assert!(matches!(m.state(), GenState::Idle { .. }), "state after run: {:?}", m.state());

    // A second call must reuse the loaded weights.
    let second = std::time::Instant::now();
    let run2 = run_task(&m, &task, &ctx, Arc::new(AtomicBool::new(false)), None).await.unwrap();
    println!("2nd call : {} ms (no reload)", second.elapsed().as_millis());
    assert!(!run2.output.echo.trim().is_empty());
}
