//! **Source guards for the one active workspace (Research OS, Phase 3).**
//! Each list is derived from the artefact, not typed: the commands from
//! `lib.rs`'s `generate_handler!`, their bodies from the file that defines
//! them.
//!
//! 1. Every registered `stats_desc_*` command's FIRST statement is the
//!    workspace check, `state.workspace.require_statistics()` followed by
//!    `?`. A check later in the body would let an earlier line run while no
//!    workspace is active (decision 27).
//! 2. The legacy Stats Verifier chat is removed (decision 8):
//!    `run_stats_chat` is neither registered nor defined in the app.
//! 3. Only `workspace_activate` activates a workspace: `WorkspaceSession::
//!    activate` has exactly one caller in the app's source, and it is that
//!    command.
//! 4. Phase 4A (decision 49): every verifier OPERATION command, listed by
//!    name below, checks the workspace first, in the same form; no EXEMPT
//!    command checks it at all; and every registered command of the
//!    verifier's modules (`stats_l1_commands`, `stats_consistency`,
//!    `stats_legacy`) is in exactly one of the two lists, so a new one must
//!    be classified before it can ship.
//!
//! 5. Phase 4B-2 (decisions 74, 76, 81): the seven `stats_ttest_*` commands check
//!    the workspace first, in the same form, and `capability_list`, which
//!    reads only the static registry, does not check it at all.
//!
//! 6. Phase 5B-2 (decision 120): every `project_commands` command is in
//!    exactly one of two lists. A gated one checks the workspace first, in
//!    the same form, and so does the plain function it runs under
//!    `spawn_blocking`, whose first statement is
//!    `let … = ws.require_statistics()…?`: the work function is what tests
//!    drive and what the command reaches after its own check. An exempt one
//!    (listing, sizes, preparing a delete, deleting) names
//!    `require_statistics` in neither.
//!
//! 7. Phase 5B-3a (decision 132): the lock codes (`project.open_elsewhere`,
//!    `project.being_deleted`, `project.lock_failed`) come only from the
//!    commands that take a project's lock: `project_open`, a first Save
//!    through `project_save_run`, and `project_delete`. The store takes the
//!    lock in a pinned set of functions and only through `File::try_lock` and
//!    `File::try_lock_shared`; every other `.lock()` in it is a `Mutex`'s,
//!    with its poisoning taken as it is.
//!
//! What this cannot see: a check written in a way that is not this exact
//! call (an alias, a macro). The runtime tests in `workspace_session.rs` call
//! every `stats_desc_*` command while no workspace is active and while one is
//! activating, so a missing check fails there too.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The source without `//` comments, so a commented-out line never counts.
fn without_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The paths inside `tauri::generate_handler![ … ]` in `lib.rs`'s `run`.
fn registered() -> Vec<String> {
    let lib = without_line_comments(&read(&root().join("src/lib.rs")));
    let start = lib
        .find("generate_handler![")
        .expect("lib.rs registers its commands with generate_handler!");
    let body = &lib[start + "generate_handler![".len()..];
    let end = body.find(']').expect("the handler list ends");
    body[..end]
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The body of `fn name` in `src`, from the `{` after its signature to the
/// matching `}`.
fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let at = src.find(&format!("fn {name}("))?;
    let open = at + src[at..].find('{')?;
    let mut depth = 0usize;
    for (i, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn every_registered_stats_desc_command_checks_the_workspace_first() {
    let all = registered();
    assert!(
        all.len() > 100,
        "only {} commands found in generate_handler! — the parse is inert",
        all.len()
    );
    let desc: Vec<&str> = all
        .iter()
        .filter_map(|p| p.strip_prefix("stats_desc_commands::"))
        .collect();
    assert!(
        desc.len() >= 8 && desc.iter().all(|n| n.starts_with("stats_desc_")),
        "the stats_desc_* commands registered: {desc:?}"
    );
    // Nothing registers a stats_desc_* command from another module.
    for p in &all {
        if p.rsplit("::")
            .next()
            .is_some_and(|n| n.starts_with("stats_desc_"))
        {
            assert!(p.starts_with("stats_desc_commands::"), "{p}");
        }
    }

    let src = without_line_comments(&read(&root().join("src/stats_desc_commands.rs")));
    let mut failures = Vec::new();
    for name in &desc {
        let Some(body) = fn_body(&src, name) else {
            failures.push(format!("{name}: no body found"));
            continue;
        };
        // Whitespace removed: rustfmt splits the call chain across lines.
        let first: String = body
            .split(';')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let checks = first.starts_with("let")
            && first.contains("=state.workspace.require_statistics()")
            && first.ends_with(")?");
        if !checks {
            failures.push(format!("{name}: first statement is `{first}`"));
        }
    }
    println!("checked {} stats_desc_* commands: {desc:?}", desc.len());
    assert!(
        failures.is_empty(),
        "a stats_desc_* command does not check the workspace first:\n{}",
        failures.join("\n")
    );
}

/// The verifier's operation commands (decision 49): each runs Level 0 or
/// Level 1 work, or is the legacy lane. (file, registered path)
const VERIFIER_OPERATIONS: &[(&str, &str)] = &[
    (
        "src/stats_consistency.rs",
        "stats_consistency::run_stats_consistency",
    ),
    ("src/stats_legacy.rs", "stats_legacy::run_stats_preview"),
    ("src/stats_legacy.rs", "stats_legacy::run_stats_verify"),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_open",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_edit_claim",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_add_claim",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_proposals",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_helper_preview",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_helper_send",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_confirm_binding",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_draft_proposal",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_compute",
    ),
];

/// The commands decision 49 never gates: they only remove a key or data,
/// read or change settings, open a link, or stop a send. (file, path, why)
const VERIFIER_EXEMPT: &[(&str, &str, &str)] = &[
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_close",
        "drops a run and its data",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_helper_revoke",
        "withdraws a consent grant",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_l1_helper_cancel",
        "stops an AI send; starts nothing",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_usage",
        "reads a run's AI usage counters",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_setting_get",
        "reads a setting",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_setting_set",
        "changes a setting",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_key_status",
        "reads whether a key is saved",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_key_save",
        "saves the user's key",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_key_test",
        "tests the key, with no user data",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_ai_key_remove",
        "removes the key",
    ),
    (
        "src/stats_l1_commands.rs",
        "stats_l1_commands::stats_open_source_link",
        "opens a link",
    ),
];

