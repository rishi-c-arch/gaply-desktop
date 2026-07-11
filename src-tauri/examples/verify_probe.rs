//! Standalone proof for the SLM-2 Ollama runtime — NOT wired into the pipeline.
//!
//! Exercises the REAL verification path end to end against a live local
//! Ollama: gaply-core's `verify_citations()` builds the actual structured
//! evidence bundle, `OllamaVerifyClient` carries it to the local Qwen3
//! reasoning model, and the core harness gates validate the reply — exactly
//! what the pipeline will do at the (later) swap step, minus the pipeline.
//!
//! ACCURACY probe — two clear-cut cases, full thinking budget:
//!   c1 — a CLEAR SUPPORTED case: claimed DOI + title match the existence-check
//!        evidence exactly, reinforced by enrichment (venue + citation counts).
//!        A correctly-calibrated model should commit to SUPPORTED.
//!   c2 — a CLEAR REFUTED case: the claimed DOI resolves to a COMPLETELY
//!        DIFFERENT work (matched_title contradicts the claimed title) — the
//!        textbook REFUTED per gaply-core's INSTRUCTION ("the DOI resolves to a
//!        different work"). A correct model should REFUTE.
//! Both carry non-empty evidence, so neither is forced by Gate 2 — the verdicts
//! are the model's own judgment, and we watch whether any harness gate fires.
//!
//! Run (Ollama must be up on 127.0.0.1:11434 with qwen3:4b pulled):
//!
//!   cargo run --release --example verify_probe
//!
//! A `RecordingClient` wrapper captures the model's RAW JSON reply so the
//! probe can show it alongside the gated report and assert no `<think>`
//! reasoning trace leaked through the stripper.

use std::sync::Mutex;
use std::time::Instant;

use serde_json::Value;

use app_lib::models::ollama_verify::OllamaVerifyClient;
use gaply_core::extract::citations::Reference;
use gaply_core::refverify::{
    Enrichment, ExistenceCheck, Provenance, ReferenceVerification, UntrustedText,
};
use gaply_core::verify_agent::{verify_citations, ProxyClient};
use gaply_core::GaplyError;

/// Delegates to the real client while keeping a copy of the raw model reply.
struct RecordingClient {
    inner: OllamaVerifyClient,
    raw: Mutex<Option<Value>>,
}

impl ProxyClient for RecordingClient {
    fn verify(&self, payload: &Value) -> Result<Value, GaplyError> {
        let response = self.inner.verify(payload)?;
        *self.raw.lock().unwrap() = Some(response.clone());
        Ok(response)
    }
}

fn provenance(source: &str, url: &str) -> Provenance {
    Provenance {
        source: source.into(),
        url: url.into(),
        fetched_at: 1_752_192_000, // fixed timestamp: probes stay deterministic
        checksum: "sha256:probe-fixture".into(),
        from_cache: false,
    }
}

fn sample_items() -> Vec<(Reference, ReferenceVerification)> {
    // ---- c1: CLEAR SUPPORTED. DOI + title match exactly; enrichment agrees.
    const WC_TITLE: &str =
        "Molecular Structure of Nucleic Acids: A Structure for Deoxyribose Nucleic Acid";
    let r1 = Reference {
        raw: "Watson, J.D., Crick, F.H. (1953). Molecular Structure of Nucleic Acids. Nature 171, 737–738.".into(),
        authors: "Watson, J.D., Crick, F.H.".into(),
        year: Some(1953),
        title: Some(WC_TITLE.into()),
        doi: Some("10.1038/171737a0".into()),
    };
    let cr = provenance("crossref", "https://api.crossref.org/works/10.1038/171737a0");
    let s2 = provenance("semanticscholar", "https://api.semanticscholar.org/10.1038/171737a0");
    let rv1 = ReferenceVerification {
        reference_raw: r1.raw.clone(),
        exists: Some(ExistenceCheck {
            source: "crossref",
            found: true,
            doi: Some("10.1038/171737a0".into()), // matches claimed exactly
            title: Some(UntrustedText::new(WC_TITLE, cr.clone())), // matches claimed exactly
            is_retracted_hint: Some(false),
            provenance: cr,
        }),
        retraction: None,
        open_access: None,
        enrichment: Some(Enrichment {
            citation_count: Some(14032),
            influential_citation_count: Some(1180),
            abstract_text: None, // excluded by design; venue is the safe signal
            venue: Some(UntrustedText::new("Nature", s2.clone())),
            provenance: s2,
        }),
        provenance: Vec::new(),
        warnings: Vec::new(),
    };

    // ---- c2: CLEAR REFUTED. Claimed DOI resolves to a DIFFERENT work.
    let r2 = Reference {
        raw: "Anderson, R., Patel, S. (2020). CRISPR-Cas9 Gene Editing in Human Embryos. Science 368, 210–215.".into(),
        authors: "Anderson, R., Patel, S.".into(),
        year: Some(2020),
        title: Some("CRISPR-Cas9 Gene Editing in Human Embryos".into()),
        doi: Some("10.1126/science.abc1234".into()),
    };
    let cr2 = provenance("crossref", "https://api.crossref.org/works/10.1126/science.abc1234");
    let rv2 = ReferenceVerification {
        reference_raw: r2.raw.clone(),
        exists: Some(ExistenceCheck {
            source: "crossref",
            found: true,
            doi: Some("10.1126/science.abc1234".into()), // the SAME DOI the author claimed…
            // …but it resolves to a completely unrelated paper:
            title: Some(UntrustedText::new(
                "Seasonal Migration Patterns of the Atlantic Herring",
                cr2.clone(),
            )),
            is_retracted_hint: Some(false),
            provenance: cr2,
        }),
        retraction: None,
        open_access: None,
        enrichment: None,
        provenance: Vec::new(),
        warnings: Vec::new(),
    };

    vec![(r1, rv1), (r2, rv2)]
}

