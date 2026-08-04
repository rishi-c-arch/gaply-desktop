//! Plagiarism / semantic-similarity agent.
//!
//! Embeds manuscript chunks with all-MiniLM-L6-v2-shaped 384-dim vectors (via
//! the [`Embedder`](crate::embed::Embedder) trait — `HashEmbedder` is the
//! interim proxy until the real model is wired) and finds semantic overlap
//! two ways:
//!   * **corpus** — each manuscript chunk vs the shared local RAG corpus;
//!   * **self-plagiarism** — manuscript chunks vs each other.
//! Candidates come from sqlite-vec KNN; final scores are exact cosine.
//!
//! # Poisoning defense — isolation
//!
//! User-uploaded manuscript text is UNTRUSTED and MUST never enter the shared
//! RAG corpus (that would let an upload poison future retrievals). A
//! [`PlagiarismSession`] therefore owns its OWN in-memory database — a
//! per-session isolated vector store — where the manuscript's chunks and
//! embeddings live. The shared corpus is only ever read (KNN + provenance
//! lookups), never written. Nothing about a plagiarism check mutates shared
//! state.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::chunk;
use crate::db::Database;
use crate::embed::Embedder;
use crate::error::GaplyError;

/// Default cosine threshold above which a pair is reported.
pub const DEFAULT_THRESHOLD: f64 = 0.80;
/// Chunks whose sequence numbers are within this distance overlap by
/// construction (64-token overlap) and are skipped for self-plagiarism.
const ADJACENT: i64 = 1;
const EXCERPT_CHARS: usize = 200;

