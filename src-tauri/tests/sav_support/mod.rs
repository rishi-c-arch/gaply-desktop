//! Hand-made SPSS `.sav` files for the tests of decision 291 (Research OS,
//! Phase 8F-2): the 176-byte file header, a numeric variable record per name,
//! the dictionary's end (record type 999) and the cases, uncompressed, laid
//! out as PSPP's "System File Format" chapter gives them, in either byte
//! order. The header's nominal case size is the caller's, so a file can have
//! variables and say 0 or -1 there, as some software writes.
//!
//! Synthetic numbers only; nothing here is read from a real file.
#![allow(dead_code)]

/// The byte order a file is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Little,
    Big,
}

struct W {
    order: Order,
    out: Vec<u8>,
}

impl W {
    fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&match self.order {
            Order::Little => v.to_le_bytes(),
            Order::Big => v.to_be_bytes(),
        });
    }
    fn f64(&mut self, v: f64) {
        self.out.extend_from_slice(&match self.order {
            Order::Little => v.to_le_bytes(),
            Order::Big => v.to_be_bytes(),
        });
    }
    fn padded(&mut self, s: &str, len: usize) {
        let mut b = s.as_bytes().to_vec();
        b.resize(len, b' ');
        self.out.extend_from_slice(&b);
    }
}

/// A `.sav` with one numeric variable per name in `vars` (names of at most
/// eight ASCII characters), `rows` cases of them, and `case_size` as the
/// header's nominal case size (a proper file gives `vars.len()`).
pub fn sav(order: Order, case_size: i32, vars: &[&str], rows: &[Vec<f64>]) -> Vec<u8> {
    let mut w = dictionary(order, 0, case_size, vars, rows.len());
    for row in rows {
        assert_eq!(row.len(), vars.len());
        for v in row {
            w.f64(*v);
        }
    }
    w.out
}

/// The same file bytecode-compressed (compression code 1, bias 100), as
/// PSPP's chapter gives it (Research OS, Phase 8F-3, decision 295): the
/// cases as blocks of eight one-byte commands, each block followed by the
/// raw eight-byte values its 253 commands name. A whole number from -99 to
/// 151 is the command `value + 100`; any other value is 253 and its raw
/// bytes, in the file's byte order. The last block is padded with 0.
pub fn sav_bytecode(order: Order, vars: &[&str], rows: &[Vec<f64>]) -> Vec<u8> {
    let mut w = dictionary(order, 1, vars.len() as i32, vars, rows.len());
    let values: Vec<f64> = rows
        .iter()
        .inspect(|r| assert_eq!(r.len(), vars.len()))
        .flatten()
        .copied()
        .collect();
    for block in values.chunks(8) {
        let mut raw = Vec::new();
        let mut commands = [0u8; 8];
        for (k, v) in block.iter().enumerate() {
            let code = v + 100.0;
            if v.fract() == 0.0 && (1.0..=251.0).contains(&code) {
                commands[k] = code as u8;
            } else {
                commands[k] = 253;
                raw.push(*v);
            }
        }
        w.out.extend_from_slice(&commands);
        for v in raw {
            w.f64(v);
        }
    }
    w.out
}

/// The header and dictionary both builders share, through the record of
/// type 999.
fn dictionary(order: Order, compression: i32, case_size: i32, vars: &[&str], cases: usize) -> W {
    let mut w = W {
        order,
        out: Vec::new(),
    };
    // The file header, 176 bytes.
    w.out.extend_from_slice(b"$FL2");
    w.padded("@(#) SPSS DATA FILE gaply synthetic", 60);
    w.i32(2); // layout code
    w.i32(case_size); // nominal case size
    w.i32(compression); // 0 none, 1 bytecode
    w.i32(0); // weight index: none
    w.i32(cases as i32); // cases
    w.f64(100.0); // bias
    w.padded("01 Jan 26", 9);
    w.padded("00:00:00", 8);
    w.padded("", 64); // file label
    w.out.extend_from_slice(&[0, 0, 0]);
    assert_eq!(w.out.len(), 176, "the header is 176 bytes");
    // A numeric variable record per name: F8.2.
    for name in vars {
        assert!(name.len() <= 8 && name.is_ascii());
        w.i32(2); // record type
        w.i32(0); // numeric
        w.i32(0); // no variable label
        w.i32(0); // no missing values
        w.i32(0x0005_0802); // print format F8.2
        w.i32(0x0005_0802); // write format F8.2
        w.padded(&name.to_ascii_uppercase(), 8);
    }
    // The dictionary's end.
    w.i32(999);
    w.i32(0);
    w
}

