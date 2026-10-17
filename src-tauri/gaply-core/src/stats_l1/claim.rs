//! **Claim identity: a Level 1 claim is a Level 0 result with a sentence
//! ordinal and a revision hash.**
//!
//! A claim's location is its section, paragraph and sentence ordinals, never
//! a byte offset. Level 0's [`Location`] carries the first two; the sentence
//! is computed HERE with Phase 1's splitter
//! ([`crate::extract::sentence::sentences_in`]) over the paragraph, so no
//! Level 0 type changes.
//!
//! The revision hash covers the claim's normalised printed strings (each
//! [`ReportedNumber::canonical`]: relation and exact interval, no printed
//! text) and that location. A claim the user adds or edits gets a new revision
//! and goes through [`ReportedNumber`] exactly as an extracted one does.
//!
//! Not `Serialize`: `raw` and the sentence are manuscript text.

use crate::extract::sentence::sentences_in;
use crate::extract::{ExtractionResult, Location, SectionKind};
use crate::stats_l0::number::{NumberNotRead, Relation, ReportedNumber};
use crate::stats_l0::result::{Family, PValue, ReportedResult, ResultReading, Source};

use super::dataset::sha256_hex;

/// A claim's position in the run's list, from 0. Stable for the run: editing
/// a claim keeps its id and changes its revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClaimId(pub usize);

/// Ordinals only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClaimLocation {
    pub section_index: usize,
    pub section: SectionKind,
    pub paragraph: usize,
    pub sentence: usize,
}

impl ClaimLocation {
    fn canonical(&self) -> String {
        format!("{}/{}/{}", self.section_index, self.paragraph, self.sentence)
    }
}

/// SHA-256, hex, of the claim's normalised strings and location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionHash(String);

impl RevisionHash {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// A revision that no claim has: for tests of staleness only.
    #[cfg(test)]
    pub(super) fn synthetic(seed: &[u8]) -> RevisionHash {
        RevisionHash(sha256_hex(seed))
    }
}

/// Where the claim came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOrigin {
    /// Read from the manuscript by Level 0's reader.
    Extracted,
    UserAdded,
    /// Edited by the user; `from` is the revision it replaced.
    UserEdited { from: RevisionHash },
}

/// One Level 1 claim.
#[derive(Clone, PartialEq)]
pub struct L1Claim {
    pub id: ClaimId,
    pub result: ReportedResult,
    pub location: ClaimLocation,
    /// The sentence holding the unit, for the local proposer. Manuscript text;
    /// never leaves the device.
    pub sentence: String,
    pub revision: RevisionHash,
    pub origin: ClaimOrigin,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for L1Claim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("L1Claim").field("id", &self.id).field("family", &self.result.family).field("location", &self.location).field("sentence_chars", &self.sentence.chars().count()).field("revision", &self.revision).field("origin", &self.origin).finish_non_exhaustive()
    }
}

/// Why a result could not be given a sentence ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotLocated {
    /// The result's location has no section index (a legacy `Location`).
    AmbiguousLocation,
    /// No section or paragraph at the location.
    NoParagraph,
    /// The printed unit is not in the paragraph.
    UnitNotInParagraph,
}

/// The sentence ordinal of the `occurrence`-th appearance (from 0) of `raw`
/// in the paragraph at `location`, and the sentence itself.
fn sentence_of(ex: &ExtractionResult, location: &Location, raw: &str, occurrence: usize) -> Result<(usize, String), NotLocated> {
    let si = location.section_index.ok_or(NotLocated::AmbiguousLocation)?;
    let text = ex
        .sections
        .get(si)
        .and_then(|s| s.paragraphs.get(location.paragraph))
        .ok_or(NotLocated::NoParagraph)?;
    let at = text.match_indices(raw).nth(occurrence).map(|(i, _)| i).ok_or(NotLocated::UnitNotInParagraph)?;
    let base = text.as_ptr() as usize;
    let sentences = sentences_in(text);
    // Each slice is a subslice of `text`, so its offset is its pointer's.
    let idx = sentences
        .iter()
        .position(|s| {
            let start = s.as_ptr() as usize - base;
            at >= start && at < start + s.len()
        })
        .or_else(|| {
            // The unit begins in the whitespace between two trimmed sentences:
            // it belongs to the next one.
            sentences.iter().position(|s| s.as_ptr() as usize - base > at)
        })
        .ok_or(NotLocated::UnitNotInParagraph)?;
    Ok((idx, sentences[idx].to_string()))
}

/// The revision of `result` at `location`: what [`L1Claim::from_result`] and
/// [`L1Claim::user_entered`] compute, for a caller that changed a result's
/// carried-over fields after building it (an edit, `run_state`).
pub(super) fn revision_of(result: &ReportedResult, location: &ClaimLocation) -> RevisionHash {
    revision(result, location)
}

