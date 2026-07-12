//! Gap Finder Set-8 memory proof — MEASUREMENT ONLY, no product behavior.
//!
//! Drives the FULL gapfinder flow's LOCAL work under a 50ms RSS sampler:
//! worst-case corpus (8 near-cap ~19MB papers) → RAG ingest → gap payload +
//! gate (MockProxyClient) → Q&A turn → draft turn → journal verification
//! (MockHttpFetcher fixtures) → fit turn. Unlike PublishReady there is NO
//! local model anywhere in this flow — the probe VERIFIES that too: app RSS
//! stays text-processing-scale (no candle load) and Ollama's /api/ps is
//! polled to confirm qwen3 never becomes resident.
//!
//!   cargo run --release --example gapfinder_flow_mem_probe

use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gaply_core::embed::HashEmbedder;
use gaply_core::gap_finder_agent::{self, ResearcherConstraints};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::MockHttpFetcher;
use gaply_core::verify_agent::MockProxyClient;
use gaply_core::{Database, GaplyError};

use app_lib::journal_registry;
use app_lib::paper_corpus::{build_corpus, PaperFetch, MAX_PAPERS};

fn own_rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

fn ollama_resident_mb() -> f64 {
    let out = Command::new("curl")
        .args(["-s", "--max-time", "2", "http://127.0.0.1:11434/api/ps"])
        .output();
    let body = match out.ok().and_then(|o| String::from_utf8(o.stdout).ok()) {
        Some(b) if !b.is_empty() => b,
        _ => return 0.0,
    };
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v["models"].as_array().map(|a| {
                a.iter().map(|m| m["size"].as_f64().unwrap_or(0.0)).sum::<f64>() / 1e6
            })
        })
        .unwrap_or(0.0)
}

struct NoFetch;
impl PaperFetch for NoFetch {
    fn get_capped(&self, _url: &str, _cap: usize) -> Result<(u16, Vec<u8>), GaplyError> {
        Ok((404, Vec::new()))
    }
}

fn write_paper(name: &str, megabytes: usize) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("gaply_gf8_{name}.txt"));
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, "Probe Paper {name}: Sleep and Memory\n\nAbstract\nWe study sleep with n = 96 and p = 0.002.\n").unwrap();
    let para = "Long study prose repeated to reach the target size, discussing sleep, memory, \
methods and results with statistics such as t(95) = 3.1 across waves. "
        .repeat(64);
    for _ in 0..(megabytes * 100) {
        f.write_all(para.as_bytes()).unwrap();
        f.write_all(b"\n\n").unwrap();
    }
    p
}

