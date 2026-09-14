//! **The linear-text equation reader — the gating item for §6b.**
//!
//! §6b.1 named an OMML reader as the gate. Measured across the six manuscripts
//! (`examples/equation_survey.rs`, `examples/textmath_scan.rs`): **21
//! equation-shaped lines survive `docparse` intact, and zero lines of OMML
//! exist in any of them.** Researchers in this corpus type their mathematics,
//! units included, and it arrives at the parser whole:
//!
//! ```text
//! DO (mg/L) = (Vtitrant × N × 8000) / Vsample
//! Chlorophyll a (mg/L) = (12.7 × A₆₆₃) − (2.69 × A₆₄₅)
//! Weighted provision = (0.108 × 0.78) + (0.500 × 0.13) + … = 0.084 + … = 21.9%
//! ```
//!
//! So this is the reader that unlocks the subsystem; `omml.rs` targets the same
//! [`Expr`] and is the smaller, later piece (3 of 17 documents).
//!
//! # What it refuses to guess
//!
//! `β1X1` is one identifier here, not `β1 × X1`. Juxtaposed letters are
//! genuinely ambiguous — only the author knows whether `Vtitrant` is one
//! variable or six — and inventing a factorisation would be the §6b.3 failure
//! this whole engine is written against. A run of letters is one name; the
//! equation still parses, still holds its structure, and still participates in
//! symbolic equivalence. Where it matters (numeric substitution) the variable
//! is simply unbound, and unbound is `UNVERIFIED`, which is an honest answer.
//!
//! Juxtaposition IS multiplication where it cannot mean anything else: a number
//! against a bracket (`237,000(0.04)²`) or against a name (`2x`), because no
//! identifier begins with a digit.
//!
//! **NO MODEL IS ON THIS PATH.** Text in, tree out, deterministically.

use super::expr::Expr;
use super::rational::Rational;

/// One side of an equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Side {
    pub expr: Expr,
    /// The source text of this side, verbatim. A finding quotes the manuscript
    /// back to its author; a re-rendering of the parse would quote the parser.
    pub text: String,
    /// A parenthesised unit annotation stripped from a label — `mg/L` in
    /// `DO (mg/L)`. Input to the dimensional check.
    pub unit: Option<String>,
}

/// An equation, possibly a CHAIN: `a = b = c = d` is four sides and three
/// claims, and a manuscript that rounds midway through one is the case this
/// exists to catch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equation {
    pub sides: Vec<Side>,
    /// The whole line as it appeared.
    pub text: String,
}

impl Equation {
    /// The `n − 1` adjacent pairs a chain asserts.
    pub fn claims(&self) -> impl Iterator<Item = (&Side, &Side)> {
        self.sides.windows(2).map(|w| (&w[0], &w[1]))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No `=`, so there is no claim to check. Not an error in the data — most
    /// lines of a manuscript are not equations.
    NotAnEquation,
    UnexpectedToken(String),
    UnexpectedEnd,
    UnbalancedBracket,
    /// A side that is empty (`x = = y`).
    EmptySide,
    TooLong,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NotAnEquation => write!(f, "no equals sign, so no claim to check"),
            ParseError::UnexpectedToken(t) => write!(f, "unexpected `{t}`"),
            ParseError::UnexpectedEnd => write!(f, "the expression ends early"),
            ParseError::UnbalancedBracket => write!(f, "unbalanced bracket"),
            ParseError::EmptySide => write!(f, "an empty side of the equation"),
            ParseError::TooLong => write!(f, "the line is too long to be an equation"),
        }
    }
}

/// A line longer than this is prose that happens to contain `=`, not an
/// equation. Bounds the parser against a pathological paragraph.
const MAX_LINE: usize = 600;

// ---------------------------------------------------------------------------
// Tokenising
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Num(Rational, u32),
    Ident(String),
    Plus,
    Minus,
    Times,
    Divide,
    Caret,
    /// A run of Unicode superscript digits, already an integer exponent.
    Super(i32),
    Percent,
    Open(char),
    Close(char),
    Eq,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

/// Subscript digits belong to the NAME (`A₆₆₃`, the absorbance at 663 nm);
/// superscript digits are an EXPONENT. Conflating them would turn a wavelength
/// into a power, or a power into part of a name.
///
/// **`char::is_alphanumeric` is true for `²`.** Superscript two is Unicode
/// category `No`, so the obvious spelling of this predicate swallows the
/// exponent into the identifier and `N × e²` silently becomes a variable called
/// `e²` — a power that vanishes with no error anywhere. Found by a
/// canonicalisation test that had been passing for the wrong reason: it
/// asserted two expressions were DIFFERENT, and they were, because both had
/// lost the same exponent. The exclusion below is the fix and
/// `a_superscript_is_an_exponent_not_part_of_the_name` is the pin.
fn is_ident_continue(c: char) -> bool {
    if superscript_digit(c).is_some() {
        return false;
    }
    c.is_alphanumeric() || c == '_' || ('\u{2080}'..='\u{2089}').contains(&c) || c == '\u{2032}'
}

fn superscript_digit(c: char) -> Option<i32> {
    match c {
        '\u{2070}' => Some(0),
        '\u{00B9}' => Some(1),
        '\u{00B2}' => Some(2),
        '\u{00B3}' => Some(3),
        '\u{2074}'..='\u{2079}' => Some(c as i32 - 0x2074 + 4),
        _ => None,
    }
}

