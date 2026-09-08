//! Planning a thesis citation audit, and reporting on one.
//!
//! Both halves are deterministic and synchronous, so they live here rather than
//! in the app crate: planning runs no model (§11 D40), and the report is a
//! query over rows the runner already wrote.
//!
//! # The plan is produced in full, before anything runs
//!
//! Parse → segment → detect markers → resolve against the library → write every
//! item in one transaction. At ~65 s an item, discovering at item 200 that the
//! manuscript was unparseable would waste three hours; discovering it during
//! planning costs seconds.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::audit_prepass::{prepass_blocks, PrepassReport, Resolution};
use super::jobs::{self, ItemKind, NewItem};
use crate::db::Database;
use crate::GaplyError;

/// What planning measured, returned before the first model call so a caller can
/// show the user what they are about to spend hours on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPlan {
    pub job_id: i64,
    pub document_types_supported: Vec<&'static str>,
    pub total_sentences: usize,
    pub cited: usize,
    pub uncited: usize,
    pub skipped: usize,
    pub markers_found: usize,
    /// Items actually queued, by kind. The difference between this and
    /// `total_sentences` is the significance filter doing its job.
    pub queued_citation_need: usize,
    pub queued_citation_support: usize,
    pub queued_unverifiable: usize,
}

/// Which sentences a plan covers.
///
/// The per-citation check is a SLICE of the thesis audit, not a second
/// pipeline: same parse, same pre-pass, same marker resolution, same job rows,
/// same runner. Only the predicate differs, so a fix to any of that shared
/// machinery reaches both, and the two can never disagree about what a
/// manuscript says.
#[derive(Debug, Clone, PartialEq)]
pub enum AuditScope {
    /// Every significant sentence: uncited ones become `citation_need`, cited
    /// ones `citation_support` or `unverifiable`.
    WholeManuscript,
    /// Only sentences whose markers resolve to ONE library entry.
    ///
    /// Uncited sentences are skipped entirely — "does this sentence need a
    /// citation?" is a question about the manuscript, not about this source,
    /// and it has its own manuscript-level action.
    Citation { library_id: String },
}

/// One manuscript sentence that cites a given source, before any model runs.
///
/// This is the whole point of showing a list first: the sentences are found
/// deterministically (§11 D40 — the pre-pass runs NO model), so the user sees
/// exactly what will be checked, and what it will cost, before agreeing to
/// spend minutes per sentence on it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitingSentence {
    pub seq: usize,
    pub page: Option<u32>,
    pub sentence: String,
    /// The marker text that resolved to this citation, e.g. "(Smith, 2019)".
    pub marker: String,
}

/// What a per-citation check would do, WITHOUT queueing anything.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationAuditPreview {
    pub library_id: String,
    /// `Some` when the cited source is indexed AND embedded, i.e. there is
    /// something to check against. `None` is not an error — it is the
    /// `unverifiable` finding (§11 D40), and the UI shows a Link/Index prompt
    /// rather than a check that cannot run.
    pub document_id: Option<i64>,
    /// Why, when `document_id` is None. Verbatim from marker resolution, which
    /// distinguishes "not linked" from "linked but not indexed" because those
    /// need different things from the user.
    pub unverifiable_reason: Option<String>,
    pub sentences: Vec<CitingSentence>,
    /// Significant sentences in the whole manuscript, for context: "3 of 412".
    pub total_sentences: usize,
    pub document_types_supported: Vec<&'static str>,
}

/// One distinct cited WORK, and whether the audit could check anything against
/// it (§11 D88).
///
/// The grain is the point. `queued_unverifiable` counts SENTENCES, and a
/// researcher does not have forty unverifiable sentences — they have four
/// sources without a PDF, cited forty times. One is a manuscript problem they
/// cannot act on; the other is a library gap with an obvious fix.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitedSourceStatus {
    /// The marker as it appears in the text — `[12]`, `(Smith, 2019)`. What the
    /// reader will recognise, not an internal id.
    pub label: String,
    /// The library entry, when resolution found one.
    pub library_id: Option<String>,
    /// The indexed, embedded document a check would read. `None` = not checkable.
    pub document_id: Option<i64>,
    /// Deterministic, from marker resolution. Distinguishes "not in your
    /// library" from "in your library but no PDF", which need different actions.
    pub reason: Option<String>,
    /// How many sentences in this manuscript cite it.
    pub citing_sentences: usize,
    /// Does the LIBRARY entry record a DOI (§11 D101)?
    ///
    /// This is what decides whether the open-access fetch can do anything —
    /// `oa_fetch` refuses without one — so it is what the card's button keys
    /// on, rather than on the manuscript's list.
    pub has_doi: bool,
    /// Has a retraction registry said this cited work is RETRACTED (§11 D110)?
    ///
    /// Deterministic and registry-backed — Retraction Watch via refverify,
    /// recorded on the library entry, not a model judgement. It is reported
    /// BEFORE the audit runs and at the TOP of the report, because it is more
    /// serious than anything the audit itself can find and it is settled
    /// without spending three hours.
    pub retracted: bool,
}

/// What an audit WOULD do, computed without creating a job or loading a model
/// (§11 D88).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThesisAuditPreview {
    pub total_sentences: usize,
    pub cited: usize,
    pub uncited: usize,
    pub skipped: usize,
    /// What planning would queue, by kind — the same arithmetic `plan_audit`
    /// does, so the card cannot promise a shape the plan will not produce.
    pub would_check: usize,
    pub would_suggest: usize,
    pub would_be_unverifiable: usize,
    /// Per distinct cited work. The card's headline is derived from this.
    pub sources: Vec<CitedSourceStatus>,
    pub checkable_sources: usize,
    pub blocked_sources: usize,
    /// The MANUSCRIPT's numbered reference list, and how many of its entries
    /// record a DOI (§11 D101). Explains WHY nothing can be fetched: an
    /// IEEE-style list routinely carries none, and R PAPER has 25 entries and
    /// zero DOIs.
    pub reference_entries: usize,
    pub reference_entries_with_doi: usize,
    /// §11 D110. How many DISTINCT cited works a registry has confirmed
    /// retracted. Surfaced on the confirmation card so it is seen BEFORE the
    /// three hours are spent, not in the report afterwards.
    pub retracted_sources: usize,
    /// Deterministic consistency findings (§11 D94). No model, and NOT gated on
    /// AI being installed — the user sees these at the moment they decide
    /// whether to spend three hours, because a structural one means the audit's
    /// resolutions cannot be trusted.
    pub consistency: crate::consistency::ConsistencyReport,
    pub document_types_supported: Vec<&'static str>,
}

