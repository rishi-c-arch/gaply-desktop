//! **What Gaply reads from every `.sav` and `.zsav` its tests use (Research
//! OS, Phase 8F-4, decision 305).** Taken under ambers 0.4.4 before the
//! upgrade to 0.4.6 and pinned here, one SHA-256 per input over the input's
//! own SHA-256 and three readings of it:
//!
//! * the reviewer agent's files: `parse_supplementary_with` on the file
//!   written to disk, under `SUPPLEMENTARY` (`parse_sav`);
//! * the same bytes through `parse_supplementary_bytes_with`
//!   (`evidence_from_bytes`), under `LEVEL_1`;
//! * Level 1's dataset from the bytes (`load_dataset_from_bytes_with`): its
//!   columns, rows, weight, metadata, issues and every cell, without its
//!   fingerprint, which holds `PARSER_VERSION` and changes with the bump.
//!
//! Each reading writes out every header, cell, label, missing rule and
//! error (their `Debug` prints counts only), so any of them that changes
//! changes the pin. The
//! inputs are the two tracked fixtures, every file `sav_support` builds for
//! 8F-2 and 8F-3, and copies of the ambers-written files of
//! `stats_l1_ingest.rs`, `supplementary.rs`, `ingest_limits.rs` and
//! `ingest_cell_cap.rs` in the shapes those tests write (found by search).
//!
//! ambers' writer stamps its own version and the clock into the header,
//! so the files it writes here have both fixed (`sav_support::fixed_stamp`)
//! before they are hashed or read.
//!
//! `GAPLY_SAV_RECORD=1` prints the record: each input's digests, and the
//! readings whole for inputs of under 200 rows. Synthetic data only.

mod desc_support;
mod sav_support;

use app_lib::stats_l1_ingest::load_dataset_from_bytes_with;
use app_lib::supplementary::SupplementaryEvidence;
use app_lib::supplementary::{
    parse_supplementary_bytes_with, parse_supplementary_with, IngestLimits, MAX_COLS,
};
use desc_support::TempDir;
use gaply_core::research_project::DataFormat;
use gaply_core::stats_l1::dataset::{sha256_hex, Cell, MissingRule};
use sav_support::{crashing, proper, same_values, sav, Order};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// A zlib `.zsav` of `vars` numeric variables `v1`… and `rows` cases, value
/// `r × v`, as `ingest_limits.rs` and `ingest_cell_cap.rs` write it.
fn sav_bytes(vars: usize, rows: usize) -> Vec<u8> {
    use arrow::array::Float64Array;
    use arrow::datatypes::{DataType, Field};
    let fields: Vec<Field> = (1..=vars)
        .map(|v| Field::new(format!("v{v}"), DataType::Float64, false))
        .collect();
    let columns: Vec<std::sync::Arc<dyn arrow::array::Array>> = (1..=vars)
        .map(|v| {
            std::sync::Arc::new(Float64Array::from(
                (0..rows).map(|r| (r * v) as f64).collect::<Vec<_>>(),
            )) as _
        })
        .collect();
    zlib(fields, columns)
}

/// A zlib `.zsav` of `rows` cases of 1.0 in each of `names`, as
/// `supplementary.rs`'s and `stats_l1_ingest.rs`'s tests write it.
fn ones(names: &[&str], rows: usize) -> Vec<u8> {
    use arrow::array::Float64Array;
    use arrow::datatypes::{DataType, Field};
    let fields = names
        .iter()
        .map(|n| Field::new(*n, DataType::Float64, false))
        .collect();
    let columns = names
        .iter()
        .map(|_| std::sync::Arc::new(Float64Array::from(vec![1.0; rows])) as _)
        .collect();
    zlib(fields, columns)
}