/// The body's first statement, whitespace removed, is the check.
fn checks_first(body: &str) -> bool {
    let first: String = body
        .split(';')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    first.starts_with("let")
        && first.contains("=state.workspace.require_statistics()")
        && first.ends_with(")?")
}

#[test]
fn every_verifier_operation_command_checks_the_workspace_first_and_no_exempt_command_does() {
    let all = registered();
    assert!(all.len() > 100, "the handler parse is inert");
    let family: Vec<&String> = all
        .iter()
        .filter(|p| {
            p.starts_with("stats_l1_commands::")
                || p.starts_with("stats_consistency::")
                || p.starts_with("stats_legacy::")
        })
        .collect();
    let mut listed: Vec<&str> = VERIFIER_OPERATIONS.iter().map(|(_, p)| *p).collect();
    listed.extend(VERIFIER_EXEMPT.iter().map(|(_, p, _)| *p));
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), listed.len(), "a command is in both lists");
    let mut registered_family: Vec<&str> = family.iter().map(|p| p.as_str()).collect();
    registered_family.sort_unstable();
    assert_eq!(
        registered_family, sorted,
        "every registered verifier command is classified, and every classified one is registered"
    );

    let mut failures = Vec::new();
    for (file, path) in VERIFIER_OPERATIONS {
        let src = without_line_comments(&read(&root().join(file)));
        let name = path.rsplit("::").next().unwrap();
        match fn_body(&src, name) {
            Some(body) if checks_first(body) => {}
            Some(body) => failures.push(format!(
                "{path}: first statement is `{}`",
                body.split(';').next().unwrap_or("").trim()
            )),
            None => failures.push(format!("{path}: no body found")),
        }
    }
    for (file, path, why) in VERIFIER_EXEMPT {
        let src = without_line_comments(&read(&root().join(file)));
        let name = path.rsplit("::").next().unwrap();
        let body: String = fn_body(&src, name)
            .unwrap_or_else(|| panic!("{path}: no body found"))
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if body.contains("require_statistics") {
            failures.push(format!("{path} is gated, but it is exempt: {why}"));
        }
    }
    println!(
        "gated {} verifier operation commands; {} exempt",
        VERIFIER_OPERATIONS.len(),
        VERIFIER_EXEMPT.len()
    );
    assert!(failures.is_empty(), "decision 49:\n{}", failures.join("\n"));
}

