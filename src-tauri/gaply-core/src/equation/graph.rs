//! **The `EquationGraph` — §6b.1's equations bound to the research state, and
//! left unbound where the binding is not unambiguous.**
//!
//! > *"Equations, once extracted, are canonicalised and stored as nodes with
//! > their variables bound to research-state fields **where the binding is
//! > unambiguous**."* — §6b.1
//!
//! # THE FAILURE THIS MODULE IS SHAPED AROUND (§11 D157)
//!
//! A binder that binds everything produces a confident Tier-0 finding resting
//! on a variable the ENGINE assigned, and a Tier-0 finding is the one thing in
//! the system that overrides every model. So the interesting number here is not
//! how many variables get bound; it is how many are refused.
//!
//! Measured in `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` — the document this
//! engine's negative control comes from — the single symbol `N` is declared
//! with **five different values**:
//!
//! ```text
//! N= target population = 237,000     <- §3.8.1, the one Slovin's formula uses
//! N = 600 usable / N = 600 planned   <- the achieved sample
//! N = 600 | Scale: 1=Strongly …      <- x9, table captions
//! N = 570                            <- an earlier SEM adequacy figure
//! N = 30                             <- the pilot
//! ```
//!
//! A document-wide symbol table binds `N = 600` on frequency and then reports
//! that `237,000/(1+237,000×0.04²) = 623.36` is wrong. **The arithmetic is
//! correct; the engine would have supplied the error.** That is the exact shape
//! of a Tier-0 finding that must never be produced.
//!
//! # WHERE BINDINGS COME FROM, STRONGEST FIRST
//!
//! ## 1. Unification between a formula and its own substitution — no prose
//!
//! A manuscript that writes `n = N/(1+Ne²)` and then
//! `n = 237,000/(1+237,000(0.04)²)` has stated the binding IN THE MATHEMATICS.
//! Unifying the two trees yields `N ↦ 237,000, e ↦ 0.04` and yields nothing
//! else: unification either succeeds with exactly one substitution or it fails.
//! **There is no sentence to interpret, so there is nothing to misread.**
//!
//! This is the preferred source, and for the §6b.2 *"same quantity written two
//! ways"* check it is the only one needed.
//!
//! ## 2. A prose declaration, under a stated predicate
//!
//! `N = target population = 237,000` is unambiguous. `where N is the population
//! size` is not. `N = 600 usable respondents` is a different claim wearing the
//! same shape. So a declaration binds when, and ONLY when, all five hold:
//!
//! 1. **Whole-line shape.** The line parses as an equality chain *in its
//!    entirety*, with no truncation. This is what separates
//!    `N = 600 | Scale: 1=Strongly Disagree…` — which the parser would happily
//!    read a `N = 600` prefix out of — from a line that IS a declaration.
//! 2. **Name on the left, number on the right.** The first side is a bare name
//!    and the last side is a bare numeric literal. A middle side is a gloss and
//!    is ignored (`N = target population = 237,000`). `N = normality of
//!    thiosulphate` declares a MEANING, not a value, and binds nothing.
//! 3. **In scope.** It lies in the declaration block belonging to the equation —
//!    see [`DeclarationBlock`]. `N = 600` two hundred paragraphs away is not a
//!    statement about this formula.
//! 4. **Unique within that scope.** Two values for one name binds NEITHER, and
//!    records the refusal with both.
//! 5. **A trailing parenthetical is admitted only as a CONSISTENT RESTATEMENT.**
//!    `e = margin of error = 0.04 (4%)` binds `0.04` because `4%` IS `0.04` —
//!    that is a check, not a guess. If the two disagree nothing binds, and the
//!    disagreement is itself reported.
//!
//! Every refusal is recorded in [`EquationGraph::refused`] with its reason and
//! its evidence. An absent binding that says nothing is indistinguishable from
//! one nobody looked for.

use std::collections::BTreeMap;

use super::check::BoundValues;
use super::expr::Expr;
use super::linear::{parse_equation, parse_equation_with, Equation};
use super::rational::Rational;
use super::units::{parse_unit, Unit, UnitEnv};

/// Where a binding came from. Never merged — a reader must be able to see
/// whether a value was derived from the mathematics or read from a sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingSource {
    /// Unifying a symbolic equation with its own numeric substitution.
    /// Unambiguous by construction.
    Substitution { symbolic: usize, numeric: usize },
    /// A declaration in the manuscript that met all five conditions.
    Declaration { text: String, paragraph: Option<usize> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub name: String,
    pub value: Rational,
    /// Decimal places the SOURCE literal displayed. Carried, not discarded:
    /// `e = 0.04` is exactly as ambiguous as `0.04` written inline, and a
    /// binding that forgets this narrows the rounded reading to a point and
    /// turns correct arithmetic into a `DETECTED` finding. See
    /// [`super::check::BoundValues`].
    pub decimals: u32,
    pub source: BindingSource,
}