fn zlib(
    fields: Vec<arrow::datatypes::Field>,
    columns: Vec<std::sync::Arc<dyn arrow::array::Array>>,
) -> Vec<u8> {
    let batch = arrow::record_batch::RecordBatch::try_new(
        std::sync::Arc::new(arrow::datatypes::Schema::new(fields)),
        columns,
    )
    .unwrap();
    let mut buf = std::io::Cursor::new(Vec::new());
    ambers::write_sav_to_writer(
        &mut buf,
        &batch,
        &ambers::SpssMetadata::default(),
        ambers::Compression::Zlib,
        None,
    )
    .unwrap();
    sav_support::fixed_stamp(buf.into_inner())
}

/// `stats_l1_ingest.rs`'s `write_sav`: `subject`, `group` (1/2/9, 9
/// user-missing, 1 and 2 labelled), `score`, and `wt` the weight when
/// `weighted`, uncompressed.
fn labelled(rows: usize, weighted: bool) -> Vec<u8> {
    use arrow::array::Float64Array;
    use arrow::datatypes::{DataType, Field, Schema};
    use std::sync::Arc;
    let subject: Vec<f64> = (1..=rows).map(|i| i as f64).collect();
    let group: Vec<f64> = (0..rows).map(|i| [1.0, 2.0, 9.0][i % 3]).collect();
    let score: Vec<f64> = (0..rows).map(|i| 10.0 + (i % 7) as f64).collect();
    let wt: Vec<f64> = (0..rows).map(|i| 0.5 + (i % 4) as f64 * 0.25).collect();
    let schema = Arc::new(Schema::new(vec![
        Field::new("subject", DataType::Float64, false),
        Field::new("group", DataType::Float64, false),
        Field::new("score", DataType::Float64, false),
        Field::new("wt", DataType::Float64, false),
    ]));
    let batch = arrow::record_batch::RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Float64Array::from(subject)),
            Arc::new(Float64Array::from(group)),
            Arc::new(Float64Array::from(score)),
            Arc::new(Float64Array::from(wt)),
        ],
    )
    .unwrap();
    let mut meta = ambers::SpssMetadata::from_arrow_schema(&schema);
    meta.variable_missing_values
        .entry("group".into())
        .or_default()
        .push(ambers::MissingSpec::Value(9.0));
    let labels = meta
        .variable_value_labels
        .entry("group".into())
        .or_default();
    labels.insert(ambers::Value::Numeric(1.0), "control".into());
    labels.insert(ambers::Value::Numeric(2.0), "treatment".into());
    if weighted {
        meta.weight_variable = Some("wt".into());
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    ambers::write_sav_to_writer(&mut buf, &batch, &meta, ambers::Compression::None, None).unwrap();
    sav_support::fixed_stamp(buf.into_inner())
}

/// Every input, by name.
fn inputs() -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = Vec::new();
    for f in ["analysis.sav", "l1_weighted.sav"] {
        v.push((
            format!("fixture {f}"),
            std::fs::read(format!("{FIXTURES}/{f}")).unwrap(),
        ));
    }
    for (name, bytes) in crashing() {
        v.push((format!("crashing {name}"), bytes));
    }
    v.push(("proper little".into(), proper(Order::Little)));
    v.push(("proper big".into(), proper(Order::Big)));
    for (order, o) in [("little", Order::Little), ("big", Order::Big)] {
        for (name, bytes, _) in same_values(o) {
            v.push((format!("same_values {order} {name}"), bytes));
        }
    }
    // stats_l1_commands.rs's proper file: the four treated and four control
    // scores of `l1_support`, and the group 1 and 2.
    let x = [12.1, 14.3, 11.8, 13.6, 15.2, 16.9, 17.4, 16.0];
    let rows: Vec<Vec<f64>> = x
        .iter()
        .enumerate()
        .map(|(i, v)| vec![*v, if i < 4 { 1.0 } else { 2.0 }])
        .collect();
    v.push((
        "level 1 score arm".into(),
        sav(Order::Little, 2, &["score", "arm"], &rows),
    ));
    for (vars, rows) in [
        (1, 4),
        (2, 6),
        (2, 10),
        (2, 2_500),
        (4, 2),
        (4, 50),
        (4, 51),
        (5, 10),
        (5, 50),
        (MAX_COLS, 2),
        (MAX_COLS + 1, 2),
    ] {
        v.push((format!("sav_bytes {vars} x {rows}"), sav_bytes(vars, rows)));
    }
    v.push(("zsav x 100".into(), ones(&["x"], 100)));
    v.push(("zlib x 200000".into(), ones(&["x"], 200_000)));
    v.push(("zlib x y z 200000".into(), ones(&["x", "y", "z"], 200_000)));
    for (rows, weighted) in [(6, true), (5_000, false), (5_001, false)] {
        v.push((
            format!("labelled {rows} weighted {weighted}"),
            labelled(rows, weighted),
        ));
    }
    v
}

