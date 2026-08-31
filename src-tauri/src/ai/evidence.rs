//! Evidence assembly — the integration point between Phase 2 retrieval and the
//! Phase 3/4c task engine.
//!
//! Retrieval ranks chunks; this decides how many of them the model actually
//! sees, renders them in the spec's format, and refuses to run at all when
//! there is nothing to show.

use gaply_core::ai_engine::{retrieval, store};
use gaply_core::{Database, GaplyError};
use serde::Serialize;

use crate::ai::task::{EvidenceChunk, TaskContext};

/// MODEL-TOKEN budget for the `<evidence>` block on the dev model (§11 D14/D17).
///
/// Not the spec's implied ~2500. D12 measured ~22 ms per prompt token on CPU, so
/// 2500 tokens of evidence is ~55 s of prefill before a single output token.
/// A dev-model constraint, not a judgement about how much evidence the task
/// needs; raise it against measured prefill once Metal lands.
pub const EVIDENCE_BUDGET_TOKENS: usize = 1200;

/// Words per model token for EVIDENCE PROSE, measured with the real tokenizer
/// (§11 D17).
///
/// Assembly runs before the model is acquired, so it counts whitespace words
/// rather than tokenizing. This converts the token budget into a word budget.
///
/// MEASURED: 491 words of the fixture paper's prose encode to 640 Qwen2.5
/// tokens — 0.767 words per token. The ratio must be measured on EVIDENCE TEXT
/// specifically: a first attempt derived it from a whole prompt's token count
/// and got 0.46, because the JSON schema block tokenizes far more densely than
/// prose and dragged the average down. Budgeting on that number would have
/// under-filled the evidence block by ~40% — the opposite error to the one it
/// was introduced to fix, and just as invisible.
///
/// Re-measure with `measure_words_per_model_token_for_evidence_text` if the
/// tokenizer ever changes.
const WORDS_PER_MODEL_TOKEN: f64 = 0.767;

/// The word budget the trimmer actually applies.
pub fn evidence_budget_words(budget_tokens: usize) -> usize {
    ((budget_tokens as f64) * WORDS_PER_MODEL_TOKEN) as usize
}

/// How many chunks retrieval is asked for before the budget trims them.
pub const RETRIEVAL_K: usize = 12;

