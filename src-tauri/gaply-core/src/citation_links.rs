//! Links between the citation library and indexed documents (migration v17).
//!
//! Phase 8's audit had to join `citation_library` to `documents` by normalised
//! TITLE, recomputed on every run, and recorded the consequence: a source
//! indexed under a different title reported `unverifiable` even though its text
//! was present. This is that fix.
//!
//! # Deterministic, and outside the AI tree on purpose
//!
//! Nothing here runs a model, and it lives in `gaply-core` beside the citation
//! library rather than under `ai_engine`, because the linkage is a fact about
//! the user's own library — useful to any feature, and subject to R1: the
//! deterministic citation path must keep working with the AI tree deleted.
//!
//! # DOI beats title, and the difference is recorded
//!
//! A DOI is an identifier; a title is a heuristic. Both are stored, but
//! `matched_by` says which was used, so a reader can weigh a link instead of
//! having to trust all links equally.
//!
//! **A caveat that decides how often DOI matching can fire:** `documents` has
//! NO doi column — it carries `source_url`. A DOI link is therefore only
//! possible when the document's URL actually contains the DOI (a `doi.org`
//! link, a `doi:` scheme, or a bare `10.x/y`). That is a real limit of the
//! current schema, not a shortcut: where the URL carries no DOI, the title
//! heuristic remains the only available evidence.

use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::now_epoch;
use crate::GaplyError;

/// How a link was established, most trustworthy first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedBy {
    Doi,
    Title,
    Manual,
}

impl MatchedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchedBy::Doi => "doi",
            MatchedBy::Title => "title",
            MatchedBy::Manual => "manual",
        }
    }
}

/// What a link pass did. Reported rather than silent: linking changes which
/// claims an audit can check at all, so it is not a background detail.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkReport {
    pub citations_considered: usize,
    pub documents_considered: usize,
    pub linked_by_doi: usize,
    pub linked_by_title: usize,
    /// Links that already existed and were left alone — including manual ones,
    /// which a refresh must never quietly overwrite.
    pub already_linked: usize,
}

/// Reduce a DOI to a comparable form: lowercase, no resolver prefix, no
/// trailing punctuation.
///
/// Returns `None` for anything that is not shaped like a DOI, so a stray URL
/// cannot become a false identifier match.
pub fn normalise_doi(raw: &str) -> Option<String> {
    let t = raw.trim().to_lowercase();
    let t = t
        .trim_start_matches("https://doi.org/")
        .trim_start_matches("http://doi.org/")
        .trim_start_matches("https://dx.doi.org/")
        .trim_start_matches("http://dx.doi.org/")
        .trim_start_matches("doi:")
        .trim();
    // Trailing punctuation is common when a DOI was copied out of prose.
    let t = t.trim_end_matches(['.', ',', ';', ')', ']']);
    // The registrant prefix is always `10.` followed by digits, then `/`.
    if !t.starts_with("10.") {
        return None;
    }
    let rest = &t[3..];
    let (registrant, suffix) = rest.split_once('/')?;
    if registrant.is_empty()
        || !registrant.chars().all(|c| c.is_ascii_digit())
        || suffix.is_empty()
    {
        return None;
    }
    Some(t.to_string())
}

/// Find a DOI inside a document's `source_url`, if it carries one.
pub fn doi_in_source_url(source_url: &str) -> Option<String> {
    if let Some(d) = normalise_doi(source_url) {
        return Some(d);
    }
    // A DOI embedded mid-URL (query strings, proxies).
    let lower = source_url.to_lowercase();
    let idx = lower.find("10.")?;
    normalise_doi(&lower[idx..])
}