/// Phase 4B-2 (decision 74, 76): the t-test's operation commands, each
/// checking the workspace first, in the same form. (registered path)
const TTEST_OPERATIONS: &[&str] = &[
    "stats_ttest_commands::stats_ttest_validate",
    "stats_ttest_commands::stats_ttest_run",
    "stats_ttest_commands::stats_ttest_status",
    "stats_ttest_commands::stats_ttest_result",
    "stats_ttest_commands::stats_ttest_cancel",
    "stats_ttest_commands::stats_ttest_export",
    // The follow-up (decision 81).
    "stats_ttest_commands::stats_ttest_group_levels",
];

/// Exempt: it reads only the static capability registry (decision 76).
/// (registered path, why)
const TTEST_EXEMPT: &[(&str, &str)] = &[(
    "stats_ttest_commands::capability_list",
    "reads the static registry; no workspace, no data",
)];

#[test]
fn every_ttest_command_checks_the_workspace_first_and_capability_list_does_not() {
    let all = registered();
    assert!(all.len() > 100, "the handler parse is inert");
    let mut family: Vec<&str> = all
        .iter()
        .map(|p| p.as_str())
        .filter(|p| p.starts_with("stats_ttest_commands::"))
        .collect();
    family.sort_unstable();
    let mut listed: Vec<&str> = TTEST_OPERATIONS.to_vec();
    listed.extend(TTEST_EXEMPT.iter().map(|(p, _)| *p));
    listed.sort_unstable();
    assert_eq!(
        family, listed,
        "every registered stats_ttest_commands command is classified, and every classified one is registered"
    );
    // Nothing registers a stats_ttest_* command from another module.
    for p in &all {
        if p.rsplit("::")
            .next()
            .is_some_and(|n| n.starts_with("stats_ttest_"))
        {
            assert!(p.starts_with("stats_ttest_commands::"), "{p}");
        }
    }

    let src = without_line_comments(&read(&root().join("src/stats_ttest_commands.rs")));
    let mut failures = Vec::new();
    for path in TTEST_OPERATIONS {
        let name = path.rsplit("::").next().unwrap();
        match fn_body(&src, name) {
            Some(body) if checks_first(body) => {}
            Some(body) => failures.push(format!(
                "{path}: first statement is `{}`",
                body.split(';').next().unwrap_or("").trim()
            )),
            None => failures.push(format!("{path}: no body found")),
        }
    }
    for (path, why) in TTEST_EXEMPT {
        let name = path.rsplit("::").next().unwrap();
        let body: String = fn_body(&src, name)
            .unwrap_or_else(|| panic!("{path}: no body found"))
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if body.contains("require_statistics") {
            failures.push(format!("{path} is gated, but it is exempt: {why}"));
        }
    }
    println!(
        "gated {} t-test commands; {} exempt",
        TTEST_OPERATIONS.len(),
        TTEST_EXEMPT.len()
    );
    assert!(
        failures.is_empty(),
        "decisions 74, 76:\n{}",
        failures.join("\n")
    );
}

/// Phase 5B-2 (decision 120): the project commands that need Statistics,
/// with the work function each runs. (registered path, work function)
const PROJECT_GATED: &[(&str, &str)] = &[
    ("project_commands::project_save_run", "save_run"),
    ("project_commands::project_open", "open"),
    ("project_commands::project_close", "close"),
    ("project_commands::project_runs", "runs"),
    ("project_commands::project_run_result", "run_result"),
    ("project_commands::project_report", "report"),
    ("project_commands::project_provenance", "provenance"),
    // Phase 5C-1a (decision 185): it runs an operation.
    ("project_commands::project_rerun", "rerun"),
];

/// The project commands that never need a workspace: seeing and deleting what
/// is stored (decision 120). (registered path, work function, why)
const PROJECT_EXEMPT: &[(&str, &str, &str)] = &[
    (
        "project_commands::project_list",
        "list",
        "lists what is stored",
    ),
    (
        "project_commands::project_sizes",
        "sizes",
        "reads sizes and caps",
    ),
    (
        "project_commands::project_prepare_delete",
        "prepare_delete",
        "issues a delete confirmation",
    ),
    (
        "project_commands::project_delete",
        "delete",
        "deletes a project",
    ),
    // Phase 5B-3b (decision 148): which project this copy has open, by id.
    (
        "project_commands::project_current",
        "current",
        "reads which project is open",
    ),
];

