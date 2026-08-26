//! Vector storage and similarity for `ai_chunk_embeddings`.
//!
//! Pure and local: this module stores and compares vectors, it never produces
//! them. Producing a vector needs a model, models need candle, and candle lives
//! in the app crate — the same seam that keeps `cargo test -p gaply_core`
//! free of TLS and ML.
//!
//! # Embedding-space identity
//!
//! A vector is comparable ONLY to another produced by the same model AND the
//! same preprocessing. Pooling, the query prefix and normalization are all part
//! of the representation, so `model_id` alone does not identify a space —
//! flipping pooling from mean to CLS changes the vectors without changing the
//! model. [`EmbeddingSpace`] is the pair, both halves are stored on every row,
//! and [`resolve_single_space`] refuses a mixture rather than silently comparing
//! vectors from two spaces, which would return confident nonsense.
//!
//! # Encoding
//!
//! `f32` little-endian, packed. Explicit rather than platform-native, so a
//! database file stays readable if it is ever moved between machines.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::GaplyError;
use crate::now_epoch;

/// The identity of a vector space: which model, produced how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingSpace {
    pub model_id: String,
    pub preprocessing_version: String,
}

impl EmbeddingSpace {
    pub fn new(model_id: impl Into<String>, preprocessing_version: impl Into<String>) -> Self {
        Self { model_id: model_id.into(), preprocessing_version: preprocessing_version.into() }
    }
}

impl std::fmt::Display for EmbeddingSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.model_id, self.preprocessing_version)
    }
}

/// A chunk that still needs a vector in a given space.
#[derive(Debug, Clone, Serialize)]
pub struct PendingChunk {
    pub chunk_id: i64,
    pub content: String,
}

