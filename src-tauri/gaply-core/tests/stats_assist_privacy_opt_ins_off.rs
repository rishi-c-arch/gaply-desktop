//! **With every opt-in off, no sentinel appears in any request body, error or trace.** (Phase 2, Step 5's sentinels.)
//!
//! Alone in its binary: it asserts absence from captured tracing, and a
//! capture shares callsite state with every test thread in its binary. See
//! `stats_assist_support`.

mod stats_assist_support;

use stats_assist_support::*;

#[test]
fn with_every_opt_in_off_no_sentinel_appears_in_any_body_error_or_trace() {
    let (bodies, errors, lines) = flow(false, false);
    assert_eq!(bodies.len(), 8, "four previews and four sent bodies");
    assert!(lines.iter().filter(|l| l.contains("stats_assist")).count() >= 8, "the flow emitted tracing: {lines:?}");
    assert!(errors.len() >= 16, "the refusals ran");
    let texts: Vec<String> = bodies.iter().map(|(_, b)| b.clone()).collect();
    assert_absent("a request body", &texts, &SENTINELS);
    assert_absent("an error", &errors, &SENTINELS);
    assert_absent("tracing", &lines, &SENTINELS);
}
