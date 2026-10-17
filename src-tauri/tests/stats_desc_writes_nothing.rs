//! **The descriptive summary's run manager, commands, contracts and
//! operation write no file (Research OS, Phase 2B; Prompt 0's DATA ON DISK
//! rule and decision 5).**
//!
//! Until an ADR approves where user data is stored, nothing of the slice may
//! persist: an export is bytes returned to the webview, and the save dialog
//! writes the one file the user names. A source scan, comments removed, over
//! `src/runs.rs`, `src/stats_desc_commands.rs`, `gaply-core/src/contracts/`
//! and `gaply-core/src/stats_desc/`, for any way to write, create, rename,
//! copy or remove a file or folder.
//!
//! It constrains what was TYPED in these files, not what a callee does: the
//! commands call Level 1's ingest, which reads and does not write, and the
//! privacy matrix (`stats_desc_privacy_matrix.rs`) checks at run time that
//! the app-data folder is unchanged, byte for byte, after the whole flow.

use std::path::{Path, PathBuf};

const SCANNED: &[&str] = &[
    "src/runs.rs",
    "src/stats_desc_commands.rs",
    "gaply-core/src/contracts",
    "gaply-core/src/stats_desc",
];

const FORBIDDEN: &[&str] = &[
    "fs::write",
    "File::create",
    "File::options",
    "OpenOptions",
    "rename(",
    "fs::copy",
    "create_dir",
    "remove_file",
    "remove_dir",
    "set_permissions",
    "write_all",
    "hard_link",
    "symlink",
];

fn rust_files(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_file() {
        out.push(p.to_path_buf());
        return;
    }
    for e in std::fs::read_dir(p)
        .unwrap_or_else(|e| panic!("{}: {e} — has it moved?", p.display()))
        .flatten()
    {
        let q = e.path();
        if q.is_dir() {
            rust_files(&q, out);
        } else if q.extension().is_some_and(|x| x == "rs") {
            out.push(q);
        }
    }
}

/// The code with `//` comments removed, a `//` inside a string kept.
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

fn violations(file: &str, src: &str) -> Vec<String> {
    let code = code_only(src);
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        for f in FORBIDDEN {
            if line.contains(f) {
                out.push(format!("{file}:{}: `{f}`", n + 1));
            }
        }
    }
    out
}

#[test]
fn the_run_manager_commands_contracts_and_operation_write_no_file() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for s in SCANNED {
        rust_files(&root.join(s), &mut files);
    }
    assert!(
        files.len() >= 6,
        "{} files scanned — the scan would pass vacuously",
        files.len()
    );
    let mut found = Vec::new();
    for f in &files {
        found.extend(violations(
            &f.display().to_string(),
            &std::fs::read_to_string(f).unwrap(),
        ));
    }
    assert!(
        found.is_empty(),
        "a way to write a file, where nothing may persist:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scan_catches_each_way_to_write() {
    for snippet in [
        "fn f() { std::fs::write(p, b).unwrap(); }",
        "fn f() { let _ = std::fs::File::create(p); }",
        "fn f() { std::fs::OpenOptions::new().append(true).open(p); }",
        "fn f() { std::fs::rename(a, b).ok(); }",
        "fn f() { f.write_all(b) }",
    ] {
        assert_eq!(violations("x.rs", snippet).len(), 1, "{snippet}");
    }
    for snippet in [
        "// std::fs::write(p, b) in a comment",
        "#[serde(rename_all = \"camelCase\")]",
        "fn f() { let url = \"https://example.org\"; } // fs::write",
    ] {
        assert!(violations("x.rs", snippet).is_empty(), "{snippet}");
    }
}
