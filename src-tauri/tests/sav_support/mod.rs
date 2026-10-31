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
    sav_bytecode_sized(order, vars.len() as i32, vars, rows)
}

/// [`sav_bytecode`] with the header's nominal case size the caller's
/// (Research OS, Phase 8F-4, decision 306).
pub fn sav_bytecode_sized(
    order: Order,
    case_size: i32,
    vars: &[&str],
    rows: &[Vec<f64>],
) -> Vec<u8> {
    let mut w = dictionary(order, 1, case_size, vars, rows.len());
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
/// in big-endian order. Since 8F-4 (decision 306) the two little-endian files
/// with variables read their values ([`CRASHING_VALUES`]; [`reads_since_8f4`])
/// and the big-endian one is refused as big-endian.
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

/// The values [`crashing`]'s two files that read since 8F-4 hold.
pub const CRASHING_VALUES: [[f64; 2]; 2] = [[1.0, 2.0], [3.0, 4.0]];

/// Whether one of [`crashing`]'s files reads under ambers 0.4.6, with 8F-2's
/// case-size check removed (decision 306): the two little-endian files with
/// variables, a case size of 0 and of -1.
pub fn reads_since_8f4(name: &str) -> bool {
    matches!(name, "case_size_0" | "case_size_minus_1")
}

/// The JSON a method's open gives for each of [`crashing`]'s files it
/// refuses: an empty table is no columns, and since 8F-4 (decision 306) the
/// big-endian file is `sav_big_endian`, which until then was malformed. A
/// code with no parameters is a bare word inside another code
/// (`stats_l1::codes`).
pub fn expected_refusal(name: &str) -> serde_json::Value {
    match name {
        "no_variables" => serde_json::json!({ "code": "dataset", "refusal": "no_columns" }),
        "case_size_minus_1_big_endian" => serde_json::json!({ "code": "sav_big_endian" }),
        other => panic!("{other} reads since 8F-4"),
    }
}

/// The reason Level 1's `data_refused` carries for the same files.
pub fn expected_level_1_reason(name: &str) -> serde_json::Value {
    match name {
        "no_variables" => serde_json::json!({ "code": "dataset", "refusal": "no_columns" }),
        "case_size_minus_1_big_endian" => serde_json::json!("sav_big_endian"),
        other => panic!("{other} reads since 8F-4"),
    }
}

/* ---- Research OS, Phase 8F-4: ambers 0.4.6 (decisions 306 and 307) ------- */

/// One variable of a [`Spec`], one eight-byte slot wide. Its name is the
/// short name's bytes as stored, so a code-page file can hold a byte that is
/// not UTF-8; its type field is written as given, so a test can declare one
/// no proper file has.
pub struct Var {
    pub name: Vec<u8>,
    pub type_field: i32,
    pub format: i32,
    pub label: Option<Vec<u8>>,
    pub missing: Vec<[u8; 8]>,
}

/// SPSS's print and write formats, packed as the variable record holds them.
pub const F8_2: i32 = 0x0005_0802;
pub const A8: i32 = 0x0001_0800;
pub const DATE11: i32 = 0x0014_0B00;

impl Var {
    pub fn number(name: &[u8]) -> Var {
        Var {
            name: name.to_vec(),
            type_field: 0,
            format: F8_2,
            label: None,
            missing: Vec::new(),
        }
    }
    pub fn date(name: &[u8]) -> Var {
        Var {
            format: DATE11,
            ..Var::number(name)
        }
    }
    /// A string variable of width 8.
    pub fn text(name: &[u8]) -> Var {
        Var {
            type_field: 8,
            format: A8,
            ..Var::number(name)
        }
    }
}

/// One cell of a [`Spec`]'s rows.
#[derive(Clone)]
pub enum Cell {
    Number(f64),
    /// At most eight bytes, padded with spaces as SPSS stores them.
    Text(Vec<u8>),
}

/// Eight bytes of a string, padded with spaces.
pub fn eight(s: &[u8]) -> [u8; 8] {
    assert!(s.len() <= 8);
    let mut b = [b' '; 8];
    b[..s.len()].copy_from_slice(s);
    b
}

/// A little-endian `.sav` laid out as PSPP's "System File Format" chapter
/// gives it: the header, a variable record per [`Var`] (with its label and
/// missing values), value labels (records 3 and 4), extension records
/// (type 7, the encoding's subtype 20 first), the dictionary's end and the
/// cases, uncompressed or bytecode-compressed as [`sav_bytecode`] writes them
/// (a string slot is always a 253 and its eight raw bytes). The header's
/// case size and case count are the variables' and the rows' unless given.
pub struct Spec {
    pub compression: i32,
    pub case_size: Option<i32>,
    pub cases: Option<i32>,
    pub vars: Vec<Var>,
    pub rows: Vec<Vec<Cell>>,
    /// The 1-based slot a set of labels is for, and its (value, label) pairs.
    pub value_labels: Vec<(i32, Vec<([u8; 8], Vec<u8>)>)>,
    /// The label count the first value-label record declares, if not its own.
    pub label_count: Option<i32>,
    pub encoding: Option<&'static str>,
    /// Extension records: subtype, element size, element count, and the
    /// bytes written, which a test may make shorter than size × count.
    pub info: Vec<(i32, i32, i32, Vec<u8>)>,
}

impl Spec {
    pub fn new(vars: Vec<Var>, rows: Vec<Vec<Cell>>) -> Spec {
        Spec {
            compression: 0,
            case_size: None,
            cases: None,
            vars,
            rows,
            value_labels: Vec::new(),
            label_count: None,
            encoding: None,
            info: Vec::new(),
        }
    }

    /// One numeric variable per name, from rows of numbers.
    pub fn numbers(names: &[&str], rows: &[Vec<f64>]) -> Spec {
        Spec::new(
            names
                .iter()
                .map(|n| Var::number(n.to_ascii_uppercase().as_bytes()))
                .collect(),
            rows.iter()
                .map(|r| r.iter().map(|v| Cell::Number(*v)).collect())
                .collect(),
        )
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut w = W {
            order: Order::Little,
            out: Vec::new(),
        };
        w.out.extend_from_slice(b"$FL2");
        w.padded("@(#) SPSS DATA FILE gaply synthetic", 60);
        w.i32(2);
        w.i32(self.case_size.unwrap_or(self.vars.len() as i32));
        w.i32(self.compression);
        w.i32(0);
        w.i32(self.cases.unwrap_or(self.rows.len() as i32));
        w.f64(100.0);
        w.padded("01 Jan 26", 9);
        w.padded("00:00:00", 8);
        w.padded("", 64);
        w.out.extend_from_slice(&[0, 0, 0]);
        assert_eq!(w.out.len(), 176, "the header is 176 bytes");
        for v in &self.vars {
            assert!(v.name.len() <= 8);
            w.i32(2);
            w.i32(v.type_field);
            w.i32(v.label.is_some() as i32);
            w.i32(v.missing.len() as i32);
            w.i32(v.format);
            w.i32(v.format);
            let mut name = v.name.clone();
            name.resize(8, b' ');
            w.out.extend_from_slice(&name);
            if let Some(label) = &v.label {
                w.i32(label.len() as i32);
                let mut l = label.clone();
                l.resize(label.len().div_ceil(4) * 4, b' ');
                w.out.extend_from_slice(&l);
            }
            for m in &v.missing {
                w.out.extend_from_slice(m);
            }
        }
        for (i, (slot, labels)) in self.value_labels.iter().enumerate() {
            w.i32(3);
            w.i32(if i == 0 {
                self.label_count.unwrap_or(labels.len() as i32)
            } else {
                labels.len() as i32
            });
            for (value, label) in labels {
                w.out.extend_from_slice(value);
                assert!(label.len() < 256);
                let mut l = vec![label.len() as u8];
                l.extend_from_slice(label);
                l.resize(l.len().div_ceil(8) * 8, b' ');
                w.out.extend_from_slice(&l);
            }
            w.i32(4);
            w.i32(1);
            w.i32(*slot);
        }
        if let Some(name) = self.encoding {
            w.i32(7);
            w.i32(20);
            w.i32(1);
            w.i32(name.len() as i32);
            w.out.extend_from_slice(name.as_bytes());
        }
        for (subtype, size, count, data) in &self.info {
            w.i32(7);
            w.i32(*subtype);
            w.i32(*size);
            w.i32(*count);
            w.out.extend_from_slice(data);
        }
        w.i32(999);
        w.i32(0);
        let slot = |w: &mut W, c: &Cell| match c {
            Cell::Number(v) => w.f64(*v),
            Cell::Text(s) => w.out.extend_from_slice(&eight(s)),
        };
        if self.compression == 0 {
            for row in &self.rows {
                assert_eq!(row.len(), self.vars.len());
                for c in row {
                    slot(&mut w, c);
                }
            }
        } else {
            let cells: Vec<&Cell> = self.rows.iter().flatten().collect();
            for block in cells.chunks(8) {
                let mut commands = [0u8; 8];
                let mut raw = Vec::new();
                for (k, c) in block.iter().enumerate() {
                    match c {
                        Cell::Number(v)
                            if v.fract() == 0.0 && (1.0..=251.0).contains(&(v + 100.0)) =>
                        {
                            commands[k] = (v + 100.0) as u8;
                        }
                        _ => {
                            commands[k] = 253;
                            raw.push((*c).clone());
                        }
                    }
                }
                w.out.extend_from_slice(&commands);
                for c in &raw {
                    slot(&mut w, c);
                }
            }
        }
        w.out
    }
}

/// ambers' writer stamps its own version into the header's product name
/// (bytes 4 to 63, `writer/records.rs:98`, "@(#) SPSS DATA FILE ambers
/// 0.4.4") and the clock's date and time (bytes 92 to 108), so two versions,
/// or two runs, write different bytes. This sets both to fixed text, which
/// nothing that reads the file uses.
pub fn fixed_stamp(mut b: Vec<u8>) -> Vec<u8> {
    let mut product = b"@(#) SPSS DATA FILE ambers".to_vec();
    product.resize(60, b' ');
    b[4..64].copy_from_slice(&product);
    b[92..109].copy_from_slice(b"01 Jan 2600:00:00");
    b
}

/// A `.zsav` (zlib, `$FL3`) of numeric variables from ambers' own writer
/// (its source is identical in 0.4.4 and 0.4.6; [`fixed_stamp`] fixes the
/// version and clock it stamps), with the header's nominal case size
/// then set to `case_size` at bytes 68 to 71, which the zlib compression
/// leaves uncompressed.
pub fn zsav_sized(case_size: i32, names: &[&str], rows: &[Vec<f64>]) -> Vec<u8> {
    use arrow::array::Float64Array;
    use arrow::datatypes::{DataType, Field, Schema};
    let fields: Vec<Field> = names
        .iter()
        .map(|n| Field::new(*n, DataType::Float64, false))
        .collect();
    let columns: Vec<std::sync::Arc<dyn arrow::array::Array>> = (0..names.len())
        .map(|i| {
            std::sync::Arc::new(Float64Array::from(
                rows.iter().map(|r| r[i]).collect::<Vec<_>>(),
            )) as _
        })
        .collect();
    let batch = arrow::record_batch::RecordBatch::try_new(
        std::sync::Arc::new(Schema::new(fields)),
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
    let mut b = fixed_stamp(buf.into_inner());
    assert_eq!(&b[..4], b"$FL3");
    assert_eq!(
        i32::from_le_bytes(b[68..72].try_into().unwrap()),
        names.len() as i32,
        "the writer's case size"
    );
    b[68..72].copy_from_slice(&case_size.to_le_bytes());
    b
}

/// The values of decision 306's files: two variables `X` and `Y`, a raw value
/// and a whole number from the bias' table in each row.
pub const SIZED: [[f64; 2]; 2] = [[1.5, 2.0], [3.25, 4.0]];

/// Decision 306: the same two variables with a case size of 0 and of -1, on
/// each of ambers' three read paths (uncompressed, bytecode, zlib), each
/// beside the same file with its proper case size, whose reading it must
/// equal.
pub fn non_positive_case_size() -> Vec<(String, Vec<u8>, Vec<u8>)> {
    let rows: Vec<Vec<f64>> = SIZED.iter().map(|r| r.to_vec()).collect();
    let mut out = Vec::new();
    for cs in [0, -1] {
        out.push((
            format!("uncompressed_case_size_{cs}"),
            sav(Order::Little, cs, &["x", "y"], &rows),
            sav(Order::Little, 2, &["x", "y"], &rows),
        ));
        out.push((
            format!("bytecode_case_size_{cs}"),
            sav_bytecode_sized(Order::Little, cs, &["x", "y"], &rows),
            sav_bytecode(Order::Little, &["x", "y"], &rows),
        ));
        out.push((
            format!("zlib_case_size_{cs}"),
            zsav_sized(cs, &["x", "y"], &rows),
            zsav_sized(2, &["x", "y"], &rows),
        ));
    }
    out
}

/// Decision 306: a big-endian file with variables and a case size of 0 or
/// -1, which must still be refused as big-endian.
pub fn big_endian_non_positive() -> Vec<(String, Vec<u8>)> {
    let rows: Vec<Vec<f64>> = SIZED.iter().map(|r| r.to_vec()).collect();
    [0, -1]
        .iter()
        .flat_map(|&cs| {
            [
                (
                    format!("big_endian_case_size_{cs}"),
                    sav(Order::Big, cs, &["x", "y"], &rows),
                ),
                (
                    format!("big_endian_bytecode_case_size_{cs}"),
                    sav_bytecode_sized(Order::Big, cs, &["x", "y"], &rows),
                ),
            ]
        })
        .collect()
}

/// Days from 1970-01-01 to a date of the proleptic Gregorian calendar
/// (Howard Hinnant's `days_from_civil`).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// An SPSS date: seconds since 14 October 1582, which is 141,428 days before
/// 1970-01-01.
pub fn spss_date(y: i64, m: i64, d: i64) -> f64 {
    ((days_from_civil(y, m, d) + 141_428) * 86_400) as f64
}

/// What one of decision 307's files should read as under ambers 0.4.6.
#[derive(Debug, Clone, PartialEq)]
pub enum Want {
    /// These headers and these cells, as Gaply prints them (a number is
    /// compared as a number). Level 1 reads the cells at `missing` (row,
    /// column) as missing: the file declares their value user-missing.
    Reads {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        missing: Vec<(usize, usize)>,
    },
    /// Refused, in supplementary's malformed words.
    Malformed,
}

/// The rows of decision 307's zero-case and case-size files: five rows of
/// `X` and `Y`, raw values and whole numbers mixed.
pub fn five_rows() -> Vec<Vec<f64>> {
    vec![
        vec![1.0, 10.0],
        vec![2.5, 20.0],
        vec![3.0, 30.25],
        vec![4.0, 40.0],
        vec![5.0, 50.0],
    ]
}

fn number_want(rows: &[Vec<f64>]) -> Want {
    Want::Reads {
        headers: vec!["X".into(), "Y".into()],
        rows: rows
            .iter()
            .map(|r| r.iter().map(|v| v.to_string()).collect())
            .collect(),
        missing: Vec::new(),
    }
}

/// Decision 307's files, each with its name, its bytes, what 0.4.6 reads,
/// and whether reading it under 0.4.4 is safe (the smaller case size reads
/// past a buffer through an unchecked pointer, and the oversized
/// declarations can ask for gigabytes; 0.4.5's CHANGELOG, issues #1 and #3).
pub fn fixes() -> Vec<(&'static str, Vec<u8>, Want, bool)> {
    let five = five_rows();
    let n = |names: &[&str], rows: &[Vec<f64>]| Spec::numbers(names, rows);
    let with = |mut s: Spec, f: &dyn Fn(&mut Spec)| {
        f(&mut s);
        s.bytes()
    };
    let two = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
    vec![
        (
            "zero_cases_uncompressed",
            with(n(&["x", "y"], &five), &|s| s.cases = Some(0)),
            number_want(&five),
            true,
        ),
        (
            "zero_cases_bytecode",
            with(n(&["x", "y"], &five), &|s| {
                s.cases = Some(0);
                s.compression = 1
            }),
            number_want(&five),
            true,
        ),
        (
            "case_size_larger_uncompressed",
            with(n(&["x", "y"], &five), &|s| s.case_size = Some(3)),
            number_want(&five),
            true,
        ),
        (
            "case_size_larger_bytecode",
            with(n(&["x", "y"], &five), &|s| {
                s.case_size = Some(3);
                s.compression = 1
            }),
            number_want(&five),
            true,
        ),
        (
            "case_size_smaller_uncompressed",
            with(n(&["x", "y"], &five), &|s| s.case_size = Some(1)),
            number_want(&five),
            false,
        ),
        (
            "case_size_smaller_bytecode",
            with(n(&["x", "y"], &five), &|s| {
                s.case_size = Some(1);
                s.compression = 1
            }),
            number_want(&five),
            false,
        ),
        (
            "date_out_of_range",
            Spec::new(
                vec![Var::date(b"D"), Var::number(b"X")],
                vec![
                    vec![Cell::Number(spss_date(2026, 1, 1)), Cell::Number(1.0)],
                    vec![Cell::Number(spss_date(12_000, 1, 1)), Cell::Number(2.0)],
                    vec![Cell::Number(1e300), Cell::Number(3.0)],
                ],
            )
            .bytes(),
            Want::Reads {
                headers: vec!["D".into(), "X".into()],
                rows: vec![
                    vec!["2026-01-01".into(), "1".into()],
                    vec!["".into(), "2".into()],
                    vec!["".into(), "3".into()],
                ],
                missing: Vec::new(),
            },
            true,
        ),
        ("code_page", code_page().bytes(), code_page_want(), true),
        (
            "duplicate_names",
            n(&["x", "x"], &two).bytes(),
            Want::Malformed,
            true,
        ),
        (
            "type_field_256",
            with(n(&["x", "y"], &two), &|s| s.vars[1].type_field = 256),
            Want::Malformed,
            true,
        ),
        (
            "label_count_oversized",
            with(n(&["x", "y"], &two), &|s| {
                s.value_labels = vec![(1, vec![(1.0f64.to_le_bytes(), b"one".to_vec())])];
                s.label_count = Some(1_000_000_000);
            }),
            Want::Malformed,
            false,
        ),
        (
            "info_record_oversized",
            with(n(&["x", "y"], &two), &|s| {
                s.info = vec![(13, 1, 1_300_000_000, b"X=x\tY=y".to_vec())]
            }),
            Want::Malformed,
            false,
        ),
    ]
}

/// Decision 307's code-page file: windows-1252 through its character-encoding
/// record (subtype 20). A numeric variable named CAFÉ (0xC9) with the label
/// "Café crème" and the value label 1 = "été"; a string variable NOM with
/// the missing value "NÉANT"; the cells 1, "Zélie" and 2, "NÉANT".
pub fn code_page() -> Spec {
    let mut cafe = Var::number(b"CAF\xC9");
    cafe.label = Some(b"Caf\xE9 cr\xE8me".to_vec());
    let mut nom = Var::text(b"NOM");
    nom.missing = vec![eight(b"N\xC9ANT")];
    let mut s = Spec::new(
        vec![cafe, nom],
        vec![
            vec![Cell::Number(1.0), Cell::Text(b"Z\xE9lie".to_vec())],
            vec![Cell::Number(2.0), Cell::Text(b"N\xC9ANT".to_vec())],
        ],
    );
    s.value_labels = vec![(1, vec![(1.0f64.to_le_bytes(), b"\xE9t\xE9".to_vec())])];
    s.encoding = Some("windows-1252");
    s
}

fn code_page_want() -> Want {
    Want::Reads {
        headers: vec!["CAFÉ".into(), "NOM".into()],
        rows: vec![
            vec!["1".into(), "Zélie".into()],
            vec!["2".into(), "NÉANT".into()],
        ],
        missing: vec![(1, 1)],
    }
}

/// Decision 307, measured only: a variable record whose type field is -2,
/// between two numeric variables, with a slot of data for it. ambers 0.4.6
/// treats a negative type other than -1 as a string continuation slot with
/// a finding (its CHANGELOG, issue #4), not as an error.
pub fn type_field_minus_2() -> Vec<u8> {
    let mut s = Spec::numbers(
        &["x", "c", "z"],
        &[vec![1.0, 9.0, 2.0], vec![3.0, 9.0, 4.0]],
    );
    s.vars[1].type_field = -2;
    s.bytes()
}

/// ambers' version in the workspace's `Cargo.lock`.
pub fn locked_ambers_version() -> String {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"))
        .expect("Cargo.lock");
    let mut lines = lock.lines();
    while let Some(l) = lines.next() {
        if l == "name = \"ambers\"" {
            let v = lines.next().expect("a version line");
            return v
                .trim_start_matches("version = \"")
                .trim_end_matches('"')
                .to_string();
        }
    }
    panic!("ambers is not in Cargo.lock")
}

/// Stops a test before it reads a file that is unsafe under ambers 0.4.4,
/// so a rollback of the pin fails the test instead of running it.
pub fn require_ambers_0_4_5_or_later(file: &str) {
    let v = locked_ambers_version();
    let parts: Vec<u32> = v.split('.').map(|p| p.parse().unwrap()).collect();
    assert!(
        parts >= vec![0, 4, 5],
        "{file} is not read under ambers {v}: before 0.4.5 it reads past a buffer or asks for gigabytes (0.4.5's CHANGELOG, issues #1 and #3)"
    );
}
