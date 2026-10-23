//! **The project store: the one Research OS module that writes to disk
//! (Phase 5B-1; decisions 99 to 112; ADR-009, ADR-004 §004b).**
//!
//! Its one caller is `crate::project_commands` (Phase 5B-2, decisions 118 to
//! 127): `tests/project_store_guards.rs` fails if any other non-test code
//! names it, and `tests/stats_desc_writes_nothing.rs` lists it as the only
//! Research OS module allowed to write.
//!
//! **One process-wide write lock** ([`write_lock`], Phase 5B-2) covers every
//! Save, create and delete, from the cap check to the commit, and the list:
//! two Saves into two projects cannot pass the all-projects cap together,
//! and a delete never runs during a Save. Opening a project and its recovery
//! do not take it; each project's own `BEGIN IMMEDIATE` orders those.
//!
//! **Two copies of the app (Phase 5B-3a, decision 132).** An open project
//! holds an operating-system lock on its empty lock file,
//! `.locks/<project_id>.lock`, shared, until it closes; the system releases
//! it if the app quits or crashes. Opening a project and a first Save take it
//! shared; a delete takes it exclusive and holds it until the folder is gone,
//! and only then removes the lock file; recovery at start takes it exclusive
//! before it finishes a tombstone or clears a staging folder, and skips one
//! it cannot lock. The order is always the same: the lock first, then the
//! check that the project's folder exists. Only `File::try_lock` and
//! `File::try_lock_shared` are called, never a blocking lock: `WouldBlock` is
//! `project.open_elsewhere` for a delete and `project.being_deleted` for an
//! open, and any other error is `project.lock_failed`. The locks folder is
//! never a project: the list, the sizes, the caps and recovery skip it.
//!
//! **A first Save** ([`ProjectRoot::create_with_save`], decision 121) is
//! written inside the project's `.creating-<id>/` folder, whose database
//! already holds its schema and its row, and the folder is renamed into place
//! only after the Save's commit: a project folder named by an id always holds
//! its first run.
//!
//! **The layout** (ADR-009 §2), under the root [`projects_root`] resolves
//! (Tauri's local app data directory, decision 99) or a folder a test passes:
//!
//! ```text
//! <root>/projects/                     0700
//!   <project_id>/                      0700, named by the id, never the title
//!     project.db                       0600, created before SQLite opens it
//!     project.db-wal, project.db-shm   SQLite's own, the db's mode; no size
//!                                      counts them (decision 173)
//!     artifacts/                       0700
//!       <sha256>                       0400, never overwritten
//!       .tmp-<32 hex>                  a write in progress
//!   .creating-<project_id>/            a create in progress
//!   .deleting-<project_id>/            a tombstone: a delete in progress
//!   .locks/                            0700, never a project (decision 132)
//!     <project_id>.lock                0600, empty: locked while open
//! ```
//!
//! Every path is joined from a parsed [`ProjectId`] or [`ArtifactHash`] or a
//! fixed name; a raw id or hash from a caller is parsed first, and refused
//! with a closed code if it is not 32 or 64 lowercase hex characters. No
//! symlink is followed: a project folder, `project.db`, `artifacts/` or an
//! artifact that is one is refused, and recovery and deletion step over
//! symlinks instead of into them.
//!
//! **A Save** ([`ProjectHandle::save`]) holds one `BEGIN IMMEDIATE` from first
//! to last: the caps; each artifact written to a temporary file, flushed and
//! synced, set to 0400, re-read and verified, renamed into place, and the
//! folder synced (on Windows a folder cannot be synced: the rename's
//! durability rests on NTFS's journal, and a lost rename leaves a row whose
//! file is missing, which recovery marks damaged); then the rows, and the
//! commit. If the rows or the commit fail, the files this Save renamed into
//! place are removed at once. An existing artifact is re-verified and
//! reused, never overwritten.
//!
//! **Recovery** ([`ProjectRoot::open`]) runs in its own `BEGIN IMMEDIATE`, so
//! it waits for, and never removes a file of, a writer holding the lock.
//!
//! **Logs** carry a project by its id and an artifact by the first 8
//! characters of its hash (`research_project::HASH_PREFIX_LEN`); never a
//! path, a title or a file's contents. Every refusal is a
//! [`ProjectRefusal`]: a closed code and, for a cap, two numbers.
//!
//! **Not tested on this Mac:** the `cfg(windows)` branches (clearing the
//! read-only attribute before a removal, the disk-full error numbers) and the
//! `cfg(unix)` permission bits on Windows. The Windows CI job runs
//! `gaply_core`'s tests only; see D281.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{CapabilityId, RunState, WorkspaceId};
use gaply_core::research_project::{
    damage_of, hex_lower, recovered_state, workspace_from_word, workspace_word, AgentVersion,
    ArtifactHash, ArtifactKind, ArtifactRef, Caps, DataFormat, FileState, ProjectCode, ProjectId,
    ProjectRecord, ProjectRefusal, ProjectTitle, ProvEdge, ProvGraph, StoredRun, StoredRunId,
    UsedRole, SCHEMA_VERSION,
};
use rusqlite::{params, Connection, ErrorCode, OpenFlags, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

const PROJECTS_DIR: &str = "projects";
const DB_FILE: &str = "project.db";
/// SQLite's own companions of `project.db` in WAL mode. No size counts them
/// (decision 173): they come and go with connections.
const WAL_FILE: &str = "project.db-wal";
const SHM_FILE: &str = "project.db-shm";
const ARTIFACTS_DIR: &str = "artifacts";
const TMP_PREFIX: &str = ".tmp-";
const CREATING_PREFIX: &str = ".creating-";
const DELETING_PREFIX: &str = ".deleting-";
/// The locks folder, beside the project folders (decision 132). Never a
/// valid project id, so the list never reads it as one.
const LOCKS_DIR: &str = ".locks";
const LOCK_SUFFIX: &str = ".lock";
/// Bytes per write and per read while hashing.
const CHUNK: usize = 64 * 1024;
/// How long a second connection waits for the write lock before `busy`.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// The provenance agent: this build of the app.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const SCHEMA_V1: &str = "
CREATE TABLE project (
    singleton      INTEGER PRIMARY KEY CHECK (singleton = 1),
    project_id     TEXT NOT NULL CHECK (length(project_id) = 32),
    title          TEXT NOT NULL,
    workspace_home TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    app_version    TEXT NOT NULL
);
CREATE TABLE artifacts (
    hash           TEXT PRIMARY KEY CHECK (length(hash) = 64),
    kind           TEXT NOT NULL,
    format         TEXT NOT NULL,
    bytes          INTEGER NOT NULL CHECK (bytes >= 0),
    schema_version INTEGER NOT NULL,
    created_at     INTEGER NOT NULL,
    damage         TEXT
) WITHOUT ROWID;
CREATE TABLE runs (
    run_id             INTEGER PRIMARY KEY,
    capability_id      TEXT NOT NULL,
    capability_version INTEGER NOT NULL,
    state              TEXT NOT NULL,
    end_code           TEXT,
    agent_version      TEXT NOT NULL,
    started_at         INTEGER NOT NULL,
    ended_at           INTEGER,
    saved_at           INTEGER NOT NULL
);
CREATE TABLE used (
    run_id INTEGER NOT NULL REFERENCES runs(run_id),
    hash   TEXT NOT NULL REFERENCES artifacts(hash),
    role   TEXT NOT NULL,
    PRIMARY KEY (run_id, hash, role)
) WITHOUT ROWID;
CREATE TABLE generated (
    hash   TEXT NOT NULL REFERENCES artifacts(hash),
    run_id INTEGER NOT NULL REFERENCES runs(run_id),
    PRIMARY KEY (hash, run_id)
) WITHOUT ROWID;
CREATE TABLE derived (
    hash       TEXT NOT NULL REFERENCES artifacts(hash),
    from_hash  TEXT NOT NULL REFERENCES artifacts(hash),
    via_run_id INTEGER NOT NULL REFERENCES runs(run_id),
    PRIMARY KEY (hash, from_hash, via_run_id)
) WITHOUT ROWID;
";

/// The folder projects live under: Tauri's local app data directory and no
/// other resolver (decision 99; pinned by `tests/project_store_guards.rs`).
/// 5B-2 calls it once and passes the result to [`ProjectRoot::open_root`].
pub fn projects_root<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<PathBuf, ProjectRefusal> {
    use tauri::Manager;
    app.path()
        .app_local_data_dir()
        .map_err(|_| ProjectRefusal::from(ProjectCode::RootUnavailable))
}

/* ------------------------------------------------------------------------ *
 *  Errors, as closed codes
 * ------------------------------------------------------------------------ */

fn is_disk_full(e: &io::Error) -> bool {
    match e.raw_os_error() {
        #[cfg(unix)]
        Some(c) => c == libc::ENOSPC || c == libc::EDQUOT,
        // ERROR_DISK_FULL and ERROR_HANDLE_DISK_FULL.
        #[cfg(windows)]
        Some(c) => c == 112 || c == 39,
        _ => false,
    }
}

fn io_code(e: &io::Error) -> ProjectCode {
    if is_disk_full(e) {
        ProjectCode::DiskFull
    } else {
        ProjectCode::IoFailed
    }
}

fn io_refusal(e: io::Error) -> ProjectRefusal {
    io_code(&e).into()
}

fn db_code(e: &rusqlite::Error) -> ProjectCode {
    match e {
        rusqlite::Error::SqliteFailure(f, _) => match f.code {
            ErrorCode::DiskFull => ProjectCode::DiskFull,
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => ProjectCode::Busy,
            ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt => ProjectCode::DbUnreadable,
            _ => ProjectCode::DbFailed,
        },
        _ => ProjectCode::DbFailed,
    }
}

fn db_refusal(e: rusqlite::Error) -> ProjectRefusal {
    db_code(&e).into()
}

/// While opening: anything but a lock is an unreadable database.
fn opening_refusal(e: rusqlite::Error) -> ProjectRefusal {
    match db_code(&e) {
        ProjectCode::Busy => ProjectCode::Busy.into(),
        _ => ProjectCode::DbUnreadable.into(),
    }
}

/* ------------------------------------------------------------------------ *
 *  Files: created owner-only, read without following a symlink
 * ------------------------------------------------------------------------ */

fn set_mode(p: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (p, mode);
        Ok(())
    }
}

fn make_private_dir(p: &Path) -> io::Result<()> {
    let mut b = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(p)?;
    set_mode(p, 0o700)
}

/// A new file, never one opened over an existing file or a symlink.
fn create_new_private_file(p: &Path, mode: u32) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(mode).custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let f = o.open(p)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(mode))?;
    }
    Ok(f)
}

fn open_read_nofollow(p: &Path) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW);
    }
    o.open(p)
}

fn set_read_only(f: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o400))
    }
    #[cfg(not(unix))]
    {
        let mut p = f.metadata()?.permissions();
        p.set_readonly(true);
        f.set_permissions(p)
    }
}

/// Makes a folder's entries durable. On Windows a folder cannot be opened to
/// sync; see the module header for what stands in its place.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Removes a file even if it is read-only: on Windows the attribute blocks a
/// deletion, so it is cleared first; on Unix the folder's mode decides.
fn remove_file_any(p: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        if let Ok(m) = fs::symlink_metadata(p) {
            let mut perm = m.permissions();
            if perm.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                fs::set_permissions(p, perm)?;
            }
        }
    }
    fs::remove_file(p)
}

/// Removes a folder and everything in it, never following a symlink: a
/// symlink inside is removed as a link, its target untouched.
fn remove_tree(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            remove_tree(&p)?;
        } else if ft.is_symlink() {
            fs::remove_file(&p).or_else(|_| fs::remove_dir(&p))?;
        } else {
            remove_file_any(&p)?;
        }
    }
    fs::remove_dir(dir)
}

/// The bytes of every regular file under `dir`, from metadata alone: nothing
/// is opened or hashed, and no symlink is followed. A missing folder is 0.
/// Files named exactly `project.db-wal` or `project.db-shm` are skipped
/// (decision 173): SQLite makes and removes them with its connections, so
/// counting them gave one project a different size at each reading. Every
/// other file counts, a tombstone's leftovers included.
pub(crate) fn tree_bytes(dir: &Path) -> io::Result<u64> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut n = 0u64;
    for entry in entries {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_dir() {
            n = n.saturating_add(tree_bytes(&entry.path())?);
        } else if ft.is_file() && !is_sqlite_companion(&entry.file_name()) {
            n = n.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(n)
}

fn is_sqlite_companion(name: &std::ffi::OsStr) -> bool {
    name == WAL_FILE || name == SHM_FILE
}

/// The bytes under the projects folder that count toward the caps and the
/// sizes: every project folder, staging folder and tombstone, and not the
/// locks folder, which is not a project (decision 132).
fn projects_bytes(projects: &Path) -> io::Result<u64> {
    let mut n = 0u64;
    for entry in fs::read_dir(projects)? {
        let entry = entry?;
        if entry.file_name() == LOCKS_DIR {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_dir() {
            n = n.saturating_add(tree_bytes(&entry.path())?);
        } else if ft.is_file() {
            n = n.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(n)
}

fn file_state(p: &Path) -> Result<FileState, ProjectRefusal> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_file() => Ok(FileState::Regular { len: m.len() }),
        Ok(_) => Ok(FileState::NotRegular),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(FileState::Missing),
        Err(e) => Err(io_refusal(e)),
    }
}

/// A real folder at `p`: `NotFound` if absent or not a folder, `symlink_refused`
/// if a symlink.
fn real_dir(p: &Path) -> Result<(), ProjectRefusal> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_symlink() => Err(ProjectCode::SymlinkRefused.into()),
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(ProjectCode::NotFound.into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ProjectCode::NotFound.into()),
        Err(e) => Err(io_refusal(e)),
    }
}

