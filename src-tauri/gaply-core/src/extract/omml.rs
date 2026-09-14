//! **The OMML reader — Word's stored mathematics, onto the same AST.**
//!
//! # Why it lives in `extract/` and not in `equation/`
//!
//! Reading OMML is INGEST: untrusted bytes, a zip, an XML parser. The Tier-0
//! engine is pure computation, and `tests/equation_is_llm_free.rs` pins its
//! imports to `std` and `serde` precisely so that nothing with a parser in it
//! can drift in. So the boundary is: extraction turns OMML into the linear
//! notation a manuscript would have typed, and
//! [`crate::equation::linear::parse_equation`] reads that.
//!
//! **That is also how "the same AST" is achieved by construction rather than by
//! two code paths agreeing.** There is one parser. An equation Word stored as
//! OMML and the same equation typed as text become the same [`Expr`] because
//! they go through the same function, not because two readers were written to
//! match. §11 D156's lesson about two vocabularies applies to parsers too.
//!
//! [`Expr`]: crate::equation::expr::Expr
//!
//! # Why this was the SECOND piece and not the first
//!
//! §6b.1 named an OMML reader as the gating item. Measured: zero of the six
//! manuscripts carry OMML and 21 equation-shaped lines survive `docparse`
//! intact. OMML appears in 3 of 17 documents on this machine, so its priority
//! rests on correctness — `docparse` used to flatten it into fabricated digits
//! (§11 D155) — not on coverage.
//!
//! # What it refuses
//!
//! Anything it cannot represent EXACTLY is an error naming the element, not a
//! partial reading. An n-ary operator, a matrix, a function with a limit — each
//! returns [`OmmlError::Unsupported`]. Silently dropping a `∑` would turn a sum
//! into its summand, which is the §11 D155 failure with a different tag.

use std::io::Read;

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::error::GaplyError;

const NS_M: &[u8] = b"http://schemas.openxmlformats.org/officeDocument/2006/math";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmmlError {
    /// An OMML construct this reader cannot render exactly.
    Unsupported(String),
    /// A structural element with the wrong number of children (a fraction with
    /// no denominator). Malformed, not merely unfamiliar.
    Malformed(String),
    Xml(String),
}

impl std::fmt::Display for OmmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OmmlError::Unsupported(e) => {
                write!(f, "`m:{e}` is not read by the exact OMML reader")
            }
            OmmlError::Malformed(e) => write!(f, "`m:{e}` has the wrong shape"),
            OmmlError::Xml(e) => write!(f, "OMML xml error: {e}"),
        }
    }
}

/// One equation found in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxEquation {
    /// Zero-based index of the `w:p` it sits in — the locator every finding
    /// needs, and the `.docx` anchoring §11 D65 settled on.
    pub paragraph: usize,
    /// The equation in linear notation, ready for the parser.
    pub linear: String,
    /// Did this element contain STRUCTURE — a fraction, a power, a radical, a
    /// delimiter — as opposed to a bare run of symbols?
    ///
    /// **This is the same predicate `docparse` uses to decide between
    /// `[equation]` and the inline text** (§11 D155's lossless-flatten rule).
    /// It is exposed so a caller splicing structured equations back into the
    /// prose stream can align the two streams by the rule rather than by
    /// counting — an inline `N` produces an equation here and no placeholder
    /// there, and a caller pairing them in order silently mismatches every
    /// equation after the first.
    pub structural: bool,
}

/// Wrapper elements whose rendering is just their children, in order.
fn is_transparent(local: &[u8]) -> bool {
    matches!(local, b"oMath" | b"oMathPara" | b"r" | b"t" | b"num" | b"den" | b"e" | b"sup" | b"sub" | b"deg")
}

/// Property containers — they carry formatting, never content.
fn is_props(local: &[u8]) -> bool {
    local.len() > 2 && local.ends_with(b"Pr")
}

struct Frame {
    name: Vec<u8>,
    parts: Vec<String>,
}

