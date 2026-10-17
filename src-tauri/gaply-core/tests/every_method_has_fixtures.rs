//! **Every entry in `MethodRegistry` has fixture files with cases in them, and
//! a tolerance rule that is a rule.**
//!
//! `registry.rs`'s header: a method is registered only when it has an
//! implementation, golden fixtures, a tolerance rule and edge-case tests. Its
//! own unit test checks that the named fixture FILES exist. This adds what a
//! file's existence does not show:
//!
//! - **Each fixture holds at least one case.** A file reduced to its header
//!   still exists. Rows are counted after `#` comment lines and the column
//!   line, the format every `stats_l0` fixture uses.
//! - **Each method's tolerance rule is well formed.** `tolerance()` is part of
//!   the method contract (`stats_l0::method::VerificationMethod`), so its
//!   presence is a compile error to omit; what can still go wrong is the value.
//!   A `RelativeOutward` margin must be finite and above zero, and below 1e-3
//!   so it stays an engine margin rather than a second rounding rule; its cap
//!   must be in (0, 1]. `RoundingOnly` is always well formed.
//!
//! The registry is walked through `MethodRegistry`, never a list typed here, so
//! a method registered tomorrow is covered by a test nobody updates.
//!
//! # What it cannot catch
//!
//! Whether the fixtures are the RIGHT ones. `l0_p_reference.rs`,
//! `l0_grim_reference.rs`, `l0_arithmetic_fixtures.rs` and
//! `l0_statcheck_departures.rs` run each method against them; this guard only
//! shows there is something for those tests to run.

use gaply_core::stats_l0::method::ToleranceRule;
use gaply_core::stats_l0::registry::MethodRegistry;

/// Data rows in a fixture: lines that are not `#` comments, not blank, and not
/// the first remaining line (the column line). `None` when the file cannot be read.
fn data_rows(fixture: &str) -> Option<usize> {
    let path = format!("{}/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .count()
            .saturating_sub(1),
    )
}

/// `Err` with the reason when a tolerance rule is not a usable rule.
fn well_formed(rule: ToleranceRule) -> Result<(), String> {
    match rule {
        ToleranceRule::RoundingOnly => Ok(()),
        ToleranceRule::RelativeOutward { margin, cap } => {
            if !(margin.is_finite() && margin > 0.0 && margin < 1e-3) {
                return Err(format!("margin {margin} is not in (0, 1e-3)"));
            }
            if !(cap.is_finite() && cap > 0.0 && cap <= 1.0) {
                return Err(format!("cap {cap} is not in (0, 1]"));
            }
            Ok(())
        }
    }
}

#[test]
fn every_method_has_fixtures() {
    let entries = MethodRegistry::LEVEL_0
        .iter()
        .map(|m| (m.method.id(), m.method.tolerance(), m.fixtures))
        .chain(
            MethodRegistry::SUMMARY
                .iter()
                .map(|m| (m.method.id(), m.method.tolerance(), m.fixtures)),
        );

    let mut problems = Vec::new();
    let mut methods = 0;
    for (id, tolerance, fixtures) in entries {
        methods += 1;
        if fixtures.is_empty() {
            problems.push(format!("{id}: names no fixture file"));
        }
        for f in fixtures {
            match data_rows(f) {
                Some(0) => problems.push(format!("{id}: {f} has no cases")),
                Some(_) => {}
                None => problems.push(format!("{id}: {f} cannot be read")),
            }
        }
        if let Err(why) = well_formed(tolerance) {
            problems.push(format!("{id}: tolerance {tolerance:?}: {why}"));
        }
    }
    // Six on 10 Oct 2026: three in LEVEL_0, three in SUMMARY. A registry that
    // silently empties would pass the loop above.
    assert!(methods >= 6, "only {methods} registered methods found");
    assert!(
        problems.is_empty(),
        "every registered method needs fixtures with cases and a tolerance rule:\n{}",
        problems.join("\n")
    );
}

/// The tolerance check must refuse the values it exists to refuse.
#[test]
fn the_guard_refuses_a_tolerance_that_is_not_a_rule() {
    for bad in [
        ToleranceRule::RelativeOutward {
            margin: 0.0,
            cap: 1.0,
        },
        ToleranceRule::RelativeOutward {
            margin: f64::NAN,
            cap: 1.0,
        },
        ToleranceRule::RelativeOutward {
            margin: 0.05,
            cap: 1.0,
        },
        ToleranceRule::RelativeOutward {
            margin: 1e-8,
            cap: 2.0,
        },
    ] {
        assert!(well_formed(bad).is_err(), "accepted {bad:?}");
    }
    assert!(well_formed(ToleranceRule::RelativeOutward {
        margin: 1e-8,
        cap: 1.0
    })
    .is_ok());
}
