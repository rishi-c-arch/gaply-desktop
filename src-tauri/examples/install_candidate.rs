//! Install a pinned generative candidate from files you already have, OFFLINE.
//!
//! # Why this exists
//!
//! The Settings install button is hard-wired to one candidate
//! (`qwen2.5-1.5b-instruct-q4km`); `ai_generative_candidates` is exposed but no
//! surface renders a picker. So there is currently no way in the product to
//! install any other pinned candidate, and after §11 D48 the choice of judge is
//! exactly "which generative model is installed and most recently registered".
//! This is that missing lane, and deliberately a CLI: it is a deployment
//! action, not something a user should be able to do by mis-clicking.
//!
//! # It does NOT trust the files
//!
//! Copying weights into place is not installing them. This stages the files and
//! then hands off to [`gen_install::install`], whose OFFLINE path re-reads every
//! byte from disk and checks it against the compiled-in sha256 before writing
//! the `ai_model_registry` row. A mismatch registers NOTHING and exits non-zero.
//! Nothing here touches the network — install()'s offline branch constructs no
//! HTTP client at all.
//!
//! Usage:
//!   cargo run --example install_candidate -- \
//!     --id qwen2.5-3b-instruct-q4km \
//!     --from ~/gaply-models/qwen2.5-3b-instruct-q4km
//!
//! `--app-data` defaults to the macOS app-data directory the desktop app uses.
//! `--dry-run` stages nothing and only reports what would happen.
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use app_lib::ai::gen_install::{self, GenInstallEvent};
use gaply_core::{Database, GaplyError};

fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args().skip_while(|a| a != name);
    it.next()?;
    it.next()
}

fn default_app_data() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Application Support/ai.gaply.app")
}

/// `~` is not expanded by the shell inside a quoted arg, and silently creating
/// a literal `./~` directory is the kind of thing that wastes an afternoon.
fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(p),
    }
}

fn main() -> Result<(), GaplyError> {
    let id = arg("--id").expect("usage: --id <registry-id> --from <dir> [--app-data <dir>] [--dry-run]");
    let from = expand(&arg("--from").expect("--from <dir> holding the candidate's files"));
    let app_data = arg("--app-data").map(|p| expand(&p)).unwrap_or_else(default_app_data);
    let dry_run = std::env::args().any(|a| a == "--dry-run");

    let c = gen_install::candidate(&id).unwrap_or_else(|| {
        eprintln!("'{id}' is not a pinned candidate. Known ids:");
        for k in gen_install::CANDIDATES {
            eprintln!("  {} — {}", k.registry_id, k.display_name);
        }
        std::process::exit(2);
    });

    let db_path = app_data.join("gaply.db");
    if !db_path.is_file() {
        eprintln!("no database at {} — is --app-data right?", db_path.display());
        std::process::exit(2);
    }
    let dir = gen_install::model_dir(&app_data, c);
    println!("candidate : {} ({})", c.registry_id, c.display_name);
    println!("source    : {}", from.display());
    println!("target    : {}", dir.display());
    println!("database  : {}\n", db_path.display());

    // Stage: copy only what is missing or the wrong size. A file already in
    // place is left alone — install() is about to hash it either way, and
    // re-copying two gigabytes to prove nothing is just slower.
    std::fs::create_dir_all(&dir)?;
    for r in c.files() {
        let src = from.join(r.local_name());
        let dst = dir.join(r.local_name());
        let have = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        if have == r.bytes {
            println!("present   {} ({} bytes)", r.local_name(), have);
            continue;
        }
        if !src.is_file() {
            eprintln!("MISSING   {} — expected at {}", r.local_name(), src.display());
            std::process::exit(2);
        }
        let src_len = std::fs::metadata(&src)?.len();
        if src_len != r.bytes {
            // Refuse early rather than copy 2 GB and fail the hash afterwards.
            eprintln!(
                "SIZE MISMATCH {}: source is {src_len} bytes, the pin says {}",
                r.local_name(),
                r.bytes
            );
            std::process::exit(1);
        }
        if dry_run {
            println!("would copy {} ({src_len} bytes)", r.local_name());
            continue;
        }
        println!("copying   {} ({src_len} bytes)…", r.local_name());
        std::fs::copy(&src, &dst)?;
    }

    if dry_run {
        println!("\n--dry-run: nothing staged, nothing verified, nothing registered.");
        return Ok(());
    }

    // The installer owns verification and registration. Everything above this
    // line only moved bytes; nothing is trusted until install() says so.
    println!("\nverifying against the compiled-in sha256 (reads every byte)…");
    let db = Database::open(&db_path)?;
    let cancel = AtomicBool::new(false);
    let report = gen_install::install(&db, &app_data, c, &cancel, &|ev| match ev {
        GenInstallEvent::AlreadyPresent { file } => println!("  verified  {file}"),
        GenInstallEvent::Verifying { file } => println!("  hashing   {file}"),
        GenInstallEvent::Verified { file } => println!("  verified  {file}"),
        GenInstallEvent::Done { model_id, .. } => println!("  registered {model_id}"),
        other => println!("  {other:?}"),
    })?;

    println!(
        "\nOK — {} registered from {} (offline: {}, bytes downloaded: {})",
        report.model_id, report.dir, report.offline, report.bytes_downloaded
    );
    if !report.offline {
        eprintln!("WARNING: this went to the network. The offline path did not match.");
    }

    // What the app will do with it, stated rather than assumed (§11 D48).
    // estimate_ram takes the GGUF, not the directory it lives in.
    match app_lib::ai::generative::estimate_ram(&Path::new(&report.dir).join(c.gguf.local_name())) {
        Ok(e) => println!(
            "RAM estimate: {:.2} GB total ({:.2} GB weights + {:.2} GB KV at {} ctx)",
            e.total_bytes as f64 / 1e9,
            e.weights_bytes as f64 / 1e9,
            e.kv_cache_bytes as f64 / 1e9,
            e.context_length
        ),
        Err(e) => println!("RAM estimate unavailable: {e}"),
    }
    println!("Relaunch the app: the registry is read at startup, newest-first.");
    Ok(())
}
