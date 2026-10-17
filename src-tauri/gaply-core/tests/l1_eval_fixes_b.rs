//! **Phase 2 evaluation fixes, part B: the AI helper layer's pure parts.**
//!
//! Written before the fixes, from the two evaluations' own failing inputs
//! (`~/gaply-corpus/eval2/joint_evaluation.md` M-2, M-8, m-10;
//! `~/gaply-corpus/eval3/deep_evaluation.md` M-6, M-7, §12.4):
//!
//! - the explanation check (every case either report named, the five
//!   contradictions that passed, and honest explanations of five real
//!   Level 1 results, which must pass);
//! - the 64-character header cut, on a character boundary;
//! - a citation attaches to the sentence where its range ends, and a range
//!   over more than two sentences attaches to none.
//!
//! The real results are the Step 8 / part A runs recorded in D270 ("Section
//! 4: real papers with open data"): PLOS ONE pone.0106616 claims 0, 1 and 9,
//! pone.0256959 claim 18 and pone.0260659 claim 2, each rebuilt here with the
//! numbers those runs printed (paper and data CC BY 4.0).

#[path = "stats_assist_support/mod.rs"]
#[allow(dead_code)]
mod stats_assist_support;

use gaply_core::stats_assist::citation::{parse_web_context, RetrievedAt, NO_CITED_SOURCES};
use gaply_core::stats_assist::consent::{DataCategory, OptIns, MAX_HEADER_CHARS};
use gaply_core::stats_assist::outbound::{
    build_binding_suggestion, build_result_explanation, request_body, ExclusionOut, NumberOut, Outbound, ResultExplanationOutbound,
};
use gaply_core::stats_assist::response::{read_response, validate_explanation, Rejection};
use gaply_core::stats_assist::spec::{allowed_web_domains, HelperTask, ModelId};
use gaply_core::stats_l1::catalogue::L1MethodId;
use gaply_core::stats_l1::codes::{self, WireCode};
use gaply_core::stats_l1::compute::ComputedVariant;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
use gaply_core::stats_l1::reproduce::{MatchNote, NotCheckedReason, NumberKind, NumberStatus};
use serde_json::{json, Value};
use stats_assist_support::*;

/* ------------------------------------------------------------------------ *
 *  The explanation check
 * ------------------------------------------------------------------------ */

fn reply(text: &str) -> String {
    json_reply(json!({ "explanation": text }))
}

fn check(sent: &ResultExplanationOutbound, text: &str) -> Result<(), Rejection> {
    validate_explanation(&read_response(reply(text).as_bytes()).unwrap(), sent).map(|_| ())
}

fn number(kind: NumberKind, printed: &str, interval: Option<[&str; 2]>, computed: f64, status: NumberStatus) -> NumberOut {
    NumberOut {
        kind: codes::number_kind(kind),
        printed: printed.to_string(),
        interval: interval.map(|[a, b]| [a.to_string(), b.to_string()]),
        computed,
        status: codes::number_status(status),
    }
}

fn variant_note(v: ComputedVariant) -> WireCode {
    codes::l1_note(gaply_core::stats_l1::reproduce::L1Note::VariantMatched(v))
}

/// A real Level 1 result rebuilt from the numbers its run printed.
struct Real {
    name: &'static str,
    sent: ResultExplanationOutbound,
}

fn base(method: &'static str, verdict: &'static str, cv: ComputedVariant, numbers: Vec<NumberOut>, n_used: usize) -> ResultExplanationOutbound {
    ResultExplanationOutbound {
        task: "result_explanation",
        method_id: Some(method),
        variant: None,
        variant_stated: false,
        computed_variant: Some(codes::computed_variant(cv)),
        verdict,
        conclusion_changes: if verdict == "not_reproduced" { Some(false) } else { None },
        could_not_check_reason: None,
        stated_alpha: vec![".05".to_string()],
        alpha_defaulted: true,
        tails: "not_stated",
        numbers,
        n_used: Some(n_used),
        exclusions: Vec::<ExclusionOut>::new(),
        notes: if verdict == "reproduced" { vec![variant_note(cv)] } else { Vec::new() },
    }
}