/// A binding that was NOT made, and why. §6b.1's *"leave it absent where it is
/// not"*, made visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub name: String,
    pub reason: String,
    /// The candidate declarations, verbatim.
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquationNode {
    pub id: usize,
    pub paragraph: Option<usize>,
    pub equation: Equation,
}

impl EquationNode {
    /// Does the right-hand side carry free variables?
    pub fn is_symbolic(&self) -> bool {
        self.equation.sides.iter().skip(1).any(|s| !s.expr.variables().is_empty())
    }
    /// The name this equation defines, if it defines one.
    pub fn defines(&self) -> Option<&str> {
        match &self.equation.sides.first()?.expr {
            Expr::Var(n) => Some(n),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquationGraph {
    pub nodes: Vec<EquationNode>,
    pub bindings: Vec<Binding>,
    pub refused: Vec<Refusal>,
    /// Units read from prose definitions in declaration blocks. A LABELLED
    /// definition's own annotation overrides these — the manuscript's formal
    /// statement outranks its gloss.
    pub prose_units: UnitEnv,
}

impl EquationGraph {
    pub fn add(&mut self, equation: Equation, paragraph: Option<usize>) -> usize {
        let id = self.nodes.len();
        self.nodes.push(EquationNode { id, paragraph, equation });
        id
    }

    /// The bindings, as the checker consumes them — both readings, built from
    /// one source so they cannot drift.
    pub fn bound_values(&self) -> BoundValues {
        let mut out = BoundValues::new();
        for b in &self.bindings {
            out.insert(b.name.clone(), b.value, b.decimals);
        }
        out
    }

    pub fn binding(&self, name: &str) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.name == name)
    }

    /// What each named quantity is measured in.
    ///
    /// **A manuscript states a quantity's units on the LEFT of its own defining
    /// equation** — `Total Hardness (mg/L as CaCO₃) = (V × N × 50,000)/Vsample`
    /// — and nowhere else. So the environment is exactly the set of labelled
    /// definitions, and a unit annotation the table cannot read is dropped with
    /// its name rather than guessed at.
    pub fn unit_env(&self) -> UnitEnv {
        let mut env = self.prose_units.clone();
        for n in &self.nodes {
            let Some(side) = n.equation.sides.first() else { continue };
            let (Expr::Var(name), Some(text)) = (&side.expr, &side.unit) else { continue };
            if let Ok(u) = parse_unit(text) {
                env.insert(name.clone(), u);
            }
        }
        // **A definition gives its left side the unit its right side derives.**
        // `Magnesium Hardness = Total Hardness − Calcium Hardness` states no
        // unit on the left, and both operands are `mg/L as CaCO₃`, so the
        // quantity IS `mg/L as CaCO₃`. Without this the equation reads as
        // unverifiable when the manuscript has in fact determined it.
        //
        // Iterated to a fixed point, because one definition can supply the
        // input to another, and bounded by the node count so a circular pair
        // cannot spin.
        for _ in 0..self.nodes.len() {
            let mut learned = false;
            for n in &self.nodes {
                let Some(side) = n.equation.sides.first() else { continue };
                let Expr::Var(name) = &side.expr else { continue };
                if env.contains_key(name) {
                    continue;
                }
                let Some(rhs) = n.equation.sides.get(1) else { continue };
                if let Ok(u) = super::units::unit_of(&rhs.expr, &env) {
                    if u.dimension.is_some() {
                        env.insert(name.clone(), u);
                        learned = true;
                    }
                }
            }
            if !learned {
                break;
            }
        }
        env
    }

    /// Multi-word names this document DECLARED, by defining them. The parser
    /// joins adjacent identifiers only into one of these.
    pub fn declared_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for n in &self.nodes {
            if let Some(Expr::Var(name)) = n.equation.sides.first().map(|s| &s.expr) {
                if name.contains(' ') && !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
        out
    }

    fn record(&mut self, name: String, value: Rational, decimals: u32, source: BindingSource) {
        match self.bindings.iter().find(|b| b.name == name) {
            // Already bound to the same value from another source: no conflict.
            Some(existing) if existing.value == value => {}
            Some(existing) => {
                let ev = vec![
                    format!("{} = {} (from {:?})", name, existing.value, existing.source),
                    format!("{} = {} (from {source:?})", name, value),
                ];
                self.bindings.retain(|b| b.name != name);
                self.refused.push(Refusal {
                    name,
                    reason: "two sources give different values; neither is preferred".into(),
                    evidence: ev,
                });
            }
            None => self.bindings.push(Binding { name, value, decimals, source }),
        }
    }

    /// **Binding source 1.** Find every symbolic equation that has a numeric
    /// counterpart defining the same name, and unify them.
    pub fn bind_by_substitution(&mut self) {
        let pairs: Vec<(usize, usize)> = {
            let mut out = Vec::new();
            for a in &self.nodes {
                if !a.is_symbolic() {
                    continue;
                }
                let Some(name) = a.defines() else { continue };
                for b in &self.nodes {
                    if b.id == a.id || b.is_symbolic() || b.defines() != Some(name) {
                        continue;
                    }
                    out.push((a.id, b.id));
                }
            }
            out
        };
        for (sym, num) in pairs {
            let a = &self.nodes[sym].equation;
            let b = &self.nodes[num].equation;
            let (Some(ax), Some(bx)) = (a.sides.get(1), b.sides.get(1)) else { continue };
            let mut subst = BTreeMap::new();
            if unify(&ax.expr, &bx.expr, &mut subst).is_ok() {
                for (name, (value, decimals)) in subst {
                    self.record(
                        name,
                        value,
                        decimals,
                        BindingSource::Substitution { symbolic: sym, numeric: num },
                    );
                }
            }
        }
    }

    /// **Binding source 2.** Read a declaration block, under the five
    /// conditions in the module header.
    ///
    /// `names_in_scope` is what the equation actually uses — a symbol table of
    /// things nobody references is a liability, not an asset.
    pub fn bind_from_declarations(&mut self, block: &DeclarationBlock, names_in_scope: &[String]) {
        let mut seen: BTreeMap<String, Vec<(Rational, u32, String)>> = BTreeMap::new();
        for line in &block.lines {
            let Some((name, value, decimals)) = declaration_binding(line) else { continue };
            if !names_in_scope.iter().any(|n| *n == name) {
                continue;
            }
            seen.entry(name).or_default().push((value, decimals, line.clone()));
        }
        for (name, mut found) in seen {
            found.dedup_by(|a, b| a.0 == b.0);
            match found.len() {
                1 => {
                    let (value, decimals, text) = found.into_iter().next().expect("len checked");
                    self.record(
                        name,
                        value,
                        decimals,
                        BindingSource::Declaration { text, paragraph: block.paragraph },
                    );
                }
                _ => self.refused.push(Refusal {
                    name,
                    reason: format!(
                        "{} different values are declared in the same block",
                        found.len()
                    ),
                    evidence: found.into_iter().map(|(_, _, t)| t).collect(),
                }),
            }
        }
    }

    /// Record that a variable an equation needs has no binding at all.
    pub fn refuse_unbound(&mut self, names: &[String]) {
        for n in names {
            if self.binding(n).is_none() && !self.refused.iter().any(|r| r.name == *n) {
                // Distinct from the ambiguity refusal above: there was nothing
                // to choose between, not a choice declined. A report that
                // spelled both the same way would hide the interesting one.
                self.refused.push(Refusal {
                    name: n.clone(),
                    reason: "no declaration in scope".into(),
                    evidence: Vec::new(),
                });
            }
        }
    }
}

/// The run of manuscript lines that belongs to one equation.
///
/// **Scope is what separates `N = 237,000` from `N = 600`**, and the two are
/// otherwise identical in shape. The block is the lines immediately following
/// an equation, through an optional `where`-style introducer, while each line
/// is itself a declaration. It ends at the first line that is not — which is
/// how `N = 600` in a table caption two hundred paragraphs away never enters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationBlock {
    pub paragraph: Option<usize>,
    pub lines: Vec<String>,
}

/// Introducers a manuscript uses to open a declaration block.
fn is_introducer(line: &str) -> bool {
    let t = line.trim().trim_end_matches(':').trim().to_lowercase();
    matches!(t.as_str(), "where" | "in which" | "here" | "with")
}

impl DeclarationBlock {
    /// Collect the block that follows `after` in `lines`.
    pub fn following(lines: &[String], after: usize, paragraph: Option<usize>) -> DeclarationBlock {
        let mut out = Vec::new();
        for line in lines.iter().skip(after + 1) {
            let t = line.trim();
            if t.is_empty() || is_introducer(t) {
                continue;
            }
            if declaration_binding(t).is_none() && !is_meaning_declaration(t) {
                break;
            }
            out.push(t.to_string());
        }
        DeclarationBlock { paragraph, lines: out }
    }
}

/// `N = normality of thiosulphate` — a declaration of MEANING. It binds no
/// value, but it does not end a block either: real blocks mix the two.
fn is_meaning_declaration(line: &str) -> bool {
    let Some((head, _)) = line.split_once('=') else { return false };
    let h = head.trim();
    !h.is_empty()
        && h.chars().count() <= 24
        && h.chars().all(|c| c.is_alphanumeric() || c == '_' || c.is_whitespace() || ('\u{2080}'..='\u{2089}').contains(&c))
}

/// Condition 5: a trailing parenthetical is admitted only when it RESTATES the
/// same value. `0.04 (4%)` → `0.04`; `0.04 (5%)` → nothing, and a caller may
/// report the disagreement.
fn consistent_restatement(expr: &Expr) -> Option<(Rational, u32)> {
    if let Some(v) = expr.as_reported_value() {
        return Some(v);
    }
    // `0.04 (4%)` parses as a product, because juxtaposition of a number and a
    // bracket is multiplication everywhere else.
    if let Expr::Mul(a, b) = expr {
        if let (Some((av, ad)), Some((bv, _))) = (a.as_reported_value(), b.as_reported_value()) {
            if av == bv {
                return Some((av, ad));
            }
        }
    }
    None
}

/// **A prose definition that supplies a UNIT rather than a value.**
///
/// `chapter3 .docx` declares 11 quantities with `(mg/L)` on the left of their
/// formulas and then states every right-hand variable in prose:
///
/// ```text
/// where Vtitrant = mL of Na₂S₂O₃ used; N = normality of thiosulphate;
///       V = volume of EDTA used (mL); L = path length (cm);
///       t = duration of the reaction (min)
/// ```
///
/// Without these the dimensional check is `UNVERIFIED` on every real formula
/// in the document richest in units — measured: 11 units declared, 0 checks
/// performed.
///
/// **The predicate.** A definition supplies a unit when the prose either
/// BEGINS with a known unit symbol (`Vtitrant = mL of …`) or ENDS with a
/// parenthetical whose entire content is one (`V = volume of EDTA used (mL)`).
/// Nothing else. The symbol must be in the unit table, so
/// `N = normality of thiosulphate` and `N = normality of AgNO₃ (0.0141 N)`
/// supply NOTHING — `N` is normality here and the newton in SI, and the table
/// refuses it (see [`super::units`]).
///
/// Note it reads the UNIT, never the name: `L = path length (cm)` gives `L` the
/// unit centimetre, and the fact that `L` is also the symbol for litre is
/// irrelevant because only the right-hand side is consulted.
pub fn unit_declaration(line: &str) -> Option<(String, Unit)> {
    let (head, rest) = line.split_once('=')?;
    // A `where` block runs as one line: `where Vtitrant = mL …; and 8000 = …`.
    // The introducer and the conjunction belong to the SENTENCE, not the name.
    let mut name = head.trim();
    for lead in ["where ", "Where ", "in which ", "and ", "with "] {
        if let Some(r) = name.strip_prefix(lead) {
            name = r.trim();
        }
    }
    if name.is_empty()
        || name.chars().count() > 24
        || !name.chars().next()?.is_alphabetic()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c.is_whitespace() || ('\u{2080}'..='\u{2089}').contains(&c))
    {
        return None;
    }
    let rest = rest.trim().trim_end_matches('.').trim();
    // Form B: a trailing parenthetical that is entirely a unit.
    if rest.ends_with(')') {
        if let Some(o) = rest.rfind('(') {
            let inner = rest[o + 1..rest.len() - 1].trim();
            if let Ok(u) = parse_unit(inner) {
                if u.dimension.is_some() {
                    return Some((name.to_string(), u));
                }
            }
        }
    }
    // Form A: the prose begins with a unit.
    let first = rest.split_whitespace().next()?;
    match parse_unit(first) {
        Ok(u) if u.dimension.is_some() => Some((name.to_string(), u)),
        _ => None,
    }
}

