//! **Which manuscripts does `ml_methodology`'s gate admit, and on what words?**
//!
//! The gate is `>= 2 of 18 ML_TERMS appear anywhere in the body`. §11 D163 and
//! the journal classifier both failed the same way: a vocabulary that appears
//! everywhere on the target corpus. This prints the MATCHED TERM WITH ITS
//! SENTENCE for every manuscript, admitted or not, so the decision can be read
//! rather than inferred from a count.
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::{self, SpecialistInput};

const ML_TERMS: &[&str] = &[
    "machine learning", "deep learning", "neural network", "training set",
    "training data", "test set", "classifier", "lstm", "transformer",
    "random forest", "gradient boosting", "xgboost", "support vector",
    "convolutional", "embedding", "fine-tun", "epochs", "hyperparameter",
];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut admitted, mut total) = (0usize, 0usize);
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        total += 1;
        let ex = extract::extract_from_text(&text);
        let lower = text.to_lowercase();

        // Which terms match, and the sentence each sits in.
        let mut hits: Vec<(&str, String)> = Vec::new();
        for t in ML_TERMS {
            if let Some(at) = lower.find(t) {
                let s = lower[..at].rfind(['.', '\n']).map(|i| i + 1).unwrap_or(0);
                let e = lower[at..].find(['.', '\n']).map(|i| at + i).unwrap_or(lower.len());
                hits.push((t, lower[s..e].trim().chars().take(105).collect()));
            }
        }

        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let ml = specialist::shipped();
        let ml = ml.iter().find(|s| s.id() == "ml_methodology").unwrap();
        let r = specialist::run(ml.as_ref(), &sin);
        let ran = r.not_applicable.is_none();
        if ran {
            admitted += 1;
        }

        println!(
            "\n{} {}  [{} term(s)]",
            if ran { "ADMITTED " } else { "declined  " },
            name,
            hits.len()
        );
        for (t, s) in hits.iter().take(4) {
            println!("    {t:<18} :: {s}");
        }
        if ran {
            for f in &r.admitted {
                println!("    -> FINDING {} :: {}", f.code,
                    f.span.as_deref().unwrap_or("(no span)").chars().take(80).collect::<String>());
            }
        }
    }
    println!("\n  admitted {admitted} of {total}");
}