/// The four files ambers 0.4.4 panics on (decision 291), each with its name:
/// no variables; and two variables with a case size of 0, of -1, and of -1
/// in big-endian order.
pub fn crashing() -> Vec<(&'static str, Vec<u8>)> {
    let rows = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
    vec![
        ("no_variables", sav(Order::Little, 0, &[], &[])),
        ("case_size_0", sav(Order::Little, 0, &["x", "y"], &rows)),
        (
            "case_size_minus_1",
            sav(Order::Little, -1, &["x", "y"], &rows),
        ),
        (
            "case_size_minus_1_big_endian",
            sav(Order::Big, -1, &["x", "y"], &rows),
        ),
    ]
}

/// A proper two-variable file, in `order`: 1.5, 2.25 and 3.0, 4.75.
pub fn proper(order: Order) -> Vec<u8> {
    sav(order, 2, &["x", "y"], &[vec![1.5, 2.25], vec![3.0, 4.75]])
}

/// The values of [`same_values`]' files, by name, one variable `X`.
pub const ONE_AND_A_HALF: [f64; 2] = [1.5, 2.25];
pub const ONE_TO_FIVE: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];

/// Three files in `order` (Research OS, Phase 8F-3, decision 295), one
/// variable `X` each: 1.5 and 2.25 uncompressed, the same bytecode-compressed
/// (each value a 253 and its raw bytes), and 1 to 5 bytecode-compressed (each
/// value a command from the bias' table, no raw bytes). ambers 0.4.4 reads
/// the first two wrongly when big-endian, and the third rightly.
pub fn same_values(order: Order) -> Vec<(&'static str, Vec<u8>, Vec<f64>)> {
    let col = |v: &[f64]| v.iter().map(|x| vec![*x]).collect::<Vec<_>>();
    vec![
        (
            "uncompressed_1_5",
            sav(order, 1, &["x"], &col(&ONE_AND_A_HALF)),
            ONE_AND_A_HALF.to_vec(),
        ),
        (
            "bytecode_1_5",
            sav_bytecode(order, &["x"], &col(&ONE_AND_A_HALF)),
            ONE_AND_A_HALF.to_vec(),
        ),
        (
            "bytecode_1_to_5",
            sav_bytecode(order, &["x"], &col(&ONE_TO_FIVE)),
            ONE_TO_FIVE.to_vec(),
        ),
    ]
}

/// The JSON a method's open gives for each of [`crashing`]'s files once
/// decision 291's check is in: an empty table is no columns, a case size of
/// 0 or less is malformed. A code with no parameters is a bare word inside
/// another code (`stats_l1::codes`).
pub fn expected_refusal(name: &str) -> serde_json::Value {
    if name == "no_variables" {
        serde_json::json!({ "code": "dataset", "refusal": "no_columns" })
    } else {
        serde_json::json!({ "code": "malformed" })
    }
}

/// The reason Level 1's `data_refused` carries for the same files.
pub fn expected_level_1_reason(name: &str) -> serde_json::Value {
    if name == "no_variables" {
        serde_json::json!({ "code": "dataset", "refusal": "no_columns" })
    } else {
        serde_json::json!("malformed")
    }
}