/// Convert digits to Unicode subscripts so a subscripted base stays ONE
/// identifier — `A₆₆₃` is the absorbance at 663 nm, not `A` raised to anything.
/// The linear tokeniser already treats `₀`–`₉` as name characters.
fn subscriptify(s: &str) -> Option<String> {
    s.chars()
        .map(|c| {
            c.to_digit(10)
                .and_then(|d| char::from_u32('\u{2080}' as u32 + d))
        })
        .collect()
}

/// Bracket a sub-part, but ONLY when it is an expression in its own right.
///
/// **Word's tree does not always match the mathematical grouping.** In
/// `Corrected_Chapters_3_4_Jitesh_Agarwal.docx` the author typed
/// `237,000(0.04)²`, and Word stored `1+237,000(0.04` as one text run with the
/// closing `)` as the SUPERSCRIPT'S BASE. Wrapping that base gives
/// `1+237,000(0.04(())^(2))` — brackets that no longer pair with the ones in
/// the run beside them.
///
/// So a part that does not parse on its own is emitted raw and left to the
/// linear parser's precedence, which then applies `^(2)` to the bracketed group
/// exactly as the author intended. A part that IS an expression is bracketed,
/// because `a+b` raised to a power must not become `a + b^n`.
fn group(part: &str) -> String {
    if crate::equation::linear::parses_as_expression(part) {
        format!("({part})")
    } else {
        part.to_string()
    }
}

/// Join OMML siblings. Adjacent siblings are JUXTAPOSITION, which is
/// multiplication when both sides could be operands — `1+N` beside `e²` means
/// `1 + N·e²`. An explicit `×` is emitted there because the linear parser
/// deliberately refuses identifier-before-bracket as ambiguous, and here the
/// ambiguity is resolved by the markup.
fn join_siblings(parts: &[String]) -> String {
    let mut out = String::new();
    for p in parts {
        let needs_times = out
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == ')' || c == '_')
            && p.chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '(');
        if needs_times {
            out.push('\u{00D7}');
        }
        out.push_str(p);
    }
    out
}

fn render(name: &[u8], mut parts: Vec<String>) -> Result<String, OmmlError> {
    parts.retain(|p| !p.trim().is_empty());
    let joined = join_siblings(&parts);
    match name {
        // A run's own text is concatenation, never juxtaposition-multiplication:
        // `m:t` fragments are pieces of one string.
        b"t" | b"r" => Ok(parts.join("")),
        n if is_transparent(n) => Ok(joined),
        b"f" => match parts.len() {
            2 => Ok(format!("(({})/({}))", parts[0], parts[1])),
            _ => Err(OmmlError::Malformed("f".into())),
        },
        b"sSup" => match parts.len() {
            2 => Ok(format!("{}^({})", group(&parts[0]), parts[1])),
            _ => Err(OmmlError::Malformed("sSup".into())),
        },
        b"sSub" => match parts.len() {
            2 => match subscriptify(parts[1].trim()) {
                // Digits: fold into the name, as a manuscript would type it.
                Some(sub) => Ok(format!("{}{}", parts[0], sub)),
                // Anything else is a named index this reader will not invent a
                // spelling for.
                None => Err(OmmlError::Unsupported("sSub".into())),
            },
            _ => Err(OmmlError::Malformed("sSub".into())),
        },
        b"rad" => match parts.len() {
            // No degree element: a square root.
            1 => Ok(format!("sqrt({})", parts[0])),
            _ => Err(OmmlError::Unsupported("rad".into())),
        },
        b"d" => Ok(format!("({joined})")),
        other => Err(OmmlError::Unsupported(String::from_utf8_lossy(other).into_owned())),
    }
}