/// Tokens, plus whether each was preceded by whitespace.
///
/// The gap matters. `2A` is a product; `600 usable` is a number followed by a
/// word — see [`P::term`].
fn tokenize(s: &str) -> Result<(Vec<Tok>, Vec<bool>), ParseError> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut spaced = Vec::new();
    let mut gap = false;
    let mut i = 0usize;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            gap = true;
            i += 1;
            continue;
        }
        let this_gap = std::mem::replace(&mut gap, false);
        while spaced.len() < out.len() {
            spaced.push(false);
        }
        spaced.push(this_gap);
        // A number: digits, with thousands separators and a decimal point. A
        // comma is only a separator when digits follow it.
        if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let start = i;
            while i < cs.len() {
                let d = cs[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == ',' && cs.get(i + 1).is_some_and(char::is_ascii_digit) {
                    i += 1;
                } else if d == '.'
                    && cs.get(i + 1).is_some_and(char::is_ascii_digit)
                    && !cs[start..i].contains(&'.')
                {
                    i += 1;
                } else {
                    break;
                }
            }
            let lit: String = cs[start..i].iter().collect();
            let (v, d) = Rational::parse_decimal(&lit).ok_or(ParseError::UnexpectedToken(lit))?;
            out.push(Tok::Num(v, d));
            continue;
        }
        if let Some(d) = superscript_digit(c) {
            let mut e = 0i32;
            let mut any = false;
            let neg = false;
            while i < cs.len() {
                match superscript_digit(cs[i]) {
                    Some(d2) => {
                        e = e.checked_mul(10).and_then(|x| x.checked_add(d2)).unwrap_or(e);
                        any = true;
                        i += 1;
                    }
                    None => break,
                }
            }
            let _ = d;
            if !any {
                return Err(ParseError::UnexpectedToken(c.to_string()));
            }
            out.push(Tok::Super(if neg { -e } else { e }));
            continue;
        }
        if is_ident_start(c) {
            let start = i;
            while i < cs.len() {
                if is_ident_continue(cs[i]) {
                    i += 1;
                    continue;
                }
                // **An ASCII hyphen tightly between two letters is a HYPHEN.**
                // `post-stratification` is one word; mathematics writes
                // subtraction with spaces or with U+2212. Without this rule the
                // longest-prefix search below turns the phrase into
                // `post − stratification` and manufactures an equation out of a
                // sentence. The Unicode dashes are ALWAYS subtraction, so a
                // manuscript that means minus and types it properly is unharmed.
                if cs[i] == '-'
                    && i > start
                    && cs[i - 1].is_alphabetic()
                    && cs.get(i + 1).is_some_and(|n| n.is_alphabetic())
                {
                    i += 1;
                    continue;
                }
                break;
            }
            out.push(Tok::Ident(cs[start..i].iter().collect()));
            continue;
        }
        // **An en/em dash tightly between two digits is a RANGE.**
        // `OR = 3.90 (2.24–6.81)` is an odds ratio with a confidence interval;
        // read as subtraction it becomes `3.90 × (2.24 − 6.81)`, which is three
        // fabrications in one line — a multiplication, a subtraction and a
        // number. Measured: this shape produced 3 of the 4 false equations the
        // prose control found. A manuscript that means minus writes U+2212 or
        // spaces it, and both still work.
        if matches!(c, '\u{2013}' | '\u{2014}' | '\u{2012}')
            && i > 0
            && cs[i - 1].is_ascii_digit()
            && cs.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            return Err(ParseError::UnexpectedToken(c.to_string()));
        }
        i += 1;
        match c {
            '+' => out.push(Tok::Plus),
            // ASCII hyphen, Unicode minus, en dash, em dash, figure dash. A
            // manuscript writes all five and means subtraction.
            '-' | '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{2012}' => out.push(Tok::Minus),
            // (the digit-range case is handled before this match; see below)
            '*' | '\u{00D7}' | '\u{22C5}' | '\u{2219}' | '\u{00B7}' => out.push(Tok::Times),
            '/' | '\u{00F7}' => out.push(Tok::Divide),
            '^' => out.push(Tok::Caret),
            '%' => out.push(Tok::Percent),
            '(' | '[' | '{' => out.push(Tok::Open(c)),
            ')' | ']' | '}' => out.push(Tok::Close(c)),
            '=' => out.push(Tok::Eq),
            // Anything else ends the equation rather than being guessed at.
            other => return Err(ParseError::UnexpectedToken(other.to_string())),
        }
    }
    while spaced.len() < out.len() {
        spaced.push(false);
    }
    spaced.truncate(out.len());
    Ok((out, spaced))
}

