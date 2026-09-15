//! **The hand-check behind §11 D165: what fraction of `Method` objects are real?**
//!
//! Prints every `Method` the scientific layer produces, with the first words of
//! the paragraph it is anchored to. **Every row, not a sample** — the D165
//! adjudication is a reading of all 152 objects across the six real
//! manuscripts, and this is the instrument that produced them.
//!
//! ```text
//! span is a genuine method statement          9 / 152   5.9%
//!   ... and `design` is correct               3 / 152   2.0%
//!   ... and `n` and `software` are too        1 / 152   0.66%
//! NO-SKILL (first paragraph of each Methods
//!           section, 12 guesses)              6 /  12   50%
//! ```
//!
//! **The no-skill row is the one that decided it.** A lane is not withdrawable
//! because its precision is low; it is withdrawable because something trivial
//! does better. D128 computed one for `citation_need` after three measurements
//! that had not, and it was the only thing that showed there was nothing there.
//!
//! Run it and read the rows. The failures are categorical and visible at a
//! glance: Turnitin page footers, table data rows, hyperparameter cells, the
//! manuscript title, the Keywords line, an Authorship Contribution Statement
//! whose `software: ["R"]` is the author's middle initial.
//!
//! ```text
//! cargo run --release --example methods_precision_probe -- <manuscript>...
//! ```

use gaply_core::extract::{self, docparse, paragraph_at, Location};
use gaply_core::scientific_model::SourceSpan;
fn main(){
    for p in std::env::args().skip(1){
        let Ok(t)=docparse::parse_path(std::path::Path::new(&p)) else {continue};
        let r=extract::extract_from_text_with(&t, extract::ExtractOptions::with_scientific());
        let Some(s)=&r.scientific else {continue};
        println!("\n##### {}  ({} methods)", std::path::Path::new(&p).file_name().unwrap().to_string_lossy(), s.methods.len());
        for (i,m) in s.methods.iter().enumerate(){
            let span = m.source_spans.first().map(|sp| match sp {
                SourceSpan::Point(l)=> paragraph_at(&r,l).map(|x|{
                    let w:Vec<&str>=x.split_whitespace().take(18).collect();
                    format!("[{:?} ¶{}] {}", l.section, l.paragraph, w.join(" "))
                }).unwrap_or_else(||format!("[{:?} ¶{}] <UNRESOLVED>", l.section, l.paragraph)),
                SourceSpan::Range(sp)=>format!("[{:?} ¶{}..]", sp.section, sp.start_paragraph),
            }).unwrap_or_else(||"<no span>".into());
            println!("{i:>3} {:<28} n={:<6} sw={:<14} | {}",
                format!("{:?}",m.design), format!("{:?}",m.sampling.size),
                format!("{:?}",m.software), span);
        }
        let _ = Location{section:gaply_core::extract::SectionKind::Other,paragraph:0};
    }
}
