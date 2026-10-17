//! **`StatsConsistencyOutcome` and its view carry no score: no field named
//! score, percent, probability, confidence or grade.**
//!
//! Level 0's verdict set is closed (`stats_l0::method::L0Verdict`: four
//! variants and no score), and the view's header says "Counts only". A number
//! that grades a manuscript would turn a set of checkable verdicts into a
//! single figure nobody can check: the presentation-class defect CLAUDE.md
//! records for the upsell's `publication_probability`. This guard makes the
//! absence structural.
//!
//! # What is scanned
//!
//! Every type REACHABLE from each root, followed through its fields, among the
//! types defined in the scanned files:
//!
//! - `StatsConsistencyOutcome`, over `gaply-core/src/stats_l0/*.rs`;
//! - `StatsConsistencyView`, the one serialisable form, over the app crate's
//!   `src/stats_consistency.rs`, read as text (a `gaply_core` test cannot
//!   import an app-crate type);
//! - the TypeScript mirror the screen renders,
//!   `src/screens/checks/statsConsistencyTypes.ts`, every property name.
//!
//! For each type: its field names, including those of struct-like enum
//! variants, and any `rename = "…"` string, since a serde rename puts a
//! different name on the wire than the one in the source. A name is split into
//! words (`_` and camelCase), and a word that is, or begins with, a banned word
//! fails (`percentage`, `scores`).
//!
//! Variant NAMES are not checked, deliberately: the first run of a version that
//! checked them failed on `NumberNotRead::Percent`, which is the reason a
//! number printed as `12%` is refused, not a percentage anyone is shown.
//!
//! # What it cannot catch
//!
//! 1. **A type defined outside the scanned files** (`Rational`, `Location`).
//!    They are printed as unresolved, not followed.
//! 2. **A score under another name** (`rating`, `index`). The list is the one
//!    asked for; widen it here.
//! 3. **A score computed in the screen** from counts. The TS scan sees the
//!    types, not the components.

use std::collections::{BTreeMap, BTreeSet};

const BANNED: &[&str] = &["score", "percent", "probability", "confidence", "grade"];

const CORE_DIR: &str = "src/stats_l0";
const APP_VIEW: &str = "../src/stats_consistency.rs";
const TS_VIEW: &str = "../../src/screens/checks/statsConsistencyTypes.ts";

/// One `struct` or `enum` as the scan sees it.
#[derive(Debug, Default)]
struct TypeDef {
    /// Field names and `rename` strings.
    names: Vec<String>,
    /// Capitalised identifiers in the body: the types it refers to.
    refers_to: BTreeSet<String>,
}

