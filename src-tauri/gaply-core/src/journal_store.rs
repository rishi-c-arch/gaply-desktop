//! **Persisting the fingerprint's requirements — and detecting conflicts as
//! they arrive.**
//!
//! Prompt 5 item 2: *"when two sources state different values for one
//! requirement (abstract 250 on one page, 300 on another), store BOTH, mark the
//! fact CONFLICTED, show both to the user, choose neither."*
//!
//! # Why this module exists at all, stated plainly
//!
//! §3.4's biggest correction was that `journal_guidelines` had *"0 rows after
//! 27 manuscripts"* and that the count was **a count of a table nothing writes
//! to**. Migration 23 added four more tables in exactly that state. This is the
//! writer, and it exists now rather than later because a schema with no
//! producer is the same artefact §3.4 spent a page correcting.
//!
//! # Conflict detection is a WRITE-TIME decision, not a query-time one
//!
//! A conflict is a fact about the journal, discovered when a second source
//! disagrees with a first. Detecting it at read time would mean every reader
//! re-deriving it, and any reader that forgot would silently show one value as
//! though it were uncontested — which is the outcome Prompt 5 forbids.
//!
//! So the write path marks BOTH rows `conflicted` and gives them a shared
//! `conflict_id`. The schema has no column that could record a preference
//! (`migrations` v23 pins that), so nothing downstream can resolve it either.
//!
//! **What counts as a conflict, precisely:** two rows for the same journal,
//! the same `kind`, and the same `article_type` — including both being unbound
//! — whose `value` differs. An abstract limit for *Article* and one for *Brief
//! Communication* are not in conflict; they are two requirements, and treating
//! them as a disagreement is the failure `journal_extract`'s article-type
//! binding exists to prevent.
//!
//! **What is NOT a conflict:** the same value from two pages. That is
//! corroboration. Both rows are kept — each quotes its own sentence, and a
//! reader asking "where does the journal say this?" deserves both answers —
//! but neither is marked conflicted.

use rusqlite::params;

use crate::error::GaplyError;
use crate::journal_corpus::{ConventionStatus, DerivedConvention};
use crate::journal_extract::{ExtractedRequirement, RequirementKind};
use crate::Database;

/// What a store call did. Every number is a count of rows, not of intentions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoreOutcome {
    /// Rows written by this call.
    pub inserted: usize,
    /// Rows skipped because this exact requirement from this exact page was
    /// already stored — a re-crawl is not new evidence.
    pub duplicates: usize,
    /// Distinct facts that became conflicted during this call.
    pub new_conflicts: usize,
    /// Rows carrying a conflicted status after this call, including rows that
    /// were already stored and got re-marked.
    pub conflicted_rows: usize,
}

/// The status a stored requirement carries. Mirrors the schema CHECK.
fn status_str(conflicted: bool) -> &'static str {
    // Everything this writer stores was read from the journal's own page with
    // its sentence recorded, so `verified` is the only non-conflicted state it
    // can produce. `inferred` belongs to the corpus lane and `unavailable` to a
    // search that found nothing — neither is this path's to emit.
    if conflicted {
        "conflicted"
    } else {
        "verified"
    }
}

fn article_key(a: &Option<String>) -> String {
    // `None` is its own bucket, not a wildcard: an unbound requirement
    // conflicts with another unbound one, never with a bound one.
    a.clone().unwrap_or_else(|| "\u{0}unbound".to_string())
}

