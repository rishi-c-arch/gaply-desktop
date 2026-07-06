//! Vector search over the `embeddings` vec0 virtual table (sqlite-vec).

use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::error::GaplyError;

/// all-MiniLM-L6-v2 output dimension.
pub const EMBEDDING_DIM: usize = 384;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KnnMatch {
    pub rowid: i64,
    pub distance: f64,
    pub source_type: String,
    pub source_id: i64,
}

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn check_dim(vector: &[f32]) -> Result<(), GaplyError> {
    if vector.len() != EMBEDDING_DIM {
        return Err(GaplyError::Validation(format!(
            "embedding must have {EMBEDDING_DIM} dimensions, got {}",
            vector.len()
        )));
    }
    Ok(())
}

impl Database {
    /// Store one embedding tagged with its source (e.g. "manuscript", 12).
    #[tracing::instrument(skip(self, vector), fields(source_type, source_id))]
    pub fn insert_embedding(
        &self,
        source_type: &str,
        source_id: i64,
        vector: &[f32],
    ) -> Result<i64, GaplyError> {
        check_dim(vector)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO embeddings (embedding, source_type, source_id) VALUES (?1, ?2, ?3)",
            params![vec_to_blob(vector), source_type, source_id],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// K-nearest-neighbour search; results ordered by ascending distance.
    #[tracing::instrument(skip(self, query))]
    pub fn knn_embeddings(&self, query: &[f32], k: usize) -> Result<Vec<KnnMatch>, GaplyError> {
        check_dim(query)?;
        if k == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT rowid, distance, source_type, source_id
             FROM embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance",
        )?;
        let rows = stmt.query_map(params![vec_to_blob(query), k as i64], |row| {
            Ok(KnnMatch {
                rowid: row.get(0)?,
                distance: row.get(1)?,
                source_type: row.get(2)?,
                source_id: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 384-dim unit vector with 1.0 at `hot`.
    fn basis(hot: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; EMBEDDING_DIM];
        v[hot] = 1.0;
        v
    }

    #[test]
    fn knn_returns_nearest_first_for_384_dim_vectors() {
        let db = Database::in_memory().unwrap();

        // three vectors: axis-0, axis-1, and one close to axis-0
        db.insert_embedding("manuscript", 1, &basis(0)).unwrap();
        db.insert_embedding("manuscript", 2, &basis(1)).unwrap();
        let mut near_axis0 = basis(0);
        near_axis0[1] = 0.15;
        db.insert_embedding("guideline", 3, &near_axis0).unwrap();

        let matches = db.knn_embeddings(&basis(0), 2).unwrap();
        assert_eq!(matches.len(), 2);
        // exact match first (distance 0), then the nearby vector
        assert_eq!(matches[0].source_id, 1);
        assert!(matches[0].distance < 1e-6);
        assert_eq!(matches[1].source_id, 3);
        assert!(matches[1].distance < 1.0, "distance: {}", matches[1].distance);
        // the orthogonal vector must not appear in a k=2 result
        assert!(matches.iter().all(|m| m.source_id != 2));
    }

    #[test]
    fn aux_columns_roundtrip() {
        let db = Database::in_memory().unwrap();
        db.insert_embedding("retraction", 77, &basis(5)).unwrap();
        let m = &db.knn_embeddings(&basis(5), 1).unwrap()[0];
        assert_eq!(m.source_type, "retraction");
        assert_eq!(m.source_id, 77);
    }

    #[test]
    fn wrong_dimension_is_validation_error() {
        let db = Database::in_memory().unwrap();
        let err = db.insert_embedding("x", 1, &[1.0, 2.0, 3.0]).unwrap_err();
        assert!(matches!(err, GaplyError::Validation(_)), "got {err:?}");
        let err = db.knn_embeddings(&[1.0; 100], 5).unwrap_err();
        assert!(matches!(err, GaplyError::Validation(_)), "got {err:?}");
    }
}
