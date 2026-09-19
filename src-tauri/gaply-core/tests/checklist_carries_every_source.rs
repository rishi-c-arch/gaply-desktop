//! **A requirement stated on six pages must not be shown as one page picked by
//! insertion order. §11 D188.**
//!
//! `requirements_for` is `ORDER BY id DESC`, and `checklist_from_requirements`
//! used to take the FIRST match per statement needle — so the sentence a
//! researcher read was whichever the crawl stored LAST. On Nature Medicine that
//! surfaced *"all fast track submissions must include…"* for data availability
//! while *"must be included with all original research manuscripts"* sat unread
//! in the same table.
//!
//! # Driven by the SHIPPED SEED, not a fixture
//!
//! The rows come from `load_bundled_seed` and the item from `build_checklist`.
//! A hand-built `StoredRequirement` would inherit whatever this author believes
//! a contested group looks like, and the whole point of D188 is that the
//! contest is a property of real journals: 13 of 25 groups across the nine
//! seeded journals, in five different publishers' houses styles.

use gaply_core::{extract, journal_store, report, Database};

/// Enough manuscript for `build_checklist` to run. Deliberately contains NONE of
/// the statements, so every statement row is NOT MET and the test is about
/// SOURCES rather than about matching.
const MANUSCRIPT: &str = "\
Title: A Study

Abstract
We examined something.

Methods
We did a thing.

Results
It worked.
";

fn checklist(journal: &str) -> Vec<report::ChecklistItem> {
    let db = Database::in_memory().expect("db");
    let seeded = journal_store::load_bundled_seed(&db).expect("seed loads");
    assert!(
        seeded.seeded.contains(&journal.to_string()),
        "the seed must carry {journal}, or this test measures an empty table: {:?}",
        seeded.seeded
    );
    let ex = extract::extract_from_text(MANUSCRIPT);
    report::build_checklist(&db, &ex, MANUSCRIPT, None, Some(journal)).expect("checklist")
}

fn row<'a>(items: &'a [report::ChecklistItem], needle: &str) -> &'a report::ChecklistItem {
    let hits: Vec<&report::ChecklistItem> =
        items.iter().filter(|i| i.requirement.contains(needle)).collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one row per statement — grouping must not change the row count: {:?}",
        hits.iter().map(|i| &i.requirement).collect::<Vec<_>>()
    );
    hits[0]
}

fn sources(i: &report::ChecklistItem) -> usize {
    usize::from(i.source_span.is_some()) + i.also_from.len()
}

#[test]
fn a_requirement_the_journal_states_on_several_pages_carries_all_of_them() {
    let items = checklist("nature-medicine");

    // POSITIVE COUNT FIRST. An empty checklist, or one where no row is
    // contested, would satisfy every assertion below vacuously.
    assert!(!items.is_empty(), "no checklist rows at all — the guard is inert");
    let contested = items.iter().filter(|i| !i.also_from.is_empty()).count();
    assert!(
        contested >= 3,
        "expected several contested rows on Nature Medicine; found {contested}. If the seed \
         changed this may be legitimate, but a run with none cannot test carrying sources."
    );

    // Competing interests is stated on four Nature Medicine pages.
    let ci = row(&items, "competing interests");
    assert_eq!(sources(ci), 4, "every page that states it: {:#?}", ci.also_from);

    // The per-source article_type is what distinguishes "for Matters Arising"
    // from "for everything", and it must survive the grouping.
    assert!(
        ci.also_from.iter().any(|s| s.article_type.as_deref() == Some("Matters Arising")),
        "the Matters Arising binding must remain visible: {:#?}",
        ci.also_from
    );
}

#[test]
fn the_sentence_that_was_being_discarded_is_present() {
    let items = checklist("nature-medicine");
    let da = row(&items, "data availability");

    let all: Vec<&str> = da
        .source_span
        .iter()
        .map(String::as_str)
        .chain(da.also_from.iter().map(|s| s.source_span.as_str()))
        .collect();

    // THE DEFECT, named exactly. Before D188 this row carried only the fast
    // track span; the general one was in the same table and unreachable.
    assert!(
        all.iter().any(|s| s.contains("all original research manuscripts")),
        "the unconditional data-availability sentence must be carried: {all:#?}"
    );
    // And the narrow one is still there — showing BOTH is the point, because a
    // reader can tell which scope covers them and the code cannot.
    assert!(
        all.iter().any(|s| s.to_lowercase().contains("fast track")),
        "the fast-track span is not hidden either: {all:#?}"
    );
}
