//! **`ReportedNumber` never hands the printed string to a float parser.**
//!
//! `stats_l0/number.rs` reads every reported number as the interval its
//! printed digits allow, exactly, as a `Rational`. A float parse of `".045"`
//! is already a different number from the one printed, and every rounding
//! interval built on it inherits the error. The module header says it never
//! does this; this scan is what makes that more than a sentence.
//!
//! Two halves, because the string has two places it could meet a float parser:
//!
//! 1. **`number.rs` itself**, code and tests both: no `parse::<f64>`, no
//!    `.parse()` at all, no `from_str`, no `FromStr`. A bare `.parse()` is
//!    banned and not only the turbofish, because its target type can be
//!    inferred from a `let x: f64` the scan would not connect to it.
//! 2. **The one route it does use**, `Rational::parse_decimal` in
//!    `equation/rational.rs`. Its body is scanned for any `f64` or `f32` at
//!    all. Today it parses the digits as an `i128` and builds the fraction.
//!
//! Both halves assert a POSITIVE count first: the route must be found, or the
//! scan would pass on a file it no longer reads correctly.
//!
//! # What it cannot catch
//!
//! A float parse reached through another helper `number.rs` calls. It calls
//! `parse_decimal` and nothing else that takes the string; a new helper would
//! need its own line here.

const NUMBER_RS: &str = "src/stats_l0/number.rs";
const RATIONAL_RS: &str = "src/equation/rational.rs";

/// A float parser, in every spelling Rust offers.
const FORBIDDEN_IN_NUMBER_RS: &[&str] = &[
    "parse::<f64>",
    "parse::<f32>",
    ".parse(",
    "str::parse",
    "from_str",
    "FromStr",
    "f64::from(",
    "lexical",
    "fast_float",
];

fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read(path: &str) -> String {
    code_only(
        &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e} — has it moved?")),
    )
}

/// Every line of `code` holding a forbidden parser, with its number.
fn float_parses(code: &str) -> Vec<String> {
    code.lines()
        .enumerate()
        .filter(|(_, l)| FORBIDDEN_IN_NUMBER_RS.iter().any(|f| l.contains(f)))
        .map(|(n, l)| format!("{}: {}", n + 1, l.trim()))
        .collect()
}

/// The body of `fn name` in `code`, brace-matched from its signature.
fn fn_body<'a>(code: &'a str, name: &str) -> Option<&'a str> {
    let start = code.find(&format!("fn {name}("))?;
    let open = start + code[start..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in code[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&code[open..=open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn reported_number_never_parses_f64() {
    let number = read(NUMBER_RS);
    let routes = number.matches("Rational::parse_decimal(").count();
    assert!(
        routes >= 1,
        "{NUMBER_RS} no longer calls Rational::parse_decimal — the scan no longer describes it"
    );

    let hits = float_parses(&number);
    assert!(
        hits.is_empty(),
        "{NUMBER_RS} must read printed digits exactly, never through a float parser. Found:\n{}",
        hits.join("\n")
    );

    let rational = read(RATIONAL_RS);
    let body = fn_body(&rational, "parse_decimal")
        .expect("Rational::parse_decimal is in equation/rational.rs");
    assert!(
        body.contains(".parse()"),
        "parse_decimal no longer parses digits — re-read it before trusting this guard"
    );
    for float in ["f64", "f32"] {
        assert!(
            !body.contains(float),
            "Rational::parse_decimal, the route {NUMBER_RS} reads every printed number through, names `{float}`:\n{body}"
        );
    }
}

/// The predicate must catch the edit it exists for.
#[test]
fn the_guard_fails_on_a_float_parse() {
    for offending in [
        "let v = raw.parse::<f64>().ok()?;",
        "let v: f64 = raw.parse().ok()?;",
        "let v = f64::from_str(raw)?;",
    ] {
        assert!(!float_parses(offending).is_empty(), "missed: {offending}");
    }
    assert!(float_parses("let r = Rational::parse_decimal(&s);").is_empty());
}