fn revision(result: &ReportedResult, location: &ClaimLocation) -> RevisionHash {
    let mut s = String::new();
    s.push_str(&format!("{:?}|", result.family));
    s.push_str(&result.statistic.canonical());
    s.push('|');
    for d in &result.df {
        s.push_str(&d.canonical());
        s.push(';');
    }
    s.push('|');
    s.push_str(&result.n.as_ref().map(ReportedNumber::canonical).unwrap_or_default());
    s.push('|');
    match &result.p {
        PValue::Reported(p) => s.push_str(&p.canonical()),
        PValue::NotSignificant => s.push_str("ns"),
    }
    s.push('|');
    if let Some(e) = &result.effect_size {
        s.push_str(&e.name.to_lowercase());
        s.push('=');
        s.push_str(&e.value.canonical());
    }
    s.push('|');
    if let Some(ci) = &result.ci {
        s.push_str(&format!("{};{};{}", ci.level.canonical(), ci.low.canonical(), ci.high.canonical()));
    }
    s.push('|');
    s.push_str(&location.canonical());
    RevisionHash(sha256_hex(s.as_bytes()))
}

impl L1Claim {
    /// A claim from a result Level 0 read, located in its document.
    /// `occurrence` is which appearance of the same printed unit in the same
    /// paragraph this result is (0 for the first).
    pub fn from_result(ex: &ExtractionResult, id: ClaimId, result: ReportedResult, occurrence: usize) -> Result<L1Claim, NotLocated> {
        let (sentence_idx, sentence) = sentence_of(ex, &result.location, &result.raw, occurrence)?;
        let location = ClaimLocation {
            section_index: result.location.section_index.ok_or(NotLocated::AmbiguousLocation)?,
            section: result.location.section,
            paragraph: result.location.paragraph,
            sentence: sentence_idx,
        };
        let revision = revision(&result, &location);
        Ok(L1Claim { id, result, location, sentence, revision, origin: ClaimOrigin::Extracted })
    }

    /// A claim the user typed. Every number goes through [`ReportedNumber`].
    pub fn user_entered(id: ClaimId, claim: &UserClaim<'_>, origin: ClaimOrigin) -> Result<L1Claim, NumberNotRead> {
        let statistic = ReportedNumber::measured(claim.statistic)?;
        let df = claim
            .df
            .iter()
            .map(|d| if d.contains('.') { ReportedNumber::measured(d) } else { ReportedNumber::count(d) })
            .collect::<Result<Vec<_>, _>>()?;
        let n = claim.n.map(ReportedNumber::count).transpose()?;
        let p = match claim.p {
            UserP::Reported { raw, relation } => PValue::Reported(ReportedNumber::p_value(raw, relation)?),
            UserP::NotSignificant => PValue::NotSignificant,
        };
        let raw = {
            let mut parts = vec![claim.statistic.trim().to_string()];
            parts.extend(claim.df.iter().map(|d| d.trim().to_string()));
            parts.extend(claim.n.map(|n| n.trim().to_string()));
            match claim.p {
                UserP::Reported { raw, .. } => parts.push(raw.trim().to_string()),
                UserP::NotSignificant => parts.push("ns".into()),
            }
            parts.join(", ")
        };
        let result = ReportedResult {
            id: id.0,
            location: Location::in_section(claim.location.section, claim.location.section_index, claim.location.paragraph),
            raw,
            family: claim.family,
            statistic,
            df,
            n,
            p,
            effect_size: None,
            ci: None,
            sentence_says_partial: false,
            source: Source::Unknown,
        };
        let revision = revision(&result, &claim.location);
        Ok(L1Claim { id, result, location: claim.location, sentence: claim.sentence.unwrap_or("").to_string(), revision, origin })
    }
}

/// A claim as the user types it: printed strings, never floats.
#[derive(Clone)]
pub struct UserClaim<'a> {
    pub family: Family,
    pub statistic: &'a str,
    pub df: Vec<&'a str>,
    pub n: Option<&'a str>,
    pub p: UserP<'a>,
    pub location: ClaimLocation,
    /// The sentence the user says the claim is in, if they give one.
    pub sentence: Option<&'a str>,
}

/// Counts only, never the text (part C follow-up, item 7).
impl std::fmt::Debug for UserClaim<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserClaim").field("family", &self.family).field("df", &self.df.len()).field("n", &self.n.is_some()).field("location", &self.location).field("sentence_chars", &self.sentence.map(|t| t.chars().count())).finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UserP<'a> {
    Reported { raw: &'a str, relation: Relation },
    NotSignificant,
}

