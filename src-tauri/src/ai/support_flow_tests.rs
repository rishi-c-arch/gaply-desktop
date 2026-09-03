//! End-to-end flow tests for citation_support: assembly → validation →
//! persistence, against a scripted model.
//!
//! The question these exist to answer is the one that matters most for this
//! task: **can an accepted card ever contain a chunk_id that was not sent, or a
//! page that disagrees with the store?** The answer must be no BY
//! CONSTRUCTION — the validator runs before any write, so an ungrounded output
//! cannot reach the table.

#![cfg(test)]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use gaply_core::ai_engine::{cards, embeddings as core_emb, registry, store};
use gaply_core::chunk::PagedChunk;
use gaply_core::Database;

use crate::ai::evidence::{assemble, Assembled, EVIDENCE_BUDGET_TOKENS};
use crate::ai::generative::{GenOutput, GenRequest, GenerationBackend, RamEstimate, StopReason};
use crate::ai::model_manager::{BackendLoader, ModelManager};
use crate::ai::task::{run_task, TaskError};
use crate::ai::tasks::citation_support::CitationSupportTask;

/* --------------------------- scripted backend ---------------------------- */

struct Scripted {
    replies: std::sync::Mutex<std::collections::VecDeque<String>>,
}

impl GenerationBackend for Scripted {
    fn generate(&self, _req: GenRequest<'_>) -> Result<GenOutput, gaply_core::GaplyError> {
        let text = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| "SCRIPT EXHAUSTED".into());
        Ok(GenOutput {
            text,
            tokens: 1,
            stop_reason: StopReason::EndOfTurn,
            elapsed_ms: 1,
            prompt_tokens: 10,
            prefill_ms: 1,
            decode_ms: 1,
            decode_tokens_per_sec: 1.0,
        })
    }
}

struct ScriptedLoader(Vec<String>);

impl BackendLoader for ScriptedLoader {
    fn model_id(&self) -> String {
        "scripted-gen".into()
    }
    fn ram_estimate(&self) -> Result<RamEstimate, gaply_core::GaplyError> {
        Ok(RamEstimate {
            weights_bytes: 1,
            kv_cache_bytes: 1,
            total_bytes: 2,
            context_length: 1,
            model_max_context: 1,
            layers: 1,
            kv_heads: 1,
            head_dim: 1,
        })
    }
    fn load(&self) -> Result<Arc<dyn GenerationBackend>, gaply_core::GaplyError> {
        Ok(Arc::new(Scripted { replies: std::sync::Mutex::new(self.0.clone().into()) }))
    }
}

fn manager<S: AsRef<str>>(replies: &[S]) -> ModelManager {
    ModelManager::new(Arc::new(ScriptedLoader(
        replies.iter().map(|s| s.as_ref().to_string()).collect(),
    )))
}

/* ------------------------------- fixture --------------------------------- */

/// Three chunks on known pages, with a mocked 3-dim embedding space.
fn fixture() -> (Database, i64, Vec<i64>) {
    let db = Database::in_memory().unwrap();
    let doc = store::create_document(&db, "Fixture", "/tmp/f.pdf", "sum-flow").unwrap();
    for (id, kind) in [("scripted-gen", "generative"), ("test-emb", "embedding")] {
        registry::register_model(
            &db,
            registry::ModelRow {
                id: id.into(),
                kind: kind.into(),
                display_name: id.into(),
                file_path: "/x".into(),
                sha256: None,
                dim: Some(3),
                quant: None,
            },
        )
        .unwrap();
    }
    let texts = [
        ("Species richness rose 31 percent under organic management.", 8u32),
        ("Organic systems are widely discussed in the literature.", 9),
        ("No significant effect was observed for soil fauna.", 10),
    ];
    let chunks: Vec<PagedChunk> = texts
        .iter()
        .enumerate()
        .map(|(i, (t, page))| PagedChunk {
            seq: i as i64,
            content: (*t).to_string(),
            token_estimate: t.split_whitespace().count(),
            page: Some(*page),
            section: Some("Results".into()),
            char_start: 0,
            char_end: t.len(),
        })
        .collect();
    store::index_chunks(&db, doc, &chunks).unwrap();
    let ids: Vec<i64> = store::list_chunks(&db, doc).unwrap().iter().map(|c| c.id).collect();
    let space = core_emb::EmbeddingSpace::new("test-emb", "p1");
    let rows: Vec<(i64, Vec<f32>)> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, vec![1.0 - i as f32 * 0.1, 0.0, 0.0]))
        .collect();
    core_emb::put_embeddings(&db, &space, &rows).unwrap();
    (db, doc, ids)
}

