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

/// Every registered model of a kind, MOST RECENTLY REGISTERED FIRST.
///
/// Distinct from [`list_models`] on purpose. That one orders by id, which is
/// stable but arbitrary; this one answers a different question — "which model
/// did the user most recently choose to install?" — and that is the question a
/// bake-off asks. Installing a new candidate makes it the preferred one without
/// any code change, which is the whole point of the registry being data (§9.9).
///
/// Ties (two installs inside the same wall-clock second) fall back to id, so
/// the order is total and the caller never sees a coin flip.
pub fn list_models_recent_first(db: &Database, kind: &str) -> Result<Vec<ModelRow>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, display_name, file_path, sha256, dim, quant
         FROM ai_model_registry WHERE kind = ?1
         ORDER BY registered_at DESC, id ASC",
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

    /// `registered_at` is second-resolution, so the test writes it directly
    /// rather than racing `now_epoch()` twice inside one second.
    fn register_at(db: &Database, id: &str, at: i64) {
        db.conn()
            .unwrap()
            .execute(
                "INSERT INTO ai_model_registry
                     (id, kind, display_name, file_path, sha256, dim, quant, registered_at)
                 VALUES (?1, 'generative', ?1, '/m', NULL, NULL, 'Q4_K_M', ?2)",
                params![id, at],
            )
            .unwrap();
    }

    #[test]
    fn recent_first_prefers_the_latest_install_and_breaks_ties_by_id() {
        let db = Database::in_memory().unwrap();
        register_at(&db, "qwen2.5-1.5b-instruct-q4km", 1000);
        register_at(&db, "qwen2.5-3b-instruct-q4km", 2000);
        // Registered in the same second as the 3B — the tie-break, not the clock,
        // has to decide, and it has to decide the same way every run.
        register_at(&db, "aaa-same-second", 2000);

        let ids: Vec<_> =
            list_models_recent_first(&db, "generative").unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(
            ids,
            ["aaa-same-second", "qwen2.5-3b-instruct-q4km", "qwen2.5-1.5b-instruct-q4km"],
            "newest first, then id"
        );
        // list_models keeps its own (id) ordering — this is an addition, not a change.
        assert_eq!(list_models(&db, "generative").unwrap()[0].id, "aaa-same-second");
        assert!(list_models_recent_first(&db, "embedding").unwrap().is_empty());
    }
}
