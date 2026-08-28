//! Generative-model installer — the second (and last) network operation in the
//! AI layer, mirroring [`crate::ai::model_install`] file-for-file.
//!
//! # Why this is not just the embedding installer with different URLs
//!
//! The embedding model is 133 MB and downloads into memory in one `GET`. A
//! generative candidate is 1–2 GB, which forces three changes and nothing else:
//!
//! - **Streamed to disk**, never buffered. Holding 2 GB of `Bytes` to hash it
//!   would cost more RAM than running the model.
//! - **RESUMABLE.** A 2 GB download over a domestic connection will be
//!   interrupted; without resume, an interrupted install is an install that
//!   never finishes. Progress is kept in a `.part` file and continued with a
//!   `Range` request.
//! - **Cancellation is polled mid-stream**, per chunk, not between files. One
//!   file IS the whole download here, so between-files cancellation would mean
//!   "cancel does nothing for twenty minutes".
//!
//! Everything else is deliberately identical: pinned revision + compiled-in
//! sha256, verify-before-rename, offline install as a first-class path, explicit
//! user action only, and registration in `ai_model_registry`.
//!
//! # The hash is over the COMPLETE file
//!
//! Resume makes this worth stating. Bytes arriving from a second connection are
//! not covered by the first connection's TLS session, and a `.part` file can be
//! anything at all between runs. The sha256 is therefore computed by reading the
//! finished file back from disk, not by hashing the stream as it arrives — a
//! streaming hash would be computed over exactly the bytes we happened to
//! receive, which is not the thing that needs verifying.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use gaply_core::GaplyError;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// One pinned candidate: everything needed to fetch, verify and register it.
///
/// `registry_id` is the id the harness's `--model` takes and the id written to
/// `ai_model_registry` — the engine is model-file-agnostic (§9.9), so this
/// string is the only handle anything outside this module needs.
pub struct GenCandidate {
    pub registry_id: &'static str,
    pub display_name: &'static str,
    pub quant: &'static str,
    /// Approximate parameter count, for display and the RAM warning.
    pub params_b: f32,
    /// True for candidates that should not be offered on an 8 GB machine.
    pub needs_16gb: bool,
    pub gguf: PinnedRemote,
    pub tokenizer: PinnedRemote,
}

/// A single pinned remote file.
pub struct PinnedRemote {
    /// `owner/repo` on Hugging Face.
    pub repo: &'static str,
    /// A full commit sha — never a branch. "Pinned" means a fixed tree.
    pub revision: &'static str,
    /// Path within the repo, and the name used on disk.
    pub file: &'static str,
    /// sha256 of the complete file, measured from the pinned revision.
    pub sha256: &'static str,
    pub bytes: u64,
}

impl PinnedRemote {
    pub fn url(&self) -> String {
        format!("https://huggingface.co/{}/resolve/{}/{}", self.repo, self.revision, self.file)
    }
    /// Name on disk. Flattened — a repo path never becomes a directory tree.
    pub fn local_name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(self.file)
    }
}

impl GenCandidate {
    pub fn files(&self) -> [&PinnedRemote; 2] {
        [&self.gguf, &self.tokenizer]
    }
    pub fn total_bytes(&self) -> u64 {
        self.gguf.bytes + self.tokenizer.bytes
    }
}