fn reals() -> Vec<Real> {
    use NumberKind::*;
    let m = NumberStatus::Matched(None);
    let mag = NumberStatus::Matched(Some(MatchNote::InMagnitude));
    let nm = NumberStatus::NotMatched;
    let prec = NumberStatus::NotChecked(NotCheckedReason::DataPrecision { decimals: 2 });
    vec![
        // pone.0106616 claim 0: t(17) = 2.47, p = .02; data t = -2.473863, p = .024207, n 18.
        Real {
            name: "0106616 claim 0 (reproduced)",
            sent: base(
                "paired_t",
                "reproduced",
                ComputedVariant::Paired,
                vec![
                    number(Statistic, "2.47", Some(["2.465", "2.475"]), -2.47386, mag),
                    number(Df { index: 0 }, "17", Some(["17", "17"]), 17.0, m),
                    number(P, "= .02", Some(["0.015", "0.025"]), 0.0242070, m),
                ],
                18,
            ),
        },
        // pone.0106616 claim 1: t(28) = 1.8, p = .12; data t = -1.582552, p = .124755, n 29.
        Real {
            name: "0106616 claim 1 (not reproduced)",
            sent: base(
                "paired_t",
                "not_reproduced",
                ComputedVariant::Paired,
                vec![
                    number(Statistic, "1.8", Some(["1.75", "1.85"]), -1.58255, nm),
                    number(Df { index: 0 }, "28", Some(["28", "28"]), 28.0, m),
                    number(P, "= .12", Some(["0.115", "0.125"]), 0.124755, m),
                ],
                29,
            ),
        },
        // pone.0106616 claim 9: one-way F reproduced, p within the file's rounding.
        Real {
            name: "0106616 claim 9 (reproduced, p not checked)",
            sent: {
                let mut s = base(
                    "one_way_anova",
                    "reproduced",
                    ComputedVariant::OneWay,
                    vec![
                        number(Statistic, "2.91", Some(["2.905", "2.915"]), 2.91234, m),
                        number(Df { index: 0 }, "1", Some(["1", "1"]), 1.0, m),
                        number(Df { index: 1 }, "50", Some(["50", "50"]), 50.0, m),
                        number(P, "= .09", Some(["0.085", "0.095"]), 0.0942113, prec),
                    ],
                    52,
                );
                s.notes.push(codes::l1_note(gaply_core::stats_l1::reproduce::L1Note::NotChecked {
                    number: P,
                    why: NotCheckedReason::DataPrecision { decimals: 2 },
                }));
                s
            },
        },
        // pone.0256959 claim 18: F(2, 183) = 4.53, p = .01; data F = 4.504384, p = .012314.
        Real {
            name: "0256959 claim 18 (not reproduced)",
            sent: base(
                "one_way_anova",
                "not_reproduced",
                ComputedVariant::OneWay,
                vec![
                    number(Statistic, "4.53", Some(["4.525", "4.535"]), 4.50438, nm),
                    number(Df { index: 0 }, "2", Some(["2", "2"]), 2.0, m),
                    number(Df { index: 1 }, "183", Some(["183", "183"]), 183.0, m),
                    number(P, "= .01", Some(["0.005", "0.015"]), 0.0123140, m),
                ],
                186,
            ),
        },
        // pone.0260659 claim 2: Welch t = 3.60, df = 1153.34, p < 0.001; data t = -3.600458.
        Real {
            name: "0260659 claim 2 (reproduced, Welch)",
            sent: base(
                "independent_t",
                "reproduced",
                ComputedVariant::Welch,
                vec![
                    number(Statistic, "3.60", Some(["3.595", "3.605"]), -3.60046, mag),
                    number(Df { index: 0 }, "1153.34", Some(["1153.335", "1153.345"]), 1153.34, m),
                    number(P, "< 0.001", Some(["0", "0.001"]), 0.000331, m),
                ],
                1155,
            ),
        },
    ]
}

/// What each case must give after the fix.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Want {
    Pass,
    Reject,
}

