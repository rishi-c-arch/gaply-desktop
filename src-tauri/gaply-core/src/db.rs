use std::path::{Path, PathBuf};
use std::sync::Once;

use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::Serialize;

use crate::error::GaplyError;
use crate::migrations;
use crate::projects::{NewProject, Project};

/// Storage abstraction the business logic depends on.
///
/// Core functions take `&dyn ProjectStore`, so tests mock this trait and the
/// shell decides the real backend. Keep methods at repository granularity
/// (domain operations, not raw SQL) so mocks stay trivial.
pub trait ProjectStore: Send + Sync {
    fn insert_project(&self, new: &NewProject) -> Result<Project, GaplyError>;
    fn list_projects(&self) -> Result<Vec<Project>, GaplyError>;
    fn get_project(&self, id: i64) -> Result<Option<Project>, GaplyError>;
    /// Cheap connectivity check for health reporting.
    fn ping(&self) -> Result<(), GaplyError>;
}

/// Register the sqlite-vec extension for every future connection.
/// Must run before the first connection is opened; `Once` makes it safe to
/// call from every constructor.
fn register_sqlite_extensions() {
    type ExtensionInit = unsafe extern "C" fn(
        *mut rusqlite::ffi::sqlite3,
        *mut *mut std::os::raw::c_char,
        *const rusqlite::ffi::sqlite3_api_routines,
    ) -> std::os::raw::c_int;

    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<*const (), ExtensionInit>(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

/// A raw connection with extensions loaded — for migration tests.
#[cfg(test)]
pub(crate) fn test_connection() -> rusqlite::Connection {
    register_sqlite_extensions();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    conn
}

#[derive(Debug, Serialize)]
pub struct DbInitReport {
    pub path: String,
    pub journal_mode: String,
    pub schema_version: i64,
}

#[derive(Debug, Serialize)]
pub struct MigrationReport {
    pub from_version: i64,
    pub to_version: i64,
    pub applied: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DbHealth {
    pub ok: bool,
    pub journal_mode: String,
    pub schema_version: i64,
    pub latest_version: i64,
    pub pending_migrations: i64,
    pub vec_version: Option<String>,
    pub size_bytes: i64,
}

/// Embedded SQLite database: connection pool, migrations, vector search.
/// No separate database service — everything lives in one file in the OS
/// app-data directory (WAL mode), or in memory for tests.
pub struct Database {
    pub(crate) pool: r2d2::Pool<SqliteConnectionManager>,
    path: Option<PathBuf>,
}

impl Database {
    /// Open (or create) the database at `path`, enable WAL, run migrations.
    pub fn open(path: &Path) -> Result<Self, GaplyError> {
        register_sqlite_extensions();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let manager = SqliteConnectionManager::file(path)
            .with_init(|c| c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;"));
        let pool = r2d2::Pool::builder().max_size(8).build(manager)?;
        let db = Self { pool, path: Some(path.to_path_buf()) };
        let report = db.migrate()?;
        tracing::info!(
            db = %path.display(),
            schema_version = report.to_version,
            applied = report.applied.len(),
            "sqlite store ready"
        );
        Ok(db)
    }

    /// In-memory database for tests. Pool is capped at one connection so
    /// every call sees the same memory database.
    pub fn in_memory() -> Result<Self, GaplyError> {
        register_sqlite_extensions();
        let manager = SqliteConnectionManager::memory()
            .with_init(|c| c.execute_batch("PRAGMA foreign_keys=ON;"));
        let pool = r2d2::Pool::builder().max_size(1).build(manager)?;
        let db = Self { pool, path: None };
        db.migrate()?;
        Ok(db)
    }

    pub(crate) fn conn(
        &self,
    ) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, GaplyError> {
        self.pool.get().map_err(Into::into)
    }

    fn journal_mode(&self) -> Result<String, GaplyError> {
        Ok(self.conn()?.query_row("PRAGMA journal_mode", [], |r| r.get(0))?)
    }

    /// Idempotent initialization report: confirms the file, journal mode and
    /// schema version. Safe to call from the frontend at any time.
    pub fn init_report(&self) -> Result<DbInitReport, GaplyError> {
        let conn = self.conn()?;
        let schema_version = migrations::current_version(&conn)?;
        drop(conn);
        Ok(DbInitReport {
            path: self
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ":memory:".into()),
            journal_mode: self.journal_mode()?,
            schema_version,
        })
    }

    /// Apply pending migrations.
    pub fn migrate(&self) -> Result<MigrationReport, GaplyError> {
        let mut conn = self.conn()?;
        let from_version = migrations::current_version(&conn)?;
        let applied = migrations::migrate_up(&mut conn)?
            .into_iter()
            .map(String::from)
            .collect();
        let to_version = migrations::current_version(&conn)?;
        Ok(MigrationReport { from_version, to_version, applied })
    }

    /// Create a manuscript row and return its id. Manuscripts own the
    /// `extractions`/`findings` produced by the Extraction Agent.
    pub fn create_manuscript(
        &self,
        title: &str,
        authors: &str,
        abstract_: &str,
    ) -> Result<i64, GaplyError> {
        if title.trim().is_empty() {
            return Err(GaplyError::Validation("manuscript title must not be empty".into()));
        }
        let now = crate::now_epoch();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO manuscripts (title, authors, abstract, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'extracted', ?4, ?4)",
            params![title.trim(), authors, abstract_, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Row count for a known table. `table` must be a trusted identifier
    /// (it is interpolated into SQL), so this is intended for tests and
    /// internal diagnostics, not for user-supplied names.
    pub fn count_rows(&self, table: &str) -> Result<i64, GaplyError> {
        Ok(self.conn()?.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
    }

    pub fn health(&self) -> Result<DbHealth, GaplyError> {
        let conn = self.conn()?;
        let ping_ok: bool = conn.query_row("SELECT 1", [], |_| Ok(true)).unwrap_or(false);
        let schema_version = migrations::current_version(&conn)?;
        let latest_version = migrations::latest_version();
        let vec_version: Option<String> =
            conn.query_row("SELECT vec_version()", [], |r| r.get(0)).ok();
        let size_bytes: i64 = conn
            .query_row(
                "SELECT (SELECT * FROM pragma_page_count()) * (SELECT * FROM pragma_page_size())",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        drop(conn);
        Ok(DbHealth {
            ok: ping_ok && schema_version == latest_version && vec_version.is_some(),
            journal_mode: self.journal_mode()?,
            schema_version,
            latest_version,
            pending_migrations: latest_version - schema_version,
            vec_version,
            size_bytes,
        })
    }
}

impl ProjectStore for Database {
    fn insert_project(&self, new: &NewProject) -> Result<Project, GaplyError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO projects (name, description, created_at) VALUES (?1, ?2, ?3)",
            params![new.name, new.description, new.created_at],
        )
        .map_err(|e| match e.into() {
            GaplyError::Conflict(_) => {
                GaplyError::Conflict(format!("a project named \"{}\" already exists", new.name))
            }
            other => other,
        })?;
        let id = conn.last_insert_rowid();
        Ok(Project {
            id,
            name: new.name.clone(),
            description: new.description.clone(),
            created_at: new.created_at,
        })
    }

    fn list_projects(&self) -> Result<Vec<Project>, GaplyError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, created_at FROM projects ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn get_project(&self, id: i64) -> Result<Option<Project>, GaplyError> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT id, name, description, created_at FROM projects WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }

    fn ping(&self) -> Result<(), GaplyError> {
        self.conn()?
            .query_row("SELECT 1", [], |_| Ok(()))
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn insert_and_read_back() {
        let s = store();
        let new = NewProject::now("Citation Atlas", "Graph of citations");
        let created = s.insert_project(&new).unwrap();
        assert!(created.id > 0);

        let fetched = s.get_project(created.id).unwrap().unwrap();
        assert_eq!(fetched.name, "Citation Atlas");
        assert_eq!(fetched.description, "Graph of citations");
    }

    #[test]
    fn duplicate_name_is_conflict() {
        let s = store();
        s.insert_project(&NewProject::now("Twin", "")).unwrap();
        let err = s.insert_project(&NewProject::now("Twin", "")).unwrap_err();
        assert!(matches!(err, GaplyError::Conflict(_)), "got: {err:?}");
    }

    #[test]
    fn list_is_newest_first() {
        let s = store();
        let mut a = NewProject::now("A", "");
        a.created_at = 100;
        let mut b = NewProject::now("B", "");
        b.created_at = 200;
        s.insert_project(&a).unwrap();
        s.insert_project(&b).unwrap();
        let names: Vec<_> = s.list_projects().unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["B", "A"]);
    }

    #[test]
    fn missing_project_is_none() {
        assert!(store().get_project(999).unwrap().is_none());
    }

    #[test]
    fn ping_succeeds() {
        assert!(store().ping().is_ok());
    }

    #[test]
    fn health_reports_ok_with_vec_loaded() {
        let h = store().health().unwrap();
        assert!(h.ok, "health: {h:?}");
        assert_eq!(h.schema_version, crate::migrations::latest_version());
        assert_eq!(h.pending_migrations, 0);
        assert!(h.vec_version.is_some(), "sqlite-vec extension not loaded");
    }

    #[test]
    fn init_report_for_memory_db() {
        let r = store().init_report().unwrap();
        assert_eq!(r.path, ":memory:");
        assert_eq!(r.schema_version, crate::migrations::latest_version());
    }

    #[test]
    fn open_on_disk_enables_wal() {
        let dir = std::env::temp_dir().join(format!("gaply-test-{}", std::process::id()));
        let path = dir.join("wal-test.db");
        let db = Database::open(&path).unwrap();
        let r = db.init_report().unwrap();
        assert_eq!(r.journal_mode, "wal");
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
