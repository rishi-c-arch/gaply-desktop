//! Phase 2, Step 8, section 4: Level 1 on a real paper and its open data,
//! through the same command functions the screen calls, with AI help off.
//! Not committed; a measurement harness.
//!
//! ```text
//! l1_paper_run <manuscript> <data>                 # claims and schema
//! l1_paper_run <manuscript> <data> <bindings.json> # bind by hand, compute
//! ```
//!
//! bindings.json: `[{"claim": 3, "method": "independent_t",
//!   "roles": {"outcome": ["Age"], "group": ["Sex"]},
//!   "levels": {"group": ["F", "M"]}, "filters": {"Sex": ["F", "M"]},
//!   "variant": "student", "ols_overall_f": null, "ols_predictor": null}]`
//! Columns are named by header as printed in the schema listing.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use app_lib::stats_ai::{MemoryCredentialStore, StatsAi, TransportFactory};
use app_lib::stats_l1_commands::{self as l1, BindingEdits, FilterEdit, L1Runs, LevelEdit, RoleEdit};
use gaply_core::stats_assist::transport::{HelperTransport, MockTransport};
use gaply_core::Database;

#[derive(serde::Deserialize)]
struct Bind {
    claim: usize,
    method: String,
    roles: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    levels: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    filters: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    ols_overall_f: Option<bool>,
    #[serde(default)]
    ols_predictor: Option<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() >= 3, "usage: l1_paper_run <manuscript> <data> [bindings.json]");
    // AI help off (the default setting), no key, and a transport that has
    // nothing to send: Level 1 must not need it.
    let mock = Arc::new(MockTransport::new());
    let m = mock.clone();
    let factory: TransportFactory = Arc::new(move |_| m.clone() as Arc<dyn HelperTransport>);
    let ai = StatsAi::for_tests(Arc::new(MemoryCredentialStore::default()), factory, "http://127.0.0.1:9/v1");
    assert!(!ai.settings().ai_help, "AI help is off");
    let db = Database::in_memory().unwrap();
    let runs = L1Runs::default();
    let v = match l1::open(&db, &runs, &ai, Path::new(&args[1]), Path::new(&args[2])) {
        Ok(v) => v,
        Err(e) => {
            println!("OPEN REFUSED {e:?}");
            std::process::exit(2);
        }
    };
    let alias_of = |header: &str| -> u32 {
        let c = v.schema.columns.iter().find(|c| c.header == header).unwrap_or_else(|| panic!("no column {header:?}"));
        c.alias.trim_start_matches('C').parse().unwrap()
    };

    if args.len() == 3 {
        println!("claims read: {}   not located: {}", v.claims.len(), v.not_located);
        for c in &v.claims {
            let l0 = c.level0.as_ref().map(|w| serde_json::to_string(w).unwrap()).unwrap_or_default();
            println!("CLAIM {} [{} s{} p{}] {} {} df={:?} p={} n={:?} L0={}", c.claim_id, c.section, c.section_index, c.paragraph, c.family, c.statistic, c.df, c.p, c.n, l0);
            println!("      {}", c.sentence);
        }
        println!("rows {}  dataset issues {}", v.schema.rows, serde_json::to_string(&v.schema.issues).unwrap());
        for c in &v.schema.columns {
            let lv: Vec<String> = c.levels.iter().take(12).map(|l| format!("{}({})", l.code, l.count)).collect();
            println!("COL {} {:?} {} missing={} levels={:?} issues={}", c.alias, c.header, c.column_type, c.has_missing, lv, serde_json::to_string(&c.issues).unwrap());
        }
        return;
    }

    let binds: Vec<Bind> = serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
    let mut tokens = Vec::new();
    for b in &binds {
        let edits = BindingEdits {
            method_id: b.method.clone(),
            roles: b.roles.iter().map(|(r, cols)| RoleEdit { role: r.clone(), aliases: cols.iter().map(|h| alias_of(h)).collect() }).collect(),
            levels: b.levels.iter().map(|(r, l)| LevelEdit { role: r.clone(), levels: l.clone() }).collect(),
            filters: b.filters.iter().map(|(h, l)| FilterEdit { alias: alias_of(h), levels: l.clone() }).collect(),
            variant_ticked: b.variant.clone(),
            ols_overall_f: b.ols_overall_f,
            ols_predictor: b.ols_predictor.as_deref().map(alias_of),
        };
        let draft = match l1::draft_proposal(&runs, v.run, b.claim, None, &edits) {
            Ok(d) => d,
            Err(e) => {
                println!("BIND claim {} DRAFT REFUSED {e:?}", b.claim);
                continue;
            }
        };
        match l1::confirm_binding(&runs, v.run, draft.proposal, None) {
            Ok(c) => {
                println!("BIND claim {} confirmed: rows_read={} filtered={} missing={} n_used={}", b.claim, c.rows_read, c.rows_filtered_out, c.rows_excluded_missing, c.n_used);
                tokens.push(c.token);
            }
            Err(e) => println!("BIND claim {} CONFIRM REFUSED {e:?}", b.claim),
        }
    }
    let out = l1::compute(&runs, v.run, &tokens).unwrap();
    println!("COUNTS {}", serde_json::to_string(&out.counts).unwrap());
    for c in &out.claims {
        if !binds.iter().any(|b| b.claim == c.claim_id) {
            continue;
        }
        println!("RESULT claim {} method={:?} verdict={}", c.claim_id, c.method, serde_json::to_string(&c.verdict).unwrap());
        for cand in &c.candidates {
            let nums: Vec<String> = cand.numbers.iter().map(|n| format!("{}:{}{} vs {:.6} -> {}", n.kind.code, n.relation, n.printed, n.computed, n.status.code)).collect();
            println!("      {} | {}", cand.variant.code, nums.join(" ; "));
        }
        if !c.notes.is_empty() {
            println!("      notes {}", serde_json::to_string(&c.notes).unwrap());
        }
    }
    assert_eq!(mock.sent_count(), 0, "no request left");
    println!("requests sent: {}", mock.sent_count());
}
