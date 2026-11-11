//! Shared by scale scores' command tests (Research OS, Phase 8M-3, decisions
//! 424 to 427): an app over Tauri's mock runtime with every command the ten
//! methods and the prepared slot use, so a test calls them through IPC with
//! the webview's argument names and reads the JSON the webview reads; a table
//! and recipes that fit all ten; and a run that waits for a test. Folders,
//! tables and the IPC call itself are `desc_support`'s, unchanged.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{WebviewWindow, WebviewWindowBuilder};

use app_lib::runs::{Clock, Point, Runs, RUN_CEILING_MS};
use app_lib::state::AppState;
use app_lib::stats_alpha_commands as alpha;
use app_lib::stats_anova_commands as anova;
use app_lib::stats_corr_commands as corr;
use app_lib::stats_desc_commands as desc;
use app_lib::stats_linear_regression_commands as linear_regression;
use app_lib::stats_onesample_commands as onesample;
use app_lib::stats_prep_commands as prep;
use app_lib::stats_ranks_commands as ranks;
use app_lib::stats_related_ranks_commands as related_ranks;
use app_lib::stats_ttest_commands as ttest;
use app_lib::workspace::{self as ws, RunsFactory, WorkspaceSession};
use gaply_core::{AppConfig, Database};

use crate::desc_support::{call, TempDir};

static WINDOWS: AtomicUsize = AtomicUsize::new(0);

pub type W = WebviewWindow<MockRuntime>;

/// An app with Statistics active, its folders, and its data file.
pub struct World {
    pub _root: TempDir,
    pub _app: tauri::App<MockRuntime>,
    pub w: W,
    pub data: PathBuf,
    pub app_data: PathBuf,
}

pub fn world(tag: &str, table: &[u8], runs: Option<Arc<Runs>>) -> World {
    let root = TempDir::new(tag);
    let data = root.0.join("data.csv");
    std::fs::write(&data, table).unwrap();
    let app_data = root.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::open(&app_data.join("gaply.db")).expect("a migrated database"));
    let factory: RunsFactory =
        Arc::new(move || runs.clone().unwrap_or_else(|| Arc::new(Runs::default())));
    let (app, w) = app_with(&app_data, db, factory);
    call(&w, "workspace_activate", json!({ "id": "statistics" })).expect("activates");
    World {
        _root: root,
        _app: app,
        w,
        data,
        app_data,
    }
}

