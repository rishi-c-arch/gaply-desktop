//! How many variables does the binder bind, and how many does it REFUSE?
//!
//! The interesting number is the second one. A binder that binds everything
//! produces a Tier-0 finding resting on a value the engine chose, and Tier 0
//! overrides every model in the system.
use gaply_core::equation::check::check_equation;
use gaply_core::equation::graph::{graph_from_lines, BindingSource};
use gaply_core::equation::units::{check_sides, DimensionVerdict};
use gaply_core::extract::{docparse, omml};
use std::path::Path;

fn main() {
    let (mut bound, mut refused, mut findings) = (0usize, 0usize, 0usize);
    let (mut consistent, mut dim_findings, mut unverified) = (0usize, 0usize, 0usize);
    for a in std::env::args().skip(1) {
        let p = Path::new(&a);
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or(&a);
        let bytes = std::fs::read(p).expect("read");
        let is_docx = p.extension().map(|e| e.eq_ignore_ascii_case("docx")).unwrap_or(false);
        let text = if is_docx {
            docparse::parse_docx(&bytes)
        } else {
            docparse::parse_pdf_bytes(&bytes)
        }
        .expect("parse");

        // Splice the structured equations back where `docparse` left a marker.
        // Both streams are in document order.
        // Only STRUCTURAL equations left a placeholder in the prose stream —
        // the two readers share that predicate, so align on it rather than on
        // position. An inline `N` is an equation here and plain text there.
        let mut equations = if is_docx {
            omml::equations_in_docx(&bytes)
                .map(|(e, _)| e)
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.structural)
                .collect::<Vec<_>>()
                .into_iter()
        } else {
            Vec::new().into_iter()
        };
        let lines: Vec<String> = text
            .lines()
            .map(|l| {
                let t = l.trim();
                if t == docparse::EQUATION_PLACEHOLDER {
                    equations.next().map(|e| e.linear).unwrap_or_else(|| t.to_string())
                } else {
                    t.to_string()
                }
            })
            .collect();

        let g = graph_from_lines(&lines);
        println!(
            "########## {name}\n  equations={}  bound={}  refused={}",
            g.nodes.len(),
            g.bindings.len(),
            g.refused.len()
        );
        for b in &g.bindings {
            let src = match &b.source {
                BindingSource::Substitution { symbolic, numeric } => {
                    format!("unified #{symbolic} against #{numeric}")
                }
                BindingSource::Declaration { text, .. } => format!("declared: {text:?}"),
            };
            println!("  BIND    {} = {} (dp={})   {src}", b.name, b.value, b.decimals);
            bound += 1;
        }
        for r in &g.refused {
            println!("  REFUSE  {}: {}", r.name, r.reason);
            for e in &r.evidence {
                println!("            {e}");
            }
            refused += 1;
        }
        for n in &g.nodes {
            println!("  EQ      {}", n.equation.text); // EQLIST
        }
        // Dimensional consistency, over the units the manuscript declares.
        let env = g.unit_env();
        if !env.is_empty() {
            println!("  units declared: {}", env.len());
        }
        for n in &g.nodes {
            for (l, r) in n.equation.claims() {
                match check_sides(&l.expr, &r.expr, &env) {
                    DimensionVerdict::Consistent { unit } => {
                        consistent += 1;
                        println!("  DIM ok   [{unit}]  {}", n.equation.text);
                    }
                    DimensionVerdict::Inconsistent { detail, .. } => {
                        dim_findings += 1;
                        println!("  DIM BAD  {detail}");
                        println!("           {}", n.equation.text);
                    }
                    DimensionVerdict::Unverified { reason } => {
                        unverified += 1;
                        println!("  DIM ?    {reason}");
                        println!("           {}", n.equation.text);
                    }
                }
            }
        }

        let vals = g.bound_values();
        for n in &g.nodes {
            for c in check_equation(&n.equation, &vals) {
                if c.is_reportable() {
                    findings += 1;
                    println!("  >>> {} — {}", c.status.label(), c.message);
                    println!("      {}", c.source_line);
                }
            }
        }
    }
    println!(
        "\ntotal bound={bound}  refused={refused}  arithmetic findings={findings}\n\
         dimensions: consistent={consistent}  inconsistent={dim_findings}  unverified={unverified}"
    );
}