/// Read every `m:oMath` in a `word/document.xml`, with its paragraph index.
///
/// An equation this reader cannot represent is SKIPPED with its error
/// reported, never rendered partially.
pub fn equations_in_document_xml(xml: &str) -> (Vec<DocxEquation>, Vec<(usize, OmmlError)>) {
    let mut reader = NsReader::from_str(xml);
    let cfg = reader.config_mut();
    cfg.trim_text(false);
    cfg.check_end_names = false;

    let mut out = Vec::new();
    let mut errors = Vec::new();
    let mut paragraph = 0usize;
    let mut stack: Vec<Frame> = Vec::new();
    let mut props_depth = 0usize;
    let mut failed: Option<OmmlError> = None;
    let mut in_text = false;
    let mut structural = false;

    loop {
        match reader.read_resolved_event() {
            Ok((rs, Event::Start(e))) => {
                let local = e.local_name();
                let local = local.as_ref().to_vec();
                let is_math = matches!(rs, ResolveResult::Bound(ns) if ns.as_ref() == NS_M);
                if is_props(&local) {
                    props_depth += 1;
                    continue;
                }
                if props_depth > 0 {
                    continue;
                }
                if !is_math {
                    continue;
                }
                if local == b"t" {
                    in_text = true;
                }
                stack.push(Frame { name: local, parts: Vec::new() });
            }
            Ok((_, Event::Empty(e))) => {
                let local = e.local_name();
                if is_props(local.as_ref()) {
                    continue;
                }
            }
            Ok((_, Event::Text(t))) if in_text && props_depth == 0 => {
                if let Some(f) = stack.last_mut() {
                    f.parts.push(t.unescape().unwrap_or_default().to_string());
                }
            }
            Ok((rs, Event::End(e))) => {
                let local = e.local_name();
                let local = local.as_ref().to_vec();
                if is_props(&local) {
                    props_depth = props_depth.saturating_sub(1);
                    continue;
                }
                if props_depth > 0 {
                    continue;
                }
                if local == b"t" {
                    in_text = false;
                }
                let is_math = matches!(rs, ResolveResult::Bound(ns) if ns.as_ref() == NS_M);
                if !is_math {
                    if local == b"p" {
                        paragraph += 1;
                    }
                    continue;
                }
                let Some(frame) = stack.pop() else { continue };
                if !is_transparent(&frame.name) {
                    structural = true;
                }
                let rendered = match render(&frame.name, frame.parts) {
                    Ok(r) => r,
                    Err(err) => {
                        failed.get_or_insert(err);
                        String::new()
                    }
                };
                if frame.name == b"oMath" && stack.iter().all(|f| f.name != b"oMath") {
                    match failed.take() {
                        Some(err) => errors.push((paragraph, err)),
                        None => {
                            let linear = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
                            if !linear.trim().is_empty() {
                                out.push(DocxEquation { paragraph, linear, structural });
                            }
                        }
                    }
                    stack.clear();
                    structural = false;
                } else if let Some(parent) = stack.last_mut() {
                    parent.parts.push(rendered);
                }
            }
            Ok((_, Event::Eof)) => break,
            Ok(_) => {}
            Err(e) => {
                errors.push((paragraph, OmmlError::Xml(e.to_string())));
                break;
            }
        }
    }
    (out, errors)
}

