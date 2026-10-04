//! #172 (K7, tur-1 P1-2 ile sıkılaştırıldı): OSP checkout HEAD'ini build zamanına göm.
//!
//! `finalize-run` ledger satırındaki `osp_revision` alanının kaynağı. Runtime
//! `rev-parse` binary'nin kaynak checkout'undan bağımsız bir çalışma dizini
//! görebilirdi (transcription sınıfı buraya da sızar); build-time gömüm binary'nin
//! derlendiği revizyonu dürüstçe bağlar. Git yoksa (eksik checkout) "unknown" —
//! finalize-run bu değeri olduğu gibi yazar, uydurmaz.
//!
//! **Watch kümesi (P1-2):** `.git/HEAD` tek başına yetmez — aynı branch'te yeni
//! commit çoğunlukla yalnız `.git/refs/heads/<branch>` (veya packed-refs)
//! değiştirir; `HEAD` dosyası `ref: refs/heads/main` olarak kalır ve incremental
//! build eski SHA'yı taşımaya devam eder. Bu yüzden aşağıdaki dosyalar watch
//! edilir; bir ref güncellemesi bunlardan birine dokunur → build script yeniden
//! koşar → gömülü SHA tazelenir:
//!
//! - `git rev-parse --git-path HEAD` (linked worktree'de worktree-özel dosya),
//! - HEAD symbolic ise `git rev-parse --git-common-dir` altındaki gerçek ref
//!   dosyası (`refs/heads/<branch>`) ve `packed-refs`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    watch_head_files(&manifest);
    let sha = current_head(&manifest);
    println!("cargo:rustc-env=OSP_GIT_REVISION={sha}");
}

/// Cargo'ya HEAD'i belirleyen GERÇEK dosyaları rerun-if-changed olarak bildir.
fn watch_head_files(manifest: &Path) {
    let Some(git_dir) = git_output(manifest, &["rev-parse", "--git-path", "HEAD"]) else {
        return; // git yok — revizyon "unknown" kalır, watch edilecek şey de yok
    };
    let head_path = resolve_against(manifest, &git_dir);
    if head_path.is_file() {
        println!("cargo:rerun-if-changed={}", head_path.display());
    }
    // HEAD symbolic ise ("ref: refs/heads/x") gerçek ref dosyası common dir'dedir
    // (linked worktree'lerde refs paylaşılır; HEAD worktree-özel olabilir).
    let Ok(head_content) = std::fs::read_to_string(&head_path) else {
        return;
    };
    let Some(refname) = head_content.trim().strip_prefix("ref: ") else {
        return; // detached HEAD: HEAD dosyasının kendisi SHA'yı taşır, yeterli
    };
    if let Some(common_dir) = git_output(manifest, &["rev-parse", "--git-common-dir"]) {
        let common = resolve_against(manifest, &common_dir);
        let ref_file = common.join(refname.trim());
        if ref_file.is_file() {
            println!("cargo:rerun-if-changed={}", ref_file.display());
        }
        let packed = common.join("packed-refs");
        if packed.is_file() {
            println!("cargo:rerun-if-changed={}", packed.display());
        }
    }
}

/// HEAD SHA'sını al (fail-open: git yoksa/hata ise "unknown").
fn current_head(manifest: &Path) -> String {
    git_output(manifest, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string())
}

/// `git -C <manifest> <args>` stdout'unu trim'li döndürür; hata durumunda None.
fn git_output(manifest: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(manifest)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// `git rev-parse --git-path` gibi komutlar göreli path döndürebilir (".git/HEAD")
/// — manifest dizinine bağlayıp absolute'e çevirir.
fn resolve_against(manifest: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        manifest.join(p)
    }
}
