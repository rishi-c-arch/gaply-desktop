//! **`stats.check`'s façade gives exactly Level 0's own outcome (Research OS,
//! Phase 4A).** `run_stats_consistency_measured`, the command's seam, now
//! runs Level 0 through `gaply_core::stats_adapters::check`. For each input it
//! is compared with the direct call the seam made before:
//! `check_reading(&ex, read_results(&ex, tables))` on the same extraction, by
//! equality (`StatsConsistencyOutcome` is `PartialEq` and deliberately not
//! `Serialize`).
//!
//! The inputs:
//! * every Level 0 input that goes through the command's seam in
//!   `src/stats_consistency.rs`'s tests (a two-result `.txt`, the `.docx` with
//!   a declared table, the 400 kB file preflight would ask about; and the two
//!   the seam refuses, a Gaply report and a missing file, which must still be
//!   refused);
//! * the texts that file's view tests read, written to files so they go
//!   through the seam too;
//! * one synthetic manuscript for each `MethodRegistry::LEVEL_0` fixture file,
//!   each asserted to make its method run.
//!
//! `MethodRegistry::SUMMARY`'s fixtures (`percent_count_fixtures.csv`,
//! `grim_fixtures.csv`) are not applicable: a manuscript cannot reach those
//! methods (`stats_l0::registry`'s header), so `stats.check` never runs them.

mod desc_support;

use app_lib::stats_consistency::run_stats_consistency_measured;
use desc_support::TempDir;
use gaply_core::extract;
use gaply_core::stats_l0::outcome::{check_reading, CheckOutcome, StatsConsistencyOutcome};
use gaply_core::stats_l0::registry::MethodRegistry;
use gaply_core::stats_l0::result::{read_results, TableStructure};
use gaply_core::{Database, GaplyError};

/// A zip archive holding one STORED entry, as `stats_consistency.rs`'s tests
/// build their `.docx`.
fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in bytes {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let (crc, len, n) = (crc32(data), data.len() as u32, name.len() as u16);
    let mut out = Vec::new();
    let w16 = |o: &mut Vec<u8>, v: u16| o.extend_from_slice(&v.to_le_bytes());
    let w32 = |o: &mut Vec<u8>, v: u32| o.extend_from_slice(&v.to_le_bytes());
    w32(&mut out, 0x0403_4B50);
    for v in [20, 0, 0, 0, 0] {
        w16(&mut out, v);
    }
    w32(&mut out, crc);
    w32(&mut out, len);
    w32(&mut out, len);
    w16(&mut out, n);
    w16(&mut out, 0);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(data);
    let cd_offset = out.len() as u32;
    w32(&mut out, 0x0201_4B50);
    for v in [20, 20, 0, 0, 0, 0] {
        w16(&mut out, v);
    }
    w32(&mut out, crc);
    w32(&mut out, len);
    w32(&mut out, len);
    w16(&mut out, n);
    for v in [0, 0, 0, 0] {
        w16(&mut out, v);
    }
    w32(&mut out, 0);
    w32(&mut out, 0);
    out.extend_from_slice(name.as_bytes());
    let cd_size = out.len() as u32 - cd_offset;
    w32(&mut out, 0x0605_4B50);
    for v in [0, 0, 1, 1] {
        w16(&mut out, v);
    }
    w32(&mut out, cd_size);
    w32(&mut out, cd_offset);
    w16(&mut out, 0);
    out
}

fn docx() -> Vec<u8> {
    let body = concat!(
        "<w:p><w:r><w:t>A Title</w:t></w:r></w:p>",
        "<w:p><w:r><w:t>Results</w:t></w:r></w:p>",
        "<w:p><w:r><w:t xml:space=\"preserve\">Scores rose, t(28) = 2.10, p = .045.</w:t></w:r></w:p>",
        "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Group</w:t></w:r></w:p></w:tc>",
        "<w:tc><w:p><w:r><w:t xml:space=\"preserve\">Scores fell, t(28) = 2.10, p = .001.</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
    );
    let doc = format!(
        "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
    );
    stored_zip("word/document.xml", doc.as_bytes())
}

