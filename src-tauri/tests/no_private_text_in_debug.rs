//! **No derived `Debug` prints the user's data, a header, a level, manuscript
//! text or a request body** (Phase 2 evaluation fixes, part C follow-up,
//! item 7; the deep evaluation's Minor "Debug derives on row-holding types are
//! unguarded").
//!
//! A derived `Debug` prints every field. Nothing in production formats these
//! types today, but one `tracing::debug!(?draft)` or one `{:?}` in an error
//! would put cells, level codes, headers or a manuscript sentence into a log,
//! and `~/gaply-dev.log` and a terminal both keep stdout.
//!
//! 1. **The scan.** In the Level 1, helper and app L1 files, every struct or
//!    enum with a DIRECT field typed `String`, a borrowed `&str`, `Vec<u8>` or
//!    `serde_json::Value` that derives `Debug` must be on [`CLOSED`], with the
//!    reason its text is not the user's (a closed code, a digest, a model id,
//!    a printed number, model or web text). Any other such type writes its own
//!    `Debug`, printing counts only. Direct fields are enough: a derived
//!    `Debug` on a container calls each field's own `Debug`, so once every
//!    type that HOLDS private text prints counts, nothing that contains one
//!    can print the text. The type list is read from the source, not typed
//!    here, so a new type with a text field is unclassified and fails.
//! 2. **The sentinels.** The main types are built with sentinel text and
//!    formatted with `{:?}` and `{:#?}`; no sentinel may appear.
//!
//! What it does not see: a `Debug` written by hand that prints text, and a
//! `{:?}` of a plain `String` holding data. It constrains what is DERIVED in
//! the scanned files, not every formatted value.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Types that derive `Debug` and have a direct text field whose text is not
/// the user's data, with why.
const CLOSED: &[(&str, &str)] = &[
    ("CitedSource", "a web source's URL, title and domain from OpenAI's reply"),
    ("EvidenceItem", "the web query, built from closed parts only"),
    ("ResultFieldsOut", "printed numbers and alphas, the result_explanation's consented fields"),
    ("RoleSlotOut", "a method's closed level rule"),
    ("NumberOut", "a printed number and its interval"),
    ("ResultExplanationOutbound", "printed alphas"),
    ("RawCitation", "a citation from OpenAI's reply"),
    ("ModelOutput", "the model's text"),
    ("ModelText", "the model's text"),
    ("Word", "a word of the model's text"),
    ("WebQuery", "the web query, built from closed parts only"),
    ("HelperDiagnostic", "the model id and OpenAI's request id"),
    ("TaskSpec", "the bundled task spec"),
    ("SpecFile", "the bundled task spec"),
    ("BindingDraft", "the method id; its levels are in LevelSelection and IncludeFilter, which print counts"),
    ("BindingProposal", "a fingerprint digest"),
    ("ConfirmedBinding", "digests"),
    ("UserP", "a printed p as typed"),
    ("DataFingerprint", "the SHA-256 and the parser version; its sheet is in TableRef, which prints no name"),
    ("AliasedColumn", "a column alias such as C3"),
    ("AlphaUsed", "printed alphas"),
    ("TokenRecord", "digests and the method id"),
    ("AiSettings", "the model id"),
    ("L1ProposalView", "the method id"),
    ("L1WholeNumberView", "a column alias"),
    ("L1RoleView", "column aliases"),
    ("L1RevisionView", "a revision digest"),
    ("L1SourceView", "a web source's URL and title from OpenAI's reply"),
    ("L1ContextItemView", "the model's sentence and the closed web query"),
    ("L1SuggestedRoleView", "a column alias"),
    ("L1HelperResultView", "the model's text and reasons"),
    ("L1SuspectCodeView", "a column alias and a numeric code"),
    ("AiKeyTestView", "a closed result word"),
    ("RoleEdit", "a closed role word"),
    ("BindingEdits", "the method id and a closed variant word; its levels are in LevelEdit and FilterEdit, which print counts"),
    ("AlphaView", "printed alphas"),
    ("ProvenanceView", "an inputs digest"),
    ("NumberView", "a printed number"),
    ("ModelId", "the model id setting"),
    ("RevisionHash", "a claim revision digest"),
];

