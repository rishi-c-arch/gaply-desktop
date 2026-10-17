//! **The Statistics capability registry (Research OS, Phase 4A).**
//!
//! * A pin: the SHA-256 of the sorted `(id, version, lifecycle)` list. A
//!   method's version raised, a capability added or a lifecycle changed
//!   turns it red, and the failure prints the list it saw.
//! * Every capability id written in production source is registered: string
//!   literals that are exactly `stats.check`, `stats.describe` or
//!   `stats.verify.<word>`, in non-test code under `src-tauri/src`,
//!   `gaply-core/src` and the frontend's `src/`. The `stats.desc.*` codes are
//!   not ids and do not match.
//! * The table is derived from the catalogue, every suite and decision record
//!   it names exists, and Level 0's input cap is `import_guard::MAX_BYTES`.
//! * The Statistics workspace descriptor's capability ids are the table's.
//! * `ConfirmationId` holds 64 lowercase hex digits only and never prints them.

use std::path::{Path, PathBuf};

use gaply_core::contracts::capability::{ConfirmationId, Lifecycle};
use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{ContentHash, WorkspaceId};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue};
use gaply_core::stats_l1::compute::L1Registry;
use gaply_core::stats_registry::{self, Kind, Registry};

/// The sorted list, one `id version lifecycle` line each, measured 18 Oct
/// 2026 (Phase 4A):
///
/// ```text
/// stats.check 1 stable
/// stats.describe 1 beta
/// stats.verify.chi_square_independence 1 beta
/// stats.verify.independent_t 1 beta
/// stats.verify.ols 1 beta
/// stats.verify.one_way_anova 1 beta
/// stats.verify.paired_t 1 beta
/// stats.verify.pearson 1 beta
/// stats.verify.spearman 1 beta
/// ```
const PIN: &str = "9bc76ee09c9d26c2a0d4fdcdc0b95ca9d4fea50225371e305d312018536b8401";

fn listing() -> String {
    let mut lines: Vec<String> = Registry::statistics()
        .entries()
        .iter()
        .map(|e| {
            let d = &e.descriptor;
            format!("{} {} {}", d.id.as_str(), d.version, d.lifecycle.wire())
        })
        .collect();
    lines.sort();
    lines.join("\n")
}

#[test]
fn the_capability_list_is_pinned() {
    let list = listing();
    let got = ContentHash::of_bytes(list.as_bytes());
    assert_eq!(
        got.as_hex(),
        PIN,
        "the (id, version, lifecycle) list changed. It is now:\n{list}\nIts SHA-256 is {}. Update the pin and the list above only on purpose.",
        got.as_hex()
    );
}

#[test]
fn the_table_is_derived_from_the_catalogue_and_names_what_exists() {
    let t = Registry::statistics();
    let verify: Vec<L1MethodId> = t
        .entries()
        .iter()
        .filter_map(|e| match e.kind {
            Kind::Verify(m) => Some(m),
            _ => None,
        })
        .collect();
    let catalogue: Vec<L1MethodId> = MethodCatalogue::ENTRIES.iter().map(|e| e.id).collect();
    assert_eq!(
        verify, catalogue,
        "one stats.verify capability per catalogue entry, in its order"
    );
    for e in t.entries() {
        let d = &e.descriptor;
        assert_eq!(d.workspace_id, WorkspaceId::Statistics);
        assert_eq!(d.id.workspace(), WorkspaceId::Statistics);
        if let Kind::Verify(m) = e.kind {
            assert_eq!(d.id.as_str(), format!("stats.verify.{}", m.id()));
            assert_eq!(
                d.version.to_string(),
                m.version(),
                "{}: the method's version",
                m.id()
            );
            let expected = if L1Registry::get(m).is_some() {
                Lifecycle::Beta
            } else {
                Lifecycle::Planned
            };
            assert_eq!(d.lifecycle, expected, "{}", m.id());
        }
        let suite = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/{}.rs", d.validation_suite_id.0));
        assert!(
            suite.is_file(),
            "{}: no test target {}",
            d.id.as_str(),
            d.validation_suite_id.0
        );
        let plan = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/AI_ENGINE_PLAN.md"),
        )
        .expect("the plan");
        assert!(
            plan.contains(&format!("\n### {} — ", d.documentation_id.0)),
            "{}: no decision record {}",
            d.id.as_str(),
            d.documentation_id.0
        );
    }
    let kinds: Vec<Kind> = t.entries().iter().map(|e| e.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == Kind::Check).count(), 1);
    assert_eq!(kinds.iter().filter(|k| **k == Kind::Describe).count(), 1);
    let describe = t
        .entries()
        .iter()
        .find(|e| e.kind == Kind::Describe)
        .unwrap();
    assert_eq!(
        describe.descriptor.id,
        gaply_core::stats_desc::capability_id(),
        "Phase 2A's id"
    );
    assert_eq!(
        describe.descriptor.version,
        gaply_core::stats_desc::CAPABILITY_VERSION
    );
    // Level 0's input cap is the manuscript cap of import_guard.
    let check = t.entries().iter().find(|e| e.kind == Kind::Check).unwrap();
    assert_eq!(
        check.descriptor.resource_profile.max_input_bytes,
        Some(gaply_core::import_guard::MAX_BYTES)
    );
}

#[test]
fn the_statistics_descriptors_capability_ids_are_the_registrys() {
    assert_eq!(
        stats_registry::workspace_descriptor(WorkspaceId::Statistics).capability_ids,
        Registry::statistics().ids()
    );
    for w in [WorkspaceId::Compute, WorkspaceId::Design] {
        assert!(
            stats_registry::workspace_descriptor(w)
                .capability_ids
                .is_empty(),
            "{w:?} claims nothing"
        );
    }
}