/// Byte offsets just past each token, for the longest-prefix search in
/// [`parse_equation`]. Stops at the first character it cannot tokenise rather
/// than failing — a prose tail is a place to CUT, not an error.
fn token_end_offsets(s: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut consumed_chars = 0usize;
    // Re-walk with the same rules as `tokenize`, recording positions. Driving
    // this from `tokenize` itself would need it to be fallible-with-a-prefix,
    // which complicates the one function that must stay easy to audit.
    let cs: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0usize;
    while i < cs.len() {
        let (_, c) = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(|(_, d)| d.is_ascii_digit()))
        {
            while i < cs.len() {
                let (_, d) = cs[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == ','
                    && cs.get(i + 1).is_some_and(|(_, e)| e.is_ascii_digit())
                {
                    i += 1;
                } else if d == '.'
                    && cs.get(i + 1).is_some_and(|(_, e)| e.is_ascii_digit())
                    && !cs[start..i].iter().any(|(_, e)| *e == '.')
                {
                    i += 1;
                } else {
                    break;
                }
            }
        } else if superscript_digit(c).is_some() {
            while i < cs.len() && superscript_digit(cs[i].1).is_some() {
                i += 1;
            }
        } else if is_ident_start(c) {
            while i < cs.len() {
                if is_ident_continue(cs[i].1) {
                    i += 1;
                } else if cs[i].1 == '-'
                    && i > start
                    && cs[i - 1].1.is_alphabetic()
                    && cs.get(i + 1).is_some_and(|(_, n)| n.is_alphabetic())
                {
                    i += 1;
                } else {
                    break;
                }
            }
        } else if matches!(c, '\u{2013}' | '\u{2014}' | '\u{2012}')
            && i > 0
            && cs[i - 1].1.is_ascii_digit()
            && cs.get(i + 1).is_some_and(|(_, n)| n.is_ascii_digit())
        {
            // A numeric range ends the tokenisable prefix — see `tokenize`.
            break;
        } else if matches!(
            c,
            '+' | '-'
                | '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{2012}'
                | '*' | '\u{00D7}' | '\u{22C5}' | '\u{2219}' | '\u{00B7}'
                | '/' | '\u{00F7}' | '^' | '%' | '(' | '[' | '{' | ')' | ']' | '}' | '='
        ) {
            i += 1;
        } else {
            // Unknown character: the tokenisable prefix ends here.
            break;
        }
        consumed_chars += 1;
        let end = cs.get(i).map(|(b, _)| *b).unwrap_or(s.len());
        ends.push(end);
    }
    let _ = consumed_chars;
    ends
}

fn closes(open: char, close: char) -> bool {
    matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}'))
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