fn sha256_hash(bytes: &[u8]) -> Result<ArtifactHash, ProjectRefusal> {
    ArtifactHash::parse(&hex_lower(&Sha256::digest(bytes))).map_err(ProjectRefusal::from)
}

/// Reads at most `limit + 1` bytes of a file, without following a symlink.
fn read_bounded(p: &Path, limit: u64) -> Result<Vec<u8>, ProjectRefusal> {
    let f = open_read_nofollow(p).map_err(io_refusal)?;
    let mut v = Vec::new();
    f.take(limit.saturating_add(1))
        .read_to_end(&mut v)
        .map_err(io_refusal)?;
    Ok(v)
}

/// The SHA-256 of a file, streamed, without following a symlink.
fn hash_file(p: &Path) -> Result<ArtifactHash, ProjectRefusal> {
    let mut f = open_read_nofollow(p).map_err(io_refusal)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let k = f.read(&mut buf).map_err(io_refusal)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
    }
    ArtifactHash::parse(&hex_lower(&h.finalize())).map_err(ProjectRefusal::from)
}

fn random_hex32() -> Result<String, ProjectRefusal> {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).map_err(|_| ProjectRefusal::from(ProjectCode::IoFailed))?;
    Ok(hex_lower(&b))
}

/* ------------------------------------------------------------------------ *
 *  The database
 * ------------------------------------------------------------------------ */

/// Opens an existing `project.db`: never creates one, never follows a
/// symlink.
fn open_conn(path: &Path, busy: Duration) -> Result<Connection, ProjectRefusal> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let conn = Connection::open_with_flags(path, flags).map_err(opening_refusal)?;
    conn.busy_timeout(busy).map_err(opening_refusal)?;
    Ok(conn)
}

fn user_version(conn: &Connection) -> Result<i64, ProjectRefusal> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(opening_refusal)
}

/// Folds the write-ahead log into `project.db` and truncates it to 0 bytes
/// (decision 173), so `project.db` holds what was saved and its size is honest
/// while the project is open. Best effort: a checkpoint that is busy (another
/// connection is reading, past the busy timeout) or fails is logged by a
/// closed code only, and never fails its caller; SQLite folds the log in at a
/// later checkpoint or when the last connection closes.
fn checkpoint(conn: &Connection) {
    match conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(0) => {}
        Ok(_) => tracing::warn!(code = "project.checkpoint_busy", "checkpoint not completed"),
        Err(_) => tracing::warn!(
            code = "project.checkpoint_failed",
            "checkpoint not completed"
        ),
    }
}

/// WAL, so a reader never blocks the one writer; `synchronous=FULL`, so a
/// commit that returned survives a power cut (sqlite.org/pragma.html).
fn configure(conn: &Connection) -> Result<(), ProjectRefusal> {
    let mode: String = conn
        .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))
        .map_err(opening_refusal)?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(ProjectCode::DbFailed.into());
    }
    conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(opening_refusal)
}

/// Brings an empty database to schema version 1. A database already at 1 is
/// left as it is; one above it is refused, unchanged.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), ProjectRefusal> {
    let v = user_version(conn)?;
    if v > i64::from(SCHEMA_VERSION) {
        return Err(ProjectCode::DbNewerSchema.into());
    }
    if v == 0 {
        configure(conn)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_refusal)?;
        tx.execute_batch(SCHEMA_V1).map_err(db_refusal)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(db_refusal)?;
        tx.commit().map_err(db_refusal)?;
    }
    Ok(())
}

/// Checks the schema and the project row of an opened database, refusing a
/// newer or unfinished schema and a row that names another project.
fn check_db(conn: &Connection, id: &ProjectId) -> Result<ProjectRecord, ProjectRefusal> {
    let v = user_version(conn)?;
    if v > i64::from(SCHEMA_VERSION) {
        return Err(ProjectCode::DbNewerSchema.into());
    }
    if v < 1 {
        return Err(ProjectCode::DbUnreadable.into());
    }
    let record = read_record(conn).map_err(|_| ProjectRefusal::from(ProjectCode::DbUnreadable))?;
    if &record.id != id {
        return Err(ProjectCode::DbUnreadable.into());
    }
    Ok(record)
}

fn read_record(conn: &Connection) -> Result<ProjectRecord, ProjectRefusal> {
    let (id, title, ws, created_at, app): (String, String, String, i64, String) = conn
        .query_row(
            "SELECT project_id, title, workspace_home, created_at, app_version FROM project WHERE singleton = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(db_refusal)?;
    let bad = || ProjectRefusal::from(ProjectCode::DbUnreadable);
    Ok(ProjectRecord {
        id: ProjectId::parse(&id).map_err(|_| bad())?,
        title: ProjectTitle::parse(&title).map_err(|_| bad())?,
        workspace_home: workspace_from_word(&ws).ok_or_else(bad)?,
        created_at,
        app_version: AgentVersion::parse(&app).map_err(|_| bad())?,
    })
}

/// The stored edges as a graph: `used` and `generated` before `derived`, so
/// each derivation finds its links; a stored cycle is refused.
fn load_graph(conn: &Connection) -> Result<ProvGraph, ProjectRefusal> {
    let bad = || ProjectRefusal::from(ProjectCode::DbUnreadable);
    let mut edges = Vec::new();
    let mut st = conn
        .prepare("SELECT run_id, hash, role FROM used ORDER BY run_id, hash, role")
        .map_err(db_refusal)?;
    let rows = st
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(db_refusal)?;
    for row in rows {
        let (run, hash, role) = row.map_err(db_refusal)?;
        edges.push(ProvEdge::Used {
            run: StoredRunId(run as u64),
            artifact: ArtifactHash::parse(&hash).map_err(|_| bad())?,
            role: UsedRole::from_wire(&role).ok_or_else(bad)?,
        });
    }
    let mut st = conn
        .prepare("SELECT hash, run_id FROM generated ORDER BY run_id, hash")
        .map_err(db_refusal)?;
    let rows = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(db_refusal)?;
    for row in rows {
        let (hash, run) = row.map_err(db_refusal)?;
        edges.push(ProvEdge::WasGeneratedBy {
            artifact: ArtifactHash::parse(&hash).map_err(|_| bad())?,
            run: StoredRunId(run as u64),
        });
    }
    let mut st = conn
        .prepare("SELECT run_id, agent_version FROM runs ORDER BY run_id")
        .map_err(db_refusal)?;
    let rows = st
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(db_refusal)?;
    for row in rows {
        let (run, agent) = row.map_err(db_refusal)?;
        edges.push(ProvEdge::WasAssociatedWith {
            run: StoredRunId(run as u64),
            agent: AgentVersion::parse(&agent).map_err(|_| bad())?,
        });
    }
    let mut st = conn
        .prepare(
            "SELECT hash, from_hash, via_run_id FROM derived ORDER BY via_run_id, hash, from_hash",
        )
        .map_err(db_refusal)?;
    let rows = st
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(db_refusal)?;
    for row in rows {
        let (hash, from, via) = row.map_err(db_refusal)?;
        edges.push(ProvEdge::WasDerivedFrom {
            artifact: ArtifactHash::parse(&hash).map_err(|_| bad())?,
            from: ArtifactHash::parse(&from).map_err(|_| bad())?,
            via: StoredRunId(via as u64),
        });
    }
    ProvGraph::from_edges(edges).map_err(ProjectRefusal::from)
}

/* ------------------------------------------------------------------------ *
 *  The process-wide write lock
 * ------------------------------------------------------------------------ */

/// The store's one write lock, for every root (Phase 5B-2): held by a Save
/// from its cap check to its commit, by a create and a delete throughout,
/// and by the list. A thread that panicked holding it leaves nothing for the
/// next to trust but the disk, which every operation re-reads, so a poisoned
/// lock is taken as it is.
fn write_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/* ------------------------------------------------------------------------ *
 *  Projects open in this process
 * ------------------------------------------------------------------------ */

fn open_projects() -> &'static Mutex<BTreeMap<PathBuf, usize>> {
    static OPEN: OnceLock<Mutex<BTreeMap<PathBuf, usize>>> = OnceLock::new();
    OPEN.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn mark_open(dir: &Path) {
    let mut m = open_projects().lock().unwrap_or_else(|p| p.into_inner());
    *m.entry(dir.to_path_buf()).or_insert(0) += 1;
}

fn mark_closed(dir: &Path) {
    let mut m = open_projects().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(n) = m.get_mut(dir) {
        *n -= 1;
        if *n == 0 {
            m.remove(dir);
        }
    }
}

fn is_open(dir: &Path) -> bool {
    open_projects()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .contains_key(dir)
}

/* ------------------------------------------------------------------------ *
 *  The lock between copies of the app (Phase 5B-3a, decision 132)
 * ------------------------------------------------------------------------ */

/// Which lock is taken on a project's lock file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockKind {
    /// Opening a project, or a first Save: other copies may open it too.
    Shared,
    /// Deleting a project, or recovery finishing one: no other holder.
    Exclusive,
}

/// A project's lock file, locked. The operating system releases the lock
/// when this is dropped, or when the process ends however it ends; the file
/// itself stays until an exclusive holder removes it.
struct ProjectLock {
    /// Held for its lock, which the system releases when it is closed.
    _file: File,
    path: PathBuf,
}

impl ProjectLock {
    /// Removes the lock file. Only an exclusive holder calls this, and only
    /// once the project's folder is gone.
    fn remove_file(&self) -> io::Result<()> {
        remove_file_any(&self.path)
    }
}

/// Opens, or makes empty and owner-only, the lock file at `p`. Never one
/// that is a symlink, and never anything but a regular file.
fn open_lock_file(p: &Path) -> Result<File, ProjectRefusal> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_symlink() => return Err(ProjectCode::SymlinkRefused.into()),
        Ok(m) if !m.file_type().is_file() => return Err(ProjectCode::LockFailed.into()),
        _ => {}
    }
    let mut o = OpenOptions::new();
    o.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let f = o
        .open(p)
        .map_err(|_| ProjectRefusal::from(ProjectCode::LockFailed))?;
    match f.metadata() {
        Ok(m) if m.is_file() => Ok(f),
        _ => Err(ProjectCode::LockFailed.into()),
    }
}

/// `kind` on the lock file at `p`, through the `try_` calls only:
/// `WouldBlock` is `would_block`, any other failure `project.lock_failed`.
fn try_lock_file(
    p: &Path,
    kind: LockKind,
    would_block: ProjectCode,
) -> Result<ProjectLock, ProjectRefusal> {
    let file = open_lock_file(p)?;
    let taken = match kind {
        LockKind::Shared => file.try_lock_shared(),
        LockKind::Exclusive => file.try_lock(),
    };
    match taken {
        Ok(()) => Ok(ProjectLock {
            _file: file,
            path: p.to_path_buf(),
        }),
        Err(std::fs::TryLockError::WouldBlock) => Err(would_block.into()),
        Err(std::fs::TryLockError::Error(_)) => Err(ProjectCode::LockFailed.into()),
    }
}

/* ------------------------------------------------------------------------ *
 *  The store
 * ------------------------------------------------------------------------ */

/// The projects under one root.
pub struct ProjectRoot {
    projects: PathBuf,
    caps: Caps,
    busy: Duration,
}

/// One project in the list: its id, its title, run count and last saved
/// time if its database could be read (or the code it was refused with), and
/// its size on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEntry {
    pub id: ProjectId,
    pub title: Option<ProjectTitle>,
    pub refused: Option<ProjectCode>,
    pub bytes: u64,
    /// The newest run's saved time, or the project's creation time.
    pub saved_at: Option<i64>,
    pub run_count: Option<u64>,
}

/// What [`ProjectRoot::list`] reads from a project's database.
struct Summary {
    title: ProjectTitle,
    saved_at: i64,
    run_count: u64,
}

/// What recovery did when a project opened, as counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecoveryReport {
    pub temporaries_removed: u64,
    pub orphans_removed: u64,
    pub damaged_marked: u64,
    pub runs_interrupted: u64,
}

impl ProjectRoot {
    /// The store under `root`, with decision 107's caps.
    pub fn open_root(root: &Path) -> Result<ProjectRoot, ProjectRefusal> {
        ProjectRoot::open_root_with(root, Caps::REAL)
    }

    /// The store under `root`. Makes `<root>/projects/` (0700) if absent,
    /// refuses it if it is a symlink, and finishes what an interrupted
    /// delete or create left: every `.deleting-<id>` and `.creating-<id>`
    /// folder is removed.
    pub fn open_root_with(root: &Path, caps: Caps) -> Result<ProjectRoot, ProjectRefusal> {
        if !root.is_absolute() {
            return Err(ProjectCode::RootUnavailable.into());
        }
        let projects = root.join(PROJECTS_DIR);
        match fs::symlink_metadata(&projects) {
            Ok(m) if m.file_type().is_symlink() => return Err(ProjectCode::SymlinkRefused.into()),
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(ProjectCode::RootUnavailable.into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut b = fs::DirBuilder::new();
                b.recursive(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    b.mode(0o700);
                }
                b.create(root).map_err(io_refusal)?;
                make_private_dir(&projects).map_err(io_refusal)?;
            }
            Err(e) => return Err(io_refusal(e)),
        }
        // SQLite's `SQLITE_OPEN_NOFOLLOW` refuses a symlink in ANY element of
        // the path (`unixFullPathname`), and the folders above the root are
        // not the store's: macOS's temp folder is under `/var`, a symlink.
        // `projects/` itself was just checked not to be one, so resolving it
        // resolves only its ancestors, and the flag still refuses every
        // symlink under the root. Windows' VFS has no such check.
        #[cfg(unix)]
        let projects = projects.canonicalize().map_err(io_refusal)?;
        let store = ProjectRoot {
            projects,
            caps,
            busy: BUSY_TIMEOUT,
        };
        store.finish_leftovers()?;
        Ok(store)
    }