fn bundle_for(db: &Database, doc: i64) -> crate::ai::evidence::EvidenceBundle {
    match assemble(db, doc, "organic richness", &[1.0, 0.0, 0.0], EVIDENCE_BUDGET_TOKENS).unwrap() {
        Assembled::Ready(b) => *b,
        Assembled::NoEvidence { reason } => panic!("expected evidence: {reason}"),
    }
}

fn valid_reply(chunk_id: &str, page: u32) -> String {
    format!(
        r#"{{"verdict":"strong","confidence":0.9,
            "supporting_chunks":[{{"chunk_id":"{chunk_id}","page":{page},
              "why":"Reports the richness increase the claim describes."}}],
            "claim_elements":[{{"element":"subject","status":"found"}},
                              {{"element":"direction","status":"found"}}],
            "explanation":"The evidence reports the same finding.",
            "suggested_rewrite":null}}"#
    )
}

/* --------------------------------- tests --------------------------------- */

#[tokio::test]
async fn a_valid_result_persists_a_card_with_queryable_chunk_refs() {
    let (db, doc, ids) = fixture();
    let b = bundle_for(&db, doc);
    let m = manager(&[valid_reply(&format!("c{}", ids[0]), 8)]);
    let task = CitationSupportTask {
        claim: "Organic farming raises species richness by about a third.".into(),
        cited_source: "Sharma et al. (2021) — Organic systems".into(),
        evidence: b.rendered.clone(),
    };
    let run = run_task(&m, &task, &b.ctx, Arc::new(AtomicBool::new(false)), None).await.unwrap();

    let card = cards::NewEvidenceCard {
        document_id: doc,
        chunk_id: Some(ids[0]),
        page: Some(8),
        claim: task.claim.clone(),
        evidence_text: run.output.explanation.clone(),
        evidence_type: "support".into(),
        verdict: Some("strong".into()),
        confidence: Some(run.output.confidence),
        model_id: run.model_id.clone(),
        model_version: "Q4_K_M".into(),
        prompt_version: run.prompt_version.into(),
        provenance_json: serde_json::json!({ "advisories": Vec::<String>::new() }).to_string(),
    };
    let id = cards::insert_card(&db, &card, &[(ids[0], cards::ChunkRole::Supporting)]).unwrap();

    let stored = cards::get_card(&db, id).unwrap().unwrap();
    assert_eq!(stored.verdict.as_deref(), Some("strong"));
    assert_eq!(stored.prompt_version, "citation_support-v1.5");
    assert_eq!(stored.model_version, "Q4_K_M");
    assert_eq!(
        cards::card_chunks(&db, id).unwrap(),
        vec![(ids[0], "supporting".to_string())],
        "chunk refs must be queryable rows, not only JSON"
    );
}

#[tokio::test]
async fn an_invented_chunk_id_never_reaches_the_table() {
    // THE guarantee. The model cites c99, which was never sent.
    let (db, doc, _ids) = fixture();
    let b = bundle_for(&db, doc);
    let bad = valid_reply("c99", 8);
    let m = manager(&[bad.clone(), bad]);
    let task = CitationSupportTask {
        claim: "Organic farming raises species richness.".into(),
        cited_source: "S (2021)".into(),
        evidence: b.rendered.clone(),
    };
    let err = run_task(&m, &task, &b.ctx, Arc::new(AtomicBool::new(false)), None)
        .await
        .unwrap_err();
    match err {
        TaskError::ValidationFailed { errors, .. } => {
            assert!(errors.iter().any(|e| e.field.contains("chunk_id") && e.is_fatal()), "{errors:?}");
        }
        other => panic!("{other:?}"),
    }
    assert!(
        cards::list_cards(&db, doc).unwrap().is_empty(),
        "a fatal failure persisted a card"
    );
}

#[tokio::test]
async fn a_page_that_disagrees_with_the_store_never_reaches_the_table() {
    let (db, doc, ids) = fixture();
    let b = bundle_for(&db, doc);
    // chunk 0 is stored on page 8; the model claims 12.
    let bad = valid_reply(&format!("c{}", ids[0]), 12);
    let m = manager(&[bad.clone(), bad]);
    let task = CitationSupportTask {
        claim: "Organic farming raises species richness.".into(),
        cited_source: "S (2021)".into(),
        evidence: b.rendered.clone(),
    };
    let err = run_task(&m, &task, &b.ctx, Arc::new(AtomicBool::new(false)), None)
        .await
        .unwrap_err();
    assert!(matches!(err, TaskError::ValidationFailed { .. }));
    assert!(cards::list_cards(&db, doc).unwrap().is_empty());
}

