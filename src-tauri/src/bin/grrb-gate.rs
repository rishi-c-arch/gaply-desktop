//! **Promote / hold / rollback, §6c.3.** Takes two GRRB reports and classifies
//! the diff.
//!
//! ```text
//! better on the families it touches, no regression elsewhere -> PROMOTE
//! mixed                                                      -> HOLD, regression named
//! worse                                                      -> ROLLBACK
//! ```
//!
//! **Scoring at baseline is HOLD, not PROMOTE.** §6c.3's last line: *"A prompt
//! variant that scores at baseline does not ship — the citation_support
//! v1.7-v1.10 precedent, now a gate instead of a lesson."* A change that moves
//! nothing has not earned a release.
//!
//! **A changed case set BLOCKS rather than compares.** Two reports over
//! different cases have different denominators, and comparing them is the defect
//! CLAUDE.md's denominator entry records. The benchmark grows by design, so this
//! is the common case, not an edge one: re-run the OLD commit against the NEW
//! case set before reading a diff.
//!
//! There was no existing promote/hold/rollback mechanism to generalise — §6c.3
//! records that v4's claim about an `ai-eval --json headSha` discipline was
//! wrong and that no such flag exists. This is built from nothing.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Report {
    head_sha: String,
    families: Vec<Family>,
    determinism_all_stable: bool,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Family {
    family: String,
    tp: u32,
    fp: u32,
    #[serde(rename = "fn_")]
    fn_: u32,
    tn: u32,
    unrunnable: u32,
    accuracy_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    outcome: String,
}

fn load(p: &str) -> Report {
    let s = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (old_p, new_p) = match (a.next(), a.next()) {
        (Some(o), Some(n)) => (o, n),
        _ => {
            eprintln!("usage: grrb-gate <baseline.json> <candidate.json>");
            std::process::exit(2);
        }
    };
    let (old, new) = (load(&old_p), load(&new_p));

    let ids = |r: &Report| r.cases.iter().map(|c| c.id.clone()).collect::<BTreeSet<_>>();
    let (oi, ni) = (ids(&old), ids(&new));
    if oi != ni {
        let added: Vec<_> = ni.difference(&oi).cloned().collect();
        let removed: Vec<_> = oi.difference(&ni).cloned().collect();
        println!("BLOCKED — the case set changed, so the two reports have different denominators.");
        if !added.is_empty() {
            println!("  added:   {}", added.join(", "));
        }
        if !removed.is_empty() {
            println!("  removed: {}", removed.join(", "));
        }
        println!("  Re-run the baseline commit against the CURRENT case set, then compare.");
        std::process::exit(3);
    }

    fn fam(r: &Report) -> BTreeMap<String, &Family> {
        r.families.iter().map(|f| (f.family.clone(), f)).collect()
    }
    let (of, nf) = (fam(&old), fam(&new));

    let mut better = Vec::new();
    let mut worse = Vec::new();
    println!("GRRB gate  {}  ->  {}", &old.head_sha[..7.min(old.head_sha.len())], &new.head_sha[..7.min(new.head_sha.len())]);
    println!("\n  {:<24} {:>12} {:>12}  {}", "family", "was", "now", "delta");
    for (name, o) in &of {
        let n = match nf.get(name) {
            Some(n) => n,
            None => continue,
        };
        // Correct answers, not a percentage: a family whose unrunnable count
        // changes moves its own denominator, and the percentage would hide that.
        let (oc, nc) = (o.tp + o.tn, n.tp + n.tn);
        let (od, nd) = (o.tp + o.tn + o.fp + o.fn_, n.tp + n.tn + n.fp + n.fn_);
        let d = nc as i64 - oc as i64;
        let mark = if d > 0 {
            better.push(name.clone());
            "better"
        } else if d < 0 {
            worse.push(format!("{name} ({oc}/{od} -> {nc}/{nd})"));
            "WORSE"
        } else if od != nd {
            "same score, denominator moved"
        } else {
            "unchanged"
        };
        println!("  {name:<24} {:>8}/{:<3} {:>8}/{:<3}  {mark}", oc, od, nc, nd);
    }

    // Determinism is not a family score and cannot be traded against one.
    if old.determinism_all_stable && !new.determinism_all_stable {
        println!("\nROLLBACK — determinism regressed: a case now answers differently on a rerun.");
        println!("  §6c.2 lists determinism as a column, and for Tier 0 it is the property §4.4");
        println!("  grants authority on. No accuracy gain trades against it.");
        std::process::exit(1);
    }

    let verdict = match (better.is_empty(), worse.is_empty()) {
        (false, true) => "PROMOTE",
        (false, false) => "HOLD",
        (true, false) => "ROLLBACK",
        (true, true) => "HOLD",
    };
    println!("\n{verdict}");
    match verdict {
        "PROMOTE" => println!("  better on {} and no regression elsewhere.", better.join(", ")),
        "ROLLBACK" => println!("  worse on {} and better nowhere.", worse.join(", ")),
        _ if !worse.is_empty() => {
            println!("  mixed. REGRESSIONS: {}", worse.join(", "));
            println!("  better on: {}", better.join(", "));
        }
        _ => println!(
            "  scores at baseline. §6c.3: a variant that scores at baseline does not ship."
        ),
    }
    // **Exit codes separate "did not improve" from "got worse", because a
    // pre-commit hook must block one and not the other.**
    //
    // §6c.3's "a variant that scores at baseline does not ship" is a rule about
    // a PROMPT VARIANT seeking promotion. Applied to every commit it would
    // reject an ordinary refactor that moves no score, which is most commits
    // touching this code. The verdict word stays faithful to §6c.3; the exit
    // code carries the distinction the hook needs.
    //
    //   0  PROMOTE              better somewhere, worse nowhere
    //   4  HOLD  (regression)   mixed — something got worse
    //   5  HOLD  (at baseline)  nothing moved; no regression to block on
    //   1  ROLLBACK             worse, better nowhere
    //   2  usage   3  BLOCKED (the case set changed)
    std::process::exit(match (verdict, worse.is_empty()) {
        ("PROMOTE", _) => 0,
        ("ROLLBACK", _) => 1,
        ("HOLD", false) => 4,
        _ => 5,
    });
}
