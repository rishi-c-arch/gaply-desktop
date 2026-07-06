//! Episodic memory: a log of agent decisions, outcomes and corrections so
//! future runs can learn from past mistakes.

use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::error::GaplyError;

#[derive(Debug, Clone, Serialize)]
pub struct Episode {
    pub id: i64,
    pub agent: String,
    pub decision: String,
    /// JSON context captured at decision time.
    pub context: String,
    pub outcome: Option<String>,
    pub correction: Option<String>,
    pub created_at: i64,
}

impl Database {
    #[tracing::instrument(skip(self, context))]
    pub fn record_episode(
        &self,
        agent: &str,
        decision: &str,
        context: &str,
    ) -> Result<i64, GaplyError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO episodic_memory (agent, decision, context, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![agent, decision, context, crate::now_epoch()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Attach an outcome (and optional correction) to a past decision.
    pub fn record_outcome(
        &self,
        episode_id: i64,
        outcome: &str,
        correction: Option<&str>,
    ) -> Result<(), GaplyError> {
        let updated = self.conn()?.execute(
            "UPDATE episodic_memory SET outcome = ?2, correction = ?3 WHERE id = ?1",
            params![episode_id, outcome, correction],
        )?;
        if updated == 0 {
            return Err(GaplyError::NotFound { entity: "episode", id: episode_id.to_string() });
        }
        Ok(())
    }

    /// Most recent episodes for an agent, newest first.
    pub fn recent_episodes(&self, agent: &str, limit: usize) -> Result<Vec<Episode>, GaplyError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, agent, decision, context, outcome, correction, created_at
             FROM episodic_memory WHERE agent = ?1
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![agent, limit as i64], |row| {
            Ok(Episode {
                id: row.get(0)?,
                agent: row.get(1)?,
                decision: row.get(2)?,
                context: row.get(3)?,
                outcome: row.get(4)?,
                correction: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_correct_an_episode() {
        let db = Database::in_memory().unwrap();
        let id = db
            .record_episode("extraction-agent", "classified table 3 as statistics", "{\"page\":7}")
            .unwrap();
        db.record_outcome(id, "wrong", Some("table 3 is demographics, not statistics"))
            .unwrap();

        let episodes = db.recent_episodes("extraction-agent", 10).unwrap();
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].outcome.as_deref(), Some("wrong"));
        assert!(episodes[0].correction.as_deref().unwrap().contains("demographics"));
    }

    #[test]
    fn outcome_for_missing_episode_is_not_found() {
        let db = Database::in_memory().unwrap();
        assert!(matches!(
            db.record_outcome(404, "ok", None).unwrap_err(),
            GaplyError::NotFound { .. }
        ));
    }
}
