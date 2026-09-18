//! **The `JournalFingerprint` — §7's three kinds of knowledge, read back.**
//!
//! Phase 3's backend writes four tables (`journal_requirements`,
//! `journal_conventions`, `journal_expectations`, `journal_standard_bindings`)
//! and migration 24's `journal_fingerprints` provenance row. Until now nothing
//! read them: the extraction side was complete and the display side had no
//! surface to render. This is that surface.
//!
//! # The three kinds stay three (§7)
//!
//! Requirements, conventions and expectations are separate fields of separate
//! types, assembled by three queries. There is no JOIN and no common row type.
//! §7's rule is that the three "must never be mixed", and the cheapest way to
//! honour it is to make a mixed record unrepresentable rather than discouraged:
//! a convention has `n` and no span, an expectation has a span and no status
//! vocabulary shared with a requirement, and neither can be constructed from
//! the other's row.
//!
//! # Two shapes the data forces, not design choices
//!
//! **A CONFLICTED fact shows both values and chooses neither.** The store
//! writes conflicting requirements as TWO rows sharing a `conflict_id`
//! (migration 23). A reader that returned one of them would be making the
//! choice §3.4 refuses, so `Conflict` groups them and carries no winner field
//! — there is nowhere to put a choice even if someone wanted to make one.
//!
//! **A requirement bound to no article type says NOT STATED.** `article_type`
//! is `Option<String>` because a journal frequently states a limit without
//! saying which submission it governs — Nature Medicine's `/nm/content` lists
//! "Analysis" and "Resource" sections whose headings the binder does not
//! recognise as article types. `None` means the journal did not say. It does
//! NOT mean the requirement applies to everything, and `ArticleTypeLabel`
//! exists so the distinction survives serialization to a renderer that would
//! otherwise be free to print "All".
//!
//! **No model touches this module.** It is SQL and structs; see
//! `tests/journal_display_is_llm_free.rs`.

use serde::Serialize;

use crate::journal_store::StoredRequirement;
use crate::{Database, GaplyError};

/// What the journal said about which submissions a requirement governs.
///
/// A three-state answer serialized as a tagged union, so a renderer cannot
/// collapse "not stated" into "all" by accident — there is no string to
/// default and no empty value to fall through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArticleTypeLabel {
    /// The journal named the article type this requirement governs.
    Stated { name: String },
    /// **The journal did not say.** Not "applies to everything".
    NotStated,
}

impl ArticleTypeLabel {
    pub fn from_option(v: Option<&str>) -> Self {
        match v.map(str::trim).filter(|s| !s.is_empty()) {
            Some(name) => ArticleTypeLabel::Stated { name: name.to_string() },
            None => ArticleTypeLabel::NotStated,
        }
    }
}

/// One requirement, with everything a reader needs to check it.
#[derive(Debug, Clone, Serialize)]
pub struct RequirementView {
    pub kind: String,
    pub value: String,
    pub article_type: ArticleTypeLabel,
    /// `verified` | `inferred` | `unavailable` | `conflicted`.
    pub status: String,
    pub source_url: String,
    pub source_heading: String,
    /// The journal's own sentence. Never empty — the schema refuses a row
    /// without one, and a requirement without its span cannot be checked.
    pub source_span: String,
}

/// Two requirements that disagree. **No winner field, by construction.**
#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub kind: String,
    pub article_type: ArticleTypeLabel,
    /// Both sides, in the order stored. A renderer shows both.
    pub values: Vec<RequirementView>,
}

/// A convention: what the journal's published papers DO. Never a rule.
#[derive(Debug, Clone, Serialize)]
pub struct ConventionView {
    pub metric: String,
    pub median: Option<f64>,
    pub iqr_low: Option<f64>,
    pub iqr_high: Option<f64>,
    /// The corpus size. `status = "inferred"` requires `n > 0` (migration 23).
    pub n: i64,
    /// Carries THE UNIT (§7 v6: pages, not words) and any scope caveat.
    pub detail: String,
    /// `inferred` | `unavailable`.
    pub status: String,
}

/// An expectation: what reviewers are asked to look for. Always cited.
#[derive(Debug, Clone, Serialize)]
pub struct ExpectationView {
    pub claim: String,
    pub frequency_k: Option<i64>,
    pub frequency_n: Option<i64>,
    pub status: String,
    pub source_url: String,
    pub source_span: String,
}

/// Which fingerprint an analysis used, and when it was fetched (item 11).
#[derive(Debug, Clone, Serialize)]
pub struct FingerprintProvenance {
    pub journal_key: String,
    pub version: i64,
    pub content_hash: String,
    pub fetched_at: i64,
    pub refetch_after: i64,
    pub source_count: i64,
    /// Present only when the fingerprint is quarantined; a UI that renders a
    /// quarantined fingerprint without saying so is showing poisoned input.
    pub quarantined_at: Option<i64>,
    pub quarantine_reason: Option<String>,
    /// **`crawled` or `bundled` — where this profile came from. §11 D186.**
    ///
    /// `fetched_at` says WHEN; this says WHO fetched it. A snapshot that shipped
    /// in the app and one this machine pulled from the journal answer different
    /// questions for a reader deciding whether to trust a word limit, and a
    /// screen that renders them identically has hidden the difference — the same
    /// reason the picker does not let a profiled journal look like an unprofiled
    /// one. A later crawl replaces the row and this value with it.
    pub origin: String,
}

