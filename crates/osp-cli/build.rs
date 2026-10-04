//! #172 (K7): OSP checkout HEAD'ini build zamanına göm.
//!
//! `finalize-run` ledger satırındaki `osp_revision` alanının kaynağı. Runtime
//! `rev-parse` binary'nin kaynak checkout'undan bağımsız bir çalışma dizini
//! görebilirdi (transcription sınıfı buraya da sızar); build-time gömüm binary'nin
//! derlendiği revizyonu dürüstçe bağlar. Git yoksa (eksik checkout) "unknown" —
//! finalize-run bu değeri olduğu gibi yazar, uydurmaz.

fn main() {
    // crates/osp-cli → workspace root .git/HEAD. HEAD dosyası commit/checkout'ta
    // değişir → cargo build script'i yeniden koşar, gömülü SHA tazelenir.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=OSP_GIT_REVISION={sha}");
}
