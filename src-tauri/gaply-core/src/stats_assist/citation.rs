//! **`web_context`'s answer, read into cited sentences. A citation comes only
//! from a `url_citation` annotation, never from the prose.**
//!
//! The text is split with Phase 1's sentence rule
//! ([`crate::extract::sentence::sentences_in`]), so a boundary never falls
//! inside a number. Each annotation's range is converted to a byte range and
//! attached to every sentence it overlaps. A sentence with no citation is
//! dropped and counted. A sentence that is nothing but cited link text (the
//! `([bmj.com](https://…))` a model appends after a full stop) is not shown on
//! its own: its citations go to the sentence before it.
//!
//! **The index unit is not documented** (Step 1, section D: only
//! "character"). decisions.md settles it: a range is read both as Unicode
//! code points and as UTF-16 code units, the end exclusive, and the citation
//! is kept only if, under one of them, the text between the indexes contains
//! the citation's URL, its domain or its title. Otherwise it is dropped and
//! counted. Which unit matched is recorded on the source, so the live check
//! can say which one the API uses.
//!
//! A citation is also dropped when its url is not `https`, carries userinfo
//! (`https://bmj.com@evil.io`), or its host is not an allowed domain or a
//! subdomain of one on a dot boundary (`bmj.com.evil.io` and `evilbmj.com`
//! are both refused).

use super::response::{ModelOutput, ModelText, RawCitation};
use crate::extract::sentence::sentences_in;

/// Shown when nothing cited survives.
pub const NO_CITED_SOURCES: &str = "No cited sources found";

/// When the request was answered, from the caller's clock: the core has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetrievedAt {
    pub unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexUnit {
    CodePoints,
    Utf16,
}

/// One source, exactly as the annotation gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedSource {
    pub url: String,
    pub title: String,
    /// The url's host, lower-cased.
    pub domain: String,
    /// The unit under which the range named this source.
    pub unit: IndexUnit,
}

/// One shown item: a sentence of model text and its sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceItem {
    pub sentence: ModelText,
    pub sources: Vec<CitedSource>,
    pub retrieved_at: RetrievedAt,
    pub query_sent: String,
}

/// Why a citation was dropped. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationDrop {
    /// A field is missing, or an index is negative.
    Malformed,
    NotHttps,
    /// Userinfo, a port, an empty or odd host.
    BadHost,
    DomainNotAllowed,
    /// Outside the text, inside a character, or end before start, in both
    /// units.
    RangeOutsideText,
    /// In range, but the text there names neither the url, the domain nor
    /// the title, in either unit.
    RangeDoesNotNameSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEvidence {
    pub items: Vec<EvidenceItem>,
    /// Sentences dropped because nothing cited them.
    pub uncited_sentences: usize,
    pub dropped_citations: Vec<CitationDrop>,
}

impl ContextEvidence {
    /// The message to show instead of items, if there are none.
    pub fn empty_message(&self) -> Option<&'static str> {
        self.items.is_empty().then_some(NO_CITED_SOURCES)
    }
}

/// The host of an https url, lower-cased, or why it is not acceptable.
pub fn https_host(url: &str) -> Result<String, CitationDrop> {
    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("https://") else { return Err(CitationDrop::NotHttps) };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') || authority.contains(':') {
        return Err(CitationDrop::BadHost);
    }
    let host = authority.strip_suffix('.').unwrap_or(authority);
    let ok = !host.is_empty()
        && !host.starts_with('.')
        && !host.contains("..")
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if !ok {
        return Err(CitationDrop::BadHost);
    }
    Ok(host.to_string())
}

/// `host` is `domain` or a subdomain of it on a dot boundary.
pub fn host_is_allowed(host: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|d| host == d || host.strip_suffix(d.as_str()).is_some_and(|pre| pre.ends_with('.')))
}

/// A unit index as a byte offset into `text`, if it falls on a character
/// boundary inside or at the end of the text.
fn byte_offset(text: &str, index: usize, unit: IndexUnit) -> Option<usize> {
    let mut count = 0usize;
    for (b, c) in text.char_indices() {
        if count == index {
            return Some(b);
        }
        count += match unit {
            IndexUnit::CodePoints => 1,
            IndexUnit::Utf16 => c.len_utf16(),
        };
        if count > index {
            // Inside a surrogate pair.
            return None;
        }
    }
    (count == index).then_some(text.len())
}

struct Placed {
    start: usize,
    end: usize,
    source: CitedSource,
}

fn place(text: &str, c: &RawCitation, allowed: &[String]) -> Result<Placed, CitationDrop> {
    let (Some(url), Some(title), Some(s), Some(e)) = (&c.url, &c.title, c.start_index, c.end_index) else {
        return Err(CitationDrop::Malformed);
    };
    let (Ok(s), Ok(e)) = (usize::try_from(s), usize::try_from(e)) else { return Err(CitationDrop::Malformed) };
    let host = https_host(url)?;
    if !host_is_allowed(&host, allowed) {
        return Err(CitationDrop::DomainNotAllowed);
    }
    let mut in_range = false;
    for unit in [IndexUnit::CodePoints, IndexUnit::Utf16] {
        let (Some(bs), Some(be)) = (byte_offset(text, s, unit), byte_offset(text, e, unit)) else { continue };
        if bs > be {
            continue;
        }
        in_range = true;
        let span = text[bs..be].to_lowercase();
        let names = span.contains(&url.to_lowercase())
            || span.contains(&host)
            || (!title.trim().is_empty() && span.contains(&title.trim().to_lowercase()));
        if names {
            return Ok(Placed {
                start: bs,
                end: be,
                source: CitedSource { url: url.clone(), title: title.clone(), domain: host, unit },
            });
        }
    }
    Err(if in_range { CitationDrop::RangeDoesNotNameSource } else { CitationDrop::RangeOutsideText })
}