/// Normalise a title for comparison: lowercase, alphanumerics and single
/// spaces. Deliberately crude, and now recorded as `matched_by='title'` rather
/// than recomputed inside every audit.
pub fn normalise_title(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build or refresh links across the whole library.
///
/// Idempotent: `UNIQUE (citation_id, document_id)` means re-running adds
/// nothing, and an existing link — including a `manual` one — is never
/// rewritten by a weaker automatic match.
pub fn link_citations(db: &Database) -> Result<LinkReport, GaplyError> {
    let now = now_epoch();
    let mut conn = db.conn()?;
    let tx = conn.transaction()?;

    let citations: Vec<(String, Option<String>, String)> = {
        let mut stmt = tx.prepare("SELECT id, doi, title FROM citation_library")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let documents: Vec<(i64, String, String)> = {
        let mut stmt = tx.prepare("SELECT id, title, source_url FROM documents")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    let mut report = LinkReport {
        citations_considered: citations.len(),
        documents_considered: documents.len(),
        ..Default::default()
    };

    // Precompute the document side once: linking is O(citations × documents)
    // and both grow with the user's library.
    let doc_keys: Vec<(i64, Option<String>, String)> = documents
        .iter()
        .map(|(id, title, url)| (*id, doi_in_source_url(url), normalise_title(title)))
        .collect();

    for (citation_id, doi, title) in &citations {
        let cite_doi = doi.as_deref().and_then(normalise_doi);
        let cite_title = normalise_title(title);

        for (doc_id, doc_doi, doc_title) in &doc_keys {
            // DOI FIRST. An identifier match outranks a heuristic one, and a
            // title collision between two different works is exactly what the
            // DOI is there to break.
            let matched = if cite_doi.is_some() && cite_doi.as_deref() == doc_doi.as_deref() {
                Some(MatchedBy::Doi)
            } else if !cite_title.is_empty() && cite_title == *doc_title {
                Some(MatchedBy::Title)
            } else {
                None
            };
            let Some(matched) = matched else { continue };

            let inserted = tx.execute(
                "INSERT INTO citation_documents (citation_id, document_id, matched_by, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (citation_id, document_id) DO NOTHING",
                params![citation_id, doc_id, matched.as_str(), now],
            )?;
            if inserted == 0 {
                report.already_linked += 1;
            } else {
                match matched {
                    MatchedBy::Doi => report.linked_by_doi += 1,
                    MatchedBy::Title => report.linked_by_title += 1,
                    MatchedBy::Manual => {}
                }
            }
        }
    }

    tx.commit()?;
    Ok(report)
}

/// Record a link a human asserted. Never overwritten by an automatic pass.
pub fn link_manually(
    db: &Database,
    citation_id: &str,
    document_id: i64,
) -> Result<(), GaplyError> {
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO citation_documents (citation_id, document_id, matched_by, created_at)
         VALUES (?1, ?2, 'manual', ?3)
         ON CONFLICT (citation_id, document_id) DO UPDATE SET matched_by = 'manual'",
        params![citation_id, document_id, now_epoch()],
    )?;
    Ok(())
}

/// Record a link established by DOI — an identifier match, not a heuristic.
///
/// Ranks below `manual` and above `title`, and the conflict clause says so: an
/// existing DOI or title link is upgraded to `doi`, a MANUAL one is left alone.
/// A person who pointed Gaply at a specific file has asserted something the
/// identifier cannot overrule, and silently replacing that assertion is exactly
/// the class of "helpful" overwrite this table exists to prevent.
pub fn link_by_doi(
    db: &Database,
    citation_id: &str,
    document_id: i64,
) -> Result<(), GaplyError> {
    let conn = db.conn()?;
    conn.execute(
        "INSERT INTO citation_documents (citation_id, document_id, matched_by, created_at)
         VALUES (?1, ?2, 'doi', ?3)
         ON CONFLICT (citation_id, document_id)
           DO UPDATE SET matched_by = 'doi' WHERE matched_by <> 'manual'",
        params![citation_id, document_id, now_epoch()],
    )?;
    Ok(())
}

/// Documents linked to a citation, most trustworthy match first.
pub fn documents_for_citation(
    db: &Database,
    citation_id: &str,
) -> Result<Vec<(i64, MatchedBy)>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(
        "SELECT document_id, matched_by FROM citation_documents
         WHERE citation_id = ?1
         ORDER BY CASE matched_by WHEN 'manual' THEN 0 WHEN 'doi' THEN 1 ELSE 2 END,
                  document_id",
    )?;
    let rows = stmt
        .query_map(params![citation_id], |r| {
            let m: String = r.get(1)?;
            Ok((
                r.get::<_, i64>(0)?,
                match m.as_str() {
                    "doi" => MatchedBy::Doi,
                    "manual" => MatchedBy::Manual,
                    _ => MatchedBy::Title,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The first linked document that is INDEXED and EMBEDDED — i.e. one an audit
/// could actually retrieve evidence from.
///
/// Chunks alone are not enough: retrieval needs vectors, and a document with
/// none returns NoEvidence for every claim, which reads as a model failure
/// rather than a missing index.
pub fn checkable_document_for_citation(
    db: &Database,
    citation_id: &str,
) -> Result<Option<(i64, MatchedBy)>, GaplyError> {
    for (doc_id, matched) in documents_for_citation(db, citation_id)? {
        let conn = db.conn()?;
        let ready: bool = conn.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM ai_chunks c
                 JOIN ai_chunk_embeddings e ON e.chunk_id = c.id
                 WHERE c.document_id = ?1
             )",
            params![doc_id],
            |r| r.get(0),
        )?;
        if ready {
            return Ok(Some((doc_id, matched)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(db: &Database, sql: &str, p: &[&dyn rusqlite::ToSql]) -> i64 {
        let conn = db.conn().unwrap();
        conn.execute(sql, p).unwrap();
        conn.last_insert_rowid()
    }

    fn cite(db: &Database, id: &str, doi: Option<&str>, title: &str) {
        exec(
            db,
            "INSERT INTO citation_library (id, csl_json, doi, title, authors, year, created_at, updated_at)
             VALUES (?1, '{}', ?2, ?3, 'Smith, J.', 2019, 1, 1)",
            &[&id, &doi, &title],
        );
    }

    fn doc(db: &Database, title: &str, url: &str, ck: &str) -> i64 {
        exec(
            db,
            "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
             VALUES ('pdf', ?1, ?2, 1, ?3, 'ready', 1)",
            &[&title, &url, &ck],
        )
    }

    /// Give a document chunks AND vectors, so it counts as checkable.
    fn make_checkable(db: &Database, document_id: i64, tag: &str) {
        exec(
            db,
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, registered_at)
             VALUES ('emb', 'embedding', 'emb', '/x', 1) ON CONFLICT(id) DO NOTHING",
            &[],
        );
        let chunk = exec(
            db,
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content,
                                    token_estimate, content_hash, created_at)
             VALUES (?1, 1, 0, 5, 'text', 1, ?2, 1)",
            &[&document_id, &tag],
        );
        exec(
            db,
            "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
             VALUES (?1, 'emb', 'p1', 1, X'00', 1)",
            &[&chunk],
        );
    }

    #[test]
    fn dois_normalise_across_the_forms_people_actually_paste() {
        for raw in [
            "10.1234/abc.def",
            "  10.1234/abc.def  ",
            "https://doi.org/10.1234/abc.def",
            "http://dx.doi.org/10.1234/ABC.DEF",
            "doi:10.1234/abc.def",
            "10.1234/abc.def.",
        ] {
            assert_eq!(normalise_doi(raw).as_deref(), Some("10.1234/abc.def"), "{raw}");
        }
        // Not DOIs — a stray URL must never become a false identifier match.
        for raw in ["", "abc", "https://example.com/paper.pdf", "10.1234", "10./x", "1234/abc"] {
            assert!(normalise_doi(raw).is_none(), "{raw:?} was accepted as a DOI");
        }
    }

    #[test]
    fn a_doi_is_found_inside_a_source_url() {
        assert_eq!(
            doi_in_source_url("https://doi.org/10.1234/abc").as_deref(),
            Some("10.1234/abc")
        );
        assert_eq!(
            doi_in_source_url("https://proxy.example.edu/fetch?id=10.1234/abc").as_deref(),
            Some("10.1234/abc")
        );
        assert!(doi_in_source_url("https://example.com/paper.pdf").is_none());
    }

    /// THE ordering rule: an identifier beats a heuristic. Two documents match
    /// one citation — one by title, one by DOI — and the DOI link is the one a
    /// consumer sees first.
    #[test]
    fn a_doi_match_outranks_a_title_match() {
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", Some("10.1234/abc"), "Organic Management and Soil Life");
        let by_title = doc(&db, "organic management and soil life", "", "ck-title");
        let by_doi = doc(&db, "A Completely Different Title", "https://doi.org/10.1234/abc", "ck-doi");

        let r = link_citations(&db).unwrap();
        assert_eq!(r.linked_by_doi, 1);
        assert_eq!(r.linked_by_title, 1);

        let links = documents_for_citation(&db, "c1").unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], (by_doi, MatchedBy::Doi), "the DOI link must rank first");
        assert!(links.contains(&(by_title, MatchedBy::Title)));
    }

    /// The case Phase 8 could not handle: the file is present but titled
    /// differently. Under the old title-only join this was unverifiable.
    #[test]
    fn a_retitled_document_links_by_doi() {
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", Some("10.5555/xyz"), "Paired Fields and Soil Fauna");
        let d = doc(&db, "smith_2019_preprint_v3_FINAL", "doi:10.5555/xyz", "ck-1");
        make_checkable(&db, d, "h1");

        let r = link_citations(&db).unwrap();
        assert_eq!(r.linked_by_doi, 1);
        assert_eq!(r.linked_by_title, 0, "the titles do not match, and should not");

        let found = checkable_document_for_citation(&db, "c1").unwrap();
        assert_eq!(found, Some((d, MatchedBy::Doi)));
    }

    #[test]
    fn no_match_means_no_link() {
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", Some("10.1/a"), "A Title Nobody Indexed");
        doc(&db, "Something Else Entirely", "https://example.com/x.pdf", "ck-1");
        let r = link_citations(&db).unwrap();
        assert_eq!(r.linked_by_doi + r.linked_by_title, 0);
        assert!(documents_for_citation(&db, "c1").unwrap().is_empty());
        assert!(checkable_document_for_citation(&db, "c1").unwrap().is_none());
    }

    /// A linked but UNEMBEDDED document is not checkable: retrieval needs the
    /// vectors, and a document with none returns NoEvidence for every claim.
    #[test]
    fn a_linked_but_unembedded_document_is_not_checkable() {
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", None, "Paired Fields");
        let d = doc(&db, "paired fields", "", "ck-1");
        link_citations(&db).unwrap();
        assert_eq!(documents_for_citation(&db, "c1").unwrap(), vec![(d, MatchedBy::Title)]);
        assert!(
            checkable_document_for_citation(&db, "c1").unwrap().is_none(),
            "a document with no vectors must not be called checkable"
        );
    }

    /// Re-running adds nothing, and never downgrades a human's decision.
    #[test]
    #[test]
    fn link_by_doi_records_doi_and_upgrades_a_title_link() {
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", Some("10.1/a"), "A Paper");
        let d = doc(&db, "A Paper", "/tmp/a.pdf", "ck-a");

        // A pre-existing TITLE link is a heuristic; an identifier match is
        // better evidence for the same fact, so it is allowed to replace it.
        exec(
            &db,
            "INSERT INTO citation_documents (citation_id, document_id, matched_by, created_at)
             VALUES ('c1', ?1, 'title', 1)",
            &[&d],
        );
        link_by_doi(&db, "c1", d).unwrap();
        assert_eq!(documents_for_citation(&db, "c1").unwrap(), vec![(d, MatchedBy::Doi)]);

        // Idempotent: re-running changes nothing and adds no second row.
        link_by_doi(&db, "c1", d).unwrap();
        assert_eq!(documents_for_citation(&db, "c1").unwrap(), vec![(d, MatchedBy::Doi)]);
    }

    #[test]
    fn link_by_doi_never_downgrades_a_manual_link() {
        // The user pointed at this file themselves. A later automatic fetch
        // that happens to agree must not rewrite the record of who decided.
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", Some("10.1/a"), "A Paper");
        let d = doc(&db, "A Paper", "/tmp/a.pdf", "ck-a");
        link_manually(&db, "c1", d).unwrap();

        link_by_doi(&db, "c1", d).unwrap();
        assert_eq!(
            documents_for_citation(&db, "c1").unwrap(),
            vec![(d, MatchedBy::Manual)],
            "a manual link was downgraded to doi"
        );
    }

    fn relinking_is_idempotent_and_never_overwrites_a_manual_link() {
        let db = Database::in_memory().unwrap();
        cite(&db, "c1", None, "Paired Fields");
        let d = doc(&db, "paired fields", "", "ck-1");
        let other = doc(&db, "unrelated title", "", "ck-2");

        link_manually(&db, "c1", other).unwrap();
        let first = link_citations(&db).unwrap();
        assert_eq!(first.linked_by_title, 1);

        let second = link_citations(&db).unwrap();
        assert_eq!(second.linked_by_title, 0, "a second pass created duplicate links");
        assert_eq!(second.already_linked, 1);

        let links = documents_for_citation(&db, "c1").unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], (other, MatchedBy::Manual), "manual must rank first and survive");
        assert!(links.contains(&(d, MatchedBy::Title)));
    }
}
