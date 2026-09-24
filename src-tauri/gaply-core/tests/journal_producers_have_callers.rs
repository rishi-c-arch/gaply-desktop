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
//! table names and missed the fifth, which was the one that mattered.
//!
//! # "Production" means the application runtime path, and nothing else
//!
//! **§11 D227.** Not tests, not benchmarks, not devtools binaries, not examples,
//! not migration utilities. Until then the scan counted all of `src/` in both
//! crates, so `src/bin/grrb.rs` — a devtools benchmark added a day after this
//! guard — counted as a production caller of `extract_requirements`. Replacing
//! the real pasted-URL call with an empty `Vec` left this guard GREEN (the
//! PublishReady audit's DT-A2-2), which is the exact defect it exists to catch.
//! It also read each module only up to its first `#[cfg(test)]`, so
//! `load_bundled_seed`, defined below a mid-file test module, was never
//! enumerated and its only caller could be deleted unnoticed (DT-A2-1).
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

/// **Inside `CALLER_DIRS` but not the application runtime path.** Each entry is
/// a Cargo `[[bin]]` directory: every binary there is a devtools harness gated
/// on `required-features = ["devtools"]` (benchmarks `grrb`/`grrb-gate`, the
/// `ai-eval` harness, the `label-cn`/`label-cs` labelling tools). A caller here
/// is a benchmark calling the function, not a user reaching it.
const NOT_RUNTIME_DIRS: [&str; 1] = ["../src/bin/"];

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

/// Production source only: every `#[cfg(test)]` ITEM removed (not everything
/// after the first one — see the header), never a `tests.rs`, and with line
/// comments removed so a doc comment naming a function does not read as a call.
/// **This stripping IS the guard** — at the commit before §11 D192 the call to
/// `extract_requirements` was present in `guidelines.rs` and sat 318 lines below
/// its `#[cfg(test)]`.
fn production_source(path: &Path, src: &str) -> String {
    if path.file_name().is_some_and(|n| n == "tests.rs") {
        return String::new();
    }
    strip_cfg_test(src)
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove every item gated by `#[cfg(test)]`. Relies on rustfmt's layout, which
/// this repo's code follows: a gated item is either one line ending in `;`
/// (`mod x;`, `use …;`) or a block closed by a `}` at the attribute's own
/// indentation. The self-check in the test — no `#[test]` survives stripping —
/// is what catches a layout this does not understand.
fn strip_cfg_test(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if !l.trim_start().starts_with("#[cfg(test)]") {
            out.push(l);
            i += 1;
            continue;
        }
        let indent = &l[..l.len() - l.trim_start().len()];
        i += 1;
        // Further attributes on the same item.
        while i < lines.len() && lines[i].trim_start().starts_with("#[") {
            i += 1;
        }
        let Some(item) = lines.get(i) else { break };
        i += 1;
        let t = item.trim_end();
        if t.ends_with(';') || (t.contains('{') && t.ends_with('}')) {
            continue;
        }
        while i < lines.len() {
            let c = lines[i];
            i += 1;
            // EXACTLY `}` at the attribute's indentation. A raw-string fixture
            // can put `}"#;` at column 0 (`ai/tasks/citation_need.rs` does), and
            // `starts_with('}')` read that as the module's end — the self-check
            // below caught it on the first run.
            if c.trim_end().strip_prefix(indent) == Some("}") {
                break;
            }
        }
    }
    out.join("\n")
}

/// Files that exist only as `#[cfg(test)] mod name;` declarations in a parent
/// (`wire_contract_tests.rs`, `ai/needle_tests.rs`, …). They are tests in their
/// own file, and a name-based rule (`tests.rs`) cannot see them. DERIVED from
/// the declarations, not typed.
fn cfg_test_module_files(files: &[std::path::PathBuf]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let lines: Vec<&str> = src.lines().collect();
        for w in lines.windows(2) {
            if !w[0].trim_start().starts_with("#[cfg(test)]") {
                continue;
            }
            let Some(name) = w[1]
                .trim()
                .strip_prefix("mod ")
                .or_else(|| w[1].trim().strip_prefix("pub mod "))
                .and_then(|r| r.strip_suffix(';'))
            else {
                continue;
            };
            let dir = f.parent().unwrap_or(Path::new(""));
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let base = if matches!(stem, "lib" | "main" | "mod") { dir.to_path_buf() } else { dir.join(stem) };
            for cand in [base.join(format!("{name}.rs")), base.join(name).join("mod.rs")] {
                out.push(cand.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out
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
    let test_files = cfg_test_module_files(&files);
    assert!(
        test_files.iter().any(|t| t.ends_with("/wire_contract_tests.rs")),
        "the #[cfg(test)] mod-file derivation found none of the known ones: {test_files:?}"
    );
    let mut excluded_bins = 0usize;
    let mut excluded_test_files = 0usize;
    let sources: Vec<(String, String)> = files
        .iter()
        .filter_map(|f| {
            let rel = f.to_string_lossy().replace('\\', "/");
            if NOT_RUNTIME_DIRS.iter().any(|d| rel.starts_with(d)) {
                excluded_bins += 1;
                return None;
            }
            if test_files.contains(&rel) {
                excluded_test_files += 1;
                return None;
            }
            let src = std::fs::read_to_string(f).expect("read");
            let prod = production_source(f, &src);
            assert!(
                !prod.contains("#[test]"),
                "{rel}: a #[test] survived #[cfg(test)] stripping — the stripper does not \
                 understand this layout, and test code would count as a production caller"
            );
            Some((rel, prod))
        })
        .collect();
    // Positive counts: an exclusion that silently matches nothing is the quiet
    // failure — the scan would count bins and test files again and say nothing.
    assert!(excluded_bins >= 1, "no src/bin/ file excluded — NOT_RUNTIME_DIRS no longer matches");
    assert!(excluded_test_files >= 1, "no #[cfg(test)] mod file excluded");
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
    let mut enumerated: Vec<String> = Vec::new();

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
            enumerated.push(f.clone());
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
        enumerated.iter().any(|f| f == "load_bundled_seed"),
        "load_bundled_seed was not enumerated — it sits below a mid-file #[cfg(test)] module, \
         so its absence means the module is being truncated again: {enumerated:?}"
    );
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
