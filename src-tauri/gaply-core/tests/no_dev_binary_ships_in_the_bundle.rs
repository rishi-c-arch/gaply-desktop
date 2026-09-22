//! **A dev-only executable must not be able to reach a production bundle.**
//!
//! # The defect
//!
//! Measured 22 Sep 2026 (docs/PROBLEM_DOSSIER.md): the built app contained
//!
//! ```text
//! gaply.app/Contents/MacOS/app     128 MB
//! gaply.app/Contents/MacOS/grrb    3.7 MB   <- a benchmark runner
//! ```
//!
//! and `src/bin/grrb.rs` calls `std::process::Command::new("git")` — an
//! external process inside the shipped application. None of the five dev
//! binaries is named in `tauri.conf.json`; they were auto-discovered cargo bin
//! targets, so the bundler saw them.
//!
//! # What this guards, and what it cannot
//!
//! It checks the CONFIG INVARIANT: every `src/bin/*.rs` target must be declared
//! with a `required-features` that is not on by default, so a production build
//! never compiles it and the bundler has nothing to copy.
//!
//! **It does not inspect a bundle.** CI builds no `.app` — `windows-build-check`
//! skips the Tauri step because the bundled models are gitignored — so a test
//! that looked for `Contents/MacOS` would pass vacuously on every runner. The
//! bundle itself is checked by hand after a release build; this checks the
//! thing that DETERMINES the bundle, on every push.
//!
//! Lives in `gaply_core` because both CI workflows run `cargo test -p
//! gaply_core` and neither runs the app crate's tests.
//!
//! # ONE CONSEQUENCE OF THE GATE, measured, so nobody quotes the wrong total
//!
//! Gating the dev binaries also removes their TEST targets from the default
//! workspace run, because cargo does not build a target whose required
//! features are off:
//!
//! ```text
//! cargo test --workspace                      26 targets, 1911 passed
//! cargo test --workspace --features devtools  31 targets, 1917 passed
//! cargo test -p gaply_core                    1437 passed   (CI, unchanged)
//! ```
//!
//! The four missing targets carry `ai-eval`'s six diagnostic tests. **The full
//! local suite is now `cargo test --workspace --features devtools`.** CI is
//! unaffected — it runs `-p gaply_core`, which never included app-crate bins —
//! but a plain `--workspace` run is now a narrower question than it looks, which
//! is the `-p gaply_core` trap in CLAUDE.md one directory over.

use std::path::PathBuf;

fn app_manifest() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml");
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn bin_sources() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/bin");
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|x| x.to_str()) == Some("rs"))
                .then(|| p.file_stem()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_dev_binary_is_gated_behind_a_non_default_feature() {
    let manifest = app_manifest();
    let bins = bin_sources();

    // POSITIVE COUNT FIRST: if `src/bin` is empty or unreadable this test would
    // otherwise pass forever while guarding nothing.
    assert!(
        !bins.is_empty(),
        "no bin sources found — this guard has gone silently inert"
    );

    // The gating feature must exist and must NOT be a default.
    assert!(
        manifest.contains("devtools = []"),
        "the `devtools` feature is gone; dev binaries are no longer gated"
    );
    let default_line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("default ="))
        .unwrap_or("default = []");
    assert!(
        !default_line.contains("devtools"),
        "`devtools` is a DEFAULT feature, so dev binaries build into a \
         production bundle after all: {default_line}"
    );

    for bin in &bins {
        // Each must be declared, and its declaration must carry the gate.
        let decl = manifest
            .split("[[bin]]")
            .find(|block| block.contains(&format!("name = \"{bin}\"")))
            .unwrap_or_else(|| {
                panic!(
                    "bin target {bin:?} is auto-discovered and ungated — a \
                     production build will compile it and the bundler can copy \
                     it. Declare it in Cargo.toml with \
                     `required-features = [\"devtools\"]`."
                )
            });
        assert!(
            decl.contains("required-features") && decl.contains("devtools"),
            "bin target {bin:?} is declared without `required-features = \
             [\"devtools\"]`, so it builds in a production build: {decl}"
        );
    }
}

/// **The reason this matters, pinned to the specific hazard.** A dev binary that
/// shells out is the one that must never ship; if the gate above is ever
/// loosened, this says what was at stake.
#[test]
fn the_benchmark_binary_that_shells_out_is_gated() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/bin/grrb.rs");
    let body = std::fs::read_to_string(&src).expect("grrb.rs");
    assert!(
        body.contains("process::Command"),
        "grrb no longer shells out; if that is deliberate, this test has served \
         its purpose and the reasoning in its header should be updated rather \
         than the assertion relaxed"
    );
    let manifest = app_manifest();
    let decl = manifest
        .split("[[bin]]")
        .find(|b| b.contains("name = \"grrb\""))
        .expect("grrb must be a declared bin target");
    assert!(
        decl.contains("devtools"),
        "the binary that calls std::process::Command is not gated: {decl}"
    );
}