/// Does this line declare a value, under conditions 1, 2 and 5?
///
/// Conditions 3 (scope) and 4 (uniqueness) are the caller's, because neither is
/// a property of a single line.
pub fn declaration_binding(line: &str) -> Option<(String, Rational, u32)> {
    let t = line.trim();
    // Condition 1: the WHOLE line is the declaration. `parse_equation` would
    // happily read `N = 600` out of `N = 600 | Scale: 1=Strongly Disagree…`;
    // requiring the parse to consume everything is what rejects it.
    let eq = parse_equation(t).ok()?;
    if eq.text != t || eq.sides.len() < 2 {
        return None;
    }
    // Reconstruct what was parsed and require it to be the whole line, so a
    // truncated prefix cannot pass as a declaration.
    let consumed: usize = eq.sides.iter().map(|s| s.text.chars().count()).sum::<usize>()
        + (eq.sides.len() - 1);
    let written = t.chars().filter(|c| !c.is_whitespace()).count();
    let parsed_chars: usize = eq
        .sides
        .iter()
        .map(|s| s.text.chars().filter(|c| !c.is_whitespace()).count())
        .sum::<usize>()
        + (eq.sides.len() - 1);
    let _ = consumed;
    if parsed_chars != written {
        return None;
    }
    // Condition 2: a bare name, then a value.
    let name = match &eq.sides[0].expr {
        Expr::Var(n) => n.clone(),
        _ => return None,
    };
    // **Every middle side must be a GLOSS, never arithmetic.**
    // `N = target population = 237,000` declares; the Slovin substitution
    // `n = 237,000/(1+…) = … = 623.36` also begins with a name and ends with a
    // literal, and reading it as a declaration of `n` swallows the equation
    // whole — the chain then never becomes a node and nothing checks it. The
    // difference is that its middle sides COMPUTE.
    if eq.sides[1..eq.sides.len() - 1].iter().any(|s| !matches!(s.expr, Expr::Var(_))) {
        return None;
    }
    let (value, decimals) = consistent_restatement(&eq.sides.last()?.expr)?;
    Some((name, value, decimals))
}