    /// The store's own leftovers at start: a tombstone is finished, a
    /// staging folder removed. Only folders named by the store's own
    /// prefixes and a parsable id are touched, and each only under its
    /// project's exclusive lock (decision 132): one another copy of the app
    /// holds, mid-delete or mid-create, is skipped and left to that copy. The
    /// lock file is removed once the leftover is gone and no project folder
    /// has the id.
    fn finish_leftovers(&self) -> Result<(), ProjectRefusal> {
        let mut tombstones = 0u64;
        let mut staged = 0u64;
        let mut skipped = 0u64;
        for entry in fs::read_dir(&self.projects).map_err(io_refusal)? {
            let entry = entry.map_err(io_refusal)?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !entry.file_type().map_err(io_refusal)?.is_dir() {
                continue;
            }
            let (rest, tombstone) = if let Some(rest) = name.strip_prefix(DELETING_PREFIX) {
                (rest, true)
            } else if let Some(rest) = name.strip_prefix(CREATING_PREFIX) {
                (rest, false)
            } else {
                continue;
            };
            let Ok(id) = ProjectId::parse(rest) else {
                continue;
            };
            let Ok(lock) = self.lock(&id, LockKind::Exclusive, ProjectCode::OpenElsewhere) else {
                skipped += 1;
                continue;
            };
            remove_tree(&entry.path()).map_err(io_refusal)?;
            if real_dir(&self.project_dir(&id)).is_err() {
                let _ = lock.remove_file();
            }
            if tombstone {
                tombstones += 1;
            } else {
                staged += 1;
            }
        }
        if tombstones + staged > 0 {
            sync_dir(&self.projects).map_err(io_refusal)?;
        }
        if tombstones + staged + skipped > 0 {
            tracing::info!(
                tombstones,
                staged,
                skipped,
                "project store finished leftovers"
            );
        }
        Ok(())
    }

    fn project_dir(&self, id: &ProjectId) -> PathBuf {
        self.projects.join(id.as_str())
    }

    fn staging_dir(&self, id: &ProjectId) -> PathBuf {
        self.projects
            .join(format!("{CREATING_PREFIX}{}", id.as_str()))
    }

    fn tombstone_dir(&self, id: &ProjectId) -> PathBuf {
        self.projects
            .join(format!("{DELETING_PREFIX}{}", id.as_str()))
    }

    /// The locks folder, made owner-only if absent; refused if it is a
    /// symlink or not a folder. Made only by a call that takes a lock, so
    /// listing and sizing never write.
    fn locks_dir(&self) -> Result<PathBuf, ProjectRefusal> {
        let d = self.projects.join(LOCKS_DIR);
        match fs::symlink_metadata(&d) {
            Ok(m) if m.file_type().is_symlink() => Err(ProjectCode::SymlinkRefused.into()),
            Ok(m) if m.is_dir() => Ok(d),
            Ok(_) => Err(ProjectCode::LockFailed.into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                match make_private_dir(&d) {
                    Ok(()) => {}
                    // Another copy made it first.
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                        real_dir(&d).map_err(|_| ProjectRefusal::from(ProjectCode::LockFailed))?
                    }
                    Err(_) => return Err(ProjectCode::LockFailed.into()),
                }
                Ok(d)
            }
            Err(_) => Err(ProjectCode::LockFailed.into()),
        }
    }

    /// `kind` on `id`'s lock file, made empty if absent (decision 132).
    fn lock(
        &self,
        id: &ProjectId,
        kind: LockKind,
        would_block: ProjectCode,
    ) -> Result<ProjectLock, ProjectRefusal> {
        let p = self
            .locks_dir()?
            .join(format!("{}{LOCK_SUFFIX}", id.as_str()));
        try_lock_file(&p, kind, would_block)
    }

    /// After a refusal because `id`'s folder is not a real folder: removes
    /// the lock file a lock may have just made, under the exclusive lock and
    /// only while the folder is still not there. Best effort: a lock file
    /// left behind is empty, and is not a project.
    fn discard_orphan_lock(&self, id: &ProjectId) {
        if let Ok(l) = self.lock(id, LockKind::Exclusive, ProjectCode::OpenElsewhere) {
            if real_dir(&self.project_dir(id)).is_err() {
                let _ = l.remove_file();
            }
        }
    }

    /// Creates a project titled `title` and opens it. It is made in
    /// `.creating-<id>/` and renamed into place only when its database holds
    /// its schema and its row, so a folder named by an id is never half-made.
    /// The commands make a project only through [`ProjectRoot::create_with_save`]
    /// (decision 121); this one serves the store's own tests.
    pub fn create(&self, title: &str) -> Result<ProjectHandle, ProjectRefusal> {
        let _w = write_lock();
        let title = ProjectTitle::parse(title)?;
        let all = projects_bytes(&self.projects).map_err(io_refusal)?;
        self.caps.admit(0, all, 0)?;
        let id = ProjectId::parse(&random_hex32()?)?;
        let staging = self.staging_dir(&id);
        make_private_dir(&staging).map_err(io_refusal)?;
        let made = self.fill_new_project(&staging, &id, &title);
        if let Err(r) = made {
            let _ = remove_tree(&staging);
            return Err(r);
        }
        fs::rename(&staging, self.project_dir(&id)).map_err(io_refusal)?;
        sync_dir(&self.projects).map_err(io_refusal)?;
        tracing::info!(project = %id.as_str(), "project created");
        self.open_parsed(&id, None)
    }

    /// Creates a project titled `title` by its first Save, in one write with
    /// it (decision 121): the project's database and row are made inside
    /// `.creating-<id>/`, the Save is written there under the caps, and the
    /// folder is renamed into place only after the Save's commit. A refused
    /// or failed Save removes the folder; a crash leaves it to the next
    /// start, which removes it. Returns the open project and the run's id.
    pub fn create_with_save(
        &self,
        title: &ProjectTitle,
        req: SaveRequest,
    ) -> Result<(ProjectHandle, StoredRunId), ProjectRefusal> {
        let _w = write_lock();
        let id = ProjectId::parse(&random_hex32()?)?;
        // The new project is opened by this Save (decision 122), so its lock
        // is taken first, shared, and held by the handle returned (decision
        // 132): another copy's recovery skips a staging folder it cannot lock.
        let lock = self.lock(&id, LockKind::Shared, ProjectCode::BeingDeleted)?;
        let staging = self.staging_dir(&id);
        let refused = |r: ProjectRefusal, lock: ProjectLock| {
            let _ = remove_tree(&staging);
            drop(lock);
            self.discard_orphan_lock(&id);
            r
        };
        if let Err(e) = make_private_dir(&staging) {
            return Err(refused(io_refusal(e), lock));
        }
        let run = match self.fill_and_save(&staging, &id, title, req) {
            Ok(run) => run,
            Err(r) => {
                tracing::warn!(project = %id.as_str(), code = r.code.wire(), "first save refused");
                return Err(refused(r, lock));
            }
        };
        #[cfg(test)]
        fault::hit(fault::Point::BeforeStagingRename);
        if let Err(e) = fs::rename(&staging, self.project_dir(&id)) {
            return Err(refused(io_refusal(e), lock));
        }
        sync_dir(&self.projects).map_err(io_refusal)?;
        tracing::info!(project = %id.as_str(), run = run.0, "project created by its first save");
        let handle = self.open_parsed(&id, Some(lock))?;
        Ok((handle, run))
    }

    /// The new project's database and row in `staging`, then the Save, in
    /// its own connection, closed before the rename.
    fn fill_and_save(
        &self,
        staging: &Path,
        id: &ProjectId,
        title: &ProjectTitle,
        req: SaveRequest,
    ) -> Result<StoredRunId, ProjectRefusal> {
        self.fill_new_project(staging, id, title)?;
        let conn = open_conn(&staging.join(DB_FILE), self.busy)?;
        configure(&conn)?;
        // Not marked open: nothing outside this call can reach it.
        let mut staged = ProjectHandle {
            id: id.clone(),
            dir: staging.to_path_buf(),
            projects: self.projects.clone(),
            conn,
            caps: self.caps,
            _lock: None,
        };
        staged.save_inner(req)
    }

    fn fill_new_project(
        &self,
        staging: &Path,
        id: &ProjectId,
        title: &ProjectTitle,
    ) -> Result<(), ProjectRefusal> {
        make_private_dir(&staging.join(ARTIFACTS_DIR)).map_err(io_refusal)?;
        let db_path = staging.join(DB_FILE);
        create_new_private_file(&db_path, 0o600)
            .and_then(|f| f.sync_all())
            .map_err(io_refusal)?;
        let mut conn = open_conn(&db_path, self.busy)?;
        migrate(&mut conn)?;
        conn.execute(
            "INSERT INTO project (singleton, project_id, title, workspace_home, created_at, app_version) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
            params![
                id.as_str(),
                title.as_str(),
                workspace_word(WorkspaceId::Statistics),
                gaply_core::now_epoch(),
                APP_VERSION
            ],
        )
        .map_err(db_refusal)?;
        conn.close().map_err(|(_, e)| db_refusal(e))?;
        sync_dir(&staging.join(ARTIFACTS_DIR)).map_err(io_refusal)?;
        sync_dir(staging).map_err(io_refusal)
    }

    /// Opens the project `raw_id` names, after refusing a bad id, a missing
    /// or symlinked folder, and a missing, unreadable or newer database (in
    /// which case nothing in the project is touched); then recovers it.
    pub fn open(&self, raw_id: &str) -> Result<ProjectHandle, ProjectRefusal> {
        let id = ProjectId::parse(raw_id)?;
        self.open_parsed(&id, None)
    }

    /// Opens `id` holding its lock shared: `held`, or one taken now. The
    /// lock comes first and the folder check after it (decision 132), so a
    /// copy that is deleting the project, which holds the lock exclusive
    /// until the folder is gone, refuses this open with
    /// `project.being_deleted`, and one that has finished leaves no folder.
    fn open_parsed(
        &self,
        id: &ProjectId,
        held: Option<ProjectLock>,
    ) -> Result<ProjectHandle, ProjectRefusal> {
        let lock = match held {
            Some(l) => l,
            None => self.lock(id, LockKind::Shared, ProjectCode::BeingDeleted)?,
        };
        let dir = self.project_dir(id);
        if let Err(r) = real_dir(&dir) {
            drop(lock);
            self.discard_orphan_lock(id);
            return Err(r);
        }
        let db_path = dir.join(DB_FILE);
        match file_state(&db_path)? {
            FileState::Missing => return Err(ProjectCode::DbMissing.into()),
            FileState::NotRegular => {
                let symlink = fs::symlink_metadata(&db_path)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                return Err(if symlink {
                    ProjectCode::SymlinkRefused
                } else {
                    ProjectCode::DbUnreadable
                }
                .into());
            }
            FileState::Regular { .. } => {}
        }
        let art = dir.join(ARTIFACTS_DIR);
        match real_dir(&art) {
            Ok(()) => {}
            Err(r) if r.code == ProjectCode::NotFound => {
                return Err(ProjectCode::DbUnreadable.into());
            }
            Err(r) => return Err(r),
        }
        let conn = open_conn(&db_path, self.busy)?;
        check_db(&conn, id)?;
        configure(&conn)?;
        mark_open(&dir);
        let mut handle = ProjectHandle {
            id: id.clone(),
            dir,
            projects: self.projects.clone(),
            conn,
            caps: self.caps,
            _lock: Some(lock),
        };
        let r = handle.recover()?;
        tracing::info!(
            project = %id.as_str(),
            temporaries = r.temporaries_removed,
            orphans = r.orphans_removed,
            damaged = r.damaged_marked,
            interrupted = r.runs_interrupted,
            "project opened"
        );
        Ok(handle)
    }

    /// The projects under the root, by id: a scan of the folder (decision
    /// 106). Each project's database is read for its row; no artifact is
    /// opened and nothing is hashed. Tombstones and staging folders are not
    /// listed. A symlinked project folder is listed as refused, unread.
    pub fn list(&self) -> Result<Vec<ProjectEntry>, ProjectRefusal> {
        let _w = write_lock();
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.projects).map_err(io_refusal)? {
            let entry = entry.map_err(io_refusal)?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Ok(id) = ProjectId::parse(&name) else {
                continue;
            };
            let ft = entry.file_type().map_err(io_refusal)?;
            if ft.is_symlink() {
                out.push(ProjectEntry {
                    id,
                    title: None,
                    refused: Some(ProjectCode::SymlinkRefused),
                    bytes: 0,
                    saved_at: None,
                    run_count: None,
                });
                continue;
            }
            if !ft.is_dir() {
                continue;
            }
            let dir = entry.path();
            let bytes = tree_bytes(&dir).map_err(io_refusal)?;
            out.push(match self.read_summary(&dir, &id) {
                Ok(s) => ProjectEntry {
                    id,
                    title: Some(s.title),
                    refused: None,
                    bytes,
                    saved_at: Some(s.saved_at),
                    run_count: Some(s.run_count),
                },
                Err(r) => ProjectEntry {
                    id,
                    title: None,
                    refused: Some(r.code),
                    bytes,
                    saved_at: None,
                    run_count: None,
                },
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn read_summary(&self, dir: &Path, id: &ProjectId) -> Result<Summary, ProjectRefusal> {
        let db_path = dir.join(DB_FILE);
        match file_state(&db_path)? {
            FileState::Missing => return Err(ProjectCode::DbMissing.into()),
            FileState::NotRegular => return Err(ProjectCode::SymlinkRefused.into()),
            FileState::Regular { .. } => {}
        }
        let conn = open_conn(&db_path, self.busy)?;
        let record = check_db(&conn, id)?;
        let (count, newest): (i64, Option<i64>) = conn
            .query_row("SELECT COUNT(*), MAX(saved_at) FROM runs", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| ProjectRefusal::from(ProjectCode::DbUnreadable))?;
        Ok(Summary {
            title: record.title,
            saved_at: newest.unwrap_or(record.created_at),
            run_count: count as u64,
        })
    }

    /// Every byte under the projects folder (tombstones included) and how
    /// many project folders it holds.
    pub fn usage(&self) -> Result<(u64, u64), ProjectRefusal> {
        let _w = write_lock();
        let total = projects_bytes(&self.projects).map_err(io_refusal)?;
        let mut count = 0u64;
        for entry in fs::read_dir(&self.projects).map_err(io_refusal)? {
            let entry = entry.map_err(io_refusal)?;
            let named = entry
                .file_name()
                .to_str()
                .is_some_and(|n| ProjectId::parse(n).is_ok());
            if named && !entry.file_type().map_err(io_refusal)?.is_file() {
                count += 1;
            }
        }
        Ok((total, count))
    }

    /// One project folder's size on disk: refused for a missing or
    /// symlinked folder, read from metadata only, so a project whose
    /// database cannot be read still has a size.
    pub fn project_bytes(&self, id: &ProjectId) -> Result<u64, ProjectRefusal> {
        let dir = self.project_dir(id);
        real_dir(&dir)?;
        tree_bytes(&dir).map_err(io_refusal)
    }

    /// Deletes the project `raw_id` names (decision 104): refused while this
    /// process holds it open (`project.still_open`), and while another copy
    /// of the app does (`project.open_elsewhere`: its lock cannot be taken
    /// exclusive, decision 132). The lock is taken first and held until the
    /// folder is gone; the folder is renamed to `.deleting-<id>`, then
    /// removed, and only then is the lock file removed. An interrupted
    /// delete is finished by the next [`ProjectRoot::open_root`]. A symlinked
    /// folder is refused and its target untouched.
    pub fn delete(&self, raw_id: &str) -> Result<(), ProjectRefusal> {
        let _w = write_lock();
        let id = ProjectId::parse(raw_id)?;
        let dir = self.project_dir(&id);
        if is_open(&dir) {
            return Err(ProjectCode::StillOpen.into());
        }
        let lock = self.lock(&id, LockKind::Exclusive, ProjectCode::OpenElsewhere)?;
        if let Err(r) = real_dir(&dir) {
            let _ = lock.remove_file();
            return Err(r);
        }
        let tomb = self.tombstone_dir(&id);
        fs::rename(&dir, &tomb).map_err(io_refusal)?;
        sync_dir(&self.projects).map_err(io_refusal)?;
        #[cfg(test)]
        fault::hit(fault::Point::AfterTombstone);
        remove_tree(&tomb).map_err(io_refusal)?;
        sync_dir(&self.projects).map_err(io_refusal)?;
        if lock.remove_file().is_err() {
            tracing::warn!(project = %id.as_str(), "project deleted; its empty lock file was left");
        }
        drop(lock);
        tracing::info!(project = %id.as_str(), "project deleted");
        Ok(())
    }
}

/* ------------------------------------------------------------------------ *
 *  An open project
 * ------------------------------------------------------------------------ */

/// One artifact of a Save: its bytes and the hash the caller already holds
/// (the ingest's content hash, or the canonical bytes' hash). Its `Debug`
/// prints the size only.
pub struct ArtifactInput {
    pub kind: ArtifactKind,
    pub format: DataFormat,
    pub schema_version: u32,
    pub expected: ArtifactHash,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for ArtifactInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ArtifactInput({:?}, {} bytes)",
            self.kind,
            self.bytes.len()
        )
    }
}

