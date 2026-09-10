//! The IPC wire contract, enforced from the COMMAND SURFACE itself (§11 D103).
//!
//! # Why this file exists, given that `ai::event_wire_tests` already existed
//!
//! §11 D53 recorded that `#[serde(rename_all)]` on an enum renames VARIANTS and
//! not fields, and built a test to end the class — including what it described
//! as a general rule, so that a new field would fail even if nobody remembered
//! to add a case.
//!
//! `FetchReport` then shipped with exactly that bug (§11 D103): `document_id`
//! reached the webview in snake_case, the Document card could never leave "No
//! document linked" after a successful fetch, and the warning that a fetched
//! abstract contained text aimed at the model became permanently unreachable.
//!
//! The guard did not fire, for two reasons worth stating plainly:
//!
//! 1. It was general over FIELDS, not over TYPES. The rule iterated a
//!    hand-written list of eight constructed values from three enums, and a
//!    type absent from that list was checked by nothing.
//! 2. Its scope was *streamed events*. `FetchReport` is a command's RETURN
//!    value. It crosses the identical boundary into the identical kind of
//!    hand-written TypeScript reader, and was never in scope.
//!
//! **The boundary is not "events". It is everything a `#[tauri::command]`
//! returns or streams.** So this file does not hold a list. It parses this
//! crate's own source, enumerates the wire types from the command signatures,
//! and checks them. Adding a command with a new return type extends the checked
//! set automatically; there is nothing to remember.
//!
//! # The rule, and why it is not "no underscores may cross"
//!
//! That was the obvious rule and it is the wrong one here. Applied to the
//! reachable set it flags 248 fields across 174 types, because much of this
//! codebase is snake_case on BOTH sides and the two agree perfectly. A guard
//! that reports 248 non-bugs is a guard someone switches off.
//!
//! The defect is never "an underscore crossed". It is **the Rust type and the
//! TypeScript reader disagreeing**. The tractable, zero-false-positive form of
//! that is internal coherence:
//!
//! > If a type DECLARES `rename_all = "camelCase"`, its fields must actually
//! > come out camelCase.
//!
//! A type that asks for camelCase and emits `document_id` is incoherent whether
//! or not anyone currently reads that field, so this needs no exemption list —
//! and an exemption list is the thing that just failed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use quote::ToTokens;
use syn::{Fields, FnArg, Item, ReturnType, Type};

/// Both crates in the workspace. `gaply-core` is included because command
/// return types are routinely defined there (`ImportPreflight`, `StoredReference`,
/// `ThesisAuditPreview`), and a type is no less on the wire for living a crate
/// away.
fn source_roots() -> Vec<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    vec![manifest.join("src"), manifest.join("gaply-core").join("src")]
}

fn rust_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for r in source_roots() {
        walk(&r, &mut out);
    }
    out.sort();
    out
}

/// One type definition, with the attribute text already delimited by the parser
/// rather than guessed at with a regex.
struct TypeDef {
    is_enum: bool,
    derives_serialize: bool,
    /// Concatenated `#[serde(...)]` arguments on the TYPE.
    serde: String,
    /// Struct fields, or the fields of every struct-variant of an enum, as
    /// `(variant_or_empty, field_name, field_level_serde_args)`.
    fields: Vec<(String, String, String)>,
    file: String,
}

fn attr_text(attrs: &[syn::Attribute], want: &str) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident(want))
        .map(|a| a.to_token_stream().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn derives_serialize(attrs: &[syn::Attribute]) -> bool {
    attr_text(attrs, "derive").contains("Serialize")
}

fn named_fields(fields: &Fields, variant: &str, out: &mut Vec<(String, String, String)>) {
    if let Fields::Named(named) = fields {
        for f in &named.named {
            if let Some(id) = &f.ident {
                out.push((variant.to_string(), id.to_string(), attr_text(&f.attrs, "serde")));
            }
        }
    }
}

/// Every struct/enum in both crates, indexed by name. Nested `mod` blocks are
/// walked, so a type declared inside an inline module is not invisible.
fn type_index() -> BTreeMap<String, TypeDef> {
    let mut index = BTreeMap::new();
    let mut unparsed = Vec::new();

    fn collect(items: &[Item], file: &str, index: &mut BTreeMap<String, TypeDef>) {
        for item in items {
            match item {
                Item::Struct(s) => {
                    let mut fields = Vec::new();
                    named_fields(&s.fields, "", &mut fields);
                    index.entry(s.ident.to_string()).or_insert(TypeDef {
                        is_enum: false,
                        derives_serialize: derives_serialize(&s.attrs),
                        serde: attr_text(&s.attrs, "serde"),
                        fields,
                        file: file.to_string(),
                    });
                }
                Item::Enum(e) => {
                    let mut fields = Vec::new();
                    for v in &e.variants {
                        named_fields(&v.fields, &v.ident.to_string(), &mut fields);
                    }
                    index.entry(e.ident.to_string()).or_insert(TypeDef {
                        is_enum: true,
                        derives_serialize: derives_serialize(&e.attrs),
                        serde: attr_text(&e.attrs, "serde"),
                        fields,
                        file: file.to_string(),
                    });
                }
                Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        collect(inner, file, index);
                    }
                }
                _ => {}
            }
        }
    }

    for path in rust_files() {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let name = path.display().to_string();
        match syn::parse_file(&text) {
            Ok(f) => collect(&f.items, &name, &mut index),
            // Never skip silently: a file this guard cannot read is a hole in
            // the guard, which is the failure mode being fixed.
            Err(e) => unparsed.push(format!("{name}: {e}")),
        }
    }
    assert!(unparsed.is_empty(), "the wire guard could not parse:\n  {}", unparsed.join("\n  "));
    index
}