/// Every result of a reading as a claim, in document order, each located.
/// The second value counts results that could not be located; those are not
/// claims and are reported as a count.
pub fn claims_from_reading(ex: &ExtractionResult, reading: &ResultReading) -> (Vec<L1Claim>, usize) {
    let mut claims = Vec::with_capacity(reading.results.len());
    let mut not_located = 0;
    for r in &reading.results {
        let occurrence = reading.results[..r.id.min(reading.results.len())]
            .iter()
            .filter(|e| e.location == r.location && e.raw == r.raw)
            .count();
        match L1Claim::from_result(ex, ClaimId(claims.len()), r.clone(), occurrence) {
            Ok(c) => claims.push(c),
            Err(_) => not_located += 1,
        }
    }
    (claims, not_located)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::extract_from_text;
    use crate::stats_l0::result::{read_results, TableStructure};

    const DOC: &str = "A Title\n\nMethods\nWe used t-tests.\n\nResults\nScores rose, t(28) = 2.10, p = .023. Accuracy was unchanged, t(28) = 0.40, p = .69. In the second study, the same, t(28) = 2.10, p = .023.\n";

    fn claims() -> (ExtractionResult, Vec<L1Claim>, usize) {
        let ex = extract_from_text(DOC);
        let reading = read_results(&ex, TableStructure::NotAvailable);
        let (c, n) = claims_from_reading(&ex, &reading);
        (ex, c, n)
    }

    #[test]
    fn each_claim_gets_its_sentence_ordinal_and_a_repeated_unit_its_own() {
        let (_, c, not_located) = claims();
        assert_eq!(not_located, 0);
        assert_eq!(c.len(), 3, "{c:?}");
        assert_eq!(c.iter().map(|c| c.location.sentence).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(c[0].sentence.starts_with("Scores rose"));
        assert!(c[2].sentence.starts_with("In the second study"));
        assert_eq!(c[0].location.section, SectionKind::Results);
        // Same printed unit, different sentence: different revisions.
        assert_eq!(c[0].result.raw, c[2].result.raw);
        assert_ne!(c[0].revision, c[2].revision);
        assert!(c.iter().all(|c| c.origin == ClaimOrigin::Extracted));
    }

    #[test]
    fn the_revision_covers_the_printed_strings_and_the_location_and_nothing_else() {
        let (_, c, _) = claims();
        let base = &c[0];
        // Re-deriving from the same result and location is stable.
        assert_eq!(revision(&base.result, &base.location), base.revision);
        // A different sentence ordinal is a different revision.
        let moved = ClaimLocation { sentence: 5, ..base.location };
        assert_ne!(revision(&base.result, &moved), base.revision);
        // A different printed p is a different revision.
        let mut edited = base.result.clone();
        edited.p = PValue::Reported(ReportedNumber::p_value(".024", Relation::Eq).unwrap());
        assert_ne!(revision(&edited, &base.location), base.revision);
        // The raw text and the source are NOT in it: the normalised strings are.
        let mut retyped = base.result.clone();
        retyped.raw = "something else".into();
        retyped.source = Source::TableCell;
        assert_eq!(revision(&retyped, &base.location), base.revision);
    }

    #[test]
    fn a_user_claim_goes_through_reported_number_and_a_bad_number_is_refused() {
        let loc = ClaimLocation { section_index: 1, section: SectionKind::Results, paragraph: 0, sentence: 0 };
        let ok = L1Claim::user_entered(
            ClaimId(0),
            &UserClaim {
                family: Family::T,
                statistic: "2.10",
                df: vec!["28"],
                n: None,
                p: UserP::Reported { raw: ".023", relation: Relation::Eq },
                location: loc,
                sentence: None,
            },
            ClaimOrigin::UserAdded,
        )
        .unwrap();
        assert_eq!(ok.result.df[0].canonical(), ReportedNumber::count("28").unwrap().canonical());
        assert_eq!(ok.origin, ClaimOrigin::UserAdded);
        // The same numbers typed give the same revision as the extracted claim
        // at the same location: identity is the strings and the place.
        let (_, c, _) = claims();
        let same = L1Claim::user_entered(
            ClaimId(0),
            &UserClaim {
                family: Family::T,
                statistic: "2.10",
                df: vec!["28"],
                n: None,
                p: UserP::Reported { raw: ".023", relation: Relation::Eq },
                location: c[0].location,
                sentence: None,
            },
            ClaimOrigin::UserAdded,
        )
        .unwrap();
        assert_eq!(same.revision, c[0].revision);

        let bad = L1Claim::user_entered(
            ClaimId(1),
            &UserClaim {
                family: Family::T,
                statistic: "2,10",
                df: vec!["28"],
                n: None,
                p: UserP::NotSignificant,
                location: loc,
                sentence: None,
            },
            ClaimOrigin::UserAdded,
        );
        assert_eq!(bad.unwrap_err(), NumberNotRead::Comma);
        // A fractional df is measured, a sign on a count is refused.
        let welch = UserClaim {
            family: Family::T,
            statistic: "2.10",
            df: vec!["27.4"],
            n: Some("-30"),
            p: UserP::NotSignificant,
            location: loc,
            sentence: None,
        };
        assert_eq!(L1Claim::user_entered(ClaimId(2), &welch, ClaimOrigin::UserAdded).unwrap_err(), NumberNotRead::NegativeCount);
    }

    #[test]
    fn a_unit_missing_from_its_paragraph_is_not_located_and_counted() {
        let (ex, c, _) = claims();
        let mut r = c[0].result.clone();
        r.raw = "t(99) = 9.99, p = .999".into();
        assert_eq!(L1Claim::from_result(&ex, ClaimId(9), r.clone(), 0).unwrap_err(), NotLocated::UnitNotInParagraph);
        r.location = Location::by_kind(SectionKind::Results, 0);
        assert_eq!(L1Claim::from_result(&ex, ClaimId(9), r, 0).unwrap_err(), NotLocated::AmbiguousLocation);
    }
}