struct P<'a> {
    t: &'a [Tok],
    /// Whether token `i` was preceded by whitespace in the source.
    spaced: &'a [bool],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn next(&mut self) -> Option<&Tok> {
        let t = self.t.get(self.i);
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.term()?;
        loop {
            match self.peek() {
                Some(Tok::Plus) => {
                    self.i += 1;
                    lhs = Expr::Add(Box::new(lhs), Box::new(self.term()?));
                }
                Some(Tok::Minus) => {
                    self.i += 1;
                    lhs = Expr::Sub(Box::new(lhs), Box::new(self.term()?));
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Times) => {
                    self.i += 1;
                    lhs = Expr::Mul(Box::new(lhs), Box::new(self.unary()?));
                }
                Some(Tok::Divide) => {
                    self.i += 1;
                    lhs = Expr::Div(Box::new(lhs), Box::new(self.unary()?));
                }
                // **Implicit multiplication, ONLY where juxtaposition cannot
                // mean anything else.** Measured against prose: allowing it
                // after a bare identifier turns `volume (mL) of acid` into
                // `volume × mL × of`, three variables the author never wrote.
                // The three admitted shapes are unambiguous because no
                // identifier begins with a digit:
                //   `237,000(0.04)` — number, open bracket
                //   `2A`            — number, name
                //   `(a)(b)`        — close bracket, open bracket
                // A name before a bracket — `f(x)`, `volume (mL)` — is
                // genuinely ambiguous between a call and a product, so it is
                // REFUSED rather than guessed (§6b.3).
                Some(Tok::Open(_))
                    if matches!(
                        self.t.get(self.i.wrapping_sub(1)),
                        Some(Tok::Num(..)) | Some(Tok::Close(_))
                    ) =>
                {
                    lhs = Expr::Mul(Box::new(lhs), Box::new(self.unary()?));
                }
                // **A SPACE BREAKS IT.** `2A` is a product; `600 usable` is a
                // number followed by a word. Measured: without this,
                // `N = 600 usable` — a table caption in
                // `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` — parses as
                // `N = 600 × usable`, and the equivalence check then reports a
                // DETECTED Tier-0 finding against a caption. Three such
                // findings appeared in one document. Mathematical writing
                // spells a product `2x` or `2·x`, never `2 x`.
                Some(Tok::Ident(_))
                    if matches!(self.t.get(self.i.wrapping_sub(1)), Some(Tok::Num(..)))
                        && !self.spaced.get(self.i).copied().unwrap_or(true) =>
                {
                    lhs = Expr::Mul(Box::new(lhs), Box::new(self.unary()?));
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.i += 1;
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Some(Tok::Plus) => {
                self.i += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let base = self.postfix()?;
        match self.peek() {
            Some(Tok::Caret) => {
                self.i += 1;
                // Right-associative: a^b^c is a^(b^c).
                let e = self.unary()?;
                Ok(Expr::Pow(Box::new(base), Box::new(e)))
            }
            Some(Tok::Super(e)) => {
                let e = *e;
                self.i += 1;
                Ok(Expr::Pow(Box::new(base), Box::new(Expr::int(e as i128))))
            }
            _ => Ok(base),
        }
    }

    /// `%` is a postfix operator meaning "divide by one hundred". `21.9%` is
    /// exactly `0.219`, and it claims THREE decimals of precision as a value
    /// even though it displays one — the precision rule reads the value, not
    /// the glyphs.
    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.primary()?;
        while let Some(Tok::Percent) = self.peek() {
            self.i += 1;
            e = match e {
                Expr::Num { value, decimals } => {
                    let hundred = Rational::from_int(100);
                    match value.div(hundred) {
                        Some(v) => Expr::num(v, decimals.saturating_add(2)),
                        None => return Err(ParseError::UnexpectedToken("%".into())),
                    }
                }
                other => Expr::Div(Box::new(other), Box::new(Expr::int(100))),
            };
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        match self.next().cloned() {
            Some(Tok::Num(v, d)) => Ok(Expr::num(v, d)),
            Some(Tok::Ident(name)) => {
                // A name immediately followed by `(` is a function call only
                // when it is a function this engine knows. Otherwise the shape
                // is ambiguous — a call, or a product — and `term` refuses it
                // rather than choosing. See the implicit-multiplication note.
                if matches!(self.peek(), Some(Tok::Open(_))) && is_known_function(&name) {
                    let open = match self.next() {
                        Some(Tok::Open(c)) => *c,
                        _ => return Err(ParseError::UnexpectedEnd),
                    };
                    let arg = self.expr()?;
                    match self.next() {
                        Some(Tok::Close(c)) if closes(open, *c) => {}
                        _ => return Err(ParseError::UnbalancedBracket),
                    }
                    if name.eq_ignore_ascii_case("sqrt") {
                        return Ok(Expr::Sqrt(Box::new(arg)));
                    }
                    return Ok(Expr::Func(name.to_lowercase(), vec![arg]));
                }
                Ok(Expr::Var(name))
            }
            Some(Tok::Open(open)) => {
                let inner = self.expr()?;
                match self.next() {
                    Some(Tok::Close(c)) if closes(open, *c) => Ok(inner),
                    Some(_) => Err(ParseError::UnbalancedBracket),
                    None => Err(ParseError::UnbalancedBracket),
                }
            }
            Some(Tok::Minus) => Ok(Expr::Neg(Box::new(self.unary()?))),
            Some(t) => Err(ParseError::UnexpectedToken(format!("{t:?}"))),
            None => Err(ParseError::UnexpectedEnd),
        }
    }
}

fn is_known_function(n: &str) -> bool {
    matches!(
        n.to_lowercase().as_str(),
        "sqrt" | "log" | "ln" | "exp" | "sin" | "cos" | "tan" | "abs" | "max" | "min"
    )
}

// ---------------------------------------------------------------------------
// Label-with-unit
// ---------------------------------------------------------------------------

/// Split `DO (mg/L)` into the name `DO` and the unit `mg/L`.
///
/// **A stated heuristic, with its failure mode stated too.** `x (a/b)` is
/// genuinely ambiguous — multiplication or an annotation — and no parser
/// resolves it without knowing what `a` and `b` are. The rule taken: a side is
/// a labelled quantity when it ENDS in a bracketed group, everything before
/// that group is names and spaces (no digits, no operators), and the group
/// carries a unit marker (`/` or the word `as`). Every real case in the corpus
/// matches — `DO (mg/L)`, `Total Hardness (mg/L as CaCO₃)`,
/// `Total Alkalinity (as CaCO₃)` — and `(A − B) × N` cannot, because it has an
/// operator inside and a factor after.
///
/// Getting this wrong in the other direction is what matters: reading
/// `DO (mg/L)` as multiplication invents two variables, `mg` and `L`, that the
/// author never declared.
fn split_label_unit(s: &str) -> Option<(String, String)> {
    let t = s.trim();
    if !t.ends_with(')') {
        return None;
    }
    let open = t.rfind('(')?;
    let head = t[..open].trim();
    let unit = t[open + 1..t.len() - 1].trim();
    if head.is_empty() || unit.is_empty() {
        return None;
    }
    if !head.chars().all(|c| c.is_alphabetic() || c.is_whitespace() || c == '_') {
        return None;
    }
    let unit_marker = unit.contains('/')
        || unit.split_whitespace().any(|w| w.eq_ignore_ascii_case("as"))
        || unit.contains('%');
    if !unit_marker {
        return None;
    }
    if unit.contains(['+', '=', '\u{00D7}', '\u{2212}']) {
        return None;
    }
    // The label is one name; spaces inside it are part of it.
    Some((head.split_whitespace().collect::<Vec<_>>().join(" "), unit.to_string()))
}

/// The most words a side may be and still be read as ONE name.
///
/// `Weighted provision`, `Total Hardness`, `Chlorophyll a` are labels a
/// manuscript puts on the left of a formula. A six-word run of prose is a
/// sentence. The cap is a bound on how wrong this can go, not the argument for
/// why it is right — that is [`parse_equation`]'s rule that an equation must
/// contain arithmetic on at least one side.
const MAX_LABEL_WORDS: usize = 5;

/// A side that is nothing but names and spaces cannot be arithmetic — there is
/// no operator in it — so it is a label.
fn as_label_phrase(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_alphabetic() || c.is_whitespace() || c == '_') {
        return None;
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    if words.is_empty() || words.len() > MAX_LABEL_WORDS {
        return None;
    }
    Some(words.join(" "))
}

fn parse_side(text: &str) -> Result<Side, ParseError> {
    parse_side_with(text, &[])
}

fn parse_side_with(text: &str, names: &[String]) -> Result<Side, ParseError> {
    let raw = text.trim();
    if raw.is_empty() {
        return Err(ParseError::EmptySide);
    }
    if let Some((label, unit)) = split_label_unit(raw) {
        return Ok(Side { expr: Expr::Var(label), text: raw.to_string(), unit: Some(unit) });
    }
    if let Some(label) = as_label_phrase(raw) {
        return Ok(Side { expr: Expr::Var(label), text: raw.to_string(), unit: None });
    }
    let (toks, spaced) = tokenize(raw)?;
    if toks.is_empty() {
        return Err(ParseError::EmptySide);
    }
    let (toks, spaced) = join_known_names(toks, spaced, names);
    let mut p = P { t: &toks, spaced: &spaced, i: 0 };
    let e = p.expr()?;
    if p.i != toks.len() {
        return Err(ParseError::UnexpectedToken(format!("{:?}", toks[p.i])));
    }
    Ok(Side { expr: e, text: raw.to_string(), unit: None })
}

/// **Join adjacent identifiers into a name the DOCUMENT declared.**
///
/// `Magnesium Hardness = Total Hardness − Calcium Hardness` is a real line from
/// `chapter3 .docx`, and `Total Hardness` is one quantity. The parser refuses
/// adjacent identifiers by default because joining them freely turns
/// `The bootstrapped indirect effect was a × b = 0.413` into an equation whose
/// left side is a six-word variable — prose wearing the shape of mathematics.
///
/// The safe form is a VOCABULARY: a run of identifiers joins only when the
/// result is a name the document itself introduced, by defining it
/// (`Total Hardness (mg/L as CaCO₃) = …`). Nothing is invented; the document's
/// own declarations decide what is a name. Longest match wins, so
/// `Total Hardness` beats `Total`.
fn join_known_names(toks: Vec<Tok>, spaced: Vec<bool>, names: &[String]) -> (Vec<Tok>, Vec<bool>) {
    if names.is_empty() {
        return (toks, spaced);
    }
    let max_words = names
        .iter()
        .map(|n| n.split_whitespace().count())
        .max()
        .unwrap_or(1)
        .min(8);
    let mut out = Vec::with_capacity(toks.len());
    let mut out_spaced = Vec::with_capacity(toks.len());
    let mut i = 0usize;
    while i < toks.len() {
        let mut matched = None;
        if matches!(toks[i], Tok::Ident(_)) {
            for run in (2..=max_words).rev() {
                if i + run > toks.len() {
                    continue;
                }
                let mut parts = Vec::with_capacity(run);
                let mut ok = true;
                for t in &toks[i..i + run] {
                    match t {
                        Tok::Ident(w) => parts.push(w.clone()),
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let joined = parts.join(" ");
                if names.iter().any(|n| *n == joined) {
                    matched = Some((joined, run));
                    break;
                }
            }
        }
        match matched {
            Some((joined, run)) => {
                out.push(Tok::Ident(joined));
                out_spaced.push(spaced.get(i).copied().unwrap_or(false));
                i += run;
            }
            None => {
                out.push(toks[i].clone());
                out_spaced.push(spaced.get(i).copied().unwrap_or(false));
                i += 1;
            }
        }
    }
    (out, out_spaced)
}

/// Does this text parse, complete, as an expression?
///
/// The OMML reader asks this before bracketing a sub-part. Word's tree does
/// not always align with mathematical grouping — a text run can span a bracket
/// — so a fragment that is not an expression must not be wrapped as one.
pub fn parses_as_expression(s: &str) -> bool {
    let Ok((toks, spaced)) = tokenize(s.trim()) else { return false };
    if toks.is_empty() {
        return false;
    }
    let mut p = P { t: &toks, spaced: &spaced, i: 0 };
    matches!(p.expr(), Ok(_) if p.i == toks.len())
}

/// Parse one line of manuscript text as an equation (or a chain of them).
///
/// Deterministic and total: every failure is a typed [`ParseError`], never a
/// panic and never a partial tree presented as a whole one.
///
/// # The longest valid PREFIX, and why there is no prose lexicon
///
/// Measured over the six manuscripts, the commonest shape is an equation with
/// prose welded to its end:
///
/// ```text
/// DO (mg/L) = (Vtitrant × N × 8000) / Vsample where Vtitrant = mL of Na₂S₂O₃ used; …
/// Weighted provision = (0.108 × 0.78) + … = 21.9% (95% CI: 17.0–26.8%).
/// ```
///
/// The obvious fix is a list of words that end an equation — `where`, `;`,
/// `(95% CI`. That list is never finished, and every entry is a guess about
/// how researchers write. Instead this takes **the longest prefix of the line
/// that is a complete, valid equation**, which needs no vocabulary at all: the
/// prose tail is simply not parseable, so it is not in the prefix.
///
/// It is bounded by [`MAX_LINE`] and it never lengthens a parse — a line that
/// parses whole is tried first and wins.
pub fn parse_equation(line: &str) -> Result<Equation, ParseError> {
    parse_equation_with(line, &[])
}

/// Parse with a vocabulary of multi-word names the document declared.
/// See [`join_known_names`] for why a vocabulary rather than free joining.
pub fn parse_equation_with(line: &str, names: &[String]) -> Result<Equation, ParseError> {
    let line = line.trim();
    if line.chars().count() > MAX_LINE {
        return Err(ParseError::TooLong);
    }
    if !line.contains('=') {
        return Err(ParseError::NotAnEquation);
    }
    let full = parse_exact_with(line, names);
    if full.is_ok() {
        return full;
    }
    let mut ends = token_end_offsets(line);
    ends.pop(); // the whole line, already tried
    while let Some(end) = ends.pop() {
        let candidate = line[..end].trim_end();
        if !candidate.contains('=') {
            break;
        }
        if truncation_would_drop_mathematics(&line[end..]) {
            continue;
        }
        if let Ok(eq) = parse_exact_with(candidate, names) {
            return Ok(eq);
        }
    }
    // Report the failure of the WHOLE line: it is the one a reader is looking
    // at, and a message about some truncation of it would mislead.
    full
}

/// **Truncation may drop PROSE. It may never drop MATHEMATICS.**
///
/// The longest-prefix rule is only safe while what falls off the end is not
/// part of the claim. Without this check, `y = 3 × f(x)` truncates to
/// `y = 3 × f` — a valid equation the author did not write, with a factor
/// silently removed. That is the same failure as the OMML flattening: a
/// well-formed result that says something the document does not.
///
/// A remainder blocks the cut when either holds:
///
/// * it begins with a binary operator, so the cut is inside an expression
///   (`× f(x)` after `y = 3`); or
/// * it parses as a complete expression on its own, so real mathematics is
///   being discarded (`(x)` after `y = 3 × f`).
///
/// Prose tails satisfy neither. `where Vtitrant = mL of Na₂S₂O₃ used; …` is two
/// adjacent names and does not parse; `(95% CI: 17.0–26.8%).` is an unbalanced
/// bracket once the untokenisable `:` ends the run.
fn truncation_would_drop_mathematics(remainder: &str) -> bool {
    let r = remainder.trim();
    if r.is_empty() {
        return false;
    }
    let (toks, spaced) = match tokenize(r) {
        Ok(t) => t,
        Err(_) => {
            // Untokenisable: take the part that IS tokenisable and judge that.
            let ends = token_end_offsets(r);
            match ends.last() {
                Some(e) => match tokenize(r[..*e].trim()) {
                    Ok(t) => t,
                    Err(_) => return false,
                },
                None => return false,
            }
        }
    };
    if toks.is_empty() {
        return false;
    }
    if matches!(
        toks[0],
        Tok::Plus | Tok::Minus | Tok::Times | Tok::Divide | Tok::Caret | Tok::Super(_) | Tok::Percent
    ) {
        return true;
    }
    let mut p = P { t: &toks, spaced: &spaced, i: 0 };
    matches!(p.expr(), Ok(_) if p.i == toks.len())
}

/// Parse a string that must be an equation in its entirety.
fn parse_exact_with(line: &str, names: &[String]) -> Result<Equation, ParseError> {
    let line = line.trim();
    if !line.contains('=') {
        return Err(ParseError::NotAnEquation);
    }
    // Split on `=` at bracket depth zero, so a `=` inside a group cannot split
    // the chain.
    let mut parts: Vec<String> = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in line.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                cur.push(c);
            }
            '=' if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    if depth != 0 {
        return Err(ParseError::UnbalancedBracket);
    }
    if parts.len() < 2 {
        return Err(ParseError::NotAnEquation);
    }
    let mut sides = Vec::with_capacity(parts.len());
    for p in &parts {
        sides.push(parse_side_with(p, names)?);
    }
    // **An equation with no arithmetic on any side is not an equation.**
    // `Weighted provision = (0.108 × 0.78) + …` is a claim this engine can
    // check; `Table A = summarizes how reliable each dimension is` is a
    // sentence with an equals sign in it, and reading it as a claim about two
    // named quantities would manufacture an equation the author never wrote.
    // This is what makes the label rule above safe rather than merely bounded.
    if sides.iter().all(|s| matches!(s.expr, Expr::Var(_))) {
        return Err(ParseError::NotAnEquation);
    }
    Ok(Equation { sides, text: line.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation::expr::Bindings;

    fn parse(s: &str) -> Equation {
        parse_equation(s).unwrap_or_else(|e| panic!("{s:?}: {e}"))
    }

    // ---- the real corpus -------------------------------------------------

    /// `chapter3 .docx`, verbatim. The parse must keep the division that the
    /// whole formula turns on.
    #[test]
    fn the_dissolved_oxygen_formula_parses_with_its_unit_and_its_division() {
        let eq = parse("DO (mg/L) = (Vtitrant × N × 8000) / Vsample");
        assert_eq!(eq.sides.len(), 2);
        assert_eq!(eq.sides[0].expr, Expr::Var("DO".into()));
        assert_eq!(eq.sides[0].unit.as_deref(), Some("mg/L"));
        // `mg` and `L` must NOT have become variables.
        assert_eq!(
            eq.sides[1].expr.variables(),
            vec!["N".to_string(), "Vsample".into(), "Vtitrant".into()]
        );
        let mut b = Bindings::new();
        b.insert("Vtitrant".into(), Rational::parse_decimal("5.0").unwrap().0);
        b.insert("N".into(), Rational::parse_decimal("0.025").unwrap().0);
        b.insert("Vsample".into(), Rational::from_int(200));
        // (5 × 0.025 × 8000) / 200 = 5
        assert_eq!(eq.sides[1].expr.eval(&b).unwrap(), Rational::from_int(5));
    }

    /// `chapter3 .docx`, verbatim. Subscripted absorbances are NAMES, and the
    /// square brackets are grouping.
    #[test]
    fn subscripts_stay_in_the_name_and_brackets_group() {
        let eq = parse("Chlorophyll a (mg/L) = (12.7 × A₆₆₃) − (2.69 × A₆₄₅)");
        assert_eq!(eq.sides[0].unit.as_deref(), Some("mg/L"));
        assert_eq!(eq.sides[1].expr.variables(), vec!["A₆₄₅".to_string(), "A₆₆₃".into()]);
        let eq2 = parse("COD (mg/L) = [(A − B) × N × 8000] / V");
        assert_eq!(eq2.sides[1].expr.variables(), vec!["A".to_string(), "B".into(), "N".into(), "V".into()]);
    }

    /// `Revised Health Economics Paper FINAL (1).docx`, verbatim. A three-link
    /// chain, and `%` is exact.
    #[test]
    fn the_weighted_provision_chain_parses_as_three_claims() {
        let eq = parse(
            "Weighted provision = (0.108 × 0.78) + (0.500 × 0.13) + (0.769 × 0.06) \
             + (0.810 × 0.03) = 0.084 + 0.065 + 0.046 + 0.024 = 21.9%",
        );
        assert_eq!(eq.sides.len(), 4);
        assert_eq!(eq.claims().count(), 3);
        let b = Bindings::new();
        assert_eq!(
            eq.sides[1].expr.eval(&b).unwrap(),
            Rational::parse_decimal("0.21968").unwrap().0
        );
        assert_eq!(
            eq.sides[2].expr.eval(&b).unwrap(),
            Rational::parse_decimal("0.219").unwrap().0
        );
        // 21.9% is exactly 0.219 — and claims three decimals as a VALUE.
        assert_eq!(eq.sides[3].expr.eval(&b).unwrap(), Rational::parse_decimal("0.219").unwrap().0);
        assert_eq!(eq.sides[3].expr.as_reported_value().unwrap().1, 3);
    }

    /// `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` §3.8.1, as the OMML reader
    /// will hand it over: implicit multiplication against a bracket, a
    /// superscript power, Indian thousands grouping.
    #[test]
    fn slovins_numeric_substitution_parses_as_a_chain_and_is_exact() {
        let eq = parse("n = 237,000/(1+237,000(0.04)²) = 237,000/(1+379.2) = 237,000/380.2 = 623.36");
        assert_eq!(eq.sides.len(), 5);
        let b = Bindings::new();
        let v1 = eq.sides[1].expr.eval(&b).unwrap();
        let v2 = eq.sides[2].expr.eval(&b).unwrap();
        let v3 = eq.sides[3].expr.eval(&b).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v2, v3);
        assert_eq!(v3.to_decimal_string(2), "623.36");
        // The reported value claims two decimals and the computation meets it.
        let (reported, dp) = eq.sides[4].expr.as_reported_value().unwrap();
        assert_eq!(dp, 2);
        assert_eq!(v3.round_to(dp).unwrap(), reported);
    }

    /// `Disha Correction .docx`, verbatim.
    #[test]
    fn a_regression_specification_parses_without_inventing_a_factorisation() {
        let eq = parse("Y = β0 + β1X1 + β2X2 + β3X3 + ε");
        assert_eq!(eq.sides.len(), 2);
        // `β1X1` is ONE name. Splitting it would be a guess about the author's
        // notation — see the module header.
        let vars = eq.sides[1].expr.variables();
        assert!(vars.contains(&"β1X1".to_string()), "{vars:?}");
        assert!(!vars.contains(&"β1".to_string()), "{vars:?}");
        assert!(vars.contains(&"ε".to_string()), "{vars:?}");
    }

    // ---- structure -------------------------------------------------------

    #[test]
    fn precedence_and_association_follow_arithmetic_not_reading_order() {
        let b = Bindings::new();
        assert_eq!(
            parse("x = 2 + 3 × 4").sides[1].expr.eval(&b).unwrap(),
            Rational::from_int(14)
        );
        assert_eq!(
            parse("x = 100 / 10 / 2").sides[1].expr.eval(&b).unwrap(),
            Rational::from_int(5)
        );
        assert_eq!(
            parse("x = 10 − 3 − 2").sides[1].expr.eval(&b).unwrap(),
            Rational::from_int(5)
        );
        assert_eq!(parse("x = 2^3^2").sides[1].expr.eval(&b).unwrap(), Rational::from_int(512));
    }

    #[test]
    fn every_dash_a_manuscript_writes_is_a_minus() {
        let b = Bindings::new();
        for dash in ["-", "\u{2212}", "\u{2013}", "\u{2014}"] {
            let e = parse(&format!("x = 7 {dash} 2"));
            assert_eq!(e.sides[1].expr.eval(&b).unwrap(), Rational::from_int(5), "{dash:?}");
        }
    }

    /// `char::is_alphanumeric('²')` is TRUE — superscript two is Unicode
    /// category `No`. Without an explicit exclusion the exponent is absorbed
    /// into the identifier and the power disappears silently, which is the
    /// same failure family as the OMML flattening this module was built after.
    #[test]
    fn a_superscript_is_an_exponent_not_part_of_the_name() {
        assert!('\u{00B2}'.is_alphanumeric(), "the Unicode fact this pin rests on");
        let e = parse("y = N × e²");
        assert_eq!(e.sides[1].expr.variables(), vec!["N".to_string(), "e".into()]);
        let mut b = Bindings::new();
        b.insert("N".into(), Rational::from_int(3));
        b.insert("e".into(), Rational::from_int(4));
        assert_eq!(e.sides[1].expr.eval(&b).unwrap(), Rational::from_int(48));
        // Subscripts still belong to the name.
        assert_eq!(
            parse("y = 12.7 × A₆₆₃").sides[1].expr.variables(),
            vec!["A₆₆₃".to_string()]
        );
    }

    #[test]
    fn a_sign_error_in_a_denominator_is_a_different_expression() {
        // §6b.2: "a sign or denominator that differs is flagged".
        let a = parse("n = N/(1+N×e²)").sides[1].expr.canonical();
        let c = parse("n = N/(1−N×e²)").sides[1].expr.canonical();
        assert_ne!(a, c);
        let d = parse("n = N/(1+N)×e²").sides[1].expr.canonical();
        assert_ne!(a, d);
    }

    #[test]
    fn the_same_quantity_written_two_ways_canonicalises_the_same() {
        // What canonical form DOES guarantee: order, association, collected
        // like terms, folded constants, division as a negative power.
        let a = parse("x = N × e × e / (b × 2)").sides[1].expr.canonical();
        let b = parse("x = e² × N / (2 × b)").sides[1].expr.canonical();
        assert_eq!(a, b);
    }

    /// **The stated limit of canonical form, pinned so it cannot be mistaken
    /// for a bug later.** `(a+b)/c` and `a/c + b/c` are equal and canonicalise
    /// differently, because normalisation does not distribute — doing so
    /// terminates on these shapes but not in general. Equal canonical forms
    /// prove equivalence; UNEQUAL ones prove nothing, which is exactly why
    /// symbolic equivalence needs a second stage rather than stopping here.
    #[test]
    fn unequal_canonical_forms_do_not_mean_unequal_expressions() {
        let a = parse("x = (a + b) / c").sides[1].expr.canonical();
        let b = parse("x = a/c + b/c").sides[1].expr.canonical();
        assert_ne!(a, b, "if this ever passes, the equivalence stage needs revisiting");

        // And they are in fact equal, at every point either is defined.
        let mut bind = Bindings::new();
        for (av, bv, cv) in [(3i128, 5i128, 2i128), (-7, 11, 4)] {
            bind.insert("a".into(), Rational::from_int(av));
            bind.insert("b".into(), Rational::from_int(bv));
            bind.insert("c".into(), Rational::from_int(cv));
            let l = parse("x = (a + b) / c").sides[1].expr.eval(&bind).unwrap();
            let r = parse("x = a/c + b/c").sides[1].expr.eval(&bind).unwrap();
            assert_eq!(l, r);
        }
    }

    // ---- refusals --------------------------------------------------------

    #[test]
    fn prose_without_an_equals_sign_is_not_an_equation() {
        assert_eq!(parse_equation("The sample was large."), Err(ParseError::NotAnEquation));
    }

    #[test]
    fn an_unparseable_side_is_an_error_not_a_partial_tree() {
        // Prose that happens to contain `=`. A parser that returned the part it
        // understood would be reporting an equation the author never wrote.
        assert!(parse_equation("Table A = summarizes how reliable each dimension is").is_err());
        assert!(parse_equation("x = (1 + 2").is_err());
        assert!(parse_equation("x = = 2").is_err());
    }

    /// The guard that makes the multi-word label rule safe.
    #[test]
    fn an_equals_sign_between_two_names_is_prose_not_an_equation() {
        assert_eq!(parse_equation("Table A = the summary"), Err(ParseError::NotAnEquation));
        // Refused too, by tokenising rather than by this rule — either way no
        // equation is manufactured out of a sentence.
        assert!(parse_equation("Weighted provision = post-stratification estimate").is_err());
        // But a name against arithmetic IS a claim.
        assert!(parse_equation("Weighted provision = 0.084 + 0.065").is_ok());
    }

    #[test]
    fn a_paragraph_is_refused_on_length_before_it_is_parsed() {
        let long = format!("x = {}", "1 + ".repeat(300));
        assert_eq!(parse_equation(&long), Err(ParseError::TooLong));
    }

    /// `f(x)` is a call or a product and nothing in the text says which, so it
    /// is refused. A known function is not ambiguous and is parsed.
    #[test]
    fn an_ambiguous_name_before_a_bracket_is_refused_not_guessed() {
        assert!(parse_equation("y = 3 × f(x)").is_err());
        let s = parse("y = sqrt(9)");
        assert_eq!(s.sides[1].expr.eval(&Bindings::new()).unwrap(), Rational::from_int(3));
    }

    /// The four prose lines the corpus control caught being turned into
    /// equations. Each is a manuscript sentence, and each must stay prose.
    #[test]
    fn prose_measured_in_the_corpus_does_not_become_an_equation() {
        // `volume × mL × of` — three variables invented from a definition list.
        let e = parse_equation(
            "where A = volume (mL) of acid used to the phenolphthalein endpoint",
        );
        assert!(e.is_err() || e.unwrap().sides[1].expr.variables().len() <= 1);

        // An odds ratio with a confidence interval. The interval must not
        // become `2.24 − 6.81`, and the juxtaposition must not become a product.
        let eq = parse_equation("OR = 3.90 (2.24–6.81)").expect("reads the reported value");
        assert_eq!(eq.sides.len(), 2);
        assert_eq!(
            eq.sides[1].expr.eval(&Bindings::new()).unwrap(),
            Rational::parse_decimal("3.90").unwrap().0
        );
        assert_eq!(eq.sides[1].text, "3.90", "the interval is not part of the claim");
    }

    /// A spaced or Unicode minus still subtracts — the range rule must not cost
    /// a manuscript that writes subtraction properly.
    #[test]
    fn the_range_rule_does_not_eat_a_real_subtraction() {
        let b = Bindings::new();
        assert_eq!(parse("x = 6.81 − 2.24").sides[1].expr.eval(&b).unwrap().to_decimal_string(2), "4.57");
        assert_eq!(parse("x = 6.81 – 2.24").sides[1].expr.eval(&b).unwrap().to_decimal_string(2), "4.57");
        assert_eq!(parse("x = 6.81-2.24").sides[1].expr.eval(&b).unwrap().to_decimal_string(2), "4.57");
    }

    #[test]
    fn a_unit_annotation_is_only_taken_where_it_cannot_be_arithmetic() {
        assert_eq!(split_label_unit("DO (mg/L)"), Some(("DO".into(), "mg/L".into())));
        assert_eq!(
            split_label_unit("Total Hardness (mg/L as CaCO₃)"),
            Some(("Total Hardness".into(), "mg/L as CaCO₃".into()))
        );
        assert_eq!(
            split_label_unit("Total Alkalinity (as CaCO₃)"),
            Some(("Total Alkalinity".into(), "as CaCO₃".into()))
        );
        // Arithmetic, not an annotation.
        assert_eq!(split_label_unit("(A − B) × N"), None);
        assert_eq!(split_label_unit("2 (x/y)"), None);
        assert_eq!(split_label_unit("x (a + b)"), None);
    }
}
