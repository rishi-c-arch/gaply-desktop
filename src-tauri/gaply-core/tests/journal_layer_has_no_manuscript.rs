//! **No manuscript text touches the journal layer.**
//!
//! Prompt 5's discipline: *"No manuscript text touches this phase. Assert it:
//! the journal module's dependency graph must not reach the manuscript
//! layer."* §3.4 makes the same claim from the other side — journal profiles
//! are *"shared and cached … none of it touches any user's manuscript"* — and
//! that is what lets a fingerprint be built once per journal per quarter and
//! served to every user. **A journal fact that depended on one user's
//! manuscript could not be shared**, so this is a privacy property and a
//! caching property at the same time, not a tidiness rule.
//!
//! # THE THIRD GUARD OF ITS KIND, AND EACH BANS SOMETHING DIFFERENT
//!
//! The differences matter more than the similarity, because a reader who
//! assumes the three are copies will extend the wrong list:
//!
//! | guard | bans | MUST allow |
//! |---|---|---|
//! | `equation_is_llm_free` | models, proxy, network, **database, filesystem, clock, entropy** | `std` + `serde` only — a Tier-0 result must be reproducible from its inputs alone |
//! | `journal_display_is_llm_free` | models, proxy, network | **the database and the clock** — reading stored rows with a `fetched_at` is the job |
//! | **this one** | **the manuscript layer** | **the database, the clock, AND the network** — the crawl fetches, and `refverify`/`sanitize` are the guarded way it does so |
//!
//! So this guard is about DATA PROVENANCE rather than purity: the journal
//! layer may reach the open internet and may write to disk; what it may not do
//! is read the user's document.
//!
//! # What it cannot catch, stated because the gaps are real
//!
//! 1. **The legitimate join.** `report::checklist_from_requirements` takes an
//!    `ExtractionResult` AND stored requirements, because comparing a
//!    manuscript against a journal is the product. That comparison lives in
//!    `report.rs`, which is NOT scanned here — the rule is that the journal
//!    modules do not reach the manuscript, not that nothing ever compares them.
//!    A future violation could therefore hide by moving code into `report.rs`,
//!    and no scan sees that.
//! 2. **Indirection.** A manuscript type reached through a helper in an
//!    allowed module is invisible; this reads `use` lines and `crate::` paths,
//!    not a call graph.
//! 3. **Data, not types.** Nothing stops a caller passing manuscript TEXT into
//!    `extract_requirements` as a `GuidelineBlock` — the parameter is a string.
//!    The type system does not distinguish the two strings, and this test does
//!    not either. What it guarantees is that the journal modules cannot
//!    *obtain* manuscript data themselves.
//! 4. **The app-crate half.** `src/journal_crawl.rs` lives in the app crate and
//!    is not readable from here; its imports are `crate::guidelines`,
//!    `gaply_core::ratelimit` and `gaply_core::refverify`, checked by hand on
//!    14 Sep 2026 and unguarded.

use std::path::Path;

/// The Phase-3 fingerprint lane.
const SCANNED_GLOB_PREFIX: &str = "journal_";

/// Modules that hold, derive from, or report on a user's manuscript.
///
/// Each entry is a module name as it appears in a `use crate::…` path or a
/// `crate::…::` reference.
const MANUSCRIPT_LAYER: &[(&str, &str)] = &[
    ("extract", "the manuscript extraction layer"),
    ("research_state", "the derived manuscript layer"),
    ("validate", "manuscript statistical validation"),
    ("stats_verify", "manuscript statistics"),
    ("stats_verdict", "manuscript statistics"),
    ("equation", "manuscript equations"),
    ("ai_detect", "manuscript AI detection"),
    ("plagiarism", "manuscript plagiarism checking"),
    ("report", "the manuscript report"),
    ("report_compose", "the manuscript report"),
    ("report_pdf", "the manuscript report"),
    ("reviewer_agent", "the manuscript reviewer"),
    ("swarm", "the manuscript debate runtime"),
    ("audit_prepass", "a manuscript pass"),
    ("chunk", "manuscript chunking"),
    ("rag", "the manuscript-bearing corpus"),
    ("manuscript", "the manuscript"),
];

fn journal_modules() -> Vec<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|x| x == "rs").unwrap_or(false)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(SCANNED_GLOB_PREFIX))
                    .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}

/// Strip comments.
///
/// **Load-bearing, and the reason is a real near-miss.**
/// `journal_standards.rs` contains the line
/// `//! [`ResearchState`]: crate::research_state::ResearchState` — a rustdoc
/// intra-doc link in a module header explaining why an unbound standard cannot
/// select an evaluator. A scan that could not tell prose from code would fire
/// on a module documenting the very boundary it respects, and the obvious
/// "fix" would be to delete the explanation.
fn code_only(src: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in src.lines() {
        let t = line.trim_start();
        if in_block {
            if let Some(rest) = t.find("*/") {
                in_block = false;
                out.push_str(&t[rest + 2..]);
                out.push('\n');
            }
            continue;
        }
        if t.starts_with("//") {
            continue;
        }
        if t.starts_with("/*") {
            in_block = !t.contains("*/");
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn the_journal_layer_cannot_reach_the_manuscript_layer() {
    let modules = journal_modules();
    assert!(modules.len() >= 6, "only {} journal modules found — has the layer moved?", modules.len());

    for path in modules {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let code = code_only(&std::fs::read_to_string(&path).unwrap());
        for (module, what) in MANUSCRIPT_LAYER {
            for route in [format!("crate::{module}::"), format!("use crate::{module};")] {
                assert!(
                    !code.contains(&route),
                    "{name} reaches {what} (`{route}`). A journal fingerprint is built once \
                     per journal and served to every user; a journal fact that depended on \
                     one user's manuscript could not be shared, and would not be private."
                );
            }
        }
    }
}

/// The scan must be reading real code, or it passes on a rename.
#[test]
fn the_guard_is_reading_the_modules_it_names() {
    let modules = journal_modules();
    let names: Vec<String> =
        modules.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
    for expected in ["journal_extract.rs", "journal_store.rs", "journal_fingerprint.rs"] {
        assert!(names.iter().any(|n| n == expected), "{expected} missing from {names:?}");
    }
    for p in &modules {
        let src = std::fs::read_to_string(p).unwrap();
        assert!(src.len() > 500, "{} is {} bytes — too small to be a module", p.display(), src.len());
    }
}

/// **The comment-stripping is itself pinned**, because without it the guard
/// fires on a module that documents the boundary it respects — and the
/// tempting fix is to delete the documentation.
#[test]
fn a_manuscript_module_named_only_in_prose_is_not_a_dependency() {
    let src = "//! See [`ResearchState`]: crate::research_state::ResearchState\n\
               /* crate::extract:: in a block comment */\n\
               use crate::journal_extract::RequirementKind;\n";
    let code = code_only(src);
    assert!(!code.contains("crate::research_state::"), "doc link survived: {code:?}");
    assert!(!code.contains("crate::extract::"), "block comment survived: {code:?}");
    assert!(code.contains("crate::journal_extract::"), "real code was stripped: {code:?}");
}