/// The run a Save records: finished, with its links by hash.
#[derive(Debug, Clone)]
pub struct RunToSave {
    pub capability_id: CapabilityId,
    pub capability_version: u32,
    pub state: RunState,
    pub started_at: i64,
    pub ended_at: i64,
    pub used: Vec<(ArtifactHash, UsedRole)>,
    pub generated: Vec<ArtifactHash>,
    /// `(artifact, from)`: the artifact was derived from `from` by this run.
    pub derived: Vec<(ArtifactHash, ArtifactHash)>,
}

/// One Save: a run and the artifacts it brings.
#[derive(Debug)]
pub struct SaveRequest {
    pub run: RunToSave,
    pub artifacts: Vec<ArtifactInput>,
}

/// An open project. Dropping it closes its connection, then releases its
/// lock (fields drop in order, so the connection is closed first).
pub struct ProjectHandle {
    id: ProjectId,
    dir: PathBuf,
    projects: PathBuf,
    conn: Connection,
    caps: Caps,
    /// The project's lock, shared (decision 132); `None` only for a first
    /// Save's staging handle, whose lock `create_with_save` holds.
    _lock: Option<ProjectLock>,
}

impl Drop for ProjectHandle {
    fn drop(&mut self) {
        checkpoint(&self.conn);
        mark_closed(&self.dir);
    }
}

impl ProjectHandle {
    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    /// Closes the project. Its `Drop` checkpoints the log first (decision
    /// 173), then the connection closes and the lock is released.
    pub fn close(self) {}

    /// Every byte in the project's folder, from metadata.
    pub fn size_on_disk(&self) -> Result<u64, ProjectRefusal> {
        tree_bytes(&self.dir).map_err(io_refusal)
    }

    fn artifacts_dir(&self) -> PathBuf {
        self.dir.join(ARTIFACTS_DIR)
    }

    /// Recovery (ADR-009 §6), in one `BEGIN IMMEDIATE`: leftover temporary
    /// files and artifact files with no row are removed; a row whose file is
    /// missing, not a regular file or of the wrong size is marked damaged;
    /// a run left unfinished becomes `Failed`, interrupted. It adds no user
    /// data.
    fn recover(&mut self) -> Result<RecoveryReport, ProjectRefusal> {
        let now = gaply_core::now_epoch();
        let art = self.artifacts_dir();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_refusal)?;
        let mut report = RecoveryReport::default();

        let rows: Vec<(String, i64, Option<String>)> = {
            let mut st = tx
                .prepare("SELECT hash, bytes, damage FROM artifacts")
                .map_err(db_refusal)?;
            let it = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(db_refusal)?;
            it.collect::<Result<_, _>>().map_err(db_refusal)?
        };
        let known: BTreeSet<&str> = rows.iter().map(|(h, _, _)| h.as_str()).collect();

        let mut removed_any = false;
        for entry in fs::read_dir(&art).map_err(io_refusal)? {
            let entry = entry.map_err(io_refusal)?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !entry.file_type().map_err(io_refusal)?.is_file() {
                continue;
            }
            if let Some(rest) = name.strip_prefix(TMP_PREFIX) {
                if ProjectId::parse(rest).is_ok() {
                    remove_file_any(&entry.path()).map_err(io_refusal)?;
                    report.temporaries_removed += 1;
                    removed_any = true;
                }
            } else if let Ok(h) = ArtifactHash::parse(&name) {
                if !known.contains(h.as_hex()) {
                    remove_file_any(&art.join(h.as_hex())).map_err(io_refusal)?;
                    report.orphans_removed += 1;
                    removed_any = true;
                }
            }
        }

        for (hash, bytes, damage) in &rows {
            if damage.is_some() {
                continue;
            }
            let h = ArtifactHash::parse(hash)
                .map_err(|_| ProjectRefusal::from(ProjectCode::DbUnreadable))?;
            let state = file_state(&art.join(h.as_hex()))?;
            if let Some(code) = damage_of(*bytes as u64, state) {
                tx.execute(
                    "UPDATE artifacts SET damage = ?1 WHERE hash = ?2",
                    params![code.wire(), h.as_hex()],
                )
                .map_err(db_refusal)?;
                report.damaged_marked += 1;
                tracing::warn!(project = %self.id.as_str(), artifact = %h.prefix(), code = code.wire(), "artifact marked damaged");
            }
        }