/// Everything the confirmation card needs, and NOTHING that starts work.
///
/// `plan_thesis_audit` persists a job and the command that wraps it spawns the
/// runner, so by the time a "start?" card could render, generation has begun
/// (§11 D88). This is the honest half of that call: the same parse, the same
/// pre-pass, the same marker resolution, and no side effect at all.
pub fn preview_thesis_audit(
    db: &Database,
    manuscript: &Path,
) -> Result<ThesisAuditPreview, GaplyError> {
    let (report, blocks) = prepass_manuscript_with_blocks(manuscript)?;
    // §11 D94. Deterministic, before anything is queued.
    let consistency = crate::consistency::check_consistency(&blocks, &report);

    // Keyed by the resolution's identity, NOT by the marker text: `[12]` and
    // `Smith (2019)` can be the same work, and counting them twice would
    // overstate the gap the card exists to describe. Falls back to the marker
    // when nothing resolved, which is the only identity available then.
    // §11 D101. `oa_fetch` reads the LIBRARY entry's DOI, so that is what the
    // card's fetch button must key on.
    let has_doi = |library_id: &Option<String>| -> bool {
        library_id
            .as_deref()
            .and_then(|id| crate::citation_library::get(db, id).ok().flatten())
            .and_then(|r| r.doi)
            .map(|d| !d.trim().is_empty())
            .unwrap_or(false)
    };
    // §11 D110. `retracted` is a durable column (Scope B), set only when a
    // registry actually answered. Absence is NOT "clean" — it is unchecked —
    // so this reports only confirmed retractions and never implies the rest
    // were cleared.
    let is_retracted = |library_id: &Option<String>| -> bool {
        library_id
            .as_deref()
            .and_then(|id| crate::citation_library::get(db, id).ok().flatten())
            .map(|r| r.retracted)
            .unwrap_or(false)
    };

    let mut order: Vec<String> = Vec::new();
    let mut by_key: BTreeMap<String, CitedSourceStatus> = BTreeMap::new();
    let (mut would_check, mut would_suggest, mut would_be_unverifiable) = (0usize, 0usize, 0usize);

    for planned in &report.planned {
        if planned.markers.is_empty() {
            would_suggest += 1;
            continue;
        }
        // The SAME first-resolvable-marker rule `plan_audit` uses, so the
        // preview and the plan cannot disagree about one sentence.
        let mut resolution = Resolution::Unverifiable {
            reason: "cited work not in library".to_string(),
            library_id: None,
        };
        let mut marker = planned.markers[0].raw.clone();
        for m in &planned.markers {
            let r = super::audit_prepass::resolve_marker_with(db, m, &report.bibliography)?;
            let checkable = matches!(r, Resolution::Checkable { .. });
            resolution = r;
            marker = m.raw.clone();
            if checkable {
                break;
            }
        }

        let (key, entry) = match &resolution {
            Resolution::Checkable { library_id, document_id } => {
                would_check += 1;
                (
                    format!("lib:{library_id}"),
                    CitedSourceStatus {
                        has_doi: has_doi(&Some(library_id.clone())),
                        retracted: is_retracted(&Some(library_id.clone())),
                        label: marker.clone(),
                        library_id: Some(library_id.clone()),
                        document_id: Some(*document_id),
                        reason: None,
                        citing_sentences: 0,
                    },
                )
            }
            Resolution::Unverifiable { reason, library_id } => {
                would_be_unverifiable += 1;
                (
                    library_id.clone().map(|l| format!("lib:{l}")).unwrap_or(format!("mark:{marker}")),
                    CitedSourceStatus {
                        has_doi: has_doi(library_id),
                        retracted: is_retracted(library_id),
                        label: marker.clone(),
                        library_id: library_id.clone(),
                        document_id: None,
                        reason: Some(reason.clone()),
                        citing_sentences: 0,
                    },
                )
            }
            // `prepass` established there IS a marker, so this cannot occur; a
            // suggestion is the safe reading rather than a panic.
            Resolution::Uncited => {
                would_suggest += 1;
                continue;
            }
        };
        if !by_key.contains_key(&key) {
            order.push(key.clone());
            by_key.insert(key.clone(), entry);
        }
        if let Some(e) = by_key.get_mut(&key) {
            e.citing_sentences += 1;
        }
    }

    let sources: Vec<CitedSourceStatus> =
        order.into_iter().filter_map(|k| by_key.remove(&k)).collect();
    let checkable_sources = sources.iter().filter(|s| s.document_id.is_some()).count();
    // §11 D110. Counted over DISTINCT works, like every other source figure on
    // this card — five sentences citing one retracted paper is one problem.
    let retracted_sources = sources.iter().filter(|s| s.retracted).count();

    Ok(ThesisAuditPreview {
        total_sentences: report.total_sentences,
        cited: report.cited,
        uncited: report.uncited,
        skipped: report.skipped,
        would_check,
        would_suggest,
        would_be_unverifiable,
        blocked_sources: sources.len() - checkable_sources,
        checkable_sources,
        reference_entries: report.bibliography.len(),
        reference_entries_with_doi: report
            .bibliography
            .values()
            .filter(|e| e.doi.as_deref().map(|d| !d.trim().is_empty()).unwrap_or(false))
            .count(),
        retracted_sources,
        sources,
        consistency,
        document_types_supported: SUPPORTED_DOCUMENT_TYPES.to_vec(),
    })
}

/// Formats `parse_path` accepts. Recorded in the plan so a caller never has to
/// guess why a `.doc` or `.tex` was rejected.
pub const SUPPORTED_DOCUMENT_TYPES: &[&str] = &["pdf", "docx", "txt", "md", "text"];

/// Parse a manuscript, plan every item, and persist the job atomically.
///
/// Reuses `extract::docparse::parse_path_paged` — the page-aware sibling of
/// `parse_path` — because a citation finding without a page number is much
/// harder to act on, and page is provenance rather than an estimate.
pub fn plan_thesis_audit(
    db: &Database,
    manuscript: &Path,
    prompt_version: &str,
) -> Result<AuditPlan, GaplyError> {
    plan_audit(db, manuscript, prompt_version, &AuditScope::WholeManuscript)
}

/// Plan a per-citation check: the same pipeline, narrowed to one source.
pub fn plan_citation_audit(
    db: &Database,
    manuscript: &Path,
    prompt_version: &str,
    library_id: &str,
) -> Result<AuditPlan, GaplyError> {
    plan_audit(
        db,
        manuscript,
        prompt_version,
        &AuditScope::Citation { library_id: library_id.to_string() },
    )
}

/// The locator fields EVERY item payload must carry (§11 D72).
///
/// These went in one payload site at a time and were missed twice — the
/// whole-manuscript `citation_support` site, then the whole-manuscript
/// `unverifiable` site — because the plan builds payloads in four places and
/// each one is a separate `json!` literal. A reader checking "did D65 land?"
/// sees the field present at the site they happen to open.
///
/// This does not make omission impossible; a new site can still forget to call
/// it. `every_planned_item_carries_a_locator` is the guard that actually holds
/// the property, over every item the plan emits.
fn locator_payload(planned: &super::audit_prepass::PlannedSentence) -> serde_json::Map<String, serde_json::Value> {
    let mut o = serde_json::Map::new();
    o.insert("paragraph".into(), serde_json::json!(planned.paragraph));
    o.insert("section".into(), serde_json::json!(planned.section));
    o
}

/// Read the manuscript and run the deterministic pre-pass. Shared by planning
/// and by the preview, so what the user is shown and what gets queued come from
/// the same parse rather than two that could drift.
fn prepass_manuscript(manuscript: &Path) -> Result<PrepassReport, GaplyError> {
    prepass_manuscript_with_blocks(manuscript).map(|(r, _)| r)
}

