//! TTL cache backed by the `cache` table. Staleness is computed from
//! `created_at + ttl_seconds`; callers pass `now` explicitly so expiry is
//! deterministic and testable (production uses `crate::now_epoch()`).

use rusqlite::params;

use crate::db::Database;
use crate::error::GaplyError;

impl Database {
    /// Insert or replace a cache entry.
    pub fn cache_put(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: i64,
        now: i64,
    ) -> Result<(), GaplyError> {
        if ttl_seconds <= 0 {
            return Err(GaplyError::Validation("ttl_seconds must be positive".into()));
        }
        self.conn()?.execute(
            "INSERT OR REPLACE INTO cache (key, value, created_at, ttl_seconds)
             VALUES (?1, ?2, ?3, ?4)",
            params![key, value, now, ttl_seconds],
        )?;
        Ok(())
    }

    /// Fetch a value only if it is still fresh at `now`. Stale entries are
    /// treated as absent (and left for `cache_evict_stale` to clean up).
    pub fn cache_get(&self, key: &str, now: i64) -> Result<Option<String>, GaplyError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT value FROM cache WHERE key = ?1 AND created_at + ttl_seconds > ?2",
        )?;
        let mut rows = stmt.query_map(params![key, now], |row| row.get(0))?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Delete every entry whose TTL has elapsed at `now`; returns the count.
    pub fn cache_evict_stale(&self, now: i64) -> Result<usize, GaplyError> {
        let removed = self
            .conn()?
            .execute("DELETE FROM cache WHERE created_at + ttl_seconds <= ?1", params![now])?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_entry_is_returned() {
        let db = Database::in_memory().unwrap();
        db.cache_put("crossref:10.1000/x", "{\"title\":\"t\"}", 3600, 1000).unwrap();
        let hit = db.cache_get("crossref:10.1000/x", 1500).unwrap();
        assert_eq!(hit.as_deref(), Some("{\"title\":\"t\"}"));
    }

    #[test]
    fn stale_entry_reads_as_absent() {
        let db = Database::in_memory().unwrap();
        db.cache_put("k", "v", 60, 1000).unwrap();
        // 61 seconds later the entry has expired
        assert_eq!(db.cache_get("k", 1061).unwrap(), None);
    }

    #[test]
    fn evict_removes_only_stale_entries() {
        let db = Database::in_memory().unwrap();
        db.cache_put("old", "v1", 60, 1000).unwrap();
        db.cache_put("fresh", "v2", 3600, 1000).unwrap();

        let removed = db.cache_evict_stale(2000).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(db.cache_get("old", 2000).unwrap(), None);
        assert_eq!(db.cache_get("fresh", 2000).unwrap().as_deref(), Some("v2"));
        // second sweep finds nothing
        assert_eq!(db.cache_evict_stale(2000).unwrap(), 0);
    }

    #[test]
    fn put_overwrites_and_refreshes_ttl() {
        let db = Database::in_memory().unwrap();
        db.cache_put("k", "old", 60, 1000).unwrap();
        db.cache_put("k", "new", 60, 2000).unwrap();
        assert_eq!(db.cache_get("k", 2030).unwrap().as_deref(), Some("new"));
    }

    #[test]
    fn non_positive_ttl_is_rejected() {
        let db = Database::in_memory().unwrap();
        assert!(matches!(
            db.cache_put("k", "v", 0, 1000).unwrap_err(),
            GaplyError::Validation(_)
        ));
    }
}