/// §7's three parts plus provenance and the standard bindings.
#[derive(Debug, Clone, Serialize)]
pub struct JournalFingerprint {
    pub journal_key: String,
    /// `None` when the journal has never been crawled. A screen must render
    /// this state rather than an empty fingerprint that looks fetched.
    pub provenance: Option<FingerprintProvenance>,
    pub requirements: Vec<RequirementView>,
    pub conflicts: Vec<Conflict>,
    pub conventions: Vec<ConventionView>,
    pub expectations: Vec<ExpectationView>,
    pub standards: Vec<StandardBindingView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StandardBindingView {
    pub standard: String,
    pub design: String,
    pub source_url: String,
    pub source_span: String,
}

fn view(r: &StoredRequirement) -> RequirementView {
    RequirementView {
        kind: r.kind.as_str().to_string(),
        value: r.value.clone(),
        article_type: ArticleTypeLabel::from_option(r.article_type.as_deref()),
        status: r.status.clone(),
        source_url: r.source_url.clone(),
        source_heading: r.source_heading.clone(),
        source_span: r.source_span.clone(),
    }
}

/// Read the whole fingerprint for one journal.
///
/// Returns a fingerprint with `provenance: None` and empty vectors for a
/// journal that has never been crawled — never an error. "Nothing ingested" is
/// a state the picker must show (item 9), not a failure to handle.
pub fn fingerprint_for(
    db: &Database,
    journal_key: &str,
) -> Result<JournalFingerprint, GaplyError> {
    let stored = crate::journal_store::requirements_for(db, journal_key)?;

    // Conflicted rows share a conflict_id and are grouped; everything else is a
    // plain requirement. The grouping is what makes "show both, choose neither"
    // the only renderable option.
    let mut conflicts: Vec<Conflict> = Vec::new();
    let mut plain: Vec<RequirementView> = Vec::new();
    let mut groups: std::collections::BTreeMap<String, Vec<&StoredRequirement>> =
        std::collections::BTreeMap::new();
    for r in &stored {
        match &r.conflict_id {
            Some(id) => groups.entry(id.clone()).or_default().push(r),
            None => plain.push(view(r)),
        }
    }
    for (_, rows) in groups {
        let first = rows[0];
        conflicts.push(Conflict {
            kind: first.kind.as_str().to_string(),
            article_type: ArticleTypeLabel::from_option(first.article_type.as_deref()),
            values: rows.iter().map(|r| view(r)).collect(),
        });
    }

    let conn = db.conn()?;

    let mut stmt = conn.prepare(
        "SELECT metric, median, iqr_low, iqr_high, n, detail, status
           FROM journal_conventions WHERE journal_key = ?1 ORDER BY metric",
    )?;
    let conventions = stmt
        .query_map(rusqlite::params![journal_key], |row| {
            Ok(ConventionView {
                metric: row.get(0)?,
                median: row.get(1)?,
                iqr_low: row.get(2)?,
                iqr_high: row.get(3)?,
                n: row.get(4)?,
                detail: row.get(5)?,
                status: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT claim, frequency_k, frequency_n, status, source_url, source_span
           FROM journal_expectations WHERE journal_key = ?1 ORDER BY id",
    )?;
    let expectations = stmt
        .query_map(rusqlite::params![journal_key], |row| {
            Ok(ExpectationView {
                claim: row.get(0)?,
                frequency_k: row.get(1)?,
                frequency_n: row.get(2)?,
                status: row.get(3)?,
                source_url: row.get(4)?,
                source_span: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT standard, design, source_url, source_span
           FROM journal_standard_bindings WHERE journal_key = ?1 ORDER BY standard, design",
    )?;
    let standards = stmt
        .query_map(rusqlite::params![journal_key], |row| {
            Ok(StandardBindingView {
                standard: row.get(0)?,
                design: row.get(1)?,
                source_url: row.get(2)?,
                source_span: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let provenance = conn
        .query_row(
            "SELECT version, content_hash, fetched_at, refetch_after, source_count,
                    quarantined_at, quarantine_reason, origin
               FROM journal_fingerprints WHERE journal_key = ?1",
            rusqlite::params![journal_key],
            |row| {
                Ok(FingerprintProvenance {
                    journal_key: journal_key.to_string(),
                    version: row.get(0)?,
                    content_hash: row.get(1)?,
                    fetched_at: row.get(2)?,
                    refetch_after: row.get(3)?,
                    source_count: row.get(4)?,
                    quarantined_at: row.get(5)?,
                    quarantine_reason: row.get(6)?,
                    origin: row.get(7)?,
                })
            },
        )
        .ok();

    Ok(JournalFingerprint {
        journal_key: journal_key.to_string(),
        provenance,
        requirements: plain,
        conflicts,
        conventions,
        expectations,
        standards,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_extract::{ExtractedRequirement, RequirementKind};
    use crate::journal_store::store_requirements;

    fn db() -> Database {
        let d = Database::in_memory().unwrap();
        d.migrate().unwrap();
        d
    }

    fn req(kind: RequirementKind, value: &str, at: Option<&str>, span: &str) -> ExtractedRequirement {
        ExtractedRequirement {
            kind,
            value: value.into(),
            article_type: at.map(str::to_string),
            source_heading: "H".into(),
            source_span: span.into(),
        }
    }

    /// **A CONFLICTED FACT SHOWS BOTH VALUES AND CHOOSES NEITHER.**
    ///
    /// The store writes a disagreement as two rows sharing a `conflict_id`. The
    /// reader must surface them as one grouped conflict carrying both — not as
    /// two loose requirements (which a renderer would show as though the
    /// journal stated both happily) and not as one winner.
    #[test]
    fn a_conflicted_requirement_returns_both_values_and_no_winner() {
        let d = db();
        store_requirements(
            &d,
            "j",
            "https://j.test/a",
            &[req(RequirementKind::AbstractLimit, "250", Some("Article"), "Abstract - up to 250 words.")],
            1,
        )
        .unwrap();
        store_requirements(
            &d,
            "j",
            "https://j.test/b",
            &[req(RequirementKind::AbstractLimit, "300", Some("Article"), "Abstract - up to 300 words.")],
            1,
        )
        .unwrap();

        let fp = fingerprint_for(&d, "j").unwrap();
        assert_eq!(fp.conflicts.len(), 1, "{fp:#?}");
        let c = &fp.conflicts[0];
        let mut values: Vec<&str> = c.values.iter().map(|v| v.value.as_str()).collect();
        values.sort();
        assert_eq!(values, ["250", "300"], "both values must survive: {c:#?}");
        // Every side carries its own span, so a reader can check which page
        // said which.
        assert!(c.values.iter().all(|v| !v.source_span.is_empty()), "{c:#?}");
        assert!(c.values.iter().all(|v| v.status == "conflicted"), "{c:#?}");
        // And the conflicted rows are NOT also emitted as plain requirements,
        // which would show the disagreement as two independent facts.
        assert!(
            fp.requirements.iter().all(|r| r.kind != "abstract_limit"),
            "a conflicted row leaked into the plain list: {:#?}",
            fp.requirements
        );
    }

    /// **A REQUIREMENT BOUND TO NO ARTICLE TYPE SAYS NOT STATED.**
    ///
    /// The type is a tagged union rather than an `Option<String>` on the wire
    /// precisely so a renderer has nothing to default. There is no empty string
    /// to fall through to "All".
    #[test]
    fn an_unbound_requirement_says_not_stated_rather_than_all() {
        let d = db();
        store_requirements(
            &d,
            "j",
            "https://j.test/a",
            &[
                req(RequirementKind::WordLimit, "4000", None, "Main text - up to 4,000 words."),
                req(RequirementKind::WordLimit, "1000", Some("Correspondence"), "Main text - up to 1,000 words."),
            ],
            1,
        )
        .unwrap();

        let fp = fingerprint_for(&d, "j").unwrap();
        let unbound = fp.requirements.iter().find(|r| r.value == "4000").unwrap();
        assert_eq!(unbound.article_type, ArticleTypeLabel::NotStated, "{unbound:#?}");

        let bound = fp.requirements.iter().find(|r| r.value == "1000").unwrap();
        assert_eq!(
            bound.article_type,
            ArticleTypeLabel::Stated { name: "Correspondence".into() }
        );

        // On the wire the distinction is a discriminant, not an absence.
        let json = serde_json::to_string(&unbound.article_type).unwrap();
        assert_eq!(json, r#"{"kind":"not_stated"}"#);
        assert!(
            !json.contains("all") && !json.contains("every"),
            "NOT STATED must never serialize as a claim about scope: {json}"
        );
    }

    /// An empty-string article type is the same absence, not a named type.
    #[test]
    fn a_blank_article_type_is_not_stated() {
        assert_eq!(ArticleTypeLabel::from_option(Some("   ")), ArticleTypeLabel::NotStated);
        assert_eq!(ArticleTypeLabel::from_option(None), ArticleTypeLabel::NotStated);
    }

    /// A journal nobody has crawled is a STATE, not an error — the picker has
    /// to render it (item 9).
    #[test]
    fn an_uncrawled_journal_returns_an_empty_fingerprint_with_no_provenance() {
        let fp = fingerprint_for(&db(), "never-crawled").unwrap();
        assert!(fp.provenance.is_none());
        assert!(fp.requirements.is_empty() && fp.conflicts.is_empty());
        assert!(fp.conventions.is_empty() && fp.expectations.is_empty());
    }
}