fn rust_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["gaply-core/src/stats_l1", "gaply-core/src/stats_assist"] {
        for e in std::fs::read_dir(Path::new(ROOT).join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    for f in [
        "src/stats_l1_commands.rs",
        "src/stats_l1_ingest.rs",
        "src/stats_ai.rs",
        "src/supplementary.rs",
        "src/stats_reproduction.rs",
    ] {
        out.push(Path::new(ROOT).join(f));
    }
    out.sort();
    out
}

/// A field type that can hold free text: `String`, a borrowed `&str` (not
/// `&'static str`, which is a literal), `Vec<u8>` or a JSON value.
fn holds_text(line: &str) -> bool {
    let words = |w: &str| {
        line.split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|x| x == w)
    };
    let borrowed_str = line.match_indices("str").any(|(i, _)| {
        let before = &line[..i];
        let after = line[i + 3..].chars().next();
        !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
            && (before.ends_with('&')
                || (before
                    .trim_end()
                    .ends_with(|c: char| c.is_alphanumeric() || c == '_')
                    && before.contains("&'")
                    && !before.trim_end().ends_with("'static")))
    });
    words("String")
        || borrowed_str
        || line.contains("Vec<u8>")
        || line.contains("serde_json::Value")
}

/// Every struct or enum before a file's tests: whether it derives `Debug`,
/// and whether a direct field holds text.
fn scan(path: &Path) -> Vec<(String, bool, bool, usize)> {
    let src = std::fs::read_to_string(path).unwrap();
    // Stop at the test MODULE: a `#[cfg(test)]` on a single item mid-file
    // (claim.rs and dataset.rs have one) must not hide the rest of the file.
    let all: Vec<&str> = src.lines().collect();
    let end = (0..all.len())
        .find(|&i| {
            all[i].trim() == "#[cfg(test)]"
                && all.get(i + 1).is_some_and(|l| {
                    l.trim_start().starts_with("mod ") || l.trim_start().starts_with("pub mod ")
                })
        })
        .unwrap_or(all.len());
    let lines = &all[..end];
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        let t = t
            .strip_prefix("pub(crate) ")
            .or_else(|| t.strip_prefix("pub "))
            .unwrap_or(t);
        let kind = ["struct ", "enum "].iter().find(|k| t.starts_with(**k));
        if let Some(k) = kind {
            let name: String = t[k.len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let mut j = i;
            let mut derives = false;
            while j > 0 {
                let a = lines[j - 1].trim();
                if !(a.starts_with("#[") || a.starts_with("//")) {
                    break;
                }
                if a.starts_with("#[derive(")
                    && a.split(|c: char| !c.is_alphanumeric())
                        .any(|w| w == "Debug")
                {
                    derives = true;
                }
                j -= 1;
            }
            // The body: a tuple struct's parentheses on its line, or braces.
            let mut text = false;
            let mut depth = 0i32;
            let mut k = i;
            loop {
                let l = lines[k];
                let code = l.split("//").next().unwrap();
                if k == i {
                    if let Some(open) = code
                        .find('(')
                        .filter(|o| code.find('{').map_or(true, |b| *o < b))
                    {
                        text |= holds_text(&code[open..]);
                    }
                } else {
                    text |= holds_text(code);
                }
                depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
                if (depth <= 0 && k > i)
                    || (k == i && code.trim_end().ends_with(';'))
                    || k + 1 == lines.len()
                {
                    break;
                }
                k += 1;
            }
            out.push((name, derives, text, i + 1));
            i = k;
        }
        i += 1;
    }
    out
}

#[test]
fn every_type_holding_private_text_prints_counts_only() {
    let closed: BTreeMap<&str, &str> = CLOSED.iter().copied().collect();
    let (mut scanned, mut closed_seen, mut bad) = (0, Vec::new(), Vec::new());
    for f in rust_files() {
        let rel = f
            .strip_prefix(ROOT)
            .unwrap()
            .display()
            .to_string()
            .replace('\\', "/");
        for (name, derives, text, line) in scan(&f) {
            scanned += 1;
            if derives && text {
                if closed.contains_key(name.as_str()) {
                    closed_seen.push(name);
                } else {
                    bad.push(format!("{rel}:{line} {name}"));
                }
            }
        }
    }
    assert!(
        scanned > 150,
        "only {scanned} types scanned: the guard is silently inert"
    );
    assert!(bad.is_empty(), "derived Debug on a type with a private-text field; write a Debug that prints counts only, or add it to CLOSED with a reason:\n{}", bad.join("\n"));
    let stale: Vec<&&str> = closed
        .keys()
        .filter(|n| !closed_seen.iter().any(|s| s == **n))
        .collect();
    assert!(
        stale.is_empty(),
        "CLOSED names types that no longer derive Debug with a text field: {stale:?}"
    );
}

