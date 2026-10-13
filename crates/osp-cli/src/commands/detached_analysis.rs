//! #204 (Karar-A + tur-4 P0): detached-worktree ölçüm hazırlığı — ORTAK yol.
//!
//! Sözleşme (issue #204 karar kaydı):
//! `MeasuredBaseline = Analyze(Checkout(SHA_base))`, `CandidateChanges ∉ MeasuredBaseline`.
//!
//! Kirli çalışma ağacında üretilen ölçüm koşu başında capture edilen **sabit
//! commit SHA**'dan açılan detached worktree'den gelir; aday içeriği (worktree'deki
//! kirli dosyalar + untracked yeni dosyalar) ölçüme ASLA sızmaz. Bu modül ölçüm
//! hazırlığını TEK yerde tutar ve üç tüketici aynı yolu kullanır:
//!
//! - `trajectory attempt` (opt-in `--allow-declared-dirty`) — baseline analizi,
//! - `trajectory resume` (originating fence `mode=declared_dirty` devralması),
//! - `realization_gate` — kendi candidate worktree'i (tur-4 P0: gate'in
//!   before/after analizi submodule-aware olmadan #203 m.5b digest-parity geçerli
//!   witnessed-resume'u yanlış RED'lerdi).
//!
//! **Submodule sözleşmesi (tur-3 P1-1 + tur-4 P1-1):** worktree checkout'u
//! gitlink'leri işaret eder ama içerik init ETMEZ. `submodule update --init
//! --checkout --recursive` pinned gitlink'lerde her seviyeyi açar; ardından
//! `submodule status --recursive` tam-doğrulaması koşar: her satırda YALNIZ
//! boşluk-öneki (temiz, beklenen commit'te) kabul edilir — `+` (sapma), `-`
//! (init'siz), `U` (conflict) ve bilinmeyen önekler RED. Status çıktısı TRİM'siz
//! okunur (temiz durumun işareti BAŞTAKİ boşluktur; trim'li yardımcı ilk satırın
//! önekini yutup fail-open yapardı — #155 parse sınıfı).
//!
//! **Yaşam döngüsü:** RAII guard — her çıkış yolunda (analiz hatası, fence red,
//! erken dönüş) worktree kaldırılır; `remove()` hatası uyarıya düşer (çalışma
//! ağacı etkilenmez; operatör `git worktree prune` ile temizler). Process crash
//! penceresi bilinçle açık kalır (dürüst sınır; otomatik temizlik iddiası yok).

use std::path::{Path, PathBuf};

use osp_analyzer::contract::AnalysisConfig;
use osp_analyzer::language::AdapterRegistry;
use osp_analyzer::pipeline::analyze_repo_with_config;

/// Detached worktree RAII guard'ı — `prepare_detached_worktree` üretir.
///
/// `path()` analiz için worktree konumunu verir; `remove()` erken bırakma
/// (analiz bitti, uzay artık bellekte). Drop güvenlik ağıdır: çıkış yollarında
/// worktree'yi ortada bırakmaz; removal hatası stderr uyarısıyla raporlanır
/// (koşu başarısızlaştırılmaz — ölçüm zaten bellekte güvenlidir).
pub struct DetachedWorktreeGuard {
    repo: PathBuf,
    pub worktree_dir: PathBuf,
    path: PathBuf,
    removed: bool,
}

impl std::fmt::Debug for DetachedWorktreeGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetachedWorktreeGuard")
            .field("path", &self.path)
            .field("removed", &self.removed)
            .finish()
    }
}

impl DetachedWorktreeGuard {
    /// Analizin koşacağı worktree konumu.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Worktree'i şimdi bırak (Drop'u beklemeden). Hata stderr'e uyarı olarak
    /// düşer — geçici temizlik hatası üretilmiş ölçümü geçersiz kılmaz.
    pub fn remove(&mut self) {
        if self.removed {
            return;
        }
        self.removed = true;
        if let Err(detail) = remove_worktree(&self.repo, &self.path, &self.worktree_dir) {
            eprintln!(
                "warning: could not remove detached analysis worktree {}: {detail} — \
                 run `git -C {} worktree prune` to clean up",
                self.path.display(),
                self.repo.display()
            );
        }
    }
}

impl Drop for DetachedWorktreeGuard {
    fn drop(&mut self) {
        self.remove();
    }
}

fn remove_worktree(repo: &Path, worktree: &Path, dir: &Path) -> Result<(), String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(worktree)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

