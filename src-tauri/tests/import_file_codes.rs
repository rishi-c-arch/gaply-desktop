//! Decision 231: `read_import_file` answers a file of another type and a file
//! over its cap with closed codes, `import.unsupported_type` and
//! `import.too_large`, in the JSON shape of a Stored data refusal, and no
//! longer with messages that carry the file's extension or size.

use app_lib::commands::read_import_file;
use gaply_core::GaplyError;

/// A folder this test makes and removes.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p =
            std::env::temp_dir().join(format!("gaply-5efix2-import-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The wire's message, as the webview reads it, and the code inside it.
fn code_of(e: &GaplyError) -> (String, String) {
    let wire = serde_json::to_value(e).unwrap();
    assert_eq!(wire["code"], "validation");
    let message = wire["message"].as_str().unwrap().to_string();
    let inner: serde_json::Value = serde_json::from_str(&message)
        .unwrap_or_else(|_| panic!("the message is not a closed code: {message}"));
    (message, inner["code"].as_str().unwrap().to_string())
}

#[test]
fn a_file_of_another_type_answers_import_unsupported_type() {
    let s = Scratch::new("type");
    for name in [
        "SENTINEL_IMPORT_a.txt",
        "SENTINEL_IMPORT_b",
        "SENTINEL_IMPORT_c.docx",
    ] {
        let p = s.0.join(name);
        std::fs::write(&p, "@article{x, title={y}}").unwrap();
        let e = read_import_file(p.to_string_lossy().into_owned()).unwrap_err();
        let (message, code) = code_of(&e);
        assert_eq!(code, "import.unsupported_type");
        assert_eq!(message, r#"{"code":"import.unsupported_type"}"#);
    }
}

#[test]
fn a_file_over_the_cap_answers_import_too_large() {
    let s = Scratch::new("size");
    let p = s.0.join("SENTINEL_IMPORT_big.bib");
    let f = std::fs::File::create(&p).unwrap();
    // Sparse: 16 MiB and one byte, never written.
    f.set_len(16 * 1024 * 1024 + 1).unwrap();
    drop(f);
    let e = read_import_file(p.to_string_lossy().into_owned()).unwrap_err();
    let (message, code) = code_of(&e);
    assert_eq!(code, "import.too_large");
    assert_eq!(message, r#"{"code":"import.too_large"}"#);
    // At the cap exactly, it is read.
    let at = s.0.join("at.bib");
    std::fs::write(&at, "").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&at)
        .unwrap()
        .set_len(16 * 1024 * 1024)
        .unwrap();
    assert!(read_import_file(at.to_string_lossy().into_owned()).is_ok());
}

#[test]
fn an_import_file_of_each_type_is_still_read() {
    let s = Scratch::new("read");
    for name in ["a.bib", "b.BIBTEX", "c.ris", "d.json"] {
        let p = s.0.join(name);
        std::fs::write(&p, "text").unwrap();
        assert_eq!(
            read_import_file(p.to_string_lossy().into_owned()).unwrap(),
            "text"
        );
    }
}