#[tokio::test]
async fn a_false_strong_is_rejected_before_it_can_be_persisted() {
    let (db, doc, ids) = fixture();
    let b = bundle_for(&db, doc);
    let bad = format!(
        r#"{{"verdict":"strong","confidence":0.95,
            "supporting_chunks":[{{"chunk_id":"c{}","page":8,"why":"Discusses the topic."}}],
            "claim_elements":[{{"element":"subject","status":"found"}},
                              {{"element":"magnitude","status":"absent"}}],
            "explanation":"Looks right.","suggested_rewrite":null}}"#,
        ids[0]
    );
    let m = manager(&[bad.clone(), bad]);
    let task = CitationSupportTask {
        claim: "Organic farming raises richness by exactly 31%.".into(),
        cited_source: "S (2021)".into(),
        evidence: b.rendered.clone(),
    };
    let err = run_task(&m, &task, &b.ctx, Arc::new(AtomicBool::new(false)), None)
        .await
        .unwrap_err();
    match err {
        TaskError::ValidationFailed { errors, .. } => assert!(
            errors.iter().any(|e| e.field == "verdict" && e.problem.contains("magnitude")),
            "the unmet element must be named: {errors:?}"
        ),
        other => panic!("{other:?}"),
    }
    assert!(cards::list_cards(&db, doc).unwrap().is_empty());
}

#[tokio::test]
async fn an_advisory_only_result_is_accepted_and_its_advisories_reach_provenance_json() {
    let (db, doc, ids) = fixture();
    let b = bundle_for(&db, doc);
    // Valid in every fatal respect; the "why" is 24 words.
    let long_why = (0..24).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
    let reply = format!(
        r#"{{"verdict":"strong","confidence":0.8,
            "supporting_chunks":[{{"chunk_id":"c{}","page":8,"why":"{long_why}"}}],
            "claim_elements":[{{"element":"subject","status":"found"}}],
            "explanation":"Fine.","suggested_rewrite":null}}"#,
        ids[0]
    );
    let m = manager(&[reply]);
    let task = CitationSupportTask {
        claim: "Organic farming raises richness.".into(),
        cited_source: "S (2021)".into(),
        evidence: b.rendered.clone(),
    };
    let run = run_task(&m, &task, &b.ctx, Arc::new(AtomicBool::new(false)), None).await.unwrap();
    assert!(!run.retried, "an advisory triggered a retry");
    assert_eq!(run.advisories.len(), 1, "{:?}", run.advisories);

    let advisories: Vec<String> = run.advisories.iter().map(|a| a.to_string()).collect();
    let card = cards::NewEvidenceCard {
        document_id: doc,
        chunk_id: Some(ids[0]),
        page: Some(8),
        claim: task.claim.clone(),
        evidence_text: run.output.explanation.clone(),
        evidence_type: "support".into(),
        verdict: Some("strong".into()),
        confidence: Some(run.output.confidence),
        model_id: run.model_id.clone(),
        model_version: "Q4_K_M".into(),
        prompt_version: run.prompt_version.into(),
        provenance_json: serde_json::json!({ "advisories": advisories }).to_string(),
    };
    let id = cards::insert_card(&db, &card, &[(ids[0], cards::ChunkRole::Supporting)]).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&cards::get_card(&db, id).unwrap().unwrap().provenance_json).unwrap();
    assert!(
        v["advisories"][0].as_str().unwrap().contains("24 words"),
        "the advisory did not survive into provenance_json: {v}"
    );
}

#[tokio::test]
async fn no_evidence_short_circuits_with_no_generation_and_no_persistence() {
    let db = Database::in_memory().unwrap();
    let doc = store::create_document(&db, "Empty", "/tmp/e.pdf", "sum-none").unwrap();
    match assemble(&db, doc, "any claim", &[1.0, 0.0, 0.0], EVIDENCE_BUDGET_TOKENS).unwrap() {
        Assembled::NoEvidence { .. } => {}
        Assembled::Ready(_) => panic!("an empty document produced an evidence block"),
    }
    // Nothing generated (no manager was even constructed) and nothing written.
    assert!(cards::list_cards(&db, doc).unwrap().is_empty());
}
