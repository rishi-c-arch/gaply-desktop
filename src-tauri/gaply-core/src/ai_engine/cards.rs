//! Persistence for `ai_evidence_cards` and its join table.
//!
//! # Chunk references are foreign keys, not JSON
//!
//! Every chunk a card drew on is a row in `ai_evidence_card_chunks` with a real
//! FK and a CHECKed role. `provenance_json` may also describe them, but it is
//! never the only record — a card whose chunks live solely in a JSON blob cannot
//! be joined, cannot be checked against the store, and cannot survive the chunk
//! being deleted without going quietly stale.
//!
//! # Only validated cards reach here
//!
//! A card is written after the task's validator has already proved that every
//! `chunk_id` was actually sent to the model and every page matches the store.
//! This module does not re-litigate that; it does enforce what the schema can —
//! FKs and the role CHECK — so a caller that skips validation still cannot
//! orphan a row.

use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::error::GaplyError;
use crate::now_epoch;

/// The role a chunk played in a card. Mirrors the schema's CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChunkRole {
    Primary,
    Supporting,
    Contradicting,
}

impl ChunkRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ChunkRole::Primary => "primary",
            ChunkRole::Supporting => "supporting",
            ChunkRole::Contradicting => "contradicting",
        }
    }
}

/// A card to write. Provenance is not optional: `model_id`, `model_version` and
/// `prompt_version` are required so a stored judgement can always say which
/// weights and which prompt produced it.
#[derive(Debug, Clone)]
pub struct NewEvidenceCard {
    pub document_id: i64,
    /// The primary chunk, or `None` for a multi-source card.
    pub chunk_id: Option<i64>,
    pub page: Option<u32>,
    pub claim: String,
    pub evidence_text: String,
    pub evidence_type: String,
    pub verdict: Option<String>,
    pub confidence: Option<f64>,
    pub model_id: String,
    pub model_version: String,
    pub prompt_version: String,
    /// Free-form provenance — advisories, retrieval parameters, dropped-chunk
    /// counts. Duplicating the chunk refs here is allowed; relying on it is not.
    pub provenance_json: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StoredCard {
    pub id: i64,
    pub document_id: i64,
    pub chunk_id: Option<i64>,
    pub page: Option<u32>,
    pub claim: String,
    pub evidence_text: String,
    pub evidence_type: String,
    pub verdict: Option<String>,
    pub confidence: Option<f64>,
    pub model_id: String,
    pub model_version: String,
    pub prompt_version: String,
    pub provenance_json: String,
    pub created_at: i64,
}

const COLS: &str = "id, document_id, chunk_id, page, claim, evidence_text, evidence_type, \
                    verdict, confidence, model_id, model_version, prompt_version, \
                    provenance_json, created_at";

fn row_to_card(r: &rusqlite::Row) -> rusqlite::Result<StoredCard> {
    Ok(StoredCard {
        id: r.get(0)?,
        document_id: r.get(1)?,
        chunk_id: r.get(2)?,
        page: r.get::<_, Option<i64>>(3)?.map(|p| p as u32),
        claim: r.get(4)?,
        evidence_text: r.get(5)?,
        evidence_type: r.get(6)?,
        verdict: r.get(7)?,
        confidence: r.get(8)?,
        model_id: r.get(9)?,
        model_version: r.get(10)?,
        prompt_version: r.get(11)?,
        provenance_json: r.get(12)?,
        created_at: r.get(13)?,
    })
}

/// Write a card and its chunk references in ONE transaction.
///
/// Atomic on purpose: a card whose join rows failed to write would claim
/// provenance it cannot produce, which is worse than no card.
pub fn insert_card(
    db: &Database,
    card: &NewEvidenceCard,
    chunks: &[(i64, ChunkRole)],
) -> Result<i64, GaplyError> {
    let now = now_epoch();
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO ai_evidence_cards
             (document_id, chunk_id, page, claim, evidence_text, evidence_type, verdict,
              confidence, model_id, model_version, prompt_version, provenance_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            card.document_id,
            card.chunk_id,
            card.page.map(i64::from),
            card.claim,
            card.evidence_text,
            card.evidence_type,
            card.verdict,
            card.confidence,
            card.model_id,
            card.model_version,
            card.prompt_version,
            card.provenance_json,
            now
        ],
    )?;
    let id = tx.last_insert_rowid();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO ai_evidence_card_chunks (card_id, chunk_id, role) VALUES (?1, ?2, ?3)",
        )?;
        for (chunk_id, role) in chunks {
            stmt.execute(params![id, chunk_id, role.as_str()])?;
        }
    }
    tx.commit()?;
    Ok(id)
}

pub fn get_card(db: &Database, id: i64) -> Result<Option<StoredCard>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM ai_evidence_cards WHERE id = ?1"))?;
    let mut rows = stmt.query_map(params![id], row_to_card)?;
    Ok(rows.next().transpose()?)
}

pub fn list_cards(db: &Database, document_id: i64) -> Result<Vec<StoredCard>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM ai_evidence_cards WHERE document_id = ?1 ORDER BY id"
    ))?;
    let rows = stmt.query_map(params![document_id], row_to_card)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The chunks a card drew on, from the JOIN TABLE — the queryable record.
