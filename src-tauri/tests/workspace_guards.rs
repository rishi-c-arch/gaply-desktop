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
    // The two the Stats Verifier page calls stay.
    for kept in ["commands::run_stats_preview", "commands::run_stats_verify"] {
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
