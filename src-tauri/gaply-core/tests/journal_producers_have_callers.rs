//! **A journal producer that nothing in production calls is the same artefact as
//! a table nothing writes to.**
//!
//! # This is the sibling `journal_tables_have_writers.rs` said it could not be
//!
//! That guard's own "what it cannot catch" list opens with the defect this one
//! catches, stated in advance and left unguarded:
//!
//! > **A writer that is never CALLED.** `store_standard_bindings` existing does
//! > not mean a crawl invokes it.
//!
//! §11 D170 found tables with readers and no writers, and the repair wrote the
//! writers. Nothing then checked that anything called them. Two months later the
//! same shape appeared one layer up and cost a whole feature: **§11 D192** —
//! `journal_extract::extract_requirements` was deterministic, tested by 20 unit
//! tests, and called from `guidelines.rs` **only inside `#[cfg(test)]`**, with
//! every other caller in `examples/`. The production pasted-URL path fetched the
//! page, sanitised it, classified it, chunked it into the RAG corpus and stepped
//! over the extractor. A user pasting their journal's author-guidelines URL got
//! the four structural rows and none of the requirements printed on the page.
//!
//! **A green suite cannot see this and never could.** Every one of those 20
//! tests passed, correctly, about a function no user could reach.
//!
//! # The list is DERIVED, not typed
//!
//! Enumerated from `pub fn` in the two modules rather than from what the author
//! remembers being there — the §11 D170 rule, whose hand survey grepped four
//! table names and missed the fifth, which was the one that mattered. Nine
//! public functions today.
//!
//! # What it cannot catch, in the same spirit
//!
//! 1. **A caller that never RUNS.** A production call site inside a branch no
//!    input reaches satisfies this scan. Only a real run counts rows.
//! 2. **A caller passing something useless.** `extract_requirements(&[])` is a
//!    call. The BEFORE/AFTER in §11 D192 is what establishes a real yield.
//! 3. **Producers outside these two modules.** The general form — every public
//!    producer in `gaply_core` — is a larger change with its own noise question,
//!    and is unbuilt. Measured while writing this: of the nine here, three had no
//!    production caller, and all three were already predicted by the sibling.

use std::path::Path;

/// The modules whose public surface must be reachable from production.
const MODULES: [&str; 2] = ["src/journal_extract.rs", "src/journal_store.rs"];

/// Everything that could legitimately call them. `..` is the app crate: `cargo
/// test -p gaply_core` runs with the package root as the working directory, so
/// `../src` is `src-tauri/src`. **The app crate is where the production callers
/// live** — `guidelines.rs`, `commands.rs`, `pipeline.rs` — so a scan confined
/// to `gaply_core` would have passed on the very defect that prompted this.
const CALLER_DIRS: [&str; 2] = ["src", "../src"];

/// **Reachable only from `examples/journal_stage2_build.rs` and their own unit
/// tests — measured 19 Sep 2026, not assumed.**
///
/// These three are the §11 D170 repair: the writers were added so the tables
/// would have producers, and the offline Stage-2 corpus build calls them. No
/// path a user takes does. They are listed rather than fixed because wiring them
/// means deciding when a crawl runs on a user's machine, which is a separate
/// change with a consent question attached.
///
/// **The allowlist is matched EXACTLY in both directions.** An allowlist that
/// silently stops matching tightens a guard into noise, which is loud; one that
/// silently matches too much exempts whole files and is quiet. So a name that
/// gains a production caller fails here too, with an instruction to delete the
/// line — the quiet direction is the one designed against.
const NO_PRODUCTION_CALLER: [&str; 3] =
    ["store_conventions", "store_standard_bindings", "store_expectations"];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has the crate moved?", dir.display());
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Production source only: everything before the first `#[cfg(test)]`, never a
/// `tests.rs`, and with line comments removed so a doc comment naming a function
/// does not read as a call. **This stripping IS the guard** — at the commit
/// before §11 D192 the call to `extract_requirements` was present in
/// `guidelines.rs` and sat 318 lines below its `#[cfg(test)]`.
fn production_source(path: &Path, src: &str) -> String {
    if path.file_name().is_some_and(|n| n == "tests.rs") {
        return String::new();
    }
    let cut = src.find("#[cfg(test)]").unwrap_or(src.len());
    src[..cut]
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `pub fn name(` at the start of a line, from the module's production source.
fn public_fns(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| l.strip_prefix("pub fn "))
        .filter_map(|r| r.split(['(', '<', ' ']).next())
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect()
}

