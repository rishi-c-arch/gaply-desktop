//! `ai_model_registry` persistence.
//!
//! The registry records WHICH weights a vector came from. It stores paths and
//! hashes; it never fetches, verifies or loads anything — acquisition and
//! verification live in the app crate, because only that side may touch the
//! network or an ML runtime.

use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::error::GaplyError;
use crate::now_epoch;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelRow {
    pub id: String,
    /// 'embedding' | 'generative' — CHECKed by the schema.
    pub kind: String,
    pub display_name: String,
    pub file_path: String,
    pub sha256: Option<String>,
    pub dim: Option<i64>,
    pub quant: Option<String>,
}

/// Register or update a model. Idempotent: re-installing the same model
/// refreshes its row rather than failing or duplicating.
pub fn register_model(db: &Database, m: ModelRow) -> Result<(), GaplyError> {
    db.conn()?.execute(
        "INSERT INTO ai_model_registry
             (id, kind, display_name, file_path, sha256, dim, quant, registered_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (id) DO UPDATE SET
             kind = excluded.kind,
             display_name = excluded.display_name,
             file_path = excluded.file_path,
             sha256 = excluded.sha256,
             dim = excluded.dim,
             quant = excluded.quant,
             registered_at = excluded.registered_at",
        params![m.id, m.kind, m.display_name, m.file_path, m.sha256, m.dim, m.quant, now_epoch()],
    )?;
    Ok(())
}

/// One model by id, or `None`.
pub fn get_model(db: &Database, id: &str) -> Result<Option<ModelRow>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, display_name, file_path, sha256, dim, quant
         FROM ai_model_registry WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |r| {
        Ok(ModelRow {
            id: r.get(0)?,
            kind: r.get(1)?,
            display_name: r.get(2)?,
            file_path: r.get(3)?,
            sha256: r.get(4)?,
            dim: r.get(5)?,
            quant: r.get(6)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}

/// Every registered model of a kind.
pub fn list_models(db: &Database, kind: &str) -> Result<Vec<ModelRow>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, display_name, file_path, sha256, dim, quant
         FROM ai_model_registry WHERE kind = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![kind], |r| {
        Ok(ModelRow {
            id: r.get(0)?,
            kind: r.get(1)?,
            display_name: r.get(2)?,
            file_path: r.get(3)?,
            sha256: r.get(4)?,
            dim: r.get(5)?,
            quant: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> ModelRow {
        ModelRow {
            id: "bge-small-en-v1.5".into(),
            kind: "embedding".into(),
            display_name: "BGE Small EN v1.5".into(),
            file_path: "/models/bge".into(),
            sha256: Some("abc".into()),
            dim: Some(384),
            quant: None,
        }
    }

    #[test]
    fn register_is_idempotent_and_round_trips() {
        let db = Database::in_memory().unwrap();
        register_model(&db, row()).unwrap();
        register_model(&db, ModelRow { display_name: "Renamed".into(), ..row() }).unwrap();
        let got = get_model(&db, "bge-small-en-v1.5").unwrap().unwrap();
        assert_eq!(got.display_name, "Renamed");
        assert_eq!(got.dim, Some(384));
        assert_eq!(list_models(&db, "embedding").unwrap().len(), 1, "re-register duplicated a row");
        assert!(list_models(&db, "generative").unwrap().is_empty());
    }

    #[test]
    fn an_unknown_kind_is_refused_by_the_schema() {
        let db = Database::in_memory().unwrap();
        assert!(register_model(&db, ModelRow { kind: "reranker".into(), ..row() }).is_err());
    }

    #[test]
    fn an_unregistered_model_is_none_not_an_error() {
        let db = Database::in_memory().unwrap();
        assert!(get_model(&db, "nothing").unwrap().is_none());
    }
}