pub fn app_with(
    app_data: &Path,
    db: Arc<Database>,
    runs: RunsFactory,
) -> (tauri::App<MockRuntime>, W) {
    let mut state = AppState::new(
        AppConfig::new(app_data.join("gaply.db").to_string_lossy().as_ref()),
        db.clone(),
        db,
        Arc::new(gaply_core::embed::HashEmbedder),
        app_data.to_path_buf(),
    );
    state.workspace = Arc::new(WorkspaceSession::with_hooks(Arc::new(ws::readiness), runs));
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            desc::stats_desc_open,
            desc::stats_desc_validate,
            desc::stats_desc_run,
            desc::stats_desc_status,
            desc::stats_desc_result,
            desc::stats_desc_export,
            desc::stats_desc_close,
            desc::stats_desc_forget_run,
            desc::stats_desc_cells,
            ttest::stats_ttest_validate,
            ttest::stats_ttest_run,
            ttest::stats_ttest_status,
            ttest::stats_ttest_result,
            ttest::stats_ttest_export,
            ttest::stats_ttest_group_levels,
            onesample::stats_onesample_validate,
            onesample::stats_onesample_run,
            onesample::stats_onesample_status,
            onesample::stats_onesample_result,
            onesample::stats_onesample_export,
            onesample::stats_paired_validate,
            onesample::stats_paired_run,
            onesample::stats_paired_status,
            onesample::stats_paired_result,
            onesample::stats_paired_export,
            anova::stats_anova_validate,
            anova::stats_anova_run,
            anova::stats_anova_status,
            anova::stats_anova_result,
            anova::stats_anova_export,
            anova::stats_anova_group_levels,
            ranks::stats_ranks_group_levels,
            ranks::stats_ranks_validate,
            ranks::stats_ranks_run,
            ranks::stats_ranks_status,
            ranks::stats_ranks_result,
            ranks::stats_ranks_export,
            related_ranks::stats_related_ranks_validate,
            related_ranks::stats_related_ranks_run,
            related_ranks::stats_related_ranks_status,
            related_ranks::stats_related_ranks_result,
            related_ranks::stats_related_ranks_cancel,
            related_ranks::stats_related_ranks_export,
            // Research OS, Phase 9A-2 (decision 686).
            linear_regression::stats_linear_regression_validate,
            linear_regression::stats_linear_regression_run,
            linear_regression::stats_linear_regression_status,
            linear_regression::stats_linear_regression_result,
            linear_regression::stats_linear_regression_cancel,
            linear_regression::stats_linear_regression_export,
            corr::stats_corr_validate,
            corr::stats_corr_run,
            corr::stats_corr_status,
            corr::stats_corr_result,
            corr::stats_corr_export,
            alpha::stats_alpha_validate,
            alpha::stats_alpha_run,
            alpha::stats_alpha_status,
            alpha::stats_alpha_result,
            alpha::stats_alpha_export,
            prep::stats_prep_make,
            prep::stats_prep_clear,
            prep::stats_prep_recipe,
            prep::stats_prep_csv,
            ws::workspace_state,
            ws::workspace_activate,
            ws::workspace_deactivate,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let n = WINDOWS.fetch_add(1, Ordering::SeqCst);
    let webview = WebviewWindowBuilder::new(&app, format!("p{n}"), Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

/// Four items on 1 to 5, a two-level group, a three-level group, and a free
/// column: every method has what it needs among them.
pub const TABLE: &[u8] = b"q1,q2,q3,q4,arm,site,age\n\
4,2,5,1,a,x,31\n5,1,4,2,a,x,44\n2,4,2,4,a,y,29\n3,3,3,3,a,y,52\n\
1,5,2,5,a,z,37\n4,2,4,2,a,z,41\n5,2,5,1,b,x,63\n2,3,3,3,b,x,35\n\
2,4,1,4,b,y,48\n3,3,4,2,b,y,27\n1,4,2,5,b,z,58\n5,1,5,2,b,z,33\n";

/// The score's column in [`TABLE`].
pub const SCORE: &str = "C8";

pub fn open(w: &World) -> Value {
    call(
        &w.w,
        "stats_desc_open",
        json!({ "path": w.data.to_string_lossy() }),
    )
    .expect("open")
}

/// A recipe of one score of `statistic` over `items`, every item needed.
pub fn recipe(columns: u32, name: &str, statistic: &str, items: &[&str]) -> Value {
    json!({
        "schemaVersion": 1,
        "capabilityId": "stats.prepare.scale_score",
        "capabilityVersion": 1,
        "engineVersion": 1,
        "sourceColumns": columns,
        "steps": [{
            "kind": "scale_score",
            "alias": format!("C{}", columns + 1),
            "name": name,
            "items": items,
            "reversed": [],
            "scale": null,
            "statistic": statistic,
            "rule": "every_item",
            "missingCodes": [],
        }],
    })
}

/// The mean of the first three items of [`TABLE`].
pub fn mean_recipe(name: &str) -> Value {
    recipe(7, name, "mean", &["C1", "C2", "C3"])
}

/// The sum of the same items: another recipe of the same width.
pub fn sum_recipe(name: &str) -> Value {
    recipe(7, name, "sum", &["C1", "C2", "C3"])
}

/// Makes the recipe, which must succeed, and returns the prepared view.
pub fn make(w: &W, slot: &Value, recipe: &Value) -> Value {
    let v = call(
        w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": recipe }),
    )
    .expect("make");
    assert!(v["errors"].as_array().unwrap().is_empty(), "{v}");
    assert!(v["dataset"].is_object(), "{v}");
    v["dataset"].clone()
}

/// A spec as a screen sends it: the view's inputRef and, on a prepared slot,
/// its preparation.
fn spec(view: &Value, capability: &str, parameters: Value) -> Value {
    let mut s = json!({
        "schemaVersion": 1,
        "operationId": 1,
        "workspaceId": "statistics",
        "capabilityId": capability,
        "capabilityVersion": 1,
        "inputs": [view["inputRef"]],
        "parameters": parameters,
        "randomSeed": null,
        "confirmationRefs": [],
    });
    if !view["preparation"].is_null() {
        s["preparation"] = view["preparation"].clone();
    }
    s
}

/// Aliases in column order, as a screen sends them (decision 338).
fn in_column_order(aliases: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = aliases.iter().map(|a| a.to_string()).collect();
    v.sort_by_key(|a| a[1..].parse::<u32>().expect("an alias"));
    v
}

/// One method, as a test drives it: its commands' prefix, and its spec on the
/// column `score`, every other choice fixed on [`TABLE`].
pub struct Method {
    pub prefix: &'static str,
    pub spec: Value,
}

/// All ten methods on `score`. The rank tests, the related rank tests and then the
/// linear regression are last, so no method's index moves.
pub fn ten(view: &Value, score: &str) -> Vec<Method> {
    vec![
        Method {
            prefix: "stats_desc",
            spec: spec(
                view,
                "stats.describe",
                json!({
                    "columns": [{ "alias": score, "treatAs": "numeric" }],
                    "missingCodes": [],
                }),
            ),
        },
        Method {
            prefix: "stats_ttest",
            spec: spec(
                view,
                "stats.analyze.independent_t",
                json!({
                    "outcome": score,
                    "group": "C5",
                    "levels": null,
                    "variant": "welch",
                    "missingCodes": [],
                }),
            ),
        },
        Method {
            prefix: "stats_onesample",
            spec: spec(
                view,
                "stats.analyze.one_sample_t",
                json!({ "outcome": score, "testValue": 3.0, "missingCodes": [] }),
            ),
        },
        Method {
            prefix: "stats_paired",
            spec: spec(
                view,
                "stats.analyze.paired_t",
                json!({ "first": "C4", "second": score, "missingCodes": [] }),
            ),
        },
        Method {
            prefix: "stats_anova",
            spec: spec(
                view,
                "stats.analyze.one_way_anova",
                json!({
                    "outcome": score,
                    "group": "C6",
                    "levels": null,
                    "postHoc": ["tukey"],
                    "missingCodes": [],
                }),
            ),
        },
        Method {
            prefix: "stats_corr",
            spec: spec(
                view,
                "stats.analyze.correlations",
                json!({
                    "variables": in_column_order(&["C7", score]),
                    "coefficients": ["pearson"],
                    "missing": "pairwise",
                    "missingCodes": [],
                }),
            ),
        },
        Method {
            prefix: "stats_alpha",
            spec: spec(
                view,
                "stats.analyze.cronbach_alpha",
                json!({
                    "items": in_column_order(&["C4", "C7", score]),
                    "reversed": [],
                    "scale": null,
                    "missingCodes": [],
                }),
            ),
        },
        // Research OS, Phase 8Y-2 (decision 621).
        Method {
            prefix: "stats_ranks",
            spec: spec(
                view,
                "stats.analyze.independent_ranks",
                json!({
                    "outcome": score,
                    "group": "C5",
                    "levels": null,
                    "missingCodes": [],
                }),
            ),
        },
        // Research OS, Phase 8Z-2 (decision 655): C4, C7 and the score, in this order. With
        // the score C8 that is column order; with stats_prep_commands.rs's C3 (ten(&o, "C3"))
        // it is C4, C7, C3, out of column order, as a screen's list may be (decision 683).
        Method {
            prefix: "stats_related_ranks",
            spec: spec(
                view,
                "stats.analyze.related_ranks",
                json!({ "columns": ["C4", "C7", score], "missingCodes": [] }),
            ),
        },
        // Research OS, Phase 9A-2 (decision 686): C7 on the score and then C4, in this
        // order. With stats_prep_commands.rs's C3 (ten(&o, "C3")) it is C7 on C3 and C4.
        Method {
            prefix: "stats_linear_regression",
            spec: spec(
                view,
                "stats.analyze.linear_regression",
                json!({ "outcome": "C7", "predictors": [score, "C4"], "missingCodes": [] }),
            ),
        },
    ]
}

/// Starts `method`'s run on the slot.
pub fn start(w: &W, slot: &Value, method: &Method) -> Result<u64, Value> {
    call(
        w,
        &format!("{}_run", method.prefix),
        json!({ "slot": slot, "spec": method.spec }),
    )
    .map(|v| v["runId"].as_u64().expect("a run id"))
}

/// Polls `{prefix}_status` until the run is terminal, for at most 30 s.
pub fn until_terminal(w: &W, prefix: &str, run: u64) -> Value {
    for _ in 0..3000 {
        let s = call(w, &format!("{prefix}_status"), json!({ "runId": run })).expect("status");
        if s["terminal"] == true {
            return s;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run} did not end within 30 s");
}

/// A method's validation errors' codes.
pub fn error_codes(report: &Value) -> Vec<String> {
    report["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .map(|e| e["code"].as_str().expect("a code").to_string())
        .collect()
}

/* ---- a run that waits ------------------------------------------------------- */

/// A run that waits at its start until the test opens the gate.
pub struct Gate {
    open: Mutex<bool>,
    turn: Condvar,
    reached: Mutex<bool>,
    reached_turn: Condvar,
}

#[derive(Default)]
struct Ticking(AtomicU64);
impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

impl Gate {
    pub fn new() -> Arc<Gate> {
        Arc::new(Gate {
            open: Mutex::new(false),
            turn: Condvar::new(),
            reached: Mutex::new(false),
            reached_turn: Condvar::new(),
        })
    }
    pub fn runs(self: &Arc<Gate>) -> Arc<Runs> {
        let g = Arc::clone(self);
        Arc::new(Runs::with_hooks(
            Arc::new(Ticking::default()),
            Some(Arc::new(move |_run, p| {
                if p == Point::BeforeStart {
                    *g.reached.lock().unwrap() = true;
                    g.reached_turn.notify_all();
                    let mut open = g.open.lock().unwrap();
                    while !*open {
                        open = g.turn.wait(open).unwrap();
                    }
                }
            })),
            RUN_CEILING_MS,
        ))
    }
    pub fn wait_reached(&self) {
        let mut r = self.reached.lock().unwrap();
        while !*r {
            r = self.reached_turn.wait(r).unwrap();
        }
    }
    pub fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.turn.notify_all();
    }
}
