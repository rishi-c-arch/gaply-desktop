//! **A store session puts user text only where ADR-009 says it may (Research
//! OS Phase 5B-1; decisions 101, 105; ADR-009 §3, §16).**
//!
//! One session through the store's public API, with sentinels planted in
//! every place user text can come from: a data file's cells, level labels
//! and header; a result bundle's header and labels; the project's title; the
//! original file's name; and the root folder's own path. Then every byte the
//! session left is read back: every file under the root (`project.db`, its
//! `-wal` and `-shm`, and each artifact), every refusal's `Display` and
//! `Debug`, and every tracing line at every level.
//!
//! * Cells, labels and headers appear only inside artifact files.
//! * The title appears only in `project.title`, read column by column from
//!   every table, and in no artifact, error or log line.
//! * The file name and the root's path appear nowhere. The API has no
//!   parameter for a file name (decision 101), so that row holds by
//!   construction; it is swept anyway, so a later parameter is caught.
//!
//! One test in its own binary: a thread-default subscriber misses events
//! when other tests share the binary and register the call sites first.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use gaply_core::contracts::{CapabilityId, RunState};
use gaply_core::research_project::{
    hex_lower, ArtifactHash, ArtifactKind, DataFormat, ProjectRefusal, UsedRole,
};
use sha2::{Digest, Sha256};

const CELL: &str = "CELL_SENTINEL_c41";
const LEVEL: &str = "LEVEL_SENTINEL_l52";
const HEADER: &str = "HEADER_SENTINEL_h63";
const TITLE: &str = "TITLE_SENTINEL_t74";
const FILE_NAME: &str = "FILENAME_SENTINEL_f85.csv";
const ROOT_MARK: &str = "ROOTPATH_SENTINEL_r96";

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn hash_of(b: &[u8]) -> ArtifactHash {
    ArtifactHash::parse(&hex_lower(&Sha256::digest(b))).unwrap()
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        let ft = e.file_type().unwrap();
        if ft.is_dir() {
            files_under(&p, out);
        } else if ft.is_file() {
            out.push(p);
        }
    }
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Every text cell of every table of the database at `db`, as
/// (table.column, value).
fn every_text_cell(db: &Path) -> Vec<(String, String)> {
    let c = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    let tables: Vec<String> = {
        let mut st = c
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap();
        let names = st
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        names
    };
    assert!(tables.len() >= 6, "{tables:?}");
    let mut out = Vec::new();
    for t in tables {
        let cols: Vec<String> = {
            let mut st = c
                .prepare(&format!("SELECT name FROM pragma_table_info('{t}')"))
                .unwrap();
            let names = st
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            names
        };
        for col in cols {
            let mut st = c
                .prepare(&format!("SELECT CAST(\"{col}\" AS TEXT) FROM \"{t}\""))
                .unwrap();
            for v in st.query_map([], |r| r.get::<_, Option<String>>(0)).unwrap() {
                if let Some(v) = v.unwrap() {
                    out.push((format!("{t}.{col}"), v));
                }
            }
        }
    }
    out
}

