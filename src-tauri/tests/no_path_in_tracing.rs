//! **No tracing span or event in the app crate records a file path, a file
//! name, a directory or a URL (Research OS decision 8, Phase 2B).**
//!
//! A path names the user's folders and often the user (`/Users/<name>/…`),
//! and a file name is often a manuscript's or a dataset's own title. The log
//! goes to stdout today (`logging.rs`), and `~/gaply-dev.log` keeps it, so a
//! path in a span is a path on disk.
//!
//! What it scans: every `.rs` file under `src/` and, since the evaluation of
//! Phases 2 and 3 (F3), under `gaply-core/src/`, comments removed and string
//! literals kept whole. Before F3 a field was judged by its key alone, so
//! `gaply_core::db`'s `db = %path.display()` passed even once the scan reached
//! it (measured: the core scan went green with that line in place).
//! * Every `#[instrument]` or `#[tracing::instrument]` attribute, including
//!   one imported under another name through a `use`. Each parameter of the
//!   function it instruments must be skipped (`skip_all`, or named in
//!   `skip(…)`) when its NAME contains `path`, `file`, `filename`, `dir` or
//!   `url`, or its TYPE is a `Path`, `PathBuf`, `OsStr`, `OsString` or
//!   `File`, or bytes (`[u8]`, `Vec<u8>`). No `fields(…)` key may be
//!   path-like.
//! * Every tracing event or span macro (`trace!` to `error!`, `event!`,
//!   `span!`, `*_span!`): no field key may be path-like (`path = …`,
//!   `%url`, `?file`), no field VALUE whose leading name is path-like
//!   (`db = %path.display()`), no inline format capture (`{path}`), and no
//!   positional format argument whose leading name is path-like. The value
//!   rule applies to `fields(…)` of an `#[instrument]` too.
//! * Every `.record("…", …)`: no path-like key.
//!
//! **What it cannot see, by construction: a path held under a name and a
//! type that say nothing about it** (`manuscript: String`). It constrains
//! what was TYPED, a name or a type, not what a span RENDERS. The rendered
//! side is `tests/no_path_in_command_spans.rs`, which runs `detect_ai`,
//! `run_stats_preview`, `run_stats_verify` and `run_stats_chat` with a path
//! sentinel and reads every span and event they produce. For a new command
//! that takes user input, `skip_all` is the rule that needs no name.
//!
//! Matching is by substring, as decision 8's guard was specified, so
//! `profile` and `direction` count as path-like; skipping them costs a log
//! field, and an allowlist would be a place for a real path to hide.
//!
//! It fails when its own matchers stop matching: it requires a positive
//! number of attributes and of macros, and `the_guard_catches_each_kind_of_violation`
//! runs each rule on a snippet that must fail and one that must pass.

use std::path::{Path, PathBuf};

/// The app crate's source.
const APP_DIR: &str = "src";
/// `gaply_core`'s source, from the app crate's root (evaluation of Phases 2
/// and 3, F3: the database path was logged from here, outside the scan).
const CORE_DIR: &str = "gaply-core/src";

/// A name is path-like when it contains one of these, in any case.
const PATH_WORDS: &[&str] = &["path", "file", "filename", "dir", "url"];

/// A parameter of one of these types holds a path or a file.
const PATH_TYPES: &[&str] = &["Path", "PathBuf", "OsStr", "OsString", "File"];

const EVENT_MACROS: &[&str] = &["trace", "debug", "info", "warn", "error", "event"];
const SPAN_MACROS: &[&str] = &[
    "span",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
];

fn path_like(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    PATH_WORDS.iter().any(|w| n.contains(w))
}

/// A name is user-text-like when one of its `_`/`.` segments is one of these
/// (evaluation of Phases 2 and 3, F3: a document's title, a search query, a
/// project's name and description and an agent's free-text decision reached
/// `gaply_core`'s spans). Whole segments, not substrings: `context` is not
/// `text`, `prompt_version` is not a prompt. A name whose last segment is
/// `id` or `ids` is an identifier, not text: `manuscript_id` passes and
/// `manuscript_name` does not (decision 40).
const TEXT_WORDS: &[&str] = &[
    "title",
    "manuscript",
    "query",
    "description",
    "decision",
    "text",
    "content",
    "sentence",
    "excerpt",
    "snippet",
    "label",
    "header",
    "cell",
];

