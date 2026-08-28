//! Startup guarantees for the generative engine, plus the env-gated real-model
//! proof of the whole path.

#![cfg(test)]

use std::sync::atomic::AtomicBool;
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


/// citation_need against the REAL model on three seed cases.
///
/// Asserts only that each case COMPLETES — either a validated output or a clean
/// TaskError::ValidationFailed. It does NOT assert answer correctness: the
/// bundled 0.5B is expected to be weak, and a test that failed on a wrong answer
/// would be measuring the model, not the engine. Measuring the model is the eval
/// harness's job (`cargo run --bin ai-eval`).
#[tokio::test]
#[ignore = "needs the bundled GGUF; run with --ignored"]
async fn citation_need_smoke_with_the_real_model() {
    use crate::ai::task::TaskError;
    use crate::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask};

    if !gen_enabled() {
        eprintln!("SKIP: no generative model resolves");
        return;
    }
    let Some(loader) = BundledGenerativeLoader::resolve() else { return };
    let m = ModelManager::new(Arc::new(loader));

    let cases = [
        (
            "cn-seed-01 empirical claim / Introduction",
            CitationNeedInput {
                sentence: "Organic farming increases soil microbial biomass by roughly a third compared with conventional systems.".into(),
                preceding_sentence: "Soil health has become central to debates about agricultural sustainability.".into(),
                following_sentence: "These differences have consequences for long-term fertility.".into(),
                section: "Introduction".into(),
            },
        ),
        (
            "cn-seed-03 author's own result / Results",
            CitationNeedInput {
                sentence: "We observed a 12% increase in species richness in the treatment plots (p = 0.03).".into(),
                preceding_sentence: "Richness was compared across the three treatments.".into(),
                following_sentence: "No comparable effect appeared in the control.".into(),
                section: "Results".into(),
            },
        ),
        (
            "cn-seed-04 transition / Introduction",
            CitationNeedInput {
                sentence: "The next section turns to the methods used to collect these data.".into(),
                preceding_sentence: "These gaps motivate the present study.".into(),
                following_sentence: "Sampling took place over two growing seasons.".into(),
                section: "Introduction".into(),
            },
        ),
    ];

    println!("\n=== citation_need SMOKE (real model, correctness NOT asserted) ===");
    for (label, input) in cases {
        let task = CitationNeedTask::new(input);
        let started = std::time::Instant::now();
        let r = run_task(&m, &task, &TaskContext::default(), Arc::new(AtomicBool::new(false)), None).await;
        let ms = started.elapsed().as_millis();
        match r {
            Ok(run) => println!(
                "  {label}\n    OK ({ms} ms, retried {}) {}",
                run.retried,
                serde_json::to_string(&run.output).unwrap()
            ),
            Err(TaskError::ValidationFailed { errors, .. }) => println!(
                "  {label}\n    VALIDATION FAILED ({ms} ms): {}",
                errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
            ),
            Err(other) => panic!("{label}: the ENGINE failed, not the model: {other}"),
        }
    }
    assert_eq!(m.in_flight(), 0, "a smoke case leaked its lease");
}


