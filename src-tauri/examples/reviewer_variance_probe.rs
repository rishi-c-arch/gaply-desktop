//! **How much does the reviewer letter a user receives change between runs?**
//!
//! §11 D204's OPEN item: `gaply-proxy/app/openai_client.py` builds
//! `{model, max_tokens, messages}` and never sets `temperature`, so every cloud
//! reply is sampled at the provider default of 1.0. §11 D205 then showed that
//! sampling spread can straddle a decision threshold. `reviewer_agent` reaches
//! the cloud through the same `ProxyClient`, so the letter a researcher reads is
//! sampled the same way — and nothing has ever measured how much two runs of it
//! differ.
//!
//! **This measures BEFORE anything is changed.** Setting a temperature first
//! would erase the evidence of what users have been getting.
//!
//! # What is held fixed, so the only variable is the sampling
//!
//! The pipeline runs ONCE and `build_review_payload` is called ONCE; the same
//! bytes are then sent N times. `build_review_payload` is documented as
//! deterministic (`reviewer_agent.rs:492` — `serde_json::Map` is a `BTreeMap`,
//! so key order is stable and equal inputs give equal bytes), and the probe
//! prints the payload's SHA-256 so that claim is checked here rather than
//! trusted. Any variation in the output is therefore the model's, not the
//! pipeline's.
//!
//! # Deviations from `run_publishready_measured`, stated
//!
//! * The App Check signer is built from `GRRB_APP_CHECK_KEY` instead of the
//!   macOS keychain (`ProxyReqwestClient::from_env` -> `TokenSigner::from_keychain`).
//!   Where the signing key came from has no bearing on what the model replies.
//! * The pipeline runs with `NetworkConsent::Denied`, so the payload is built
//!   from a fully local report. That fixes ONE payload; it is not a claim about
//!   what a consent-granted report would contain.
//! * Everything downstream of that is the shipped path: the same
//!   `build_review_payload`, the same `verify_with_envelope` call
//!   (`commands.rs:1004-1027`), and the same `gate_reviewer_response`.
//!
//! # Body capture
//!
//! `publication_probability` reaches no screen and no export (audited 22 Sep
//! 2026), but `body` is model prose rendered verbatim, so a percentage could
//! still reach a reader through it. The probe writes each body to
//! `GRRB_BODY_DIR` when set and always reports `pct_in_body` /
//! `prob_word_in_body`, so that question is answered by the run rather than
//! left open. The first baseline could not answer it: it hashed the body and
//! threw the text away.
//!
//! ```text
//! GRRB_APP_CHECK_KEY=<key> GAPLY_PROXY_URL=http://127.0.0.1:8080 \
//! GRRB_BODY_DIR=/tmp/reviewer-bodies \
//!   cargo run --release --example reviewer_variance_probe -- <manuscript> [runs]
//! ```
use gaply_core::app_check::{TokenSigner, DEFAULT_APP_ID};
use gaply_core::reviewer_agent::{self, TargetJournal};
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::Database;
use std::sync::Arc;

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>()[..16].to_string()
}