/// The same parse, keeping the BLOCKS — the consistency checks need the
/// captions and headings the significance filter drops (§11 D94), and parsing
/// the manuscript twice to get them would double the wait.
fn prepass_manuscript_with_blocks(
    manuscript: &Path,
) -> Result<(PrepassReport, Vec<crate::extract::docparse::PagedBlock>), GaplyError> {
    let blocks = crate::extract::docparse::parse_path_paged(manuscript)?;
    // `prepass_blocks`, NOT the `(page, text)` shim. The shim discards
    // `PagedBlock.style`, and with it every declared heading and table cell
    // (§11 D65) and the auto-numbered reference ordinals (§11 D67) — on
    // `R PAPER .docx` that was 22 of 25 references. This path is the whole
    // reason those exist, and it was still converting them away.

    // §11 D80: REFUSE OUR OWN REPORT, before a job exists and before anything
    // is spent. Both `plan_audit` and `preview_citation_audit` come through
    // here, so the preview refuses too rather than offering a list the plan
    // would then reject.
    //
    // An audit of an audit runs to completion and looks normal — 98 items, a
    // health score, a full report — while every judgement is about report
    // furniture. The failures are the visible part; the items that "worked"
    // are the danger.
    if let Some(markers) = super::audit_prepass::detect_gaply_report(&blocks) {
        return Err(GaplyError::Validation(format!(
            "this looks like a Gaply audit report, not a manuscript — it contains {}. \
             Auditing a report produces a plausible-looking result about the report's own \
             wording rather than about your writing. Select the manuscript instead.",
            markers.iter().map(|m| format!("“{m}”")).collect::<Vec<_>>().join(", ")
        )));
    }

    let report = prepass_blocks(&blocks);
    Ok((report, blocks))
}

/// Does any of this sentence's markers resolve to `library_id`, and is it
/// checkable? Returns the resolution that decided, plus the marker text.
fn resolve_for_citation(
    db: &Database,
    markers: &[super::audit_prepass::Marker],
    library_id: &str,
    bibliography: &std::collections::BTreeMap<u32, super::audit_prepass::BibEntry>,
) -> Result<Option<(Resolution, String)>, GaplyError> {
    let mut fallback: Option<(Resolution, String)> = None;
    for m in markers {
        match super::audit_prepass::resolve_marker_with(db, m, bibliography)? {
            // A checkable hit on the right work ends the search immediately.
            r @ Resolution::Checkable { .. } => {
                if matches!(&r, Resolution::Checkable { library_id: id, .. } if id == library_id) {
                    return Ok(Some((r, m.raw.clone())));
                }
            }
            // Resolved to this work, but its source is not checkable. Keep it —
            // the sentence DOES cite this source, and saying so with a
            // Link/Index prompt beats dropping it silently — but keep looking
            // in case another marker on the same sentence resolves properly.
            r @ Resolution::Unverifiable { .. } => {
                if matches!(&r, Resolution::Unverifiable { library_id: Some(id), .. } if id == library_id)
                    && fallback.is_none()
                {
                    fallback = Some((r, m.raw.clone()));
                }
            }
            Resolution::Uncited => {}
        }
    }
    Ok(fallback)
}

/// Every manuscript sentence citing one source, with no job created and no
/// model run. The list the user confirms before anything is spent.
pub fn preview_citation_audit(
    db: &Database,
    manuscript: &Path,
    library_id: &str,
) -> Result<CitationAuditPreview, GaplyError> {
    let report = prepass_manuscript(manuscript)?;
    let mut sentences = Vec::new();
    let mut document_id = None;
    let mut unverifiable_reason = None;

    for (seq, planned) in report.planned.iter().enumerate() {
        if planned.markers.is_empty() {
            continue;
        }
        let Some((resolution, marker)) = resolve_for_citation(db, &planned.markers, library_id, &report.bibliography)?
        else {
            continue;
        };
        match &resolution {
            Resolution::Checkable { document_id: doc, .. } => document_id = Some(*doc),
            Resolution::Unverifiable { reason, .. } => {
                if unverifiable_reason.is_none() {
                    unverifiable_reason = Some(reason.clone());
                }
            }
            Resolution::Uncited => {}
        }
        sentences.push(CitingSentence {
            seq,
            page: planned.page,
            sentence: planned.sentence.clone(),
            marker,
        });
    }

    // A checkable source anywhere settles it for the whole set: the reason only
    // describes why a check CANNOT run, and it can.
    if document_id.is_some() {
        unverifiable_reason = None;
    }

    Ok(CitationAuditPreview {
        library_id: library_id.to_string(),
        document_id,
        unverifiable_reason,
        sentences,
        total_sentences: report.total_sentences,
        document_types_supported: SUPPORTED_DOCUMENT_TYPES.to_vec(),
    })
}


/// The page to record for one sentence, and whether it is exact (§11 D95).
///
/// Derived from the page that PRINTS the sentence when that can be identified;
/// otherwise the reflowed block's page, which is a hint and is labelled as one;
/// otherwise nothing. Never a page that might be wrong without saying so.
fn resolve_page(
    locator: &mut crate::page_locate::PageLocator,
    planned: &super::audit_prepass::PlannedSentence,
) -> (Option<u32>, bool) {
    match locator.locate(&planned.sentence) {
        Some(p) => (Some(p), true),
        None => (planned.page, false),
    }
}

