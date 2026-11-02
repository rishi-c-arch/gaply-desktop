//! **Two rules about what test code may do to the process it shares (Research
//! OS, Phase 8M-2, decision 415).**
//!
//! A test binary is one process, and libtest runs its tests side by side on
//! threads, so a test that changes something process-wide changes it for its
//! neighbours.
//!
//! * **No test swaps the panic hook.** The hook is one global. While a test has
//!   it swapped, a neighbour that panics loses its message, so its failure is
//!   an empty block with a name under it. That is what 8K found in 8I-2's log.
//!   This scan covers `tests/` and every `#[cfg(test)]` item, in both crates.
//!   Production code (`logging.rs`) is out of its scope.
//! * **No test in the app crate starts a process.** A child holds a copy of the
//!   binary's descriptors until its exec closes them, and a `flock` lasts while
//!   any copy does, so a project lock a neighbour has released looks held (8K,
//!   decision 401). `std::process::id()` is not a spawn.
//!
//! **What this scan can see.** It reads source, with comments and string
//! literals blanked, so a sentence or a pattern about a call is not a call, and
//! this file needs no exemption of its own. It constrains what is TYPED: a
//! macro from another crate that expands to one of these calls is invisible to
//! it. A positive count of files and of `#[cfg(test)]` items, and each
//! exemption still having to match, keep it from passing on empty input.

