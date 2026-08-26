//! The ONE network operation in the AI layer (R4).
//!
//! Downloads the pinned `bge-small-en-v1.5` files, verifies each against a
//! compiled-in sha256, and registers the model in `ai_model_registry`.
//!
//! # Rules this module exists to keep
//!
//! - It runs ONLY from the `ai_model_install` command, i.e. explicit user
//!   action. Nothing calls it at startup.
//! - It sends nothing. Three GETs to fixed URLs; no query string, no headers
//!   carrying identity, no body, no telemetry.
//! - An OFFLINE INSTALL is a first-class path, not a fallback: if the files are
//!   already in the app data dir and their hashes match, the model registers
//!   with no network call at all. That is what makes an air-gapped install
//!   possible, and it is why verification is separated from acquisition.
//!
//! # Pinning
//!
//! URLs name a REVISION, not `main`, so "pinned" means a fixed tree rather than
//! a moving branch. The sha256 constants were computed from the files actually
//! downloaded from that revision; a mismatch fails the install rather than
//! proceeding with unverified weights.

use std::path::{Path, PathBuf};

use gaply_core::GaplyError;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::ai::{EMBED_DIM, EMBED_MODEL_ID};

/// The exact revision the hashes below were taken from.
pub const MODEL_REVISION: &str = "5c38ec7c405ec4b44b94cc5a9bb96e735b38267a";

const BASE: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve";

