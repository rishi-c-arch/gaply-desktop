//! **Journal guideline discovery: FETCH-AND-CLASSIFY, not match-a-lexicon.**
//!
//! # The inversion, and the evidence for it (§3.4 v6, §11 D159, D160)
//!
//! §3.4 and Prompt 5 both specify discovery as *"follow only links whose anchor
//! text or path names a guideline topic"*. That rule has a failure mode with no
//! symptom: a crawl that skipped the one page with the numbers on it looks
//! exactly like a crawl that found everything.
//!
//! **The evidence is Nature Medicine.** Every extractable requirement it
//! publishes — main-text limits of 4,000 / 2,000 / 1,000 words, abstract 150,
//! references 10, display items 2, each already bound to an article type — is on
//! ONE page: `https://www.nature.com/nm/content`. Its path is `/nm/content` and
//! its anchor text is *"Content types"*. Neither names a guideline topic. A
//! 30-term lexicon written by reading PLOS does not contain the phrase, and
//! PLOS has no equivalent page to learn it from.
//!
//! The deciding argument is not that a lexicon is incomplete — every heuristic
//! is — but that **its completeness is unknowable for a publisher you have not
//! read, and its failure is silent.** So the lexicon is demoted to CRAWL ORDER:
//! it decides what to fetch first, never what to fetch at all. What admits a
//! page is [`crate::guidelines::classify_page`], which reads the page itself.
//!
//! **The standing evidence is [`CrawlOutcome::lexicon_misses`]** — pages
//! classified `Guideline` that no lexicon term would have admitted. It is
//! reported for every crawl, and every one of them is a page the old rule would
//! have dropped without saying so.
//!
//! # What makes this affordable
//!
//! Fetch-and-classify costs one request per candidate, so the bounds are the
//! whole difference between crawling a journal and crawling a publisher. Both
//! live in `config/journal-crawl.json` and neither has a default in code: a
//! missing or malformed config is an error, because a budget that silently
//! falls back to a compiled-in number is a budget nobody set.
//!
//! # An interstitial's links are the bot-wall's, not the journal's
//!
//! [`PageVerdict::Interstitial`] means the request never reached the journal.
//! Its `<a>` elements belong to the challenge page, and following them spends
//! the budget on a vendor's error furniture. They are not enqueued.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use url::Url;

use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{HttpFetcher, HttpRequest};
use gaply_core::GaplyError;

use crate::guidelines::{classify_page, html_title, html_to_text_public, PageVerdict};

/// Cost bounds. Deserialised from `config/journal-crawl.json`; **no `Default`
/// impl, deliberately** — see the module header.
#[derive(Debug, Clone, Deserialize)]
pub struct CrawlBudget {
    /// Hard cap on pages FETCHED. A crawl that ends here has not covered the
    /// journal, and [`CrawlOutcome::stopped_by`] says so.
    pub max_pages: usize,
    /// 0 = the entry page alone.
    pub max_depth: usize,
    /// Publisher-wide guideline hosts, allowlisted rather than inferred.
    #[serde(default)]
    pub author_services_hosts: Vec<String>,
    /// Hosts that serve MANY journals, and how many leading path segments
    /// identify one. Absent host = the host is the journal.
    #[serde(default)]
    pub journal_path_segments: std::collections::BTreeMap<String, usize>,
    /// Half-second ticks the crawl waits for a rate-limit token before
    /// recording that it could not proceed. In config because it is a cost
    /// bound like the others.
    #[serde(default)]
    pub max_rate_wait_ticks: u32,
    /// The journals §3.4 names, as (key, display name, crawl entry).
    ///
    /// **Config, not code, for the same reason as the bounds** — and because
    /// the list previously lived in `examples/journal_crawl_probe.rs`, an
    /// example binary the app cannot read, so the picker had no way to offer
    /// "the ten profiled journals" the architecture document describes.
    #[serde(default)]
    pub profiled_journals: Vec<ProfiledJournal>,
}

/// One entry of [`CrawlBudget::profiled_journals`].
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ProfiledJournal {
    /// Primary key of every `journal_*` table. Changing it orphans that
    /// journal's stored rows.
    pub key: String,
    pub name: String,
    /// Where a crawl starts. Not a page to scrape — an entry point (§3.4 v6).
    pub entry: String,
}

impl CrawlBudget {
    pub fn from_json(s: &str) -> Result<CrawlBudget, GaplyError> {
        let b: CrawlBudget = serde_json::from_str(s)
            .map_err(|e| GaplyError::Config(format!("journal-crawl.json is not valid: {e}")))?;
        if b.max_pages == 0 {
            return Err(GaplyError::Config("max_pages must be > 0".into()));
        }
        Ok(b)
    }
}

