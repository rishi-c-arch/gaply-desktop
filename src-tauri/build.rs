fn main() {
    // Record the EXACT compiler that built this binary, for the eval reports.
    //
    // §11 D61: a 2.65x CPU prefill regression against the D34 baseline had to be
    // attributed between the OS and the toolchain, and the reports recorded
    // `binaryHash`, `nCtx` and load context but not `rustc`. Closing it needed
    // forensics on `~/.rustup` directory mtimes to prove the toolchain had never
    // been updated. That evidence is circumstantial and it expires the first
    // time anyone runs `rustup update`.
    //
    // `RUSTC` is set by cargo to the compiler actually invoked, so this is the
    // real one — NOT whatever `rustc` happens to be on PATH at runtime, which is
    // a different question and the wrong answer.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GAPLY_BUILD_RUSTC={version}");
    println!("cargo:rerun-if-env-changed=RUSTC");

    tauri_build::build()
}
