//! **With "column headers" on, only the header appears, and only in binding_suggestion bodies.** (Phase 2, Step 5's sentinels.)
//!
//! Alone in its binary: it asserts absence from captured tracing, and a
//! capture shares callsite state with every test thread in its binary. See
//! `stats_assist_support`.

mod stats_assist_support;

use stats_assist_support::*;

#[test]
fn with_column_headers_on_only_the_header_appears_and_only_in_binding_suggestion() {
    let (bodies, errors, lines) = flow(false, true);
    let binding: Vec<String> = bodies.iter().filter(|(t, _)| *t == HelperTask::BindingSuggestion).map(|(_, b)| b.clone()).collect();
    let others: Vec<String> = bodies.iter().filter(|(t, _)| *t != HelperTask::BindingSuggestion).map(|(_, b)| b.clone()).collect();
    assert_eq!(binding.len(), 2);
    assert!(binding.iter().all(|b| b.contains(HEADER)), "the opted-in header is in the binding body");
    assert_absent("a binding body", &binding, &[CELL, LEVEL, MS, PATH]);
    assert_absent("another task's body", &others, &SENTINELS);
    assert_absent("an error", &errors, &SENTINELS);
    assert_absent("tracing", &lines, &SENTINELS);
}