use std::path::{Path, PathBuf};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("a directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// The source with every comment and the inside of every string and char
/// literal replaced by spaces. Newlines stay, so a byte offset keeps its line.
fn code_only(source: &str) -> String {
    let b = source.as_bytes();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut out[from..to] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let end = b[i..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .map_or(b.len(), |n| i + n);
                blank(&mut out, i, end);
                i = end;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let (mut depth, mut j) = (1, i + 2);
                while j < b.len() && depth > 0 {
                    if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut out, i, j);
                i = j;
            }
            b'r' if !(i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'))
                && matches!(b.get(i + 1), Some(b'"') | Some(b'#')) =>
            {
                let mut j = i + 1;
                let mut hashes = 0;
                while b.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if b.get(j) != Some(&b'"') {
                    i += 1; // a raw identifier, r#type
                    continue;
                }
                let close: Vec<u8> = std::iter::once(b'"')
                    .chain(std::iter::repeat_n(b'#', hashes))
                    .collect();
                let end = b[j + 1..]
                    .windows(close.len())
                    .position(|w| w == close.as_slice())
                    .map_or(b.len(), |n| j + 1 + n + close.len());
                blank(&mut out, i + 1, end);
                i = end;
            }
            b'"' => {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = (j + 1).min(b.len());
                blank(&mut out, i + 1, end.saturating_sub(1));
                i = end;
            }
            b'\'' => {
                // A char literal, or a lifetime, which has no closing quote.
                let end = if b.get(i + 1) == Some(&b'\\') {
                    b[i + 2..]
                        .iter()
                        .position(|c| *c == b'\'')
                        .map(|n| i + 2 + n)
                } else {
                    // One character of up to four bytes, then the quote.
                    (2..=5)
                        .find(|n| b.get(i + n) == Some(&b'\''))
                        .filter(|n| {
                            std::str::from_utf8(&b[i + 1..i + n])
                                .is_ok_and(|s| s.chars().count() == 1)
                        })
                        .map(|n| i + n)
                };
                match end {
                    Some(end) => {
                        blank(&mut out, i + 1, end);
                        i = end + 1;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).expect("only ASCII spaces replaced whole characters")
}

/// True for an attribute that compiles what follows only under test.
fn is_test_attribute(attribute: &str) -> bool {
    let compact: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    (compact.starts_with("#[cfg(") || compact.starts_with("#![cfg("))
        && !compact.contains("not(test)")
        && ["(test", ",test"].iter().any(|t| compact.contains(t))
}

/// A `#[cfg(test)]` region of one source file, in code-only text.
enum Region {
    Inline(String),
    /// `#[cfg(test)] mod name;`: a file of its own.
    File(String),
}

fn test_regions(code: &str) -> Vec<Region> {
    let b = code.as_bytes();
    let mut regions = Vec::new();
    let mut from = 0;
    while let Some(n) = code[from..].find("#") {
        let start = from + n;
        from = start + 1;
        let Some(len) = code[start..].find(']') else {
            break;
        };
        let attribute = &code[start..=start + len];
        if !is_test_attribute(attribute) {
            continue;
        }
        if attribute.starts_with("#!") {
            regions.push(Region::Inline(code[start..].to_string()));
            break;
        }
        // The item the attribute applies to runs to its first `;` or to the
        // brace that closes its first `{`.
        let mut j = start + len + 1;
        let mut depth = 0usize;
        let mut opened = false;
        while j < b.len() {
            match b[j] {
                b'{' => {
                    depth += 1;
                    opened = true;
                }
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if opened && depth == 0 {
                        break;
                    }
                }
                b';' if !opened => break,
                _ => {}
            }
            j += 1;
        }
        let end = (j + 1).min(b.len());
        let item = &code[start + len + 1..end];
        let words: Vec<&str> = item
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| !w.is_empty())
            .collect();
        if !opened && words.iter().any(|w| *w == "mod") {
            regions.push(Region::File(
                words.last().expect("a mod has a name").to_string(),
            ));
        } else {
            regions.push(Region::Inline(item.to_string()));
        }
        from = end;
    }
    regions
}

/// The file a `mod name;` in `parent` is read from.
fn module_file(parent: &Path, name: &str) -> PathBuf {
    let dir = parent.parent().expect("a source file has a directory");
    let stem = parent.file_stem().expect("a file name").to_string_lossy();
    let base = if matches!(stem.as_ref(), "mod" | "lib" | "main") {
        dir.to_path_buf()
    } else {
        dir.join(stem.as_ref())
    };
    [
        base.join(format!("{name}.rs")),
        base.join(name).join("mod.rs"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .unwrap_or_else(|| panic!("{}: no file for test module {name}", parent.display()))
}

/// One piece of test code: where it came from, and its code-only text.
struct TestCode {
    path: PathBuf,
    code: String,
}

struct Scan {
    pieces: Vec<TestCode>,
    /// Files read under `tests/`.
    test_files: usize,
    /// `#[cfg(test)]` items found under `src/`.
    test_items: usize,
}

/// Every file under `<root>/tests` and every `#[cfg(test)]` item under
/// `<root>/src`.
fn scan_crate(root: &Path) -> Scan {
    let mut scan = Scan {
        pieces: Vec::new(),
        test_files: 0,
        test_items: 0,
    };
    let mut files = Vec::new();
    rust_files(&root.join("tests"), &mut files);
    for path in files {
        let code = code_only(&std::fs::read_to_string(&path).expect("a test file"));
        scan.test_files += 1;
        scan.pieces.push(TestCode { path, code });
    }
    let mut sources = Vec::new();
    rust_files(&root.join("src"), &mut sources);
    for path in sources {
        let code = code_only(&std::fs::read_to_string(&path).expect("a source file"));
        for region in test_regions(&code) {
            scan.test_items += 1;
            match region {
                Region::Inline(code) => scan.pieces.push(TestCode {
                    path: path.clone(),
                    code,
                }),
                Region::File(name) => {
                    let file = module_file(&path, &name);
                    let code = code_only(&std::fs::read_to_string(&file).expect("a test module"));
                    scan.pieces.push(TestCode { path: file, code });
                }
            }
        }
    }
    scan
}

fn relative(path: &Path) -> String {
    path.strip_prefix(manifest())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Each `(path, pattern)` where test code holds a pattern, comparing whole
/// words so `settle_hook` or `Command::newest` is not a finding.
fn findings(pieces: &[TestCode], patterns: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for piece in pieces {
        for pattern in patterns {
            if holds(&piece.code, pattern) {
                out.push((relative(&piece.path), pattern.to_string()));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn holds(code: &str, pattern: &str) -> bool {
    let wordy = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices(pattern).any(|(at, _)| {
        let before_ok =
            !pattern.starts_with(wordy) || !code[..at].chars().next_back().is_some_and(wordy);
        let after_ok = !pattern.ends_with(wordy)
            || !code[at + pattern.len()..].chars().next().is_some_and(wordy);
        before_ok && after_ok
    })
}

/// What changes the process-wide panic hook, however it was imported.
const HOOK_CALLS: &[&str] = &["set_hook", "take_hook", "update_hook"];

/// The one test allowed to, and why: it is the only test in its own binary
/// (pinned below), so no neighbour shares its hook.
const HOOK_EXEMPT: &str = "tests/runs_seam_lines.rs";

/// What starts a process. `process::id` is not on it. A `Command`'s `spawn`,
/// `output` and `status` all need one of the first three to name it; on their
/// own those words are also a thread's, a response's and a guard's.
const SPAWN_CALLS: &[&str] = &[
    "process::Command",
    "process::{",
    "Command::new",
    "libc::fork",
    "libc::vfork",
    "libc::system",
    "libc::popen",
    "libc::execv",
    "libc::execve",
    "libc::execvp",
    "posix_spawn",
    "posix_spawnp",
];

/// The one test binary allowed to, and why: it runs the `ai-eval` binary as
/// its subject, in a test binary of its own that holds no project lock (both
/// pinned below).
const SPAWN_EXEMPT: &str = "tests/ai_eval_cli.rs";

#[test]
fn no_test_changes_the_process_wide_panic_hook() {
    let core_dir = manifest().join("gaply-core");
    // The app's `src` holds no part of the core crate; `gaply-core/` is its
    // own root, so nothing is read twice.
    let app = scan_crate(&manifest());
    let core = scan_crate(&core_dir);
    assert!(
        app.test_files >= 50 && core.test_files >= 50,
        "too few test files read, so the scan would pass vacuously: app {}, core {}",
        app.test_files,
        core.test_files
    );
    assert!(
        app.test_items >= 50 && core.test_items >= 50,
        "too few #[cfg(test)] items read, so the scan would pass vacuously: app {}, core {}",
        app.test_items,
        core.test_items
    );
    let mut all = app.pieces;
    all.extend(core.pieces);
    let found = findings(&all, HOOK_CALLS);
    assert!(
        found.iter().any(|(path, _)| path == HOOK_EXEMPT),
        "{HOOK_EXEMPT} no longer swaps the hook, or the scan cannot see it: drop the exemption, or repair the scan"
    );
    let offenders: Vec<_> = found
        .iter()
        .filter(|(path, _)| path != HOOK_EXEMPT)
        .collect();
    assert!(
        offenders.is_empty(),
        "test code changes the process-wide panic hook, so a neighbour's failure can lose its \
         message (decision 415). Catch the panic and check its payload instead: {offenders:?}"
    );
}

#[test]
fn the_hook_exemption_is_the_only_test_in_its_binary() {
    let source = std::fs::read_to_string(manifest().join(HOOK_EXEMPT)).expect("the exempt file");
    assert_eq!(
        code_only(&source).matches("#[test]").count(),
        1,
        "{HOOK_EXEMPT} shares its binary now, so its hook is shared too: it loses its exemption"
    );
}

#[test]
fn no_app_test_starts_a_process() {
    let app = scan_crate(&manifest());
    assert!(
        app.test_files >= 50 && app.test_items >= 50,
        "too little read, so the scan would pass vacuously: {} files, {} items",
        app.test_files,
        app.test_items
    );
    let found = findings(&app.pieces, SPAWN_CALLS);
    assert!(
        found.iter().any(|(path, _)| path == SPAWN_EXEMPT),
        "{SPAWN_EXEMPT} no longer starts a process, or the scan cannot see it: drop the exemption, or repair the scan"
    );
    let offenders: Vec<_> = found
        .iter()
        .filter(|(path, _)| path != SPAWN_EXEMPT)
        .collect();
    assert!(
        offenders.is_empty(),
        "an app test starts a process: its child holds this binary's descriptors until its exec \
         closes them, so a released project lock looks held (decisions 401 and 415). Make the \
         system call directly through libc: {offenders:?}"
    );
}

#[test]
fn the_spawn_exemption_holds_no_project_lock() {
    let source = std::fs::read_to_string(manifest().join(SPAWN_EXEMPT)).expect("the exempt file");
    let code = code_only(&source);
    for word in ["project_store", "ProjectStore", "flock"] {
        assert!(
            !holds(&code, word),
            "{SPAWN_EXEMPT} names {word}: it may hold a lock now, and its child would keep it"
        );
    }
}

#[test]
fn the_scan_reads_code_and_not_what_is_said_about_it() {
    let source = concat!(
        "// std::panic::set_hook in a comment\n",
        "/* take_hook /* nested */ set_hook */\n",
        "const A: &str = \"set_hook\";\n",
        "const B: &str = r#\"Command::new(\"x\")\"#;\n",
        "const C: char = '\"';\n",
        "fn f<'a>(x: &'a str) { let _ = x; }\n",
        "fn g() { std::panic::set_hook(Box::new(|_| {})); }\n",
    );
    let code = code_only(source);
    assert_eq!(code.matches("set_hook").count(), 1, "{code}");
    assert!(holds(&code, "set_hook"));
    assert!(!holds(&code, "Command::new"), "{code}");
    assert!(!holds(&code, "take_hook"), "{code}");
    assert!(!holds("fn offset_hook() {}", "set_hook"));
    assert!(!holds("std::process::id()", "process::Command"));
    assert!(holds("use std::process::{self};", "process::{"));
}

#[test]
fn a_test_region_ends_where_its_item_ends() {
    let code = code_only(concat!(
        "fn production() { real_call(); }\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    fn inner() { test_call(); }\n",
        "}\n",
        "fn more_production() { other_call(); }\n",
        "#[cfg(not(test))]\n",
        "fn not_a_test() { hidden_call(); }\n",
        "#[cfg(test)]\n",
        "mod external;\n",
        "#[cfg(all(test, unix))]\n",
        "fn helper() { helper_call(); }\n",
    ));
    let regions = test_regions(&code);
    assert_eq!(regions.len(), 3);
    let Region::Inline(first) = &regions[0] else {
        panic!("the inline module");
    };
    assert!(
        first.contains("test_call") && !first.contains("other_call"),
        "{first}"
    );
    assert!(!first.contains("real_call"), "{first}");
    assert!(matches!(&regions[1], Region::File(name) if name == "external"));
    let Region::Inline(third) = &regions[2] else {
        panic!("the helper");
    };
    assert!(
        third.contains("helper_call") && !third.contains("hidden_call"),
        "{third}"
    );
}