/// One pinned file: name on disk, URL, expected sha256, expected byte length.
pub struct PinnedFile {
    pub name: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

/// The three files the engine needs. Hashes measured from the pinned revision.
pub const PINNED_FILES: &[PinnedFile] = &[
    PinnedFile {
        name: "config.json",
        sha256: "094f8e891b932f2000c92cfc663bac4c62069f5d8af5b5278c4306aef3084750",
        bytes: 743,
    },
    PinnedFile {
        name: "tokenizer.json",
        sha256: "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66",
        bytes: 711_396,
    },
    PinnedFile {
        name: "model.safetensors",
        sha256: "3c9f31665447c8911517620762200d2245a2518d6e7208acc78cd9db317e21ad",
        bytes: 133_466_304,
    },
];

/// Progress for the install command's Channel.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstallEvent {
    Started { files: usize, total_bytes: u64 },
    /// Emitted when a file was already present and verified — no bytes moved.
    AlreadyPresent { file: String },
    Downloading { file: String, bytes: u64 },
    Verified { file: String },
    Cancelled,
    Done { model_id: String, dir: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallReport {
    pub model_id: String,
    pub dir: String,
    /// True when every file was already present and verified — no network was
    /// used at all.
    pub offline: bool,
}

/// Where the model lives: `<app data>/models/bge-small-en-v1.5/`.
pub fn model_dir(app_data: &Path) -> PathBuf {
    app_data.join("models").join(EMBED_MODEL_ID)
}

fn sha256_file(path: &Path) -> Result<String, GaplyError> {
    let bytes = std::fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

/// Is this file present with the expected hash?
pub fn file_verified(dir: &Path, f: &PinnedFile) -> bool {
    let p = dir.join(f.name);
    p.is_file() && sha256_file(&p).map(|h| h == f.sha256).unwrap_or(false)
}

/// Are all three files present and verified? The offline-install check, and the
/// startup precondition — **this performs no network call.**
pub fn all_files_verified(dir: &Path) -> bool {
    PINNED_FILES.iter().all(|f| file_verified(dir, f))
}

/// Verify an already-present install, naming the first file that fails.
///
/// Separated from acquisition on purpose: startup calls THIS and never the
/// downloader.
pub fn verify_install(dir: &Path) -> Result<(), GaplyError> {
    for f in PINNED_FILES {
        let p = dir.join(f.name);
        if !p.is_file() {
            return Err(GaplyError::NotFound {
                entity: "model file",
                id: p.display().to_string(),
            });
        }
        let got = sha256_file(&p)?;
        if got != f.sha256 {
            return Err(GaplyError::Validation(format!(
                "{} failed verification — expected sha256 {}, found {}. The file is corrupt or \
                 not the pinned revision; re-run the model install.",
                f.name, f.sha256, got
            )));
        }
    }
    Ok(())
}

/// Record the verified model in `ai_model_registry`.
pub fn register(db: &gaply_core::Database, dir: &Path) -> Result<(), GaplyError> {
    gaply_core::ai_engine::registry::register_model(
        db,
        gaply_core::ai_engine::registry::ModelRow {
            id: EMBED_MODEL_ID.to_string(),
            kind: "embedding".to_string(),
            display_name: "BGE Small EN v1.5".to_string(),
            file_path: dir.display().to_string(),
            sha256: Some(
                PINNED_FILES
                    .iter()
                    .find(|f| f.name == "model.safetensors")
                    .expect("weights are pinned")
                    .sha256
                    .to_string(),
            ),
            dim: Some(EMBED_DIM as i64),
            quant: None,
        },
    )
}

/// Download (only what is missing or mismatched), verify, and register.
///
/// Blocking; call from `spawn_blocking`. `cancel` is polled between files, so
/// cancelling stops before the next request rather than mid-stream.
pub fn install(
    db: &gaply_core::Database,
    app_data: &Path,
    cancel: &std::sync::atomic::AtomicBool,
    emit: &dyn Fn(InstallEvent),
) -> Result<InstallReport, GaplyError> {
    use std::sync::atomic::Ordering;

    let dir = model_dir(app_data);
    std::fs::create_dir_all(&dir)?;

    // OFFLINE PATH FIRST. If everything is already here and verified, we never
    // construct an HTTP client at all.
    if all_files_verified(&dir) {
        register(db, &dir)?;
        for f in PINNED_FILES {
            emit(InstallEvent::AlreadyPresent { file: f.name.to_string() });
        }
        emit(InstallEvent::Done {
            model_id: EMBED_MODEL_ID.to_string(),
            dir: dir.display().to_string(),
        });
        return Ok(InstallReport {
            model_id: EMBED_MODEL_ID.to_string(),
            dir: dir.display().to_string(),
            offline: true,
        });
    }

    emit(InstallEvent::Started {
        files: PINNED_FILES.len(),
        total_bytes: PINNED_FILES.iter().map(|f| f.bytes).sum(),
    });

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("Gaply/", env!("CARGO_PKG_VERSION"), " (model-install)"))
        .timeout(std::time::Duration::from_secs(600))
        // The download is the one permitted network call; it is not permitted
        // to be downgraded to plaintext by a redirect.
        .https_only(true)
        .build()
        .map_err(|e| GaplyError::Internal(format!("http client build failed: {e}")))?;

    for f in PINNED_FILES {
        if cancel.load(Ordering::SeqCst) {
            emit(InstallEvent::Cancelled);
            return Err(GaplyError::Cancelled);
        }
        if file_verified(&dir, f) {
            emit(InstallEvent::AlreadyPresent { file: f.name.to_string() });
            continue;
        }
        emit(InstallEvent::Downloading { file: f.name.to_string(), bytes: f.bytes });
        let url = format!("{BASE}/{MODEL_REVISION}/{}", f.name);
        let bytes = client
            .get(&url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map_err(|e| GaplyError::Internal(format!("downloading {}: {e}", f.name)))?;

        // Verify BEFORE the bytes are given the real filename, so a failed
        // download can never be mistaken for a good install.
        let mut h = Sha256::new();
        h.update(&bytes);
        let got = format!("{:x}", h.finalize());
        if got != f.sha256 {
            return Err(GaplyError::Validation(format!(
                "{} failed verification after download — expected sha256 {}, got {}. \
                 Nothing was installed.",
                f.name, f.sha256, got
            )));
        }
        let tmp = dir.join(format!("{}.part", f.name));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, dir.join(f.name))?;
        emit(InstallEvent::Verified { file: f.name.to_string() });
    }

    verify_install(&dir)?;
    register(db, &dir)?;
    emit(InstallEvent::Done {
        model_id: EMBED_MODEL_ID.to_string(),
        dir: dir.display().to_string(),
    });
    Ok(InstallReport {
        model_id: EMBED_MODEL_ID.to_string(),
        dir: dir.display().to_string(),
        offline: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_metadata_is_internally_consistent() {
        assert_eq!(PINNED_FILES.len(), 3);
        for f in PINNED_FILES {
            assert_eq!(f.sha256.len(), 64, "{} has a malformed sha256", f.name);
            assert!(f.sha256.chars().all(|c| c.is_ascii_hexdigit()), "{}", f.name);
            assert!(f.bytes > 0);
        }
        assert_eq!(MODEL_REVISION.len(), 40, "revision must be a full commit sha, not a branch");
    }

    #[test]
    fn verification_of_a_missing_or_corrupt_install_fails_clearly() {
        let dir = std::env::temp_dir().join(format!("gaply-mi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // nothing present
        assert!(!all_files_verified(&dir));
        let err = verify_install(&dir).unwrap_err();
        assert_eq!(err.code(), "not_found");

        // present but wrong bytes → a validation error naming the file
        for f in PINNED_FILES {
            std::fs::write(dir.join(f.name), b"not the real weights").unwrap();
        }
        assert!(!all_files_verified(&dir));
        let err = verify_install(&dir).unwrap_err();
        assert_eq!(err.code(), "validation");
        assert!(err.to_string().contains("failed verification"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