fn main() -> Result<(), gaply_core::GaplyError> {
    let path = std::env::args().nth(1).expect("usage: reviewer_variance_probe <manuscript> [runs]");
    let runs: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(5);
    let key = std::env::var("GRRB_APP_CHECK_KEY")
        .expect("GRRB_APP_CHECK_KEY must match the proxy's APP_CHECK_SIGNING_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());

    // ---- ONE pipeline run, ONE payload -------------------------------------
    let db = Arc::new(Database::in_memory()?);
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
    let events = std::cell::RefCell::new(Vec::new());
    let emit = |e: app_lib::pipeline::AnalysisEvent| events.borrow_mut().push(e);
    eprintln!("running the pipeline on {path} ...");
    let t0 = std::time::Instant::now();
    app_lib::pipeline::run_pipeline_measured(
        db.clone(),
        embedder.clone(),
        path.clone(),
        None,
        None,
        None,
        None,
        app_lib::pipeline::NetworkConsent::Denied,
        &emit,
    )?;
    let report_id = events
        .into_inner()
        .into_iter()
        .find_map(|e| match e {
            app_lib::pipeline::AnalysisEvent::Finished { report_id } => Some(report_id),
            _ => None,
        })
        .expect("pipeline finished without a report id");
    let json = db
        .cache_get(&app_lib::pipeline::report_cache_key(&report_id), gaply_core::now_epoch())?
        .expect("compiled report in cache");
    let report: gaply_core::report::PublishReadyReport =
        serde_json::from_str(&json).expect("compiled report parses");
    eprintln!("pipeline done in {:?}; report {report_id}", t0.elapsed());

    let journal = TargetJournal { name: "PLOS ONE".into(), quartile: "Q1".into() };
    let (payload, sent_ids) = reviewer_agent::build_review_payload(&report, &journal, &[], &report_id);
    let payload_bytes = serde_json::to_vec(&payload).expect("payload serialises");
    eprintln!(
        "payload sha256[0..16]={}  bytes={}  findings_sent={}  report.verdict={:?}",
        sha256_hex(&payload_bytes),
        payload_bytes.len(),
        sent_ids.findings.len(),
        report.verdict
    );
    // The payload carries the deterministic verdict, so the model is not being
    // asked to derive the recommendation from nothing. Printed because it is the
    // anchor any stability in the result has to be attributed to.
    eprintln!("summary.overall_verdict sent to the model = {}", payload["summary"]["overall_verdict"]);

    let client = app_lib::models::proxy_client::ProxyReqwestClient::new(
        &url,
        TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID),
    )?;
    eprintln!("proxy: {url}  reachable: {}\n", client.reachable());

    // ---- N identical calls --------------------------------------------------
    // `body` is MODEL PROSE and `ReviewerLetterPanel` renders it verbatim at
    // `pr-body`. The panel deliberately shows no probability gauge (an ONTOLOGY
    // §4.20 PRESENTATION-class violation), but nothing stops the model writing a
    // percentage into the prose — so the suppression is only as good as what the
    // body says. The first baseline could not answer that, because this probe
    // printed a hash and a length and threw the text away. It no longer does.
    let body_dir = std::env::var("GRRB_BODY_DIR").ok().map(std::path::PathBuf::from);
    if let Some(d) = &body_dir {
        std::fs::create_dir_all(d).expect("create body dir");
        eprintln!("bodies -> {}", d.display());
    }
    println!(
        "run\tmodel\trecommendation\tprob\tissues\tfinding_refs_in_order\talternatives\t\
         warnings\tbody_chars\tbody_sha\tpct_in_body\tprob_word_in_body"
    );
    for i in 1..=runs {
        // Re-serialise nothing: the SAME `payload` value is sent every time.
        match client.verify_with_envelope(&payload) {
            Ok((reply, env)) => {
                let model = env.model.clone().unwrap_or_else(|| "UNREPORTED".into());
                match reviewer_agent::gate_reviewer_response(&reply, &sent_ids) {
                    Ok(ev) => {
                        let refs: Vec<&str> =
                            ev.issues.iter().map(|x| x.finding_ref.as_str()).collect();
                        // Capture the body VERBATIM. A span that is hashed and
                        // discarded cannot be checked by a human, which is the
                        // whole reason the first baseline left a question open.
                        if let Some(d) = &body_dir {
                            std::fs::write(d.join(format!("body-run{i}.txt")), &ev.body)
                                .expect("write body");
                        }
                        let low = ev.body.to_lowercase();
                        println!(
                            "{i}\t{model}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                            ev.recommendation,
                            ev.publication_probability
                                .map(|p| format!("{p}"))
                                .unwrap_or_else(|| "-".into()),
                            ev.issues.len(),
                            refs.join(","),
                            ev.alternatives.len(),
                            ev.warnings.len(),
                            ev.body.chars().count(),
                            sha256_hex(ev.body.as_bytes()),
                            ev.body.contains('%'),
                            low.contains("probability") || low.contains("likelihood"),
                        );
                    }
                    Err(e) => println!("{i}\t{model}\tGATE_REJECTED\t-\t-\t-\t-\t-\t-\t{e}"),
                }
            }
            Err(e) => println!("{i}\t-\tTRANSPORT_ERROR\t-\t-\t-\t-\t-\t-\t{e}"),
        }
    }
    Ok(())
}