/// A call is `name(` or `name (` outside the declaring module. Deliberately not
/// a parse: the failure direction of a loose match is a MISSED offender, and the
/// allowlist below is what keeps that honest.
fn calls(src: &str, name: &str) -> bool {
    let mut hay = src;
    while let Some(i) = hay.find(name) {
        let (before, after) = (&hay[..i], &hay[i + name.len()..]);
        let boundary_l = before.chars().last().is_none_or(|c| !c.is_alphanumeric() && c != '_');
        let boundary_r = after.trim_start().starts_with('(');
        if boundary_l && boundary_r && !before.trim_end().ends_with("fn") {
            return true;
        }
        hay = &hay[i + name.len()..];
    }
    false
}

#[test]
fn every_public_journal_producer_is_called_from_production_code() {
    let mut files = Vec::new();
    for d in CALLER_DIRS {
        rust_files(Path::new(d), &mut files);
    }
    files.sort();
    // Windows CI went red on `location_is_unambiguous.rs` for exactly this, so
    // every path comparison below is on a normalised string.
    let sources: Vec<(String, String)> = files
        .iter()
        .map(|f| {
            let rel = f.to_string_lossy().replace('\\', "/");
            let src = std::fs::read_to_string(f).expect("read");
            let prod = production_source(f, &src);
            (rel, prod)
        })
        .collect();
    assert!(
        sources.len() > 50,
        "only {} source files found across {CALLER_DIRS:?} — the scan has lost the tree and \
         would pass vacuously",
        sources.len()
    );
    assert!(
        sources.iter().any(|(p, _)| p.contains("../src/guidelines.rs")),
        "the app crate is not in the scan — a guard confined to gaply_core would have passed \
         on §11 D192 itself"
    );

    let mut unreachable = Vec::new();
    let mut allowlisted_but_called = Vec::new();
    let mut checked = 0usize;

    for module in MODULES {
        let msrc = std::fs::read_to_string(module).unwrap_or_else(|e| panic!("{module}: {e}"));
        let fns = public_fns(&production_source(Path::new(module), &msrc));
        assert!(
            fns.len() >= 2,
            "only {} public fns parsed from {module} — the parser has drifted from the source \
             and this guard would pass vacuously",
            fns.len()
        );
        for f in fns {
            checked += 1;
            let called = sources
                .iter()
                .any(|(p, s)| !p.ends_with(module.trim_start_matches("src/")) && calls(s, &f));
            let allowed = NO_PRODUCTION_CALLER.contains(&f.as_str());
            match (called, allowed) {
                (false, false) => unreachable.push(format!("{module}::{f}")),
                (true, true) => allowlisted_but_called.push(f),
                _ => {}
            }
        }
    }

    assert!(checked >= 8, "only {checked} functions checked — the guard is close to inert");
    assert!(
        unreachable.is_empty(),
        "these produce something and nothing in production calls them — the §11 D192 shape, \
         where 20 passing unit tests described a function no user could reach:\n  {}\n\
         Either wire it to the path that needs it, or add it to NO_PRODUCTION_CALLER with the \
         measurement that says why.",
        unreachable.join("\n  ")
    );
    assert!(
        allowlisted_but_called.is_empty(),
        "these are on NO_PRODUCTION_CALLER and now HAVE a production caller: {allowlisted_but_called:?}\n\
         Delete those lines. An allowlist that keeps matching after the reason has gone is the \
         quiet failure direction — it exempts real code and nothing says so."
    );
}