#[test]
fn user_text_stays_in_artifacts_and_the_title_in_its_own_column() {
    let base = std::env::temp_dir().join(format!("gaply-5b1-{ROOT_MARK}-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let base = base.canonicalize().unwrap();
    let root = base.join("root");
    let root_text = root.to_string_lossy().into_owned();

    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();

    let data = format!("{HEADER},group\n1.5,{LEVEL}\n{CELL},{LEVEL}\n").into_bytes();
    let spec = br#"{"schemaVersion":1,"inputs":[0],"columns":[0,1]}"#.to_vec();
    let bundle = format!(r#"{{"header":"{HEADER}","levels":["{LEVEL}"],"n":2}}"#).into_bytes();
    let (d, s, b) = (hash_of(&data), hash_of(&spec), hash_of(&bundle));
    let request = |data: Vec<u8>| SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse("stats.describe").unwrap(),
            capability_version: 1,
            state: RunState::Succeeded,
            started_at: 10,
            ended_at: 11,
            used: vec![(d.clone(), UsedRole::Data), (s.clone(), UsedRole::Spec)],
            generated: vec![b.clone()],
            derived: vec![(b.clone(), d.clone())],
        },
        artifacts: vec![
            ArtifactInput {
                kind: ArtifactKind::DataFile,
                format: DataFormat::Csv,
                schema_version: 1,
                expected: d.clone(),
                bytes: data,
            },
            ArtifactInput {
                kind: ArtifactKind::OperationSpec,
                format: DataFormat::Json,
                schema_version: 1,
                expected: s.clone(),
                bytes: spec.clone(),
            },
            ArtifactInput {
                kind: ArtifactKind::ResultBundle,
                format: DataFormat::Json,
                schema_version: 1,
                expected: b.clone(),
                bytes: bundle.clone(),
            },
        ],
    };

    let (db_files_open, id, refusals, debugs) =
        tracing::subscriber::with_default(subscriber, || {
            let mut refusals: Vec<ProjectRefusal> = Vec::new();
            let store = ProjectRoot::open_root(&root).unwrap();
            let mut h = store.create(TITLE).unwrap();
            let id = h.id().as_str().to_string();
            let saved = h.save(request(data.clone()));
            saved.unwrap();
            assert_eq!(h.read_artifact(d.as_hex()).unwrap(), data);
            store.list().unwrap();

            // Refusals, each carrying a sentinel in what was refused.
            refusals.push(store.create(&format!("{TITLE}\n")).err().unwrap());
            refusals.push(store.open(&format!("../{FILE_NAME}")).err().unwrap());
            refusals.push(store.open(&root_text).err().unwrap());
            refusals.push(h.read_artifact(CELL).err().unwrap());
            let mut lying = request(data.clone());
            lying.artifacts[0].bytes = format!("{CELL},{LEVEL}\n").into_bytes();
            refusals.push(h.save(lying).err().unwrap());
            refusals.push(store.delete(&id).err().unwrap());
            let debugs = format!("{:?}\n", request(data.clone()));

            // SQLite's own files exist while the project is open.
            let mut open_files = Vec::new();
            files_under(&root, &mut open_files);
            let db_files: Vec<(PathBuf, Vec<u8>)> = open_files
                .iter()
                .filter(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("project.db")
                })
                .map(|p| (p.clone(), std::fs::read(p).unwrap()))
                .collect();
            drop(h);
            (db_files, id, refusals, debugs)
        });
    let mut errors = debugs;
    for r in &refusals {
        errors.push_str(&format!("{r}\n{r:?}\n"));
    }
    assert_eq!(refusals.len(), 6);

    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.lines().count() >= 4,
        "the capture is live: {} lines",
        logs.lines().count()
    );
    assert!(logs.contains(&id), "a project appears in the log by its id");
    assert!(
        logs.contains(&d.as_hex()[..8]) && !logs.contains(d.as_hex()),
        "an artifact by its prefix only"
    );
    assert!(errors.lines().count() >= 7);

    let names: Vec<String> = db_files_open
        .iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with("-wal")) && names.iter().any(|n| n.ends_with("-shm")),
        "{names:?}"
    );

    let mut after = Vec::new();
    files_under(&root, &mut after);
    let mut db_bytes: Vec<(String, Vec<u8>)> = db_files_open
        .iter()
        .map(|(p, b)| (format!("{} (open)", p.display()), b.clone()))
        .collect();
    let mut artifact_bytes: Vec<(String, Vec<u8>)> = Vec::new();
    for p in &after {
        let label = p.display().to_string();
        let bytes = std::fs::read(p).unwrap();
        if p.parent().unwrap().file_name().unwrap() == "artifacts" {
            artifact_bytes.push((label, bytes));
        } else {
            db_bytes.push((label, bytes));
        }
    }
    assert_eq!(artifact_bytes.len(), 3);
    assert!(
        db_bytes.len() >= 3,
        "project.db open with -wal and -shm, then closed"
    );

    for user_text in [CELL, LEVEL, HEADER] {
        assert!(
            artifact_bytes.iter().any(|(_, b)| contains(b, user_text)),
            "{user_text} is in an artifact"
        );
        for (label, b) in &db_bytes {
            assert!(!contains(b, user_text), "{user_text} in {label}");
        }
        assert!(!errors.contains(user_text), "{user_text} in an error");
        assert!(!logs.contains(user_text), "{user_text} in a log line");
    }

    assert!(
        db_bytes.iter().any(|(_, b)| contains(b, TITLE)),
        "the title is stored"
    );
    let db = after
        .iter()
        .find(|p| p.file_name().unwrap() == "project.db")
        .unwrap();
    let holding: Vec<String> = every_text_cell(db)
        .into_iter()
        .filter(|(_, v)| v.contains(TITLE))
        .map(|(c, _)| c)
        .collect();
    assert_eq!(
        holding,
        ["project.title"],
        "the title is in its own column and nowhere else"
    );
    for (label, b) in &artifact_bytes {
        assert!(!contains(b, TITLE), "the title in {label}");
    }
    assert!(!errors.contains(TITLE), "the title in an error");
    assert!(!logs.contains(TITLE), "the title in a log line");

    for nowhere in [
        FILE_NAME,
        ROOT_MARK,
        root_text.as_str(),
        base.to_string_lossy().as_ref(),
    ] {
        for (label, b) in db_bytes.iter().chain(artifact_bytes.iter()) {
            assert!(!contains(b, nowhere), "{nowhere} in {label}");
        }
        assert!(!errors.contains(nowhere), "{nowhere} in an error");
        assert!(!logs.contains(nowhere), "{nowhere} in a log line");
    }

    std::fs::remove_dir_all(&base).ok();
    if base.exists() {
        // Artifacts are 0400; their folder is the store's, 0700, so a plain
        // removal works on Unix. Anything left is reported, not hidden.
        panic!("the test folder could not be removed");
    }
}
