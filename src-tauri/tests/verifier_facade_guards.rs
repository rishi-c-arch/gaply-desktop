//! **The verifier's operation paths go through their façades (Research OS,
//! Phase 4A), by source scan.** The runtime tests show the outputs are
//! unchanged; this shows the registry check is on the path that runs:
//!
//! 1. Level 0's seam calls `stats_adapters::check` and Level 1's open
//!    `stats_adapters::check_with_reading` (one reading, F5); Level
//!    1's compute calls `stats_adapters::verify`; the describe validation
//!    calls `describe_validate`; in the run manager, each method's
//!    implementation calls its own façades (`describe_validate` and
//!    `describe_until`, `ttest_validate` and `ttest_run`), the one start and
//!    worker reach them only through it, and it never calls a method's own
//!    functions around them (Research OS, Phase 8C).
//! 2. No production source in the app calls Level 0's `check_reading` or
//!    Level 1's `reproduce` directly, except the one named seam,
//!    `stats_reproduction::run_stats_reproduction_measured`, which only
//!    tests and examples call (its callers are checked here too).
//! 3. The helper path never computes: `stats_l1_guards.rs` forbids
//!    `run_stats_reproduction_measured` in the helper functions; compute now
//!    runs through the façade, so the same functions may not name
//!    `stats_adapters::verify`, `verify_spec` or `reproduce(` either.
//! 4. `helper_send` checks the method an AI answer names against the
//!    registry (`runnable_method`) for both tasks whose answer names one.
//!
//! Comments are removed before matching, and `#[cfg(test)]` modules are not
//! production source.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn code(rel: &str) -> String {
    let src = std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn without_test_modules(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let semi = after.find(';').unwrap_or(after.len());
        let brace = after.find('{').unwrap_or(after.len());
        if semi < brace {
            rest = &after[(semi + 1).min(after.len())..];
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

fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
    let at = src
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("no fn {name}"));
    let open = at + src[at..].find('{').expect("a body");
    let mut depth = 0usize;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open..open + i + 1];
                }
            }
            _ => {}
        }
    }
    panic!("{name}: unbalanced body")
}

/// Whether `src` calls the free function `name`: `name(` with no identifier
/// character or `.` before it.
fn calls(src: &str, name: &str) -> bool {
    let needle = format!("{name}(");
    src.match_indices(&needle).any(|(i, _)| {
        !src[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
    })
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn each_operation_path_calls_its_facade() {
    let checks = [
        (
            "src/stats_consistency.rs",
            "run_stats_consistency_measured",
            "stats_adapters::check(",
        ),
        (
            "src/stats_l1_commands.rs",
            "open",
            "stats_adapters::check_with_reading(",
        ),
        (
            "src/stats_l1_commands.rs",
            "compute",
            "stats_adapters::verify(",
        ),
        (
            "src/stats_desc_commands.rs",
            "validate",
            "stats_adapters::describe_validate(",
        ),
    ];
    for (file, f, facade) in checks {
        let src = without_test_modules(&code(file));
        let body = squash(fn_body(&src, f));
        assert!(
            body.contains(facade),
            "{file}: `{f}` does not call `{facade}`"
        );
    }
    // The run manager (Research OS, Phase 8C, decision 249): each method's
    // implementation calls its own façades, the one start and the one worker
    // reach them only through the implementation, and each kind's start
    // begins its own method. A body is read inside its own `impl` block, so
    // the trait's bodiless declarations above it are never what is read.
    let runs_src = without_test_modules(&code("src/runs.rs"));
    for (method, validate, compute) in [
        (
            "Summary",
            "stats_adapters::describe_validate(",
            "stats_adapters::describe_until(",
        ),
        (
            "IndependentT",
            "stats_adapters::ttest_validate(",
            "stats_adapters::ttest_run(",
        ),
    ] {
        let block = impl_block(&runs_src, &format!("impl Method for {method} {{"));
        for (f, facade) in [("validate", validate), ("compute", compute)] {
            assert!(
                squash(fn_body(block, f)).contains(facade),
                "src/runs.rs: {method}'s `{f}` does not call `{facade}`"
            );
        }
    }
    for (f, through) in [
        ("begin<M: Method>", "M::validate("),
        ("work", "M::compute("),
        ("start", "self.begin::<Summary>("),
        ("start_then", "self.begin::<Summary>("),
        ("start_ttest", "self.begin::<IndependentT>("),
        ("start_ttest_then", "self.begin::<IndependentT>("),
    ] {
        assert!(
            squash(fn_body(&runs_src, f)).contains(through),
            "src/runs.rs: `{f}` does not call `{through}`"
        );
    }
    // And the run manager never calls a method's functions itself.
    let runs = squash(&runs_src);
    for direct in [
        "stats_desc::validate(",
        "stats_desc::describe_until(",
        "stats_ttest::validate(",
        "stats_ttest::run(",
    ] {
        assert!(
            !runs.contains(direct),
            "src/runs.rs calls `{direct}` around its façade"
        );
    }
}

/// The `{ … }` block that opens with `head` (which ends in its `{`).
fn impl_block<'a>(src: &'a str, head: &str) -> &'a str {
    let at = src.find(head).unwrap_or_else(|| panic!("no `{head}`"));
    let open = at + head.len() - 1;
    let mut depth = 0usize;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open..open + i + 1];
                }
            }
            _ => {}
        }
    }
    panic!("{head}: unbalanced block")
}