/// A missing rule written out: its `Debug` prints no value.
fn rule(r: &MissingRule) -> String {
    match r {
        MissingRule::Value(v) => format!("value {v:?}"),
        MissingRule::Range { lo, hi } => format!("range {lo:?} {hi:?}"),
        MissingRule::Text(t) => format!("text {t:?}"),
    }
}

/// Supplementary's evidence written out whole. Its `Debug` prints counts
/// only (part C follow-up, item 7), so every header, cell, label and rule is
/// written here.
fn evidence(r: Result<SupplementaryEvidence, gaply_core::GaplyError>) -> String {
    let ev = match r {
        Err(e) => return format!("error {e:?}"),
        Ok(ev) => ev,
    };
    let mut out = format!(
        "kind {:?}\nsummary {:?}\nnotes {:?}\nweight {:?}\nrows_truncated {}\nvariables_held {:?}\ncells_over {}\nsheet {:?}\n",
        ev.kind, ev.text_summary, ev.notes, ev.weight_variable, ev.rows_truncated, ev.variables_held, ev.cells_over, ev.sheet_name
    );
    for v in &ev.variables {
        out.push_str(&format!(
            "variable {:?} {:?} numeric {} label {:?} value_labels {:?} user_missing {:?} rules {:?} codes {:?}\n",
            v.name,
            v.declared_type,
            v.is_numeric,
            v.label,
            v.value_labels,
            v.user_missing,
            v.user_missing_rules.iter().map(rule).collect::<Vec<_>>(),
            v.value_label_codes
        ));
    }
    for t in &ev.tables {
        out.push_str(&format!("headers {:?}\n", t.headers));
        for row in &t.rows {
            out.push_str(&format!("row {row:?}\n"));
        }
    }
    out
}

/// Level 1's dataset written out whole, without its fingerprint.
fn dataset(
    r: Result<
        gaply_core::stats_l1::dataset::Dataset,
        gaply_core::stats_l1::dataset::L1IngestRefusal,
    >,
) -> String {
    let d = match r {
        Err(e) => return format!("refused {e:?}"),
        Ok(d) => d,
    };
    let s = &d.schema;
    let mut out = format!(
        "rows {}\nweight {:?}\nmetadata {:?}\nissues {:?}\n",
        s.rows, s.weight_variable, s.source_metadata, s.issues
    );
    for c in &s.columns {
        let levels: Option<Vec<_>> = c.levels.as_ref().map(|l| {
            l.levels
                .iter()
                .map(|x| (x.code.clone(), x.label.clone(), x.count))
                .collect()
        });
        out.push_str(&format!(
            "column {:?} {:?} {:?} missing {} distinct {:?} bucket {:?} levels {:?} issues {:?}\n",
            c.alias,
            c.header,
            c.column_type,
            c.has_missing,
            c.distinct,
            c.level_bucket,
            levels,
            c.issues
        ));
        let cells: Vec<String> = d
            .column_cells(c.alias)
            .unwrap_or_default()
            .iter()
            .map(|x| match x {
                Cell::Missing => "missing".to_string(),
                Cell::Number(v) => format!("{v:?}"),
                Cell::Text(t) => format!("{t:?}"),
            })
            .collect();
        out.push_str(&format!("cells {cells:?}\n"));
    }
    out
}

