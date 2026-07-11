//! Set-5 memory proof: a large supplementary file is REJECTED without being
//! read into memory. Stream-creates a 120MB CSV (creation bounded to a ~1MB
//! chunk buffer) then parses it; the size cap rejects it via metadata alone,
//! so peak RSS stays tiny. Run under `/usr/bin/time -l` to see peak footprint.

use std::io::Write;

use app_lib::supplementary::{parse_supplementary, MAX_FILE_SIZE_BYTES};

fn main() {
    let path = std::env::temp_dir().join("gaply_supp_mem_probe.csv");
    {
        let mut f = std::fs::File::create(&path).expect("create");
        let chunk = "1,2,3,4,5,6,7,8,9,10\n".repeat(50_000); // ~1 MB
        for _ in 0..120 {
            f.write_all(chunk.as_bytes()).expect("write");
        }
    }
    let size = std::fs::metadata(&path).unwrap().len();
    println!("file size: {} MB (cap {} MB)", size / (1024 * 1024), MAX_FILE_SIZE_BYTES / (1024 * 1024));

    match parse_supplementary(&path) {
        Ok(_) => {
            eprintln!("UNEXPECTED: parsed a file that should be rejected");
            let _ = std::fs::remove_file(&path);
            std::process::exit(1);
        }
        Err(e) => println!("REJECTED cleanly (no full load): {e}"),
    }
    let _ = std::fs::remove_file(&path);
}