/// Store one page's extracted requirements, detecting conflicts against what
/// is already there.
pub fn store_requirements(
    db: &Database,
    journal_key: &str,
    source_url: &str,
    reqs: &[ExtractedRequirement],
    fetched_at: i64,
) -> Result<StoreOutcome, GaplyError> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    let mut out = StoreOutcome::default();

    for r in reqs {
        // A re-crawl of the same page is not new evidence.
        let already: i64 = tx.query_row(
            "SELECT count(*) FROM journal_requirements
              WHERE journal_key = ?1 AND kind = ?2 AND value = ?3 AND source_url = ?4
                AND ifnull(article_type, '') = ifnull(?5, '')",
            params![journal_key, r.kind.as_str(), r.value, source_url, r.article_type],
            |row| row.get(0),
        )?;
        if already > 0 {
            out.duplicates += 1;
            continue;
        }

        // Does anything already stored disagree? Same journal, same kind, same
        // article type, different value.
        let mut stmt = tx.prepare(
            "SELECT id, value, conflict_id FROM journal_requirements
              WHERE journal_key = ?1 AND kind = ?2
                AND ifnull(article_type, '') = ifnull(?3, '')",
        )?;
        let existing: Vec<(i64, String, Option<String>)> = stmt
            .query_map(params![journal_key, r.kind.as_str(), r.article_type], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        // **Only a SINGLE-VALUED kind can disagree with itself.** A journal
        // binds CONSORT to trials and PRISMA to systematic reviews; the second
        // does not contradict the first. See
        // `RequirementKind::is_single_valued` for what this cost before it
        // existed.
        let disagreeing: Vec<&(i64, String, Option<String>)> = if r.kind.is_single_valued() {
            existing.iter().filter(|(_, v, _)| *v != r.value).collect()
        } else {
            Vec::new()
        };

        let conflict_id = if disagreeing.is_empty() {
            None
        } else {
            // Reuse an existing conflict id so a third value joins the same
            // group rather than starting a second one.
            let id = disagreeing
                .iter()
                .find_map(|(_, _, c)| c.clone())
                .unwrap_or_else(|| {
                    out.new_conflicts += 1;
                    format!("cf-{journal_key}-{}-{}", r.kind.as_str(), article_key(&r.article_type))
                });
            // Re-mark every row already stored for this fact. A value that was
            // uncontested a moment ago is not uncontested now.
            for (row_id, _, _) in &existing {
                let n = tx.execute(
                    "UPDATE journal_requirements SET status = 'conflicted', conflict_id = ?2
                      WHERE id = ?1",
                    params![row_id, id],
                )?;
                out.conflicted_rows += n;
            }
            Some(id)
        };

        tx.execute(
            "INSERT INTO journal_requirements
               (journal_key, kind, value, article_type, status, source_url, source_heading,
                source_span, extracted_by, conflict_id, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pattern', ?9, ?10)",
            params![
                journal_key,
                r.kind.as_str(),
                r.value,
                r.article_type,
                status_str(conflict_id.is_some()),
                source_url,
                r.source_heading,
                r.source_span,
                conflict_id,
                fetched_at,
            ],
        )?;
        out.inserted += 1;
        if conflict_id.is_some() {
            out.conflicted_rows += 1;
        }
    }

    tx.commit()?;
    Ok(out)
}

/// Store a journal's convention profile.
///
/// **Every metric is written, including the `Unavailable` ones.** §3.4 requires
/// absence to be a status rather than a silence, and a table that held only the
/// computable metrics could not distinguish "we looked and the source does not
/// carry it" from "nobody asked". The schema enforces the other half: an
/// `inferred` row with `n = 0` is unstorable (`migrations` v23).
pub fn store_conventions(
    db: &Database,
    journal_key: &str,
    corpus_run_id: &str,
    conventions: &[DerivedConvention],
    computed_at: i64,
) -> Result<usize, GaplyError> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    // A convention profile REPLACES its predecessor for a run: a journal has
    // one current profile, and keeping every run would make "the median" a
    // question about which row a reader picked.
    tx.execute("DELETE FROM journal_conventions WHERE journal_key = ?1", params![journal_key])?;
    let mut n = 0usize;
    for c in conventions {
        tx.execute(
            "INSERT INTO journal_conventions
               (journal_key, metric, median, iqr_low, iqr_high, n, detail, status,
                corpus_run_id, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                journal_key,
                c.metric.as_str(),
                c.median,
                c.iqr_low,
                c.iqr_high,
                c.n as i64,
                c.detail,
                match c.status {
                    ConventionStatus::Inferred => "inferred",
                    ConventionStatus::Unavailable => "unavailable",
                },
                corpus_run_id,
                computed_at,
            ],
        )?;
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}

/// Store a journal's reporting-standard bindings.
///
/// # This table had a READER AND NO WRITER — §11 D170
///
/// `journal_fingerprint::fingerprint_for` has queried
/// `journal_standard_bindings` since it was written, and until now the only
/// things that ever inserted a row were migration 23 and one unit test.
/// `journal_standards::bindings_from` produced the values and every caller
/// dropped them on the floor, so a fingerprint's `standards` field was
/// permanently empty and looked merely unpopulated.
///
/// **That is the exact defect this module's own header says it exists to
/// prevent** — *"a schema with no producer is the same artefact §3.4 spent a
/// page correcting"* — and it sat in two of the four tables this module serves.
/// `tests/journal_tables_have_writers.rs` is what now makes the rule cost
/// something.
///
/// # Idempotent by `UNIQUE (journal_key, design, standard)`
///
/// A crawl re-run must not multiply rows, and a second page stating the same
/// binding is corroboration rather than a new fact — the same position
/// [`store_requirements`] takes. `INSERT OR IGNORE` keeps the FIRST span,
/// because the span is evidence and the earliest source is the one already
/// cited elsewhere. The return value is rows actually inserted, so a caller can
/// tell a new binding from a repeat.
pub fn store_standard_bindings(
    db: &Database,
    journal_key: &str,
    source_url: &str,
    bindings: &[crate::journal_standards::StandardBinding],
    fetched_at: i64,
) -> Result<usize, GaplyError> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    let mut inserted = 0usize;
    for b in bindings {
        let n = tx.execute(
            "INSERT OR IGNORE INTO journal_standard_bindings
               (journal_key, design, standard, source_url, source_span, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                journal_key,
                b.design,
                b.standard.as_str(),
                source_url,
                b.source_span,
                fetched_at,
            ],
        )?;
        inserted += n;
    }
    tx.commit()?;
    Ok(inserted)
}