/// Isolation case: claimed metadata is TITLE + DOI ONLY (no authors, no year),
/// with evidence confirming both. Since the current `bundle_citation` schema
/// never emits matched authors/year, this removes the only claimed fields the
/// evidence structurally cannot confirm — so "matches the claimed metadata" is
/// fully satisfiable. If the SAME 8GB model now says SUPPORTED, the earlier
/// UNKNOWN was the schema gap; if it still hedges, it is model conservatism.
fn minimal_supported_item() -> Vec<(Reference, ReferenceVerification)> {
    const WC_TITLE: &str =
        "Molecular Structure of Nucleic Acids: A Structure for Deoxyribose Nucleic Acid";
    let r = Reference {
        raw: "Molecular Structure of Nucleic Acids. Nature (1953).".into(),
        authors: String::new(), // nothing for the evidence to fail to confirm
        year: None,
        title: Some(WC_TITLE.into()),
        doi: Some("10.1038/171737a0".into()),
    };
    let cr = provenance("crossref", "https://api.crossref.org/works/10.1038/171737a0");
    let rv = ReferenceVerification {
        reference_raw: r.raw.clone(),
        exists: Some(ExistenceCheck {
            source: "crossref",
            found: true,
            doi: Some("10.1038/171737a0".into()), // matches claimed
            title: Some(UntrustedText::new(WC_TITLE, cr.clone())), // matches claimed
            is_retracted_hint: Some(false),
            provenance: cr,
        }),
        retraction: None,
        open_access: None,
        enrichment: None,
        provenance: Vec::new(),
        warnings: Vec::new(),
    };
    vec![(r, rv)]
}

fn main() {
    // Optional first arg = model tag, so the SAME bundle can be run against a
    // different tier (e.g. `verify_probe gpt-oss:20b`). Defaults to the 8GB tier.
    let model = std::env::args().nth(1).unwrap_or_else(|| "qwen3:4b".to_string());
    // arg2 == "minimal" → the schema-gap isolation case (single citation).
    let minimal = std::env::args().nth(2).as_deref() == Some("minimal");

    let t0 = Instant::now();
    let inner = match OllamaVerifyClient::with_endpoint("http://127.0.0.1:11434", &model) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("CLIENT BUILD FAILED: {e}");
            std::process::exit(1);
        }
    };
    println!("client built in {:?} (model {model} @ 127.0.0.1:11434)", t0.elapsed());
    println!("note: Ollama loads the model lazily — first-call latency includes model load.\n");

    let client = RecordingClient { inner, raw: Mutex::new(None) };
    let items = if minimal { minimal_supported_item() } else { sample_items() };
    if minimal {
        println!("MINIMAL mode: 1 citation, claimed = title+DOI only (schema-gap isolation; expect SUPPORTED)…");
    } else {
        println!("sending 1 bundled call with {} citations (c1 expect SUPPORTED, c2 expect REFUTED)…", items.len());
    }

    let t1 = Instant::now();
    let report = match verify_citations(&client, &items) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("VERIFY FAILED after {:?}: {e}", t1.elapsed());
            std::process::exit(1);
        }
    };
    let call = t1.elapsed();

    let raw = client.raw.lock().unwrap().take().expect("raw response recorded");
    let raw_pretty = serde_json::to_string_pretty(&raw).unwrap();
    let report_pretty = serde_json::to_string_pretty(&report).unwrap();

    println!("\n--- raw model JSON (post think-strip, pre-gate) ---\n{raw_pretty}");
    println!("\n--- gated VerificationReport (what the swarm would see) ---\n{report_pretty}");

    let leak = raw_pretty.contains("<think>") || report_pretty.contains("<think>");

    println!("\n--- summary ---");
    println!("connected + responded : yes");
    println!("response time         : {:.1}s (single bundled call, incl. model load + thinking)", call.as_secs_f64());
    println!("schema-valid JSON     : yes — verify_citations() strict-parsed + gated it without error");
    println!("<think> leak          : {}", if leak { "FAIL — reasoning trace present" } else { "none (stripper worked)" });
    let expected: &[(&str, &str)] =
        if minimal { &[("c1", "Supported")] } else { &[("c1", "Supported"), ("c2", "Refuted")] };
    let mut any_gate = false;
    for v in &report.verdicts {
        let want = expected.iter().find(|(id, _)| *id == v.citation_id).map(|(_, w)| *w);
        let got = format!("{:?}", v.verdict);
        let mark = match want {
            Some(w) if w == got => "✓ correct",
            Some(w) => &format!("✗ expected {w}"),
            None => "?",
        };
        any_gate |= !v.gate_flags.is_empty();
        println!(
            "verdict {} : {} (confidence {:.2}) — {}  gates: {:?}",
            v.citation_id, got, v.confidence, mark, v.gate_flags
        );
    }
    println!("harness gate fired   : {}", if any_gate { "yes" } else { "no (model well-formed; gates are the safety net for when it isn't)" });
    if leak {
        std::process::exit(1);
    }
}