/// Why the crawl ended. **Only `FrontierExhausted` supports a coverage claim.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoppedBy {
    /// Every reachable candidate within the depth bound was fetched.
    FrontierExhausted,
    /// The page budget ran out with candidates still queued. The journal is
    /// NOT covered and any count derived from this crawl is a lower bound.
    Budget,
    /// **The entry URL never resolved to a page of the journal's.**
    ///
    /// A third state, because the first two could not tell it apart from
    /// success: Nature Communications' entry 303s to
    /// `?error=cookies_not_supported`, the crawl fetched one page, found no
    /// links, and reported `FrontierExhausted` — the same verdict as a journal
    /// whose entire guideline tree had been read. *"Nothing to crawl"* and
    /// *"crawled everything"* are opposite outcomes and were one word.
    ///
    /// It is the [`PageVerdict::Interstitial`] / [`PageVerdict::Navigation`]
    /// distinction one level up: did the request reach the thing at all.
    EntryUnreachable { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawledPage {
    pub url: String,
    pub anchor: String,
    pub depth: usize,
    pub verdict: &'static str,
    pub chars: usize,
    pub obligations: usize,
    pub requirements: usize,
    /// Would the lexicon have admitted this link? `false` on a `Guideline` page
    /// is a page the old discovery rule would have silently skipped.
    pub lexicon_hit: bool,
    /// **The page body, retained for `Guideline` pages only — §11 D174.**
    ///
    /// # Why the crawl must hand this on rather than let a caller re-ask
    ///
    /// Every consumer needs `extract_requirements`, which reads BLOCKS, and
    /// blocks are built from the raw HTML. The crawl has that HTML in hand and
    /// used to drop it, so `journal_stage2_build` re-fetched every guideline URL
    /// from a host it had just pulled 120 pages from.
    ///
    /// **That second fetch was REDUNDANT before it was HARMFUL, which is why
    /// nobody noticed.** On a host that answers every request it is pure waste:
    /// the same bytes twice, and identical results. It only becomes visible when
    /// a host throttles the second pass — BMJ returned a 12,361-character stub
    /// for seven different URLs — at which point the extractor sees nothing and
    /// the journal reads as having no requirements. A redundancy that costs only
    /// time draws no attention until the day it costs data.
    ///
    /// `None` for navigation, interstitial and duplicate pages: nothing extracts
    /// from those, and keeping their bodies would multiply the memory a crawl
    /// holds for no reader.
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlOutcome {
    pub entry: String,
    pub host: String,
    pub fetched: usize,
    pub guideline: usize,
    pub navigation: usize,
    pub interstitial: usize,
    pub errors: usize,
    /// **The standing evidence for the inversion.** Pages classified
    /// `Guideline` whose link no lexicon term would have matched.
    pub lexicon_misses: usize,
    /// Candidates still queued when the crawl ended.
    pub unvisited: usize,
    /// Pages fetched whose CONTENT was byte-identical to a page already seen
    /// under a different URL. Counted, not hidden: they cost a request.
    pub duplicate_content: usize,
    /// The crawl gave up waiting for a rate-limit token. Like `Budget`, this
    /// means the journal is not covered.
    pub rate_limited: bool,
    pub stopped_by: StoppedBy,
    pub pages: Vec<CrawledPage>,
}

/// **Is this URL a document that could CARRY guidance at all?** §11 D185.
///
/// Two kinds reached the extractor in the re-crawl and produced requirements
/// from documents that are not guidance — D163's shape, where a price list
/// yielded `word_limit = 12000`:
///
/// * **An article LISTING.** `bmj.com/content/by/section/Research%20Methods%20%26
///   Reporting` is an issue index, and the extractor read three paper TITLES as
///   requirements: *"DOI: 10.1136/bmj-2025-088561 CONSORT-C 2026 explanation and
///   elaboration"* became "this journal requires CONSORT". A page listing papers
///   ABOUT reporting guidelines is the single most likely page to be misread
///   this way, which is why it is excluded by shape rather than fought in the
///   extractor.
/// * **A binary attachment.** `BMJ%20Author%20Licence%20March%202013.doc`
///   yielded `word_limit = 300` from *"The right to use selected figures and
///   tables (of which the author or his employer owns…)"* — a copyright licence
///   parsed as prose.
///
/// **The rule keys on the URL PATH, not on content**, deliberately: the content
/// of an article listing looks like guidance (it is full of the vocabulary), and
/// that is exactly why content-based classification admitted it. `/content/by/`
/// is a listing route; an extension in `DOCUMENT_EXTENSIONS` is a file to
/// download rather than a page to read.
///
/// **Named limit: the extension is checked on the PATH only.** A file served
/// through a query parameter — `…/s/file?id=x/guidelines.pdf` — slips through.
/// That shape was NOT observed producing a bad requirement in any of the ten
/// journals; it is recorded here rather than covered, because a rule extended
/// from a case nobody measured is how the lexicon mistakes in this file started.
///
/// It is a REFUSAL, so its failure direction is a journal losing a real
/// requirements page whose URL happens to match. `/content/by/` is specific
/// enough that no guidance page observed across ten journals uses it, and the
/// extensions are not HTML at all.
fn carries_guidance(u: &Url) -> bool {
    let path = u.path().to_lowercase();
    if path.contains("/content/by/") {
        return false;
    }
    const DOCUMENT_EXTENSIONS: &[&str] = &[".doc", ".docx", ".pdf", ".rtf", ".xls", ".xlsx", ".zip"];
    !DOCUMENT_EXTENSIONS.iter().any(|e| path.ends_with(e))
}

/// Topic terms. **Crawl ORDER only.** Adding a term changes what is fetched
/// first; it can never change what is admitted, so a term missing here costs
/// latency rather than coverage — which is the entire point of the inversion.
const LEXICON: &[&str] = &[
    "author", "submission", "guideline", "instruction", "format", "style", "ethic",
    "data availability", "data-availability", "reporting", "standard", "checklist",
    "policy", "policies", "conflict", "competing interest", "preprint", "licens",
    "article type", "article-type", "manuscript", "peer review", "trial registration",
    "authorship", "figure", "reference", "word limit", "scope", "publication",
    "open access", "copyright", "permission", "supporting information", "for-authors",
];

fn hit_of(url: &str, anchor: &str) -> bool {
    lexicon_hit(url, anchor)
}

fn lexicon_hit(url: &str, anchor: &str) -> bool {
    let hay = format!("{url} {anchor}").to_lowercase();
    LEXICON.iter().any(|t| hay.contains(t))
}

/// URL shapes that cannot carry author requirements, whatever they link to.
///
/// **This is a COST rule and it DROPS candidates, so it is confined to things
/// that are never guidance.** A research article, a table of contents and a
/// subject taxonomy are not author instructions in any journal.
///
/// The first list was too narrow and the measurement showed it: a 10-journal
/// crawl spent 23 of its 44 lexicon-misses on research articles
/// (`article?id=10.1371/journal.pgen.1012293`) and subject browse pages
/// (`/topic/browse/000079`). Those are pages the lexicon would have RIGHTLY
/// skipped, so counting them as evidence against the lexicon was wrong — the
/// number measured the crawler wandering, not the lexicon failing.
const NEVER_FOLLOW: &[&str] = &[
    "/login", "/register", "/subscribe", "/cart", "/search", "/rss", ".pdf", ".zip",
    "javascript:", "mailto:",
    // research content
    "/article/", "/articles/", "article?id=", "/doi/", "/lookup/", "/content/early",
    // tables of contents, issues, taxonomies. `/volumes/` is Nature's spelling
    // of `/issue/` — `nature.com/nm/volumes/32/issues/7` was admitted as
    // guidance and is 74 blocks of article titles (§11 D163).
    "/toc/", "/issue/", "/volumes/", "/topic/", "/subject/", "/results/", "/browse",
    // site furniture
    "/sitemap", "/accessibility", "/advertis", "/permissions", "/alerts", "/cookies",
    // legal and corporate pages. An allowlisted author-services host is the
    // whole publisher website, so without this a crawl of The Lancet reaches
    // `elsevier.com/legal/privacy-policy` — measured.
    "/legal/", "/privacy", "/terms", "/about-us", "/careers", "/contact",
    // **PAID SERVICES SOLD TO AUTHORS (§11 D161).** A publisher's author-services site
    // mixes guidance with commerce, and its marketing copy is full of numbers
    // that look exactly like requirements. Measured, and it reached the
    // database: `authorservices.springernature.com` put
    // "Premium Chinese Translation … 1,500 words" and "… 12,000 words" into
    // Nature Medicine's `journal_requirements` as WORD LIMITS, beside the real
    // 4,000. A price list is not a requirement, and a requirement invented
    // from one is §11 D155's fabrication with a different surface.
    //
    // **THESE PATHS ARE NOT THE FIX, AND SAYING SO HERE IS THE POINT (D163).**
    // D161 added exactly this list at exactly this problem, and EIGHT pages of
    // the same host walked past it: `/english-language-editing/` does not
    // contain `/english-editing`, and `/formatting/`,
    // `/figure-and-table-formatting/`, `/research-promotion/`,
    // `/featured-articles/…` and the bare host root were never named. A site
    // names its own pages; a path list is an attempt to enumerate a naming
    // scheme nobody here controls. The host itself was removed from
    // `author_services_hosts` on a measured 0/9 precision, and THAT is what
    // closed it. This list stays as depth for publishers that mix commerce
    // into a host still worth crawling — it is not load-bearing on its own.
    "/translation", "/academic-translation", "/pricing", "/scientific-editing",
    "/language-editing", "/english-editing", "/illustration", "/poster",
    "/infographic", "/reprints", "/shop", "/order",
];

fn never_follow(url: &str) -> bool {
    let u = url.to_lowercase();
    if NEVER_FOLLOW.iter().any(|p| u.contains(p)) {
        return true;
    }
    // `/content/12345` is an article; `/content` and `/content-types` are not.
    if let Some(rest) = u.split("/content/").nth(1) {
        return rest.starts_with(|c: char| c.is_ascii_digit());
    }
    false
}

/// **The journal's own corner of a shared host.**
///
/// "Bounded to the journal's own domain" is not enough where the domain IS the
/// publisher. `journals.plos.org` serves every PLOS journal, and a crawl of
/// PLOS ONE measured here wandered into PLOS Genetics, PLOS Pathogens and PLOS
/// Biology — spending a budget meant for one journal on six.
///
/// **How many leading segments identify a journal is per-publisher knowledge,
/// so it lives in config rather than being guessed from the URL.** One for
/// `journals.plos.org/plosone/…`, two for
/// `frontiersin.org/journals/public-health/…`, none for a host that serves a
/// single journal.
///
/// **This is the same KIND of per-publisher knowledge as the lexicon, and the
/// difference is that its failure is visible.** A scope too narrow shows up as
/// a low guideline count with candidates left unvisited; too wide shows up as
/// other journals' URLs in the page list. The lexicon's failure showed up as
/// nothing at all, which is why that one had to go and this one can stay.
/// **Which profiled journal does this URL belong to?**
///
/// The ONLY way a fetched page may acquire a `journal_key`. Before this, a
/// pasted URL inherited whatever journal the picker was showing, so
/// `link.springer.com/journal/11418/submission-guidelines` (Journal of Natural
/// Medicines) stored a requirement under `nature-medicine` — measured live in
/// `docs/RUN31_MEASUREMENT.md` §A.3, with the row printed back through the
/// product's own reader.
///
/// **Host equality is not sufficient and is not the rule here.**
/// `www.nature.com` serves both Nature Medicine and Nature Communications, and
/// `journals.plos.org` serves every PLOS journal, so the leading path segments
/// decide — the same per-publisher knowledge [`journal_scope`] already encodes,
/// read from the same config rather than a second copy of it.
///
/// Returns `None` for any URL no profiled journal claims, which includes every
/// publisher-wide author-services page. That is the intended answer, not a
/// failure: it means nothing may be stored against a journal.
pub fn key_for_url(url: &str, budget: &CrawlBudget) -> Option<String> {
    let target = Url::parse(url).ok()?;
    let target_host = target.host_str()?.to_ascii_lowercase();
    for j in &budget.profiled_journals {
        let Ok(entry) = Url::parse(&j.entry) else { continue };
        let Some(entry_host) = entry.host_str() else { continue };
        if entry_host.to_ascii_lowercase() != target_host {
            continue;
        }
        // No sub-scoping configured for this host: it serves one journal, so
        // the host settles it.
        match journal_scope(&entry, budget) {
            None => return Some(j.key.clone()),
            Some(want) => {
                if journal_scope(&target, budget).as_ref() == Some(&want) {
                    return Some(j.key.clone());
                }
            }
        }
    }
    None
}

fn journal_scope(entry: &Url, budget: &CrawlBudget) -> Option<Vec<String>> {
    let host = entry.host_str()?;
    let n = *budget.journal_path_segments.get(host)?;
    if n == 0 {
        return None;
    }
    let segs: Vec<String> = entry
        .path_segments()?
        .filter(|s| !s.is_empty())
        .take(n)
        .map(|s| s.to_string())
        .collect();
    (segs.len() == n).then_some(segs)
}

fn in_scope(u: &Url, scope: &Option<Vec<String>>) -> bool {
    let Some(want) = scope else { return true };
    let Some(segs) = u.path_segments() else { return false };
    let got: Vec<&str> = segs.filter(|s| !s.is_empty()).take(want.len()).collect();
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| a == b)
}

/// **How confidently a homepage named its own author guidelines.**
///
/// The two are reported separately because they answer different questions. An
/// `Explicit` link is the journal telling us where its guidance is. A `Lexicon`
/// link is Gaply guessing from a topic word, and a guess that lands on a policy
/// page is a discovery limit, not a journal without guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    /// The anchor or path says author guidance in so many words.
    Explicit,
    /// Only a `LEXICON` topic term matched. A candidate, not a claim.
    Lexicon,
}