#[test]
fn the_explanation_check_matches_its_forty_cases() {
    let w = world();
    let rep = build_result_explanation(&w.reproduced, &w.ctx); // t(6) = 4.44 (computed -4.44097), p = .004 (computed .00437), n 8
    let mut nr = rep.clone();
    nr.verdict = "not_reproduced";
    nr.conclusion_changes = Some(false);
    nr.notes.clear();
    let (t, df, p) = printed();
    assert_eq!((t.as_str(), df.as_str(), p.as_str()), ("4.44", "6", ".004"), "the fixture's printed numbers");
    let real = reals();
    let r = |name: &str| &real.iter().find(|x| x.name.starts_with(name)).expect(name).sent;

    use Want::*;
    let cases: Vec<(&str, &ResultExplanationOutbound, &str, Want)> = vec![
        // eval2 M-2: numbers and verdicts the engine did not state (verdict reproduced).
        ("M-2 number words", &rep, "The result was reproduced with a t of four point four.", Reject),
        ("M-2 Arabic-Indic digits", &rep, "The result was reproduced with t = ٤٫٤٤.", Reject),
        ("M-2 full-width digits", &rep, "The result was reproduced with t = ４.４４.", Reject),
        ("M-2 p is 0", &rep, "The exact p is 0, so the effect is certain.", Reject),
        ("M-2 p is 1", &rep, "The p-value is 1.", Reject),
        ("M-2 failed to replicate", &rep, "The data failed to replicate the printed result.", Reject),
        ("M-2 irreproducible", &rep, "This result is irreproducible.", Reject),
        ("M-2 do not match the data", &rep, "The printed numbers do not match the data.", Reject),
        ("M-2 a negative t matches the computed sign", &rep, "The result was reproduced; the computed t was -4.44.", Pass),
        // part B's own list.
        ("one point five", &rep, "It was reproduced, with an effect of one point five.", Reject),
        ("forty-four", &rep, "It was reproduced with forty-four degrees of freedom.", Reject),
        ("superscript", &rep, "It was reproduced; the effect was 4² units.", Reject),
        ("vulgar fraction", &rep, "It was reproduced; about ½ of the rows were used.", Reject),
        ("percent word", &rep, "It was reproduced at the five percent level.", Reject),
        ("df of 2", &rep, "It was reproduced with a df of 2.", Reject),
        ("p = 1", &rep, "It was reproduced; p = 1.", Reject),
        ("p rounded below its printed precision", &rep, "It was reproduced with p = 0.", Reject),
        ("t rounded below its printed precision", &rep, "It was reproduced with t = 4.4.", Reject),
        ("unattached 95 stays allowed", &rep, "It was reproduced; a 95% interval was not part of this check.", Pass),
        ("a positive statistic when the computed one is negative", &rep, "It was reproduced; the t statistic was positive.", Reject),
        ("a minus on a positive input", &rep, "It was reproduced with p = -.004.", Reject),
        ("confirmed", &nr, "The paper's statistics are confirmed by the data.", Reject),
        ("verified", &nr, "The result was verified and is correct.", Reject),
        ("could not be reproduced on a reproduced claim", &rep, "The result could not be reproduced.", Reject),
        ("misconduct: fabricated", &nr, "The numbers look fabricated.", Reject),
        ("misconduct: p-hacking", &nr, "This pattern suggests p-hacking.", Reject),
        ("misconduct: fraud", &nr, "This is not fraud, only a mismatch; the result was not reproduced.", Reject),
        // eval3 M-6: the instructions' own token, and honest not-reproduced wordings.
        ("M-6 the verdict token", &nr, "The verdict is not_reproduced: the printed t of 9.44 does not fit the data.", Reject /* 9.44 is not an input */),
        ("M-6 the verdict token, true numbers", &nr, "The verdict is not_reproduced: the printed t does not match the computed one.", Pass),
        ("M-6 reproduction failed", &nr, "Reproduction failed for the t statistic, so the result is not reproduced.", Pass),
        ("M-6 not reproduced in words", &nr, "The result is not reproduced: the printed t statistic differs from the one computed from the data.", Pass),
        // eval3 M-7: the five contradictions that passed (verdict not reproduced).
        ("M-7 all numbers match", &nr, "All printed numbers match the data.", Reject),
        ("M-7 verified and correct", &nr, "The result was verified and is correct.", Reject),
        ("M-7 every number matched", &nr, "Every number matched exactly.", Reject),
        ("M-7 confirmed by the data", &nr, "The paper's statistics are confirmed by the data.", Reject),
        ("M-7 no discrepancy", &nr, "There is no discrepancy between the paper and the data.", Reject),
        // "confirm" about the binding is not a verdict, but only when the
        // binding is what was confirmed: a nearby "binding" is not a pass.
        ("binding confirmed is not a verdict", &rep, "The binding was confirmed, and the result is reproduced.", Pass),
        ("binding as a decoy before", &nr, "Despite the binding, it was confirmed.", Reject),
        ("binding as a decoy after", &nr, "It was confirmed despite the binding.", Reject),
        // Honest explanations of the fixture's result.
        ("honest fixture 1", &rep, "The result is reproduced: t(6) = 4.44 and p = .004 match the values computed from the data.", Pass),
        ("honest fixture 2", &rep, "Gaply recomputed the test from 8 rows and every printed number matched, so the result is reproduced.", Pass),
        // Honest explanations of real results (Step 8 / part A runs).
        ("real 0106616 c0 a", r("0106616 claim 0"), "The result is reproduced: the paired t of 2.47 with 17 degrees of freedom and p = .02 all match the data.", Pass),
        ("real 0106616 c0 b", r("0106616 claim 0"), "Using 18 pairs, the data give t = -2.474 and p = .024, which round to the printed values. Reproduced.", Pass),
        ("real 0106616 c0 c", r("0106616 claim 0"), "The sign of t was ignored because it depends on the order of the two measurements; with that, the result is reproduced.", Pass),
        ("real 0106616 c1 a", r("0106616 claim 1"), "The result is not reproduced: the printed t of 1.8 differs from the t computed from the data, 1.58, while the df of 28 and p of .12 match.", Pass),
        ("real 0106616 c1 b", r("0106616 claim 1"), "The printed statistic does not fit the data, although the degrees of freedom and the p-value do. The result is not reproduced.", Pass),
        ("real 0106616 c1 c", r("0106616 claim 1"), "Not reproduced. With 29 pairs the data give t = -1.583 and p = .125.", Pass),
        ("real 0106616 c9 a", r("0106616 claim 9"), "The result is reproduced: F(1, 50) = 2.91 matches the data. The p-value was not compared, because the file's rounding to 2 decimals could explain the difference.", Pass),
        ("real 0106616 c9 b", r("0106616 claim 9"), "Using 52 rows, the F statistic and both degrees of freedom match. The result is reproduced, with the p left unchecked.", Pass),
        ("real 0106616 c9 c", r("0106616 claim 9"), "Reproduced. The computed F is 2.912.", Pass),
        ("real 0256959 c18 a", r("0256959 claim 18"), "The result is not reproduced: the printed F of 4.53 differs from the F computed from the data, 4.50, while the df of 2 and 183 and p = .01 match.", Pass),
        ("real 0256959 c18 b", r("0256959 claim 18"), "The F statistic printed in the paper is not the one the data give. The result is not reproduced.", Pass),
        ("real 0256959 c18 c", r("0256959 claim 18"), "Not reproduced: the computed F was 4.504 with p = .0123.", Pass),
        ("real 0260659 c2 a", r("0260659 claim 2"), "The result is reproduced under Welch's version of the t-test: t = 3.60 with 1153.34 degrees of freedom and p < 0.001.", Pass),
        ("real 0260659 c2 b", r("0260659 claim 2"), "Using 1155 rows, the data give a t of -3.600 and p = .00033. The result is reproduced.", Pass),
        ("real 0260659 c2 c", r("0260659 claim 2"), "The sign of t was not compared, because it depends on which group comes first. The result is reproduced.", Pass),
    ];
    assert!(cases.len() >= 40, "{} cases", cases.len());
    let mut wrong = Vec::new();
    let (mut honest, mut honest_ok) = (0, 0);
    for (name, sent, text, want) in &cases {
        let got = check(sent, text);
        let ok = matches!((want, &got), (Want::Pass, Ok(())) | (Want::Reject, Err(_)));
        println!("{:<55} want {:?} got {:?}", name, want, got);
        if *want == Want::Pass && (name.starts_with("honest") || name.starts_with("real")) {
            honest += 1;
            honest_ok += usize::from(got.is_ok());
        }
        if !ok {
            wrong.push(format!("{name}: want {want:?}, got {got:?}"));
        }
    }
    println!("honest explanations passing: {honest_ok} of {honest}; cases: {}", cases.len());
    assert!(honest >= 17);
    assert!(wrong.is_empty(), "{} of {} cases wrong:\n{}", wrong.len(), cases.len(), wrong.join("\n"));
}