/// citation_support against the REAL model on 3 seed shapes.
///
/// Asserts COMPLETION only — a validated card or a clean ValidationFailed. It
/// does NOT assert verdict correctness: the bundled 0.5B is expected to be weak
/// at a six-field nested schema, and a test that failed on a wrong verdict would
/// be measuring the model rather than the engine. What it DOES assert is the
/// guarantee that matters: nothing ungrounded was accepted.
#[tokio::test]
#[ignore = "needs the bundled GGUF; run with --ignored"]
async fn citation_support_smoke_with_the_real_model() {
    use crate::ai::evidence::{assemble, Assembled, EVIDENCE_BUDGET_TOKENS};
    use crate::ai::task::TaskError;
    use crate::ai::tasks::citation_support::CitationSupportTask;
    use gaply_core::ai_engine::{embeddings as core_emb, registry, store};
    use gaply_core::chunk::PagedChunk;
    use gaply_core::Database;

    if !gen_enabled() {
        eprintln!("SKIP: no generative model resolves");
        return;
    }
    let Some(loader) = BundledGenerativeLoader::resolve() else { return };
    let m = ModelManager::new(Arc::new(loader));

    let db = Database::in_memory().unwrap();
    let doc = store::create_document(&db, "Fixture", "/tmp/f.txt", "smoke-cs").unwrap();
    registry::register_model(
        &db,
        registry::ModelRow {
            id: "smoke-emb".into(),
            kind: "embedding".into(),
            display_name: "s".into(),
            file_path: "/x".into(),
            sha256: None,
            dim: Some(3),
            quant: None,
        },
    )
    .unwrap();
    let passages = [
        "Species richness rose 31 percent under organic management (p less than 0.01).",
        "Organic systems are widely discussed in the agronomic literature.",
        "No significant effect of management was observed for earthworm abundance.",
    ];
    let chunks: Vec<PagedChunk> = passages
        .iter()
        .enumerate()
        .map(|(i, t)| PagedChunk {
            seq: i as i64,
            content: (*t).to_string(),
            token_estimate: t.split_whitespace().count(),
            page: Some(i as u32 + 1),
            section: Some("Results".into()),
            char_start: 0,
            char_end: t.len(),
        })
        .collect();
    store::index_chunks(&db, doc, &chunks).unwrap();
    let space = core_emb::EmbeddingSpace::new("smoke-emb", "p1");
    let pending = core_emb::chunks_missing_embeddings(&db, None, "smoke-emb").unwrap();
    let rows: Vec<(i64, Vec<f32>)> =
        pending.iter().map(|p| (p.chunk_id, vec![1.0, 0.0, 0.0])).collect();
    core_emb::put_embeddings(&db, &space, &rows).unwrap();

    let claims = [
        ("supported", "Organic management increased species richness by about 31 percent."),
        ("wrong magnitude", "Organic management doubled species richness."),
        ("null result", "Organic management significantly increased earthworm abundance."),
    ];

    println!("\n=== citation_support SMOKE (real model; verdict NOT asserted) ===");
    let mut accepted = 0usize;
    for (label, claim) in claims {
        let Assembled::Ready(b) =
            assemble(&db, doc, claim, &[1.0, 0.0, 0.0], EVIDENCE_BUDGET_TOKENS).unwrap()
        else {
            panic!("{label}: expected evidence")
        };
        let task = CitationSupportTask {
            claim: claim.to_string(),
            cited_source: "Paired-fields study (2024) — Organic Management".into(),
            evidence: b.rendered.clone(),
        };
        let started = std::time::Instant::now();
        let r = run_task(&m, &task, &b.ctx, Arc::new(AtomicBool::new(false)), None).await;
        let ms = started.elapsed().as_millis();
        match r {
            Ok(run) => {
                accepted += 1;
                println!(
                    "  {label}\n    OK ({ms} ms, sent {} dropped {}, retried {}) {}",
                    b.chunks_sent,
                    b.chunks_dropped,
                    run.retried,
                    serde_json::to_string(&run.output).unwrap()
                );
                // THE guarantee: every cited chunk was sent, and every page
                // matches. The validator already enforced this; asserting it
                // here proves an ACCEPTED output cannot violate it.
                for sc in &run.output.supporting_chunks {
                    let sent = b
                        .ctx
                        .get(&sc.chunk_id)
                        .unwrap_or_else(|| panic!("{label}: ACCEPTED an unsent chunk {}", sc.chunk_id));
                    if let Some(p) = sc.page {
                        assert_eq!(
                            Some(p),
                            sent.page,
                            "{label}: ACCEPTED a page disagreeing with the store"
                        );
                    }
                }
            }
            Err(TaskError::ValidationFailed {
                errors,
                primary,
                first_raw,
                retry_raw,
                ..
            }) => {
                // VERBATIM, both attempts, undecorated. A summarised failure is
                // not evidence of what the model said — and the raw text is the
                // only place the failure mode is legible.
                println!(
                    "  {label}  (sent {} dropped {})\n    VALIDATION FAILED ({ms} ms, {primary:?} kept): {}",
                    b.chunks_sent,
                    b.chunks_dropped,
                    errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
                );
                println!("    --- first attempt, verbatim ---\n{first_raw}");
                println!("    --- retry, verbatim ---\n{retry_raw}");
                println!("    --- end ---");
            }
            Err(other) => panic!("{label}: the ENGINE failed, not the model: {other}"),
        }
    }
    println!("  accepted {accepted}/3 — ungrounded acceptances: 0 (asserted above)");
    assert_eq!(m.in_flight(), 0);
}


/// Measure the words-per-model-token ratio for EVIDENCE TEXT specifically,
/// using the real tokenizer. The first Phase 5 attempt derived this from a whole
/// prompt's token count, which folds in the schema and rules scaffolding and is
/// therefore not the ratio the evidence budget needs.
#[test]
#[ignore = "needs the bundled tokenizer; run with --ignored"]
fn measure_words_per_model_token_for_evidence_text() {
    let Some((_gguf, tok_path)) = crate::models::stage1_lm_paths() else {
        eprintln!("SKIP: no tokenizer resolves");
        return;
    };
    if !tok_path.exists() {
        eprintln!("SKIP: {} absent", tok_path.display());
        return;
    }
    let tok = tokenizers::Tokenizer::from_file(&tok_path).expect("tokenizer loads");

    let fixture = std::path::Path::new("evals/fixtures/organic_soil_biodiversity.txt");
    let Ok(text) = std::fs::read_to_string(fixture) else {
        eprintln!("SKIP: fixture absent");
        return;
    };
    // Render as the evidence block actually is: one chunk per paragraph, with
    // the spec header, since the header is part of what the budget pays for.
    let mut rendered = String::from("<evidence>\n");
    for (i, para) in text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| p.split_whitespace().count() >= 8)
        .enumerate()
    {
        rendered.push_str(&format!("[CHUNK_ID=c{} PAGE={} SECTION=Results] {}\n", i + 1, (i / 3) + 1, para.replace('\n', " ")));
    }
    rendered.push_str("</evidence>");

    let words = rendered.split_whitespace().count();
    let tokens = tok.encode(rendered.as_str(), false).expect("encode").get_ids().len();
    println!("\n=== EVIDENCE TEXT: words per model token ===");
    println!("words           : {words}");
    println!("model tokens    : {tokens}");
    println!("words per token : {:.3}", words as f64 / tokens as f64);
    println!("tokens per word : {:.3}", tokens as f64 / words as f64);
    assert!(words > 100 && tokens > 100);
}