/// Pack `f32`s as little-endian bytes.
pub fn vector_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Unpack a little-endian `f32` blob. A length that is not a multiple of 4 is a
/// corrupt row and is reported, never silently truncated.
pub fn blob_to_vector(b: &[u8]) -> Result<Vec<f32>, GaplyError> {
    if b.len() % 4 != 0 {
        return Err(GaplyError::Database(format!(
            "embedding blob is {} bytes, not a whole number of f32s",
            b.len()
        )));
    }
    Ok(b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// Cosine similarity. Vectors are stored L2-normalized, so this is a dot
/// product in practice, but the norms are divided out anyway: a caller that
/// stores un-normalized vectors gets a correct answer instead of a silently
/// scaled one. Returns 0.0 when either vector is all zeros — an honest "no
/// similarity information", not a division by zero.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// L2-normalize in place. A zero vector is left alone rather than turned into
/// NaNs.
pub fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Store one vector. Upsert on `(chunk_id, model_id)` so re-embedding a chunk
/// replaces its vector rather than accumulating duplicates.
pub fn put_embedding(
    db: &Database,
    chunk_id: i64,
    space: &EmbeddingSpace,
    vector: &[f32],
) -> Result<(), GaplyError> {
    db.conn()?.execute(
        "INSERT INTO ai_chunk_embeddings
             (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (chunk_id, model_id) DO UPDATE SET
             preprocessing_version = excluded.preprocessing_version,
             dim = excluded.dim,
             vector = excluded.vector,
             created_at = excluded.created_at",
        params![
            chunk_id,
            space.model_id,
            space.preprocessing_version,
            vector.len() as i64,
            vector_to_blob(vector),
            now_epoch()
        ],
    )?;
    Ok(())
}

/// Store a batch in one transaction.
pub fn put_embeddings(
    db: &Database,
    space: &EmbeddingSpace,
    rows: &[(i64, Vec<f32>)],
) -> Result<usize, GaplyError> {
    let now = now_epoch();
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO ai_chunk_embeddings
                 (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (chunk_id, model_id) DO UPDATE SET
                 preprocessing_version = excluded.preprocessing_version,
                 dim = excluded.dim,
                 vector = excluded.vector,
                 created_at = excluded.created_at",
        )?;
        for (chunk_id, v) in rows {
            stmt.execute(params![
                chunk_id,
                space.model_id,
                space.preprocessing_version,
                v.len() as i64,
                vector_to_blob(v),
                now
            ])?;
        }
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Chunks with no vector yet in this model's space.
///
/// This is what makes embedding RESUMABLE BY CONSTRUCTION: a cancelled or
/// crashed run simply leaves rows pending, and the next run picks up exactly
/// those. There is no progress cursor to get out of step with reality.
pub fn chunks_missing_embeddings(
    db: &Database,
    document_id: Option<i64>,
    model_id: &str,
) -> Result<Vec<PendingChunk>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT c.id, c.content
         FROM ai_chunks c
         LEFT JOIN ai_chunk_embeddings e
           ON e.chunk_id = c.id AND e.model_id = ?2
         WHERE e.id IS NULL AND (?1 IS NULL OR c.document_id = ?1)
         ORDER BY c.id",
    )?;
    let rows = stmt.query_map(params![document_id, model_id], |r| {
        Ok(PendingChunk { chunk_id: r.get(0)?, content: r.get(1)? })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Every distinct embedding space present in the table.
pub fn distinct_spaces(db: &Database) -> Result<Vec<EmbeddingSpace>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT model_id, preprocessing_version FROM ai_chunk_embeddings
         ORDER BY model_id, preprocessing_version",
    )?;
    let rows = stmt.query_map([], |r| Ok(EmbeddingSpace { model_id: r.get(0)?, preprocessing_version: r.get(1)? }))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The one space search may run in, or a typed error.
///
/// Retrieval over a mixture of spaces does not degrade gracefully — it returns
/// confident scores computed across incompatible representations, which is
/// worse than returning nothing. So a mixture is refused by name, and the error
/// says which spaces were found so the caller can re-embed deliberately.
pub fn resolve_single_space(db: &Database) -> Result<EmbeddingSpace, GaplyError> {
    let spaces = distinct_spaces(db)?;
    match spaces.len() {
        1 => Ok(spaces.into_iter().next().expect("len checked")),
        0 => Err(GaplyError::NotFound { entity: "embedding space", id: "none stored".into() }),
        _ => Err(GaplyError::Conflict(format!(
            "refusing to search across {} embedding spaces ({}); vectors from different \
             models or preprocessing versions are not comparable — re-embed to one space",
            spaces.len(),
            spaces.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// Load vectors for specific chunks, in one space.
pub fn load_vectors(
    db: &Database,
    chunk_ids: &[i64],
    space: &EmbeddingSpace,
) -> Result<Vec<(i64, Vec<f32>)>, GaplyError> {
    if chunk_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let conn = db.conn()?;
    let sql = format!(
        "SELECT chunk_id, vector FROM ai_chunk_embeddings
         WHERE model_id = ? AND preprocessing_version = ? AND chunk_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(chunk_ids.len() + 2);
    args.push(Box::new(space.model_id.clone()));
    args.push(Box::new(space.preprocessing_version.clone()));
    for id in chunk_ids {
        args.push(Box::new(*id));
    }
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())), |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
    })?;
    rows.map(|r| {
        let (id, blob) = r?;
        Ok((id, blob_to_vector(&blob)?))
    })
    .collect()
}

/// Every vector in one space, optionally scoped to a document. The fallback
/// path when the lexical prefilter is too thin; brute force is acceptable at
/// current scale (plan §11 D7).
pub fn load_all_vectors(
    db: &Database,
    document_id: Option<i64>,
    space: &EmbeddingSpace,
) -> Result<Vec<(i64, Vec<f32>)>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT e.chunk_id, e.vector
         FROM ai_chunk_embeddings e
         JOIN ai_chunks c ON c.id = e.chunk_id
         WHERE e.model_id = ?1 AND e.preprocessing_version = ?2
           AND (?3 IS NULL OR c.document_id = ?3)",
    )?;
    let rows = stmt.query_map(params![space.model_id, space.preprocessing_version, document_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
    })?;
    rows.map(|r| {
        let (id, blob) = r?;
        Ok((id, blob_to_vector(&blob)?))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_round_trips_and_rejects_a_truncated_row() {
        let v = vec![1.0f32, -0.5, 0.25, 0.0];
        assert_eq!(blob_to_vector(&vector_to_blob(&v)).unwrap(), v);
        assert!(blob_to_vector(&[0u8, 1, 2]).is_err(), "a partial f32 was accepted");
    }

    #[test]
    fn cosine_matches_hand_computed_values() {
        // identical → 1
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        // orthogonal → 0
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        // opposite → -1
        assert!((cosine(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-6);
        // 45° → cos 45° = 0.70710678
        assert!((cosine(&[1.0, 0.0], &[1.0, 1.0]) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        // magnitude-invariant: scaling either side changes nothing
        assert!((cosine(&[3.0, 4.0], &[6.0, 8.0]) - 1.0).abs() < 1e-6);
        // hand-computed: a·b = 1*4 + 2*5 + 3*6 = 32; |a| = sqrt(14); |b| = sqrt(77)
        let expected = 32.0f32 / (14.0f32.sqrt() * 77.0f32.sqrt());
        assert!((cosine(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]) - expected).abs() < 1e-6);
        // degenerate inputs are 0, never NaN
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0, "mismatched dims must not compare");
    }

    #[test]
    fn l2_normalize_matches_hand_computed_values() {
        let mut v = vec![3.0f32, 4.0];
        l2_normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
        assert!((v.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-6);
        // a zero vector stays zero rather than becoming NaN
        let mut z = vec![0.0f32, 0.0];
        l2_normalize(&mut z);
        assert_eq!(z, vec![0.0, 0.0]);
    }
}