/* ------------------------------------------------------------------------ *
 *  The header cut
 * ------------------------------------------------------------------------ */

#[test]
fn every_header_reaches_the_body_cut_to_exactly_64_characters_on_a_character_boundary() {
    let h65 = format!("{}Z", "a".repeat(64));
    let h200 = "b".repeat(200);
    // 62 ASCII, then multi-byte characters at positions 63, 64 and 65.
    let multi = format!("{}éЖ😀tail", "c".repeat(62));
    let headers = vec![h65.clone(), h200.clone(), multi.clone(), "arm".to_string()];
    let rows: Vec<Vec<String>> = (0..6).map(|i| vec![format!("{i}.5"), "1".into(), "2".into(), if i % 2 == 0 { "x" } else { "y" }.into()]).collect();
    let ds = build_dataset(RawTable::from_rows(headers, rows), DataFingerprint::of_bytes(b"headers", "in-memory", TableRef::FirstTable)).unwrap();
    let opt = OptIns::new(HelperTask::BindingSuggestion, &[DataCategory::ColumnHeaders]).unwrap();
    let o = build_binding_suggestion(L1MethodId::IndependentT, &ds.schema, opt.column_headers(&ds.schema), None).unwrap();
    let body = request_body(&Outbound::BindingSuggestion(o), &ModelId::default_model());
    let v: Value = serde_json::from_str(&body).unwrap();
    let input: Value = serde_json::from_str(v["input"].as_str().unwrap()).unwrap();
    let sent: Vec<String> = input["headers"].as_array().unwrap().iter().map(|h| h["header"].as_str().unwrap().to_string()).collect();
    assert_eq!(MAX_HEADER_CHARS, 64);
    // The spec counts characters (Unicode scalar values), not bytes.
    assert_eq!(sent[0], "a".repeat(64), "65 characters cut to 64");
    assert_eq!(sent[1], "b".repeat(64), "200 characters cut to 64");
    assert_eq!(sent[2].chars().count(), 64);
    assert_eq!(sent[2], format!("{}éЖ", "c".repeat(62)), "the 63rd and 64th are multi-byte and survive whole; the 65th does not");
    assert!(!body.contains('Z') && !body.contains("tail") && !body.contains('😀'), "nothing past the 64th character");
    assert_eq!(sent[3], "arm");
}