/* ------------------------------------------------------------------------ *
 *  THE RULE: a type that declares camelCase must actually emit camelCase.
 * ------------------------------------------------------------------------ */

#[test]
fn a_type_that_declares_camel_case_must_actually_emit_it() {
    let index = type_index();
    let mut violations = Vec::new();

    for (name, def) in &index {
        if !def.derives_serialize || !def.serde.contains("rename_all = \"camelCase\"") {
            continue;
        }
        // On a STRUCT, `rename_all` does rename the fields — nothing to check.
        // On an ENUM it renames the VARIANTS only; struct-variant fields keep
        // their Rust spelling unless `rename_all_fields` says otherwise. That
        // asymmetry is the entire bug, twice over (§11 D53, §11 D103).
        if !def.is_enum || def.serde.contains("rename_all_fields") {
            continue;
        }
        for (variant, field, field_serde) in &def.fields {
            if field.contains('_') && !field_serde.contains("rename") {
                violations.push(format!(
                    "{name}::{variant}.{field} → serializes as `{field}` while {name} declares \
                     camelCase ({})",
                    def.file
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "these types ask for camelCase and do not get it — add \
         `rename_all_fields = \"camelCase\"` to the enum (§11 D103):\n  {}",
        violations.join("\n  ")
    );
}

/* ------------------------------------------------------------------------ *
 *  THE SURFACE: what has to be covered, computed rather than remembered.
 * ------------------------------------------------------------------------ */

/// Type names that are not wire types of ours: primitives, std containers, the
/// error type, and Tauri's own transport. Everything else must RESOLVE to a
/// definition this guard can see — an unknown name fails rather than passes,
/// because failing open is how the previous guard missed `FetchReport`.
const NOT_A_WIRE_TYPE: &[&str] = &[
    "Result", "Option", "Vec", "Box", "String", "str", "bool", "char", "static", "usize", "u8",
    "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64", "HashMap", "BTreeMap", "PathBuf",
    "Value", "GaplyError", "State", "Channel", "AppHandle", "Window", "Response", "Duration",
];

/// Collect every type name mentioned in a type expression.
fn idents_in(ty: &Type) -> BTreeSet<String> {
    ty.to_token_stream()
        .to_string()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty() && s.chars().next().is_some_and(|c| c.is_uppercase()))
        .map(|s| s.to_string())
        .collect()
}

/// Every type a `#[tauri::command]` returns or streams, read off the signatures.
fn command_surface() -> BTreeSet<String> {
    let mut roots = BTreeSet::new();

    fn collect(items: &[Item], roots: &mut BTreeSet<String>) {
        for item in items {
            match item {
                Item::Fn(f) => {
                    let is_command = f
                        .attrs
                        .iter()
                        .any(|a| a.to_token_stream().to_string().contains("tauri :: command"));
                    if !is_command {
                        continue;
                    }
                    if let ReturnType::Type(_, ty) = &f.sig.output {
                        roots.extend(idents_in(ty));
                    }
                    // A streamed event is as much on the wire as a return value
                    // — and is the half the previous guard covered.
                    for arg in &f.sig.inputs {
                        if let FnArg::Typed(pat) = arg {
                            let text = pat.ty.to_token_stream().to_string();
                            if text.contains("Channel") {
                                roots.extend(idents_in(&pat.ty));
                            }
                        }
                    }
                }
                Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        collect(inner, roots);
                    }
                }
                _ => {}
            }
        }
    }

    for path in rust_files() {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if let Ok(f) = syn::parse_file(&text) {
            collect(&f.items, &mut roots);
        }
    }
    roots.retain(|r| !NOT_A_WIRE_TYPE.contains(&r.as_str()));
    roots
}

/// `use gaply_core::stats_verdict::VerificationReport as StatsVerificationReport;`
/// means the signature names a type the index holds under a different key. That
/// is a gap in this guard, not a type nobody checks — so it is resolved rather
/// than allowlisted. Allowlisting a name it could not see is precisely how the
/// previous guard came to be trusted while checking nothing.
fn use_aliases() -> BTreeMap<String, String> {
    fn collect(tree: &syn::UseTree, out: &mut BTreeMap<String, String>) {
        match tree {
            syn::UseTree::Rename(r) => {
                out.insert(r.rename.to_string(), r.ident.to_string());
            }
            syn::UseTree::Path(p) => collect(&p.tree, out),
            syn::UseTree::Group(g) => {
                for t in &g.items {
                    collect(t, out);
                }
            }
            _ => {}
        }
    }
    let mut out = BTreeMap::new();
    for path in rust_files() {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if let Ok(f) = syn::parse_file(&text) {
            for item in &f.items {
                if let Item::Use(u) = item {
                    collect(&u.tree, &mut out);
                }
            }
        }
    }
    out
}

#[test]
fn every_command_wire_type_resolves_to_a_definition_this_guard_can_see() {
    let index = type_index();
    let surface = command_surface();
    let aliases = use_aliases();

    // The surface is computed. If this collapses, the extraction broke and the
    // rule above would be checking an empty set while reporting success.
    assert!(
        surface.len() > 40,
        "only {} wire types found — the command-surface scan is broken, and a \
         guard over an empty set passes for the wrong reason",
        surface.len()
    );
    // The type whose drift motivated this file must be in the computed set.
    assert!(surface.contains("FetchReport"), "FetchReport is not in the computed surface");

    let unresolved: Vec<&String> = surface
        .iter()
        .filter(|n| {
            !index.contains_key(*n)
                && !aliases.get(*n).is_some_and(|target| index.contains_key(target))
        })
        .collect();
    assert!(
        unresolved.is_empty(),
        "these types cross the IPC boundary but this guard cannot see their \
         definition, so nothing is checking them (§11 D103): {unresolved:?}"
    );
}

/* ------------------------------------------------------------------------ *
 *  And the exact bytes, for the type whose fields the UI branches on.
 * ------------------------------------------------------------------------ */

/// The coherence rule above cannot catch everything: renaming the TAG, or
/// dropping a field, keeps a type internally coherent while breaking every
/// reader. So the shape the Document card actually reads is pinned key by key.
/// This is the assertion whose absence let §11 D103 ship.
#[test]
fn the_open_access_fetch_report_is_camel_case_key_by_key() {
    use crate::oa_fetch::{FetchOutcome, FetchReport, FetchSubject};
    use gaply_core::oa_fetch::OaSource;

    let fetched = FetchReport {
        subject: FetchSubject::Citation { citation_id: "cite-x".into() },
        title: Some("SMOTE".into()),
        outcome: FetchOutcome::Fetched {
            document_id: 13,
            chunks_indexed: 113,
            chunks_embedded: 113,
            checkable: true,
            source: OaSource::OpenAlex,
            license: Some("cc-by".into()),
        },
    };
    assert_eq!(
        serde_json::to_value(&fetched).unwrap(),
        serde_json::json!({
            "subject": { "kind": "citation", "citationId": "cite-x" },
            "title": "SMOTE",
            "outcome": "fetched",
            "documentId": 13,
            "chunksIndexed": 113,
            "chunksEmbedded": 113,
            "checkable": true,
            // `OaSource` declares snake_case and is coherent with itself; no
            // TypeScript reads it. Pinned as it actually is, not as the
            // surrounding type's convention would suggest — the point of this
            // file is to record the contract, not to impose a house style.
            "source": "open_alex",
            "license": "cc-by",
        }),
        "DocumentRow reads report.documentId to leave its \"No document linked\" \
         state; it read undefined for the whole of §11 D103"
    );

    // The abstract arm carries the injection flag, which is a SAFETY sentence:
    // it was silently unreachable for as long as the spelling was wrong.
    let abstract_only = FetchReport {
        subject: FetchSubject::Citation { citation_id: "cite-y".into() },
        title: None,
        outcome: FetchOutcome::AbstractOnly {
            document_id: 12,
            chunks_indexed: 1,
            chunks_embedded: 1,
            checkable: true,
            source: OaSource::OpenAlex,
            injection_flagged: true,
        },
    };
    let v = serde_json::to_value(&abstract_only).unwrap();
    assert_eq!(v["outcome"], "abstractOnly");
    assert_eq!(v["injectionFlagged"], true);
    assert_eq!(v["documentId"], 12);

    // And the arm the retry advice is computed from.
    let limited = FetchReport {
        subject: FetchSubject::Citation { citation_id: "cite-z".into() },
        title: None,
        outcome: FetchOutcome::RateLimited { retry_after_secs: 30 },
    };
    assert_eq!(serde_json::to_value(&limited).unwrap()["retryAfterSecs"], 30);
}