/// The work function's first statement is the check, on the session it is
/// given: `let … = ws.require_statistics()…?`.
fn work_checks_first(body: &str) -> bool {
    let first: String = body
        .split(';')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    first.starts_with("let") && first.contains("=ws.require_statistics()") && first.ends_with(")?")
}

#[test]
fn every_project_command_is_gated_or_exempt_as_decision_120_says() {
    let all = registered();
    assert!(all.len() > 100, "the handler parse is inert");
    let mut family: Vec<&str> = all
        .iter()
        .map(|p| p.as_str())
        .filter(|p| p.starts_with("project_commands::"))
        .collect();
    family.sort_unstable();
    let mut listed: Vec<&str> = PROJECT_GATED.iter().map(|(p, _)| *p).collect();
    listed.extend(PROJECT_EXEMPT.iter().map(|(p, _, _)| *p));
    listed.sort_unstable();
    assert_eq!(
        family, listed,
        "every registered project command is classified, and every classified one is registered"
    );
    // Nothing registers a project_* command from another module.
    for p in &all {
        if p.rsplit("::")
            .next()
            .is_some_and(|n| n.starts_with("project_"))
        {
            assert!(p.starts_with("project_commands::"), "{p}");
        }
    }

    let src = without_line_comments(&read(&root().join("src/project_commands.rs")));
    let mut failures = Vec::new();
    for (path, work) in PROJECT_GATED {
        let name = path.rsplit("::").next().unwrap();
        match fn_body(&src, name) {
            Some(body) if checks_first(body) => {}
            Some(body) => failures.push(format!(
                "{path}: first statement is `{}`",
                body.split(';').next().unwrap_or("").trim()
            )),
            None => failures.push(format!("{path}: no body found")),
        }
        match fn_body(&src, name).map(|b| b.contains(&format!("{work}(&root, &ws"))) {
            Some(true) => {}
            _ => failures.push(format!("{path} does not run `{work}`")),
        }
        match fn_body(&src, work) {
            Some(body) if work_checks_first(body) => {}
            Some(body) => failures.push(format!(
                "{work}: first statement is `{}`",
                body.split(';').next().unwrap_or("").trim()
            )),
            None => failures.push(format!("{work}: no body found")),
        }
    }
    for (path, work, why) in PROJECT_EXEMPT {
        let name = path.rsplit("::").next().unwrap();
        for f in [name, *work] {
            let body: String = fn_body(&src, f)
                .unwrap_or_else(|| panic!("{f}: no body found"))
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if body.contains("require_statistics") {
                failures.push(format!("{f} is gated, but {path} is exempt: {why}"));
            }
        }
    }
    println!(
        "gated {} project commands; {} exempt",
        PROJECT_GATED.len(),
        PROJECT_EXEMPT.len()
    );
    assert!(
        failures.is_empty(),
        "decision 120:\n{}",
        failures.join("\n")
    );
}

/// Phase 5D-1a (decision 206): the Stored data section's command counts what
/// the old database and the app's folders keep, and never needs a workspace
/// (decisions 49 and 120). It joins no other list. (registered path, work
/// function, why)
const STORED_DATA_EXEMPT: &[(&str, &str, &str)] = &[(
    "stored_data_commands::stored_data_summary",
    "summary",
    "counts what is stored",
)];