/// The three readings of one input, as text.
fn readings(dir: &TempDir, bytes: &[u8]) -> [String; 3] {
    let p = dir.write("input.sav", bytes);
    [
        evidence(parse_supplementary_with(&p, IngestLimits::SUPPLEMENTARY)),
        evidence(parse_supplementary_bytes_with(
            bytes,
            "sav",
            IngestLimits::LEVEL_1,
        )),
        dataset(load_dataset_from_bytes_with(
            bytes,
            DataFormat::Sav,
            IngestLimits::LEVEL_1,
        )),
    ]
}

/// Each input's SHA-256 and its record's, taken under ambers 0.4.4 at
/// 7a7a24f before any change. Under 0.4.6 every one read the same before
/// decision 306's change; the three marked changed with it, on purpose (the
/// files of case size 0 and -1 read their values, and the big-endian one of
/// case size -1 is refused as big-endian, where all three were malformed).
const PINS: &[(&str, &str, &str)] = &[
    (
        "fixture analysis.sav",
        "314d890480e05ae57c6c5b1a51379c84031edc01681826b15714722141154204",
        "8084dd5ebf7cbf0d2880eb09cf5141ee9990bde469674b8a6025a0b3216c2fa0",
    ),
    (
        "fixture l1_weighted.sav",
        "d8113be4abef0d0b369f489f8f79a8679f9cc2a9025ac97e6dddc0f34c9bd9d6",
        "9499de07d3730cf4b6a7868a99b102657f1836d691871b28706372d2d016b2f2",
    ),
    (
        "crashing no_variables",
        "b10bc8cfccf51ac7b538063694e421ec1c5e355107af3d444cdc6d2ffae11096",
        "56340eefaf3c788ab0b7dee6f15ed9a4e68ca405735f801da00cf4b78748b340",
    ),
    // Changed on purpose in 8F-4 (decision 306): read, or refused as big-endian.
    (
        "crashing case_size_0",
        "e9409d250f2f23d10118da07cb393d5cf415b0558e01c9c20785fbe61509280f",
        "7581919020e5b84b90ea53f4dd8bd3a5020e7868a377fa43e562e79d5ed0e4d7",
    ),
    // Changed on purpose in 8F-4 (decision 306): read, or refused as big-endian.
    (
        "crashing case_size_minus_1",
        "893c8c052b5272869d0156f848f9c80bf8f835c105e0ad243432007f09ce2d31",
        "48e4a5ba956d59e2148157a1ec883eb9353b5d59b11777afdcf8315d164d0afe",
    ),
    // Changed on purpose in 8F-4 (decision 306): read, or refused as big-endian.
    (
        "crashing case_size_minus_1_big_endian",
        "cbb9e2bcad71cb0428ded9763bb892dc8f6939637573859dcd80f21edd57cb0c",
        "2f39189c0da8cc3c06aa537ae70bf1650eb0afc521fd71b5444320c4dcae846a",
    ),
    (
        "proper little",
        "a6dccf7b2a1caedb50df9f777dc13ee628f3bb3dae0b17fc8c667cfebcd0a839",
        "eee02e4a323ef6a93382b51a0c78c862bc6a8765db45c56db25eb03193279c08",
    ),
    (
        "proper big",
        "c6a5c51ee393d0765ecb0a37580ff4ddbd86187119cb106bb5af33c29df1055a",
        "a561e8772e413db053051a1c5de3313b071da87ae221c44aeb460384005ae6ba",
    ),
    (
        "same_values little uncompressed_1_5",
        "35ff8cadc6197c7953b5d4d2670f6b9f09feee2251ba7a3436c75f4fd61fd92c",
        "ca2220aa2acfb1b2fe2a12779024791e960406446e926cd48d4ab482959a2a6a",
    ),
    (
        "same_values little bytecode_1_5",
        "f981af1e3ee40457ed8f6dfde154b23dc7d259f8640be38a5029818662446d1d",
        "5c09137f706a4b9a420639cb9738e28b18106df633efae21f37e14b6295f668e",
    ),
    (
        "same_values little bytecode_1_to_5",
        "b41cc8d7dc1a975a2d346b2afdc551b796f0e19db5892f41e8d0e6e5ed91590e",
        "1623c3277fcb590f7bebbdcc740d07e6f7c83bba15a7351f047e86aac0cc9ef2",
    ),
    (
        "same_values big uncompressed_1_5",
        "1e72b6113222fe51223c1cb6603f320433a411c56213a1fbfc597a3b9270b0c3",
        "c17df11c58154abd1db5e459454401ff3088e5b13fad4da69f6f665afd5bde16",
    ),
    (
        "same_values big bytecode_1_5",
        "09221c2f21fe22e6dcd136311586ab668d2e8041913e2550429557dd86660437",
        "37454f98822af7f8d5cb6b84a8248a6f3a548644f6f9571a70bef5d8f8bffe0f",
    ),
    (
        "same_values big bytecode_1_to_5",
        "569f1349398af9867e307f29aa09d51eaaa077683309524f6cd69f800454f325",
        "a5f00e6832fbe9f290287f76b765298e33883f6919565f5a79e7066d9926bd52",
    ),
    (
        "level 1 score arm",
        "d6dc1650114ba732e0b1a7ac7d5ac9b24d16d7e5d95c5b0478c3975a9a7115a1",
        "4d2db7fac3090b16624b4e9f70d7512ef2d46fd790ebc793d4864e65ea21b6fa",
    ),
    (
        "sav_bytes 1 x 4",
        "b8c6930ea50f895d02f5fafc0f351c4da254a06cce03e02195bf27e4565eecc0",
        "77ec77d616240d2f59f19da4d63681ba3bfc90569ac13e89255328299173c1f1",
    ),
    (
        "sav_bytes 2 x 6",
        "51c50b410661ac8afa864f31cb1debbd6ac50d632eb43cfffa22380f63b72504",
        "4bd0604ddb6fb0f57d2ca3fa588804a074a4f88a5b483aad7f6d57dd00f7a8df",
    ),
    (
        "sav_bytes 2 x 10",
        "ed55792e249c1f5bffcd96a899edbc0407d83323c14dead5a98b645822eed8b5",
        "df028bf1d86bc4613e9985c112d286fe9dbc90333f439670f7eaf2609657ec9a",
    ),
    (
        "sav_bytes 2 x 2500",
        "04c1ab3bda0c276abb20f0abad0ab202cb2d05528ea7a101c1afa1a6cd0a8ab4",
        "005d2b691dc8ce0637544d19ac2e87515e603e8b3e16a2847633d8d98807e3ab",
    ),
    (
        "sav_bytes 4 x 2",
        "8c5f55e0ffd689fe3e864790710c4602e89c8cd0698685a676252ef3336daa46",
        "60b0a1afdf1ecb7d0cd452cd0b380b845148c4eaee33f7abaa1070a6146ec342",
    ),
    (
        "sav_bytes 4 x 50",
        "6c2e16f96c3801b616bca62076fad5030025f285c58531693e19849dbc000c04",
        "8c5404385af61bcac7dda7df32a19b86a3ecc6af1e91332e7ac3e7fab1819c80",
    ),
    (
        "sav_bytes 4 x 51",
        "0294fab5dc029e71c49fe3655aff34d2e3da9af596d12e614ff1ed8f6c54d6a8",
        "4f093c9464a05e889aa266ba5ba773338d8e767369dc4a27eaaf5f314af7e6ef",
    ),
    (
        "sav_bytes 5 x 10",
        "52a846de500fe249bbce7a0964356f9ffde44f5f421a9b70c0d73069f75e9aab",
        "ebbc9d04730fef972c18564e721dfb3db19b988d0d7cb3d479cd7271277d3b4b",
    ),
    (
        "sav_bytes 5 x 50",
        "32935a036ff150a34782137185da4b342519e44a5bd492803c017f00e25fb712",
        "0c2f9eed7f038469384fc9fbd24931784ef2f09cfc64f77fe55042eab8bf8d15",
    ),
    (
        "sav_bytes 128 x 2",
        "77df3ac8cc008793fd6c80e8926a03f934c8c91dc6a4beb9e949cc7162d5f11f",
        "b7b16fcedd45cf3dd37932edc51ce8b2549729cccbf72ba162a0cf2fa3b85b11",
    ),
    (
        "sav_bytes 129 x 2",
        "4e355f42cc91e71d1edcfd0aa4a07af9eb6a27d48b036725ca6c8e94ecf8536e",
        "3ac707048e5d7cbe5134f544cd50dcd139c5e4acf3c4394da674f8a6345b0f63",
    ),
    (
        "zsav x 100",
        "d8c034754d6b870176805cb1e320b40481f6bddace7c040964f27d626e0fe733",
        "c1b7b394c6116b0e7e4949fe280ad345c71a8a78b3d749711d4e0e53b6f3ecb8",
    ),
    (
        "zlib x 200000",
        "40bd86c1e1406be1f8e7f5291a6a8aabc18f834c3cdc6dc16ab11236e0012c45",
        "20651ab5b60f59144debe33e23dfd66f4ced67e6d838397e6a2c08fe920babf6",
    ),
    (
        "zlib x y z 200000",
        "5773ca8600c260ab77671d1b415a8715a0291ed5d92cf0d45afc3088995dd36f",
        "f2cc38dadf6c100be0e88d7ba828787f0ea87cfc36df911f3a54f0cf1f33fb7f",
    ),
    (
        "labelled 6 weighted true",
        "5aba0bd5b9ace97425d19c805b1dea9ba5d1bafc50084e1b1f7ff6c8995b699e",
        "7c6479d2f09a283fbe75f17899c49939c55fc00c8b123608fa0f1713d8ed0aed",
    ),
    (
        "labelled 5000 weighted false",
        "d0ef4ea6aa12992102ef227ec25ca481ea0852318005321246f31903e95a4d77",
        "da2cc99b579c7c95fabe921c0b03b7bdd0f82a3f479c0948dd23c5d762e88fc4",
    ),
    (
        "labelled 5001 weighted false",
        "9edfe7b8166349610ce5a32ac9a8287c17061e66bccd59fae5aa38cb783b668d",
        "ab870319e78325c6d61515dcb722c7618ad7beb827b1da1bca4522a35538bd57",
    ),
];