fn main() {
    let stop = Arc::new(AtomicBool::new(false));
    let peak_app = Arc::new(Mutex::new(own_rss_mb()));
    let peak_ollama = Arc::new(Mutex::new(0f64));
    let sampler = {
        let (stop, peak_app, peak_ollama) = (stop.clone(), peak_app.clone(), peak_ollama.clone());
        thread::spawn(move || {
            let mut tick = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let r = own_rss_mb();
                let mut p = peak_app.lock().unwrap();
                if r > *p {
                    *p = r;
                }
                drop(p);
                // Ollama poll every ~1s (curl is comparatively expensive).
                if tick % 20 == 0 {
                    let o = ollama_resident_mb();
                    let mut po = peak_ollama.lock().unwrap();
                    if o > *po {
                        *po = o;
                    }
                }
                tick += 1;
                thread::sleep(Duration::from_millis(50));
            }
        })
    };

    let baseline = own_rss_mb();
    println!("baseline RSS: {baseline:.0} MB; ollama resident now: {:.0} MB", ollama_resident_mb());

    // ---- Stage 1: worst-case corpus (8 × ~19MB) + RAG ingest --------------
    let paths: Vec<String> = (0..MAX_PAPERS)
        .map(|i| write_paper(&format!("near{i}"), 19).to_string_lossy().into_owned())
        .collect();
    let db = Arc::new(Database::in_memory().unwrap());
    let corpus_report = build_corpus(&db, &HashEmbedder, "gf8", &paths, &[], &NoFetch, &RateLimiter::new(100.0, 100.0))
        .expect("corpus builds");
    println!("corpus: {} digests — RSS now {:.0} MB", corpus_report.digests.len(), own_rss_mb());
    let corpus = serde_json::to_value(&corpus_report).unwrap();

    // ---- Stage 2: gap payload + gate (mock cloud) -------------------------
    let proxy = MockProxyClient::returning(serde_json::json!({
        "gaps": [{ "description": "field dose-response gap", "rationale": "r", "paper_refs": ["p1"] }],
        "suggestions": []
    }));
    let findings = gap_finder_agent::find_gaps(&proxy, "gf8", &corpus).expect("gaps");
    println!("gaps: {} grounded — RSS {:.0} MB", findings.grounded_gaps.len(), own_rss_mb());
    let gaps = serde_json::to_value(&findings.grounded_gaps).unwrap();

    // ---- Stage 3: Q&A turn (mock) -----------------------------------------
    let qa_proxy = MockProxyClient::returning(serde_json::json!({
        "updated_constraints": { "funding_level": "small grant" },
        "follow_up_question": "",
        "achievable_gaps": [{ "gap_ref": "g1", "description": "narrowed", "rationale": "fits" }]
    }));
    let turn = gap_finder_agent::qa_turn(
        Some(&qa_proxy), "gf8", &corpus, &gaps,
        &serde_json::to_value(ResearcherConstraints::default()).unwrap(), "we have a small grant",
    )
    .expect("qa turn");
    println!("qa: {} achievable — RSS {:.0} MB", turn.achievable_gaps.len(), own_rss_mb());
    let achievable = serde_json::to_value(&turn.achievable_gaps).unwrap();

    // ---- Stage 4: draft turn (mock) ---------------------------------------
    let draft_proxy = MockProxyClient::returning(serde_json::json!({
        "drafts": [{ "gap_ref": "g1", "objectives": ["short objective"], "methodology_steps": ["short step"] }]
    }));
    let draft = gap_finder_agent::draft_turn(
        Some(&draft_proxy), "gf8", &corpus, &achievable,
        &serde_json::to_value(&turn.constraints).unwrap(), "draft the design",
    )
    .expect("draft turn");
    println!("draft: {} scaffolds — RSS {:.0} MB", draft.drafts.len(), own_rss_mb());

    // ---- Stage 5: journal verification (fixture fetcher) + fit ------------
    let fetcher = MockHttpFetcher::new()
        .route("api.openalex.org", 200, r#"{"results":[{"display_name":"J Sleep Res","issn_l":"1365-2869","is_in_doaj":true,"works_count":5120,"counts_by_year":[{"year":2026,"works_count":180}],"x_concepts":[{"display_name":"Sleep medicine"}]}]}"#)
        .route("doaj.org", 200, r#"{"total":1,"results":[{}]}"#);
    let card = journal_registry::verify_journal(&db, &fetcher, &RateLimiter::new(100.0, 100.0), "1365-2869", "J Sleep Res", &[])
        .expect("journal card");
    let fit_proxy = MockProxyClient::returning(serde_json::json!({
        "fits": [{ "gap_ref": "g1", "journal_ref": "j1", "verdict": "good_fit", "reasoning": "in scope" }]
    }));
    let fit = gap_finder_agent::fit_turn(
        Some(&fit_proxy), "gf8", &corpus, &achievable, &serde_json::to_value(&card).unwrap(),
    )
    .expect("fit turn");
    println!("journal verified + fit: {} verdicts — RSS {:.0} MB", fit.fits.len(), own_rss_mb());

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    for p in paths {
        let _ = std::fs::remove_file(p);
    }

    let peak_app = *peak_app.lock().unwrap();
    let peak_ollama = *peak_ollama.lock().unwrap();
    println!("\n========== GAPFINDER FLOW MEMORY REPORT ==========");
    println!("peak app RSS (full local flow, 8×19MB corpus): {peak_app:.0} MB");
    println!("peak Ollama resident during the flow           : {peak_ollama:.0} MB (must be 0 — no model in this flow)");
    println!("physical RAM                                   : 8192 MB");
    println!("margin (8192 - peak app)                       : {:.0} MB", 8192.0 - peak_app);
    println!("==================================================");
    if peak_ollama > 1.0 {
        println!("WARNING: a model was resident during the gapfinder flow — investigate!");
        std::process::exit(1);
    }
}