fn code_only(src: &str) -> String {
    // Test modules define their own types; they are not the outcome.
    let src = src.split("#[cfg(test)]").next().unwrap_or(src);
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Identifiers in `s`, each with its byte offset.
fn idents(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices().chain(std::iter::once((s.len(), ' '))) {
        match (start, is_ident(c)) {
            (None, true) => start = Some(i),
            (Some(b), false) => {
                if !s[b..i].starts_with(|c: char| c.is_ascii_digit()) {
                    out.push((b, &s[b..i]));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// The text from `open` (a bracket) to its match, exclusive of both.
fn matched(code: &str, open: usize) -> Option<&str> {
    let (o, c) = match code.as_bytes()[open] {
        b'{' => (b'{', b'}'),
        b'(' => (b'(', b')'),
        _ => return None,
    };
    let mut depth = 0usize;
    for (i, b) in code.as_bytes()[open..].iter().enumerate() {
        if *b == o {
            depth += 1;
        } else if *b == c {
            depth -= 1;
            if depth == 0 {
                return Some(&code[open + 1..open + i]);
            }
        }
    }
    None
}

/// Every `struct` and `enum` in `code`, by name.
fn type_defs(code: &str) -> BTreeMap<String, TypeDef> {
    let mut out = BTreeMap::new();
    let ids = idents(code);
    for (k, &(_, kw)) in ids.iter().enumerate() {
        if kw != "struct" && kw != "enum" {
            continue;
        }
        let Some(&(at, name)) = ids.get(k + 1) else {
            continue;
        };
        let after = at + name.len();
        let Some(rel) = code[after..].find(['{', '(', ';']) else {
            continue;
        };
        let open = after + rel;
        let Some(body) = matched(code, open) else {
            continue;
        };
        let mut def = TypeDef::default();

        // Field names: an identifier followed by one `:` (not `::`, and not
        // the tail of a path).
        let b = body.as_bytes();
        for &(i, id) in &idents(body) {
            let end = i + id.len();
            let mut j = end;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let colon = j < b.len() && b[j] == b':' && b.get(j + 1) != Some(&b':');
            let after_path = i >= 1 && b[i - 1] == b':';
            if colon && !after_path && id.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
            {
                def.names.push(id.to_string());
            }
            if id.starts_with(|c: char| c.is_ascii_uppercase()) {
                def.refers_to.insert(id.to_string());
            }
        }
        // A serde rename is the name on the wire.
        for piece in body.split("rename = \"").skip(1) {
            if let Some(end) = piece.find('"') {
                def.names.push(piece[..end].to_string());
            }
        }
        out.insert(name.to_string(), def);
    }
    out
}

/// The words of an identifier: split on `_` and before each capital.
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in name.split('_') {
        let mut cur = String::new();
        for c in part.chars() {
            if c.is_ascii_uppercase() && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c.to_ascii_lowercase());
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

fn banned_word(name: &str) -> Option<&'static str> {
    words(name)
        .iter()
        .find_map(|w| BANNED.iter().copied().find(|b| w.starts_with(b)))
}

/// Walk from `root` through `defs`. Returns (visited types, banned names
/// found, unresolved type names).
fn walk(
    root: &str,
    defs: &BTreeMap<String, TypeDef>,
) -> (BTreeSet<String>, Vec<String>, BTreeSet<String>) {
    let (mut seen, mut bad, mut unresolved) = (BTreeSet::new(), Vec::new(), BTreeSet::new());
    let mut todo = vec![root.to_string()];
    while let Some(t) = todo.pop() {
        if !seen.insert(t.clone()) {
            continue;
        }
        let Some(def) = defs.get(&t) else {
            seen.remove(&t);
            unresolved.insert(t);
            continue;
        };
        for n in &def.names {
            if let Some(w) = banned_word(n) {
                bad.push(format!("{t}.{n} (`{w}`)"));
            }
        }
        todo.extend(def.refers_to.iter().filter(|r| !seen.contains(*r)).cloned());
    }
    (seen, bad, unresolved)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e} — has it moved?"))
}

#[test]
fn stats_outcome_has_no_score() {
    let mut core = BTreeMap::new();
    let mut files = 0;
    for e in std::fs::read_dir(CORE_DIR)
        .expect("stats_l0 is readable")
        .flatten()
    {
        if e.path().extension().is_some_and(|x| x == "rs") {
            files += 1;
            core.extend(type_defs(&code_only(&read(
                &e.path().display().to_string(),
            ))));
        }
    }
    assert!(files >= 13, "only {files} stats_l0 files read");
    let app = type_defs(&code_only(&read(APP_VIEW)));

    let mut bad = Vec::new();
    for (root, defs, at_least) in [
        ("StatsConsistencyOutcome", &core, 10),
        ("StatsConsistencyView", &app, 10),
    ] {
        let (seen, found, unresolved) = walk(root, defs);
        println!("{root}: {} types followed: {seen:?}", seen.len());
        println!("{root}: not followed (defined outside the scan, or a std type): {unresolved:?}");
        // The walk must reach the types it exists to check. Counted on
        // 10 Oct 2026; a parser that stops reading bodies would find one.
        assert!(
            seen.len() >= at_least,
            "{root}: only {} types reached: {seen:?}",
            seen.len()
        );
        bad.extend(found);
    }

    // The TypeScript mirror: every property name, `name:` or `name?:`.
    let ts = read(TS_VIEW);
    let mut ts_names = 0;
    for line in ts
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('*'))
    {
        for (i, id) in idents(line) {
            let rest = line[i + id.len()..].trim_start_matches('?');
            if rest.starts_with(':') {
                ts_names += 1;
                if let Some(w) = banned_word(id) {
                    bad.push(format!("{TS_VIEW}: `{id}` (`{w}`)"));
                }
            }
        }
    }
    assert!(
        ts_names >= 20,
        "only {ts_names} property names read from {TS_VIEW}"
    );

    assert!(
        bad.is_empty(),
        "Level 0 reports verdicts and counts, never a score. Found:\n{}",
        bad.join("\n")
    );
}

/// The scan must catch the shapes it exists for: a field, a field of a
/// struct-like variant, a serde rename, a camelCase property.
#[test]
fn the_guard_finds_a_score_in_every_shape() {
    let code = code_only(
        "pub struct Out { pub claims: usize, pub inner: Inner }\n\
         pub struct Inner { #[serde(rename = \"overallGrade\")] pub g: u8 }\n\
         pub enum V { Consistent, HighConfidence(u8), Other { percent_ok: f64 } }\n",
    );
    let defs = type_defs(&code);
    let (_, bad, _) = walk("Out", &defs);
    assert_eq!(
        bad,
        vec!["Inner.overallGrade (`grade`)".to_string()],
        "the rename, reached through a field"
    );
    let (_, bad, _) = walk("V", &defs);
    assert_eq!(
        bad,
        vec!["V.percent_ok (`percent`)".to_string()],
        "a variant's field, not its name"
    );
    assert_eq!(banned_word("publicationProbability"), Some("probability"));
    assert_eq!(banned_word("percentage"), Some("percent"));
    assert_eq!(banned_word("could_not_check"), None);
}
