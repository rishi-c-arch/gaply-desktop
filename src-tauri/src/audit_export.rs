//! Turns a finished audit's job rows into a report model, and exports it.
//!
//! The composer (`gaply_core::audit_report`) owns wording and ordering; the
//! renderers own format. This module owns only the READ: pulling items,
//! results and evidence out of the store and shaping them, so neither of the
//! other two has to know what a job row looks like.
//!
//! # Why the evidence text is fetched HERE
//!
//! §11 D18 validates `chunk_id` and `page` against the store but nothing ties a
//! model's prose to the passage. The report's defence is to print the passage
//! beside the prose — which means the report needs the passage TEXT, and the
//! only honest source for it is the store, not the model's reply. So the quotes
//! come from `chunks_by_ids`, and a chunk the store cannot produce is dropped
//! rather than substituted: an item with no recoverable evidence loses its
//! prose, which is exactly the D18 rule doing its job.

use gaply_core::ai_engine::{jobs, store};
use gaply_core::audit_report::{AuditReportModel, ReportEvidence, ReportItem};
use gaply_core::{Database, GaplyError};

/// Every item of a job, in seq order. Jobs are hundreds of rows at most.
fn all_items(db: &Database, job_id: i64) -> Result<Vec<jobs::JobItem>, GaplyError> {
    let mut out = Vec::new();
    let mut offset = 0i64;
    loop {
        let page = jobs::job_results(db, job_id, offset, 200)?;
        if page.is_empty() {
            break;
        }
        offset += page.len() as i64;
        out.extend(page);
    }
    Ok(out)
}

fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(str::to_string)
}

/// The model's own output object, whichever task produced it.
fn output_of(result: &serde_json::Value) -> &serde_json::Value {
    result.get("output").unwrap_or(result)
}

/// Resolve the chunk ids a support verdict rested on into quoted passages.
///
/// Ids arrive as the model wrote them (`"c7"`), so the `c` is stripped to reach
/// the row. A chunk that no longer exists yields nothing — see the module docs.
fn evidence_for(db: &Database, out: &serde_json::Value) -> Vec<ReportEvidence> {
    let refs = match out.get("supporting_chunks").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let ids: Vec<i64> = refs
        .iter()
        .filter_map(|c| c.get("chunk_id")?.as_str()?.trim_start_matches('c').parse().ok())
        .collect();
    if ids.is_empty() {
        return Vec::new();
    }
    let stored = store::chunks_by_ids(db, &ids).unwrap_or_default();
    stored
        .into_iter()
        .map(|c| ReportEvidence {
            chunk_id: format!("c{}", c.id),
            // The STORE's page, not the model's echo of it. D18 validates them
            // as equal at judgement time; this reads the authority directly.
            page: c.page,
            quote: c.content,
        })
        .collect()
}

/// What a reader can do about an item that could not be checked.
fn next_step_for(reason: &str) -> String {
    if reason.contains("not in your library") {
        "Add this work to your library — “Fetch open-access PDF” will try to find a free copy by \
         DOI, or attach a PDF you already have."
            .to_string()
    } else if reason.contains("not indexed") || reason.contains("link the document") {
        "The work is in your library but has no readable source attached. Link its document from \
         the citation's Document card."
            .to_string()
    } else if reason.contains("not in the reference list") {
        "This number has no entry in the paper's own reference list. Check the citation is \
         numbered correctly."
            .to_string()
    } else {
        "Add the cited work to your library so this sentence can be checked against it.".to_string()
    }
}