#[test]
fn every_sav_input_reads_as_it_did_under_ambers_0_4_4() {
    let dir = TempDir::new("sav_record");
    let print = std::env::var("GAPLY_SAV_RECORD").is_ok();
    let mut wrong = Vec::new();
    let all = inputs();
    for (name, bytes) in &all {
        let input = sha256_hex(bytes);
        let [a, b, c] = readings(&dir, bytes);
        let record = sha256_hex(format!("{input}\n{a}\n{b}\n{c}").as_bytes());
        if print {
            println!("INPUT\t{name}\t{input}\t{record}");
            if c.len() < 100_000 {
                println!("PATH\t{a}\nBYTES\t{b}\nLEVEL1\t{c}");
            }
            println!("PIN\t(\"{name}\", \"{input}\", \"{record}\"),");
        }
        match PINS.iter().find(|p| p.0 == name) {
            Some(&(_, i, r)) if i == input && r == record => {}
            Some(&(_, i, _)) if i != input => wrong.push(format!("{name}: the input changed")),
            Some(_) => wrong.push(format!("{name}: the reading changed")),
            None => wrong.push(format!("{name}: not pinned")),
        }
    }
    assert_eq!(all.len(), PINS.len(), "one pin per input");
    assert!(wrong.is_empty(), "{wrong:#?}");
}