fn app_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            app_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_production_source_calls_level_0_or_level_1_around_the_facades() {
    let mut files = Vec::new();
    app_sources(&root().join("src"), &mut files);
    assert!(files.len() > 50, "only {} source files", files.len());
    let mut found = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(root())
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        let src = without_test_modules(&code(&rel));
        for name in ["check_reading", "reproduce"] {
            if calls(&src, name) {
                found.push(format!("{rel}: {name}("));
            }
        }
    }
    found.sort();
    found.dedup();
    // The one seam: `reproduce(state, pairs, …)` inside
    // `run_stats_reproduction_measured`, and its import.
    assert_eq!(
        found,
        vec!["src/stats_reproduction.rs: reproduce(".to_string()],
        "a direct call of Level 0 or Level 1 outside its façade"
    );
    // That seam is called by tests and examples only.
    for f in &files {
        let rel = f
            .strip_prefix(root())
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        if rel == "src/stats_reproduction.rs" {
            continue;
        }
        let src = without_test_modules(&code(&rel));
        assert!(
            !src.contains("run_stats_reproduction_measured"),
            "{rel} calls the seam that bypasses the registry"
        );
    }
}

#[test]
fn the_helper_path_never_computes_through_the_facade_either() {
    let src = code("src/stats_l1_commands.rs");
    for f in [
        "helper_preview",
        "helper_send",
        "helper_send_in",
        "transmit",
        "record_usage",
        "helper_revoke",
        "helper_cancel",
    ] {
        let body = fn_body(&src, f);
        for needle in ["stats_adapters::verify(", "verify_spec(", "reproduce("] {
            assert!(
                !body.contains(needle),
                "`{f}` contains `{needle}`: a Level 1 computation from the helper path"
            );
        }
    }
}

#[test]
fn helper_send_checks_the_method_an_ai_answer_names() {
    let src = code("src/stats_l1_commands.rs");
    // The command's path passes the real table (decision 96).
    let send = squash(fn_body(&src, "helper_send"));
    assert!(
        send.contains("helper_send_in(&Registry::statistics(),"),
        "{send}"
    );
    let body = fn_body(&src, "helper_send_in");
    let checks = body.matches("runnable_method(registry,").count();
    assert_eq!(
        checks, 2,
        "claim_method and binding_suggestion each check their method: {checks}"
    );
    let rule = squash(fn_body(&src, "runnable_method"));
    assert!(
        rule.contains(
            "stats_adapters::method_is_runnable_in(registry,m,crate::workspace::build())"
        ),
        "{rule}"
    );
    assert!(
        rule.contains("ReplyRejection::UnknownMethod"),
        "the validator's own refusal"
    );
}

/// The Phase 4 evaluation's F5: `stats_l1_open` read Level 0's results twice,
/// once for the document context and the claims and again inside
/// `stats_adapters::check`, which doubles the slowest step of opening a
/// large manuscript. `open` reads them once and hands that reading to the
/// façade (`check_with_reading`), which still asks the registry first.
#[test]
fn level_1s_open_reads_level_0s_results_once() {
    let src = without_test_modules(&code("src/stats_l1_commands.rs"));
    let body = squash(fn_body(&src, "open"));
    let reads = body.matches("read_results(").count();
    assert_eq!(reads, 1, "`open` reads Level 0's results {reads} times");
    assert!(
        !body.contains("stats_adapters::check("),
        "`open` calls the façade that reads the results again"
    );
    assert!(
        body.contains("stats_adapters::check_with_reading("),
        "`open` hands its reading to Level 0's façade"
    );
}

/// Decision 96: a test builds a capability table with one capability changed
/// or removed (`Registry::with_lifecycle`, `Registry::without`) and passes it
/// in as a parameter. No production source may build one: the app's and
/// gaply_core's non-test code never call either constructor, so every
/// shipped path reads `Registry::statistics()` as it is.
#[test]
fn no_production_source_builds_a_modified_capability_table() {
    let mut files = Vec::new();
    app_sources(&root().join("src"), &mut files);
    app_sources(&root().join("gaply-core/src"), &mut files);
    let mut scanned = 0;
    for f in files {
        if f.file_name().is_some_and(|n| n == "tests.rs") {
            continue;
        }
        let rel = f
            .strip_prefix(root())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let src = without_test_modules(&code(&rel));
        scanned += 1;
        for needle in [".with_lifecycle(", ".without("] {
            assert!(
                !src.contains(needle),
                "{rel} calls `{needle}` outside a test"
            );
        }
    }
    assert!(
        scanned > 200,
        "only {scanned} files scanned: the guard is inert"
    );
}
