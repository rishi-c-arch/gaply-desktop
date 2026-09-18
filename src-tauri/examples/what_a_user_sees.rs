//! **The product, not an instrument.**
//!
//! Every measurement this week called a probe. This calls
//! `run_publishready_measured` — the `#[doc(hidden)]` seam that IS
//! `run_publishready`'s body — and prints what the frontend actually receives:
//! the finding rows in the order they render, the checklist, the reviewer
//! letter, and the declined lanes. If it reads badly, it reads badly to a
//! researcher.
use app_lib::pipeline::{run_pipeline_measured, NetworkConsent};
use gaply_core::db::Database;
use gaply_core::embed::{Embedder, HashEmbedder};
use std::sync::Arc;

fn main() {
    std::env::set_var("GAPLY_DISABLE_DEEP", "1");
    let path = std::env::args().nth(1).expect("manuscript path");
    let db = Arc::new(Database::in_memory().expect("db"));
    let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);

    // **Consent DENIED, which is the common configuration and the only one that
    // runs headless.** `run_publishready_measured` hardcodes
    // `NetworkConsent::Granted` (PublishReady's consent gate is in the frontend
    // at PublishReadyPage.tsx:142), and with consent granted the verification
    // lane reaches `verify_proxy` -> the OS keychain. A freshly built example
    // binary is not in that keychain item's ACL, so macOS prompts and a headless
    // run blocks at 0.0% CPU forever. That is this harness, not the product: the
    // signed app is in the ACL. Denied gives the SAME report a user with no
    // proxy configured sees, and `reviewer_letter_availability` exists because
    // the default proxy is loopback, so that is the common case.
    let out = run_pipeline_measured(
        db, embedder, path.clone(), None, None, None, NetworkConsent::Denied, &|_e| {},
    )
    .expect("pipeline");

    let r = serde_json::to_value(&out.report).expect("serialise");
    let r = &r;
    println!("=========== VERDICT ===========");
    println!("verdict            : {}", r["verdict"]);
    println!("combined_confidence: {}", r["combined_confidence"]);

    let f = r["findings"].as_array().cloned().unwrap_or_default();
    println!("\n=========== FINDINGS: {} ===========", f.len());
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for x in &f { *kinds.entry(x["title"].as_str().unwrap_or("?").to_string()).or_default() += 1; }
    println!("  by title:");
    for (k, n) in &kinds { println!("    {n:>3}  {k}"); }
    let located = f.iter().filter(|x| !x["location"].is_null()).count();
    println!("  findings carrying a location: {located} of {}", f.len());
    println!("  IN RENDER ORDER:");
    for (i, x) in f.iter().enumerate() {
        println!("    {:>2}. [{:<5}] {}", i + 1,
            x["severity"].as_str().unwrap_or("?"),
            x["title"].as_str().unwrap_or("?").chars().take(78).collect::<String>());
    }
    for (i, x) in f.iter().take(5).enumerate() {
        println!("\n--- row {} ---", i + 1);
        println!("  [{}] {}  ({})", x["severity"].as_str().unwrap_or("?"),
            x["title"].as_str().unwrap_or("?"), x["certainty_label"].as_str().unwrap_or("?"));
        println!("  agent : {}", x["agent"].as_str().unwrap_or("?"));
        println!("  detail: {}", x["detail"].as_str().unwrap_or("?"));
        println!("  where : {}", x["location"]);
    }

    let c = r["checklist"].as_array().cloned().unwrap_or_default();
    println!("\n=========== CHECKLIST: {} ===========", c.len());
    for x in c.iter().take(10) {
        println!("  [{}] {}",
            if x["passed"].as_bool().unwrap_or(false) { "PASS" } else { "FAIL" },
            x["requirement"].as_str().unwrap_or("?"));
        if let Some(d) = x["detail"].as_str() { println!("        {d}"); }
    }

    println!("\n=========== REVIEWER LETTER (no proxy configured) ===========");
    let rv = gaply_core::reviewer_agent::ReviewerEvaluation::unavailable_offline();
    println!("available      : {}", rv.available);
    println!("recommendation : {:?}", rv.recommendation);
    println!("body           : {}", rv.body);

    println!("\n=========== WHAT GAPLY DOES NOT ASSESS ===========");
    for d in gaply_core::declined::DECLINED_LANES {
        println!("  {} - {} [{}]", d.lane, d.reason, d.record);
    }
    println!("\n=========== LANES ===========");
    println!("{:?}", out.lanes);
}
