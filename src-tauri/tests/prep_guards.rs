//! **What must not name a prepared table (Research OS, Phase 8M-3, decision
//! 427; ADR-011 Part 11), and how the make command gets its cap (decision
//! 424).** A source scan, comments blanked, by whole identifier: `PreparedSend`
//! (`stats_assist/session.rs`, `stats_l1_commands.rs`) is not `Prepared`.
//!
//! Level 1 and PublishReady never see a score. `SupplementaryEvidence`, what
//! PublishReady's reviewer is sent, is built by its own ingest and never from a
//! slot; Level 1 opens through its own run store. Those are facts about what
//! the code does today, and this holds them: none of these files may name the
//! module, its types or the constructor that adds a column.
//!
//! It constrains what is TYPED in these files, not what a callee does, and it
//! reads every file by name, so a file that moves fails it rather than leaving
//! the scan quietly shorter.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const FORBIDDEN: &[&str] = &[
    "stats_prep",
    "Prepared",
    "Recipe",
    "Preparation",
    "with_appended_columns",
];

/// Each named: the stats_assist files, the Level 1 commands and the ingest both
/// paths share, and the PublishReady path.
const SCANNED: &[&str] = &[
    "gaply-core/src/stats_assist/citation.rs",
    "gaply-core/src/stats_assist/codes.rs",
    "gaply-core/src/stats_assist/consent.rs",
    "gaply-core/src/stats_assist/mod.rs",
    "gaply-core/src/stats_assist/outbound.rs",
    "gaply-core/src/stats_assist/query.rs",
    "gaply-core/src/stats_assist/response.rs",
    "gaply-core/src/stats_assist/session.rs",
    "gaply-core/src/stats_assist/spec.rs",
    "gaply-core/src/stats_assist/transport.rs",
    "src/stats_ai.rs",
    "src/stats_l1_commands.rs",
    "src/stats_l1_ingest.rs",
    "src/commands.rs",
    "src/pipeline.rs",
    "src/report_build.rs",
    "src/reviewer_synthesis.rs",
    "src/supplementary.rs",
    "gaply-core/src/report.rs",
    "gaply-core/src/report_compose.rs",
    "gaply-core/src/reviewer_agent.rs",
];

/// `//` comments blanked, a `//` inside a string kept.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| {
            let mut in_str = false;
            let b = l.as_bytes();
            for i in 0..b.len() {
                match b[i] {
                    b'"' if i == 0 || b[i - 1] != b'\\' => in_str = !in_str,
                    b'/' if !in_str && b.get(i + 1) == Some(&b'/') => return &l[..i],
                    _ => {}
                }
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every forbidden name `src` holds as a whole identifier, with its line.
fn mentions(file: &str, src: &str) -> Vec<String> {
    let code = code_only(src);
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        for w in line.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if FORBIDDEN.contains(&w) {
                out.push(format!("{file}:{}: `{w}`", n + 1));
            }
        }
    }
    out
}

#[test]
fn level_1_stats_assist_and_publishready_never_name_a_prepared_table() {
    let base = root();
    let mut scanned = 0;
    let mut found = Vec::new();
    for rel in SCANNED {
        let path: &Path = &base.join(rel);
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{rel}: {e} — it has moved, so name where it went"));
        scanned += 1;
        found.extend(mentions(rel, &src));
    }
    assert_eq!(
        scanned,
        SCANNED.len(),
        "the scan read fewer files than it names"
    );
    assert!(scanned >= 20, "{scanned} files: the list is inert");
    println!("scanned {scanned} files");
    assert!(
        found.is_empty(),
        "decision 427, ADR-011 Part 11:\n{}",
        found.join("\n")
    );
}

/// The list is complete for stats_assist: a new file there fails this until it
/// is named above.
#[test]
fn every_stats_assist_source_file_is_named() {
    let dir = root().join("gaply-core/src/stats_assist");
    let mut missing = Vec::new();
    let mut seen = 0;
    for e in std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
    {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "rs") {
            seen += 1;
            let rel = format!(
                "gaply-core/src/stats_assist/{}",
                p.file_name().unwrap().to_string_lossy()
            );
            if !SCANNED.contains(&rel.as_str()) {
                missing.push(rel);
            }
        }
    }
    assert!(seen >= 10, "{seen} files: the folder scan is inert");
    assert!(missing.is_empty(), "not scanned: {missing:?}");
}

/// A name is matched whole, in code and not in a comment.
#[test]
fn the_scan_keys_on_whole_identifiers_in_code() {
    for hit in [
        "use gaply_core::stats_prep::Recipe;",
        "fn f(p: Prepared) {}",
        "let x: Option<Preparation> = None;",
        "d.with_appended_columns(&c, f);",
    ] {
        assert!(!mentions("x", hit).is_empty(), "{hit}");
    }
    for miss in [
        "struct PreparedSend;",
        "// Prepared, Recipe and stats_prep in a comment",
        "let prepared = 1; let recipes = 2;",
        "let url = \"https://example.org\"; // Recipe",
    ] {
        assert!(mentions("x", miss).is_empty(), "{miss}");
    }
}

/// The command passes this computer's cap, typed inline as its last argument,
/// as the open passes its limits (decisions 286 and 424); the tests' bodies
/// take any cap, so nothing else would notice another value here.
#[test]
fn the_make_command_passes_this_computers_cells_cap_inline() {
    let src = code_only(
        &std::fs::read_to_string(root().join("src/stats_prep_commands.rs")).expect("the commands"),
    );
    let at = src
        .find("pub async fn stats_prep_make(")
        .expect("the make command");
    let body = &src[at..];
    let squashed: String = body
        .split("pub async fn stats_prep_clear(")
        .next()
        .unwrap()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        squashed.contains(
            "make(&runs,&slots,slot,&recipe,IngestLimits::this_computer().max_cellsasu64,)"
        ),
        "{squashed}"
    );
}