/// "lexical-overlap signal", not "semantic signal": the score is cosine over
/// [`crate::embed::HashEmbedder`], a bag-of-words encoder (`embed.rs:33-53`) —
/// the only `Embedder` in the tree. It measures shared vocabulary, not meaning,
/// so calling it semantic claimed a capability that does not exist here. See the
/// `match_type_label` note in `report.rs` for the full reasoning.
pub const ISOLATION_NOTE: &str = "User-uploaded text is analyzed in a per-session isolated \
store and is never written to the shared corpus. Similarity is a lexical-overlap signal for \
human review, not a determination of plagiarism.";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchSource {
    /// A chunk in the shared local corpus.
    Corpus {
        document_id: i64,
        chunk_id: i64,
        title: String,
        source_url: String,
        source_type: String,
        excerpt: String,
    },
    /// Another chunk within the same manuscript (self-plagiarism).
    SelfManuscript { other_chunk_seq: i64, excerpt: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchSpan {
    pub manuscript_chunk_seq: i64,
    pub manuscript_excerpt: String,
    /// Cosine similarity in [0, 1].
    pub similarity: f64,
    pub source: MatchSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlagiarismReport {
    pub chunk_count: usize,
    /// How many CORPUS chunks were available to compare against.
    ///
    /// Without this, "zero matches" is ambiguous between *the manuscript
    /// overlaps nothing* and *there was nothing to overlap with* — the exact
    /// clean-versus-unchecked ambiguity §26 PR-4 exists to resolve. A lane whose
    /// criterion cannot distinguish those two must not enter the
    /// verdict-relevance denominator, so this datum is the lane's prerequisite.
    pub corpus_chunks_available: usize,
    pub threshold: f64,
    pub corpus_matches: Vec<MatchSpan>,
    pub self_matches: Vec<MatchSpan>,
    /// Isolation + interpretation note. Never empty.
    pub note: String,
}

struct ManuscriptChunk {
    seq: i64,
    text: String,
    embedding: Vec<f32>,
}

/// A per-session isolated plagiarism check. Owns its own in-memory vector
/// store; the shared corpus is only ever read.
pub struct PlagiarismSession {
    store: Database,
    chunks: Vec<ManuscriptChunk>,
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (*x as f64, *y as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
    }
}

fn excerpt(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() <= EXCERPT_CHARS {
        t.to_string()
    } else {
        let cut: String = t.chars().take(EXCERPT_CHARS).collect();
        format!("{cut}…")
    }
}

impl PlagiarismSession {
    /// Create a fresh, isolated session with its own in-memory vector store.
    pub fn new() -> Result<Self, GaplyError> {
        Ok(Self { store: Database::in_memory()?, chunks: Vec::new() })
    }

    /// Chunk, embed, and store the manuscript in the ISOLATED session store.
    /// Never touches the shared corpus.
    pub fn ingest_manuscript(
        &mut self,
        embedder: &dyn Embedder,
        text: &str,
    ) -> Result<usize, GaplyError> {
        self.ingest_with(embedder, text, chunk::CHUNK_TOKENS, chunk::CHUNK_OVERLAP)
    }

    pub(crate) fn ingest_with(
        &mut self,
        embedder: &dyn Embedder,
        text: &str,
        chunk_tokens: usize,
        overlap: usize,
    ) -> Result<usize, GaplyError> {
        for c in chunk::chunk_text(text, chunk_tokens, overlap) {
            let embedding = embedder.embed(&c.content)?;
            self.store.insert_embedding("manuscript_chunk", c.seq, &embedding)?;
            self.chunks.push(ManuscriptChunk { seq: c.seq, text: c.content, embedding });
        }
        tracing::info!(chunks = self.chunks.len(), "manuscript ingested into isolated store");
        Ok(self.chunks.len())
    }

    /// Near-duplicate spans within the manuscript itself. Candidates via KNN
    /// on the isolated store; adjacent (overlapping) chunks are excluded.
    pub fn self_plagiarism(&self, threshold: f64) -> Result<Vec<MatchSpan>, GaplyError> {
        let mut out = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        for (i, c) in self.chunks.iter().enumerate() {
            for m in self.store.knn_embeddings(&c.embedding, 6)? {
                let j = m.source_id as usize;
                if j >= self.chunks.len() || j == i {
                    continue;
                }
                if (j as i64 - i as i64).abs() <= ADJACENT {
                    continue;
                }
                let other = &self.chunks[j];
                let sim = cosine(&c.embedding, &other.embedding);
                if sim >= threshold && seen.insert((i.min(j), i.max(j))) {
                    out.push(MatchSpan {
                        manuscript_chunk_seq: c.seq,
                        manuscript_excerpt: excerpt(&c.text),
                        similarity: sim,
                        source: MatchSource::SelfManuscript {
                            other_chunk_seq: other.seq,
                            excerpt: excerpt(&other.text),
                        },
                    });
                }
            }
        }
        out.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        Ok(out)
    }

    /// Matches of manuscript chunks against the shared corpus (read-only).
    pub fn compare_to_corpus(
        &self,
        shared: &Database,
        threshold: f64,
        top_k: usize,
    ) -> Result<Vec<MatchSpan>, GaplyError> {
        let mut out = Vec::new();
        for c in &self.chunks {
            for m in shared.knn_embeddings(&c.embedding, top_k)? {
                if m.source_type != "chunk" {
                    continue;
                }
                let Some(corpus_vec) = shared.fetch_embedding(m.rowid)? else { continue };
                let sim = cosine(&c.embedding, &corpus_vec);
                if sim < threshold {
                    continue;
                }
                let Some(info) = corpus_chunk_info(shared, m.source_id)? else { continue };
                out.push(MatchSpan {
                    manuscript_chunk_seq: c.seq,
                    manuscript_excerpt: excerpt(&c.text),
                    similarity: sim,
                    source: MatchSource::Corpus {
                        document_id: info.document_id,
                        chunk_id: info.chunk_id,
                        title: info.title,
                        source_url: info.source_url,
                        source_type: info.source_type,
                        excerpt: excerpt(&info.content),
                    },
                });
            }
        }
        out.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        Ok(out)
    }

    pub fn report(
        &self,
        shared: &Database,
        threshold: Option<f64>,
    ) -> Result<PlagiarismReport, GaplyError> {
        let t = threshold.unwrap_or(DEFAULT_THRESHOLD);
        Ok(PlagiarismReport {
            chunk_count: self.chunks.len(),
            corpus_chunks_available: corpus_chunk_count(shared)?,
            threshold: t,
            corpus_matches: self.compare_to_corpus(shared, t, 5)?,
            self_matches: self.self_plagiarism(t)?,
            note: ISOLATION_NOTE.to_string(),
        })
    }
}

struct CorpusChunkInfo {
    chunk_id: i64,
    document_id: i64,
    content: String,
    title: String,
    source_url: String,
    source_type: String,
}

/// M1: source types EXCLUDED from the shared-corpus plagiarism scan. These are
/// the user's OWN working documents — Gap Finder base papers (`research_paper`)
/// and PublishReady author-guidelines pages Gaply fetched for them
/// (`journal_guideline`) — NOT third-party source material. Matching a manuscript
/// against them produced false "corpus matches". DENYLIST, not allowlist: a
/// genuine corpus feed added later under any other `source_type` is scanned
/// automatically, without re-touching this query.
const EXCLUDED_CORPUS_SOURCE_TYPES: [&str; 2] = ["research_paper", "journal_guideline"];

/// How many chunks the shared corpus offers for comparison, applying the SAME
/// exclusions `corpus_chunk_info` applies — so the count is what could actually
/// have matched, not what happens to be stored. A count computed differently
/// from the comparison would answer a different question than the one the lane
/// criterion asks.
fn corpus_chunk_count(shared: &Database) -> Result<usize, GaplyError> {
    use rusqlite::params;
    let conn = shared.conn()?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks c JOIN documents d ON d.id = c.document_id
         WHERE d.source_type NOT IN (?1, ?2)",
        params![EXCLUDED_CORPUS_SOURCE_TYPES[0], EXCLUDED_CORPUS_SOURCE_TYPES[1]],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

fn corpus_chunk_info(
    shared: &Database,
    chunk_id: i64,
) -> Result<Option<CorpusChunkInfo>, GaplyError> {
    use rusqlite::{params, OptionalExtension};
    let conn = shared.conn()?;
    // (?2, ?3) bind the two EXCLUDED_CORPUS_SOURCE_TYPES — the user's own working
    // docs never surface as corpus matches (M1). A rejected doc → None → the
    // caller's `else { continue }` (compare_to_corpus) drops it cleanly.
    let info = conn
        .query_row(
            "SELECT c.id, c.document_id, c.content, d.title, d.source_url, d.source_type
             FROM chunks c JOIN documents d ON d.id = c.document_id
             WHERE c.id = ?1 AND d.status = 'ingested'
               AND d.source_type NOT IN (?2, ?3)",
            params![chunk_id, EXCLUDED_CORPUS_SOURCE_TYPES[0], EXCLUDED_CORPUS_SOURCE_TYPES[1]],
            |row| {
                Ok(CorpusChunkInfo {
                    chunk_id: row.get(0)?,
                    document_id: row.get(1)?,
                    content: row.get(2)?,
                    title: row.get(3)?,
                    source_url: row.get(4)?,
                    source_type: row.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::HashEmbedder;
    use crate::rag::{ingest_document, RawDocument, SourceType};
    use crate::vector::EMBEDDING_DIM;

    // A GENUINE-corpus document (a non-excluded source_type). `Retraction` is used
    // deliberately: `journal_guideline` / `research_paper` are now excluded from the
    // plagiarism scan (M1), so a corpus fixture must use a scannable type to prove a
    // real corpus doc still matches.
    fn corpus_doc(title: &str, url: &str, content: &str) -> RawDocument {
        RawDocument {
            source_type: SourceType::Retraction,
            title: title.into(),
            source_url: url.into(),
            fetched_at: 1_700_000_000,
            content: content.into(),
        }
    }

    const ORIGINAL: &str = "Mitochondrial dysfunction contributes to neurodegeneration by \
        impairing cellular energy production and elevating oxidative stress in affected neurons \
        across the cortex and hippocampus of the ageing brain.";
    // one word changed ("elevating" -> "increasing") — a near-duplicate
    const NEAR_DUP: &str = "Mitochondrial dysfunction contributes to neurodegeneration by \
        impairing cellular energy production and increasing oxidative stress in affected neurons \
        across the cortex and hippocampus of the ageing brain.";

    #[test]
    fn embeddings_are_384_dimensional() {
        let v = HashEmbedder.embed("a manuscript chunk about protein folding").unwrap();
        assert_eq!(v.len(), 384);
        assert_eq!(EMBEDDING_DIM, 384);
    }

    #[test]
    fn near_duplicate_of_corpus_is_detected_with_provenance() {
        let shared = Database::in_memory().unwrap();
        ingest_document(&shared, &HashEmbedder, &corpus_doc("Neuro Review", "https://x.example/nr", ORIGINAL))
            .unwrap();

        let mut session = PlagiarismSession::new().unwrap();
        session.ingest_manuscript(&HashEmbedder, NEAR_DUP).unwrap();

        let matches = session.compare_to_corpus(&shared, 0.8, 5).unwrap();
        assert!(!matches.is_empty(), "near-duplicate not detected");
        assert!(matches[0].similarity > 0.8, "similarity {} too low", matches[0].similarity);
        assert!(matches[0].similarity < 1.0, "should be near-dup, not identical");
        match &matches[0].source {
            MatchSource::Corpus { title, source_url, .. } => {
                assert_eq!(title, "Neuro Review");
                assert_eq!(source_url, "https://x.example/nr");
            }
            other => panic!("expected corpus provenance, got {other:?}"),
        }
    }

    #[test]
    fn self_plagiarism_flags_repeated_distant_passage() {
        // a distinctive 8-token phrase, 22 tokens of filler, then the phrase again
        let phrase = "alpha bravo charlie delta echo foxtrot golf hotel";
        let filler: String = (1..=22).map(|i| format!("f{i} ")).collect();
        let text = format!("{phrase} {filler} {phrase}");

        let mut session = PlagiarismSession::new().unwrap();
        // small chunks so the repeated phrase forms two non-adjacent chunks
        session.ingest_with(&HashEmbedder, &text, 8, 2).unwrap();

        let matches = session.self_plagiarism(0.9).unwrap();
        assert!(!matches.is_empty(), "repeated passage not flagged");
        match &matches[0].source {
            MatchSource::SelfManuscript { other_chunk_seq, .. } => {
                let a = matches[0].manuscript_chunk_seq;
                assert!((a - other_chunk_seq).abs() > ADJACENT, "adjacent chunks should be skipped");
            }
            other => panic!("expected self-manuscript match, got {other:?}"),
        }
    }

    #[test]
    fn user_upload_never_writes_to_shared_corpus() {
        let shared = Database::in_memory().unwrap();
        // seed the shared corpus with one legitimate document
        ingest_document(&shared, &HashEmbedder, &corpus_doc("Seed", "https://s.example", ORIGINAL))
            .unwrap();

        let before_docs = shared.count_rows("documents").unwrap();
        let before_chunks = shared.count_rows("chunks").unwrap();
        let before_embeddings = shared.count_rows("embeddings").unwrap();

        // ingest a large user manuscript and run the full report
        let user_text = format!("{NEAR_DUP} {ORIGINAL} {}", "extra sentence. ".repeat(50));
        let mut session = PlagiarismSession::new().unwrap();
        let ingested = session.ingest_manuscript(&HashEmbedder, &user_text).unwrap();
        assert!(ingested >= 1);
        let _ = session.report(&shared, None).unwrap();

        // the shared corpus is byte-for-byte unchanged
        assert_eq!(shared.count_rows("documents").unwrap(), before_docs);
        assert_eq!(shared.count_rows("chunks").unwrap(), before_chunks);
        assert_eq!(shared.count_rows("embeddings").unwrap(), before_embeddings);
    }

    #[test]
    fn users_own_working_docs_never_surface_as_corpus_matches() {
        // M1: the user's OWN Gap Finder papers (research_paper) + PublishReady
        // guidelines (journal_guideline) must NEVER surface as plagiarism "corpus
        // matches" — even when the corpus text is IDENTICAL to the manuscript.
        let shared = Database::in_memory().unwrap();
        // Distinct content per doc (a unique trailing token) so ingest_document's
        // checksum dedup doesn't drop any — each stays highly similar to ORIGINAL.
        let ingest = |st: SourceType, url: &str, tag: &str| {
            ingest_document(
                &shared,
                &HashEmbedder,
                &RawDocument {
                    source_type: st,
                    title: "Doc".into(),
                    source_url: url.into(),
                    fetched_at: 1_700_000_000,
                    content: format!("{ORIGINAL} {tag}"),
                },
            )
            .unwrap();
        };
        ingest(SourceType::ResearchPaper, "gapfinder://s/p1", "refA");
        ingest(SourceType::JournalGuideline, "https://j.example/guidelines", "refB");

        let mut session = PlagiarismSession::new().unwrap();
        session.ingest_manuscript(&HashEmbedder, ORIGINAL).unwrap();

        // LEAK CLOSED: near-identical text in the user's own working docs → no match.
        let matches = session.compare_to_corpus(&shared, 0.8, 5).unwrap();
        assert!(
            matches.is_empty(),
            "the user's own working docs must not be corpus matches, got {matches:?}"
        );

        // DENYLIST PRECISE (not a blanket disable): a genuine, non-excluded corpus
        // type (retraction) with the same-ish text STILL matches.
        ingest(SourceType::Retraction, "https://retractionwatch.example/r1", "refC");
        let matches2 = session.compare_to_corpus(&shared, 0.8, 5).unwrap();
        assert!(
            !matches2.is_empty(),
            "a non-excluded corpus type (retraction) must still be scanned"
        );
        match &matches2[0].source {
            MatchSource::Corpus { source_type, .. } => assert_eq!(source_type, "retraction"),
            other => panic!("expected a retraction corpus match, got {other:?}"),
        }
    }

    #[test]
    fn report_always_carries_isolation_note() {
        let shared = Database::in_memory().unwrap();
        let mut session = PlagiarismSession::new().unwrap();
        session.ingest_manuscript(&HashEmbedder, ORIGINAL).unwrap();
        let report = session.report(&shared, None).unwrap();
        assert!(!report.note.is_empty());
        assert!(report.note.contains("isolated"));
    }
}