fn plan_audit(
    db: &Database,
    manuscript: &Path,
    prompt_version: &str,
    scope: &AuditScope,
) -> Result<AuditPlan, GaplyError> {
    let (report, blocks): (PrepassReport, _) = prepass_manuscript_with_blocks(manuscript)?;

    // §11 D95. The stored page comes from a REFLOWED block, and a block that
    // spans a page break keeps one page for all of it — 8% of locators were off
    // by one. Derive the real page from the per-page text; fall back to the
    // block's page LABELLED approximate; print neither rather than a wrong one.
    let page_texts = crate::extract::docparse::page_texts(manuscript)?.unwrap_or_default();
    let mut locator = crate::page_locate::PageLocator::new(&page_texts);

    let mut items: Vec<NewItem> = Vec::with_capacity(report.planned.len());
    let (mut need, mut support, mut unver) = (0usize, 0usize, 0usize);

    for (seq, planned) in report.planned.iter().enumerate() {
        let (item_page, page_exact) = resolve_page(&mut locator, planned);
        // SCOPED TO ONE CITATION: keep only the sentences that cite it, and
        // queue them exactly as the whole-manuscript audit would. Uncited
        // sentences are not this source's business.
        if let AuditScope::Citation { library_id } = scope {
            if planned.markers.is_empty() {
                continue;
            }
            let Some((resolution, marker)) =
                resolve_for_citation(db, &planned.markers, library_id, &report.bibliography)?
            else {
                continue;
            };
            match resolution {
                Resolution::Checkable { library_id, document_id } => {
                    support += 1;
                    items.push(NewItem {
                        seq: seq as i64,
                        kind: ItemKind::CitationSupport,
                        chunk_id: None,
                        page: item_page,
                        sentence: planned.sentence.clone(),
                        payload_json: serde_json::json!({
                            "documentId": document_id,
                            "libraryId": library_id,
                            "citedSource": marker,
                            "paragraph": planned.paragraph,
                            "section": planned.section,
                            "pageApproximate": !page_exact,
                        })
                        .to_string(),
                    });
                }
                Resolution::Unverifiable { reason, .. } => {
                    unver += 1;
                    items.push(NewItem {
                        seq: seq as i64,
                        kind: ItemKind::Unverifiable,
                        chunk_id: None,
                        page: item_page,
                        sentence: planned.sentence.clone(),
                        payload_json: serde_json::json!({
                            "reason": reason,
                            "paragraph": planned.paragraph,
                            "section": planned.section,
                            "pageApproximate": !page_exact,
                        })
                        .to_string(),
                    });
                }
                Resolution::Uncited => {}
            }
            continue;
        }

        // A sentence with no marker is a candidate for "needs a citation".
        if planned.markers.is_empty() {
            need += 1;
            items.push(NewItem {
                seq: seq as i64,
                kind: ItemKind::CitationNeed,
                chunk_id: None,
                page: item_page,
                sentence: planned.sentence.clone(),
                // The section the sentence sits under. The prompt has a rule
                // keyed on it that could never fire while this was empty.
                payload_json: serde_json::json!({
                    "section": planned.section,
                    "paragraph": planned.paragraph,
                    "pageApproximate": !page_exact,
                })
                .to_string(),
            });
            continue;
        }

        // Cited: checkable only if the source is in the library AND indexed AND
        // embedded. The FIRST resolvable marker decides — a sentence citing
        // three works is one claim, and checking it against one source it
        // actually cites is the honest reading.
        let mut resolution = Resolution::Unverifiable {
            reason: "cited work not in library".to_string(),
            library_id: None,
        };
        for m in &planned.markers {
            let r = super::audit_prepass::resolve_marker_with(db, m, &report.bibliography)?;
            let checkable = matches!(r, Resolution::Checkable { .. });
            resolution = r;
            if checkable {
                break;
            }
        }

        match resolution {
            Resolution::Checkable { library_id, document_id } => {
                support += 1;
                let cited_source = planned
                    .markers
                    .first()
                    .map(|m| m.raw.clone())
                    .unwrap_or_else(|| "cited source".to_string());
                items.push(NewItem {
                    seq: seq as i64,
                    kind: ItemKind::CitationSupport,
                    chunk_id: None,
                    page: item_page,
                    sentence: planned.sentence.clone(),
                    payload_json: serde_json::json!({
                        "documentId": document_id,
                        "libraryId": library_id,
                        "citedSource": cited_source,
                        // D65's locator. This site was MISSED when the other
                        // three payloads got it — the whole-manuscript path is
                        // the one a thesis audit actually takes.
                        "paragraph": planned.paragraph,
                        "section": planned.section,
                        "pageApproximate": !page_exact,
                    })
                    .to_string(),
                });
            }
            Resolution::Unverifiable { reason, .. } => {
                unver += 1;
                items.push(NewItem {
                    seq: seq as i64,
                    kind: ItemKind::Unverifiable,
                    chunk_id: None,
                    page: item_page,
                    sentence: planned.sentence.clone(),
                    payload_json: {
                        let mut o = locator_payload(planned);
                        o.insert("reason".into(), serde_json::json!(reason));
                        serde_json::Value::Object(o).to_string()
                    },
                });
            }
            // `prepass` already established there IS a marker, so this arm is
            // unreachable; treated as uncited rather than panicking.
            Resolution::Uncited => {
                need += 1;
                items.push(NewItem {
                    seq: seq as i64,
                    kind: ItemKind::CitationNeed,
                    chunk_id: None,
                    page: item_page,
                    sentence: planned.sentence.clone(),
                    payload_json: "{}".to_string(),
                });
            }
        }
    }

    // The job KIND records the scope, so a resumed job, a status query and the
    // health report all know whether they are looking at a whole manuscript or
    // one citation's slice of it.
    let job_kind = match scope {
        AuditScope::WholeManuscript => "thesis_audit",
        AuditScope::Citation { .. } => "citation_audit",
    };
    let job_id = jobs::create_job(db, job_kind, None, prompt_version, &items)?;

    // §11 D94. Persisted WITH the job, because the export has only the
    // manuscript's NAME — not its path — so it cannot re-read the file to
    // recompute them, and a researcher wants the list in the report.
    let consistency = crate::consistency::check_consistency(&blocks, &report);
    if !consistency.findings.is_empty() {
        if let Ok(json) = serde_json::to_string(&consistency) {
            jobs::set_job_summary(db, job_id, &json)?;
        }
    }

    Ok(AuditPlan {
        job_id,
        document_types_supported: SUPPORTED_DOCUMENT_TYPES.to_vec(),
        total_sentences: report.total_sentences,
        cited: report.cited,
        uncited: report.uncited,
        skipped: report.skipped,
        markers_found: report.markers_found,
        queued_citation_need: need,
        queued_citation_support: support,
        queued_unverifiable: unver,
    })
}

/// One finding worth a human's attention.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlaggedItem {
    pub seq: i64,
    pub kind: String,
    pub page: Option<u32>,
    pub sentence: String,
    /// The item's recorded result, verbatim. Not summarised: the raw judgement
    /// is what a reader needs to disagree with it.
    pub result: Option<serde_json::Value>,
    /// For support items: which chunks the verdict rested on.
    pub evidence_chunk_ids: Vec<i64>,
}

/// The audit report.
///
/// NOTE: the spec does not define a block by this name; the shape here is the
/// one Phase 8 asked for — counts per category, a per-verdict breakdown, and
/// flagged items carrying sentence, result and evidence refs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThesisHealth {
    pub job_id: i64,
    pub total_items: i64,
    pub completed_items: i64,
    /// `kind` → `status` → count, straight from SQL rather than re-derived.
    pub counts_per_category: BTreeMap<String, BTreeMap<String, i64>>,
    /// citation_support verdicts, and citation_need needs_citation outcomes.
    pub verdict_breakdown: BTreeMap<String, i64>,
    pub unverifiable_reasons: BTreeMap<String, i64>,
    /// Items that were never judged — a missing precondition rather than a
    /// finding. Kept out of `flagged` and out of every verdict tally.
    pub skipped_reasons: BTreeMap<String, i64>,
    pub flagged: Vec<FlaggedItem>,
}