/* ---- every id in production source is registered ------------------------ */

fn files(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable", dir.display());
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            // Test fixtures are test code.
            if name != "fixtures" && name != "__snapshots__" && name != "node_modules" {
                files(&p, exts, out);
            }
        } else if exts.iter().any(|x| name.ends_with(x))
            && !name.contains(".vitest.")
            && !name.contains(".test.")
            && name != "tests.rs"
        {
            out.push(p);
        }
    }
}

/// The Rust source without its `#[cfg(test)] mod … { … }` blocks.
fn without_test_modules(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(brace) = after.find('{') else {
            rest = "";
            break;
        };
        // `#[cfg(test)] mod tests;` declares a file, skipped by name.
        if after[..brace].contains(';') {
            let semi = after.find(';').unwrap();
            rest = &after[semi + 1..];
            continue;
        }
        let mut depth = 0usize;
        let mut end = after.len();
        for (i, c) in after[brace..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = brace + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// String literals of a source file: `"…"` and, in TypeScript, `'…'` and
/// `` `…` ``.
fn literals(src: &str, ts: bool) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let q = chars[i];
        if q == '"' || (ts && (q == '\'' || q == '`')) {
            let mut j = i + 1;
            let mut s = String::new();
            while j < chars.len() && chars[j] != q && chars[j] != '\n' {
                if chars[j] == '\\' {
                    j += 1;
                }
                if j < chars.len() {
                    s.push(chars[j]);
                }
                j += 1;
            }
            out.push(s);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn is_capability_id(s: &str) -> bool {
    let tail = |t: &str| {
        !t.is_empty()
            && t.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    s == "stats.check" || s == "stats.describe" || s.strip_prefix("stats.verify.").is_some_and(tail)
}

fn ids_in_source(roots: &[(&Path, &[&str], bool)]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (dir, exts, ts) in roots {
        let mut fs = Vec::new();
        files(dir, exts, &mut fs);
        assert!(
            fs.len() > 20,
            "{}: {} files — the scan is inert",
            dir.display(),
            fs.len()
        );
        for f in fs {
            let src = std::fs::read_to_string(&f).unwrap_or_default();
            let code = if *ts { src } else { without_test_modules(&src) };
            for l in literals(&code, *ts) {
                if is_capability_id(&l) {
                    found.push((f.display().to_string(), l));
                }
            }
        }
    }
    found
}

#[test]
fn every_capability_id_in_production_source_is_registered() {
    let core = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let app = core.join("../src");
    let web = core.join("../../src");
    let found = ids_in_source(&[
        (&core.join("src"), &[".rs"], false),
        (&app, &[".rs"], false),
        (&web, &[".ts", ".tsx", ".json"], true),
    ]);
    let registered: Vec<String> = Registry::statistics()
        .ids()
        .iter()
        .map(|i| i.as_str().to_string())
        .collect();
    let unregistered: Vec<&(String, String)> = found
        .iter()
        .filter(|(_, id)| !registered.contains(id))
        .collect();
    println!("capability ids written in production source: {found:?}");
    assert!(
        found.len() >= 3,
        "only {} id literals found — the scan is inert",
        found.len()
    );
    assert!(
        unregistered.is_empty(),
        "ids written in source but not registered: {unregistered:?}"
    );
}

#[test]
fn the_id_scan_sees_what_it_is_for() {
    assert!(
        is_capability_id("stats.check")
            && is_capability_id("stats.verify.ols")
            && is_capability_id("stats.verify.x_2")
    );
    for not in [
        "stats.desc.unknown_capability",
        "stats.verify.",
        "stats.checks",
        "stats.verify.Ols",
        "compute.solve",
    ] {
        assert!(!is_capability_id(not), "{not}");
    }
    let rs = "fn a() { let x = \"stats.verify.kruskal\"; }\n#[cfg(test)]\nmod tests {\n    const Y: &str = \"stats.verify.mann\";\n}\n";
    let lits: Vec<String> = literals(&without_test_modules(rs), false)
        .into_iter()
        .filter(|l| is_capability_id(l))
        .collect();
    assert_eq!(
        lits,
        vec!["stats.verify.kruskal".to_string()],
        "production code is read, a test module is not"
    );
    let ts = "const a = 'stats.describe'; const b = `stats.check`;";
    assert_eq!(
        literals(ts, true)
            .iter()
            .filter(|l| is_capability_id(l))
            .count(),
        2
    );
}

/* ---- ConfirmationId ------------------------------------------------------ */

#[test]
fn a_confirmation_id_is_64_lowercase_hex_and_prints_nothing() {
    let ok = "0123456789abcdef".repeat(4);
    let id = ConfirmationId::parse(&ok).expect("64 hex");
    assert_eq!(id.as_str(), ok);
    assert_eq!(format!("{id:?}"), "ConfirmationId(..)");
    for bad in [
        "0123456789ABCDEF".repeat(4),
        "0123456789abcdef".repeat(4) + "0",
        "0123456789abcde".to_string(),
        "/Users/someone/data.csv".to_string(),
        "g".repeat(64),
    ] {
        assert!(ConfirmationId::parse(&bad).is_none(), "{bad}");
        let json = serde_json::to_string(&bad).unwrap();
        let e = serde_json::from_str::<ConfirmationId>(&json)
            .unwrap_err()
            .to_string();
        assert!(
            !e.contains(&bad),
            "the refusal does not echo its input: {e}"
        );
    }
    assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{ok}\""));
}
