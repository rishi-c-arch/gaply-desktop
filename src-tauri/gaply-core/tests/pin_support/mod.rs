//! **The per-platform pin's layout check (Research OS, decision 346),
//! shared by the exports' pins (8H-2, decision 354).** Moved unchanged from
//! `anova_export.rs`, where 8H wrote it: on a platform with no pin of its
//! own, an export's CSV is checked against a pin's layout, every field byte
//! for byte except the numbers (the unquoted value field, within
//! `LAYOUT_ULPS` of the pin's) and the output hash (64 lowercase hex
//! characters), and its manifest's JSON line by line but the output hash's.
//! It applies to any export of the eight fields with the value at index 6
//! and a `"manifest","output_sha256"` record: the ANOVA's and the
//! correlations'.
#![allow(dead_code)]

/// How far a number may be from the pin's on a platform with no pin of its
/// own: Linux x86_64's measured difference is 1 (decision 345).
pub const LAYOUT_ULPS: u64 = 4;

/// Units in the last place between two doubles: their distance in the
/// order of all doubles, so it holds across zero and a power of two.
pub fn ulps(a: f64, b: f64) -> u64 {
    let key = |x: f64| {
        let magnitude = (x.to_bits() & !(1u64 << 63)) as i128;
        if x.is_sign_negative() {
            -magnitude
        } else {
            magnitude
        }
    };
    (key(a) - key(b)).unsigned_abs() as u64
}

pub fn is_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Each record's fields as written, quotes included. The pin has no CRLF
/// inside a field; a `""` inside one flips the quote state twice.
fn raw_records(bytes: &[u8]) -> Result<Vec<Vec<&str>>, String> {
    let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let body = s
        .strip_suffix("\r\n")
        .ok_or("no CRLF after the last record")?;
    Ok(body
        .split("\r\n")
        .map(|line| {
            let (mut fields, mut start, mut quoted) = (Vec::new(), 0, false);
            for (i, c) in line.char_indices() {
                match c {
                    '"' => quoted = !quoted,
                    ',' if !quoted => {
                        fields.push(&line[start..i]);
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            fields.push(&line[start..]);
            fields
        })
        .collect())
}

/// Decision 346's check of `got` against the pin's layout: the same records
/// and fields, every field byte for byte except the numbers (the unquoted
/// value field, within `LAYOUT_ULPS` of the pin's) and the output hash (64
/// lowercase hex characters).
pub fn csv_layout_matches(got: &[u8], pin: &[u8]) -> Result<(), String> {
    let (got, pin) = (raw_records(got)?, raw_records(pin)?);
    if got.len() != pin.len() {
        return Err(format!("{} records, the pin has {}", got.len(), pin.len()));
    }
    for (i, (g, p)) in got.iter().zip(&pin).enumerate() {
        if g.len() != p.len() {
            return Err(format!(
                "record {i}: {} fields, the pin has {}",
                g.len(),
                p.len()
            ));
        }
        let hash_line = p[0] == "\"manifest\"" && p[1] == "\"output_sha256\"";
        for (k, (gf, pf)) in g.iter().zip(p).enumerate() {
            if hash_line && k == 7 {
                let hex = gf.strip_prefix('"').and_then(|h| h.strip_suffix('"'));
                if !hex.is_some_and(is_hash) {
                    return Err(format!("record {i}: {gf} is not a hash"));
                }
            } else if i > 0 && k == 6 && !pf.is_empty() && !pf.starts_with('"') {
                let (a, b) = match (gf.parse::<f64>(), pf.parse::<f64>()) {
                    (Ok(a), Ok(b)) => (a, b),
                    _ => return Err(format!("record {i}: {gf} against the pin's {pf}")),
                };
                if ulps(a, b) > LAYOUT_ULPS {
                    return Err(format!(
                        "record {i}: {gf} is {} ulps from the pin's {pf}",
                        ulps(a, b)
                    ));
                }
            } else if gf != pf {
                return Err(format!(
                    "record {i}, field {k}: {gf} where the pin has {pf}"
                ));
            }
        }
    }
    Ok(())
}

/// The same for the manifest's JSON: every line byte for byte (its numbers
/// are counts and versions, which no maths library touches) except the
/// output hash's.
pub fn json_layout_matches(got: &[u8], pin: &[u8]) -> Result<(), String> {
    let got = std::str::from_utf8(got).map_err(|e| e.to_string())?;
    let pin = std::str::from_utf8(pin).map_err(|e| e.to_string())?;
    let (g, p): (Vec<&str>, Vec<&str>) = (got.split('\n').collect(), pin.split('\n').collect());
    if g.len() != p.len() {
        return Err(format!("{} lines, the pin has {}", g.len(), p.len()));
    }
    const KEY: &str = "  \"outputHash\": \"";
    for (i, (gl, pl)) in g.iter().zip(&p).enumerate() {
        if let Some(rest) = pl.strip_prefix(KEY) {
            let tail = &rest[64..];
            let hex = gl.strip_prefix(KEY).and_then(|r| r.strip_suffix(tail));
            if !hex.is_some_and(is_hash) {
                return Err(format!("line {i}: {gl} is not the hash line"));
            }
        } else if gl != pl {
            return Err(format!("line {i}: {gl} where the pin has {pl}"));
        }
    }
    Ok(())
}