/// The pinned bake-off candidates (§11 D21: Qwen2.5-Instruct only — the loader
/// is qwen2-specific).
///
/// The 7B entry is absent by decision, not by omission: the official Q4_K_M is
/// published as two split GGUF files and the loader takes one path (§11 D22).
pub const CANDIDATES: &[GenCandidate] = &[
    GenCandidate {
        registry_id: "qwen2.5-1.5b-instruct-q4km",
        display_name: "Qwen2.5 1.5B Instruct (Q4_K_M)",
        quant: "Q4_K_M",
        params_b: 1.5,
        needs_16gb: false,
        gguf: PinnedRemote {
            repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
            revision: "91cad51170dc346986eccefdc2dd33a9da36ead9",
            file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
            bytes: 1_117_320_736,
        },
        tokenizer: PinnedRemote {
            repo: "Qwen/Qwen2.5-1.5B-Instruct",
            revision: "989aa7980e4cf806f80c7fef2b1adb7bc71aa306",
            file: "tokenizer.json",
            sha256: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
            bytes: 7_031_645,
        },
    },
    GenCandidate {
        registry_id: "qwen2.5-3b-instruct-q4km",
        display_name: "Qwen2.5 3B Instruct (Q4_K_M)",
        quant: "Q4_K_M",
        params_b: 3.0,
        needs_16gb: false,
        gguf: PinnedRemote {
            repo: "Qwen/Qwen2.5-3B-Instruct-GGUF",
            revision: "7dabda4d13d513e3e842b20f0d435c732f172cbe",
            file: "qwen2.5-3b-instruct-q4_k_m.gguf",
            sha256: "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d",
            bytes: 2_104_932_768,
        },
        tokenizer: PinnedRemote {
            repo: "Qwen/Qwen2.5-3B-Instruct",
            revision: "aa8e72537993ba99e69dfaafa59ed015b17504d1",
            file: "tokenizer.json",
            // MEASURED byte-identical to the 1.5B tokenizer (D23). Pinned
            // separately anyway: the fact that they match today is a
            // measurement, not a guarantee about the next revision.
            sha256: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
            bytes: 7_031_645,
        },
    },
];

pub fn candidate(registry_id: &str) -> Option<&'static GenCandidate> {
    CANDIDATES.iter().find(|c| c.registry_id == registry_id)
}

/// Progress for the install command's Channel.
///
/// `Progress` carries absolute byte counts rather than a percentage so a caller
/// resuming a half-finished file sees where it actually is.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GenInstallEvent {
    Started { model_id: String, files: usize, total_bytes: u64 },
    AlreadyPresent { file: String },
    /// `from_bytes` > 0 means this is a RESUME, not a fresh start.
    Downloading { file: String, from_bytes: u64, total_bytes: u64 },
    Progress { file: String, bytes: u64, total_bytes: u64 },
    Verifying { file: String },
    Verified { file: String },
    Cancelled,
    Done { model_id: String, dir: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenInstallReport {
    pub model_id: String,
    pub dir: String,
    /// True when every file was already present and verified — no network at all.
    pub offline: bool,
    pub bytes_downloaded: u64,
}

/// `<app data>/models/<registry id>/`.
pub fn model_dir(app_data: &Path, c: &GenCandidate) -> PathBuf {
    app_data.join("models").join(c.registry_id)
}

/// sha256 of a file, read back from disk in bounded chunks.
///
/// Chunked because these files are gigabytes: `std::fs::read` would allocate the
/// whole thing to hash it.
pub fn sha256_file(path: &Path) -> Result<String, GaplyError> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Is this file present, the right length, and the right hash?
///
/// The length check is first because it is nearly free and rules out the common
/// case (a truncated download) without reading a gigabyte.
pub fn file_verified(dir: &Path, r: &PinnedRemote) -> bool {
    let p = dir.join(r.local_name());
    if !p.is_file() {
        return false;
    }
    if r.bytes > 0 && std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) != r.bytes {
        return false;
    }
    sha256_file(&p).map(|h| h == r.sha256).unwrap_or(false)
}

pub fn all_files_verified(dir: &Path, c: &GenCandidate) -> bool {
    c.files().iter().all(|r| file_verified(dir, r))
}

/// Verify an already-present install, naming the first file that fails.
///
/// This is the OFFLINE MANUAL PATH's check and the startup precondition. It
/// performs no network call, which is what makes an air-gapped install possible:
/// drop the files in by hand, run the install command, and it registers.
pub fn verify_install(dir: &Path, c: &GenCandidate) -> Result<(), GaplyError> {
    for r in c.files() {
        let p = dir.join(r.local_name());
        if !p.is_file() {
            return Err(GaplyError::NotFound {
                entity: "generative model file",
                id: p.display().to_string(),
            });
        }
        if r.bytes > 0 {
            let len = std::fs::metadata(&p)?.len();
            if len != r.bytes {
                return Err(GaplyError::Validation(format!(
                    "{} is {len} bytes, expected {}. The download is incomplete or not the \
                     pinned revision; re-run the model install.",
                    r.local_name(),
                    r.bytes
                )));
            }
        }
        let got = sha256_file(&p)?;
        if got != r.sha256 {
            return Err(GaplyError::Validation(format!(
                "{} failed verification — expected sha256 {}, found {}. The file is corrupt or \
                 not the pinned revision; re-run the model install.",
                r.local_name(),
                r.sha256,
                got
            )));
        }
    }
    Ok(())
}