fn long_text() -> String {
    let mut body = String::from("A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n\n");
    let filler =
        "Participants completed the questionnaire in a quiet room before the session began.\n\n";
    while body.len() < 400_000 {
        body.push_str(filler);
    }
    body
}

/// `stats_consistency.rs`'s seam and view inputs.
fn existing_inputs() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "ok.txt",
            b"A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045. Scores fell, t(28) = 2.10, p = .001.\n".to_vec(),
        ),
        ("units.docx", docx()),
        ("long.txt", long_text().into_bytes()),
        (
            "four_verdicts.txt",
            b"A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045. Scores fell, t(28) = 2.10, p = .001. \
              Recall improved, t(28) = 8.50, p = .0001. Errors dropped, t(28) = 2.10, p = .022. \
              Speed changed, r(28) = 1.50, p = .020. A third, t = 3.1, p = .004.\n"
                .to_vec(),
        ),
        (
            "one_tailed.txt",
            b"A Title\n\nMethods\nAll tests were one-tailed.\n\nResults\nErrors dropped, t(28) = 2.10, p = .022.\n".to_vec(),
        ),
        (
            "two_thresholds.txt",
            b"A Title\n\nMethods\nSignificance was set at p < .05, and at p < .01 for the secondary outcomes.\n\nResults\nScores fell, t(28) = 2.10, p = .001.\n"
                .to_vec(),
        ),
        (
            "one_threshold.txt",
            b"A Title\n\nMethods\nSignificance was set at p < .01.\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n".to_vec(),
        ),
        (
            "fractional_threshold.txt",
            b"A Title\n\nMethods\nSignificance was set at p < .05/6.\n\nResults\nScores rose, t(28) = 2.10, p = .045.\n".to_vec(),
        ),
    ]
}

/// One synthetic manuscript per `LEVEL_0` fixture file, the method it must
/// reach, and whether that method can RUN on a manuscript. `df_matches_n`
/// cannot today: the reader gives no r its n (`df_matches_n.rs`'s header,
/// pinned by `the_reader_gives_no_r_an_n_today`), so on an r it is consulted
/// and recorded as not applicable, and that is what is asserted.
fn per_fixture() -> Vec<(&'static str, &'static str, bool, Vec<u8>)> {
    vec![
        (
            "stats_l0/p_fixtures.csv",
            "p_consistency",
            true,
            b"A Title\n\nResults\nScores rose, t(28) = 2.10, p = .045. Effects held, F(2, 40) = 3.30, p = .047.\n".to_vec(),
        ),
        (
            "stats_l0/statcheck_diff_fixtures.csv",
            "p_consistency",
            true,
            b"A Title\n\nResults\nThe groups differed, t(28) = 4.20, p = .000.\n".to_vec(),
        ),
        (
            "stats_l0/estimate_interval_fixtures.csv",
            "estimate_in_interval",
            true,
            b"A Title\n\nResults\nIt worked, t(30) = 2.50, d = 0.45, 95% CI [0.10, 0.80], p = .018.\n".to_vec(),
        ),
        (
            "stats_l0/df_n_fixtures.csv",
            "df_matches_n",
            false,
            b"A Title\n\nResults\nThe scores correlated, r(48) = .30, p = .034.\n".to_vec(),
        ),
    ]
}

fn direct(path: &std::path::Path) -> StatsConsistencyOutcome {
    let ex = extract::extract_path(path).expect("extracts");
    let docx = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("docx"));
    let tables = if docx {
        TableStructure::Declared
    } else {
        TableStructure::NotAvailable
    };
    check_reading(&ex, read_results(&ex, tables))
}