/// One discovered candidate, with the anchor text that justified it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredLink {
    pub url: String,
    pub anchor: String,
    pub confidence: LinkConfidence,
    /// **The link leaves the host the homepage was served from.**
    ///
    /// Reported rather than dropped, and the distinction is load-bearing. §11
    /// D160's defect was a publisher's author-SERVICES site — which sells
    /// translation and editing — being read as guidance; that is a rule about
    /// what may be ADMITTED, and the page classifier and the extractor are what
    /// enforce it. Enumeration is a different job: a publisher serving one
    /// journal's guidance from a sibling host is ordinary, and a discovery step
    /// that cannot see those links reports them as journals that publish no
    /// guidance. Measured: Elsevier's journal homepages carry a "Guide for
    /// authors" link onto `sciencedirect.com`.
    ///
    /// Same-host candidates come first, so a caller that takes `first()` keeps
    /// the conservative behaviour without asking for it.
    pub off_host: bool,
}

/// Anchor/path shapes that name author guidance outright. **Not a second
/// lexicon**: `LEXICON` decides crawl ORDER over pages already in scope, and
/// changing it cannot change what is admitted. This decides which single link to
/// FOLLOW from a homepage we have no crawl budget for, so it has to be specific
/// or the answer is a policy page every time.
const EXPLICIT: &[&str] = &[
    "instructions-for-authors", "instructions_for_authors", "instructionsforauthors",
    "author-guidelines", "author_guidelines", "authorguidelines",
    "submission-guidelines", "submissionguidelines",
    "for-authors", "forauthors", "for_authors",
    "authors/instructions", "authors-instructions",
    "information-for-authors", "guide-for-authors", "guideforauthors",
];

/// **Does the homepage name its own author guidelines, and how plainly?**
///
/// Deterministic: no model, no proxy. It reuses this module's own `links`,
/// `never_follow` and `carries_guidance` so a homepage cannot send the caller to
/// a research article or a PDF, and it stays on the host it started from —
/// a journal's guidance lives on its own platform, and following off-host links
/// is how §11 D160's publisher-wide author-services page got mistaken for
/// guidance.
///
/// Returns candidates best-first: every `Explicit` match before any `Lexicon`
/// one. **The caller must report which tier it used** — a `Lexicon`-only result
/// that yields nothing is evidence about this function, not about the journal.
///
/// Added for the §11 D192 follow-up measurement: the bundled Scopus directory
/// carries websites and no guideline URLs (258 of 258), so sampling it needs a
/// discovery step, and a discovery failure must be distinguishable from a page
/// that loads empty.
pub fn discover_guidelines_links(html: &str, base: &Url) -> Vec<DiscoveredLink> {
    let host = base.host_str().unwrap_or_default().to_lowercase();
    let mut explicit = Vec::new();
    let mut lexicon = Vec::new();
    let mut off_explicit = Vec::new();
    let mut off_lexicon = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for (url, anchor) in links(html, base) {
        if never_follow(&url) {
            continue;
        }
        let Ok(parsed) = Url::parse(&url) else { continue };
        if !carries_guidance(&parsed) {
            continue;
        }
        let h = parsed.host_str().unwrap_or_default().to_lowercase();
        let off_host = h != host;
        if !seen.insert(normalise(&url)) {
            continue;
        }
        let hay = format!("{} {}", url.to_lowercase(), anchor.to_lowercase());
        let plain = hay.replace(' ', "-");
        if EXPLICIT.iter().any(|e| plain.contains(e))
            || (hay.contains("author")
                && (hay.contains("guideline") || hay.contains("instruction")))
        {
            let l = DiscoveredLink {
                url,
                anchor,
                confidence: LinkConfidence::Explicit,
                off_host,
            };
            if off_host {
                off_explicit.push(l)
            } else {
                explicit.push(l)
            }
        } else if lexicon_hit(&url, &anchor) {
            let l = DiscoveredLink {
                url,
                anchor,
                confidence: LinkConfidence::Lexicon,
                off_host,
            };
            if off_host {
                off_lexicon.push(l)
            } else {
                lexicon.push(l)
            }
        }
    }
    // Same host before anything else, explicit before a guess. `first()` is the
    // conservative choice and stays so.
    explicit.extend(lexicon);
    explicit.extend(off_explicit);
    explicit.extend(off_lexicon);
    explicit
}

/// Extract `(absolute_url, anchor_text)` for every `<a href>`.
fn links(html: &str, base: &Url) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while let Some(rel) = html[i..].find("<a ") {
        let start = i + rel;
        let Some(close) = html[start..].find('>').map(|k| start + k) else { break };
        let tag = &html[start..close];
        i = close + 1;
        let Some(h) = find_attr(tag, "href") else { continue };
        let Some(end) = html[i..].find("</a>").map(|k| i + k) else { continue };
        let anchor = strip_tags_inline(&html[i..end]);
        let _ = bytes;
        if let Ok(abs) = base.join(&h) {
            let mut abs = abs;
            abs.set_fragment(None);
            out.push((abs.to_string(), anchor));
        }
    }
    out
}