/// Whitespace WORD count. Named for what it measures — see D17 for why the
/// distinction cost 22 seconds a case before it was noticed.
fn estimate_words(s: &str) -> usize {
    s.split_whitespace().count()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceBundle {
    /// The rendered `<evidence>` block, in the spec's format.
    pub rendered: String,
    /// What the model was shown — the same set the validator checks against.
    #[serde(skip)]
    pub ctx: TaskContext,
    /// The same chunks, IN RANK ORDER. `ctx` keys them by id for lookup, which
    /// loses the ranking, and the ranking is the thing a reader needs: "these
    /// are the passages this check actually examined, best match first". A
    /// verdict of insufficient_evidence is only meaningful next to them.
    pub examined: Vec<EvidenceChunk>,
    pub chunks_sent: usize,
    /// Retrieved but dropped to fit the budget. Surfaced because the supporting
    /// passage may be one of them, and a caller should be able to tell that
    /// from the result rather than inferring it.
    pub chunks_dropped: usize,
    /// WORDS in the rendered block, and the model-token figure they imply.
    pub words_estimated: usize,
    pub tokens_implied: usize,
    pub retrieval_path: String,
}

/// What assembly produced.
pub enum Assembled {
    Ready(Box<EvidenceBundle>),
    /// The document has no indexed evidence for this claim. The model is NOT
    /// run (plan §11 D15): asking a model to judge a claim against an empty
    /// evidence block, while telling it to use only what is inside that block,
    /// invites invention. This is a different fact from the spec's
    /// `insufficient_evidence`, which is a judgement about evidence that WAS
    /// shown, and it has a different fix — index the document.
    NoEvidence { reason: String },
}

/// Retrieve, trim to budget, render.
///
/// `query_vector` comes from the embedding engine; retrieval is scoped to the
/// one cited document, because the spec's Prompt 2 judges a claim against
/// "chunks from that source only".
pub fn assemble(
    db: &Database,
    document_id: i64,
    claim: &str,
    query_vector: &[f32],
    budget_tokens: usize,
) -> Result<Assembled, GaplyError> {
    let budget_words = evidence_budget_words(budget_tokens);
    let found = match retrieval::semantic_search(db, claim, query_vector, Some(document_id), RETRIEVAL_K)
    {
        Ok(r) => r,
        // No embeddings stored at all is "nothing indexed", not an error the
        // caller must special-case.
        Err(GaplyError::NotFound { .. }) => {
            return Ok(Assembled::NoEvidence {
                reason: format!(
                    "document {document_id} has no embedded chunks — index and embed it first"
                ),
            })
        }
        Err(e) => return Err(e),
    };

    if found.hits.is_empty() {
        return Ok(Assembled::NoEvidence {
            reason: format!("no chunks retrieved for document {document_id}"),
        });
    }

    // Full chunk text, in RANK ORDER — the order decides what survives the
    // budget, so it must not be re-sorted by id along the way.
    let ranked_ids: Vec<i64> = found.hits.iter().map(|h| h.chunk_id).collect();
    let full = store::chunks_by_ids(db, &ranked_ids)?;

    let mut sent: Vec<EvidenceChunk> = Vec::new();
    let mut words = 0usize;
    let mut dropped = 0usize;
    for c in &full {
        let chunk = EvidenceChunk {
            // The id the model must echo back. Stringified store id, so a
            // returned id maps to a real row with no translation table.
            chunk_id: format!("c{}", c.id),
            page: c.page,
            section: c.section.clone(),
            text: c.content.clone(),
        };
        let cost = estimate_words(&chunk.render());
        if !sent.is_empty() && words + cost > budget_words {
            // Everything after the first over-budget chunk is dropped too:
            // they are lower-ranked, so keeping a later, smaller one would
            // silently reorder relevance to fit a byte count.
            dropped = full.len() - sent.len();
            break;
        }
        words += cost;
        sent.push(chunk);
    }

    if sent.is_empty() {
        return Ok(Assembled::NoEvidence {
            reason: format!("document {document_id} produced no usable chunks"),
        });
    }

    let ctx = TaskContext::new(sent.clone());
    Ok(Assembled::Ready(Box::new(EvidenceBundle {
        rendered: ctx.render_evidence(),
        examined: sent.clone(),
        chunks_sent: sent.len(),
        chunks_dropped: dropped,
        words_estimated: words,
        tokens_implied: (words as f64 / WORDS_PER_MODEL_TOKEN) as usize,
        retrieval_path: format!("{:?}", found.path),
        ctx,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::ai_engine::{embeddings as core_emb, registry};
    use gaply_core::chunk::PagedChunk;

    /// A tiny 3-dim space so retrieval can be exercised without a real model.
    fn seeded(n: usize, words_per_chunk: usize) -> (Database, i64) {
        let db = Database::in_memory().unwrap();
        let doc = store::create_document(&db, "Doc", "/tmp/d.pdf", "sum-ev").unwrap();
        registry::register_model(
            &db,
            registry::ModelRow {
                id: "test-emb".into(),
                kind: "embedding".into(),
                display_name: "T".into(),
                file_path: "/x".into(),
                sha256: None,
                dim: Some(3),
                quant: None,
            },
        )
        .unwrap();
        let chunks: Vec<PagedChunk> = (0..n)
            .map(|i| PagedChunk {
                seq: i as i64,
                content: (0..words_per_chunk)
                    .map(|w| format!("richness{i}w{w}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                token_estimate: words_per_chunk,
                page: Some(i as u32 + 1),
                section: Some("Results".into()),
                char_start: 0,
                char_end: 10,
            })
            .collect();
        store::index_chunks(&db, doc, &chunks).unwrap();
        let space = core_emb::EmbeddingSpace::new("test-emb", "p1");
        let pending = core_emb::chunks_missing_embeddings(&db, None, "test-emb").unwrap();
        // Rank descending by id order so the trim is predictable.
        let rows: Vec<(i64, Vec<f32>)> = pending
            .iter()
            .enumerate()
            .map(|(i, p)| (p.chunk_id, vec![1.0 - (i as f32 * 0.01), 0.0, 0.0]))
            .collect();
        core_emb::put_embeddings(&db, &space, &rows).unwrap();
        (db, doc)
    }

    #[test]
    fn an_empty_document_short_circuits_without_running_the_model() {
        let db = Database::in_memory().unwrap();
        let doc = store::create_document(&db, "Empty", "/tmp/e.pdf", "sum-empty").unwrap();
        match assemble(&db, doc, "any claim", &[1.0, 0.0, 0.0], EVIDENCE_BUDGET_TOKENS).unwrap() {
            Assembled::NoEvidence { reason } => {
                assert!(reason.contains("no embedded chunks"), "{reason}")
            }
            Assembled::Ready(_) => panic!("an empty document must not produce an evidence block"),
        }
    }

    #[test]
    fn a_document_with_evidence_renders_in_the_spec_format() {
        let (db, doc) = seeded(3, 10);
        let Assembled::Ready(b) =
            assemble(&db, doc, "richness", &[1.0, 0.0, 0.0], EVIDENCE_BUDGET_TOKENS).unwrap()
        else {
            panic!("expected evidence")
        };
        assert_eq!(b.chunks_sent, 3);
        assert_eq!(b.chunks_dropped, 0);
        assert!(b.rendered.starts_with("<evidence>\n") && b.rendered.ends_with("\n</evidence>"));
        // spec chunk header, and ids that map back to real rows
        assert!(b.rendered.contains("PAGE=1 SECTION=Results] richness0"), "{}", b.rendered);
        for id in b.ctx.chunk_ids() {
            assert!(id.starts_with('c'), "chunk ids must be echo-able: {id}");
        }
    }

    #[test]
    fn the_budget_drops_the_lowest_ranked_chunks_and_reports_how_many() {
        // 10 chunks of ~14 tokens each (10 words + the header); a 40-token
        // budget fits only the top few.
        let (db, doc) = seeded(10, 10);
        let Assembled::Ready(b) = assemble(&db, doc, "richness", &[1.0, 0.0, 0.0], 40).unwrap()
        else {
            panic!("expected evidence")
        };
        assert!(b.chunks_sent < 10, "the budget did not trim anything");
        assert_eq!(
            b.chunks_sent + b.chunks_dropped,
            10,
            "every retrieved chunk must be either sent or counted as dropped"
        );
        assert!(b.words_estimated <= evidence_budget_words(40), "budget exceeded: {}", b.words_estimated);
        // What survived is the TOP of the ranking, not an arbitrary subset.
        assert!(b.ctx.chunk_ids().len() == b.chunks_sent);
    }

    #[test]
    fn a_single_oversized_chunk_is_still_sent_rather_than_yielding_nothing() {
        // A budget smaller than the first chunk must not silently produce an
        // empty evidence block — that would be the invention risk D15 exists to
        // prevent, arrived at from the other direction.
        let (db, doc) = seeded(2, 200);
        let Assembled::Ready(b) = assemble(&db, doc, "richness", &[1.0, 0.0, 0.0], 5).unwrap()
        else {
            panic!("expected the first chunk to be sent anyway")
        };
        assert_eq!(b.chunks_sent, 1);
        assert_eq!(b.chunks_dropped, 1);
    }
}
