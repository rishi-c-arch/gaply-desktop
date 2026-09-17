//! Print, in full, every finding `claim_evidence_strength` now admits.
//! A check that starts firing is a check whose first firing must be READ.
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::{self, claim_strength::ClaimStrengthSpecialist, SpecialistInput};

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let input = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let r = specialist::run(&ClaimStrengthSpecialist, &input);
        for f in &r.admitted {
            println!("\n######## {name}");
            println!("  code     : {}", f.code);
            println!("  severity : {:?}", f.severity);
            println!("  location : {:?}", f.location);
            println!("  summary  : {}", f.summary);
            println!("  SPAN:\n{}", f.span.as_deref().unwrap_or("<none>"));
        }
        for rej in &r.rejected {
            println!("\n######## {name}  REJECTED BY POLICY: {}", rej.reason);
        }
    }
}
