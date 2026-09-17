//! Candidate signals for "is this a paper REPORTING a trained model", measured
//! against the 20 before any gate is changed. Vocabulary count is the signal
//! that failed; these are the alternatives.
use gaply_core::extract::docparse;
use regex::Regex;

const ML_TERMS: &[&str] = &[
    "machine learning", "deep learning", "neural network", "training set",
    "training data", "test set", "classifier", "lstm", "transformer",
    "random forest", "gradient boosting", "xgboost", "support vector",
    "convolutional", "embedding", "fine-tun", "epochs", "hyperparameter",
];

fn main() {
    // A performance metric REPORTED WITH A NUMBER — the thing a paper that
    // trained a model has and a paper that discusses AI does not.
    let perf = Regex::new(
        r"(?i)\b(accuracy|f1[- ]?score|f1|auc|auroc|precision|recall|rmse|mae)\b[^.\n]{0,40}?\b\d{1,3}(\.\d+)?\s*%?",
    )
    .unwrap();
    // A split/training artefact: a paper that trained one says how.
    let split = Regex::new(r"(?i)\b(train(ing)?[ /-]?(and[ /-]?)?test split|cross[- ]validation|k[- ]fold|held[- ]out|train/test)\b").unwrap();

    println!("{:<50} {:>6} {:>6} {:>7}", "manuscript", "terms", "perf", "split");
    let (mut a_terms, mut a_perf, mut a_both) = (0usize, 0usize, 0usize);
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let lower = text.to_lowercase();
        let terms = ML_TERMS.iter().filter(|t| lower.contains(**t)).count();
        let nperf = perf.find_iter(&text).count();
        let nsplit = split.find_iter(&text).count();
        if terms >= 2 { a_terms += 1; }
        if nperf > 0 { a_perf += 1; }
        if terms >= 2 && nperf > 0 && nsplit > 0 { a_both += 1; }
        println!("{name:<50} {terms:>6} {nperf:>6} {nsplit:>7}");
    }
    println!("\n  terms>=2 (TODAY'S GATE)                  {a_terms}");
    println!("  any performance metric with a number     {a_perf}");
    println!("  terms>=2 AND perf AND split              {a_both}");
}