/// Read a `web_context` answer.
pub fn parse_web_context(out: &ModelOutput, query_sent: &str, retrieved_at: RetrievedAt, allowed: &[String]) -> ContextEvidence {
    let text = out.text.as_str();
    let mut dropped = Vec::new();
    let mut placed = Vec::new();
    for c in &out.citations {
        match place(text, c, allowed) {
            Ok(p) => placed.push(p),
            Err(d) => dropped.push(d),
        }
    }

    // Sentences with their byte ranges in `text`.
    let base = text.as_ptr() as usize;
    let sentences: Vec<(usize, usize, &str)> = sentences_in(text)
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| {
            let start = s.as_ptr() as usize - base;
            (start, start + s.len(), s)
        })
        .collect();

    // Each citation's sentences: those its range overlaps. A citation that
    // LEADS its sentence (nothing alphanumeric before it there) was written
    // after the previous sentence's full stop, so it belongs to that one.
    let mut sources: Vec<Vec<CitedSource>> = vec![Vec::new(); sentences.len()];
    for p in &placed {
        let mut targets: Vec<usize> =
            sentences.iter().enumerate().filter(|(_, &(s0, s1, _))| p.start < s1 && p.end > s0).map(|(i, _)| i).collect();
        if targets.is_empty() {
            // On whitespace between sentences or after the last one: the
            // sentence before it.
            targets.extend(sentences.iter().rposition(|&(_, s1, _)| s1 <= p.start));
        }
        if let Some(&first) = targets.first() {
            let s0 = sentences[first].0;
            let leads = p.start >= s0 && !text[s0..p.start].chars().any(char::is_alphanumeric);
            if leads && first > 0 {
                targets[0] = first - 1;
            }
        }
        for t in targets {
            if !sources[t].iter().any(|x| x.url == p.source.url) {
                sources[t].push(p.source.clone());
            }
        }
    }

    let mut items = Vec::new();
    let mut uncited = 0;
    for (i, &(s0, _, sentence)) in sentences.iter().enumerate() {
        if !sources[i].is_empty() {
            items.push(EvidenceItem {
                sentence: ModelText::new(sentence),
                sources: std::mem::take(&mut sources[i]),
                retrieved_at,
                query_sent: query_sent.to_string(),
            });
        } else if !here_is_link_only(sentence, s0, &placed) {
            // Link text whose citation went to the sentence before is not a
            // sentence of its own; anything else uncited is dropped, counted.
            uncited += 1;
        }
    }
    ContextEvidence { items, uncited_sentences: uncited, dropped_citations: dropped }
}

/// The sentence holds no letter outside cited ranges.
fn here_is_link_only(sentence: &str, s0: usize, placed: &[Placed]) -> bool {
    let mut any_cited = false;
    for (off, ch) in sentence.char_indices() {
        let at = s0 + off;
        let cited = placed.iter().any(|p| p.start <= at && at < p.end);
        any_cited |= cited;
        if ch.is_alphabetic() && !cited {
            return false;
        }
    }
    any_cited
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        ["bmj.com", "ncbi.nlm.nih.gov"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hosts_are_checked_on_a_dot_boundary() {
        let a = allowed();
        assert!(host_is_allowed("bmj.com", &a));
        assert!(host_is_allowed("www.bmj.com", &a));
        assert!(host_is_allowed("pmc.ncbi.nlm.nih.gov", &a));
        assert!(!host_is_allowed("bmj.com.evil.io", &a));
        assert!(!host_is_allowed("evilbmj.com", &a));
        assert!(!host_is_allowed("nih.gov", &a));
        assert_eq!(https_host("https://www.BMJ.com/content/1"), Ok("www.bmj.com".to_string()));
        assert_eq!(https_host("http://bmj.com/"), Err(CitationDrop::NotHttps));
        assert_eq!(https_host("https://bmj.com@evil.io/"), Err(CitationDrop::BadHost));
        assert_eq!(https_host("https://bmj.com:8443/"), Err(CitationDrop::BadHost));
        assert_eq!(https_host("javascript:alert(1)"), Err(CitationDrop::NotHttps));
    }

    #[test]
    fn offsets_count_code_points_and_utf16_units() {
        let t = "é—😀x";
        assert_eq!(byte_offset(t, 0, IndexUnit::CodePoints), Some(0));
        assert_eq!(byte_offset(t, 2, IndexUnit::CodePoints), Some(5));
        assert_eq!(byte_offset(t, 3, IndexUnit::CodePoints), Some(9));
        assert_eq!(byte_offset(t, 4, IndexUnit::CodePoints), Some(10));
        assert_eq!(byte_offset(t, 5, IndexUnit::CodePoints), None);
        // The emoji is two UTF-16 units: index 3 falls inside it.
        assert_eq!(byte_offset(t, 3, IndexUnit::Utf16), None);
        assert_eq!(byte_offset(t, 4, IndexUnit::Utf16), Some(9));
        assert_eq!(byte_offset(t, 5, IndexUnit::Utf16), Some(10));
    }
}