// ---------------------------------------------------------------------------
// Unification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifyError {
    /// The two trees have different shapes.
    Structure,
    /// One name would have to take two values.
    Conflict(String),
    /// A variable on the concrete side — it is not a substitution instance.
    NotConcrete(String),
}

/// Unify a symbolic expression against a concrete one, yielding the ONE
/// substitution that turns the first into the second, or an error.
///
/// It never searches: each position either matches or it does not, so a success
/// is unique and a partial match is not a result.
pub fn unify(
    pattern: &Expr,
    concrete: &Expr,
    subst: &mut BTreeMap<String, (Rational, u32)>,
) -> Result<(), UnifyError> {
    match (pattern, concrete) {
        // The substituted literal's DISPLAYED precision travels with its value.
        (Expr::Var(n), Expr::Num { value, decimals }) => match subst.get(n) {
            Some((existing, _)) if existing != value => Err(UnifyError::Conflict(n.clone())),
            _ => {
                subst.insert(n.clone(), (*value, *decimals));
                Ok(())
            }
        },
        (Expr::Var(a), Expr::Var(b)) if a == b => Ok(()),
        (Expr::Var(_), Expr::Var(b)) => Err(UnifyError::NotConcrete(b.clone())),
        (Expr::Num { value: a, .. }, Expr::Num { value: b, .. }) if a == b => Ok(()),
        (Expr::Neg(a), Expr::Neg(b)) | (Expr::Sqrt(a), Expr::Sqrt(b)) => unify(a, b, subst),
        (Expr::Add(a1, a2), Expr::Add(b1, b2))
        | (Expr::Sub(a1, a2), Expr::Sub(b1, b2))
        | (Expr::Mul(a1, a2), Expr::Mul(b1, b2))
        | (Expr::Div(a1, a2), Expr::Div(b1, b2))
        | (Expr::Pow(a1, a2), Expr::Pow(b1, b2)) => {
            unify(a1, b1, subst)?;
            unify(a2, b2, subst)
        }
        (Expr::Func(n1, a1), Expr::Func(n2, a2)) if n1 == n2 && a1.len() == a2.len() => {
            for (x, y) in a1.iter().zip(a2) {
                unify(x, y, subst)?;
            }
            Ok(())
        }
        _ => Err(UnifyError::Structure),
    }
}