/// Record a verified generative model in `ai_model_registry`.
pub fn register(
    db: &gaply_core::Database,
    dir: &Path,
    c: &GenCandidate,
) -> Result<(), GaplyError> {
    gaply_core::ai_engine::registry::register_model(
        db,
        gaply_core::ai_engine::registry::ModelRow {
            id: c.registry_id.to_string(),
            kind: "generative".to_string(),
            display_name: c.display_name.to_string(),
            // The GGUF itself, not the directory: the loader takes a file path,
            // and a registry row that needs a naming convention applied to it is
            // a row that can be read wrong.
            file_path: dir.join(c.gguf.local_name()).display().to_string(),
            sha256: Some(c.gguf.sha256.to_string()),
            dim: None,
            quant: Some(c.quant.to_string()),
        },
    )
}

/// Stream one pinned file to disk, resuming a `.part` if one is there.
///
/// Returns the number of bytes that actually crossed the network, so the caller
/// can report a resumed install honestly rather than claiming a full download.
fn download_resumable(
    client: &reqwest::blocking::Client,
    dir: &Path,
    r: &PinnedRemote,
    url: &str,
    cancel: &AtomicBool,
    emit: &dyn Fn(GenInstallEvent),
) -> Result<u64, GaplyError> {
    let part = dir.join(format!("{}.part", r.local_name()));
    let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    // A .part longer than the pinned length is not a resume point, it is
    // garbage from a different file. Start over rather than seeking into it.
    let have = if r.bytes > 0 && have >= r.bytes { 0 } else { have };
    if have == 0 {
        let _ = std::fs::remove_file(&part);
    }

    emit(GenInstallEvent::Downloading {
        file: r.local_name().to_string(),
        from_bytes: have,
        total_bytes: r.bytes,
    });

    let mut req = client.get(url);
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let resp = req
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| GaplyError::Internal(format!("downloading {}: {e}", r.local_name())))?;

    // If a resume was requested and the server ignored it, the response body is
    // the WHOLE file — appending it would concatenate two copies. Detect that by
    // the status code and restart the file instead.
    let resuming = have > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut written = if resuming { have } else { 0 };
    let mut f = if resuming {
        let mut f = std::fs::OpenOptions::new().write(true).open(&part)?;
        f.seek(SeekFrom::Start(have))?;
        f
    } else {
        std::fs::File::create(&part)?
    };

    let mut body = resp;
    let mut buf = vec![0u8; 1 << 20];
    let mut downloaded = 0u64;
    let mut last_emit = 0u64;
    loop {
        // Polled PER CHUNK. One file is the whole download here, so polling
        // between files would make cancel a no-op for the entire install.
        if cancel.load(Ordering::SeqCst) {
            f.flush()?;
            // The .part is deliberately KEPT — that is the resume point.
            emit(GenInstallEvent::Cancelled);
            return Err(GaplyError::Cancelled);
        }
        let n = body
            .read(&mut buf)
            .map_err(|e| GaplyError::Internal(format!("reading {}: {e}", r.local_name())))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n])?;
        written += n as u64;
        downloaded += n as u64;
        if written - last_emit >= 8 << 20 {
            last_emit = written;
            emit(GenInstallEvent::Progress {
                file: r.local_name().to_string(),
                bytes: written,
                total_bytes: r.bytes,
            });
        }
    }
    f.flush()?;
    drop(f);

    // Verify the COMPLETE file from disk before it is given the real name.
    // Hashing the stream would hash only the bytes this run happened to fetch,
    // which for a resumed download is not the file being verified.
    emit(GenInstallEvent::Verifying { file: r.local_name().to_string() });
    let got = sha256_file(&part)?;
    if got != r.sha256 {
        // A bad .part must not survive as a resume point for the next attempt.
        let _ = std::fs::remove_file(&part);
        return Err(GaplyError::Validation(format!(
            "{} failed verification after download — expected sha256 {}, got {}. \
             Nothing was installed.",
            r.local_name(),
            r.sha256,
            got
        )));
    }
    std::fs::rename(&part, dir.join(r.local_name()))?;
    emit(GenInstallEvent::Verified { file: r.local_name().to_string() });
    Ok(downloaded)
}