/// Today, as a report reads it. Kept here rather than in the composer so the
/// composer stays a pure function of its model and its tests never depend on a
/// clock.
pub fn today_label() -> String {
    // No chrono in this crate; the epoch-to-civil conversion is short and
    // exact (Howard Hinnant's algorithm), and a report date is worth getting
    // right rather than approximating.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

/// Build the report model from the store.
/// Build the report model from the store.
///
/// `installed_model_id` is used ONLY when the job predates model recording, and
/// it is labelled as a guess when it is. Jobs run before `set_job_model` existed
/// have no model on the row and none in their item results, so the truthful
/// options are "not recorded" — which tells the reader nothing — or the model
/// installed now, said to be exactly that. A bare model name that might not be
/// the one that produced the verdicts would be the one unacceptable option.
pub fn build_model_with(
    db: &Database,
    job_id: i64,
    manuscript_name: &str,
    generated_on: &str,
    installed_model_id: Option<&str>,
) -> Result<AuditReportModel, GaplyError> {
    let job = jobs::get_job(db, job_id)?
        .ok_or_else(|| GaplyError::NotFound { entity: "job", id: job_id.to_string() })?;
    let items = all_items(db, job_id)?;

    let mut m = AuditReportModel {
        manuscript_name: manuscript_name.to_string(),
        // §11 D94. Read back from the job — computed at plan time, when the
        // manuscript was still in hand.
        consistency: gaply_core::ai_engine::jobs::get_job(db, job_id)?
            .and_then(|j| j.summary_json)
            .and_then(|j| serde_json::from_str::<gaply_core::consistency::ConsistencyReport>(&j).ok())
            .map(|c| c.findings)
            .unwrap_or_default(),
        generated_on: generated_on.to_string(),
        model_id: job.model_id.clone().unwrap_or_else(|| match installed_model_id {
            Some(id) => format!("{id} (not recorded for this run; this is the model installed now)"),
            None => "not recorded".to_string(),
        }),
        prompt_version: job.prompt_version.clone(),
        // A PDF has pages; a .docx has none. Read from the items rather than
        // from the file name: the planner is the authority on what extraction
        // produced, and a renamed file must not change the report's claims.
        has_pages: items.iter().any(|i| i.page.is_some()),
        total_sentences: items.len(),
        checked: items.iter().filter(|i| i.status == "done").count(),
        ..Default::default()
    };

    let mut by_kind: std::collections::BTreeMap<String, usize> = Default::default();
    let mut verdicts: std::collections::BTreeMap<String, usize> = Default::default();

    for it in &items {
        *by_kind.entry(it.kind.as_str().to_string()).or_insert(0) += 1;

        let parsed: Option<serde_json::Value> =
            it.result_json.as_deref().and_then(|r| serde_json::from_str(r).ok());
        let payload: serde_json::Value =
            serde_json::from_str(&it.payload_json).unwrap_or(serde_json::Value::Null);

        let mut item = ReportItem {
            seq: it.seq,
            page: it.page,
            // §11 D65. The pre-pass records a paragraph ordinal for sources
            // with no pages; it rides in the payload beside `section`.
            paragraph: payload
                .get("paragraph")
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok()),
            sentence: it.sentence.clone(),
            ..Default::default()
        };

        match it.kind.as_str() {
            "unverifiable" => {
                let reason = str_field(&payload, "reason")
                    .unwrap_or_else(|| "no reason recorded".to_string());
                item.next_step = Some(next_step_for(&reason));
                item.reference_entry = str_field(&payload, "referenceEntry");
                item.reason = Some(reason);
                m.unverifiable.push(item);
            }
            _ if it.status == "failed" => {
                item.reason = it.error.clone().or_else(|| Some("no reason recorded".into()));
                m.failed.push(item);
            }
            "citation_support" => {
                if let Some(p) = &parsed {
                    let out = output_of(p);
                    let v = str_field(out, "verdict");
                    if let Some(v) = &v {
                        *verdicts.entry(format!("support: {v}")).or_insert(0) += 1;
                    }
                    item.verdict = v;
                    item.explanation = str_field(out, "explanation");
                    item.evidence = evidence_for(db, out);
                }
                m.supported.push(item);
            }
            "citation_need" => {
                if let Some(p) = &parsed {
                    let out = output_of(p);
                    let needs = out.get("needs_citation").and_then(|v| v.as_bool());
                    let label = match needs {
                        Some(true) => "needs_citation",
                        Some(false) => "no_citation_needed",
                        None => "not determined",
                    };
                    *verdicts.entry(label.to_string()).or_insert(0) += 1;
                    item.verdict = Some(label.to_string());
                    // A citation_need judgement rests on NO evidence by
                    // construction — the sentence cites nothing. Its rationale
                    // is therefore prose about the sentence itself rather than
                    // about a source, so it is carried as `reason`, which the
                    // composer prints unconditionally, and NOT as `explanation`,
                    // which D18 would suppress for want of a quote.
                    item.reason = str_field(out, "reason").or_else(|| str_field(out, "rationale"));
                }
                m.needs_citation.push(item);
            }
            _ => {}
        }
    }

    m.counts_by_category = by_kind.into_iter().collect();
    m.verdict_counts = verdicts.into_iter().collect();
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaply_core::ai_engine::jobs::{ItemKind, NewItem};

    fn db() -> Database {
        Database::in_memory().unwrap()
    }

    fn seed_job(db: &Database, items: Vec<NewItem>) -> i64 {
        jobs::create_job(db, "thesis_audit", None, "citation_support-v1.4", &items).unwrap()
    }

    fn item(seq: i64, kind: ItemKind, sentence: &str, payload: &str) -> NewItem {
        NewItem {
            seq,
            kind,
            chunk_id: None,
            page: Some(1),
            sentence: sentence.to_string(),
            payload_json: payload.to_string(),
        }
    }

    #[test]
    fn the_date_label_is_a_real_civil_date() {
        // Epoch-to-civil arithmetic, so worth pinning rather than trusting.
        let label = today_label();
        let parts: Vec<&str> = label.split(' ').collect();
        assert_eq!(parts.len(), 3, "{label}");
        let day: u32 = parts[0].parse().expect("a day number");
        assert!((1..=31).contains(&day), "{label}");
        let year: i32 = parts[2].parse().expect("a year");
        assert!((2024..2100).contains(&year), "{label}");
        assert!(
            ["January","February","March","April","May","June","July","August","September",
             "October","November","December"].contains(&parts[1]),
            "{label}"
        );
    }

    #[test]
    fn an_unverifiable_item_carries_its_reason_and_a_next_step() {
        let db = db();
        let job = seed_job(
            &db,
            vec![item(
                1,
                ItemKind::Unverifiable,
                "A cited claim [5].",
                r#"{"reason":"[5] S. Mohammad, Crowdsourcing — not in your library"}"#,
            )],
        );
        let m = build_model(&db, job, "R PAPER .pdf", "3 September 2026").unwrap();
        assert_eq!(m.unverifiable.len(), 1);
        let it = &m.unverifiable[0];
        assert!(it.reason.as_deref().unwrap().contains("not in your library"));
        // The step names the actions the UI actually offers.
        let step = it.next_step.as_deref().unwrap();
        assert!(step.contains("Fetch open-access PDF"), "{step}");
        assert!(step.contains("attach a PDF"), "{step}");
    }

    #[test]
    fn the_next_step_matches_the_reason_rather_than_being_generic() {
        // "not in your library" and "in your library but not indexed" need
        // different actions; one message for both would send half the readers
        // to the wrong place.
        assert!(next_step_for("[5] X — not in your library").contains("Fetch open-access"));
        assert!(next_step_for("[5] is in your library but its source is not indexed — link the document")
            .contains("Link its document"));
        assert!(next_step_for("[9] is not in the reference list").contains("numbered correctly"));
    }

    #[test]
    fn a_citation_need_rationale_is_carried_as_reason_not_as_explanation() {
        // These items rest on no evidence by construction, so prose in
        // `explanation` would be suppressed by D18 and the reader would be told
        // "withheld" for an item where the model's reasoning IS the finding.
        let db = db();
        let job = seed_job(&db, vec![item(1, ItemKind::CitationNeed, "An uncited claim.", "{}")]);
        {
            let items = jobs::job_results(&db, job, 0, 10).unwrap();
            jobs::complete_item(
                &db,
                items[0].id,
                "done",
                Some(r#"{"output":{"needs_citation":true,"reason":"This asserts an empirical result."}}"#),
                None,
            )
            .unwrap();
        }
        let m = build_model(&db, job, "p.pdf", "today").unwrap();
        let it = &m.needs_citation[0];
        assert_eq!(it.verdict.as_deref(), Some("needs_citation"));
        assert_eq!(it.reason.as_deref(), Some("This asserts an empirical result."));
        assert!(it.explanation.is_none(), "rationale must not land in the D18-gated field");

        // And it survives into the rendered report.
        let text = gaply_core::report_html::render_html(
            &gaply_core::audit_report::compose_audit(&m),
            "t",
        );
        assert!(text.contains("This asserts an empirical result."), "{text}");
    }

    #[test]
    fn counts_and_verdicts_are_both_derived_from_the_rows() {
        let db = db();
        let job = seed_job(
            &db,
            vec![
                item(1, ItemKind::CitationNeed, "One.", "{}"),
                item(2, ItemKind::CitationNeed, "Two.", "{}"),
                item(3, ItemKind::Unverifiable, "Three [1].", r#"{"reason":"not in your library"}"#),
            ],
        );
        let ids: Vec<i64> = jobs::job_results(&db, job, 0, 10).unwrap().iter().map(|i| i.id).collect();
        jobs::complete_item(&db, ids[0], "done", Some(r#"{"output":{"needs_citation":true}}"#), None).unwrap();
        jobs::complete_item(&db, ids[1], "done", Some(r#"{"output":{"needs_citation":false}}"#), None).unwrap();

        let m = build_model(&db, job, "p.pdf", "today").unwrap();
        assert_eq!(m.total_sentences, 3);
        assert_eq!(
            m.counts_by_category,
            vec![("citation_need".to_string(), 2), ("unverifiable".to_string(), 1)]
        );
        // The two numbers are different facts and both are reported.
        assert_eq!(
            m.verdict_counts,
            vec![("needs_citation".to_string(), 1), ("no_citation_needed".to_string(), 1)]
        );
    }

    #[test]
    fn a_failed_item_is_reported_rather_than_dropped() {
        // An unanswered sentence is not a clean one.
        let db = db();
        let job = seed_job(&db, vec![item(1, ItemKind::CitationNeed, "A claim.", "{}")]);
        let ids: Vec<i64> = jobs::job_results(&db, job, 0, 10).unwrap().iter().map(|i| i.id).collect();
        jobs::complete_item(&db, ids[0], "failed", None, Some("failed validation twice")).unwrap();

        let m = build_model(&db, job, "p.pdf", "today").unwrap();
        assert_eq!(m.failed.len(), 1);
        assert_eq!(m.needs_citation.len(), 0);
        assert!(m.failed[0].reason.as_deref().unwrap().contains("validation"));
    }
}

/// Back-compatible entry point: no installed-model hint available.
pub fn build_model(
    db: &Database,
    job_id: i64,
    manuscript_name: &str,
    generated_on: &str,
) -> Result<AuditReportModel, GaplyError> {
    build_model_with(db, job_id, manuscript_name, generated_on, None)
}