/// Build the report from what the runner has written so far.
///
/// Readable MID-JOB by design: an audit takes hours, and a report that only
/// exists at the end is not one.
pub fn thesis_health(db: &Database, job_id: i64) -> Result<ThesisHealth, GaplyError> {
    let job = jobs::get_job(db, job_id)?
        .ok_or_else(|| GaplyError::NotFound { entity: "job", id: job_id.to_string() })?;

    let mut counts: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for (kind, status, n) in jobs::item_tallies(db, job_id)? {
        *counts.entry(kind).or_default().entry(status).or_insert(0) += n;
    }

    let mut verdicts: BTreeMap<String, i64> = BTreeMap::new();
    let mut reasons: BTreeMap<String, i64> = BTreeMap::new();
    let mut skipped_reasons: BTreeMap<String, i64> = BTreeMap::new();
    let mut flagged = Vec::new();

    // Page through rather than loading 250 results at once.
    let mut offset = 0i64;
    loop {
        let page = jobs::job_results(db, job_id, offset, 100)?;
        if page.is_empty() {
            break;
        }
        offset += page.len() as i64;
        for item in page {
            let Some(raw) = item.result_json.as_deref() else { continue };
            // A SKIPPED item was never judged — a missing precondition, not a
            // finding. Counting it would put "the embedder wasn't installed"
            // in the same list as "this claim is unsupported", which is the
            // kind of blending the three-category split exists to prevent.
            if item.status == "skipped" {
                *skipped_reasons
                    .entry(
                        serde_json::from_str::<serde_json::Value>(raw)
                            .ok()
                            .and_then(|v| {
                                v.get("reason")
                                    .or_else(|| v.get("outcome"))
                                    .and_then(|r| r.as_str())
                                    .map(str::to_string)
                            })
                            .unwrap_or_else(|| "unspecified".to_string()),
                    )
                    .or_insert(0) += 1;
                continue;
            }
            let parsed: serde_json::Value =
                serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);

            // The match is exhaustive and every arm decides, so `flag` is the
            // match's value rather than a mutable that clippy can see is never
            // read before assignment.
            let flag = match item.kind {
                ItemKind::CitationSupport => {
                    let verdict = parsed
                        .get("output")
                        .and_then(|o| o.get("verdict"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| {
                            parsed.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown")
                        });
                    *verdicts.entry(format!("support:{verdict}")).or_insert(0) += 1;
                    // Anything but clean support is worth a human's eye.
                    !matches!(verdict, "strong")
                }
                ItemKind::CitationNeed => {
                    let needs = parsed
                        .get("output")
                        .and_then(|o| o.get("needs_citation"))
                        .and_then(|v| v.as_bool());
                    let key = match needs {
                        Some(true) => "need:needs_citation",
                        Some(false) => "need:no_citation_required",
                        None => "need:unknown",
                    };
                    *verdicts.entry(key.to_string()).or_insert(0) += 1;
                    needs == Some(true)
                }
                ItemKind::Unverifiable => {
                    let reason = parsed
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unspecified")
                        .to_string();
                    *reasons.entry(reason).or_insert(0) += 1;
                    true
                }
            };

            if flag {
                let evidence_chunk_ids = parsed
                    .get("output")
                    .and_then(|o| o.get("supporting_chunks"))
                    .and_then(|c| c.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|c| c.get("chunk_id").and_then(|v| v.as_str()))
                            .filter_map(|s| s.trim_start_matches('c').parse::<i64>().ok())
                            .collect()
                    })
                    .unwrap_or_default();
                flagged.push(FlaggedItem {
                    seq: item.seq,
                    kind: item.kind.as_str().to_string(),
                    page: item.page,
                    sentence: item.sentence.clone(),
                    result: Some(parsed),
                    evidence_chunk_ids,
                });
            }
        }
    }

    Ok(ThesisHealth {
        job_id,
        total_items: job.total_items,
        completed_items: job.done_items,
        counts_per_category: counts,
        verdict_breakdown: verdicts,
        unverifiable_reasons: reasons,
        skipped_reasons,
        flagged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A .docx with a STYLED heading and an auto-numbered reference list —
    /// exactly the shape §11 D65 and D67 exist for, written the way Word writes
    /// it (the ordinals live in numbering.xml, never in the paragraph text).
    fn write_styled_docx(dir: &Path) -> std::path::PathBuf {
        use std::io::Write;
        let para = |style: Option<&str>, text: &str| {
            let ppr = style
                .map(|s| {
                    format!("<w:pPr><w:pStyle w:val=\"{s}\"/><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr></w:pPr>")
                })
                .unwrap_or_default();
            format!("<w:p>{ppr}<w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>")
        };
        let body = [
            para(None, "Deep models often overfit on small corpora [2]."),
            para(Some("Heading1"), "Experimental Configuration"),
            para(None, "All runs used a fixed seed."),
            para(Some("References"), "References"),
            para(Some("ListParagraph"), "First, A. Paper one. J. One, 2001."),
            para(Some("ListParagraph"), "Second, B. Paper two. J. Two, 2002."),
        ]
        .join("");
        let doc = format!(
            "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
        );
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(doc.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let p = dir.join("styled.docx");
        std::fs::write(&p, &buf).unwrap();
        p
    }

    /// §11 D72. EVERY item the plan emits carries a locator — not the ones I
    /// remembered to edit.
    ///
    /// The locator fields went in one payload site at a time and were missed
    /// TWICE: first the whole-manuscript `citation_support` site, then the
    /// whole-manuscript `unverifiable` site, which is why 16 items of a real
    /// audit still printed "no page numbers" while 67 showed a paragraph. This
    /// asserts the property over the plan's OUTPUT, so a site added later is
    /// covered without anyone remembering it exists.
    #[test]
    fn every_planned_item_carries_a_locator() {
        let dir = std::env::temp_dir().join(format!("gaply-locator-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_styled_docx(&dir);
        let db = Database::in_memory().unwrap();

        let plan = plan_thesis_audit(&db, &path, "test-v1").unwrap();
        let items = jobs::job_results(&db, plan.job_id, 0, 500).unwrap();
        assert!(!items.is_empty(), "the fixture produced no items");

        // The fixture is a .docx: no pages, so a paragraph is the ONLY locator
        // available and every item must have one.
        let mut kinds_seen: std::collections::BTreeSet<String> = Default::default();
        for it in &items {
            kinds_seen.insert(format!("{:?}", it.kind));
            let payload: serde_json::Value =
                serde_json::from_str(&it.payload_json).unwrap_or(serde_json::Value::Null);
            assert!(
                payload.get("paragraph").and_then(|v| v.as_u64()).is_some(),
                "{:?} item seq {} has no paragraph locator — a payload site was \
                 added or edited without one. payload: {}",
                it.kind,
                it.seq,
                it.payload_json
            );
        }
        // And the fixture must actually exercise more than one kind, or this
        // test passes by only covering the site that was already right.
        assert!(
            kinds_seen.len() >= 2,
            "the fixture only produced {kinds_seen:?} — it cannot catch a miss in another arm"
        );
    }

    /// §11 D68. THE STANDING CHECK: a pre-pass change must be verified through
    /// the SHIPPED entry point, not the module it edits.
    ///
    /// D65 and D67 both landed in `audit_prepass` and both were measured with
    /// probes calling `prepass_blocks` directly — while `prepass_manuscript`,
    /// the audit's own entry, still converted blocks to `(page, text)` and
    /// called the shim. Every declared heading and every auto-numbered
    /// reference was discarded before the product ever saw it, and two D-entries
    /// described behaviour the product did not have.
    ///
    /// This test goes through the entry point, so that cannot recur silently.
    #[test]
    fn the_shipped_manuscript_path_keeps_declared_structure() {
        let dir = std::env::temp_dir()
            .join(format!("gaply-shipped-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_styled_docx(&dir);
        let pre = prepass_manuscript(&path).expect("the docx must parse");

        // D67: auto-numbered entries carry the ordinal Word displays. The shim
        // returns ZERO here, because the numbers are not in the text.
        assert_eq!(
            pre.bibliography.len(),
            2,
            "the shipped path lost the auto-numbered reference list — it is \
             calling the (page, text) shim again"
        );
        assert!(pre.bibliography[&1].raw.contains("Paper one"));

        // D65: the declared heading reaches the planned sentences as a section.
        assert!(
            pre.planned.iter().any(|p| p.section.as_deref() == Some("Experimental Configuration")),
            "the declared Heading1 never reached the plan: {:?}",
            pre.planned.iter().map(|p| p.section.clone()).collect::<Vec<_>>()
        );
    }

    fn write_fixture(dir: &Path) -> std::path::PathBuf {
        let p = dir.join("chapter.txt");
        std::fs::write(&p, include_str!("testdata/thesis_chapter.txt")).unwrap();
        p
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("gaply-audit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn exec(db: &Database, sql: &str, p: &[&dyn rusqlite::ToSql]) -> i64 {
        let conn = db.conn().unwrap();
        conn.execute(sql, p).unwrap();
        conn.last_insert_rowid()
    }

    fn slice_fixture(dir: &Path) -> std::path::PathBuf {
        let p = dir.join("citing.txt");
        std::fs::write(&p, include_str!("testdata/citing_one_source.txt")).unwrap();
        p
    }

    /// Smith 2019 in the library, its source indexed AND embedded → checkable.
    fn seed_checkable_smith(db: &Database) -> i64 {
        exec(
            db,
            "INSERT INTO citation_library (id, csl_json, title, authors, year, created_at, updated_at)
             VALUES ('lib-smith', '{}', 'Organic Management and Soil Life', 'Smith, J.', 2019, 1, 1)",
            &[],
        );
        let doc = exec(
            db,
            "INSERT INTO documents (source_type, title, fetched_at, checksum, status, created_at)
             VALUES ('pdf', 'organic management and soil life', 1, 'ck-slice', 'ready', 1)",
            &[],
        );
        let chunk = exec(
            db,
            "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content,
                                    token_estimate, content_hash, created_at)
             VALUES (?1, 4, 0, 10, 'richness rose', 3, 'h-slice', 1)",
            &[&doc],
        );
        exec(
            db,
            "INSERT INTO ai_model_registry (id, kind, display_name, file_path, registered_at)
             VALUES ('bge-small-en-v1.5', 'embedding', 'bge', '/x', 1)",
            &[],
        );
        exec(
            db,
            "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
             VALUES (?1, 'bge-small-en-v1.5', 'bge-v1.5-p2', 1, X'00', 1)",
            &[&chunk],
        );
        crate::citation_links::link_citations(db).unwrap();
        doc
    }

    /// THE per-citation slice. Three manuscript sentences cite Smith 2019; the
    /// preview finds all three with NO model and NO job, which is what the user
    /// confirms before any time is spent (§11 D40 — the pre-pass is
    /// deterministic).
    #[test]
    fn preview_finds_every_sentence_citing_one_source_and_nothing_else() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("preview");
        let path = slice_fixture(&dir);
        let doc = seed_checkable_smith(&db);

        let p = preview_citation_audit(&db, &path, "lib-smith").unwrap();

        assert_eq!(p.sentences.len(), 3, "expected 3 citing sentences, got {:?}", p.sentences);
        assert_eq!(p.document_id, Some(doc), "the checkable source was not resolved");
        assert!(p.unverifiable_reason.is_none());
        assert!(p.total_sentences > 3, "the preview lost the manuscript's own size");
        for s in &p.sentences {
            assert!(s.sentence.contains("Smith, 2019"), "unrelated sentence kept: {s:?}");
            assert_eq!(s.marker, "(Smith, 2019)");
        }
        // The Jones sentence cites a DIFFERENT source and must not appear.
        assert!(!p.sentences.iter().any(|s| s.sentence.contains("Jones")));
        // Uncited sentences are not this source's business — that question has
        // its own manuscript-level action.
        assert!(!p.sentences.iter().any(|s| s.sentence.contains("tropical systems")));

        // No job was created: a preview costs nothing and commits to nothing.
        assert!(jobs::get_job(&db, 1).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Confirming queues exactly those sentences, as ordinary job items on the
    /// existing runner — the same rows a whole-manuscript audit would produce.
    #[test]
    fn planning_the_slice_queues_one_support_item_per_citing_sentence() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("slice-plan");
        let path = slice_fixture(&dir);
        seed_checkable_smith(&db);

        let plan = plan_citation_audit(&db, &path, "citation_support-v1.4", "lib-smith").unwrap();

        assert_eq!(plan.queued_citation_support, 3);
        assert_eq!(plan.queued_unverifiable, 0);
        // NOT the manuscript-level question, even though the fixture is full of
        // uncited sentences.
        assert_eq!(plan.queued_citation_need, 0);

        let items = jobs::job_results(&db, plan.job_id, 0, 50).unwrap();
        assert_eq!(items.len(), 3);
        for it in &items {
            assert_eq!(it.kind, ItemKind::CitationSupport);
            let payload: serde_json::Value = serde_json::from_str(&it.payload_json).unwrap();
            assert_eq!(payload["libraryId"], "lib-smith");
            assert_eq!(payload["citedSource"], "(Smith, 2019)");
        }
        let job = jobs::get_job(&db, plan.job_id).unwrap().unwrap();
        assert_eq!(job.kind, "citation_audit", "the slice must be distinguishable from a full audit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The unlinked source. The sentences ARE found — they genuinely cite this
    /// work — and each becomes an `unverifiable` item carrying the reason,
    /// rather than being dropped or presented as a check that could run.
    #[test]
    fn an_unlinked_source_still_finds_its_sentences_and_says_why_it_cannot_check() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("slice-unlinked");
        let path = slice_fixture(&dir);
        exec(
            &db,
            "INSERT INTO citation_library (id, csl_json, title, authors, year, created_at, updated_at)
             VALUES ('lib-smith', '{}', 'Organic Management and Soil Life', 'Smith, J.', 2019, 1, 1)",
            &[],
        );

        let p = preview_citation_audit(&db, &path, "lib-smith").unwrap();
        assert_eq!(p.sentences.len(), 3, "sentences were dropped for an unlinked source");
        assert_eq!(p.document_id, None);
        assert!(
            p.unverifiable_reason.as_deref().unwrap_or_default().contains("no indexed document"),
            "{:?}",
            p.unverifiable_reason
        );

        let plan = plan_citation_audit(&db, &path, "citation_support-v1.4", "lib-smith").unwrap();
        assert_eq!(plan.queued_unverifiable, 3);
        assert_eq!(plan.queued_citation_support, 0, "an unlinked source must cost ZERO model calls");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A citation nothing cites. Not an error, and not an empty job.
    #[test]
    fn a_source_no_sentence_cites_previews_as_zero() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("slice-none");
        let path = slice_fixture(&dir);
        seed_checkable_smith(&db);

        let p = preview_citation_audit(&db, &path, "lib-nobody-cites-this").unwrap();
        assert!(p.sentences.is_empty());
        assert_eq!(p.document_id, None);
        assert!(p.total_sentences > 0, "the manuscript was still read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// End to end, no model: a real .txt chapter becomes a persisted plan whose
    /// item kinds match what the pre-pass measured.
    #[test]
    fn planning_a_chapter_queues_items_and_records_what_it_measured() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("plan");
        let path = write_fixture(&dir);

        let plan = plan_thesis_audit(&db, &path, "citation_need-v2").unwrap();

        assert_eq!(plan.markers_found, 20, "the planner disagreed with the pre-pass");
        assert!(plan.total_sentences > 20);
        assert!(plan.uncited > 0, "the chapter has uncited claims");

        // Nothing is in the library, so every cited sentence is unverifiable —
        // and that is a RESULT, queued rather than dropped (§11 D40).
        assert_eq!(plan.queued_citation_support, 0);
        assert!(plan.queued_unverifiable >= 19, "cited sentences were dropped: {plan:?}");
        assert!(plan.queued_citation_need > 0);

        let queued = plan.queued_citation_need + plan.queued_citation_support + plan.queued_unverifiable;
        let job = jobs::get_job(&db, plan.job_id).unwrap().unwrap();
        assert_eq!(job.total_items as usize, queued, "planned and persisted counts disagree");
        assert_eq!(job.status, jobs::JobStatus::Queued);

        // the references section contributed nothing
        let items = jobs::job_results(&db, plan.job_id, 0, 500).unwrap();
        assert!(!items.iter().any(|i| i.sentence.contains("Journal of Soil Biology")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Phase 8b, and the number the findings report. Same chapter, same
    /// planner, one linked source — a sentence that was `unverifiable` becomes
    /// a real `citation_support` item the audit can actually check.
    #[test]
    fn linking_one_source_turns_an_unverifiable_sentence_into_a_support_item() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("link");
        let path = write_fixture(&dir);

        // ---- BEFORE: nothing in the library ----
        let before = plan_thesis_audit(&db, &path, "citation_need-v2").unwrap();
        assert_eq!(before.queued_citation_support, 0);
        let unverifiable_before = before.queued_unverifiable;
        assert!(unverifiable_before >= 19, "{before:?}");

        // ---- link ONE source, deliberately under a DIFFERENT title so the
        // old title-only join could not have found it ----
        {
            let conn = db.conn().unwrap();
            conn.execute(
                "INSERT INTO citation_library (id, csl_json, doi, title, authors, year, created_at, updated_at)
                 VALUES ('lib-smith', '{}', '10.1234/soil', 'Soil invertebrates under intensification',
                         'Smith, J.', 2019, 1, 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO documents (source_type, title, source_url, fetched_at, checksum, status, created_at)
                 VALUES ('pdf', 'smith_2019_scan_FINAL_v2', 'https://doi.org/10.1234/soil', 1, 'ck-s', 'ready', 1)",
                [],
            )
            .unwrap();
            let doc = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO ai_model_registry (id, kind, display_name, file_path, registered_at)
                 VALUES ('emb', 'embedding', 'emb', '/x', 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO ai_chunks (document_id, page, char_start, char_end, content,
                                        token_estimate, content_hash, created_at)
                 VALUES (?1, 1, 0, 20, 'richness rose 31 percent', 4, 'h-s', 1)",
                rusqlite::params![doc],
            )
            .unwrap();
            let chunk = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO ai_chunk_embeddings (chunk_id, model_id, preprocessing_version, dim, vector, created_at)
                 VALUES (?1, 'emb', 'p1', 1, X'00', 1)",
                rusqlite::params![chunk],
            )
            .unwrap();
        }
        let report = crate::citation_links::link_citations(&db).unwrap();
        assert_eq!(report.linked_by_doi, 1, "the DOI link did not form");
        assert_eq!(report.linked_by_title, 0, "the titles differ; a title match would be wrong");

        // ---- AFTER: replan ----
        let after = plan_thesis_audit(&db, &path, "citation_need-v2").unwrap();
        assert_eq!(
            after.queued_citation_support, 1,
            "the linked source did not become a checkable item"
        );
        assert_eq!(
            after.queued_unverifiable,
            unverifiable_before - 1,
            "exactly one sentence should have moved from unverifiable to support"
        );
        assert_eq!(
            after.queued_citation_need, before.queued_citation_need,
            "linking must not disturb uncited sentences"
        );

        // and the item carries the document the audit will retrieve from
        let items = jobs::job_results(&db, after.job_id, 0, 500).unwrap();
        let support = items.iter().find(|i| i.kind == ItemKind::CitationSupport).unwrap();
        assert!(support.payload_json.contains("documentId"));
        assert!(support.sentence.contains("(Smith, 2019)"), "{}", support.sentence);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §11 D80. Auditing our own exported report is refused before a job exists.
    ///
    /// This actually happened: 98 items, a health score and a full report, all
    /// of it judging report furniture. The five items that failed validation
    /// were the visible part — the 72 that "succeeded" are the danger, because
    /// a researcher reads those.
    #[test]
    fn a_gaply_audit_report_is_refused_as_a_manuscript() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("selfaudit");
        let p = dir.join("audit.txt");
        // The real skeleton, including the footer-glued-to-heading shape the
        // PDF extractor actually produced.
        std::fs::write(
            &p,
            "Thesis citation audit\nR PAPER .pdf\n\nAt a glance\n\
             The counts\n84 sentences were read and 79 were checked against a source \
             or judged for whether they need one.\n\
             Claims checked against their source\nEach sentence below cites a source \
             Gaply could read.\n\
             PublishReady 3 Sentences that may need a citation These sentences carry \
             no citation.\n",
        )
        .unwrap();

        let err = plan_thesis_audit(&db, &p, "citation_need-v4").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Gaply audit report"), "{msg}");
        // The refusal NAMES what it matched, so a false positive is arguable.
        assert!(msg.contains("Thesis citation audit"), "the markers are not named: {msg}");
        // And nothing was queued.
        assert!(jobs::get_job(&db, 1).unwrap().is_none(), "a job was created anyway");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §11 D80. One or two markers is NOT enough, and must not be.
    ///
    /// A manuscript may quote a heading; a paper about this tool would name
    /// several. Refusing on a single hit would block real work to prevent a
    /// mistake, so the threshold is three and this holds it there.
    #[test]
    fn a_manuscript_that_merely_mentions_the_report_is_not_refused() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("mentions");
        let p = dir.join("thesis.txt");
        std::fs::write(
            &p,
            "Introduction\nRecent tools produce a thesis citation audit automatically [1]. \
             We evaluate whether such an audit is worth a second look for research writing. \
             Prior work established that citation screening improves accuracy [2].\n",
        )
        .unwrap();

        let plan = plan_thesis_audit(&db, &p, "citation_need-v4")
            .expect("a manuscript mentioning two markers must not be refused");
        assert!(plan.total_sentences > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §11 D88. A preview must create NOTHING — that is its entire reason to
    /// exist. `plan_thesis_audit` persists a job and the command spawns the
    /// runner, so a card built on it is asking permission for work already
    /// under way.
    #[test]
    fn a_preview_creates_no_job_and_no_items() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("preview-none");
        let p = dir.join("thesis.txt");
        std::fs::write(
            &p,
            "Organic farming increases soil microbial biomass by roughly a third [1].\n\
             The authors measured this themselves on their own plots this season.\n\
             References\n[1] R. Smith, Soil biology and management, 2019.\n",
        )
        .unwrap();

        let pre = preview_thesis_audit(&db, &p).unwrap();
        assert!(pre.total_sentences > 0, "the pre-pass read nothing");
        assert!(jobs::get_job(&db, 1).unwrap().is_none(), "the preview created a job");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §11 D88. The grain is the whole point: SOURCES, not sentences.
    ///
    /// A work cited eleven times is ONE thing to fix, and reporting eleven
    /// unverifiable sentences describes a manuscript problem the researcher
    /// cannot act on instead of a library gap they can.
    #[test]
    fn a_preview_counts_sources_not_sentences() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("preview-sources");
        let p = dir.join("thesis.txt");
        // ONE cited work, THREE citing sentences.
        std::fs::write(
            &p,
            "Soil microbial biomass rises by roughly a third under organic management [1].\n\
             A later synthesis reported the same direction of effect across Europe [1].\n\
             The effect is strongest on previously degraded arable land [1].\n\
             References\n[1] R. Smith, Soil biology and management, 2019.\n",
        )
        .unwrap();

        let pre = preview_thesis_audit(&db, &p).unwrap();
        assert_eq!(pre.sources.len(), 1, "one work cited three times must be ONE source: {:?}", pre.sources);
        assert_eq!(pre.sources[0].citing_sentences, 3, "{:?}", pre.sources);
        assert_eq!(pre.checkable_sources, 0, "nothing is in the library");
        assert_eq!(pre.blocked_sources, 1);
        // And the sentence-grain number still exists, because the estimate needs
        // it — it simply is not what the headline is built from.
        assert_eq!(pre.would_be_unverifiable, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §11 D88. The preview and the plan must not disagree about one sentence:
    /// a card that promises 12 checks and a run that performs 4 is worse than
    /// no card.
    #[test]
    fn the_preview_matches_what_planning_actually_queues() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("preview-agrees");
        let p = dir.join("thesis.txt");
        std::fs::write(
            &p,
            "Soil microbial biomass rises by roughly a third under organic management [1].\n\
             Every experiment reported here was run on our own instrument this season.\n\
             A later synthesis reported the same direction of effect across Europe [1].\n\
             References\n[1] R. Smith, Soil biology and management, 2019.\n",
        )
        .unwrap();

        let pre = preview_thesis_audit(&db, &p).unwrap();
        let plan = plan_thesis_audit(&db, &p, "citation_need-v4").unwrap();

        assert_eq!(pre.would_check, plan.queued_citation_support, "support count drifted");
        assert_eq!(pre.would_suggest, plan.queued_citation_need, "need count drifted");
        assert_eq!(pre.would_be_unverifiable, plan.queued_unverifiable, "unverifiable count drifted");
        assert_eq!(pre.total_sentences, plan.total_sentences);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unsupported_format_is_refused_before_any_work() {
        let db = Database::in_memory().unwrap();
        let dir = tmpdir("fmt");
        let p = dir.join("thesis.tex");
        std::fs::write(&p, "\\section{Intro}").unwrap();
        let err = plan_thesis_audit(&db, &p, "citation_need-v2").unwrap_err();
        assert!(err.to_string().contains("unsupported"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The report is readable MID-JOB, and it separates the three categories
    /// rather than blending them into one accuracy figure.
    #[test]
    fn thesis_health_reports_categories_verdicts_and_flags_mid_job() {
        let db = Database::in_memory().unwrap();
        let job = jobs::create_job(
            &db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[
                NewItem {
                    seq: 0,
                    kind: ItemKind::CitationNeed,
                    chunk_id: None,
                    page: Some(1),
                    sentence: "Richness rose sharply across every plot.".into(),
                    payload_json: "{}".into(),
                },
                NewItem {
                    seq: 1,
                    kind: ItemKind::CitationSupport,
                    chunk_id: None,
                    page: Some(2),
                    sentence: "Organic management increased richness (Smith, 2019).".into(),
                    payload_json: "{}".into(),
                },
                NewItem {
                    seq: 2,
                    kind: ItemKind::Unverifiable,
                    chunk_id: None,
                    page: Some(3),
                    sentence: "Leaching fell (Jones, 2020).".into(),
                    payload_json: "{}".into(),
                },
                // deliberately left queued: the report must work before the end
                NewItem {
                    seq: 3,
                    kind: ItemKind::CitationNeed,
                    chunk_id: None,
                    page: Some(4),
                    sentence: "A further uncited claim about soil fauna abundance.".into(),
                    payload_json: "{}".into(),
                },
            ],
        )
        .unwrap();

        let a = jobs::claim_next_item(&db, job).unwrap().unwrap();
        jobs::complete_item(
            &db,
            a.id,
            "done",
            Some(r#"{"category":"citation_need","output":{"needs_citation":true}}"#),
            None,
        )
        .unwrap();
        let b = jobs::claim_next_item(&db, job).unwrap().unwrap();
        jobs::complete_item(
            &db,
            b.id,
            "done",
            Some(r#"{"category":"citation_support","output":{"verdict":"weak","supporting_chunks":[{"chunk_id":"c7"},{"chunk_id":"c9"}]}}"#),
            None,
        )
        .unwrap();
        let c = jobs::claim_next_item(&db, job).unwrap().unwrap();
        jobs::complete_item(
            &db,
            c.id,
            "done",
            Some(r#"{"category":"unverifiable","reason":"cited work not in library"}"#),
            None,
        )
        .unwrap();

        let h = thesis_health(&db, job).unwrap();
        assert_eq!(h.total_items, 4);
        assert_eq!(h.completed_items, 3, "the report must work mid-job");

        assert_eq!(h.counts_per_category["citation_need"]["done"], 1);
        assert_eq!(h.counts_per_category["citation_need"]["queued"], 1);
        assert_eq!(h.counts_per_category["citation_support"]["done"], 1);

        assert_eq!(h.verdict_breakdown["support:weak"], 1);
        assert_eq!(h.verdict_breakdown["need:needs_citation"], 1);
        assert_eq!(h.unverifiable_reasons["cited work not in library"], 1);

        // all three completed items are flagged: a weak verdict, a sentence
        // that needs a citation, and an unverifiable source
        assert_eq!(h.flagged.len(), 3, "{:#?}", h.flagged);
        let support = h.flagged.iter().find(|f| f.kind == "citation_support").unwrap();
        assert_eq!(support.page, Some(2));
        assert_eq!(support.evidence_chunk_ids, vec![7, 9], "evidence refs were lost");
        assert!(support.result.is_some(), "the raw judgement must survive into the report");
    }

    /// Found by the real-model smoke: capped/skipped items were appearing as
    /// unverifiable FINDINGS. An item that was never judged is a missing
    /// precondition, and listing it beside "this claim is unsupported" is
    /// exactly the blending the three-category split exists to prevent.
    #[test]
    fn a_skipped_item_is_not_a_finding() {
        let db = Database::in_memory().unwrap();
        let job = jobs::create_job(
            &db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[NewItem {
                seq: 0,
                kind: ItemKind::Unverifiable,
                chunk_id: None,
                page: Some(1),
                sentence: "Leaching fell (Jones, 2020).".into(),
                payload_json: "{}".into(),
            }],
        )
        .unwrap();
        let it = jobs::claim_next_item(&db, job).unwrap().unwrap();
        jobs::complete_item(&db, it.id, "skipped", Some(r#"{"reason":"capped by the operator"}"#), None)
            .unwrap();

        let h = thesis_health(&db, job).unwrap();
        assert!(h.flagged.is_empty(), "a skipped item was reported as a finding: {:?}", h.flagged);
        assert!(h.unverifiable_reasons.is_empty(), "a skipped item polluted the verdict tallies");
        assert_eq!(h.skipped_reasons["capped by the operator"], 1, "the skip was not recorded");
    }

    /// A clean `strong` verdict is NOT flagged — otherwise the report is just
    /// the item list again and the reader has been given nothing.
    #[test]
    fn a_clean_strong_verdict_is_not_flagged() {
        let db = Database::in_memory().unwrap();
        let job = jobs::create_job(
            &db,
            "thesis_audit",
            None,
            "citation_support-v1.4",
            &[NewItem {
                seq: 0,
                kind: ItemKind::CitationSupport,
                chunk_id: None,
                page: Some(1),
                sentence: "Organic management increased richness (Smith, 2019).".into(),
                payload_json: "{}".into(),
            }],
        )
        .unwrap();
        let it = jobs::claim_next_item(&db, job).unwrap().unwrap();
        jobs::complete_item(
            &db,
            it.id,
            "done",
            Some(r#"{"category":"citation_support","output":{"verdict":"strong","supporting_chunks":[{"chunk_id":"c1"}]}}"#),
            None,
        )
        .unwrap();
        let h = thesis_health(&db, job).unwrap();
        assert_eq!(h.verdict_breakdown["support:strong"], 1);
        assert!(h.flagged.is_empty(), "a clean strong verdict should not need review");
    }
}
