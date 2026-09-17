//! Print the sentence any one marker was keyed on, for one manuscript.
use gaply_core::extract::docparse;
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let needle = a.next().unwrap().to_lowercase();
    let text = docparse::parse_path(std::path::Path::new(&path)).unwrap();
    let lower = text.to_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower[from..].find(&needle) {
        let i = from + at;
        let s = lower[..i].rfind(['.', '\n']).map(|x| x + 1).unwrap_or(0);
        let e = lower[i..].find(['.', '\n']).map(|x| i + x).unwrap_or(lower.len());
        println!("  :: {}", lower[s..e].trim().chars().take(320).collect::<String>());
        from = i + needle.len();
    }
}