/// **Attribute values in HTML are entity-encoded, so the raw slice is not the
/// value. §11 D193.**
///
/// `&` MUST be written `&amp;` inside an attribute, so every query-string URL in
/// every page arrives encoded. Returning the raw slice produced a URL containing
/// a literal `&amp;`, and measured on Taylor & Francis that is not a different
/// spelling of the same page — it is a **404**:
///
/// ```text
/// .../authorSubmission?show=instructions&amp;journalCode=rwar20  -> 404, 2,378 chars
/// .../authorSubmission?show=instructions&journalCode=rwar20      -> 200, 20,981 chars
/// ```
///
/// One journal in the D193 sample, and the blast radius is every crawl: `links`
/// is what builds the frontier, so any site that routes author guidance through a
/// query string was unreachable and looked like a site without guidance.
fn find_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let at = lower.find(&format!("{name}="))? + name.len() + 1;
    let rest = &tag[at..];
    let quote = rest.chars().next()?;
    let raw = if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)? + 1;
        &rest[1..end]
    } else {
        let end = rest.find([' ', '>']).unwrap_or(rest.len());
        &rest[..end]
    };
    Some(decode_entities(raw))
}

/// The named and numeric entities that appear in URLs. **Deliberately not a full
/// HTML entity table**: this decodes an attribute value, and the only characters
/// that must be escaped there are `&`, the quote in use, and `<`. A broad table
/// would start rewriting path segments that legitimately contain `&#` sequences.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        // Longest first, so `&amp;` is never read as a bare `&` followed by text.
        let matched = [
            ("&amp;", "&"),
            ("&#38;", "&"),
            ("&#x26;", "&"),
            ("&#X26;", "&"),
            ("&quot;", "\""),
            ("&#34;", "\""),
            ("&apos;", "'"),
            ("&#39;", "'"),
            ("&lt;", "<"),
            ("&gt;", ">"),
            ("&#47;", "/"),
            ("&#61;", "="),
            ("&#63;", "?"),
        ]
        .iter()
        .find(|(e, _)| tail.starts_with(*e))
        .copied();
        match matched {
            Some((e, r)) => {
                out.push_str(r);
                rest = &tail[e.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn strip_tags_inline(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ if depth <= 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The dedup key for a fetched page.
///
/// **The scheme is dropped, and that is not cosmetic.** Journals link the same
/// page as both `http://` and `https://` — PLOS does it throughout its own
/// guideline tree — and the http form 301s to the https one. Keeping the scheme
/// in the key fetched every such page TWICE: measured at a 120-page budget,
/// PLOS ONE reported 52 guideline pages and 23 lexicon misses where the real
/// figures are roughly half that, with `…/s/tables` appearing at depth 1 as
/// https and depth 2 as http.
///
/// The inflation is the dangerous part: it lands on `guideline` and
/// `lexicon_misses`, the two numbers this crawl is judged by, and it inflates
/// them in the flattering direction.
fn normalise(u: &str) -> String {
    let no_scheme = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(u);
    no_scheme.trim_end_matches('/').to_lowercase()
}

/// Crawl one journal's guideline ecosystem from an entry URL.
pub fn crawl(
    fetcher: &dyn HttpFetcher,
    limiter: &RateLimiter,
    budget: &CrawlBudget,
    entry: &str,
) -> Result<CrawlOutcome, GaplyError> {
    let entry_url =
        Url::parse(entry).map_err(|e| GaplyError::Validation(format!("bad entry url: {e}")))?;
    let host = entry_url.host_str().unwrap_or_default().to_string();

    let scope = journal_scope(&entry_url, budget);
    let allowed = |u: &Url| -> bool {
        if !carries_guidance(u) {
            return false;
        }
        let h = u.host_str().unwrap_or_default();
        if budget.author_services_hosts.iter().any(|a| a == h) && h != host {
            // An allowlisted publisher host carries requirements for many
            // journals; the journal-segment scope does not apply to it.
            return true;
        }
        h == host && in_scope(u, &scope)
    };

    let mut entry_interstitial: Option<String> = None;
    // **Dedup on CONTENT, not on URL.** `nature.com/nm/content` and
    // `nature.com/nm/about/content` are one page under two paths; URL dedup
    // cannot see that, and the first fingerprint build stored twelve
    // requirements twice and reported them as twelve per "source document" for
    // two documents. Two URLs serving identical bytes is one document, and a
    // hash says so at the cost of one pass over text already in memory.
    let mut content_seen: BTreeMap<[u8; 32], String> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(normalise(entry));
    // Two queues, so a lexicon hit is fetched FIRST without ever being the
    // condition for being fetched at all.
    let mut priority: VecDeque<(String, String, usize)> = VecDeque::new();
    let mut rest: VecDeque<(String, String, usize)> = VecDeque::new();
    priority.push_back((entry.to_string(), String::new(), 0));

    let mut out = CrawlOutcome {
        entry: entry.to_string(),
        host: host.clone(),
        fetched: 0,
        guideline: 0,
        navigation: 0,
        interstitial: 0,
        errors: 0,
        lexicon_misses: 0,
        unvisited: 0,
        duplicate_content: 0,
        rate_limited: false,
        stopped_by: StoppedBy::FrontierExhausted,
        pages: Vec::new(),
    };

    while out.fetched < budget.max_pages {
        // **DEPTH DOMINATES, the lexicon orders WITHIN a depth.**
        //
        // The first version drained the lexicon-hit queue completely before
        // touching the rest, which put every lexicon MISS behind every hit at
        // any depth — so under a budget the pages the inversion exists to find
        // were the first to be cut off. Measured: a 40-page crawl of Nature
        // Medicine never reached `/nm/content`, the one page carrying every one
        // of its extractable requirements and the case this whole design is
        // justified by. **The mechanism that made the crawl efficient was
        // suppressing the evidence for it.**
        //
        // A link one hop from the author-guidelines page is likelier to be
        // guidance than a lexicon match three hops away, so depth is the outer
        // key and the lexicon the inner one.
        let take_priority = match (priority.front(), rest.front()) {
            (Some((_, _, dp)), Some((_, _, dr))) => dp <= dr,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => break,
        };
        let Some((url, anchor, depth)) =
            (if take_priority { priority.pop_front() } else { rest.pop_front() })
        else {
            break;
        };
        // **Waiting for a token is not the same as giving up.** The first
        // version pushed the candidate back and broke out when it was the only
        // one left; because the limiter is per-host and shared across journals,
        // a crawl of a second journal on the same host began with an empty
        // bucket and ended having fetched ZERO pages while reporting
        // `FrontierExhausted`. Measured: Nature Communications, immediately
        // after Nature Medicine's 40 requests to `www.nature.com`.
        let mut waited = 0u32;
        while !limiter.try_consume(&host).allowed {
            if waited >= budget.max_rate_wait_ticks {
                out.rate_limited = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            waited += 1;
        }
        if out.rate_limited {
            rest.push_front((url, anchor, depth));
            break;
        }
        let is_entry = depth == 0;
        let resp = match fetcher.get(&HttpRequest::get(&url)) {
            Ok(r) => r,
            Err(e) => {
                out.errors += 1;
                out.fetched += 1;
                if is_entry {
                    out.stopped_by = StoppedBy::EntryUnreachable { reason: e.to_string() };
                    break;
                }
                continue;
            }
        };
        out.fetched += 1;
        if resp.status != 200 {
            out.errors += 1;
            if is_entry {
                out.stopped_by = StoppedBy::EntryUnreachable {
                    reason: format!("the entry URL answered http {}", resp.status),
                };
                break;
            }
            continue;
        }
        let text = html_to_text_public(&resp.body);

        // Hash the EXTRACTED TEXT, not the raw body: two URLs for one page
        // differ in their own canonical link and nav highlighting while the
        // guidance is identical, and it is the guidance that decides whether
        // this is a second document.
        let digest: [u8; 32] = {
            use sha2::Digest;
            let mut h = sha2::Sha256::new();
            h.update(text.as_bytes());
            h.finalize().into()
        };
        if let Some(first) = content_seen.get(&digest) {
            out.duplicate_content += 1;
            out.pages.push(CrawledPage {
                url: url.clone(),
                anchor: anchor.clone(),
                depth,
                verdict: "duplicate",
                chars: text.chars().count(),
                obligations: 0,
                requirements: 0,
                lexicon_hit: hit_of(&url, &anchor),
                body: None,
            });
            tracing::debug!(url = %url, first = %first, "same content under a second url");
            // Its links are the first copy's links and are already queued.
            continue;
        }
        content_seen.insert(digest, url.clone());

        let verdict = classify_page(&html_title(&resp.body), &text);
        let hit = lexicon_hit(&url, &anchor);
        let (label, ob, rq) = match &verdict {
            PageVerdict::Guideline { obligations, requirements } => {
                out.guideline += 1;
                if !hit {
                    // The page the lexicon would have skipped.
                    out.lexicon_misses += 1;
                }
                ("guideline", *obligations, *requirements)
            }
            PageVerdict::Navigation { obligations, requirements } => {
                out.navigation += 1;
                ("navigation", *obligations, *requirements)
            }
            PageVerdict::Interstitial { signature } => {
                out.interstitial += 1;
                if is_entry {
                    entry_interstitial = Some((*signature).to_string());
                }
                ("interstitial", 0, 0)
            }
        };
        out.pages.push(CrawledPage {
            url: url.clone(),
            anchor: anchor.clone(),
            depth,
            verdict: label,
            chars: text.chars().count(),
            obligations: ob,
            requirements: rq,
            lexicon_hit: hit,
            // Retained ONLY for guideline pages — see the field's docs.
            body: matches!(verdict, PageVerdict::Guideline { .. })
                .then(|| resp.body.clone()),
        });

        // **An interstitial's links are the bot-wall's.** Never enqueued.
        if matches!(verdict, PageVerdict::Interstitial { .. }) || depth >= budget.max_depth {
            continue;
        }
        let Ok(base) = Url::parse(&url) else { continue };
        for (abs, anchor_text) in links(&resp.body, &base) {
            let key = normalise(&abs);
            if seen.contains(&key) || never_follow(&abs) {
                continue;
            }
            let Ok(parsed) = Url::parse(&abs) else { continue };
            if !allowed(&parsed) {
                continue;
            }
            seen.insert(key);
            let item = (abs.clone(), anchor_text.clone(), depth + 1);
            if lexicon_hit(&abs, &anchor_text) {
                priority.push_back(item);
            } else {
                rest.push_back(item);
            }
        }
    }

    out.unvisited = priority.len() + rest.len();
    if let Some(sig) = entry_interstitial {
        // The entry was a bot wall. Its links were never followed (they are the
        // wall's), so an empty frontier here says nothing about the journal.
        out.stopped_by = StoppedBy::EntryUnreachable {
            reason: format!("the entry URL returned an interstitial ({sig:?})"),
        };
    } else if !matches!(out.stopped_by, StoppedBy::EntryUnreachable { .. })
        && out.unvisited > 0
        && out.fetched >= budget.max_pages
    {
        out.stopped_by = StoppedBy::Budget;
    }
    // NOTE the state deliberately NOT added: an entry that answers 200 with a
    // real page carrying no guidance and no links HAS resolved. Reporting that
    // as unreachable would hide a true result — "we read the journal's entry
    // and it has nothing" — behind a transport failure. `FrontierExhausted`
    // with `guideline == 0` is the honest verdict there.
    Ok(out)
}

#[cfg(test)]
mod tests {

    /// **A document that cannot carry guidance is refused by URL SHAPE.** §11 D185.
    ///
    /// Both of these produced requirements in a real crawl. The listing gave
    /// three: the extractor read paper titles as the journal's own policy. The
    /// `.doc` gave a word limit from a copyright licence.
    #[test]
    fn an_article_listing_and_a_binary_attachment_carry_no_guidance() {
        let no = [
            "https://www.bmj.com/content/by/section/Research%20Methods%20%26amp%3B%20Reporting",
            "http://www.bmj.com/sites/default/files/BMJ%20Author%20Licence%20March%202013.doc",
        ];
        for u in no {
            assert!(
                !carries_guidance(&Url::parse(u).unwrap()),
                "must be refused: {u}"
            );
        }

        // **The negative control, and it is the half that matters.** This is a
        // REFUSAL, so its failure direction is losing a real guidance page. Each
        // of these is an entry a journal in the shipped config actually uses.
        let yes = [
            "https://journals.plos.org/plosone/s/submission-guidelines",
            "https://www.bmj.com/about-bmj/resources-authors",
            "https://www.nature.com/nm/for-authors",
            "https://bmcpublichealth.biomedcentral.com/submission-guidelines",
            "https://www.bmj.com/content/bmj/section/research",
        ];
        for u in yes {
            assert!(
                carries_guidance(&Url::parse(u).unwrap()),
                "must still be admitted: {u}"
            );
        }
    }
    use super::*;
    use gaply_core::refverify::MockHttpFetcher;

    fn budget(max_pages: usize, max_depth: usize) -> CrawlBudget {
        CrawlBudget {
            max_pages,
            max_depth,
            author_services_hosts: vec![],
            journal_path_segments: Default::default(),
            max_rate_wait_ticks: 0,
            // The crawl does not read this; it is the picker's list. Empty here
            // so a test cannot accidentally depend on the shipped ten.
            profiled_journals: vec![],
        }
    }
    fn roomy() -> RateLimiter {
        RateLimiter::new(1000.0, 1000.0)
    }
    /// A page that classifies as guideline content.
    /// **The title is repeated IN THE BODY on purpose.** `html_to_text` strips
    /// `<head>`, so a fixture whose only difference is its `<title>` is a
    /// byte-identical document — and the content dedup correctly calls it one.
    /// Two fixtures meant to be different pages have to differ where a reader
    /// would see it.
    fn guideline_body(title: &str, extra_links: &str) -> String {
        format!(
            "<html><head><title>{title}</title></head><body><h1>{title}</h1>\
             Authors must declare all competing interests. Manuscripts should be submitted \
             through the online system. Please ensure the data availability statement is \
             complete. Authors are required to register trials prospectively. {extra_links}\
             </body></html>"
        )
    }
    fn nav_body(title: &str, extra_links: &str) -> String {
        format!("<html><head><title>{title}</title></head><body>Latest content Archive Jobs \
                 {extra_links}</body></html>")
    }

    /// **THE INVERSION, as a test.** The requirements page is linked with an
    /// anchor no lexicon term matches — Nature Medicine's real case, where
    /// `/nm/content` / "Content types" carries every extractable limit. It must
    /// still be fetched, still be classified `Guideline`, and be COUNTED as a
    /// lexicon miss.
    #[test]
    fn a_page_no_lexicon_term_would_admit_is_still_found_and_counted() {
        let entry = nav_body(
            "For authors",
            r#"<a href="https://j.test/content">Content types</a>"#,
        );
        let content = guideline_body("Content Types", "");
        let f = MockHttpFetcher::new()
            .route("j.test/for-authors", 200, &entry)
            .route("j.test/content", 200, &content);
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/for-authors").unwrap();

        assert_eq!(out.guideline, 1, "{:#?}", out.pages);
        let page = out.pages.iter().find(|p| p.url.contains("/content")).expect("fetched");
        assert_eq!(page.verdict, "guideline");
        assert!(!page.lexicon_hit, "the whole point: no lexicon term matches it");
        assert_eq!(out.lexicon_misses, 1, "the standing evidence for the inversion");
    }

    /// **Depth dominates the lexicon.** A lexicon miss one hop away must be
    /// fetched before a lexicon hit three hops away — otherwise a budget cuts
    /// off exactly the pages fetch-and-classify exists to find. Measured:
    /// before this, a 40-page crawl of Nature Medicine never reached
    /// `/nm/content`.
    #[test]
    fn a_near_lexicon_miss_beats_a_distant_lexicon_hit() {
        let entry = nav_body(
            "For authors",
            r#"<a href="https://j.test/content">Content types</a>
               <a href="https://j.test/author-guidelines">Author guidelines</a>"#,
        );
        // The guidelines page links onward to more lexicon hits at depth 2.
        let deep: String = (0..10)
            .map(|i| format!(r#"<a href="https://j.test/author-deep{i}">Author formatting {i}</a>"#))
            .collect();
        let mut f = MockHttpFetcher::new()
            .route("j.test/for-authors", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("Guidelines", &deep))
            .route("j.test/content", 200, &guideline_body("Content Types", ""));
        for i in 0..10 {
            f = f.route(&format!("j.test/author-deep{i}"), 200, &guideline_body("Deep", ""));
        }
        // A budget that cannot hold everything: entry + both depth-1 pages + some.
        let out = crawl(&f, &roomy(), &budget(4, 3), "https://j.test/for-authors").unwrap();
        assert_eq!(out.stopped_by, StoppedBy::Budget);
        assert!(
            out.pages.iter().any(|p| p.url.ends_with("/content")),
            "the depth-1 lexicon miss must be reached before depth-2 hits: {:#?}",
            out.pages.iter().map(|p| (&p.url, p.depth)).collect::<Vec<_>>()
        );
        assert_eq!(out.lexicon_misses, 1);
    }

    /// The lexicon still ORDERS the crawl: a matching link is fetched before a
    /// non-matching one at the same depth.
    #[test]
    fn the_lexicon_decides_order_and_never_admission() {
        let entry = nav_body(
            "Home",
            r#"<a href="https://j.test/zzz">Content types</a>
               <a href="https://j.test/author-guidelines">Author guidelines</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/zzz", 200, &guideline_body("Content Types", ""))
            .route("j.test/author-guidelines", 200, &guideline_body("Guidelines", ""));
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        let order: Vec<&str> = out.pages.iter().map(|p| p.url.as_str()).collect();
        let g = order.iter().position(|u| u.contains("author-guidelines")).unwrap();
        let z = order.iter().position(|u| u.contains("zzz")).unwrap();
        assert!(g < z, "the lexicon hit should be fetched first: {order:?}");
        // And both were fetched, which is what "order, not admission" means.
        assert_eq!(out.guideline, 2);
    }

    /// **An interstitial's links are the bot-wall's.** The challenge page links
    /// to a page that would classify as guideline content; it must never be
    /// fetched, because the request never reached the journal.
    #[test]
    fn links_from_an_interstitial_are_not_followed() {
        let challenge = format!(
            "<html><head><title>Client Challenge</title></head><body>\
             A required part of this site couldn't load.\
             <a href=\"https://j.test/vendor-help\">Help</a></body></html>"
        );
        let f = MockHttpFetcher::new()
            .route("j.test/guidelines", 200, &challenge)
            .route("j.test/vendor-help", 200, &guideline_body("Help", ""));
        let out = crawl(&f, &roomy(), &budget(10, 3), "https://j.test/guidelines").unwrap();
        assert_eq!(out.interstitial, 1);
        assert_eq!(out.fetched, 1, "the challenge page's links must not be followed");
        assert!(out.pages.iter().all(|p| !p.url.contains("vendor-help")), "{:#?}", out.pages);
    }

    /// A navigation page's links ARE followed — it is the journal's page, just
    /// not guidance. Without this the crawl stops at every hub.
    #[test]
    fn links_from_a_navigation_page_are_followed() {
        let entry = nav_body("Home", r#"<a href="https://j.test/author-guidelines">Authors</a>"#);
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("Guidelines", ""));
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out.navigation, 1);
        assert_eq!(out.guideline, 1);
    }

    /// **A crawl stopped by its budget is a coverage claim you cannot make**,
    /// and the outcome has to say so.
    #[test]
    fn a_crawl_stopped_by_the_budget_says_so() {
        let many: String = (0..30)
            .map(|i| format!(r#"<a href="https://j.test/p{i}">Author guidelines {i}</a>"#))
            .collect();
        let mut f = MockHttpFetcher::new().route("j.test/home", 200, &nav_body("Home", &many));
        for i in 0..30 {
            f = f.route(&format!("j.test/p{i}"), 200, &guideline_body("Guidelines", ""));
        }
        let out = crawl(&f, &roomy(), &budget(5, 3), "https://j.test/home").unwrap();
        assert_eq!(out.fetched, 5, "the budget is a HARD cap");
        assert_eq!(out.stopped_by, StoppedBy::Budget);
        assert!(out.unvisited > 0, "candidates were left queued");

        // And the same crawl with room finishes, so the flag tracks the budget
        // rather than always being set.
        let out2 = crawl(&f, &roomy(), &budget(100, 3), "https://j.test/home").unwrap();
        assert_eq!(out2.stopped_by, StoppedBy::FrontierExhausted);
        assert_eq!(out2.unvisited, 0);
    }

    /// **A shared host is not a journal.** `journals.plos.org` serves every
    /// PLOS journal; a crawl of PLOS ONE that wanders into PLOS Genetics is
    /// spending one journal's budget on six. Measured before this rule existed.
    #[test]
    fn a_crawl_stays_inside_the_journals_own_path_segment() {
        let entry = nav_body(
            "Submission guidelines",
            r#"<a href="https://j.test/plosone/s/tables">Tables</a>
               <a href="https://j.test/plosgenetics/s/tables">Tables</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/plosone/s/submission-guidelines", 200, &entry)
            .route("j.test/plosone/s/tables", 200, &guideline_body("Tables", ""))
            .route("j.test/plosgenetics/s/tables", 200, &guideline_body("Tables", ""));
        let mut b = budget(10, 2);
        b.journal_path_segments.insert("j.test".into(), 1);
        let out =
            crawl(&f, &roomy(), &b, "https://j.test/plosone/s/submission-guidelines").unwrap();
        assert_eq!(out.fetched, 2, "{:#?}", out.pages);
        assert!(out.pages.iter().all(|p| !p.url.contains("plosgenetics")), "{:#?}", out.pages);
    }

    /// Research articles and subject taxonomies are never author guidance, and
    /// following them spends the budget on content. This is the rule that made
    /// `lexicon_misses` mean what it claims.
    #[test]
    fn research_content_and_taxonomies_are_never_followed() {
        // A publisher's paid services — measured reaching the database as
        // Nature Medicine "word limits" of 1,500 and 12,000.
        for u in [
            "https://authorservices.springernature.com/academic-translation-services/",
            "https://authorservices.springernature.com/pricing/",
            "https://authorservices.springernature.com/translation",
            "https://authorservices.springernature.com/scientific-editing/",
            "https://www.elsevier.com/legal/privacy-policy",
            "https://onlinelibrary.wiley.com/termsAndConditions",
            "https://journals.plos.org/plosone/article?id=10.1371/journal.pgen.1012293",
            "https://onlinelibrary.wiley.com/topic/browse/000079",
            "https://onlinelibrary.wiley.com/toc/10970258/current",
            "https://www.bmj.com/content/378/bmj-2021-069048",
            "https://www.bmj.com/lookup/ijlink/abc",
            "https://onlinelibrary.wiley.com/sitemap",
            // Nature's spelling of an issue table of contents. Admitted as
            // guidance before D163; 74 blocks of article titles.
            "https://www.nature.com/nm/volumes/32/issues/7",
        ] {
            assert!(never_follow(u), "should not be followed: {u}");
        }
        // …and the shapes that LOOK similar but are guidance.
        for u in [
            "https://www.nature.com/nm/content",
            "https://www.thelancet.com/what-we-publish",
            "https://journals.plos.org/plosone/s/tables",
        ] {
            assert!(!never_follow(u), "must still be followed: {u}");
        }
    }

    /// **Waiting for a token is not giving up.** The first version broke out of
    /// the loop when the limiter refused the last queued candidate, so a
    /// second journal on the same host could report `FrontierExhausted` having
    /// fetched nothing. Measured on Nature Communications.
    #[test]
    fn an_exhausted_rate_limiter_is_reported_not_silently_treated_as_finished() {
        let entry = nav_body("Home", r#"<a href="https://j.test/author-guidelines">Authors</a>"#);
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("G", ""));
        // A limiter with no tokens and no refill within the wait window.
        // One token, no refill: the entry is fetched, the next candidate is not.
        let empty = RateLimiter::new(1.0, 0.000_01);
        let out = crawl(&f, &empty, &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out.fetched, 1);
        assert!(out.rate_limited, "a crawl that fetched nothing must say why");
        assert!(out.unvisited > 0, "the entry is still queued, not consumed");
    }

    /// **"Nothing to crawl" and "crawled everything" were one word.**
    /// Measured on Nature Communications: its entry 303s to a cookie-error URL,
    /// the crawl fetched one page, found no links, and reported
    /// `FrontierExhausted` — the same verdict as a journal whose entire
    /// guideline tree had been read.
    #[test]
    fn an_entry_that_never_resolved_is_its_own_outcome() {
        // 1. The entry answers a non-200.
        let f = MockHttpFetcher::new().route("j.test/submit", 303, "");
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/submit").unwrap();
        match &out.stopped_by {
            StoppedBy::EntryUnreachable { reason } => assert!(reason.contains("303"), "{reason}"),
            other => panic!("expected EntryUnreachable, got {other:?}"),
        }

        // 2. The entry is a bot wall. Its links are the wall's, so an empty
        //    frontier afterwards says nothing about the journal.
        let wall = "<html><head><title>Client Challenge</title></head><body>\
                    <a href=\"https://j.test/x\">x</a></body></html>";
        let f2 = MockHttpFetcher::new().route("j.test/guidelines", 200, wall);
        let out2 = crawl(&f2, &roomy(), &budget(10, 2), "https://j.test/guidelines").unwrap();
        assert!(matches!(out2.stopped_by, StoppedBy::EntryUnreachable { .. }), "{:?}", out2.stopped_by);

        // 3. THE BOUNDARY, in the other direction. A 200 with a real page that
        //    happens to carry no guidance and no links HAS resolved, and is
        //    reported as exhausted — not as unreachable. Calling it unreachable
        //    would hide a true result behind a transport failure.
        let f3 = MockHttpFetcher::new()
            .route("j.test/empty", 200, "<html><head><title>J</title></head><body>.</body></html>");
        let out3 = crawl(&f3, &roomy(), &budget(10, 2), "https://j.test/empty").unwrap();
        assert_eq!(out3.stopped_by, StoppedBy::FrontierExhausted);
        assert_eq!(out3.guideline, 0, "and the emptiness is visible in the count");

        // …and a real crawl still reports FrontierExhausted, so the new state
        // is not simply swallowing the old one.
        let entry = nav_body("Home", r#"<a href="https://j.test/author-guidelines">Authors</a>"#);
        let f4 = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("G", ""));
        let out4 = crawl(&f4, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out4.stopped_by, StoppedBy::FrontierExhausted);
    }

    #[test]
    fn the_depth_bound_holds_and_zero_means_the_entry_page_alone() {
        let entry = nav_body("Home", r#"<a href="https://j.test/author-guidelines">Authors</a>"#);
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("G", ""));
        let out = crawl(&f, &roomy(), &budget(10, 0), "https://j.test/home").unwrap();
        assert_eq!(out.fetched, 1);
        assert_eq!(out.stopped_by, StoppedBy::FrontierExhausted);
    }

    #[test]
    fn the_crawl_stays_on_the_journals_domain_unless_a_host_is_allowlisted() {
        let entry = nav_body(
            "Home",
            r#"<a href="https://elsewhere.test/author-guidelines">Authors</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("elsewhere.test/author-guidelines", 200, &guideline_body("G", ""));
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out.fetched, 1, "off-domain must not be followed");

        let mut b = budget(10, 2);
        b.author_services_hosts = vec!["elsewhere.test".into()];
        let out2 = crawl(&f, &roomy(), &b, "https://j.test/home").unwrap();
        assert_eq!(out2.fetched, 2, "an allowlisted publisher host is followed");
    }

    /// **`http://` and `https://` are the same page.** Journals link both, the
    /// http form redirects to the https one, and fetching each twice inflates
    /// exactly the two numbers this crawl is judged by.
    #[test]
    fn the_same_page_over_http_and_https_is_fetched_once() {
        let entry = nav_body(
            "Home",
            r#"<a href="https://j.test/author-guidelines">Authors</a>
               <a href="http://j.test/author-guidelines">Authors</a>
               <a href="https://j.test/author-guidelines/">Authors</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("G", ""));
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out.fetched, 2, "{:#?}", out.pages);
        assert_eq!(out.guideline, 1);
        assert_eq!(out.lexicon_misses, 0);
    }

    /// **Two URLs serving identical bytes is ONE document.** Measured on
    /// Nature Medicine: `/nm/content` and `/nm/about/content` are the same
    /// page, and the first fingerprint build stored its twelve requirements
    /// twice and reported them as twelve per "source document" for two
    /// documents. URL dedup cannot see this; a content hash can.
    #[test]
    fn the_same_page_under_two_paths_is_one_document() {
        // One document, served at two paths — identical bytes, as Nature does.
        let body = guideline_body("Content Types", "");
        let entry = nav_body(
            "Home",
            r#"<a href="https://j.test/content">Content types</a>
               <a href="https://j.test/about/content">Content types</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/for-authors", 200, &entry)
            .route("j.test/content", 200, &body)
            .route("j.test/about/content", 200, &body);
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/for-authors").unwrap();

        // Both were FETCHED — a duplicate still costs a request, and hiding
        // that would understate what the crawl spent.
        assert_eq!(out.fetched, 3);
        assert_eq!(out.duplicate_content, 1);
        // …but only ONE is guideline content, so nothing downstream stores it
        // twice.
        assert_eq!(out.guideline, 1, "{:#?}", out.pages);
        assert_eq!(out.lexicon_misses, 1, "and the miss is not double-counted");
        assert!(out.pages.iter().any(|p| p.verdict == "duplicate"));
    }

    /// Different content under similar URLs is NOT a duplicate — the hash must
    /// not collapse two real pages.
    #[test]
    fn two_different_pages_are_not_collapsed_by_the_hash() {
        let entry = nav_body(
            "Home",
            // NOTE the paths do not prefix one another: `MockHttpFetcher`
            // routes by substring, so `/author-guidelines` would also answer
            // for `/author-guidelines-2` and the second fixture would never be
            // served — a harness artefact that looks exactly like a hash
            // collision.
            r#"<a href="https://j.test/author-guidelines">Authors</a>
               <a href="https://j.test/formatting">Formatting</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/author-guidelines", 200, &guideline_body("One", ""))
            .route("j.test/formatting", 200,
                   &guideline_body("Two", "<p>Authors must also register the trial prospectively.</p>"));
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out.duplicate_content, 0, "{:#?}", out.pages);
        assert_eq!(out.guideline, 2);
    }

    #[test]
    fn a_page_is_fetched_once_however_many_times_it_is_linked() {
        let entry = nav_body(
            "Home",
            r#"<a href="https://j.test/g">Authors</a><a href="https://j.test/g/">Authors</a>
               <a href="https://j.test/g#section">Authors</a>"#,
        );
        let f = MockHttpFetcher::new()
            .route("j.test/home", 200, &entry)
            .route("j.test/g", 200, &guideline_body("G", ""));
        let out = crawl(&f, &roomy(), &budget(10, 2), "https://j.test/home").unwrap();
        assert_eq!(out.fetched, 2, "{:#?}", out.pages);
    }

    // ---- the budget lives in config, not code -------------------------

    #[test]
    fn the_budget_is_read_from_config_and_has_no_compiled_in_default() {
        let b = CrawlBudget::from_json(
            r#"{"max_pages": 7, "max_depth": 2, "author_services_hosts": ["x.test"]}"#,
        )
        .unwrap();
        assert_eq!(b.max_pages, 7);
        assert_eq!(b.max_depth, 2);
        // A budget of zero is a configuration error, not "crawl nothing".
        assert!(CrawlBudget::from_json(r#"{"max_pages": 0, "max_depth": 2}"#).is_err());
        assert!(CrawlBudget::from_json("not json").is_err());
    }

    /// The shipped config must parse, or the crawl has no bounds at runtime.
    #[test]
    fn the_shipped_config_file_parses() {
        let s = include_str!("../config/journal-crawl.json");
        let b = CrawlBudget::from_json(s).expect("config/journal-crawl.json");
        assert!(b.max_pages >= 1 && b.max_depth <= 5, "{b:?}");
        assert!(!b.author_services_hosts.is_empty());
        assert!(b.max_rate_wait_ticks > 0, "a zero wait gives up on the first refusal");
        assert!(b.max_pages >= 120, "the measured bound, raised from 40: {b:?}");
        assert!(b.journal_path_segments.contains_key("journals.plos.org"), "{b:?}");
    }

    #[test]
    fn links_are_resolved_relative_to_the_page_they_were_found_on() {
        let base = Url::parse("https://j.test/a/b").unwrap();
        let got = links(
            r#"<a href="../c">C</a><a href="/d">D</a><a href="https://j.test/e">E</a>"#,
            &base,
        );
        let urls: Vec<&str> = got.iter().map(|(u, _)| u.as_str()).collect();
        assert_eq!(urls, vec!["https://j.test/c", "https://j.test/d", "https://j.test/e"]);
        assert_eq!(got[0].1, "C");
    }
    // ---------------------------------------------------------------
    // Discovery from a homepage (§11 D192 follow-up)
    // ---------------------------------------------------------------

    fn home(body: &str) -> String {
        format!("<html><body>{body}</body></html>")
    }
    fn base() -> Url {
        Url::parse("https://j.test/").unwrap()
    }

    /// An explicit naming outranks a topic word, and the tier is reported. A
    /// caller that cannot tell the two apart cannot tell "this journal hid its
    /// guidance" from "Gaply guessed and guessed wrong".
    #[test]
    fn an_explicit_author_guidelines_link_outranks_a_lexicon_guess() {
        let html = home(
            r#"<a href="/about/policies">Editorial policies</a>
               <a href="/authors/instructions">Instructions for authors</a>"#,
        );
        let got = discover_guidelines_links(&html, &base());
        assert_eq!(got[0].url, "https://j.test/authors/instructions", "{got:?}");
        assert_eq!(got[0].confidence, LinkConfidence::Explicit);
        // The policy page is still a candidate — it is just not the first one.
        assert!(got.iter().any(|l| l.confidence == LinkConfidence::Lexicon), "{got:?}");
    }

    /// A homepage that names nothing must return NOTHING, not its least bad
    /// link. This is the bucket the measurement needs kept separate: "no URL
    /// discoverable" is a discovery limit, and inventing a candidate here would
    /// silently reclassify it as "the page had no guidance on it".
    #[test]
    fn a_homepage_that_names_no_guidance_yields_no_candidate() {
        let html = home(r#"<a href="/issues/current">Current issue</a>
                           <a href="/contact">Contact us</a>"#);
        assert!(discover_guidelines_links(&html, &base()).is_empty());
    }

    /// Off-host links are refused. §11 D160: a publisher's author-SERVICES site
    /// sells translation and editing to the same authors it advises, and reading
    /// it as guidance is how a price list became a 12000-word limit.
    /// **Off-host links are FLAGGED and ranked last, not dropped.**
    ///
    /// This test asserted `len() == 1` until the §11 D192 follow-up run showed
    /// what dropping costs: Elsevier serves every journal's "Guide for authors"
    /// from `sciencedirect.com`, so a same-host-only enumeration reported seven
    /// live journals as publishing no guidance. The POLICY is unchanged —
    /// `first()` is still same-host — but the caller can now see what it is
    /// declining, which is the difference between a measurement and a blind spot.
    #[test]
    fn an_off_host_link_ranks_last_and_says_that_it_is_off_host() {
        let html = home(
            r#"<a href="https://authorservices.example.com/author-guidelines">Author guidelines</a>
               <a href="/authors/guidelines">Author guidelines</a>"#,
        );
        let got = discover_guidelines_links(&html, &base());
        assert_eq!(got.len(), 2, "{got:?}");
        // The conservative pick is first and is on the host we started from.
        assert_eq!(got[0].url, "https://j.test/authors/guidelines");
        assert!(!got[0].off_host);
        // The author-SERVICES page is visible and marked, so a caller cannot
        // follow it by accident and cannot miss that it existed.
        assert!(got[1].off_host, "{got:?}");
        assert!(got[1].url.contains("authorservices"));
    }

    /// The existing cost rules still apply — a PDF or a research article cannot
    /// become the discovered guidance page however it is labelled.
    #[test]
    fn discovery_reuses_the_crawlers_refusals_rather_than_restating_them() {
        let html = home(
            r#"<a href="/files/author-guidelines.pdf">Author guidelines (PDF)</a>
               <a href="/article/10.1000/x">Author guidelines</a>"#,
        );
        assert!(discover_guidelines_links(&html, &base()).is_empty());
    }

    /// **The measured case. §11 D193.** The encoded URL is a 404 and the decoded
    /// one is the journal's real author instructions, so this is not cosmetic.
    #[test]
    fn an_href_with_an_encoded_ampersand_yields_a_url_that_resolves() {
        let html = r#"<a href="/action/authorSubmission?show=instructions&amp;journalCode=rwar20">Instructions for authors</a>"#;
        let got = links(html, &Url::parse("https://www.tandfonline.com/").unwrap());
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(
            got[0].0,
            "https://www.tandfonline.com/action/authorSubmission?show=instructions&journalCode=rwar20"
        );
        assert!(!got[0].0.contains("amp;"), "a literal &amp; in a URL is a 404: {}", got[0].0);
    }

    /// A bare `&` that is not an entity must survive untouched, and a `&` inside
    /// a path must not become anything else.
    #[test]
    fn decoding_leaves_a_non_entity_ampersand_alone() {
        assert_eq!(decode_entities("a&b"), "a&b");
        assert_eq!(decode_entities("?x=1&amp;y=2&amp;z=3"), "?x=1&y=2&z=3");
        assert_eq!(decode_entities("no entities here"), "no entities here");
        // Numeric forms of the same character.
        assert_eq!(decode_entities("?a=1&#38;b=2"), "?a=1&b=2");
        // `&amp;amp;` is a double-encoded ampersand: one pass, one level, no loop.
        assert_eq!(decode_entities("&amp;amp;"), "&amp;");
    }

}
