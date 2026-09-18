//! **Are the other two keyword branches right?** One hardcoded `contains` was
//! wrong (§11 D181), so the other two are checked against the corpus rather
//! than assumed. Prints, per manuscript, what each branch decides and what the
//! manuscript actually says.
use gaply_core::extract::{self, docparse, SectionKind};

const COI_SINGULAR: &str = "conflict of interest";
const COI_SYNONYMS: &[&str] = &[
    "competing interest", "conflict of interest", "conflicts of interest",
    "declaration of interest", "disclosure statement",
];

fn sentence(lower: &str, original: &str, names: &[&str]) -> Option<String> {
    let at = names.iter().filter_map(|n| lower.find(n)).min()?;
    let s = original[..at].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
    let e = original[at..].find(['.', '\n']).map(|i| at + i + 1).unwrap_or(original.len());
    Some(original[s..e].trim().chars().take(96).collect())
}

fn main() {
    let (mut differ, mut total) = (0usize, 0usize);
    let (mut abs_yes, mut num_pass) = (0usize, 0usize);
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let lower = text.to_lowercase();
        let ex = extract::extract_from_text(&text);
        total += 1;

        let shipped = lower.contains(COI_SINGULAR);
        let with_syn = sentence(&lower, &text, COI_SYNONYMS);
        let has_abstract = ex.sections.iter().any(|s| s.kind == SectionKind::Abstract);
        if has_abstract { abs_yes += 1 }
        let refs = ex.references.len();
        let numbered = ex.references.iter().filter(|r| {
            let t = r.raw.trim_start();
            t.starts_with(|c: char| c.is_ascii_digit()) || t.starts_with('[')
        }).count();
        let vanc = refs > 0 && numbered == refs;
        if vanc { num_pass += 1 }

        let flag = if shipped != with_syn.is_some() { differ += 1; "  <-- BRANCHES DISAGREE" } else { "" };
        println!("\n######## {name}{flag}");
        println!("  COI shipped(contains singular)={shipped}   with synonyms={}", with_syn.is_some());
        if let Some(s) = &with_syn { println!("     sentence: {s}"); }
        println!("  structured abstract: passes={has_abstract} (checks only that an Abstract SECTION exists)");
        println!("  vancouver: {numbered}/{refs} numbered -> passes={vanc}");
    }
    println!("\n  manuscripts {total};  COI branches disagree on {differ}");
    println!("  would PASS 'structured abstract': {abs_yes};  would PASS 'vancouver': {num_pass}");
}
