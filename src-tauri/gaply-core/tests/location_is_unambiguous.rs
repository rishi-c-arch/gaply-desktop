//! **No PRODUCTION path may construct an ambiguous `extract::Location`.**
//!
//! # Why this is a test and not "every producer uses `in_section` today"
//!
//! §11 D169 replaced `Location { kind, paragraph }` with one that also carries
//! `section_index`, because a `SectionKind` is not a key: over 20 real
//! manuscripts, **283 of 869 statistical claims resolved to a paragraph that
//! did not contain them**, and those claims are the inputs to `validate.rs`'s
//! five Tier-0 rules, which `swarm.rs` treats as `hard_constraint` — never voted
//! on, always overriding every model.
//!
//! Fixing the producers made the defect zero **today**. Nothing made it stay
//! zero. `Location::by_kind` and a `section_index: None` literal both still
//! compile, both look entirely ordinary in review, and the next producer added
//! has no reason to know any of the above — the failure is silent, the wrong
//! paragraph is displayed beside the wrong finding, and the two agree. That is
//! exactly how D169 hid for as long as it did.
//!
//! **This guard was written immediately after the fix and FAILED on its first
//! run, naming eight production sites** — `claims.rs`, `novelty.rs`,
//! `frequentist.rs`, `claim_strength.rs` (x2), `ml.rs`, `report.rs`'s
//! `section_check` and `extract::locate_line`. The claim it was checking was
//! *"every producer now uses `in_section`"*, which had been stated in a commit
//! message an hour earlier and was true only of the extraction producers. A
//! guard that fails on its first run is the guard working: predict the failure,
//! and if it passes immediately, distrust it.
//!
//! # What it cannot catch, stated because the gap is real
//!
//! 1. **A `Location` deserialized from old data**, which legitimately carries
//!    `None`. That is the compatibility path D169 designed for, and it is
//!    invisible to a source scan.
//! 2. **An index that is simply WRONG** — `in_section(kind, 0, p)` inside a
//!    loop over section 4. `paragraph_at` verifies the KIND at the index, which
//!    catches the common shape of that mistake at runtime, not here.
//! 3. **A helper in another module that returns an ambiguous location.** The
//!    scan sees the construction site, which is where the decision is made.
//!
//! # It failed on Windows before it failed on anything real
//!
//! The first CI run went red on `windows-build-check` with BOTH allowlist
//! entries reported as violations. The scan builds a path from `read_dir`,
//! which on Windows yields `src\extract\mod.rs`, while `ALLOWED` is written
//! with `/` — so `ends_with` matched nothing and every exemption evaporated.
//! macOS passed, and so did `verify-clean-checkout.sh`, because both use `/`.
//!
//! Worth keeping because the failure mode is the guard's own: an allowlist that
//! silently stops matching does not loosen a check, it tightens it into noise,
//! and the reverse — an allowlist that silently matches too much — would have
//! been invisible. Paths are compared on a normalised string now.

use std::path::Path;

/// The two ways to build a `Location` that cannot name its section.
const AMBIGUOUS: &[&str] = &["Location::by_kind", "section_index: None"];

/// Lines that MAY construct an ambiguous location, each with the reason it is
/// allowed. A file-level exemption would let a whole module drift; these are
/// pinned to an exact line of code so a new one has to be argued for here.
///
/// Format: `(file, exact trimmed line, why)`.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "src/extract/mod.rs",
        "Self { section, paragraph, section_index: None }",
        "the body of `Location::by_kind` itself — the constructor has to be able \
         to build the thing it constructs",
    ),
    (
        "src/research_state.rs",
        "Location::by_kind(r.section, r.start_paragraph)",
        "`scientific_model::Span` carries no section index, so a Location derived \
         from a `SourceSpan::Range` cannot have one. NOTHING in the crate \
         constructs `Range` — every producer emits `Point(loc)` — so this arm is \
         defensive, not live. If that changes, widen `Span` FIRST",
    ),
];

/// Source roots that ship. Test files and examples are excluded by name below;
/// a fixture is allowed to be ambiguous, a product path is not.
const SCANNED_DIR: &str = "src";

fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has the crate moved?", dir.display());
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Everything from `#[cfg(test)]` to the end of the file, plus any `tests.rs`,
/// is fixture territory. A test fixture SHOULD be able to write an ambiguous
/// location — that is what `by_kind` is for.
fn production_lines(path: &Path, src: &str) -> Vec<(usize, String)> {
    if path.file_name().is_some_and(|n| n == "tests.rs") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        // Strip line comments: this file's own prose names both patterns, and
        // so do the doc comments on `by_kind` and on the allowed lines above.
        let code = match line.find("//") {
            Some(j) => &line[..j],
            None => line,
        };
        out.push((i + 1, code.to_string()));
    }
    out
}

#[test]
fn no_production_path_constructs_an_ambiguous_location() {
    let files = rust_files(Path::new(SCANNED_DIR));
    assert!(!files.is_empty(), "the scan found no files — it would pass vacuously");

    let mut violations = Vec::new();
    let mut allowed_hits = 0usize;

    for f in &files {
        let src = std::fs::read_to_string(f).expect("read");
        // NORMALISED. Windows yields `src\\extract\\mod.rs`; the ALLOWED entries
        // are written with `/`. Without this, `ends_with` misses on Windows and
        // every allowed line reads as a violation — which is exactly how this
        // guard failed its first CI run while passing on macOS and in the
        // clean-checkout worktree. A path comparison is a portability decision,
        // and a test that only runs on the author's platform is not a guard.
        let rel = f.to_string_lossy().replace('\\', "/");
        for (lineno, code) in production_lines(f, &src) {
            let Some(pat) = AMBIGUOUS.iter().find(|p| code.contains(**p)) else {
                continue;
            };
            let trimmed = code.trim();
            if ALLOWED.iter().any(|(file, line, _)| rel.ends_with(file) && trimmed.contains(line)) {
                allowed_hits += 1;
                continue;
            }
            violations.push(format!("{rel}:{lineno}  [{pat}]  {trimmed}"));
        }
    }

    assert!(
        violations.is_empty(),
        "a production path builds a Location that cannot name its section \
         (§11 D169 — a SectionKind is not a key, and 283 of 869 Tier-0 inputs \
         resolved to the wrong paragraph before this was fixed).\n\n\
         Use `Location::in_section(kind, section_index, paragraph)`. The index is \
         available wherever a Location is built: the construction always sits \
         inside a loop over sections, so `.iter().enumerate()` supplies it.\n\n\
         If it genuinely cannot — the enclosing type carries no index — add the \
         line to ALLOWED in this file WITH ITS REASON rather than deleting this \
         assertion.\n\n{}",
        violations.join("\n")
    );

    // NON-VACUITY, both halves. The scan must have looked at real code, and the
    // allowlist must still describe lines that exist — a stale entry silently
    // widens the guard.
    assert!(files.len() > 20, "only {} files scanned — the scan is not reaching the crate", files.len());
    assert_eq!(
        allowed_hits,
        ALLOWED.len(),
        "the allowlist has {} entries but matched {allowed_hits} line(s): an entry \
         no longer matches any code and must be removed, or one matches twice",
        ALLOWED.len()
    );
}