        let runs: Vec<(i64, String)> = {
            let mut st = tx
                .prepare("SELECT run_id, state FROM runs")
                .map_err(db_refusal)?;
            let it = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(db_refusal)?;
            it.collect::<Result<_, _>>().map_err(db_refusal)?
        };
        for (run_id, state) in runs {
            let state = RunState::from_wire(&state).ok_or(ProjectCode::DbUnreadable)?;
            if let Some((to, code)) = recovered_state(state) {
                tx.execute(
                    "UPDATE runs SET state = ?1, end_code = ?2, ended_at = COALESCE(ended_at, ?3) WHERE run_id = ?4",
                    params![to.wire(), code.wire(), now, run_id],
                )
                .map_err(db_refusal)?;
                report.runs_interrupted += 1;
            }
        }
        tx.commit().map_err(db_refusal)?;
        if removed_any {
            sync_dir(&art).map_err(io_refusal)?;
        }
        Ok(report)
    }

    /// Saves one finished run and its artifacts (ADR-009 §5), all or
    /// nothing. Returns the run's id in this project.
    pub fn save(&mut self, req: SaveRequest) -> Result<StoredRunId, ProjectRefusal> {
        let _w = write_lock();
        let result = self.save_inner(req);
        if let Err(r) = &result {
            tracing::warn!(project = %self.id.as_str(), code = r.code.wire(), "save refused");
        }
        result
    }

    fn save_inner(&mut self, req: SaveRequest) -> Result<StoredRunId, ProjectRefusal> {
        let SaveRequest { run, artifacts } = req;
        if !run.state.is_terminal() {
            return Err(ProjectCode::RunNotFinished.into());
        }
        let mut inputs: Vec<ArtifactInput> = Vec::new();
        for a in artifacts {
            if !a.kind.admits(a.format) {
                return Err(ProjectCode::BadFormat.into());
            }
            if sha256_hash(&a.bytes)? != a.expected {
                return Err(ProjectCode::HashMismatch.into());
            }
            if !inputs.iter().any(|b| b.expected == a.expected) {
                inputs.push(a);
            }
        }
        let agent = AgentVersion::parse(APP_VERSION)?;
        let now = gaply_core::now_epoch();
        let art = self.artifacts_dir();
        let project_dir = self.dir.clone();
        let projects = self.projects.clone();
        let caps = self.caps;
        let id = self.id.clone();

        // 1. The write lock, held to the end.
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_refusal)?;

        let existing: BTreeSet<String> = {
            let mut st = tx
                .prepare("SELECT hash FROM artifacts")
                .map_err(db_refusal)?;
            let it = st.query_map([], |r| r.get(0)).map_err(db_refusal)?;
            it.collect::<Result<_, _>>().map_err(db_refusal)?
        };
        let known = |h: &ArtifactHash| {
            existing.contains(h.as_hex()) || inputs.iter().any(|a| &a.expected == h)
        };
        let named = run
            .used
            .iter()
            .map(|(h, _)| h)
            .chain(run.generated.iter())
            .chain(run.derived.iter().flat_map(|(a, b)| [a, b]));
        for h in named {
            if !known(h) {
                return Err(ProjectCode::UnknownArtifact.into());
            }
        }

        let next: i64 = tx
            .query_row("SELECT COALESCE(MAX(run_id), 0) + 1 FROM runs", [], |r| {
                r.get(0)
            })
            .map_err(db_refusal)?;
        let run_id = StoredRunId(next as u64);
        let mut graph = load_graph(&tx)?;
        let mut new_edges: Vec<ProvEdge> = Vec::new();
        for (h, role) in &run.used {
            new_edges.push(ProvEdge::Used {
                run: run_id,
                artifact: h.clone(),
                role: *role,
            });
        }
        for h in &run.generated {
            new_edges.push(ProvEdge::WasGeneratedBy {
                artifact: h.clone(),
                run: run_id,
            });
        }
        new_edges.push(ProvEdge::WasAssociatedWith {
            run: run_id,
            agent: agent.clone(),
        });
        for (a, from) in &run.derived {
            new_edges.push(ProvEdge::WasDerivedFrom {
                artifact: a.clone(),
                from: from.clone(),
                via: run_id,
            });
        }
        for e in &new_edges {
            graph.add(e.clone())?;
        }

        // 2. The caps, before anything is written.
        let mut incoming = 0u64;
        for a in &inputs {
            if file_state(&art.join(a.expected.as_hex()))? == FileState::Missing {
                incoming = incoming.saturating_add(a.bytes.len() as u64);
            }
        }
        let project_now = tree_bytes(&project_dir).map_err(io_refusal)?;
        let all_now = projects_bytes(&projects).map_err(io_refusal)?;
        caps.admit(project_now, all_now, incoming)?;
        #[cfg(test)]
        fault::hit(fault::Point::AfterCaps);

        // 3. The files: each verified and renamed into place, or reused.
        let mut created: Vec<PathBuf> = Vec::new();
        let mut written: Vec<&ArtifactHash> = Vec::new();
        let mut reused = 0u64;
        let cleanup = |created: &[PathBuf]| {
            for p in created {
                let _ = remove_file_any(p);
            }
            let _ = sync_dir(&art);
        };
        for a in &inputs {
            let final_path = art.join(a.expected.as_hex());
            let outcome = match file_state(&final_path) {
                Ok(FileState::Regular { .. }) => match hash_file(&final_path) {
                    Ok(h) if h == a.expected => {
                        reused += 1;
                        Ok(())
                    }
                    Ok(_) => Err(ProjectCode::ArtifactWrongHash.into()),
                    Err(r) => Err(r),
                },
                Ok(FileState::NotRegular) => Err(ProjectCode::ArtifactNotRegular.into()),
                Ok(FileState::Missing) => write_artifact(&art, a, &final_path).map(|()| {
                    created.push(final_path.clone());
                    written.push(&a.expected);
                }),
                Err(r) => Err(r),
            };
            if let Err(r) = outcome {
                cleanup(&created);
                return Err(r);
            }
        }

        // 4. The rows, then the commit.
        let rows = (|| -> rusqlite::Result<()> {
            for a in &inputs {
                tx.execute(
                    "INSERT OR IGNORE INTO artifacts (hash, kind, format, bytes, schema_version, created_at, damage) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                    params![
                        a.expected.as_hex(),
                        a.kind.wire(),
                        a.format.wire(),
                        a.bytes.len() as i64,
                        a.schema_version,
                        now
                    ],
                )?;
                // A row marked damaged whose bytes were just written and
                // verified again is whole once more.
                tx.execute(
                    "UPDATE artifacts SET damage = NULL WHERE hash = ?1 AND damage IS NOT NULL",
                    params![a.expected.as_hex()],
                )?;
            }
            tx.execute(
                "INSERT INTO runs (run_id, capability_id, capability_version, state, end_code, agent_version, started_at, ended_at, saved_at) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8)",
                params![
                    next,
                    run.capability_id.as_str(),
                    run.capability_version,
                    run.state.wire(),
                    agent.as_str(),
                    run.started_at,
                    run.ended_at,
                    now
                ],
            )?;
            for e in &new_edges {
                match e {
                    ProvEdge::Used { artifact, role, .. } => tx.execute(
                        "INSERT OR IGNORE INTO used (run_id, hash, role) VALUES (?1, ?2, ?3)",
                        params![next, artifact.as_hex(), role.wire()],
                    )?,
                    ProvEdge::WasGeneratedBy { artifact, .. } => tx.execute(
                        "INSERT OR IGNORE INTO generated (hash, run_id) VALUES (?1, ?2)",
                        params![artifact.as_hex(), next],
                    )?,
                    ProvEdge::WasDerivedFrom { artifact, from, .. } => tx.execute(
                        "INSERT OR IGNORE INTO derived (hash, from_hash, via_run_id) VALUES (?1, ?2, ?3)",
                        params![artifact.as_hex(), from.as_hex(), next],
                    )?,
                    ProvEdge::WasAssociatedWith { .. } => 0,
                };
            }
            Ok(())
        })();
        #[cfg(test)]
        fault::hit(fault::Point::BeforeCommit);
        if let Err(e) = rows.and_then(|()| tx.commit()) {
            cleanup(&created);
            return Err(db_refusal(e));
        }
        // Committed: the Save stands whatever the checkpoint does.
        checkpoint(&self.conn);
        for h in &written {
            tracing::debug!(project = %id.as_str(), artifact = %h.prefix(), "artifact written");
        }
        tracing::info!(
            project = %id.as_str(),
            run = run_id.0,
            written = created.len(),
            reused,
            "run saved"
        );
        Ok(run_id)
    }

    /// Reads an artifact's bytes, hashing them on the way: a mismatch, a
    /// missing file or a wrong size marks the row damaged and refuses the
    /// read with that code; a row already marked is refused with its code.
    pub fn read_artifact(&mut self, raw_hash: &str) -> Result<Vec<u8>, ProjectRefusal> {
        let hash = ArtifactHash::parse(raw_hash)?;
        let row: Option<(i64, Option<String>)> = self
            .conn
            .query_row(
                "SELECT bytes, damage FROM artifacts WHERE hash = ?1",
                params![hash.as_hex()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_refusal)?;
        let Some((bytes, damage)) = row else {
            return Err(ProjectCode::UnknownArtifact.into());
        };
        if let Some(d) = damage {
            return Err(ProjectCode::from_wire(&d)
                .unwrap_or(ProjectCode::DbUnreadable)
                .into());
        }
        let p = self.artifacts_dir().join(hash.as_hex());
        let damage = match damage_of(bytes as u64, file_state(&p)?) {
            Some(code) => Err(code),
            None => {
                let data = read_bounded(&p, bytes as u64)?;
                if data.len() as u64 != bytes as u64 {
                    Err(ProjectCode::ArtifactWrongSize)
                } else if sha256_hash(&data)? != hash {
                    Err(ProjectCode::ArtifactWrongHash)
                } else {
                    Ok(data)
                }
            }
        };
        match damage {
            Ok(data) => Ok(data),
            Err(code) => {
                self.mark_damaged(&hash, code)?;
                Err(code.into())
            }
        }
    }

    fn mark_damaged(
        &mut self,
        hash: &ArtifactHash,
        code: ProjectCode,
    ) -> Result<(), ProjectRefusal> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_refusal)?;
        tx.execute(
            "UPDATE artifacts SET damage = ?1 WHERE hash = ?2",
            params![code.wire(), hash.as_hex()],
        )
        .map_err(db_refusal)?;
        tx.commit().map_err(db_refusal)?;
        tracing::warn!(project = %self.id.as_str(), artifact = %hash.prefix(), code = code.wire(), "artifact marked damaged");
        Ok(())
    }

    /// The project's own row.
    pub fn record(&self) -> Result<ProjectRecord, ProjectRefusal> {
        read_record(&self.conn)
    }

    /// Every artifact row, by hash.
    pub fn artifacts(&self) -> Result<Vec<ArtifactRef>, ProjectRefusal> {
        let bad = || ProjectRefusal::from(ProjectCode::DbUnreadable);
        let mut st = self
            .conn
            .prepare("SELECT hash, bytes, kind, format, schema_version, damage FROM artifacts ORDER BY hash")
            .map_err(db_refusal)?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(db_refusal)?;
        let mut out = Vec::new();
        for row in rows {
            let (hash, bytes, kind, format, schema, damage) = row.map_err(db_refusal)?;
            out.push(ArtifactRef {
                hash: ArtifactHash::parse(&hash).map_err(|_| bad())?,
                bytes: bytes as u64,
                kind: ArtifactKind::from_wire(&kind).ok_or_else(bad)?,
                format: DataFormat::from_wire(&format).ok_or_else(bad)?,
                schema_version: schema as u32,
                damage: match damage {
                    Some(d) => Some(ProjectCode::from_wire(&d).ok_or_else(bad)?),
                    None => None,
                },
            });
        }
        Ok(out)
    }

    /// Every run row, by id.
    pub fn runs(&self) -> Result<Vec<StoredRun>, ProjectRefusal> {
        let bad = || ProjectRefusal::from(ProjectCode::DbUnreadable);
        let mut st = self
            .conn
            .prepare("SELECT run_id, capability_id, capability_version, state, end_code, agent_version, started_at, ended_at, saved_at FROM runs ORDER BY run_id")
            .map_err(db_refusal)?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, i64>(8)?,
                ))
            })
            .map_err(db_refusal)?;
        let mut out = Vec::new();
        for row in rows {
            let (run_id, cap, version, state, end, agent, started, ended, saved) =
                row.map_err(db_refusal)?;
            out.push(StoredRun {
                run_id: StoredRunId(run_id as u64),
                capability_id: CapabilityId::parse(&cap).ok_or_else(bad)?,
                capability_version: version as u32,
                state: RunState::from_wire(&state).ok_or_else(bad)?,
                end_code: match end {
                    Some(c) => Some(ProjectCode::from_wire(&c).ok_or_else(bad)?),
                    None => None,
                },
                agent: AgentVersion::parse(&agent).map_err(|_| bad())?,
                started_at: started,
                ended_at: ended,
                saved_at: saved,
            });
        }
        Ok(out)
    }

    /// The project's provenance graph.
    pub fn provenance(&self) -> Result<ProvGraph, ProjectRefusal> {
        load_graph(&self.conn)
    }
}

/// One artifact into place (ADR-009 §5, step 3): a temporary file created
/// new, written, flushed and synced, set to 0400, re-read and verified,
/// renamed, and the folder synced. Any failure removes the temporary file.
fn write_artifact(art: &Path, a: &ArtifactInput, final_path: &Path) -> Result<(), ProjectRefusal> {
    let tmp = art.join(format!("{TMP_PREFIX}{}", random_hex32()?));
    let mut f = create_new_private_file(&tmp, 0o600).map_err(io_refusal)?;
    let done = (|| -> Result<(), ProjectRefusal> {
        for chunk in a.bytes.chunks(CHUNK) {
            #[cfg(test)]
            fault::before_write(chunk.len() as u64).map_err(io_refusal)?;
            f.write_all(chunk).map_err(io_refusal)?;
        }
        f.flush().map_err(io_refusal)?;
        f.sync_all().map_err(io_refusal)?;
        #[cfg(test)]
        fault::corrupt(&tmp);
        set_read_only(&f).map_err(io_refusal)?;
        if hash_file(&tmp)? != a.expected {
            return Err(ProjectCode::HashMismatch.into());
        }
        #[cfg(test)]
        fault::hit(fault::Point::BeforeRename);
        fs::rename(&tmp, final_path).map_err(io_refusal)?;
        sync_dir(art).map_err(io_refusal)
    })();
    if done.is_err() {
        let _ = remove_file_any(&tmp);
    }
    done
}

/// Fault points for the store's own tests (blueprint §18.1). Compiled into
/// test builds only, so no release build contains them. A `Crash` panics
/// at its point: nothing after it runs, no clean-up included, and the open
/// transaction rolls back as SQLite rolls back after a real crash.
#[cfg(test)]
pub(crate) mod fault {
    use std::cell::RefCell;
    use std::path::Path;
    use std::sync::mpsc::{Receiver, Sender};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Point {
        BeforeRename,
        BeforeCommit,
        AfterTombstone,
        AfterCaps,
        BeforeStagingRename,
    }

    pub enum Fault {
        Crash(Point),
        Pause(Point, Sender<()>, Receiver<()>),
        DiskFullAfter(u64),
        CorruptTempAfterSync,
    }

    thread_local! {
        static FAULT: RefCell<Option<Fault>> = const { RefCell::new(None) };
    }

    pub fn set(f: Fault) {
        FAULT.with(|c| *c.borrow_mut() = Some(f));
    }

    pub fn hit(at: Point) {
        let fired = FAULT.with(|c| {
            let mut slot = c.borrow_mut();
            let due = matches!(slot.as_ref(), Some(Fault::Crash(p)) | Some(Fault::Pause(p, _, _)) if *p == at);
            if due {
                slot.take()
            } else {
                None
            }
        });
        match fired {
            Some(Fault::Crash(_)) => panic!("simulated crash at {at:?}"),
            Some(Fault::Pause(_, reached, resume)) => {
                reached.send(()).expect("the test is waiting");
                resume.recv().expect("the test resumes");
            }
            _ => {}
        }
    }

    /// Before each write of `len` bytes: with `DiskFullAfter(n)`, the write
    /// that would pass byte `n` fails as a full disk does.
    pub fn before_write(len: u64) -> std::io::Result<()> {
        let full = FAULT.with(|c| {
            let mut slot = c.borrow_mut();
            match slot.as_mut() {
                Some(Fault::DiskFullAfter(left)) if *left < len => {
                    *slot = None;
                    true
                }
                Some(Fault::DiskFullAfter(left)) => {
                    *left -= len;
                    false
                }
                _ => false,
            }
        });
        if full {
            #[cfg(unix)]
            return Err(std::io::Error::from_raw_os_error(libc::ENOSPC));
            #[cfg(windows)]
            return Err(std::io::Error::from_raw_os_error(112));
        }
        Ok(())
    }

    /// Flips the first byte of the temporary file after it was synced, as a
    /// bad write that read back differently would.
    pub fn corrupt(tmp: &Path) {
        let fire = FAULT.with(|c| matches!(c.borrow().as_ref(), Some(Fault::CorruptTempAfterSync)));
        if fire {
            FAULT.with(|c| c.borrow_mut().take());
            let mut b = std::fs::read(tmp).expect("read the temporary file");
            b[0] ^= 0xff;
            std::fs::write(tmp, b).expect("rewrite the temporary file");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::{mpsc, Arc};

    /// A folder under the OS temp folder that the test made and removes.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> TempRoot {
            let p =
                std::env::temp_dir().join(format!("gaply-5b1-store-{}", random_hex32().unwrap()));
            fs::create_dir(&p).unwrap();
            TempRoot(p)
        }
        fn root(&self) -> PathBuf {
            self.0.join("root")
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            if self.0.exists() {
                remove_tree(&self.0).expect("remove the test folder");
            }
        }
    }

    fn hash_of(bytes: &[u8]) -> ArtifactHash {
        sha256_hash(bytes).unwrap()
    }

    fn art(kind: ArtifactKind, format: DataFormat, bytes: &[u8]) -> ArtifactInput {
        ArtifactInput {
            kind,
            format,
            schema_version: 1,
            expected: hash_of(bytes),
            bytes: bytes.to_vec(),
        }
    }