pub fn card_chunks(db: &Database, card_id: i64) -> Result<Vec<(i64, String)>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT chunk_id, role FROM ai_evidence_card_chunks WHERE card_id = ?1 ORDER BY chunk_id",
    )?;
    let rows = stmt.query_map(params![card_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_engine::{registry, store};
    use crate::chunk::PagedChunk;

    fn seed() -> (Database, i64, Vec<i64>) {
        let db = Database::in_memory().unwrap();
        let doc = store::create_document(&db, "Doc", "/tmp/d.pdf", "sum-card").unwrap();
        registry::register_model(
            &db,
            registry::ModelRow {
                id: "m1".into(),
                kind: "generative".into(),
                display_name: "M".into(),
                file_path: "/tmp/m".into(),
                sha256: None,
                dim: None,
                quant: None,
            },
        )
        .unwrap();
        let chunks: Vec<PagedChunk> = (0..3)
            .map(|i| PagedChunk {
                seq: i,
                content: format!("chunk {i} content"),
                token_estimate: 3,
                page: Some(i as u32 + 1),
                section: Some("Results".into()),
                char_start: 0,
                char_end: 10,
            })
            .collect();
        store::index_chunks(&db, doc, &chunks).unwrap();
        let ids = store::list_chunks(&db, doc).unwrap().iter().map(|c| c.id).collect();
        (db, doc, ids)
    }

    fn card(doc: i64, chunk: Option<i64>) -> NewEvidenceCard {
        NewEvidenceCard {
            document_id: doc,
            chunk_id: chunk,
            page: Some(1),
            claim: "Organic farming increases soil biodiversity.".into(),
            evidence_text: "Richness rose 31%.".into(),
            evidence_type: "support".into(),
            verdict: Some("strong".into()),
            confidence: Some(0.8),
            model_id: "m1".into(),
            model_version: "q4km".into(),
            prompt_version: "citation_support-v1".into(),
            provenance_json: r#"{"advisories":[]}"#.into(),
        }
    }

    #[test]
    fn a_card_round_trips_with_full_provenance() {
        let (db, doc, ids) = seed();
        let id = insert_card(&db, &card(doc, Some(ids[0])), &[(ids[0], ChunkRole::Supporting)])
            .unwrap();
        let got = get_card(&db, id).unwrap().unwrap();
        assert_eq!(got.verdict.as_deref(), Some("strong"));
        assert_eq!(got.confidence, Some(0.8));
        assert_eq!(got.model_id, "m1");
        assert_eq!(got.model_version, "q4km");
        assert_eq!(got.prompt_version, "citation_support-v1");
        assert_eq!(got.page, Some(1));
        // the chunk refs are QUERYABLE, not only in the JSON
        assert_eq!(card_chunks(&db, id).unwrap(), vec![(ids[0], "supporting".to_string())]);
    }

    #[test]
    fn advisories_survive_in_provenance_json() {
        let (db, doc, ids) = seed();
        let mut c = card(doc, Some(ids[0]));
        c.provenance_json = r#"{"advisories":["why: is 24 words; the limit is 20"]}"#.into();
        let id = insert_card(&db, &c, &[(ids[0], ChunkRole::Supporting)]).unwrap();
        let got = get_card(&db, id).unwrap().unwrap();
        let v: serde_json::Value = serde_json::from_str(&got.provenance_json).unwrap();
        assert_eq!(v["advisories"][0], "why: is 24 words; the limit is 20");
    }

    #[test]
    fn a_multi_source_card_may_have_no_primary_chunk_but_still_has_join_rows() {
        let (db, doc, ids) = seed();
        let id = insert_card(
            &db,
            &card(doc, None),
            &[(ids[0], ChunkRole::Supporting), (ids[1], ChunkRole::Contradicting)],
        )
        .unwrap();
        assert_eq!(get_card(&db, id).unwrap().unwrap().chunk_id, None);
        let refs = card_chunks(&db, id).unwrap();
        assert_eq!(refs.len(), 2, "a card with no primary must still record its chunks");
    }

    #[test]
    fn an_orphan_chunk_reference_aborts_the_whole_card() {
        // Atomicity is the point: a card whose join rows failed would claim
        // provenance it cannot produce.
        let (db, doc, ids) = seed();
        let before = list_cards(&db, doc).unwrap().len();
        let err = insert_card(
            &db,
            &card(doc, Some(ids[0])),
            &[(ids[0], ChunkRole::Supporting), (999_999, ChunkRole::Supporting)],
        );
        assert!(err.is_err(), "an orphan chunk reference was accepted");
        assert_eq!(
            list_cards(&db, doc).unwrap().len(),
            before,
            "the card survived even though its chunk refs failed"
        );
    }

    #[test]
    fn an_unregistered_model_id_is_refused() {
        let (db, doc, ids) = seed();
        let mut c = card(doc, Some(ids[0]));
        c.model_id = "never-registered".into();
        assert!(insert_card(&db, &c, &[(ids[0], ChunkRole::Supporting)]).is_err());
    }

    #[test]
    fn deleting_a_chunk_takes_its_join_rows_with_it() {
        let (db, doc, ids) = seed();
        let id = insert_card(&db, &card(doc, Some(ids[0])), &[(ids[0], ChunkRole::Supporting)])
            .unwrap();
        store::clear_chunks(&db, doc).unwrap();
        // The join rows cascade; a stale reference to a deleted chunk would be
        // a card pointing at text nobody can read.
        assert!(card_chunks(&db, id).unwrap().is_empty());
    }
}