/// Read every equation out of a `.docx`.
pub fn equations_in_docx(
    bytes: &[u8],
) -> Result<(Vec<DocxEquation>, Vec<(usize, OmmlError)>), GaplyError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|e| GaplyError::Validation(format!("not a valid docx (zip): {e}")))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|e| GaplyError::Validation(format!("docx missing document.xml: {e}")))?
        .read_to_string(&mut xml)?;
    Ok(equations_in_document_xml(&xml))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation::check::{check_equation, BoundValues, ClaimKind};
    use crate::equation::expr::Bindings;
    use crate::equation::linear::parse_equation;
    use crate::epistemic::EpistemicStatus;

    fn mrun(t: &str) -> String {
        format!("<m:r><w:rPr><w:rFonts w:ascii=\"Cambria Math\"/></w:rPr><m:t>{t}</m:t></m:r>")
    }
    fn doc(body: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?><w:document \
             xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
             xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">\
             <w:body>{body}</w:body></w:document>"
        )
    }

    /// Slovin's SYMBOLIC form, exactly as `Corrected_Chapters_3_4_Jitesh_Agarwal.docx`
    /// stores it: `n = N/(1 + Ne²)`.
    fn slovin_symbolic() -> String {
        format!(
            "<w:p><m:oMath>{n}<m:f><m:fPr><m:ctrlPr/></m:fPr><m:num>{num}</m:num>\
             <m:den>{den}<m:sSup><m:sSupPr><m:ctrlPr/></m:sSupPr><m:e>{e}</m:e>\
             <m:sup>{two}</m:sup></m:sSup></m:den></m:f></m:oMath></w:p>",
            n = mrun("n="),
            num = mrun("N"),
            den = mrun("1+N"),
            e = mrun("e"),
            two = mrun("2"),
        )
    }

    /// Slovin's NUMERIC substitution, the element whose flattening produced
    /// `237,0001` before §11 D155.
    fn slovin_numeric() -> String {
        format!(
            "<w:p><m:oMath>{n}<m:f><m:fPr><m:ctrlPr/></m:fPr><m:num>{a}</m:num>\
             <m:den>{b}<m:sSup><m:sSupPr><m:ctrlPr/></m:sSupPr><m:e>{c}</m:e>\
             <m:sup>{two}</m:sup></m:sSup></m:den></m:f>{eq}<m:f><m:num>{d}</m:num>\
             <m:den>{f}</m:den></m:f>{eq2}<m:f><m:num>{g}</m:num><m:den>{h}</m:den></m:f>\
             {tail}</m:oMath></w:p>",
            n = mrun("n="),
            a = mrun("237,000"),
            b = mrun("1+237,000(0.04"),
            c = mrun(")"),
            two = mrun("2"),
            eq = mrun("="),
            d = mrun("237,000"),
            f = mrun("1+379.2"),
            eq2 = mrun("="),
            g = mrun("237,000"),
            h = mrun("380.2"),
            tail = mrun("=623.36"),
        )
    }

    #[test]
    fn a_fraction_keeps_its_bar_and_a_superscript_its_power() {
        let (eqs, errs) = equations_in_document_xml(&doc(&slovin_symbolic()));
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(eqs.len(), 1);
        assert_eq!(eqs[0].linear, "n=((N)/(1+N×(e)^(2)))");
        assert!(eqs[0].structural, "a fraction is structure");

        // And it parses to the RIGHT tree — the denominator contains the power.
        let eq = parse_equation(&eqs[0].linear).expect("parses");
        let right = &eq.sides[1].expr;
        assert_eq!(right.variables(), vec!["N".to_string(), "e".into()]);
        let mut b = Bindings::new();
        b.insert("N".into(), crate::equation::rational::Rational::from_int(237_000));
        b.insert("e".into(), crate::equation::rational::Rational::parse_decimal("0.04").unwrap().0);
        assert_eq!(right.eval(&b).unwrap().to_decimal_string(2), "623.36");
    }

    /// **The §11 D155 fabrication, now impossible by construction.** The old
    /// text path produced `n=N1+Ne2`, whose denominator is wrong. The structured
    /// reader must not produce anything that canonicalises to that.
    #[test]
    fn the_structured_reading_is_not_the_flattened_one() {
        let (eqs, _) = equations_in_document_xml(&doc(&slovin_symbolic()));
        let good = parse_equation(&eqs[0].linear).unwrap().sides[1].expr.canonical();
        let flattened = parse_equation("n = N1+N×e2");
        // Either the flattened string does not parse to the same thing, or it
        // does not parse at all — both are fine; equality is not.
        if let Ok(f) = flattened {
            assert_ne!(good, f.sides[1].expr.canonical());
        }
        assert!(!eqs[0].linear.contains("N1+"), "{}", eqs[0].linear);
    }

    /// **THE NEGATIVE CONTROL, END TO END.** The numeric substitution is read
    /// from OMML, parsed, and checked — and every link holds, so the engine
    /// says nothing.
    #[test]
    fn the_slovin_substitution_reads_from_omml_and_produces_no_finding() {
        let (eqs, errs) = equations_in_document_xml(&doc(&slovin_numeric()));
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(eqs.len(), 1);

        let eq = parse_equation(&eqs[0].linear)
            .unwrap_or_else(|e| panic!("{:?}: {e}", eqs[0].linear));
        let claims = check_equation(&eq, &BoundValues::new());
        assert_eq!(claims.len(), 4, "{:?}", eqs[0].linear);
        assert!(matches!(claims[0].kind, ClaimKind::Definition { .. }));
        for c in &claims[1..] {
            assert_eq!(c.status, EpistemicStatus::Confirmed, "{}", c.message);
            assert!(!c.is_reportable());
        }
        // And the fabricated numeral is nowhere in the linear form.
        assert!(!eqs[0].linear.contains("237,0001"), "{}", eqs[0].linear);
    }

    #[test]
    fn a_subscript_becomes_part_of_the_name_not_a_power() {
        let body = format!(
            "<w:p><m:oMath>{}<m:sSub><m:e>{}</m:e><m:sub>{}</m:sub></m:sSub></m:oMath></w:p>",
            mrun("x=12.7×"),
            mrun("A"),
            mrun("663")
        );
        let (eqs, errs) = equations_in_document_xml(&doc(&body));
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(eqs[0].linear, "x=12.7×A₆₆₃");
        let eq = parse_equation(&eqs[0].linear).unwrap();
        assert_eq!(eq.sides[1].expr.variables(), vec!["A₆₆₃".to_string()]);
    }

    #[test]
    fn a_radical_becomes_a_square_root() {
        let body = format!(
            "<w:p><m:oMath>{}<m:rad><m:radPr/><m:e>{}</m:e></m:rad></m:oMath></w:p>",
            mrun("s="),
            mrun("2.25")
        );
        let (eqs, errs) = equations_in_document_xml(&doc(&body));
        assert!(errs.is_empty(), "{errs:?}");
        let eq = parse_equation(&eqs[0].linear).unwrap();
        assert_eq!(
            eq.sides[1].expr.eval(&Bindings::new()).unwrap().to_decimal_string(1),
            "1.5"
        );
    }

    /// **An unrepresentable construct is an ERROR, never a partial reading.**
    /// Dropping the `∑` from `∑xᵢ` leaves `xᵢ`, which is a different quantity
    /// and looks like a successful parse — §11 D155 with a different tag.
    #[test]
    fn an_nary_operator_is_refused_rather_than_silently_dropped() {
        let body = format!(
            "<w:p><m:oMath>{}<m:nary><m:naryPr/><m:e>{}</m:e></m:nary></m:oMath></w:p>",
            mrun("S="),
            mrun("x")
        );
        let (eqs, errs) = equations_in_document_xml(&doc(&body));
        assert!(eqs.is_empty(), "a partial reading escaped: {eqs:?}");
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0].1, OmmlError::Unsupported(ref e) if e == "nary"), "{errs:?}");
    }

    /// The two readers must agree on what counts as structure, or a caller
    /// splicing one stream into the other mismatches every equation after the
    /// first — measured, and it is how the Slovin chain went missing from the
    /// graph probe.
    #[test]
    fn a_bare_symbol_is_not_structural_and_docparse_leaves_it_inline() {
        let body = format!("<w:p><m:oMath>{}</m:oMath></w:p>", mrun("N"));
        let (eqs, _) = equations_in_document_xml(&doc(&body));
        assert_eq!(eqs.len(), 1);
        assert!(!eqs[0].structural, "a lone run carries no structure");

        // And `docparse` keeps it inline rather than emitting a placeholder,
        // which is the other half of the same rule.
        let mut buf = Vec::new();
        {
            use std::io::Write;
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(doc(&body).as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let text = crate::extract::docparse::parse_docx(&buf).unwrap();
        assert!(text.contains('N'), "{text:?}");
        assert!(
            !text.contains(crate::extract::docparse::EQUATION_PLACEHOLDER),
            "{text:?}"
        );
    }

    #[test]
    fn the_paragraph_index_locates_the_equation() {
        let body = format!(
            "<w:p><w:r><w:t>Intro</w:t></w:r></w:p><w:p><w:r><w:t>More</w:t></w:r></w:p>{}",
            slovin_symbolic()
        );
        let (eqs, _) = equations_in_document_xml(&doc(&body));
        assert_eq!(eqs[0].paragraph, 2, "third paragraph, zero-based");
    }
}