/// Download (only what is missing or mismatched), verify, and register.
///
/// Blocking; call from `spawn_blocking`. Runs ONLY from the explicit
/// `ai_model_install_generative` command — never at startup.
pub fn install(
    db: &gaply_core::Database,
    app_data: &Path,
    c: &GenCandidate,
    cancel: &AtomicBool,
    emit: &dyn Fn(GenInstallEvent),
) -> Result<GenInstallReport, GaplyError> {
    let dir = model_dir(app_data, c);
    std::fs::create_dir_all(&dir)?;

    // OFFLINE PATH FIRST — no HTTP client is constructed at all. This is the
    // air-gapped install: files placed by hand, hashes matched, row written.
    if all_files_verified(&dir, c) {
        register(db, &dir, c)?;
        for r in c.files() {
            emit(GenInstallEvent::AlreadyPresent { file: r.local_name().to_string() });
        }
        emit(GenInstallEvent::Done {
            model_id: c.registry_id.to_string(),
            dir: dir.display().to_string(),
        });
        return Ok(GenInstallReport {
            model_id: c.registry_id.to_string(),
            dir: dir.display().to_string(),
            offline: true,
            bytes_downloaded: 0,
        });
    }

    emit(GenInstallEvent::Started {
        model_id: c.registry_id.to_string(),
        files: c.files().len(),
        total_bytes: c.total_bytes(),
    });

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("Gaply/", env!("CARGO_PKG_VERSION"), " (model-install)"))
        // A generous CEILING, not an expectation: 2 GB on a slow line is a long
        // job. Resume is what makes this safe to bound at all — a download the
        // timeout kills continues from its `.part` on the next attempt instead
        // of starting over, which is the whole reason resume exists here.
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(3 * 3600))
        .https_only(true)
        .build()
        .map_err(|e| GaplyError::Internal(format!("http client build failed: {e}")))?;

    let mut bytes_downloaded = 0u64;
    for r in c.files() {
        if cancel.load(Ordering::SeqCst) {
            emit(GenInstallEvent::Cancelled);
            return Err(GaplyError::Cancelled);
        }
        if file_verified(&dir, r) {
            emit(GenInstallEvent::AlreadyPresent { file: r.local_name().to_string() });
            continue;
        }
        bytes_downloaded += download_resumable(&client, &dir, r, &r.url(), cancel, emit)?;
    }

    verify_install(&dir, c)?;
    register(db, &dir, c)?;
    emit(GenInstallEvent::Done {
        model_id: c.registry_id.to_string(),
        dir: dir.display().to_string(),
    });
    Ok(GenInstallReport {
        model_id: c.registry_id.to_string(),
        dir: dir.display().to_string(),
        offline: false,
        bytes_downloaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(format!("gaply-gi-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn every_candidate_is_pinned_to_a_commit_with_a_real_hash() {
        assert!(!CANDIDATES.is_empty());
        for c in CANDIDATES {
            for r in c.files() {
                assert_eq!(
                    r.revision.len(),
                    40,
                    "{} pins a branch, not a commit: {}",
                    c.registry_id,
                    r.revision
                );
                assert!(
                    r.revision.chars().all(|ch| ch.is_ascii_hexdigit()),
                    "{} revision is not a sha",
                    c.registry_id
                );
                assert_eq!(r.sha256.len(), 64, "{} / {} has no measured hash", c.registry_id, r.file);
                assert!(
                    r.sha256.chars().all(|ch| ch.is_ascii_hexdigit()),
                    "{} / {} hash is not hex",
                    c.registry_id,
                    r.file
                );
            }
            assert!(c.gguf.bytes > 0, "{} has no pinned length", c.registry_id);
        }
    }

    #[test]
    fn urls_are_https_and_name_the_pinned_revision() {
        for c in CANDIDATES {
            for r in c.files() {
                let u = r.url();
                assert!(u.starts_with("https://"), "{u}");
                assert!(u.contains(r.revision), "{u} does not pin its revision");
                assert!(!u.contains("/main/"), "{u} points at a moving branch");
            }
        }
    }

    #[test]
    fn candidate_lookup_is_by_registry_id() {
        assert!(candidate("qwen2.5-1.5b-instruct-q4km").is_some());
        assert!(candidate("nope").is_none());
    }

    #[test]
    fn a_short_file_is_rejected_before_its_hash_is_read() {
        let dir = tmp("short");
        let c = &CANDIDATES[0];
        std::fs::write(dir.join(c.gguf.local_name()), b"too short").unwrap();
        assert!(!file_verified(&dir, &c.gguf));
        let err = verify_install(&dir, c).unwrap_err();
        assert_eq!(err.code(), "validation");
        assert!(err.to_string().contains("bytes, expected"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_install_is_not_found_not_a_hash_failure() {
        let dir = tmp("missing");
        let c = &CANDIDATES[0];
        assert!(!all_files_verified(&dir, c));
        assert_eq!(verify_install(&dir, c).unwrap_err().code(), "not_found");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The offline path must be reachable WITHOUT the pinned length matching a
    /// real download, so this uses a candidate-shaped fixture with hashes
    /// measured from the bytes written here.
    #[test]
    fn a_correct_hash_verifies_and_a_wrong_one_does_not() {
        let dir = tmp("hash");
        let body = b"pretend weights";
        let p = dir.join("f.gguf");
        std::fs::write(&p, body).unwrap();
        let real = sha256_file(&p).unwrap();

        let good = PinnedRemote {
            repo: "x/y",
            revision: "0".repeat(40).leak(),
            file: "f.gguf",
            sha256: real.clone().leak(),
            bytes: body.len() as u64,
        };
        assert!(file_verified(&dir, &good));

        let bad = PinnedRemote { sha256: &"a".repeat(64).leak()[..], ..good };
        assert!(!file_verified(&dir, &bad), "a wrong hash must not verify");
        let _ = std::fs::remove_dir_all(&dir);
    }


    /* ---------------------------------------------------------------------
     * A minimal HTTP server, because resume is the one behaviour here that
     * cannot be tested without a server that honours (or refuses) `Range`.
     * ------------------------------------------------------------------ */

    use std::net::TcpListener;

    /// Serves `body` once per connection. `honour_range` false makes it reply
    /// 200 with the WHOLE body even when a Range was asked for — the real
    /// misbehaviour that would otherwise concatenate two copies of a file.
    fn serve(body: Vec<u8>, honour_range: bool) -> (String, std::thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/f", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            for stream in l.incoming().take(1) {
                let mut st = match stream {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let mut buf = [0u8; 2048];
                let n = st.read(&mut buf).unwrap_or(0);
                // Lowercased: hyper writes header names lowercase, so a
                // case-sensitive match here silently never sees the Range and
                // the test would "pass" against a server that ignores it.
                let req = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                let start = req
                    .lines()
                    .find_map(|l| l.strip_prefix("range: bytes="))
                    .and_then(|v| v.split('-').next().and_then(|s| s.trim().parse::<usize>().ok()));
                let (code, slice) = match start {
                    Some(fromb) if honour_range && fromb < body.len() => {
                        ("206 Partial Content", &body[fromb..])
                    }
                    _ => ("200 OK", &body[..]),
                };
                let head = format!(
                    "HTTP/1.1 {code}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\r\n",
                    slice.len()
                );
                let _ = st.write_all(head.as_bytes());
                let _ = st.write_all(slice);
                let _ = st.flush();
            }
        });
        (url, h)
    }

    fn client() -> reqwest::blocking::Client {
        // https_only is deliberately OFF here and ON in production; the test
        // server is plaintext loopback.
        reqwest::blocking::Client::builder().build().unwrap()
    }

    fn remote_for(body: &[u8]) -> PinnedRemote {
        let mut h = Sha256::new();
        h.update(body);
        PinnedRemote {
            repo: "test/repo",
            revision: "0".repeat(40).leak(),
            file: "f.gguf",
            sha256: format!("{:x}", h.finalize()).leak(),
            bytes: body.len() as u64,
        }
    }

    #[test]
    fn a_partial_file_resumes_and_only_the_missing_bytes_cross_the_network() {
        let dir = tmp("resume");
        let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let r = remote_for(&body);

        // Half a file already on disk, exactly as an interrupted run leaves it.
        let have = 120_000usize;
        std::fs::write(dir.join("f.gguf.part"), &body[..have]).unwrap();

        let (url, h) = serve(body.clone(), true);
        let downloaded = download_resumable(
            &client(),
            &dir,
            &r,
            &url,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        h.join().unwrap();

        assert_eq!(
            downloaded,
            (body.len() - have) as u64,
            "a resume must fetch only the remainder, not the whole file"
        );
        assert_eq!(std::fs::read(dir.join("f.gguf")).unwrap(), body, "the joined file is wrong");
        assert!(!dir.join("f.gguf.part").exists(), "the .part should be renamed away");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_server_that_ignores_range_restarts_instead_of_concatenating() {
        // The dangerous case: we ask for bytes 120000- and get the whole file
        // back with a 200. Appending it would produce a file that is too long
        // and whose hash fails for a reason nobody could diagnose.
        let dir = tmp("norange");
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 97) as u8).collect();
        let r = remote_for(&body);
        std::fs::write(dir.join("f.gguf.part"), &body[..120_000]).unwrap();

        let (url, h) = serve(body.clone(), false);
        download_resumable(&client(), &dir, &r, &url, &AtomicBool::new(false), &|_| {}).unwrap();
        h.join().unwrap();

        assert_eq!(std::fs::read(dir.join("f.gguf")).unwrap(), body);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hash_mismatch_after_download_is_refused_and_the_bad_part_is_removed() {
        let dir = tmp("mismatch");
        let body = vec![1u8; 50_000];
        // Pin the hash of DIFFERENT bytes, so what arrives cannot verify.
        let r = remote_for(&vec![2u8; 50_000]);

        let (url, h) = serve(body, true);
        let err = download_resumable(&client(), &dir, &r, &url, &AtomicBool::new(false), &|_| {})
            .unwrap_err();
        h.join().unwrap();

        assert_eq!(err.code(), "validation");
        assert!(err.to_string().contains("failed verification"), "{err}");
        assert!(!dir.join("f.gguf").exists(), "an unverified file must never get the real name");
        assert!(
            !dir.join("f.gguf.part").exists(),
            "a corrupt .part must not survive as a resume point for the next attempt"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_part_longer_than_the_pinned_length_is_discarded_not_resumed_from() {
        let dir = tmp("toolong");
        let body: Vec<u8> = (0..80_000u32).map(|i| (i % 13) as u8).collect();
        let r = remote_for(&body);
        // Garbage from some other download, longer than this file.
        std::fs::write(dir.join("f.gguf.part"), vec![9u8; 200_000]).unwrap();

        let (url, h) = serve(body.clone(), true);
        let downloaded =
            download_resumable(&client(), &dir, &r, &url, &AtomicBool::new(false), &|_| {}).unwrap();
        h.join().unwrap();

        assert_eq!(downloaded, body.len() as u64, "it must have started over");
        assert_eq!(std::fs::read(dir.join("f.gguf")).unwrap(), body);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancelling_keeps_the_part_file_so_the_next_run_resumes() {
        let dir = tmp("cancel");
        let body = vec![5u8; 400_000];
        let r = remote_for(&body);
        let (url, h) = serve(body, true);

        // Already cancelled: the loop must stop at its first check.
        let cancel = AtomicBool::new(true);
        let err =
            download_resumable(&client(), &dir, &r, &url, &cancel, &|_| {}).unwrap_err();
        let _ = h.join();

        assert_eq!(err.code(), "cancelled");
        assert!(
            dir.join("f.gguf.part").exists(),
            "cancel must leave the .part behind — that is the resume point"
        );
        assert!(!dir.join("f.gguf").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }


    /// The generative half of `startup_performs_no_network_call`.
    ///
    /// Asserted STRUCTURALLY, like the embedding one: the download lives behind
    /// `install`, and the two things startup is allowed to do — resolve a loader
    /// and verify hashes — are in modules that must never name the network.
    #[test]
    fn nothing_on_the_startup_path_can_reach_the_downloader() {
        // 1. The loader that startup uses must not mention the network at all.
        let gen_src = include_str!("generative.rs");
        let gen_body = gen_src.split("mod tests").next().unwrap();
        for forbidden in ["reqwest", "TcpStream", "http://", "https://", "gen_install::install"] {
            assert!(
                !gen_body.contains(forbidden),
                "the generative loader references {forbidden:?} — loading a model must never \
                 acquire one"
            );
        }

        // 2. In THIS module the network is reachable only from `install` and its
        //    helper. Verification and resolution must be usable without it —
        //    that is what makes the offline/air-gapped path real.
        let me = include_str!("gen_install.rs");
        let body = me.split("mod tests").next().unwrap();
        let client_builds = body.matches("Client::builder").count();
        assert_eq!(
            client_builds, 1,
            "an HTTP client is constructed in {client_builds} places; there must be exactly one, \
             inside install()"
        );
        let before_install = body.split("pub fn install(").next().unwrap();
        assert!(
            !before_install.contains("Client::builder"),
            "an HTTP client is constructed before install() — verification must not need one"
        );

        // 3. A directory with no model resolves to nothing, without a network
        //    call, rather than trying to fetch one.
        let dir = tmp("startup");
        for c in CANDIDATES {
            assert!(!all_files_verified(&dir, c));
            assert!(
                crate::ai::generative::InstalledGenerativeLoader::resolve(&dir, c.registry_id)
                    .is_none(),
                "{} resolved from an empty directory",
                c.registry_id
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The OFFLINE MANUAL path: files placed by hand, hashes match, the model
    /// registers with no network. Uses a fixture candidate because the real
    /// candidates are gigabytes.
    #[test]
    fn files_present_with_matching_hashes_register_without_any_network() {
        let dir = tmp("offline");
        let body = b"pretend gguf bytes";
        let tok = b"pretend tokenizer";
        std::fs::write(dir.join("m.gguf"), body).unwrap();
        std::fs::write(dir.join("tokenizer.json"), tok).unwrap();
        let sha = |b: &[u8]| {
            let mut h = Sha256::new();
            h.update(b);
            format!("{:x}", h.finalize())
        };
        let c = GenCandidate {
            registry_id: "fixture-model",
            display_name: "Fixture",
            quant: "Q4_K_M",
            params_b: 0.1,
            needs_16gb: false,
            gguf: PinnedRemote {
                repo: "x/y",
                revision: "0".repeat(40).leak(),
                file: "m.gguf",
                sha256: sha(body).leak(),
                bytes: body.len() as u64,
            },
            tokenizer: PinnedRemote {
                repo: "x/y",
                revision: "0".repeat(40).leak(),
                file: "tokenizer.json",
                sha256: sha(tok).leak(),
                bytes: tok.len() as u64,
            },
        };

        assert!(all_files_verified(&dir, &c));
        verify_install(&dir, &c).expect("a hand-placed, hash-matching install must verify");

        let db = gaply_core::Database::in_memory().unwrap();
        register(&db, &dir, &c).unwrap();
        let row = gaply_core::ai_engine::registry::get_model(&db, "fixture-model")
            .unwrap()
            .expect("the model must be registered");
        assert_eq!(row.kind, "generative");
        assert_eq!(row.quant.as_deref(), Some("Q4_K_M"));
        assert!(
            row.file_path.ends_with("m.gguf"),
            "the registry must point at the GGUF itself, not its directory: {}",
            row.file_path
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_of_a_large_file_is_chunked_not_slurped() {
        // 5 MB — bigger than the 1 MB read buffer, so the chunk loop is exercised.
        let dir = tmp("big");
        let p = dir.join("big.bin");
        let block = vec![7u8; 1 << 20];
        {
            let mut f = std::fs::File::create(&p).unwrap();
            for _ in 0..5 {
                f.write_all(&block).unwrap();
            }
        }
        let mut h = Sha256::new();
        for _ in 0..5 {
            h.update(&block);
        }
        assert_eq!(sha256_file(&p).unwrap(), format!("{:x}", h.finalize()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