/* ------------------------------------------------------------------------ *
 *  Citations
 * ------------------------------------------------------------------------ */

fn evidence(text: &str, annotations: Value) -> gaply_core::stats_assist::citation::ContextEvidence {
    let out = read_response(envelope(text, annotations).as_bytes()).unwrap();
    parse_web_context(&out, "independent samples t-test assumptions", RetrievedAt { unix_seconds: 1_792_000_000 }, allowed_web_domains())
}

fn cp(text: &str, needle: &str) -> (usize, usize) {
    let b = text.find(needle).expect("needle");
    let s = text[..b].chars().count();
    (s, s + needle.chars().count())
}

const URL: &str = "https://www.bmj.com/content/1";

#[test]
fn one_citation_spanning_the_whole_text_attaches_to_nothing() {
    // eval2 m-10: one range over the whole answer cited every sentence.
    let text = "Welch's test does not assume equal variances. Student's test does. Both assume independence ([bmj.com](https://www.bmj.com/content/1)).";
    let n = text.chars().count();
    let e = evidence(text, json!([{"type": "url_citation", "url": URL, "title": "BMJ", "start_index": 0, "end_index": n}]));
    assert!(e.items.is_empty(), "a range over three sentences cites none: {e:?}");
    assert_eq!(e.empty_message(), Some(NO_CITED_SOURCES));
    assert_eq!(e.dropped_citations.len(), 1, "and it counts as dropped");
}

#[test]
fn a_citation_attaches_to_the_sentence_where_its_range_ends() {
    // A range from inside the first sentence to the link at the end of the
    // second: two sentences, so it attaches to the second, where it ends.
    let text = "Welch's test does not assume equal variances. Student's pooled test does ([bmj.com](https://www.bmj.com/content/1)).";
    let (_, end) = cp(text, "([bmj.com](https://www.bmj.com/content/1))");
    let start = cp(text, "equal variances").0;
    let e = evidence(text, json!([{"type": "url_citation", "url": URL, "title": "BMJ", "start_index": start, "end_index": end}]));
    assert_eq!(e.items.len(), 1, "{e:?}");
    assert!(e.items[0].sentence.as_str().starts_with("Student's pooled test does"), "{:?}", e.items[0].sentence);
    assert_eq!(e.uncited_sentences, 1);
}