/// Store a journal's reviewer expectations.
///
/// # Also a reader with no writer — §11 D170, same as the bindings above
///
/// # `status` is always `inferred` from this path, and that is a claim
///
/// §7: an expectation is *"a frequency, with the evidence, never as a rule"*.
/// `journal_expect::extract_expectations` reads a reviewer-guidance page and
/// returns the sentence; it does NOT count how many papers exhibit the
/// behaviour, so `frequency_k`/`frequency_n` are `None` and the status cannot be
/// `verified`. Writing `verified` here would promote a sentence to a measured
/// frequency, which is the mixing §7 forbids.
///
/// The schema refuses the incoherent shapes itself — `frequency_k` without
/// `frequency_n`, and `frequency_n = 0` — so a future counted path cannot store
/// a frequency that means nothing.
pub fn store_expectations(
    db: &Database,
    journal_key: &str,
    source_url: &str,
    expectations: &[crate::journal_expect::ExtractedExpectation],
    fetched_at: i64,
) -> Result<usize, GaplyError> {
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;
    let mut inserted = 0usize;
    for e in expectations {
        // An expectation with no sentence is an assertion; the schema's
        // `length(source_span) > 0` refuses it, and skipping here means a bad
        // row cannot abort a whole crawl's worth of good ones.
        if e.source_span.trim().is_empty() || e.claim.trim().is_empty() {
            continue;
        }
        tx.execute(
            "INSERT INTO journal_expectations
               (journal_key, claim, frequency_k, frequency_n, status, source_url,
                source_span, fetched_at)
             VALUES (?1, ?2, ?3, ?4, 'inferred', ?5, ?6, ?7)",
            params![
                journal_key,
                e.claim,
                e.frequency_k,
                e.frequency_n,
                source_url,
                e.source_span,
                fetched_at,
            ],
        )?;
        inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

/// Record that a journal's fingerprint was built — the provenance row.
///
/// # The third reader-without-writer, and the one a hand survey missed
///
/// §11 D170 was opened after finding `journal_standard_bindings` and
/// `journal_expectations` in that state. Both were found by grepping four table
/// names. `tests/journal_tables_have_writers.rs` enumerates the tables from the
/// SCHEMA instead, and immediately named a fifth: `journal_fingerprints`
/// (migration 24), read by `journal_fingerprint::fingerprint_for` and written
/// by nothing. **A list you write by hand inherits what you already believe is
/// there; a list derived from the schema does not.**
///
/// # Without this row a fingerprint cannot say WHEN it was built
///
/// `JournalFingerprint::provenance` is `Option`, and its own doc says `None`
/// means *"the journal has never been crawled"* and that a screen must render
/// that state rather than an empty fingerprint that looks fetched. With no
/// writer, **every** fingerprint was `None` — so a fully crawled journal and an
/// unknown one were the same value, and the distinction the `Option` exists to
/// carry could not be made.
///
/// # `refetch_after` is a caller's decision, not a default here
///
/// The schema enforces `refetch_after > fetched_at` and nothing else. §7 says
/// profiles are rebuilt *"once per journal per quarter"*, which is a policy, and
/// a policy belongs where it can be changed and measured rather than hidden in
/// a store call. Callers pass both timestamps.
pub fn store_fingerprint_provenance(
    db: &Database,
    p: &crate::journal_fingerprint::FingerprintProvenance,
) -> Result<(), GaplyError> {
    let conn = db.conn()?;
    // REPLACE: one current provenance row per journal, keyed by `journal_key`.
    // A re-crawl supersedes its predecessor — keeping both would make "when was
    // this built" a question about which row a reader picked, the same reason
    // `store_conventions` deletes before inserting.
    conn.execute(
        "INSERT OR REPLACE INTO journal_fingerprints
           (journal_key, version, content_hash, fetched_at, refetch_after,
            source_count, quarantined_at, quarantine_reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            p.journal_key,
            p.version,
            p.content_hash,
            p.fetched_at,
            p.refetch_after,
            p.source_count,
            p.quarantined_at,
            p.quarantine_reason,
        ],
    )?;
    Ok(())
}

/// One stored requirement, as a reader gets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRequirement {
    pub kind: RequirementKind,
    pub value: String,
    pub article_type: Option<String>,
    pub status: String,
    pub source_url: String,
    pub source_heading: String,
    pub source_span: String,
    pub conflict_id: Option<String>,
}

/// Every requirement stored for a journal, newest first.
///
/// **Reads ONE table.** §7's three kinds of knowledge are three queries, never
/// a JOIN — a reader that assembled a requirement and a convention into one
/// record would be doing exactly what §7 forbids, and the schema's disjoint
/// columns (migration 23) make it incoherent rather than merely wrong.
pub fn requirements_for(
    db: &Database,
    journal_key: &str,
) -> Result<Vec<StoredRequirement>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT kind, value, article_type, status, source_url, source_heading, source_span,
                conflict_id
           FROM journal_requirements WHERE journal_key = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map(params![journal_key], |row| {
            let kind: String = row.get(0)?;
            Ok(StoredRequirement {
                kind: kind_from_str(&kind),
                value: row.get(1)?,
                article_type: row.get(2)?,
                status: row.get(3)?,
                source_url: row.get(4)?,
                source_heading: row.get(5)?,
                source_span: row.get(6)?,
                conflict_id: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// How many of a journal's requirements came from `pattern` vs `model`.
///
/// **The split §3.4 asks to be reported per journal.** It is a count over
/// `extracted_by`, which is a CHECK-constrained column rather than a
/// convention, so a third value cannot appear without a migration.
pub fn count_by_extractor(
    db: &Database,
    journal_key: &str,
    extracted_by: &str,
) -> Result<usize, GaplyError> {
    let conn = db.conn()?;
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM journal_requirements
          WHERE journal_key = ?1 AND extracted_by = ?2",
        params![journal_key, extracted_by],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

fn kind_from_str(s: &str) -> RequirementKind {
    match s {
        "word_limit" => RequirementKind::WordLimit,
        "abstract_limit" => RequirementKind::AbstractLimit,
        "section_required" => RequirementKind::SectionRequired,
        "reference_style" => RequirementKind::ReferenceStyle,
        "reference_limit" => RequirementKind::ReferenceLimit,
        "data_policy" => RequirementKind::DataPolicy,
        "reporting_standard" => RequirementKind::ReportingStandard,
        "figure_limit" => RequirementKind::FigureLimit,
        _ => RequirementKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_extract::ExtractedRequirement;

    fn db() -> Database {
        Database::in_memory().unwrap()
    }

    fn req(
        kind: RequirementKind,
        value: &str,
        article_type: Option<&str>,
        span: &str,
    ) -> ExtractedRequirement {
        ExtractedRequirement {
            kind,
            value: value.into(),
            article_type: article_type.map(Into::into),
            source_heading: "Format".into(),
            source_span: span.into(),
        }
    }

    /// **The table stops being one nothing writes to.** §3.4's largest
    /// correction was that `journal_guidelines`'s zero rows were a count of a
    /// table with no producer; migration 23 added four more in that state.
    #[test]
    fn requirements_reach_the_table_and_can_be_read_back_with_their_span() {
        let db = db();
        let reqs = vec![
            req(RequirementKind::WordLimit, "4000", Some("Article"),
                "Format Main text – up to 4,000 words, excluding abstract, Methods."),
            req(RequirementKind::AbstractLimit, "150", Some("Article"),
                "Abstract – up to 150 words, unreferenced."),
        ];
        let out =
            store_requirements(&db, "nature-medicine", "https://www.nature.com/nm/content", &reqs, 1)
                .unwrap();
        assert_eq!(out.inserted, 2);
        assert_eq!(out.new_conflicts, 0);

        let back = requirements_for(&db, "nature-medicine").unwrap();
        assert_eq!(back.len(), 2);
        let w = back.iter().find(|r| r.kind == RequirementKind::WordLimit).unwrap();
        assert_eq!(w.value, "4000");
        assert_eq!(w.article_type.as_deref(), Some("Article"));
        assert_eq!(w.status, "verified");
        assert!(w.source_span.contains("excluding abstract"), "the sentence survives the round trip");
        assert!(w.conflict_id.is_none());
    }

    /// **Prompt 5's named test: seed a conflict, assert neither value is
    /// silently preferred.** Abstract 250 on one page, 300 on another.
    #[test]
    fn two_sources_disagreeing_store_both_and_prefer_neither() {
        let db = db();
        store_requirements(
            &db,
            "j",
            "https://j.test/author-guidelines",
            &[req(RequirementKind::AbstractLimit, "250", None, "The abstract must not exceed 250 words.")],
            1,
        )
        .unwrap();
        let out = store_requirements(
            &db,
            "j",
            "https://j.test/article-types",
            &[req(RequirementKind::AbstractLimit, "300", None, "Abstract – up to 300 words.")],
            2,
        )
        .unwrap();

        assert_eq!(out.new_conflicts, 1);
        let back = requirements_for(&db, "j").unwrap();

        // BOTH values are present.
        let values: Vec<&str> = back.iter().map(|r| r.value.as_str()).collect();
        assert!(values.contains(&"250") && values.contains(&"300"), "{values:?}");

        // BOTH are marked conflicted — including the one stored first, which
        // was uncontested a moment ago and is not uncontested now.
        assert!(back.iter().all(|r| r.status == "conflicted"), "{back:#?}");

        // They share one conflict id, so a reader gets the pair rather than two
        // unrelated facts.
        let ids: std::collections::BTreeSet<&str> =
            back.iter().filter_map(|r| r.conflict_id.as_deref()).collect();
        assert_eq!(ids.len(), 1, "{back:#?}");

        // NEITHER is preferred, and nothing in the row could express a
        // preference — each keeps its own page and its own sentence.
        let urls: std::collections::BTreeSet<&str> =
            back.iter().map(|r| r.source_url.as_str()).collect();
        assert_eq!(urls.len(), 2, "each value keeps the page it came from");
        assert!(back.iter().any(|r| r.source_span.contains("must not exceed 250")));
        assert!(back.iter().any(|r| r.source_span.contains("up to 300")));
    }

    /// A third disagreeing value joins the SAME conflict, rather than starting
    /// a second one that a reader would show as two separate disputes.
    #[test]
    fn a_third_value_joins_the_existing_conflict() {
        let db = db();
        for (i, v) in ["250", "300", "200"].iter().enumerate() {
            store_requirements(
                &db,
                "j",
                &format!("https://j.test/p{i}"),
                &[req(RequirementKind::AbstractLimit, v, None, "Abstract limit.")],
                i as i64,
            )
            .unwrap();
        }
        let back = requirements_for(&db, "j").unwrap();
        assert_eq!(back.len(), 3);
        let ids: std::collections::BTreeSet<&str> =
            back.iter().filter_map(|r| r.conflict_id.as_deref()).collect();
        assert_eq!(ids.len(), 1, "one dispute, not two: {back:#?}");
        assert!(back.iter().all(|r| r.status == "conflicted"));
    }

    /// **Two article types are not a disagreement.** This is the failure
    /// `journal_extract`'s heading binding exists to prevent, arriving one
    /// layer down: without the article type in the conflict key, Nature
    /// Medicine's 4,000-word Article limit and 2,000-word Brief Communication
    /// limit would be stored as a journal contradicting itself.
    #[test]
    fn limits_for_different_article_types_are_two_requirements_not_a_conflict() {
        let db = db();
        let out = store_requirements(
            &db,
            "nm",
            "https://www.nature.com/nm/content",
            &[
                req(RequirementKind::WordLimit, "4000", Some("Article"), "Main text – up to 4,000 words."),
                req(RequirementKind::WordLimit, "2000", Some("Brief Communication"), "Main text – up to 2,000 words."),
            ],
            1,
        )
        .unwrap();
        assert_eq!(out.new_conflicts, 0, "different types, no dispute");
        let back = requirements_for(&db, "nm").unwrap();
        assert!(back.iter().all(|r| r.status == "verified"), "{back:#?}");
    }

    /// An UNBOUND requirement conflicts with another unbound one, and never
    /// with a bound one — `None` is its own bucket, not a wildcard.
    #[test]
    fn an_unbound_requirement_does_not_conflict_with_a_bound_one() {
        let db = db();
        store_requirements(&db, "j", "https://j.test/a",
            &[req(RequirementKind::WordLimit, "4000", Some("Article"), "s")], 1).unwrap();
        let out = store_requirements(&db, "j", "https://j.test/b",
            &[req(RequirementKind::WordLimit, "3000", None, "s")], 2).unwrap();
        assert_eq!(out.new_conflicts, 0, "a bound and an unbound limit are not the same fact");

        // …but two unbound ones do conflict.
        let out2 = store_requirements(&db, "j", "https://j.test/c",
            &[req(RequirementKind::WordLimit, "2500", None, "s")], 3).unwrap();
        assert_eq!(out2.new_conflicts, 1);
    }

    /// **The same value from two pages is corroboration, not a conflict.**
    /// Both rows are kept — a reader asking where the journal says this
    /// deserves both answers — and neither is marked conflicted.
    #[test]
    fn the_same_value_from_two_pages_is_corroboration() {
        let db = db();
        store_requirements(&db, "j", "https://j.test/a",
            &[req(RequirementKind::ReferenceStyle, "Vancouver", None, "References follow Vancouver.")], 1).unwrap();
        let out = store_requirements(&db, "j", "https://j.test/b",
            &[req(RequirementKind::ReferenceStyle, "Vancouver", None, "Use the Vancouver style.")], 2).unwrap();
        assert_eq!(out.new_conflicts, 0);
        assert_eq!(out.inserted, 1);
        let back = requirements_for(&db, "j").unwrap();
        assert_eq!(back.len(), 2, "both pages are kept as evidence");
        assert!(back.iter().all(|r| r.status == "verified"));
    }

    /// **A journal binds many reporting standards and contradicts none of
    /// them.** Measured on Nature Medicine: six standards — CONSORT, PRISMA,
    /// STROBE, STARD, TRIPOD, ARRIVE — were stored as one conflicted fact,
    /// each span naming the design it applies to.
    #[test]
    fn many_reporting_standards_are_not_a_journal_contradicting_itself() {
        let db = db();
        let reqs: Vec<ExtractedRequirement> = ["CONSORT", "PRISMA", "STROBE", "ARRIVE"]
            .iter()
            .map(|s| req(RequirementKind::ReportingStandard, s, None, &format!("must follow {s}.")))
            .collect();
        let out = store_requirements(&db, "nm", "https://j.test/clinicalresearch", &reqs, 1).unwrap();
        assert_eq!(out.new_conflicts, 0, "four standards, four requirements");
        let back = requirements_for(&db, "nm").unwrap();
        assert_eq!(back.len(), 4);
        assert!(back.iter().all(|r| r.status == "verified"), "{back:#?}");

        // A data policy is multi-valued too.
        let out2 = store_requirements(&db, "nm", "https://j.test/data",
            &[req(RequirementKind::DataPolicy, "deposition required", None, "s"),
              req(RequirementKind::DataPolicy, "statement required", None, "s2")], 2).unwrap();
        assert_eq!(out2.new_conflicts, 0);

        // …and a word limit still conflicts, so the rule narrowed rather than
        // disappeared.
        store_requirements(&db, "nm", "https://j.test/a",
            &[req(RequirementKind::WordLimit, "4000", None, "s")], 3).unwrap();
        let out3 = store_requirements(&db, "nm", "https://j.test/b",
            &[req(RequirementKind::WordLimit, "3000", None, "s")], 4).unwrap();
        assert_eq!(out3.new_conflicts, 1);
    }

    /// **Every metric is stored, including the ones the source cannot supply**,
    /// and the schema refuses an `inferred` row with no `n`.
    #[test]
    fn a_convention_profile_stores_its_unavailable_metrics_too() {
        use crate::journal_corpus::{derive_conventions, CorpusBounds, PublishedPaper};
        let db = db();
        // A journal that identifies articles by number: no length, but a
        // statistical style.
        let papers: Vec<PublishedPaper> = (0..20)
            .map(|i| PublishedPaper {
                id: format!("W{i}"),
                title: "t".into(),
                abstract_text: "The hazard ratio was 1.4 (95% CI 1.1-1.8), p = 0.01.".into(),
                publication_date: "2025-06-01".into(),
                work_type: "article".into(),
                first_page: Some(format!("e03{i:05}")),
                last_page: Some(format!("e03{i:05}")),
            })
            .collect();
        let cs = derive_conventions(&papers, &CorpusBounds { minimum: 10, target: 50, maximum: 200 });
        let n = store_conventions(&db, "plos-one", "run-1", &cs, 1).unwrap();
        assert_eq!(n, 5, "all five metrics written");

        let conn = db.conn().unwrap();
        let unavailable: i64 = conn
            .query_row(
                "SELECT count(*) FROM journal_conventions WHERE journal_key='plos-one'                  AND status='unavailable'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unavailable, 4, "length plus the three OpenAlex cannot supply");

        // The schema refuses an inference from nothing.
        assert!(conn
            .execute(
                "INSERT INTO journal_conventions
                   (journal_key, metric, n, status, corpus_run_id, computed_at)
                 VALUES ('x','length',0,'inferred','r',1)",
                [],
            )
            .is_err());
    }

    /// A second run REPLACES the profile: a journal has one current answer to
    /// "what is the typical length", not a history a reader has to pick from.
    #[test]
    fn a_second_corpus_run_replaces_the_profile() {
        use crate::journal_corpus::{ConventionMetric, DerivedConvention};
        let db = db();
        let one = DerivedConvention {
            metric: ConventionMetric::Length,
            median: Some(9.0),
            iqr_low: Some(8.0),
            iqr_high: Some(12.0),
            n: 41,
            detail: "PAGES per recently published RESEARCH paper".into(),
            status: ConventionStatus::Inferred,
        };
        store_conventions(&db, "nm", "run-1", std::slice::from_ref(&one), 1).unwrap();
        let two = DerivedConvention { median: Some(10.0), n: 55, ..one.clone() };
        store_conventions(&db, "nm", "run-2", std::slice::from_ref(&two), 2).unwrap();

        let conn = db.conn().unwrap();
        let (n, median): (i64, f64) = conn
            .query_row(
                "SELECT n, median FROM journal_conventions WHERE journal_key='nm' AND metric='length'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((n, median), (55, 10.0));
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM journal_conventions WHERE journal_key='nm'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "one profile, not two");
    }

    /// A re-crawl of the same page is not new evidence.
    #[test]
    fn re_storing_the_same_page_inserts_nothing() {
        let db = db();
        let r = vec![req(RequirementKind::WordLimit, "4000", Some("Article"), "s")];
        let first = store_requirements(&db, "j", "https://j.test/a", &r, 1).unwrap();
        let second = store_requirements(&db, "j", "https://j.test/a", &r, 2).unwrap();
        assert_eq!(first.inserted, 1);
        assert_eq!(second.inserted, 0);
        assert_eq!(second.duplicates, 1);
        assert_eq!(requirements_for(&db, "j").unwrap().len(), 1);
    }
    // ---------------------------------------------------------------------
    // §11 D170 — the three writers that did not exist
    // ---------------------------------------------------------------------

    fn binding(std_: crate::journal_standards::Standard, design: &str, span: &str)
        -> crate::journal_standards::StandardBinding
    {
        crate::journal_standards::StandardBinding {
            standard: std_,
            design: design.to_string(),
            source_span: span.to_string(),
        }
    }

    /// The fingerprint's `standards` field was permanently empty, and empty is
    /// how "this journal binds no standards" renders. The round trip is the
    /// assertion: store, then read back through the REAL reader.
    #[test]
    fn a_stored_binding_reaches_the_fingerprint() {
        use crate::journal_standards::Standard;
        let db = db();

        // BEFORE — and this is the state every journal was in.
        let before = crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap();
        assert!(before.standards.is_empty(), "precondition: nothing stored yet");

        let n = store_standard_bindings(
            &db,
            "j",
            "https://j.test/authors",
            &[
                binding(Standard::Consort, "randomised trial",
                        "Randomised trials must follow CONSORT."),
                binding(Standard::Strobe, "cohort study",
                        "Observational studies (cohort, case-control) follow STROBE."),
            ],
            1,
        )
        .unwrap();
        assert_eq!(n, 2);

        let after = crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap();
        assert_eq!(after.standards.len(), 2, "{:#?}", after.standards);
        // The SPAN travels — a binding without its sentence is an assertion.
        assert!(
            after.standards.iter().any(|s| s.source_span.contains("must follow CONSORT")),
            "{:#?}", after.standards
        );
    }

    /// A re-crawl must not multiply rows. The UNIQUE constraint is the
    /// mechanism; this pins that the SECOND run reports 0 inserted rather than
    /// failing, so a caller can tell a new binding from a repeat.
    #[test]
    fn re_storing_the_same_binding_inserts_nothing_and_does_not_error() {
        use crate::journal_standards::Standard;
        let db = db();
        let b = [binding(Standard::Prisma, "systematic review", "Reviews follow PRISMA.")];
        assert_eq!(store_standard_bindings(&db, "j", "https://j.test/a", &b, 1).unwrap(), 1);
        assert_eq!(
            store_standard_bindings(&db, "j", "https://j.test/b", &b, 2).unwrap(),
            0,
            "a second page stating the same binding is corroboration, not a new fact"
        );
        let fp = crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap();
        assert_eq!(fp.standards.len(), 1);
        assert!(
            fp.standards[0].source_url.ends_with("/a"),
            "the FIRST span is kept: {:?}", fp.standards[0]
        );
    }

    /// §7: an expectation is a frequency with its evidence, never a rule. This
    /// path counts nothing, so it may not claim `verified`.
    #[test]
    fn a_stored_expectation_is_inferred_and_never_verified() {
        let db = db();
        let e = crate::journal_expect::ExtractedExpectation {
            claim: "Reviewers are asked to comment on statistical rigour.".into(),
            source_span: "Reviewers are asked to comment on the statistical rigour of the work."
                .into(),
            source_heading: "For reviewers".into(),
            frequency_k: None,
            frequency_n: None,
        };
        assert_eq!(store_expectations(&db, "j", "https://j.test/reviewers", &[e], 1).unwrap(), 1);

        let fp = crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap();
        assert_eq!(fp.expectations.len(), 1);
        assert_eq!(
            fp.expectations[0].status, "inferred",
            "an uncounted claim must not be stored as a measured frequency"
        );
        assert!(fp.expectations[0].frequency_n.is_none());
    }

    /// A span-less expectation is an assertion; it is skipped rather than
    /// allowed to abort the crawl that produced its neighbours.
    #[test]
    fn a_span_less_expectation_is_skipped_not_stored() {
        let db = db();
        let bad = crate::journal_expect::ExtractedExpectation {
            claim: "Something".into(),
            source_span: "   ".into(),
            source_heading: String::new(),
            frequency_k: None,
            frequency_n: None,
        };
        assert_eq!(store_expectations(&db, "j", "https://j.test/r", &[bad], 1).unwrap(), 0);
        assert!(crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap().expectations.is_empty());
    }

    /// `provenance: None` is documented to mean "never crawled". With no writer
    /// it meant that for every journal, including fully crawled ones — so this
    /// pins that the distinction can now be made at all.
    #[test]
    fn provenance_distinguishes_a_crawled_journal_from_an_unknown_one() {
        let db = db();
        assert!(
            crate::journal_fingerprint::fingerprint_for(&db, "unknown").unwrap().provenance.is_none(),
            "an uncrawled journal has no provenance"
        );

        let p = crate::journal_fingerprint::FingerprintProvenance {
            journal_key: "j".into(),
            version: 1,
            content_hash: "abc123".into(),
            fetched_at: 1_000,
            refetch_after: 2_000,
            source_count: 7,
            quarantined_at: None,
            quarantine_reason: None,
        };
        store_fingerprint_provenance(&db, &p).unwrap();

        let got = crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap().provenance;
        let got = got.expect("a crawled journal HAS provenance");
        assert_eq!(got.source_count, 7);
        assert_eq!(got.content_hash, "abc123");
    }

    /// A re-crawl supersedes; one current row per journal.
    #[test]
    fn re_storing_provenance_replaces_rather_than_accumulates() {
        let db = db();
        let mk = |n: i64| crate::journal_fingerprint::FingerprintProvenance {
            journal_key: "j".into(),
            version: 1,
            content_hash: format!("hash{n}"),
            fetched_at: n,
            refetch_after: n + 1_000,
            source_count: n,
            quarantined_at: None,
            quarantine_reason: None,
        };
        store_fingerprint_provenance(&db, &mk(1_000)).unwrap();
        store_fingerprint_provenance(&db, &mk(5_000)).unwrap();
        let got = crate::journal_fingerprint::fingerprint_for(&db, "j").unwrap().provenance.unwrap();
        assert_eq!(got.fetched_at, 5_000, "the later crawl wins");
        assert_eq!(got.content_hash, "hash5000");
    }

}