#[test]
fn stats_check_through_its_facade_equals_level_0_on_every_input() {
    let dir = TempDir::new("check-facade");
    let db = Database::in_memory().unwrap();
    let mut compared = 0;
    let mut claims = 0;
    for (name, bytes) in existing_inputs() {
        let p = dir.write(name, &bytes);
        let wrapped =
            run_stats_consistency_measured(&db, &p).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(wrapped, direct(&p), "{name}");
        claims += wrapped.claims_read;
        compared += 1;
    }
    // Every LEVEL_0 fixture file has a manuscript, and its method runs on it.
    let fixtures: Vec<&str> = MethodRegistry::LEVEL_0
        .iter()
        .flat_map(|m| m.fixtures.iter().copied())
        .collect();
    let inputs = per_fixture();
    assert_eq!(
        inputs.iter().map(|(f, _, _, _)| *f).collect::<Vec<_>>(),
        fixtures,
        "one synthetic manuscript per LEVEL_0 fixture file, in the registry's order"
    );
    for (fixture, method, runs, bytes) in inputs {
        let p = dir.write(&format!("{}.txt", method), &bytes);
        let wrapped =
            run_stats_consistency_measured(&db, &p).unwrap_or_else(|e| panic!("{fixture}: {e:?}"));
        assert_eq!(wrapped, direct(&p), "{fixture}");
        let reached = wrapped
            .claims
            .iter()
            .flat_map(|c| c.checks.iter())
            .any(|c| c.method_id() == method && (!runs || matches!(c, CheckOutcome::Ran(_))));
        assert!(
            reached,
            "{fixture}: {method} was not reached on its manuscript: {:?}",
            wrapped.claims
        );
        claims += wrapped.claims_read;
        compared += 1;
    }
    println!("compared {compared} inputs, {claims} claims, by equality");
    assert_eq!(compared, 8 + 4);
}

#[test]
fn the_files_the_seam_refused_it_still_refuses() {
    let dir = TempDir::new("check-facade-refused");
    let db = Database::in_memory().unwrap();
    let markers = gaply_core::ai_engine::audit_prepass::GAPLY_REPORT_MARKERS.join("\n");
    let report = dir.write("report.txt", markers.as_bytes());
    let err = run_stats_consistency_measured(&db, &report).unwrap_err();
    assert!(
        matches!(err, GaplyError::Validation(ref m) if m.contains("Gaply audit report")),
        "{err:?}"
    );
    assert!(run_stats_consistency_measured(&db, &dir.0.join("missing.txt")).is_err());
}

/// The Phase 4 evaluation's F5: Level 1's open hands its own reading to
/// `stats_adapters::check_with_reading` instead of letting the façade read the
/// results again. On every input above, that façade's outcome equals Level 0
/// called directly, and so equals `stats_adapters::check` (which reads them
/// itself).
#[test]
fn the_check_facade_on_a_given_reading_equals_level_0_on_every_input() {
    use gaply_core::contracts::capability::Build;
    use gaply_core::stats_adapters::{check, check_spec, check_with_reading};
    let dir = TempDir::new("check-facade-reading");
    let mut compared = 0;
    let inputs: Vec<(String, Vec<u8>)> = existing_inputs()
        .into_iter()
        .map(|(n, b)| (n.to_string(), b))
        .chain(
            per_fixture()
                .into_iter()
                .map(|(_, m, _, b)| (format!("{m}.txt"), b)),
        )
        .collect();
    for (name, bytes) in inputs {
        let p = dir.write(&name, &bytes);
        let ex = extract::extract_path(&p).expect("extracts");
        let tables = if name.ends_with(".docx") {
            TableStructure::Declared
        } else {
            TableStructure::NotAvailable
        };
        let given = check_with_reading(
            &check_spec(),
            Build::Standard,
            &ex,
            read_results(&ex, tables),
        )
        .expect("runnable");
        assert_eq!(given, direct(&p), "{name}");
        assert_eq!(
            given,
            check(&check_spec(), Build::Standard, &ex, tables).expect("runnable"),
            "{name}"
        );
        compared += 1;
    }
    assert_eq!(compared, 8 + 4);
}