    /// A finished describe run: it used a data file and a spec and generated
    /// a bundle derived from the data.
    fn request(data: &[u8], spec: &[u8], bundle: &[u8]) -> SaveRequest {
        let (d, s, b) = (hash_of(data), hash_of(spec), hash_of(bundle));
        SaveRequest {
            run: RunToSave {
                capability_id: CapabilityId::parse("stats.describe").unwrap(),
                capability_version: 1,
                state: RunState::Succeeded,
                started_at: 1_000,
                ended_at: 1_001,
                used: vec![(d.clone(), UsedRole::Data), (s, UsedRole::Spec)],
                generated: vec![b.clone()],
                derived: vec![(b, d)],
            },
            artifacts: vec![
                art(ArtifactKind::DataFile, DataFormat::Csv, data),
                art(ArtifactKind::OperationSpec, DataFormat::Json, spec),
                art(ArtifactKind::ResultBundle, DataFormat::Json, bundle),
            ],
        }
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .map(|it| {
                it.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    fn temporaries(dir: &Path) -> usize {
        files_in(dir)
            .iter()
            .filter(|n| n.starts_with(TMP_PREFIX))
            .count()
    }

    fn artifact_count(h: &ProjectHandle) -> i64 {
        h.conn
            .query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0))
            .unwrap()
    }

    fn run_count(h: &ProjectHandle) -> i64 {
        h.conn
            .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))
            .unwrap()
    }

    fn sha_file(p: &Path) -> String {
        hex_lower(&Sha256::digest(fs::read(p).unwrap()))
    }

    #[test]
    fn a_save_writes_its_artifacts_rows_and_edges_and_reads_back() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("Chapter 3").unwrap();
        let req = request(b"x,y\n1,2\n", b"{\"spec\":1}", b"{\"bundle\":1}");
        let run = h.save(req).unwrap();
        assert_eq!(run, StoredRunId(1));
        let arts = h.artifacts().unwrap();
        assert_eq!(arts.len(), 3);
        assert!(arts.iter().all(|a| a.damage.is_none()));
        let art_dir = h.artifacts_dir();
        assert_eq!(files_in(&art_dir).len(), 3, "three files, no temporary");
        for a in &arts {
            assert_eq!(
                files_in(&art_dir)
                    .iter()
                    .filter(|n| *n == a.hash.as_hex())
                    .count(),
                1
            );
        }
        assert_eq!(
            h.read_artifact(hash_of(b"x,y\n1,2\n").as_hex()).unwrap(),
            b"x,y\n1,2\n"
        );
        let runs = h.runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].state, RunState::Succeeded);
        assert_eq!(runs[0].agent.as_str(), APP_VERSION);
        let g = h.provenance().unwrap();
        assert!(g.is_acyclic());
        assert_eq!(
            g.edges().len(),
            5,
            "used ×2, generated, associated, derived"
        );
        assert!(g
            .edges()
            .iter()
            .any(|e| matches!(e, ProvEdge::WasAssociatedWith { .. })));
        assert_eq!(h.record().unwrap().title.as_str(), "Chapter 3");
        assert_eq!(h.record().unwrap().workspace_home, WorkspaceId::Statistics);
    }

    #[test]
    fn schema_version_1_migrates_from_an_empty_database() {
        let t = TempRoot::new();
        // The resolved path: `NOFOLLOW` refuses the temp folder's `/var`.
        let p = t.0.canonicalize().unwrap().join("empty.db");
        create_new_private_file(&p, 0o600).unwrap();
        let mut conn = open_conn(&p, BUSY_TIMEOUT).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 0);
        migrate(&mut conn).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 1);
        let tables: Vec<String> = {
            let mut st = conn
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .unwrap();
            let names = st
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            names
        };
        assert_eq!(
            tables,
            [
                "artifacts",
                "derived",
                "generated",
                "project",
                "runs",
                "used"
            ]
        );
        migrate(&mut conn).expect("a database at version 1 is left as it is");
        assert_eq!(user_version(&conn).unwrap(), 1);
    }

    #[test]
    fn the_journal_is_wal_and_every_commit_is_synced() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let h = store.create("p").unwrap();
        let mode: String = h
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let sync: i64 = h
            .conn
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sync, 2, "FULL");
        let fk: i64 = h
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn an_interrupted_write_leaves_no_artifact_and_no_row() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        let id = h.id().as_str().to_string();
        let art_dir = h.artifacts_dir();
        fault::set(fault::Fault::Crash(fault::Point::BeforeRename));
        let crashed = catch_unwind(AssertUnwindSafe(|| h.save(request(b"d", b"s", b"b"))));
        assert!(crashed.is_err(), "the crash fired");
        drop(h);
        assert_eq!(
            temporaries(&art_dir),
            1,
            "the crash left its temporary file"
        );
        assert!(
            !files_in(&art_dir)
                .iter()
                .any(|n| ArtifactHash::parse(n).is_ok()),
            "no artifact"
        );
        let h = store.open(&id).unwrap();
        assert_eq!(artifact_count(&h), 0, "no artifact row");
        assert_eq!(run_count(&h), 0, "no run row");
        assert_eq!(
            temporaries(&art_dir),
            0,
            "recovery removed the temporary file"
        );
        assert!(files_in(&art_dir).is_empty());
    }

    #[test]
    fn a_crash_between_rename_and_commit_leaves_orphans_that_recovery_removes() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        let id = h.id().as_str().to_string();
        let art_dir = h.artifacts_dir();
        fault::set(fault::Fault::Crash(fault::Point::BeforeCommit));
        let crashed = catch_unwind(AssertUnwindSafe(|| h.save(request(b"d", b"s", b"b"))));
        assert!(crashed.is_err());
        drop(h);
        assert_eq!(
            files_in(&art_dir).len(),
            3,
            "three renamed files, no rows yet"
        );
        let h = store.open(&id).unwrap();
        assert_eq!(artifact_count(&h), 0);
        assert_eq!(run_count(&h), 0);
        assert!(
            files_in(&art_dir).is_empty(),
            "recovery removed the files with no row"
        );
    }

    #[test]
    fn bytes_that_read_back_differently_are_refused_and_nothing_is_left() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        fault::set(fault::Fault::CorruptTempAfterSync);
        let r = h.save(request(b"d", b"s", b"b")).unwrap_err();
        assert_eq!(r.code, ProjectCode::HashMismatch);
        assert!(
            files_in(&h.artifacts_dir()).is_empty(),
            "no artifact and no temporary file"
        );
        assert_eq!(artifact_count(&h), 0);
        assert_eq!(run_count(&h), 0);
    }

    #[test]
    fn a_file_whose_hash_does_not_match_its_row_is_refused_on_read() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"data-bytes", b"s", b"b")).unwrap();
        let hash = hash_of(b"data-bytes");
        let p = h.artifacts_dir().join(hash.as_hex());
        set_mode(&p, 0o600).unwrap();
        fs::write(&p, b"DATA-BYTES").unwrap();
        let r = h.read_artifact(hash.as_hex()).unwrap_err();
        assert_eq!(r.code, ProjectCode::ArtifactWrongHash);
        let row = h
            .artifacts()
            .unwrap()
            .into_iter()
            .find(|a| a.hash == hash)
            .unwrap();
        assert_eq!(
            row.damage,
            Some(ProjectCode::ArtifactWrongHash),
            "marked damaged"
        );
        assert_eq!(
            h.read_artifact(hash.as_hex()).unwrap_err().code,
            ProjectCode::ArtifactWrongHash
        );
        assert_eq!(
            h.read_artifact(&hash.as_hex().to_uppercase())
                .unwrap_err()
                .code,
            ProjectCode::BadHash
        );
    }

    #[test]
    fn a_full_disk_at_the_write_is_a_closed_refusal_and_the_project_still_opens() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        let id = h.id().as_str().to_string();
        let big = vec![7u8; CHUNK * 2 + 10];
        fault::set(fault::Fault::DiskFullAfter(CHUNK as u64));
        let r = h.save(request(&big, b"s", b"b")).unwrap_err();
        assert_eq!(r.code, ProjectCode::DiskFull);
        assert!(
            files_in(&h.artifacts_dir()).is_empty(),
            "no temporary and no artifact"
        );
        assert_eq!(artifact_count(&h), 0);
        assert_eq!(run_count(&h), 0);
        drop(h);
        let mut h = store.open(&id).expect("the project still opens");
        h.save(request(&big, b"s", b"b"))
            .expect("and saves once there is room");
    }

    #[test]
    fn sqlite_full_at_the_insert_is_a_closed_refusal_with_nothing_left() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        let id = h.id().as_str().to_string();
        let pages: i64 = h
            .conn
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap();
        let set: i64 = h
            .conn
            .query_row(&format!("PRAGMA max_page_count = {pages}"), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(set, pages);
        let mut refused = None;
        for i in 0..400u32 {
            let data = format!("data-{i}");
            let before = (
                artifact_count(&h),
                run_count(&h),
                files_in(&h.artifacts_dir()),
            );
            match h.save(request(
                data.as_bytes(),
                format!("s{i}").as_bytes(),
                format!("b{i}").as_bytes(),
            )) {
                Ok(_) => continue,
                Err(r) => {
                    let after = (
                        artifact_count(&h),
                        run_count(&h),
                        files_in(&h.artifacts_dir()),
                    );
                    refused = Some((i, r, before, after));
                    break;
                }
            }
        }
        let (i, r, before, after) = refused.expect("the database ran out of pages");
        assert!(i > 0, "at least one save fitted the pages there were");
        assert_eq!(r.code, ProjectCode::DiskFull);
        assert_eq!(before, after, "no row and no artifact file left");
        assert_eq!(temporaries(&h.artifacts_dir()), 0);
        drop(h);
        let h = store.open(&id).expect("the project still opens");
        assert_eq!(run_count(&h), i64::from(i));
    }

    #[test]
    fn recovery_never_removes_a_file_another_connection_is_writing() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let writer = store.create("p").unwrap();
        let id = writer.id().as_str().to_string();
        let art_dir = writer.artifacts_dir();
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut writer = writer;
            fault::set(fault::Fault::Pause(
                fault::Point::BeforeRename,
                reached_tx,
                resume_rx,
            ));
            let r = writer.save(request(b"d", b"s", b"b"));
            (writer, r)
        });
        reached_rx.recv().unwrap();
        assert_eq!(
            temporaries(&art_dir),
            1,
            "the writer's temporary file is there"
        );
        let mut quick = ProjectRoot::open_root(&t.root()).unwrap();
        quick.busy = Duration::from_millis(200);
        // `still_open` is about deleting; a second open of an open project is
        // allowed, and its recovery must wait for the writer's lock.
        let second = quick.open(&id);
        assert_eq!(second.err().map(|r| r.code), Some(ProjectCode::Busy));
        assert_eq!(temporaries(&art_dir), 1, "recovery did not remove it");
        resume_tx.send(()).unwrap();
        let (writer, r) = worker.join().unwrap();
        r.expect("the writer's save completes");
        drop(writer);
        let h = store.open(&id).unwrap();
        assert_eq!(artifact_count(&h), 3);
        assert_eq!(run_count(&h), 1);
        assert_eq!(files_in(&art_dir).len(), 3);
    }

    #[test]
    fn a_run_left_running_reads_failed_after_reopening() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let h = store.create("p").unwrap();
        let id = h.id().as_str().to_string();
        for (n, state) in [(1, "running"), (2, "queued"), (3, "succeeded")] {
            h.conn
                .execute(
                    "INSERT INTO runs (run_id, capability_id, capability_version, state, end_code, agent_version, started_at, ended_at, saved_at) VALUES (?1, 'stats.describe', 1, ?2, NULL, '0.1.0', 5, NULL, 5)",
                    params![n, state],
                )
                .unwrap();
        }
        drop(h);
        let h = store.open(&id).unwrap();
        let runs = h.runs().unwrap();
        assert_eq!(runs[0].state, RunState::Failed);
        assert_eq!(runs[0].end_code, Some(ProjectCode::RunInterrupted));
        assert!(runs[0].ended_at.is_some());
        assert_eq!(runs[1].state, RunState::Failed);
        assert_eq!(runs[1].end_code, Some(ProjectCode::RunInterrupted));
        assert_eq!(
            runs[2].state,
            RunState::Succeeded,
            "a finished run is left as it is"
        );
        assert_eq!(runs[2].end_code, None);
    }

    #[test]
    fn an_interrupted_delete_finishes_at_the_next_start() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"d", b"s", b"b")).unwrap();
        let id = h.id().as_str().to_string();
        drop(h);
        fault::set(fault::Fault::Crash(fault::Point::AfterTombstone));
        let crashed = catch_unwind(AssertUnwindSafe(|| store.delete(&id)));
        assert!(crashed.is_err());
        let projects = t.root().join(PROJECTS_DIR);
        // Phase 5B-3a: the locks folder is beside the projects, and the
        // crashed delete left its project's lock file (decision 132).
        assert_eq!(
            files_in(&projects),
            [format!("{DELETING_PREFIX}{id}"), LOCKS_DIR.to_string()]
        );
        assert_eq!(
            files_in(&projects.join(LOCKS_DIR)),
            [format!("{id}{LOCK_SUFFIX}")]
        );
        assert!(
            store.list().unwrap().is_empty(),
            "a tombstone is not listed"
        );
        let again = ProjectRoot::open_root(&t.root()).unwrap();
        assert_eq!(
            files_in(&projects),
            [LOCKS_DIR],
            "the next start finished it"
        );
        assert!(
            files_in(&projects.join(LOCKS_DIR)).is_empty(),
            "and its lock file"
        );
        assert!(again.list().unwrap().is_empty());
        let mut stack = vec![t.0.clone()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap().flatten() {
                assert!(
                    !e.file_name().to_string_lossy().contains(&id),
                    "nothing named by the id"
                );
                if e.file_type().unwrap().is_dir() {
                    stack.push(e.path());
                }
            }
        }
    }

    #[test]
    fn bad_ids_and_hashes_are_refused_and_nothing_outside_the_root_is_touched() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"d", b"s", b"b")).unwrap();
        let real = h.id().as_str().to_string();
        // A victim outside the root, shaped like a project.
        let victim = t.0.join("outside").join("victim");
        fs::create_dir_all(victim.join(ARTIFACTS_DIR)).unwrap();
        fs::write(victim.join(DB_FILE), b"not yours").unwrap();
        fs::write(victim.join(ARTIFACTS_DIR).join("x"), b"keep").unwrap();
        let before = (
            sha_file(&victim.join(DB_FILE)),
            sha_file(&victim.join(ARTIFACTS_DIR).join("x")),
        );
        let absolute = victim.to_string_lossy().into_owned();
        for raw in [
            "../../outside/victim",
            "../",
            absolute.as_str(),
            "0123456789abcdef",
            real.to_uppercase().as_str(),
        ] {
            assert_eq!(
                store.open(raw).err().map(|r| r.code),
                Some(ProjectCode::BadProjectId),
                "{raw}"
            );
            assert_eq!(
                store.delete(raw).err().map(|r| r.code),
                Some(ProjectCode::BadProjectId),
                "{raw}"
            );
        }
        for raw in [
            "../../../outside/victim/artifacts/x",
            absolute.as_str(),
            "abc",
        ] {
            assert_eq!(
                h.read_artifact(raw).err().map(|r| r.code),
                Some(ProjectCode::BadHash),
                "{raw}"
            );
        }
        // A symlinked project folder, named by a valid id.
        #[cfg(unix)]
        {
            let linked = "fedcba9876543210fedcba9876543210";
            std::os::unix::fs::symlink(&victim, t.root().join(PROJECTS_DIR).join(linked)).unwrap();
            assert_eq!(
                store.open(linked).err().map(|r| r.code),
                Some(ProjectCode::SymlinkRefused)
            );
            assert_eq!(
                store.delete(linked).err().map(|r| r.code),
                Some(ProjectCode::SymlinkRefused)
            );
            let listed = store.list().unwrap();
            let entry = listed.iter().find(|e| e.id.as_str() == linked).unwrap();
            assert_eq!(entry.refused, Some(ProjectCode::SymlinkRefused));
            assert_eq!(entry.bytes, 0, "its target is not even measured");
            drop(h);
            ProjectRoot::open_root(&t.root()).unwrap();
            // An artifact replaced by a symlink to the victim is not read.
            let mut h = store.open(&real).unwrap();
            let d = hash_of(b"d");
            let p = h.artifacts_dir().join(d.as_hex());
            fs::remove_file(&p).unwrap();
            std::os::unix::fs::symlink(victim.join(ARTIFACTS_DIR).join("x"), &p).unwrap();
            assert_eq!(
                h.read_artifact(d.as_hex()).err().map(|r| r.code),
                Some(ProjectCode::ArtifactNotRegular)
            );
            drop(h);
            store.open(&real).unwrap();
            assert!(
                fs::symlink_metadata(&p).unwrap().file_type().is_symlink(),
                "recovery left the link"
            );
        }
        let after = (
            sha_file(&victim.join(DB_FILE)),
            sha_file(&victim.join(ARTIFACTS_DIR).join("x")),
        );
        assert_eq!(before, after, "the victim is unchanged");
        assert_eq!(files_in(&victim), [ARTIFACTS_DIR, DB_FILE]);
    }

    #[test]
    fn two_projects_never_share_a_folder_or_an_artifact_file() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut a = store.create("a").unwrap();
        let mut b = store.create("b").unwrap();
        assert_ne!(a.id(), b.id());
        assert_ne!(a.dir, b.dir);
        a.save(request(b"same", b"s", b"b")).unwrap();
        b.save(request(b"same", b"s", b"b")).unwrap();
        let h = hash_of(b"same");
        let pa = a.artifacts_dir().join(h.as_hex());
        let pb = b.artifacts_dir().join(h.as_hex());
        assert_ne!(pa, pb);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_ne!(
                fs::metadata(&pa).unwrap().ino(),
                fs::metadata(&pb).unwrap().ino()
            );
        }
        let a_id = a.id().as_str().to_string();
        drop(a);
        store.delete(&a_id).unwrap();
        assert!(!pa.exists());
        assert_eq!(b.read_artifact(h.as_hex()).unwrap(), b"same");
    }

    #[test]
    fn the_caps_admit_their_limit_and_refuse_one_byte_more() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let allowance = 1_000u64;
        let (data, spec, bundle) = (vec![1u8; 300], vec![2u8; 200], vec![3u8; 100]);
        let incoming = 600u64;

        // The project cap: exactly at it, then one byte over.
        for over in [1u64, 0] {
            let mut h = store.create("p").unwrap();
            let now = tree_bytes(&h.dir).unwrap();
            h.caps = Caps {
                project: now + incoming + allowance - over,
                all_projects: u64::MAX,
                db_allowance: allowance,
            };
            let r = h.save(request(&data, &spec, &bundle));
            if over == 1 {
                let r = r.unwrap_err();
                assert_eq!(r.code, ProjectCode::ProjectCap);
                assert_eq!(
                    (r.needed, r.limit),
                    (
                        Some(now + incoming + allowance),
                        Some(now + incoming + allowance - 1)
                    )
                );
                assert!(files_in(&h.artifacts_dir()).is_empty());
                assert_eq!(artifact_count(&h), 0);
                assert_eq!(tree_bytes(&h.dir).unwrap(), now, "nothing was written");
            } else {
                r.expect("exactly at the cap");
            }
        }

        // The all-projects cap, the same way.
        for over in [1u64, 0] {
            let mut h = store.create("q").unwrap();
            let all = tree_bytes(&t.root().join(PROJECTS_DIR)).unwrap();
            h.caps = Caps {
                project: u64::MAX,
                all_projects: all + incoming + allowance - over,
                db_allowance: allowance,
            };
            let r = h.save(request(&data, &spec, &bundle));
            if over == 1 {
                assert_eq!(r.unwrap_err().code, ProjectCode::AllProjectsCap);
                assert!(files_in(&h.artifacts_dir()).is_empty());
            } else {
                r.expect("exactly at the cap");
            }
        }

        // Several artifacts that pass the cap only with the last: none written.
        let mut h = store.create("r").unwrap();
        let now = tree_bytes(&h.dir).unwrap();
        h.caps = Caps {
            project: now + 300 + 200 + allowance,
            all_projects: u64::MAX,
            db_allowance: allowance,
        };
        assert_eq!(
            h.save(request(&data, &spec, &bundle)).unwrap_err().code,
            ProjectCode::ProjectCap
        );
        assert!(
            files_in(&h.artifacts_dir()).is_empty(),
            "not even the first two"
        );
        assert_eq!(artifact_count(&h), 0);
        assert_eq!(run_count(&h), 0);
    }

    #[test]
    fn a_tombstone_counts_toward_the_all_projects_cap() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let projects = t.root().join(PROJECTS_DIR);
        let before = tree_bytes(&projects).unwrap();
        let tomb = projects.join(format!("{DELETING_PREFIX}{}", "a".repeat(32)));
        fs::create_dir(&tomb).unwrap();
        fs::write(tomb.join("leftover"), vec![0u8; 4096]).unwrap();
        assert_eq!(tree_bytes(&projects).unwrap(), before + 4096);
        let mut h = store.create("p").unwrap();
        let all = tree_bytes(&projects).unwrap();
        h.caps = Caps {
            project: u64::MAX,
            all_projects: all + 6 + 10 - 4096,
            db_allowance: 10,
        };
        assert_eq!(
            h.save(request(b"dd", b"ss", b"bb")).unwrap_err().code,
            ProjectCode::AllProjectsCap
        );
    }

    /// Decision 173 (Research OS, 5B-4b): one size, whenever it is read. The
    /// hand test saw 0.10, 0.04 and 0.07 for one project, because each count
    /// took SQLite's `-wal` and `-shm` as they were at that moment.
    #[test]
    fn a_projects_size_read_while_it_is_open_equals_its_size_after_it_is_closed() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        let db = h.dir.join(DB_FILE);
        let db_at_create = fs::metadata(&db).unwrap().len();
        // Enough Saves to add pages to project.db: a Save that only rewrites
        // pages the file already has leaves its size the same whether or not
        // the log was folded in, and this test then could not see the
        // checkpoint (a deletion test of it stayed green with two Saves).
        for i in 0..40u8 {
            h.save(request(&[i; 64], &[i, 1], &[i, 2])).unwrap();
        }
        h.save(request(&[7u8; 5000], b"s2", b"b2")).unwrap();
        assert!(
            fs::metadata(&db).unwrap().len() > db_at_create,
            "project.db grew while open, so the size below includes what the Saves wrote"
        );
        let id = h.id().clone();
        let open = [
            store.list().unwrap()[0].bytes,
            store.usage().unwrap().0,
            store.project_bytes(&id).unwrap(),
            h.size_on_disk().unwrap(),
        ];
        assert!(open[0] > 5000, "the artifacts and the database count");
        assert!(
            open.iter().all(|&b| b == open[0]),
            "every size agrees while open: {open:?}"
        );
        h.close();
        let closed = [
            store.list().unwrap()[0].bytes,
            store.usage().unwrap().0,
            store.project_bytes(&id).unwrap(),
        ];
        assert_eq!(closed, [open[0]; 3], "the same size after it is closed");
    }

    /// The log is checkpointed and truncated after each Save (decision 173),
    /// so `project.db` holds what was saved while the project is open.
    /// `TRUNCATE` leaves the file there, at 0 bytes.
    #[test]
    fn after_a_second_save_the_log_is_empty_while_the_project_is_open() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"d1", b"s1", b"b1")).unwrap();
        h.save(request(b"d2", b"s2", b"b2")).unwrap();
        let wal = h.dir.join(WAL_FILE);
        assert!(wal.exists(), "open, so the log is there");
        assert_eq!(fs::metadata(&wal).unwrap().len(), 0, "and empty");
        assert_eq!(run_count(&h), 2);
    }

    /// `tree_bytes` skips exactly the two files SQLite owns, by their full
    /// names; every other file counts, a tombstone's included.
    #[test]
    fn a_size_skips_exactly_the_log_and_the_shared_memory_file() {
        let t = TempRoot::new();
        let dir = t.root().join("sized");
        fs::create_dir_all(dir.join("inner")).unwrap();
        for (name, len) in [
            (WAL_FILE, 1000usize),
            (SHM_FILE, 2000),
            ("inner/project.db-wal", 4000),
            ("project.db", 3),
            ("x.db-wal", 5),
            ("project.db-wal.old", 7),
            ("inner/project.db-journal", 11),
        ] {
            fs::write(dir.join(name), vec![0u8; len]).unwrap();
        }
        assert_eq!(tree_bytes(&dir).unwrap(), 3 + 5 + 7 + 11);
    }

    #[test]
    fn refused_databases_are_left_untouched() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let snapshot = |dir: &Path| -> Vec<(String, String)> {
            let mut v = Vec::new();
            for n in files_in(dir)
                .into_iter()
                .filter(|n| !n.ends_with("-wal") && !n.ends_with("-shm"))
            {
                let p = dir.join(&n);
                if p.is_file() {
                    v.push((n, sha_file(&p)));
                }
            }
            for n in files_in(&dir.join(ARTIFACTS_DIR)) {
                v.push((
                    format!("artifacts/{n}"),
                    sha_file(&dir.join(ARTIFACTS_DIR).join(&n)),
                ));
            }
            v
        };
        let make = |title: &str| {
            let mut h = store.create(title).unwrap();
            h.save(request(title.as_bytes(), b"s", b"b")).unwrap();
            let id = h.id().as_str().to_string();
            let dir = h.dir.clone();
            drop(h);
            (id, dir)
        };

        let (missing, dir_m) = make("missing");
        fs::remove_file(dir_m.join(DB_FILE)).unwrap();
        let before = snapshot(&dir_m);
        assert_eq!(
            store.open(&missing).err().map(|r| r.code),
            Some(ProjectCode::DbMissing)
        );
        assert_eq!(snapshot(&dir_m), before);

        let (corrupt, dir_c) = make("corrupt");
        set_mode(&dir_c.join(DB_FILE), 0o600).unwrap();
        fs::write(dir_c.join(DB_FILE), vec![0x5a; 8192]).unwrap();
        let before = snapshot(&dir_c);
        assert_eq!(
            store.open(&corrupt).err().map(|r| r.code),
            Some(ProjectCode::DbUnreadable)
        );
        assert_eq!(snapshot(&dir_c), before);

        let (newer, dir_n) = make("newer");
        {
            let c = open_conn(&dir_n.join(DB_FILE), BUSY_TIMEOUT).unwrap();
            c.pragma_update(None, "user_version", 2).unwrap();
        }
        let before = snapshot(&dir_n);
        assert_eq!(
            store.open(&newer).err().map(|r| r.code),
            Some(ProjectCode::DbNewerSchema)
        );
        assert_eq!(snapshot(&dir_n), before);
        assert_eq!(
            files_in(&dir_n.join(ARTIFACTS_DIR)).len(),
            3,
            "its artifacts stay"
        );

        let listed = store.list().unwrap();
        let code_of = |id: &str| listed.iter().find(|e| e.id.as_str() == id).unwrap().refused;
        assert_eq!(code_of(&missing), Some(ProjectCode::DbMissing));
        assert_eq!(code_of(&corrupt), Some(ProjectCode::DbUnreadable));
        assert_eq!(code_of(&newer), Some(ProjectCode::DbNewerSchema));
        assert_eq!(snapshot(&dir_n), before, "listing touched nothing either");
    }

    #[cfg(unix)]
    #[test]
    fn folders_databases_and_artifacts_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777;
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"d", b"s", b"b")).unwrap();
        assert_eq!(mode(&t.root().join(PROJECTS_DIR)), 0o700);
        assert_eq!(mode(&h.dir), 0o700);
        assert_eq!(mode(&h.artifacts_dir()), 0o700);
        assert_eq!(mode(&h.dir.join(DB_FILE)), 0o600);
        let wal = h.dir.join("project.db-wal");
        let shm = h.dir.join("project.db-shm");
        assert!(
            wal.exists() && shm.exists(),
            "open, so SQLite's own files are there"
        );
        assert_eq!(mode(&wal), 0o600);
        assert_eq!(mode(&shm), 0o600);
        for n in files_in(&h.artifacts_dir()) {
            assert_eq!(mode(&h.artifacts_dir().join(&n)), 0o400, "{n}");
        }
    }

    #[test]
    fn an_artifact_is_reused_never_overwritten_and_a_damaged_one_is_refused() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"d", b"s1", b"b1")).unwrap();
        let p = h.artifacts_dir().join(hash_of(b"d").as_hex());
        let modified = fs::metadata(&p).unwrap().modified().unwrap();
        h.save(request(b"d", b"s2", b"b2")).unwrap();
        assert_eq!(
            fs::metadata(&p).unwrap().modified().unwrap(),
            modified,
            "reused, not rewritten"
        );
        assert_eq!(h.runs().unwrap().len(), 2);
        set_mode(&p, 0o600).unwrap();
        fs::write(&p, b"D").unwrap();
        let r = h.save(request(b"d", b"s3", b"b3")).unwrap_err();
        assert_eq!(r.code, ProjectCode::ArtifactWrongHash);
        assert_eq!(fs::read(&p).unwrap(), b"D", "never overwritten");
        assert_eq!(h.runs().unwrap().len(), 2);
        assert!(
            !h.artifacts_dir().join(hash_of(b"s3").as_hex()).exists(),
            "the Save left nothing"
        );
    }

    #[test]
    fn a_save_is_refused_before_writing_when_its_request_is_wrong() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();

        let mut cycle = request(b"d", b"s", b"b");
        cycle.run.generated.push(hash_of(b"d"));
        assert_eq!(h.save(cycle).unwrap_err().code, ProjectCode::ProvCycle);

        let mut unfinished = request(b"d", b"s", b"b");
        unfinished.run.state = RunState::Running;
        assert_eq!(
            h.save(unfinished).unwrap_err().code,
            ProjectCode::RunNotFinished
        );

        let mut lying = request(b"d", b"s", b"b");
        lying.artifacts[0].bytes = b"e".to_vec();
        assert_eq!(h.save(lying).unwrap_err().code, ProjectCode::HashMismatch);

        let mut format = request(b"d", b"s", b"b");
        format.artifacts[0].format = DataFormat::Json;
        assert_eq!(h.save(format).unwrap_err().code, ProjectCode::BadFormat);

        let mut unknown = request(b"d", b"s", b"b");
        unknown
            .run
            .used
            .push((hash_of(b"elsewhere"), UsedRole::Data));
        assert_eq!(
            h.save(unknown).unwrap_err().code,
            ProjectCode::UnknownArtifact
        );

        assert!(files_in(&h.artifacts_dir()).is_empty());
        assert_eq!(artifact_count(&h), 0);
        assert_eq!(run_count(&h), 0);
    }

    #[test]
    fn delete_waits_for_close_and_removes_read_only_files() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut h = store.create("p").unwrap();
        h.save(request(b"d", b"s", b"b")).unwrap();
        let id = h.id().as_str().to_string();
        assert_eq!(store.delete(&id).unwrap_err().code, ProjectCode::StillOpen);
        h.close();
        store.delete(&id).unwrap();
        let projects = t.root().join(PROJECTS_DIR);
        assert_eq!(files_in(&projects), [LOCKS_DIR]);
        assert!(
            files_in(&projects.join(LOCKS_DIR)).is_empty(),
            "the lock file went last"
        );
        assert_eq!(
            store.open(&id).err().map(|r| r.code),
            Some(ProjectCode::NotFound)
        );
        assert_eq!(
            store.delete(&id).err().map(|r| r.code),
            Some(ProjectCode::NotFound)
        );
    }

    #[test]
    fn the_list_is_a_scan_of_the_folder() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut a = store.create("Alpha").unwrap();
        a.save(request(b"d", b"s", b"b")).unwrap();
        let b = store.create("Beta").unwrap();
        let (ida, idb) = (a.id().clone(), b.id().clone());
        drop((a, b));
        let projects = t.root().join(PROJECTS_DIR);
        fs::create_dir(projects.join("not-a-project")).unwrap();
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 2);
        for e in &listed {
            assert_eq!(e.refused, None);
            assert!(e.bytes > 0);
            let want = if e.id == ida { "Alpha" } else { "Beta" };
            assert!(e.id == ida || e.id == idb);
            assert_eq!(e.title.as_ref().unwrap().as_str(), want);
        }
    }

    #[test]
    fn a_crashed_create_leaves_no_project_and_the_next_start_clears_it() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let projects = t.root().join(PROJECTS_DIR);
        let staged = projects.join(format!("{CREATING_PREFIX}{}", "b".repeat(32)));
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join(DB_FILE), b"half").unwrap();
        assert!(
            store.list().unwrap().is_empty(),
            "a staging folder is not a project"
        );
        ProjectRoot::open_root(&t.root()).unwrap();
        assert_eq!(files_in(&projects), [LOCKS_DIR]);
        assert!(files_in(&projects.join(LOCKS_DIR)).is_empty());
    }

    #[test]
    fn errors_map_to_closed_codes() {
        #[cfg(unix)]
        {
            assert_eq!(
                io_code(&io::Error::from_raw_os_error(libc::ENOSPC)),
                ProjectCode::DiskFull
            );
            assert_eq!(
                io_code(&io::Error::from_raw_os_error(libc::EDQUOT)),
                ProjectCode::DiskFull
            );
            assert_eq!(
                io_code(&io::Error::from_raw_os_error(libc::EACCES)),
                ProjectCode::IoFailed
            );
        }
        let full = rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(13), None);
        assert_eq!(db_code(&full), ProjectCode::DiskFull);
        let busy = rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(5), None);
        assert_eq!(db_code(&busy), ProjectCode::Busy);
        let notadb = rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(26), None);
        assert_eq!(db_code(&notadb), ProjectCode::DbUnreadable);
        assert_eq!(
            ProjectRoot::open_root(Path::new("relative"))
                .err()
                .map(|r| r.code),
            Some(ProjectCode::RootUnavailable)
        );
    }

    // ---- Phase 5B-2: the first Save, and the process-wide lock -------------

    fn title(t: &str) -> ProjectTitle {
        ProjectTitle::parse(t).unwrap()
    }

    #[test]
    fn a_first_save_makes_its_project_and_its_run_in_one_step() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let (mut h, run) = store
            .create_with_save(&title("Chapter 3"), request(b"d", b"s", b"b"))
            .unwrap();
        assert_eq!(run, StoredRunId(1));
        assert_eq!(h.record().unwrap().title.as_str(), "Chapter 3");
        assert_eq!(h.read_artifact(hash_of(b"d").as_hex()).unwrap(), b"d");
        let projects = t.root().join(PROJECTS_DIR);
        assert_eq!(
            files_in(&projects),
            [LOCKS_DIR.to_string(), h.id().as_str().to_string()],
            "no staging folder left"
        );
        assert_eq!(
            files_in(&projects.join(LOCKS_DIR)),
            [format!("{}{LOCK_SUFFIX}", h.id().as_str())],
            "the open project's lock file"
        );
        let id = h.id().clone();
        drop(h);
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
        assert_eq!(listed[0].run_count, Some(1));
        assert!(listed[0].saved_at.is_some());
    }

    #[test]
    fn a_crash_during_a_first_save_leaves_no_project() {
        for at in [
            fault::Point::AfterCaps,
            fault::Point::BeforeCommit,
            fault::Point::BeforeStagingRename,
        ] {
            let t = TempRoot::new();
            let store = ProjectRoot::open_root(&t.root()).unwrap();
            fault::set(fault::Fault::Crash(at));
            let crashed = catch_unwind(AssertUnwindSafe(|| {
                store.create_with_save(&title("p"), request(b"d", b"s", b"b"))
            }));
            assert!(crashed.is_err(), "{at:?}: the crash fired");
            assert!(
                store.list().unwrap().is_empty(),
                "{at:?}: no project is listed"
            );
            let projects = t.root().join(PROJECTS_DIR);
            assert!(
                files_in(&projects)
                    .iter()
                    .all(|n| n.starts_with(CREATING_PREFIX) || n == LOCKS_DIR),
                "{at:?}: only a staging folder: {:?}",
                files_in(&projects)
            );
            ProjectRoot::open_root(&t.root()).unwrap();
            assert_eq!(
                files_in(&projects),
                [LOCKS_DIR],
                "{at:?}: the next start cleared it"
            );
            assert!(
                files_in(&projects.join(LOCKS_DIR)).is_empty(),
                "{at:?}: and its lock file"
            );
        }
    }

    #[test]
    fn a_refused_first_save_leaves_no_folder() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root_with(
            &t.root(),
            Caps {
                project: 1024,
                all_projects: u64::MAX,
                db_allowance: 0,
            },
        )
        .unwrap();
        let r = store
            .create_with_save(&title("p"), request(&[7u8; 2048], b"s", b"b"))
            .err()
            .map(|r| r.code);
        assert_eq!(r, Some(ProjectCode::ProjectCap));
        let mut bad = request(b"d", b"s", b"b");
        bad.run.state = RunState::Running;
        assert_eq!(
            store
                .create_with_save(&title("p"), bad)
                .err()
                .map(|r| r.code),
            Some(ProjectCode::RunNotFinished)
        );
        let projects = t.root().join(PROJECTS_DIR);
        assert_eq!(files_in(&projects), [LOCKS_DIR]);
        assert!(
            files_in(&projects.join(LOCKS_DIR)).is_empty(),
            "a refused first Save leaves no lock file"
        );
    }

    /// A Save in another thread, paused just after its cap check until the
    /// test lets it go.
    fn paused_save(
        mut h: ProjectHandle,
        req: SaveRequest,
    ) -> (
        std::thread::JoinHandle<(ProjectHandle, Result<StoredRunId, ProjectRefusal>)>,
        mpsc::Receiver<()>,
        mpsc::Sender<()>,
    ) {
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            fault::set(fault::Fault::Pause(
                fault::Point::AfterCaps,
                reached_tx,
                resume_rx,
            ));
            let r = h.save(req);
            (h, r)
        });
        (worker, reached_rx, resume_tx)
    }

    #[test]
    fn two_saves_into_two_projects_cannot_pass_the_all_projects_cap_together() {
        let t = TempRoot::new();
        let store = ProjectRoot::open_root(&t.root()).unwrap();
        let mut a = store.create("a").unwrap();
        let mut b = store.create("b").unwrap();
        let (data_a, data_b) = (vec![1u8; 4000], vec![2u8; 4000]);
        // Room for one of the two Saves, not both.
        let all = tree_bytes(&t.root().join(PROJECTS_DIR)).unwrap();
        let caps = Caps {
            project: u64::MAX,
            all_projects: all + 4000 + 2 + 2 + 100 + 3000,
            db_allowance: 100,
        };
        a.caps = caps;
        b.caps = caps;
        let (worker, reached, resume) = paused_save(a, request(&data_a, b"sa", b"ba"));
        reached.recv().unwrap();
        // A has passed its cap check and holds the lock. B's Save runs in its
        // own thread: it must wait for A, then see A's bytes.
        let (done_tx, done_rx) = mpsc::channel();
        let other = std::thread::spawn(move || {
            let r = b.save(request(&data_b, b"sb", b"bb"));
            done_tx.send(()).unwrap();
            r
        });
        // Observed first, asserted after A is let go, so a failure is a red
        // test rather than a paused thread left waiting.
        let early = done_rx.recv_timeout(Duration::from_millis(300)).is_ok();
        resume.send(()).unwrap();
        let (_a, ra) = worker.join().unwrap();
        let rb = other.join().unwrap();
        assert!(!early, "B's Save finished while A held the lock");
        ra.expect("A fits");
        assert_eq!(
            rb.unwrap_err().code,
            ProjectCode::AllProjectsCap,
            "B sees A's bytes"
        );
    }

    #[test]
    fn a_delete_waits_for_a_save_in_flight() {
        let t = TempRoot::new();
        let store = Arc::new(ProjectRoot::open_root(&t.root()).unwrap());
        let a = store.create("a").unwrap();
        let other = store.create("other").unwrap();
        let other_id = other.id().as_str().to_string();
        drop(other);
        let (worker, reached, resume) = paused_save(a, request(b"d", b"s", b"b"));
        reached.recv().unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let s2 = Arc::clone(&store);
        let deleter = std::thread::spawn(move || {
            let r = s2.delete(&other_id);
            done_tx.send(()).unwrap();
            r
        });
        let early = done_rx.recv_timeout(Duration::from_millis(300)).is_ok();
        resume.send(()).unwrap();
        let saved = worker.join().unwrap().1;
        let deleted = deleter.join().unwrap();
        assert!(!early, "the delete ran during the Save");
        saved.expect("the Save completes");
        deleted.expect("then the delete");
    }

    #[test]
    fn the_root_is_tauris_local_app_data_dir() {
        use tauri::Manager;
        let app = tauri::test::mock_app();
        let want = app.path().app_local_data_dir().unwrap();
        assert_eq!(projects_root(app.handle()).unwrap(), want);
    }
}
