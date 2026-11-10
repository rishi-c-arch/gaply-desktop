//! **The levels commands on a prepared slot (Research OS, Phase 8M-3, decision
//! 425).** Reached only from a prepared slot, so tested through IPC. A score's
//! values are never listed as levels: each command refuses it with its own
//! `score_as_group` and the column's alias. A column the table does not have
//! stays `unknown_column`, and a file's column is listed as on any table.

mod desc_support;
mod prep_support;

use serde_json::{json, Value};

use desc_support::{call, refusal};
use prep_support::*;

fn levels(w: &World, command: &str, slot: &Value, column: &str) -> Result<Value, Value> {
    call(&w.w, command, json!({ "slot": slot, "column": column }))
}

const COMMANDS: [(&str, &str); 3] = [
    ("stats_ttest_group_levels", "stats.ttest"),
    ("stats_anova_group_levels", "stats.anova"),
    // Phase 8Y-1 (decision 607).
    ("stats_ranks_group_levels", "stats.ranks"),
];

#[test]
fn a_levels_request_on_a_score_is_refused_with_score_as_group() {
    let w = world("levels-score", TABLE, None);
    let o = open(&w);
    make(&w.w, &o["slot"], &mean_recipe("wellbeing"));
    for (command, words) in COMMANDS {
        let e = levels(&w, command, &o["slot"], SCORE).expect_err(command);
        let r = refusal(&e);
        assert_eq!(
            r["code"],
            format!("{words}.score_as_group"),
            "{command}: {r}"
        );
        assert_eq!(
            r["params"]["column"], 8,
            "{command}: the column's alias: {r}"
        );
    }
}

#[test]
fn a_column_the_table_lacks_stays_unknown_and_a_files_column_is_listed() {
    let w = world("levels-file", TABLE, None);
    let o = open(&w);
    for (command, _) in COMMANDS {
        let before = levels(&w, command, &o["slot"], "C5").expect("the file's column");
        assert_eq!(before["levels"].as_array().unwrap().len(), 2, "{before}");
    }
    let before: Vec<Value> = COMMANDS
        .iter()
        .map(|(c, _)| levels(&w, c, &o["slot"], "C6").expect("the file's column"))
        .collect();
    make(&w.w, &o["slot"], &mean_recipe("wellbeing"));
    for ((command, words), was) in COMMANDS.iter().zip(&before) {
        let e = levels(&w, command, &o["slot"], "C9").expect_err(command);
        assert_eq!(
            refusal(&e)["code"],
            format!("{words}.unknown_column"),
            "{command}"
        );
        let after = levels(&w, command, &o["slot"], "C6").expect("still listed");
        assert_eq!(
            &after, was,
            "{command}: the same answer on the prepared slot"
        );
    }
    // Cleared, the score's column is gone, and is unknown again.
    call(&w.w, "stats_prep_clear", json!({ "slot": o["slot"] })).expect("clears");
    for (command, words) in COMMANDS {
        let e = levels(&w, command, &o["slot"], SCORE).expect_err(command);
        assert_eq!(refusal(&e)["code"], format!("{words}.unknown_column"));
    }
}