/// #204: detached worktree aç + submodule'leri pinned gitlink'lerde hazırla.
///
/// `sha` mutlaka KOŞU BAŞINDA capture edilmiş tam 40-hex commit kimliği olmalı
/// (hareketli ref DEĞİL): başka bir süreç branch'i ilerletse bile ölçüm kaynağı
/// değişmez; HEAD hareketi post-fence'in HEAD-eşitlik kontrolünde RED'lenir.
pub fn prepare_detached_worktree(repo: &Path, sha: &str) -> anyhow::Result<DetachedWorktreeGuard> {
    let worktree_dir = std::env::temp_dir().join(format!(
        "osp-detached-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&worktree_dir)
        .map_err(|e| anyhow::anyhow!("cannot create detached-analysis tempdir: {e}"))?;
    let worktree_path = worktree_dir.join("analysis");

    // (a) Worktree @ sabit SHA. Başarısızlıkta guard HENÜZ yok — temp'i burada
    // temizle (worktree kaydı oluşmadıysa prune gereksizdir; oluştuysa add'in
    // kendi hatası zaten kısmi durumu git'e bildirir).
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(&worktree_path)
        .arg(sha)
        .output()
        .map_err(|e| anyhow::anyhow!("git worktree add failed to spawn: {e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&worktree_dir);
        anyhow::bail!(
            "detached worktree creation failed at {sha}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let guard = DetachedWorktreeGuard {
        repo: repo.to_path_buf(),
        worktree_dir: worktree_dir.clone(),
        path: worktree_path.clone(),
        removed: false,
    };

    // (b)+(c) Submodule hazırlığı — guard'ın İÇİNDE: herhangi bir adım patlarsa
    // Drop worktree'yi kaldırır (kullanıcı çalışma ağacına dokunulmaz).
    initialize_and_verify_submodules(&worktree_path, sha)?;
    Ok(guard)
}

/// Submodule'leri pinned gitlink'lerde aç ve durumlarını doğrula (tur-3 P1-1).
fn initialize_and_verify_submodules(worktree: &Path, sha: &str) -> anyhow::Result<()> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--checkout")
        .arg("--recursive")
        .output()
        .map_err(|e| anyhow::anyhow!("git submodule update failed to spawn: {e}"))?;
    anyhow::ensure!(
        out.status.success(),
        "submodule initialization failed in detached worktree at {sha}: {} — \
         cannot guarantee the measured HEAD space (init needs the pinned gitlink \
         commits; missing network/cache is a refusal, not a degraded measurement)",
        String::from_utf8_lossy(&out.stderr)
    );
    verify_submodule_status(worktree, sha)
}

/// `git submodule status --recursive` tam-doğrulaması.
///
/// P2-1 (tur-3): çıktı HAM okunur (yalnız satır sonları işlenir) — temiz durumun
/// işareti satırın İLK karakterindeki BOŞLUKTUR; `trim()` baştan yutup ilk
/// satırı her durumda "temiz" gösterirdi (fail-open). Kabul edilen tek önek
/// boşluktur: `+` sapma, `-` init'siz, `U` conflict, diğerleri bilinmeyen.
fn verify_submodule_status(worktree: &Path, sha: &str) -> anyhow::Result<()> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .arg("submodule")
        .arg("status")
        .arg("--recursive")
        .output()
        .map_err(|e| anyhow::anyhow!("git submodule status failed to spawn: {e}"))?;
    anyhow::ensure!(
        out.status.success(),
        "submodule status failed in detached worktree at {sha}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let raw = String::from_utf8_lossy(&out.stdout);
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        classify_submodule_status_line(line).map_err(|detail| {
            anyhow::anyhow!("submodule not at its pinned gitlink at {sha}: {detail}")
        })?;
    }
    Ok(())
}

/// Tek bir `git submodule status` satırını sınıfla (saf fonksiyon — trim yutma
/// tuzağı pin'lenir). Satır biçimi: `<önek><sha> <path> (açıklama)`; önek:
/// `' '` temiz/beklenen-commit · `+` sapma · `-` init'siz · `U` conflict.
pub(crate) fn classify_submodule_status_line(line: &str) -> Result<(), String> {
    let prefix = line.as_bytes()[0] as char;
    match prefix {
        ' ' => Ok(()),
        '+' => Err(format!(
            "{}: the checked-out commit differs from the gitlink",
            &line[1..]
        )),
        '-' => Err(format!("{}: the submodule is not initialized", &line[1..])),
        'U' => Err(format!("{}: the submodule has merge conflicts", &line[1..])),
        other => Err(format!(
            "{}: unknown submodule state prefix {other:?}",
            &line[1..]
        )),
    }
}

/// #204: detached ölçüm — sabit SHA worktree'inde, attempt akışıyla BİREBİR aynı
/// inşaatla analiz (mod.rs canlı-analiz akışının aynısı; #203 m.5b parite
/// sözleşmesinin #204 karşılığı). Uzay belleğe alındıktan sonra worktree bırakılır.
pub fn analyze_detached(
    repo: &Path,
    sha: &str,
) -> anyhow::Result<osp_analyzer::contract::AnalysisResult> {
    let mut guard = prepare_detached_worktree(repo, sha)?;
    let registry = AdapterRegistry::default_all();
    let config = AnalysisConfig::default();
    let result = analyze_repo_with_config(guard.path(), &registry, &config)
        .map_err(|e| anyhow::anyhow!("detached analysis failed at pinned HEAD {sha}: {e}"))?;
    guard.remove();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gerçek mini git repo (tek commit) — guard yaşam döngüsü gerçek `git
    /// worktree` kayıtları üzerinden doğrulanır.
    fn git_repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@t"]);
        run(&["config", "user.name", "t"]);
        std::fs::write(dir.path().join("a.rs"), "fn main() {}\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-qm", "init"]);
        let head = run(&["rev-parse", "HEAD"]);
        (dir, head)
    }

    fn worktree_list(repo: &Path) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("worktree")
            .arg("list")
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// P2-1 (tur-3): status satırı sınıflayıcı — temiz durumun işareti BAŞTAKİ
    /// boşluktur; trim'li okuma ilk satırı her durumda "temiz" gösterirdi.
    #[test]
    fn submodule_status_classifier_pins_the_leading_space_prefix() {
        classify_submodule_status_line(" 0123456789abcdef0123456789abcdef01234567 mod (v1)")
            .expect("clean submodule (space prefix) must pass");
        for (line, hint) in [
            (
                "+0123456789abcdef0123456789abcdef01234567 mod (v1)",
                "differs from the gitlink",
            ),
            (
                "-0123456789abcdef0123456789abcdef01234567 mod",
                "not initialized",
            ),
            (
                "U0123456789abcdef0123456789abcdef01234567 mod",
                "merge conflicts",
            ),
            (
                "X0123456789abcdef0123456789abcdef01234567 mod",
                "unknown submodule state",
            ),
        ] {
            let err =
                classify_submodule_status_line(line).expect_err("non-clean prefixes must reject");
            let msg = err.to_string();
            assert!(msg.contains(hint), "{line}: {msg}");
        }
    }

    /// RAII: guard düştüğünde worktree kaydı temizlenir (analiz hatası/erken
    /// dönüş worktree'yi ortada bırakmaz; kullanıcı ağacına dokunulmaz).
    #[test]
    fn guard_drop_removes_the_worktree() {
        let (dir, head) = git_repo();
        let before = worktree_list(dir.path());
        {
            let guard = prepare_detached_worktree(dir.path(), &head)
                .expect("worktree at a valid pinned HEAD must prepare");
            assert!(
                guard.path().join("a.rs").exists(),
                "checkout content present"
            );
            let during = worktree_list(dir.path());
            assert!(
                during.lines().count() == before.lines().count() + 1,
                "worktree registered: {during}"
            );
        } // Drop — RAII temizliği.
        let after = worktree_list(dir.path());
        assert_eq!(
            after, before,
            "drop must restore the original worktree list"
        );
    }

    /// Add-başarısızlığı: geçersiz SHA → RED + worktree listesi DEĞİŞMEZ
    /// (yarım kayıt kalması yok).
    #[test]
    fn guard_add_failure_leaves_no_worktree_behind() {
        let (dir, _head) = git_repo();
        let before = worktree_list(dir.path());
        let err = prepare_detached_worktree(dir.path(), "0".repeat(40).as_str())
            .expect_err("nonexistent pinned SHA must fail");
        assert!(
            format!("{err}").contains("detached worktree creation failed"),
            "{err}"
        );
        assert_eq!(
            worktree_list(dir.path()),
            before,
            "failed add must not leave a worktree registered"
        );
    }

    /// Remove-başarısızlığı: worktree dizini önceden gittiğinde remove() uyarıya
    /// düşer (panik YOK — üretilmiş ölçüm geçersizleşmez); prune kaydı temizler.
    #[test]
    fn guard_remove_failure_degrades_to_a_warning_not_a_panic() {
        let (dir, head) = git_repo();
        let mut guard = prepare_detached_worktree(dir.path(), &head).unwrap();
        // Worktree içeriğini dışarıdan yok et — `git worktree remove` artık
        // başarısız olacak; remove() bunu uyarı olarak raporlamalı.
        std::fs::remove_dir_all(guard.path()).unwrap();
        guard.remove(); // panic etmemeli
        let prune = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .arg("worktree")
            .arg("prune")
            .output()
            .unwrap();
        assert!(prune.status.success());
    }

    // NOT (bilinçli test-kapsamı kararı): "detached == init'li canlı analiz"
    // SpaceDigest paritesinin birim karşılığı, işleme-içi `file://` protokol
    // ortamı gerektirdiğinden (aracın güvenlik varsayınını gevşatmamak için
    // env yalnız CAĞIRANDAN gelir) E2E'de yaşar: submodule fixture'ında
    // attempt → resume → realization-gate zinciri gate'in digest-parity
    // fence'ini canlı koşar (completed_loop.rs, #204 tur-4 P0 regresyonu).
}