/// Build a graph from lines of manuscript text, with paragraph indices.
///
/// Equations become nodes; the block after each equation is read for
/// declarations of the names THAT equation uses.
pub fn graph_from_lines(lines: &[String]) -> EquationGraph {
    // FIRST PASS with no vocabulary, to learn which multi-word names the
    // document declares; SECOND PASS with them, so `Total Hardness − Calcium
    // Hardness` reads as two quantities rather than failing on adjacent
    // identifiers. Only names the document introduced are ever joined.
    let first = build(lines, &[]);
    let names = first.declared_names();
    if names.is_empty() {
        return first;
    }
    build(lines, &names)
}

fn build(lines: &[String], names: &[String]) -> EquationGraph {
    let mut g = EquationGraph::default();
    let mut equation_at: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if declaration_binding(line).is_some() {
            continue;
        }
        if let Ok(eq) = parse_equation_with(line, names) {
            let needs_check = eq.sides.iter().skip(1).any(|s| !s.expr.variables().is_empty())
                || eq.sides.len() > 2;
            if needs_check {
                g.add(eq, Some(i));
                equation_at.push(i);
            }
        }
    }
    g.bind_by_substitution();
    let mut all_names: Vec<String> = Vec::new();
    for (node_index, line_index) in equation_at.iter().enumerate() {
        let names: Vec<String> = g.nodes[node_index]
            .equation
            .sides
            .iter()
            .flat_map(|s| s.expr.variables())
            .collect();
        let block = DeclarationBlock::following(lines, *line_index, Some(*line_index));
        g.bind_from_declarations(&block, &names);
        for line in &block.lines {
            // `where` blocks are semicolon-separated in this corpus.
            for clause in line.split(';') {
                if let Some((n, u)) = unit_declaration(clause) {
                    g.prose_units.entry(n).or_insert(u);
                }
            }
        }
        for n in names {
            if !all_names.contains(&n) {
                all_names.push(n);
            }
        }
    }
    // **Every name an equation uses and the record cannot value is RECORDED.**
    // An absent binding that says nothing is indistinguishable from one nobody
    // looked for, and the count of refusals is how a reader checks that this
    // binder is not binding everything it sees.
    g.refuse_unbound(&all_names);
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epistemic::EpistemicStatus;
    use crate::equation::check::check_equation;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// ¶249 / ¶250 / ¶251 of `Corrected_Chapters_3_4_Jitesh_Agarwal.docx`,
    /// exactly as the OMML reader and `docparse` deliver them.
    fn slovin_block() -> Vec<String> {
        lines(&[
            "Equation 1:Slovin’s formula:",
            "n=((N)/(1+N×(e)^(2)))",
            "Where:",
            "N= target population = 237,000",
            "e= margin of error = 0.04 (4%)",
            "n=((237,000)/(1+237,000(0.04)^(2)))=((237,000)/(1+379.2))=((237,000)/(380.2))=623.36",
        ])
    }

    // ---- binding source 1: the mathematics binds itself ------------------

    #[test]
    fn a_formula_and_its_own_substitution_bind_without_reading_any_prose() {
        let mut g = EquationGraph::default();
        g.add(parse_equation("n=((N)/(1+N×(e)^(2)))").unwrap(), Some(249));
        g.add(
            parse_equation("n=((237,000)/(1+237,000(0.04)^(2)))=((237,000)/(380.2))=623.36")
                .unwrap(),
            Some(251),
        );
        g.bind_by_substitution();

        assert_eq!(g.binding("N").unwrap().value, Rational::from_int(237_000));
        assert_eq!(
            g.binding("e").unwrap().value,
            Rational::parse_decimal("0.04").unwrap().0
        );
        assert!(matches!(
            g.binding("N").unwrap().source,
            BindingSource::Substitution { symbolic: 0, numeric: 1 }
        ));
        assert!(g.refused.is_empty(), "{:?}", g.refused);
    }

    /// Unification does not search: a shape that does not match is an error,
    /// not a partial binding.
    #[test]
    fn a_substitution_that_does_not_match_binds_nothing() {
        let mut g = EquationGraph::default();
        g.add(parse_equation("n=((N)/(1+N×(e)^(2)))").unwrap(), None);
        // Same name, different formula — a denominator without the square.
        g.add(parse_equation("n=((237,000)/(1+237,000))").unwrap(), None);
        g.bind_by_substitution();
        assert!(g.bindings.is_empty(), "{:?}", g.bindings);
    }

    #[test]
    fn unification_refuses_to_bind_one_name_to_two_values() {
        let pattern = parse_equation("y = N + N").unwrap().sides[1].expr.clone();
        let concrete = parse_equation("y = 3 + 4").unwrap().sides[1].expr.clone();
        let mut s = BTreeMap::new();
        assert_eq!(unify(&pattern, &concrete, &mut s), Err(UnifyError::Conflict("N".into())));
    }

    // ---- binding source 2: the declaration predicate ---------------------

    #[test]
    fn the_slovin_declaration_binds_and_the_gloss_is_ignored() {
        assert_eq!(
            declaration_binding("N= target population = 237,000"),
            Some(("N".into(), Rational::from_int(237_000), 0))
        );
    }

    /// Condition 5: `0.04 (4%)` binds because `4%` IS `0.04`. That is a check,
    /// not a guess — and a restatement that disagrees binds nothing.
    #[test]
    fn a_trailing_parenthetical_binds_only_when_it_restates_the_same_value() {
        assert_eq!(
            declaration_binding("e= margin of error = 0.04 (4%)"),
            Some(("e".into(), Rational::parse_decimal("0.04").unwrap().0, 2))
        );
        assert_eq!(declaration_binding("e = margin of error = 0.04 (5%)"), None);
    }

    /// **Condition 1, and the one that matters most.** The parser will read
    /// `N = 600` out of a table caption by longest-prefix; requiring the whole
    /// line to be the declaration is what stops it.
    #[test]
    fn a_table_caption_is_not_a_declaration_even_though_it_starts_like_one() {
        for caption in [
            "N = 600 | Scale: 1=Strongly Disagree to 5=Strongly Agree",
            "N = 600 usable",
            "N = 600 planned",
            "N = 600 from an estimated population of 2,37,000+ units (0.25% sampling rate)",
            "Data basis: pilot N = 30; main analytical N = 600.",
        ] {
            assert_eq!(declaration_binding(caption), None, "bound from {caption:?}");
        }
        // A bare one still binds — the rule is about completeness, not length.
        assert_eq!(
            declaration_binding("N = 570"),
            Some(("N".into(), Rational::from_int(570), 0))
        );
    }

    /// Condition 2: a declaration of MEANING is not a declaration of value.
    #[test]
    fn a_prose_definition_declares_a_meaning_and_binds_nothing() {
        assert_eq!(declaration_binding("N = normality of thiosulphate"), None);
        assert_eq!(declaration_binding("Vtitrant = mL of Na₂S₂O₃ used"), None);
    }

    /// **Condition 4, on the real conflict.** Two values for one name in one
    /// block binds NEITHER, and says so with both.
    #[test]
    fn two_values_for_one_name_in_one_block_bind_neither() {
        let mut g = EquationGraph::default();
        let block = DeclarationBlock {
            paragraph: Some(1),
            lines: lines(&["N = 237,000", "N = 600"]),
        };
        g.bind_from_declarations(&block, &["N".to_string()]);
        assert!(g.binding("N").is_none());
        assert_eq!(g.refused.len(), 1);
        assert_eq!(g.refused[0].name, "N");
        assert_eq!(g.refused[0].evidence.len(), 2, "both candidates are kept");
    }

    /// **Condition 3.** The block ends at the first line that is not a
    /// declaration, which is how a caption far below never enters.
    #[test]
    fn the_declaration_block_ends_at_the_first_line_that_is_not_one() {
        let src = lines(&[
            "n=((N)/(1+N×(e)^(2)))",
            "Where:",
            "N= target population = 237,000",
            "e= margin of error = 0.04 (4%)",
            "The calculation yields approximately n = 624 as a planning benchmark.",
            "N = 600 usable",
        ]);
        let block = DeclarationBlock::following(&src, 0, Some(0));
        assert_eq!(block.lines.len(), 2, "{:?}", block.lines);
        assert!(block.lines.iter().all(|l| l.starts_with('N') || l.starts_with('e')));
    }

    /// A name nothing references is not put in the table.
    #[test]
    fn a_declaration_for_a_name_the_equation_does_not_use_is_ignored() {
        let mut g = EquationGraph::default();
        let block = DeclarationBlock { paragraph: None, lines: lines(&["k = 7"]) };
        g.bind_from_declarations(&block, &["N".to_string()]);
        assert!(g.bindings.is_empty());
    }

    // ---- the two sources together ---------------------------------------

    #[test]
    fn the_two_sources_agreeing_is_not_a_conflict() {
        let g = graph_from_lines(&slovin_block());
        assert_eq!(g.binding("N").unwrap().value, Rational::from_int(237_000));
        assert_eq!(
            g.binding("e").unwrap().value,
            Rational::parse_decimal("0.04").unwrap().0
        );
        // Unification is the stronger source and wins the slot; the prose
        // declaration agrees, so `record` accepts it silently rather than
        // treating agreement as a conflict.
        assert!(matches!(
            g.binding("N").unwrap().source,
            BindingSource::Substitution { .. }
        ));
        // `n` is the quantity being DEFINED. Nothing values it, and that is
        // recorded rather than passed over in silence.
        assert_eq!(g.refused.len(), 1, "{:?}", g.refused);
        assert_eq!(g.refused[0].name, "n");
        assert_eq!(g.refused[0].reason, "no declaration in scope");
    }

    /// The two kinds of absence must not read the same. One is "nothing to
    /// choose between"; the other is "a choice I declined to make", and only
    /// the second is evidence that the manuscript is inconsistent.
    #[test]
    fn the_two_reasons_for_an_absent_binding_are_worded_differently() {
        let mut g = EquationGraph::default();
        g.bind_from_declarations(
            &DeclarationBlock { paragraph: None, lines: lines(&["N = 237,000", "N = 600"]) },
            &["N".to_string()],
        );
        g.refuse_unbound(&["Vsample".to_string()]);
        let ambiguous = g.refused.iter().find(|r| r.name == "N").unwrap();
        let absent = g.refused.iter().find(|r| r.name == "Vsample").unwrap();
        assert_ne!(ambiguous.reason, absent.reason);
        assert!(!ambiguous.evidence.is_empty(), "an ambiguity keeps its candidates");
        assert!(absent.evidence.is_empty());
    }

    /// If the prose and the mathematics disagree, NEITHER wins — the engine
    /// does not pick a side it has no grounds to pick.
    #[test]
    fn prose_that_contradicts_the_mathematics_unbinds_rather_than_choosing() {
        let mut src = slovin_block();
        src[3] = "N= target population = 250,000".into();
        let g = graph_from_lines(&src);
        assert!(g.binding("N").is_none(), "{:?}", g.bindings);
        let r = g.refused.iter().find(|r| r.name == "N").expect("a refusal for N");
        assert!(r.reason.contains("different values"), "{}", r.reason);
        assert_eq!(r.evidence.len(), 2);
    }

    // ---- what the graph is FOR: §6b.2's "same quantity, two ways" --------

    /// With `N` and `e` bound, the symbolic formula recomputes to the number
    /// the numeric form reports. This is the check the graph exists to enable.
    #[test]
    fn the_symbolic_form_recomputes_to_the_reported_value_once_bound() {
        let g = graph_from_lines(&slovin_block());
        let b = g.bound_values();
        let symbolic = &g.nodes[0].equation;
        let value = symbolic.sides[1].expr.eval(b.exact()).expect("bound");
        assert_eq!(value.to_decimal_string(2), "623.36");

        // And every claim in the numeric chain still holds.
        for node in &g.nodes {
            for c in check_equation(&node.equation, &b) {
                assert!(
                    !c.is_reportable(),
                    "the negative control must stay silent: {} — {}",
                    c.source_line,
                    c.message
                );
            }
        }
    }

    /// The negative control's counterpart: change the reported result and the
    /// bound graph catches it.
    #[test]
    fn a_wrong_reported_value_is_caught_once_the_graph_supplies_the_inputs() {
        let mut src = slovin_block();
        src[5] = src[5].replace("623.36", "723.36");
        let g = graph_from_lines(&src);
        let b = g.bound_values();
        let found: Vec<_> = g
            .nodes
            .iter()
            .flat_map(|n| check_equation(&n.equation, &b))
            .filter(|c| c.is_reportable())
            .collect();
        assert!(!found.is_empty(), "a wrong value must be caught");
        assert_eq!(found[0].status, EpistemicStatus::Detected);
    }

    /// A definition whose right side has a derivable unit gives its left side
    /// that unit — the graph doing what §6b.1 describes.
    #[test]
    fn a_definition_propagates_its_unit_to_the_quantity_it_defines() {
        let src = lines(&[
            "Total Hardness (mg/L as CaCO₃) = (V × N × 50,000) / Vsample",
            "Calcium Hardness (mg/L as CaCO₃) = (V × N × 50,000) / Vsample",
            "Magnesium Hardness = Total Hardness − Calcium Hardness",
        ]);
        let g = graph_from_lines(&src);
        let env = g.unit_env();
        let m = env.get("Magnesium Hardness").expect("propagated from the subtraction");
        assert_eq!(m.basis.as_deref(), Some("CaCO₃"));
        assert_eq!(m.dimension.unwrap().render(), "M·L⁻³");
    }

    /// A prose `where` clause supplies units for the variables a formula uses.
    #[test]
    fn a_where_clause_supplies_units_under_the_stated_predicate() {
        // Form A: the prose begins with the unit.
        let (n, u) = unit_declaration("Vtitrant = mL of Na₂S₂O₃ used").expect("form A");
        assert_eq!(n, "Vtitrant");
        assert_eq!(u.dimension.unwrap().render(), "L³");
        // Form B: a trailing parenthetical that is entirely a unit.
        let (n, u) = unit_declaration("V = volume of EDTA used (mL)").expect("form B");
        assert_eq!(n, "V");
        assert_eq!(u.dimension.unwrap().render(), "L³");
        // The introducer belongs to the sentence, not the name.
        assert_eq!(unit_declaration("where Vtitrant = mL of X").unwrap().0, "Vtitrant");
        // It reads the UNIT, never the name: `L` here is a path length in cm.
        let (n, u) = unit_declaration("L = path length (cm)").expect("a name that looks like a unit");
        assert_eq!(n, "L");
        assert_eq!(u.dimension.unwrap().render(), "L");
    }

    /// **`N` supplies nothing, and that is deliberate.** It is normality here
    /// and the newton in SI. Admitting it would make ten of `chapter3 .docx`'s
    /// fourteen formulas *appear* checkable while their dimensioned constants
    /// (8000, 50,000, 35.45) stayed unitless — and a partial unit system
    /// manufactures mismatches on correct formulas.
    #[test]
    fn a_prose_definition_with_no_machine_readable_unit_supplies_nothing() {
        assert_eq!(unit_declaration("N = normality of thiosulphate"), None);
        assert_eq!(unit_declaration("N = normality of AgNO₃ (0.0141 N)"), None);
        assert_eq!(unit_declaration("ε = molar extinction coefficient"), None);
        assert_eq!(unit_declaration("8000 = milliequivalent weight of O₂ × 1000"), None);
        // And a value declaration is not a unit declaration.
        assert_eq!(unit_declaration("N = 237,000"), None);
    }

    /// Unbound names are RECORDED as unbound, not silently dropped.
    #[test]
    fn a_variable_with_no_declaration_is_recorded_as_refused() {
        let mut g = EquationGraph::default();
        g.refuse_unbound(&["Vtitrant".to_string(), "Vsample".to_string()]);
        assert_eq!(g.refused.len(), 2);
        assert!(g.refused.iter().all(|r| r.reason == "no declaration in scope"));
    }
}