fn text_like(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let segments: Vec<&str> = n.split(|c: char| c == '_' || c == '.').collect();
    if matches!(segments.last(), Some(&"id") | Some(&"ids")) {
        return false;
    }
    segments.iter().any(|seg| TEXT_WORDS.contains(seg))
}

/* ---- reading the source ---------------------------------------------------- */

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has it moved?", dir.display());
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// The source twice, char for char the same length: `code` with comments
/// blanked and strings kept, and `masked` with the inside of every string and
/// char literal blanked too, for matching brackets and keys.
struct Source {
    code: Vec<char>,
    masked: Vec<char>,
}

fn lex(src: &str) -> Source {
    let s: Vec<char> = src.chars().collect();
    let mut code = s.clone();
    let mut masked = s.clone();
    let blank = |v: &mut Vec<char>, i: usize| {
        if v[i] != '\n' {
            v[i] = ' ';
        }
    };
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        let next = s.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < s.len() && s[i] != '\n' {
                blank(&mut code, i);
                blank(&mut masked, i);
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < s.len() {
                if s[i] == '/' && s.get(i + 1) == Some(&'*') {
                    depth += 1;
                    for k in i..i + 2 {
                        blank(&mut code, k);
                        blank(&mut masked, k);
                    }
                    i += 2;
                } else if s[i] == '*' && s.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    for k in i..i + 2 {
                        blank(&mut code, k);
                        blank(&mut masked, k);
                    }
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut code, i);
                    blank(&mut masked, i);
                    i += 1;
                }
            }
        } else if c == 'r'
            && (next == Some('"') || next == Some('#'))
            && !ident_char(s.get(i.wrapping_sub(1)).copied())
        {
            // A raw string: r"…", r#"…"#, and br"…" through the 'b' before it.
            let mut j = i + 1;
            let mut hashes = 0;
            while s.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if s.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            j += 1;
            let start = j;
            loop {
                if j >= s.len() {
                    break;
                }
                if s[j] == '"' && (0..hashes).all(|h| s.get(j + 1 + h) == Some(&'#')) {
                    break;
                }
                j += 1;
            }
            for k in start..j.min(s.len()) {
                blank(&mut masked, k);
            }
            i = j + 1 + hashes;
        } else if c == '"' {
            let mut j = i + 1;
            while j < s.len() && s[j] != '"' {
                if s[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            for k in i + 1..j.min(s.len()) {
                blank(&mut masked, k);
            }
            i = j + 1;
        } else if c == '\'' {
            // A char literal, or a lifetime.
            if next == Some('\\') {
                let mut j = i + 2;
                while j < s.len() && s[j] != '\'' {
                    j += 1;
                }
                for k in i + 1..j.min(s.len()) {
                    blank(&mut masked, k);
                }
                i = j + 1;
            } else if s.get(i + 2) == Some(&'\'') {
                blank(&mut masked, i + 1);
                i += 3;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    Source { code, masked }
}

fn ident_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn line_of(s: &[char], at: usize) -> usize {
    s[..at.min(s.len())].iter().filter(|&&c| c == '\n').count() + 1
}

/// The index of the bracket closing the one at `open`, over masked text.
fn matching(m: &[char], open: usize) -> Option<usize> {
    let (o, c) = match m[open] {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        '{' => ('{', '}'),
        _ => return None,
    };
    let mut depth = 0;
    for (i, &ch) in m.iter().enumerate().skip(open) {
        if ch == o {
            depth += 1;
        } else if ch == c {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// The pieces of `m[from..to]` between top-level commas, as index ranges.
/// Angle brackets count as nesting except in `->` and `=>`.
fn split_commas(m: &[char], from: usize, to: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut depth, mut angle, mut start) = (0i32, 0i32, from);
    for i in from..to {
        match m[i] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '<' => angle += 1,
            '>' if i > 0 && (m[i - 1] == '-' || m[i - 1] == '=') => {}
            '>' => angle = (angle - 1).max(0),
            ',' if depth == 0 && angle == 0 => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
    }
    if m[start..to].iter().any(|c| !c.is_whitespace()) {
        out.push((start, to));
    }
    out
}

fn text(v: &[char], (a, b): (usize, usize)) -> String {
    v[a..b].iter().collect::<String>().trim().to_string()
}

fn skip_ws(m: &[char], mut i: usize) -> usize {
    while i < m.len() && m[i].is_whitespace() {
        i += 1;
    }
    i
}

/// The identifier ending just before `end` (exclusive), and where it starts.
fn ident_before(m: &[char], end: usize) -> Option<(usize, String)> {
    let mut a = end;
    while a > 0 && ident_char(Some(m[a - 1])) {
        a -= 1;
    }
    (a < end).then(|| (a, m[a..end].iter().collect()))
}

/// Every identifier in a piece of text.
fn idents(t: &str) -> Vec<String> {
    t.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty() && !w.chars().next().unwrap().is_ascii_digit())
        .map(str::to_string)
        .collect()
}

/// The leading `a.b.c` chain of an expression, `&`, `*` and `%`/`?` dropped.
fn leading_chain(t: &str) -> String {
    let t = t.trim_start_matches(|c: char| {
        c == '&' || c == '*' || c == '%' || c == '?' || c.is_whitespace()
    });
    let t = t.strip_prefix("mut ").unwrap_or(t);
    t.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .collect::<String>()
        .trim_end_matches('.')
        .to_string()
}

/* ---- the rules ------------------------------------------------------------- */

#[derive(Debug, Default)]
struct Scan {
    violations: Vec<String>,
    attributes: usize,
    macros: usize,
    /// Whether user-text-like names count too (`gaply_core`'s scan).
    text_rule: bool,
}

impl Scan {
    /// Path-like, or user-text-like when the text rule is on.
    fn sensitive(&self, name: &str) -> bool {
        path_like(name) || (self.text_rule && text_like(name))
    }
}

/// A field value read through `.len()` is a count, whatever it counts.
fn a_count(chain: &str) -> bool {
    chain.ends_with(".len")
}

/// Names under which `tracing::instrument` is in scope in this file.
fn instrument_names(code: &str) -> Vec<String> {
    let mut names = vec!["instrument".to_string(), "tracing::instrument".to_string()];
    for stmt in code.split(';') {
        let s: String = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
        let Some(rest) = s.trim().strip_prefix("use tracing::") else {
            continue;
        };
        let rest = rest.trim_start_matches('{').trim_end_matches('}');
        for item in rest.split(',') {
            let parts: Vec<&str> = item.split_whitespace().collect();
            if let ["instrument", "as", alias] = parts.as_slice() {
                names.push(alias.to_string());
            }
        }
    }
    names
}

fn scan_attributes(file: &str, src: &Source, scan: &mut Scan) {
    let (c, m) = (&src.code, &src.masked);
    let code: String = c.iter().collect();
    let names = instrument_names(&code);
    let mut i = 0;
    while i + 1 < m.len() {
        if !(m[i] == '#' && m[i + 1] == '[') {
            i += 1;
            continue;
        }
        let open = i + 1;
        let Some(close) = matching(m, open) else {
            break;
        };
        let inner: String = m[open + 1..close].iter().collect();
        let head: String = inner
            .chars()
            .take_while(|ch| *ch != '(')
            .filter(|ch| !ch.is_whitespace())
            .collect();
        if !names.contains(&head) {
            i = close;
            continue;
        }
        scan.attributes += 1;
        let line = line_of(m, i);
        // The attribute's arguments.
        let args_open = (open + 1..close).find(|&k| m[k] == '(');
        let (mut skip_all, mut skipped, mut field_keys) =
            (false, Vec::<String>::new(), Vec::<String>::new());
        if let Some(ao) = args_open {
            let ac = matching(m, ao).unwrap_or(close);
            for piece in split_commas(m, ao + 1, ac) {
                let t = text(m, piece);
                if t == "skip_all" {
                    skip_all = true;
                } else if let Some(list) = t
                    .strip_prefix("skip")
                    .map(str::trim_start)
                    .and_then(|r| r.strip_prefix('('))
                {
                    skipped.extend(idents(list.trim_end_matches(')')));
                } else if t
                    .strip_prefix("fields")
                    .map(str::trim_start)
                    .is_some_and(|r| r.starts_with('('))
                {
                    let fo = piece.0 + text_offset(m, piece) + t.find('(').expect("checked");
                    let fc = matching(m, fo).unwrap_or(piece.1);
                    for f in split_commas(m, fo + 1, fc) {
                        field_keys.push(field_key(&text(m, f), &text(c, f)));
                        if let Some(v) = field_value_chain(&text(m, f)).filter(|v| !a_count(v)) {
                            field_keys.push(v);
                        }
                    }
                }
            }
        }
        for k in field_keys {
            if scan.sensitive(&k) {
                scan.violations
                    .push(format!("{file}:{line}: instrument fields key `{k}`"));
            }
        }
        // The function it instruments.
        let Some(fn_at) = find_word(m, close + 1, "fn") else {
            i = close;
            continue;
        };
        let mut k = skip_ws(m, fn_at + 2);
        let name_start = k;
        while k < m.len() && ident_char(Some(m[k])) {
            k += 1;
        }
        let fn_name: String = m[name_start..k].iter().collect();
        k = skip_ws(m, k);
        if m.get(k) == Some(&'<') {
            let mut angle = 0;
            while k < m.len() {
                match m[k] {
                    '<' => angle += 1,
                    '>' if m[k - 1] != '-' => {
                        angle -= 1;
                        if angle == 0 {
                            k += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
            k = skip_ws(m, k);
        }
        if m.get(k) != Some(&'(') {
            i = close;
            continue;
        }
        let pc = matching(m, k).unwrap_or(k);
        for piece in split_commas(m, k + 1, pc) {
            let t = text(m, piece);
            let Some(colon) = top_level_colon(&t) else {
                continue;
            };
            let (pattern, ty) = (&t[..colon], &t[colon + 1..]);
            let ty_compact: String = ty.chars().filter(|ch| !ch.is_whitespace()).collect();
            let typed_path = idents(ty).iter().any(|w| PATH_TYPES.contains(&w.as_str()))
                || ty_compact.contains("[u8]")
                || ty_compact.contains("Vec<u8>");
            for binding in idents(pattern) {
                if ["mut", "ref", "self"].contains(&binding.as_str())
                    || binding.chars().next().is_some_and(char::is_uppercase)
                {
                    continue;
                }
                if (scan.sensitive(&binding) || typed_path)
                    && !skip_all
                    && !skipped.contains(&binding)
                {
                    scan.violations.push(format!(
                        "{file}:{line}: `{fn_name}` records parameter `{binding}`"
                    ));
                }
            }
        }
        i = close;
    }
}

/// Where the trimmed text of a piece starts, relative to the piece.
fn text_offset(m: &[char], (a, b): (usize, usize)) -> usize {
    (a..b).find(|&k| !m[k].is_whitespace()).map_or(0, |k| k - a)
}

fn find_word(m: &[char], from: usize, word: &str) -> Option<usize> {
    let w: Vec<char> = word.chars().collect();
    (from..m.len().saturating_sub(w.len())).find(|&k| {
        m[k..k + w.len()] == w[..]
            && !ident_char(k.checked_sub(1).map(|p| m[p]))
            && !ident_char(m.get(k + w.len()).copied())
    })
}

/// A `:` that is not part of `::`, outside any brackets.
fn top_level_colon(t: &str) -> Option<usize> {
    let b = t.as_bytes();
    let mut depth = 0;
    for i in 0..b.len() {
        match b[i] {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b':' if depth == 0 && b.get(i + 1) != Some(&b':') && (i == 0 || b[i - 1] != b':') => {
                return Some(i)
            }
            _ => {}
        }
    }
    None
}

/// The key of one field argument: `key = v`, `%key`, `?key`, `key`, or
/// `"key" = v`. `masked` locates the `=`; `code` supplies a quoted key.
fn field_key(masked: &str, code: &str) -> String {
    let t = masked.trim();
    if let Some(rest) = t.strip_prefix('%').or_else(|| t.strip_prefix('?')) {
        return leading_chain(rest);
    }
    let b = t.as_bytes();
    let mut depth = 0;
    for i in 0..b.len() {
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                let prev = if i > 0 { b[i - 1] } else { b' ' };
                let next = b.get(i + 1).copied().unwrap_or(b' ');
                if next != b'=' && next != b'>' && !matches!(prev, b'=' | b'!' | b'<' | b'>') {
                    let key = code.trim()[..i].trim();
                    return key.trim_matches('"').to_string();
                }
            }
            _ => {}
        }
    }
    leading_chain(t)
}

/// The leading `a.b.c` chain of a `key = value` field's value, or `None`
/// when the field has no top-level `=` (`%key`, `?key`, `key`).
fn field_value_chain(masked: &str) -> Option<String> {
    let t = masked.trim();
    let b = t.as_bytes();
    let mut depth = 0;
    for i in 0..b.len() {
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                let prev = if i > 0 { b[i - 1] } else { b' ' };
                let next = b.get(i + 1).copied().unwrap_or(b' ');
                if next != b'=' && next != b'>' && !matches!(prev, b'=' | b'!' | b'<' | b'>') {
                    return Some(leading_chain(&t[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn is_string_literal(t: &str) -> bool {
    let t = t.trim();
    t.starts_with('"') || t.starts_with("r\"") || t.starts_with("r#")
}

/// `{name}` and `{name:…}` captures in a format string.
fn captures(format: &str) -> Vec<String> {
    let chars: Vec<char> = format.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if chars.get(i + 1) == Some(&'{') {
                i += 2;
                continue;
            }
            let name: String = chars[i + 1..]
                .iter()
                .take_while(|c| c.is_alphanumeric() || **c == '_')
                .collect();
            if !name.is_empty() && !name.chars().next().unwrap().is_ascii_digit() {
                out.push(name);
            }
        }
        i += 1;
    }
    out
}

fn scan_macros(file: &str, src: &Source, scan: &mut Scan) {
    let (c, m) = (&src.code, &src.masked);
    for i in 0..m.len() {
        if m[i] != '!' {
            continue;
        }
        let Some((start, name)) = ident_before(m, i) else {
            continue;
        };
        let is_event = EVENT_MACROS.contains(&name.as_str());
        let is_span = SPAN_MACROS.contains(&name.as_str());
        if !is_event && !is_span {
            continue;
        }
        // `tracing::info!` or a bare `info!`; never `log::info!`.
        if start >= 2 && m[start - 1] == ':' && m[start - 2] == ':' {
            let Some((_, owner)) = ident_before(m, start - 2) else {
                continue;
            };
            if owner != "tracing" {
                continue;
            }
        }
        let open = skip_ws(m, i + 1);
        if !matches!(m.get(open), Some('(') | Some('{') | Some('[')) {
            continue;
        }
        let Some(close) = matching(m, open) else {
            continue;
        };
        scan.macros += 1;
        let line = line_of(m, i);
        let pieces = split_commas(m, open + 1, close);
        let mut seen_string = false;
        for piece in pieces {
            let (tm, tc) = (text(m, piece), text(c, piece));
            let prefix = ["target", "parent", "name"].iter().any(|p| {
                tm.strip_prefix(p)
                    .map(str::trim_start)
                    .is_some_and(|r| r.starts_with(':') && !r.starts_with("::"))
            });
            if prefix || tm.starts_with("Level::") || tm.starts_with("tracing::Level::") {
                continue;
            }
            if is_string_literal(&tc) {
                for cap in captures(&tc) {
                    if scan.sensitive(&cap) {
                        scan.violations
                            .push(format!("{file}:{line}: `{name}!` format captures `{cap}`"));
                    }
                }
                if !seen_string {
                    seen_string = true;
                    continue;
                }
            }
            // Before an event's message, and after a span's name: fields.
            // After an event's message: positional format arguments.
            if is_event && seen_string {
                let chain = leading_chain(&tm);
                if scan.sensitive(&chain) {
                    scan.violations
                        .push(format!("{file}:{line}: `{name}!` formats `{chain}`"));
                }
            } else {
                let key = field_key(&tm, &tc);
                if scan.sensitive(&key) {
                    scan.violations
                        .push(format!("{file}:{line}: `{name}!` field `{key}`"));
                } else if let Some(v) =
                    field_value_chain(&tm).filter(|v| !a_count(v) && scan.sensitive(v))
                {
                    scan.violations.push(format!(
                        "{file}:{line}: `{name}!` field `{key}` takes `{v}`"
                    ));
                }
            }
        }
    }
    // `.record("key", …)` on a span.
    for i in 0..m.len().saturating_sub(8) {
        if m[i..i + 8].iter().collect::<String>() == ".record(" {
            let open = i + 7;
            let Some(close) = matching(m, open) else {
                continue;
            };
            if let Some(first) = split_commas(m, open + 1, close).first() {
                let t = text(c, *first);
                if is_string_literal(&t) && scan.sensitive(t.trim_matches('"')) {
                    scan.violations.push(format!(
                        "{}:{}: span record key {t}",
                        file,
                        line_of(m, i)
                    ));
                }
            }
        }
    }
}

fn scan_source(file: &str, src: &str, text_rule: bool) -> Scan {
    let source = lex(src);
    let mut scan = Scan {
        text_rule,
        ..Scan::default()
    };
    scan_attributes(file, &source, &mut scan);
    scan_macros(file, &source, &mut scan);
    scan
}

/* ---- the tests ------------------------------------------------------------- */

/// Scans every `.rs` file under `dir` and asserts positive counts (at least
/// `min_files` files, `min_attributes` attributes and `min_macros` macros, so
/// a matcher that stops matching fails here) and no violation. With
/// `text_rule`, user-text-like names count as well as path-like ones.
fn assert_no_path_in_tracing(
    dir: &str,
    text_rule: bool,
    min_files: usize,
    min_attributes: usize,
    min_macros: usize,
) {
    let files = rust_files(Path::new(dir));
    assert!(
        files.len() >= min_files,
        "{} files under {dir} — the scan would pass vacuously",
        files.len()
    );
    let mut total = Scan::default();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read");
        let s = scan_source(&f.display().to_string(), &src, text_rule);
        total.violations.extend(s.violations);
        total.attributes += s.attributes;
        total.macros += s.macros;
    }
    println!(
        "{dir}: {} files; instrument attributes: {}; tracing macros: {}",
        files.len(),
        total.attributes,
        total.macros
    );
    assert!(
        total.attributes >= min_attributes,
        "only {} instrument attributes found under {dir} — the matcher has stopped matching",
        total.attributes
    );
    assert!(
        total.macros >= min_macros,
        "only {} tracing macros found under {dir} — the matcher has stopped matching",
        total.macros
    );
    assert!(
        total.violations.is_empty(),
        "{} place(s) can record a path, file name, directory, URL or (with the text rule) user text in tracing:\n{}",
        total.violations.len(),
        total.violations.join("\n")
    );
}

/// With the text rule since decision 40 (18 Oct 2026): eight commands'
/// spans recorded search queries, a manuscript's sentences, a title, a
/// manuscript name and a project description.
#[test]
fn no_span_or_event_in_the_app_crate_records_a_path_file_name_directory_url_or_user_text() {
    assert_no_path_in_tracing(APP_DIR, true, 50, 100, 110);
}

/// Evaluation of Phases 2 and 3, F3: `gaply_core` logs too (the store's
/// opening, migrations, the agents), through the same subscriber, and with
/// the text rule: no title, query, description, decision or other user text.
#[test]
fn no_span_or_event_in_gaply_core_records_a_path_file_name_directory_url_or_user_text() {
    assert_no_path_in_tracing(CORE_DIR, true, 100, 15, 30);
}

#[test]
fn the_guard_catches_each_kind_of_violation() {
    let must_fail = [
        "#[tracing::instrument]\npub fn detect_ai(path: String) {}\n",
        "#[instrument(skip(state))]\nfn f(state: u8, data_path: String) {}\n",
        "#[tracing::instrument(skip(state))]\nfn f(state: u8, p: &Path) {}\n",
        "#[tracing::instrument]\nfn f(bytes: Vec<u8>) {}\n",
        "#[tracing::instrument(skip_all, fields(file = %x))]\nfn f(x: u8) {}\n",
        "use tracing::instrument as traced;\n#[traced]\nfn f(url: String) {}\n",
        "#[tracing::instrument(\n    skip(a)\n)]\npub async fn f<T: Fn(u8) -> u8>(a: T, dir: String) {}\n",
        "fn g() { tracing::warn!(path = %p.display(), \"x\"); }\n",
        "fn g() { tracing::info!(%url, \"x\"); }\n",
        "fn g() { tracing::debug!(?self.file, \"x\"); }\n",
        "fn g() { tracing::info!(\"opened {path}\"); }\n",
        "fn g() { tracing::error!(\"opened {}\", file_name); }\n",
        "fn g() { let s = tracing::info_span!(\"open\", dir = 1); }\n",
        "fn g() { tracing::span!(tracing::Level::INFO, \"open\", url = 1); }\n",
        "fn g() { span.record(\"path\", 1); }\n",
        "fn g() { tracing::info!(db = %path.display(), \"x\"); }\n",
        "#[tracing::instrument(skip_all, fields(n = %file.display()))]\nfn f(x: u8) {}\n",
    ];
    for snippet in must_fail {
        let s = scan_source("snippet", snippet, false);
        assert_eq!(
            s.violations.len(),
            1,
            "should fail exactly once: {snippet}\n{:?}",
            s.violations
        );
    }
    let must_pass = [
        "#[tracing::instrument(skip_all)]\npub fn detect_ai(path: String) {}\n",
        "#[tracing::instrument(skip(path, state))]\nfn f(state: u8, path: String) {}\n",
        "#[tracing::instrument]\nfn f(count: u64) {}\n",
        "fn g() { tracing::warn!(rows = n, \"x\"); }\n",
        "fn g() { log::info!(path = 1); }\n",
        "fn g() { tracing::info!(\"see https://example.org // not a comment\"); }\n",
        "// #[tracing::instrument]\n// fn f(path: String) {}\n",
        "fn g() { let s = \"#[tracing::instrument] fn f(path: String)\"; }\n",
        "fn g() { tracing::info!(rows = n, \"{} rows\", n); }\n",
        "fn g() { tracing::info!(rows = data.len(), schema_version = r.to_version, \"x\"); }\n",
        "fn g() { tracing::info!(uploads = files.len(), \"x\"); }\n",
    ];
    for snippet in must_pass {
        let s = scan_source("snippet", snippet, false);
        assert!(
            s.violations.is_empty(),
            "should pass: {snippet}\n{:?}",
            s.violations
        );
    }
}

/// The text rule (`gaply_core`'s scan), each kind on a snippet that must fail
/// with it, and must pass without it or once fixed.
#[test]
fn the_text_rule_catches_user_text_and_passes_ids_and_counts() {
    let must_fail = [
        "#[tracing::instrument(skip(db))]\nfn search(db: u8, query: &str) {}\n",
        "#[tracing::instrument(skip(store), fields(n = 1))]\nfn f(store: u8, description: &str) {}\n",
        "#[tracing::instrument(skip_all, fields(t = %doc.title))]\nfn f(doc: u8) {}\n",
        "#[tracing::instrument(skip(self, context))]\nfn f(&self, decision: &str, context: &str) {}\n",
        "fn g() { tracing::info!(sentence = %s, \"x\"); }\n",
        "#[tracing::instrument(skip(state))]\nasync fn f(state: u8, manuscript_name: String) {}\n",
    ];
    for snippet in must_fail {
        let with = scan_source("snippet", snippet, true);
        assert_eq!(
            with.violations.len(),
            1,
            "should fail once: {snippet}\n{:?}",
            with.violations
        );
        let without = scan_source("snippet", snippet, false);
        assert!(
            without.violations.is_empty(),
            "path rule alone passes: {snippet}"
        );
    }
    let must_pass = [
        "fn g() { tracing::info!(manuscript_id, flags = 3, \"x\"); }\n",
        "#[tracing::instrument(skip(text), fields(len = text.len()))]\nfn f(text: &str) {}\n",
        "#[tracing::instrument(skip(self, decision, context))]\nfn f(&self, decision: &str, context: &str) {}\n",
        "fn g() { tracing::info!(prompt_version = 1, \"x\"); }\n",
    ];
    for snippet in must_pass {
        let s = scan_source("snippet", snippet, true);
        assert!(
            s.violations.is_empty(),
            "should pass: {snippet}\n{:?}",
            s.violations
        );
    }
}
