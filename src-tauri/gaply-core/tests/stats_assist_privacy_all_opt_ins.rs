//! **With every opt-in on, no cell, level label, path or out-of-sentence manuscript text leaves.** (Phase 2, Step 5's sentinels.)
//!
//! Alone in its binary: it asserts absence from captured tracing, and a
//! capture shares callsite state with every test thread in its binary. See
//! `stats_assist_support`.

mod stats_assist_support;

use stats_assist_support::*;

#[test]
fn no_opt_in_ever_lets_a_cell_a_level_label_or_a_path_through() {
    let (bodies, errors, lines) = flow(true, true);
    let texts: Vec<String> = bodies.iter().map(|(_, b)| b.clone()).collect();
    assert_absent("a body with every opt-in on", &texts, &[CELL, LEVEL, PATH, MS]);
    // What every opt-in DID let through, so the test is not vacuous.
    let cm = &bodies.iter().find(|(t, _)| *t == HelperTask::ClaimMethod).unwrap().1;
    assert!(cm.contains("assuming equal variances") && cm.contains("scored lower than control"), "{cm}");
    let web = &bodies.iter().find(|(t, _)| *t == HelperTask::WebContext).unwrap().1;
    assert!(web.contains("10.5555/example.2026.01"));
    assert_absent("an error", &errors, &SENTINELS);
    assert_absent("tracing", &lines, &SENTINELS);
}