#[test]
fn every_stored_data_command_is_exempt_as_decision_206_says() {
    let all = registered();
    assert!(all.len() > 100, "the handler parse is inert");
    let mut family: Vec<&str> = all
        .iter()
        .map(|p| p.as_str())
        .filter(|p| p.starts_with("stored_data_commands::"))
        .collect();
    family.sort_unstable();
    let mut listed: Vec<&str> = STORED_DATA_EXEMPT.iter().map(|(p, _, _)| *p).collect();
    listed.sort_unstable();
    assert_eq!(
        family, listed,
        "every registered stored data command is classified, and every classified one is registered"
    );
    // Nothing registers a stored_data_* command from another module.
    for p in &all {
        if p.rsplit("::")
            .next()
            .is_some_and(|n| n.starts_with("stored_data_"))
        {
            assert!(p.starts_with("stored_data_commands::"), "{p}");
        }
    }
    // It joins no other list.
    let mut elsewhere: Vec<&str> = VERIFIER_EXEMPT.iter().map(|(_, p, _)| *p).collect();
    elsewhere.extend(VERIFIER_OPERATIONS.iter().map(|(_, p)| *p));
    elsewhere.extend(TTEST_EXEMPT.iter().map(|(p, _)| *p));
    elsewhere.extend(TTEST_OPERATIONS.iter().copied());
    elsewhere.extend(PROJECT_GATED.iter().map(|(p, _)| *p));
    elsewhere.extend(PROJECT_EXEMPT.iter().map(|(p, _, _)| *p));
    for (path, _, _) in STORED_DATA_EXEMPT {
        assert!(!elsewhere.contains(path), "{path} is in another list too");
    }

    let src = without_line_comments(&read(&root().join("src/stored_data_commands.rs")));
    let mut failures = Vec::new();
    for (path, work, why) in STORED_DATA_EXEMPT {
        let name = path.rsplit("::").next().unwrap();
        match fn_body(&src, name).map(|b| b.contains(&format!("{work}(&"))) {
            Some(true) => {}
            _ => failures.push(format!("{path} does not run `{work}`")),
        }
        for f in [name, *work] {
            let body: String = fn_body(&src, f)
                .unwrap_or_else(|| panic!("{f}: no body found"))
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if body.contains("require_statistics") {
                failures.push(format!("{f} is gated, but {path} is exempt: {why}"));
            }
        }
    }
    println!("{} stored data command exempt", STORED_DATA_EXEMPT.len());
    assert!(
        failures.is_empty(),
        "decision 206:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_project_check_catches_a_missing_check() {
    assert!(work_checks_first(
        " let engine = ws.require_statistics().map_err(refuse_workspace)?; x()"
    ));
    assert!(!work_checks_first(
        " let store = ws.projects(); ws.require_statistics()?;"
    ));
    assert!(!work_checks_first(" // nothing\n x()"));
}

#[test]
fn the_legacy_stats_chat_is_not_registered_or_defined() {
    let all = registered();
    assert!(all.len() > 100, "the handler parse is inert");
    for removed in ["run_stats_chat"] {
        assert!(
            !all.iter().any(|p| p.rsplit("::").next() == Some(removed)),
            "{removed} is registered"
        );
        let mut defined = Vec::new();
        let mut stack = vec![root().join("src")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs")
                    && without_line_comments(&read(&p)).contains(&format!("fn {removed}("))
                {
                    defined.push(p);
                }
            }
        }
        assert!(defined.is_empty(), "{removed} is defined in {defined:?}");
    }
    // The two the Stats Verifier page calls stay, registered from
    // `stats_legacy` since Phase 4A.
    for kept in [
        "stats_legacy::run_stats_preview",
        "stats_legacy::run_stats_verify",
    ] {
        assert!(all.iter().any(|p| p == kept), "{kept} is registered");
    }
}

#[test]
fn only_workspace_activate_activates_a_workspace() {
    let mut callers = Vec::new();
    let mut stack = vec![root().join("src")];
    let mut files = 0;
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files += 1;
                let src = without_line_comments(&read(&p));
                for (i, line) in src.lines().enumerate() {
                    if line.contains(".activate(") {
                        callers.push((p.clone(), i + 1, line.trim().to_string()));
                    }
                }
            }
        }
    }
    assert!(files > 50, "only {files} source files scanned");
    assert_eq!(callers.len(), 1, "callers of .activate(: {callers:#?}");
    let (file, _, line) = &callers[0];
    assert!(file.ends_with("workspace.rs"), "{callers:?}");
    let src = without_line_comments(&read(file));
    let body = fn_body(&src, "workspace_activate").expect("the command exists");
    assert!(
        body.contains(line.as_str()),
        "the one caller is workspace_activate: {line}"
    );
}

/// Phase 5B-3a (decision 132): where the store takes a project's lock, by
/// function. A lock taken anywhere else must be added here, deliberately.
const LOCK_TAKERS: &[&str] = &[
    "finish_leftovers",
    "create_with_save",
    "open_parsed",
    "discard_orphan_lock",
    "delete",
];