/* ---- sentinels ------------------------------------------------------------ */

const SECRETS: [&str; 5] = [
    "CELL_SECRET_d1",
    "HEADER_SECRET_d2",
    "LEVEL_SECRET_d3",
    "SENTENCE_SECRET_d4",
    "SHEET_SECRET_d5",
];

fn assert_clean(what: &str, s: String) {
    for secret in SECRETS {
        assert!(!s.contains(secret), "{what} printed {secret}: {s}");
    }
}

#[test]
fn the_main_types_print_no_sentinel() {
    use gaply_core::stats_l1::binding::{BindingDraft, IncludeFilter, LevelSelection};
    use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, VariantChoice, VariantKind};
    use gaply_core::stats_l1::claim::{
        ClaimId, ClaimLocation, ClaimOrigin, L1Claim, UserClaim, UserP,
    };
    use gaply_core::stats_l1::dataset::{
        build_dataset, Cell, ColumnAlias, DataFingerprint, MissingRule, RawTable, RawVariable,
        TableRef,
    };

    let csv = b"HEADER_SECRET_d2,group\n1.5,LEVEL_SECRET_d3\n2.5,b\nCELL_SECRET_d1,b\n";
    let ev = app_lib::supplementary::parse_supplementary_bytes(csv, "csv").unwrap();
    for s in [
        format!("{ev:?}"),
        format!("{ev:#?}"),
        format!("{:?}", ev.tables[0]),
    ] {
        assert_clean("SupplementaryEvidence", s);
    }
    let mut raw = RawTable::from_rows(ev.tables[0].headers.clone(), ev.tables[0].rows.clone());
    raw.variables = vec![
        RawVariable {
            user_missing: vec![MissingRule::Text("CELL_SECRET_d1".into())],
            value_labels: vec![("1".into(), "LEVEL_SECRET_d3".into())],
            declared_text: None,
        },
        RawVariable::default(),
    ];
    assert_clean("RawTable", format!("{raw:?} {raw:#?}"));
    let table = TableRef::Sheet {
        index: 0,
        name: "SHEET_SECRET_d5".into(),
    };
    assert_clean("TableRef", format!("{table:?}"));
    let d = build_dataset(raw, DataFingerprint::of_bytes(csv, "v", table)).unwrap();
    assert_clean("Dataset", format!("{d:?} {d:#?} {:?}", d.schema));
    assert_clean("Cell", format!("{:?}", Cell::Text("CELL_SECRET_d1".into())));

    let u = UserClaim {
        family: gaply_core::stats_l0::result::Family::T,
        statistic: "2.10",
        df: vec!["20"],
        n: None,
        p: UserP::Reported {
            raw: ".040",
            relation: gaply_core::stats_l0::number::Relation::Eq,
        },
        location: ClaimLocation {
            section_index: 0,
            section: gaply_core::extract::SectionKind::Results,
            paragraph: 0,
            sentence: 0,
        },
        sentence: Some("SENTENCE_SECRET_d4 t(20) = 2.10, p = .040."),
    };
    assert_clean("UserClaim", format!("{u:?}"));
    let c = L1Claim::user_entered(ClaimId(0), &u, ClaimOrigin::UserAdded).unwrap();
    assert_clean("L1Claim", format!("{c:?} {c:#?}"));

    let draft = BindingDraft::new(
        L1MethodId::IndependentT,
        VariantChoice::not_stated(VariantKind::T),
    )
    .role(RoleName::Outcome, &[ColumnAlias(1)])
    .role(RoleName::Group, &[ColumnAlias(2)]);
    let mut draft = draft;
    draft.selected_levels.push(LevelSelection {
        role: RoleName::Group,
        levels: vec!["LEVEL_SECRET_d3".into()],
    });
    draft.filters.push(IncludeFilter {
        column: ColumnAlias(2),
        levels: vec!["LEVEL_SECRET_d3".into()],
    });
    assert_clean("BindingDraft", format!("{draft:?} {draft:#?}"));
}