/// The store functions that take a lock, as the commands call them, and the
/// command whose work function calls each: these are the commands that can
/// answer with a lock code. (store call, work function, registered command)
const LOCK_COMMANDS: &[(&str, &str, &str)] = &[
    ("store.open(", "open", "project_commands::project_open"),
    (
        "store.create_with_save(",
        "save_run",
        "project_commands::project_save_run",
    ),
    (
        "store.delete(",
        "delete",
        "project_commands::project_delete",
    ),
];

/// The name of the `fn` whose body holds byte `at` of `src`.
fn enclosing_fn(src: &str, at: usize) -> String {
    let start = src[..at].rfind("fn ").expect("inside a function") + 3;
    src[start..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

#[test]
fn the_lock_codes_come_only_from_the_commands_that_take_a_lock() {
    let store = without_line_comments(&read(&root().join("src/project_store.rs")));
    let store = &store[..store.find("#[cfg(test)]\nmod tests").unwrap_or(store.len())];
    let commands = without_line_comments(&read(&root().join("src/project_commands.rs")));
    let mut failures = Vec::new();

    // Where the store takes the lock.
    let mut takers: Vec<String> = store
        .match_indices("self.lock(")
        .map(|(i, _)| enclosing_fn(store, i))
        .collect();
    takers.sort();
    takers.dedup();
    let mut pinned: Vec<String> = LOCK_TAKERS.iter().map(|s| s.to_string()).collect();
    pinned.sort();
    if takers != pinned {
        failures.push(format!(
            "the lock is taken in {takers:?}, pinned {pinned:?}"
        ));
    }
    // The two refusal codes of a WouldBlock name which side refused.
    for (code, side) in [("OpenElsewhere", "delete"), ("BeingDeleted", "open")] {
        let at = store.match_indices(&format!("ProjectCode::{code}")).count();
        if at == 0 {
            failures.push(format!("project.{code} is never given ({side})"));
        }
    }

    // Only the try_ calls, in one place.
    let try_lock = fn_body(store, "try_lock_file").unwrap_or_default();
    for call in ["file.try_lock_shared()", "file.try_lock()"] {
        if !try_lock.contains(call) {
            failures.push(format!("try_lock_file does not call {call}"));
        }
    }
    let flat: String = store.chars().filter(|c| !c.is_whitespace()).collect();
    for blocking in [".lock_shared(", "libc::flock", "LockFileEx", "fs2::"] {
        if flat.contains(blocking) {
            failures.push(format!(
                "the store calls {blocking}: a blocking or second lock"
            ));
        }
    }
    for (i, _) in flat.match_indices(".lock()") {
        let after = &flat[i + ".lock()".len()..];
        if !after.starts_with(".unwrap_or_else(|p|p.into_inner())") {
            failures.push(format!(
                "a .lock() that is not a Mutex's: …{}…",
                &flat[i.saturating_sub(40)..(i + 40).min(flat.len())]
            ));
        }
    }
    if flat.matches(".lock()").count() < 3 {
        failures.push("the Mutex scan found too few sites: it is inert".to_string());
    }

    // The commands that can answer with a lock code, and their classes.
    let gated: Vec<&str> = PROJECT_GATED.iter().map(|(p, _)| *p).collect();
    let exempt: Vec<&str> = PROJECT_EXEMPT.iter().map(|(p, _, _)| *p).collect();
    for (call, work, command) in LOCK_COMMANDS {
        match fn_body(&commands, work) {
            Some(body) if body.contains(call) => {}
            _ => failures.push(format!("{work} does not call {call}")),
        }
        let classified = gated.contains(command) || exempt.contains(command);
        if !classified {
            failures.push(format!("{command} is not classified"));
        }
    }
    let mut callers: Vec<String> = ["store.open(", "store.create_with_save(", "store.delete("]
        .iter()
        .flat_map(|c| {
            commands
                .match_indices(c)
                .map(|(i, _)| enclosing_fn(&commands, i))
                .collect::<Vec<_>>()
        })
        .collect();
    callers.sort();
    if callers != ["delete", "open", "save_run"] {
        failures.push(format!(
            "the lock-taking store calls are made from {callers:?}"
        ));
    }
    println!(
        "lock taken in {} store functions; {} commands can answer with a lock code",
        LOCK_TAKERS.len(),
        LOCK_COMMANDS.len()
    );
    assert!(
        failures.is_empty(),
        "decision 132:\n{}",
        failures.join("\n")
    );
}
