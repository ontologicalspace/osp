//! Node-level witness extraction — git history → per-file evidence (§3.2).
//!
//! Repo-level `WitnessSet` (claim/PR onayı, `osp_core::witness`) **node-level**
//! karşılığı. Her source file'ın git history'sinden "battle-tested vs
//! speculative" kanıtı çıkarır: kaç commit dokundu, kaç yazar, son değişiklik
//! ne zaman, ne kadar churn, sahiplik ne kadar konsantre.
//!
//! **Performans yaklaşımı:** tek `git log --numstat --format=...` subprocess
//! tüm repo history'yi tek pas'ta tarar. svelte (3448 node, ~20k commit)
//! ölçeğinde saniyeler mertebesinde. On-demand (per-node git blame) değil —
//! `analyze_repo` çıktısının parçası, böylece scatter coloring (churn=color)
//! mümkün.
//!
//! **Nullabilite:** `.git` yoksa `repo_has_git=false` → tüm `by_file` boş.
//! Pipeline bu durumda NodeWitness'ı None bırakır (SCIP-yok gibi graceful).

use std::collections::HashMap;
use std::path::Path;

use osp_core::space::NodeWitness;

/// Tüm repo için node-level witness profili — `git log` aggregate çıktısı.
///
/// `by_file` anahtarları repo-relative forward-slash path'lerdir (pipeline'ın
/// `node_paths` key'leri ile aynı normalizasyon). `repo_has_git=false` iken
/// `by_file` her zaman boştur.
#[derive(Debug, Clone, Default)]
pub struct WitnessProfile {
    /// Repo-relative path → NodeWitness.
    pub by_file: HashMap<String, NodeWitness>,
    /// Repo bir git checkout mı? `.git` yoksa `false` → tüm NodeWitness None.
    pub repo_has_git: bool,
    /// Analiz anındaki HEAD commit kısa hash'i (stale detection için).
    pub head_commit: Option<String>,
}

impl WitnessProfile {
    /// Boş profil — git yok durumu. `repo_has_git=false`.
    pub fn no_git() -> Self {
        Self {
            by_file: HashMap::new(),
            repo_has_git: false,
            head_commit: None,
        }
    }

    /// Belirli bir dosya için witness — yoksa `None`. Git yoksa her zaman `None`.
    pub fn for_file(&self, rel_path: &str) -> Option<&NodeWitness> {
        if !self.repo_has_git {
            return None;
        }
        self.by_file.get(rel_path)
    }
}

/// Bir reponun git history'sini tara → per-file NodeWitness profili.
///
/// Tek `git log --numstat --format=... --no-merges` pas'ı. Merge commit'ler
/// hariç (sahte churn önler — merge'ler genelde hiçbir satırı gerçekten
/// değiştirmez). Binary dosyalar (`-\t-\tpath`) skip edilir.
///
/// `.git` yoksa veya `git` çalışmazsa `WitnessProfile::no_git()` döner —
/// hata üretmez (SCIP-yok fallback ile paralel).
pub fn extract_witness(repo: &Path) -> WitnessProfile {
    // HEAD commit hash (stale detection + "is this a git repo" testi).
    let head_commit = match git_output(repo, &["rev-parse", "--short", "HEAD"]) {
        Ok(h) => Some(h.trim().to_string()),
        Err(_) => return WitnessProfile::no_git(),
    };

    // Tek pas: commit header + altındaki numstat satırları.
    // Format: "OSPSEP{hash}|{author}|{commit_date}" sonra her dosya için
    // "{added}\t{deleted}\t{path}" satırları.
    //
    // %cI (committer date) tercih edilir %aI (author date) yerine — "last
    // modified" için bir dosyanın en son commit'e girdiği an (rebase/cherry-pick
    // dahil) daha anlamlıdır. Author date rebase'te korunur → eski dosyalar
    // yanıltıcı şekilde "yıllar önce" görünür (svelte corpus'ta tespit edildi:
    // %aI mean 1359 gün, %cI çok daha yakın).
    let log = match git_output(
        repo,
        &[
            "log",
            "--numstat",
            "--format=OSPSEP%H|%an|%cI",
            "--no-merges",
            "-U0",
            "HEAD",
        ],
    ) {
        Ok(out) => out,
        Err(_) => return WitnessProfile::no_git(),
    };

    let aggregates = parse_log(&log);

    WitnessProfile {
        by_file: aggregates,
        repo_has_git: true,
        head_commit,
    }
}

/// `git log --numstat --format=OSPSEP...` çıktısını per-file aggregate'e çevir.
///
/// Parser: commit header (`OSPSEP...`) ile başlar, ardından 0+ numstat satırı
/// gelir. Her numstat satırı o commit'in dokunduğu bir dosyadır. Aggregate:
/// per-path commits_touching++, distinct_authors (set), churn += added+deleted,
/// last_modified = en yakın commit'in gün sayısı, ownership = en aktif yazarın payı.
///
/// `pub` değil — test edilebilirlik için `pub(super)`/`pub(crate)`. Testler
/// `#[cfg(test)]` içinde aynı modülden çağırır.
fn parse_log(log: &str) -> HashMap<String, NodeWitness> {
    // İki geçişli: önce per-path ham veri topla (yazar seti, commit listesi),
    // sonra NodeWitness'a dönüştür.
    #[derive(Default)]
    struct Agg {
        commits: usize,
        authors: std::collections::HashSet<String>,
        churn: u64,
        last_commit_days_ago: Option<u64>, // en küçük (en yakın) commit
        // commit'leri yazar'a saymak için (ownership concentration)
        author_commits: HashMap<String, usize>,
    }

    let mut by_file: HashMap<String, Agg> = HashMap::new();
    // HEAD commit zamanını "gün 0" referansı olarak al — ama git log kronolojik
    // ters (en yeni ilk). İlk commit header = en yeni = days_ago en küçük.
    // Basitlik için: her commit'in mutlak tarihini parse edip HEAD'e göre
    // fark hesaplamak yerine, commit sırasını kullanırız (en yeni = 0. pozisyon).
    // Bunun yerine aI (ISO date) parse edip gün farkı hesaplıyoruz — robust.
    let head_date = extract_head_date(log);

    let mut current_author = String::new();
    let mut current_date = String::new();
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("OSPSEP") {
            // Commit header: {hash}|{author}|{iso_date}
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                current_author = parts[1].to_string();
                current_date = parts[2].to_string();
            }
            continue;
        }
        // Numstat satırı: "{added}\t{deleted}\t{path}"
        // Binary: "-\t-\tpath" → skip.
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 3 {
            continue;
        }
        let added: u64 = match cols[0].parse() {
            Ok(n) => n,
            Err(_) => continue, // binary ("-") veya garbage → skip
        };
        let deleted: u64 = match cols[1].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let path = normalize_path(cols[2]);
        if path.is_empty() {
            continue;
        }

        let agg = by_file.entry(path).or_default();
        agg.commits += 1;
        agg.authors.insert(current_author.clone());
        agg.churn += added + deleted;
        *agg.author_commits
            .entry(current_author.clone())
            .or_insert(0) += 1;

        // last_modified: en yakın commit'in gün farkı (en küçük days_ago).
        if let Some(head) = &head_date {
            if let Some(days) = days_between(&current_date, head) {
                agg.last_commit_days_ago = Some(match agg.last_commit_days_ago {
                    Some(prev) => prev.min(days),
                    None => days,
                });
            }
        }
    }

    // Aggregate → NodeWitness. ownership_concentration = en aktif yazarın payı.
    by_file
        .into_iter()
        .map(|(path, agg)| {
            let ownership_concentration = if agg.commits > 0 {
                let max_author_commits = agg.author_commits.values().copied().max().unwrap_or(0);
                max_author_commits as f64 / agg.commits as f64
            } else {
                0.0
            };
            (
                path,
                NodeWitness {
                    commits_touching: agg.commits,
                    distinct_authors: agg.authors.len(),
                    last_modified_days_ago: agg.last_commit_days_ago.unwrap_or(0) as u32,
                    churn: agg.churn,
                    ownership_concentration,
                },
            )
        })
        .collect()
}

/// Path'i repo-relative forward-slash normalizasyonuna çevir.
/// `git log --numstat` çıktısı bazen `./foo/bar` veya `{old} => {new}` (rename)
/// formatında gelebilir. Rename'i `=>`'den sonra alırız (new path). Absolute
/// Windows path'leri (nadiren) normalize edilir.
fn normalize_path(raw: &str) -> String {
    let trimmed = raw.trim();
    // Rename: "old/path => new/path" → new path al.
    let path = if let Some(idx) = trimmed.rfind("=>") {
        trimmed[idx + 2..]
            .trim()
            .trim_matches('{')
            .trim_matches('}')
    } else {
        trimmed
    };
    // "./" prefix'i kaldır.
    let stripped = path.strip_prefix("./").unwrap_or(path);
    // Backslash → forward slash (Windows).
    stripped.replace('\\', "/")
}

/// `git log` çıktısındaki ilk commit header'dan (en yeni = HEAD) ISO date çıkar.
/// Bu, her commit'in "gün önce" değerini hesaplamak için referans.
fn extract_head_date(log: &str) -> Option<String> {
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("OSPSEP") {
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                return Some(parts[2].to_string());
            }
        }
    }
    None
}

/// İki ISO-8601 tarih arasındaki **gün** farkı (`earlier`, `later`). `later` daha
/// yeni varsayılır; negatif olmaz (clamp 0). Parse başarısızsa `None`.
///
/// Sadece `YYYY-MM-DD` kısmını alır (timezone offset + saat/dk/sn hariç — gün
/// hassasiyeti yeterli; timezone farkı ±1 günü aşmaz ve ±1 gün "days_ago"
/// için tolere edilebilir).
///
/// Önemli: `days_from_civil` zaten **gün** döndürür (epoch'tan itibaren gün).
/// Bu yüzden burada ek bölme YOK — eski implementasyon `/ 86_400` yapıyordu
/// ki bu çift bölmeydi (gün / saniye = 0). Bug tespiti: svelte corpus'ta tüm
/// dosyalar "0 gün önce" çıkıyordu (12 gün / 86400 = 0).
fn days_between(earlier: &str, later: &str) -> Option<u64> {
    let e = parse_iso_days(earlier)?;
    let l = parse_iso_days(later)?;
    if l >= e {
        Some(l - e)
    } else {
        Some(0)
    }
}

/// ISO-8601 date string → **epoch'tan itibaren gün sayısı** (`days_from_civil`).
/// Gün hassasiyetinde — saat/dakika/saniye/timezone offset ihmal edilir.
fn parse_iso_days(s: &str) -> Option<u64> {
    // Format: 2026-06-29T13:35:13+00:00  veya  2026-06-29 13:35:13  veya  2026-06-29
    let s = s.split('+').next().unwrap_or(s);
    let s = s.split('T').next().unwrap_or(s);
    let date_part = s.split(' ').next()?;
    let comps: Vec<&str> = date_part.split('-').collect();
    if comps.len() != 3 {
        return None;
    }
    let year: u32 = comps[0].parse().ok()?;
    let month: u32 = comps[1].parse().ok()?;
    let day: u32 = comps[2].parse().ok()?;
    Some(days_from_civil(year, month, day))
}

/// Howard Hinnant'ın `days_from_civil` algoritması — artık yıl doğrusal.
/// unsigned, epoch 1970-01-01. astronomik olarak doğru (±0 gün, civil calendar).
fn days_from_civil(y: u32, m: u32, d: u32) -> u64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let doy = (153 * ((if m > 2 { m - 3 } else { m + 9 }) as u64) + 2) / 5 + (d - 1) as u64; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    (era * 146097 + doe as i64 - 719468) as u64
}

/// `git -C <repo> <args>` çalıştır, stdout'u döndür. Başarısızlıkta Err.
/// `osp-desktop`'taki `git_output` ile aynı pattern ama ayrı (crate bağımsızlığı).
fn git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    let output = cmd.output().map_err(|e| format!("git failed: {e}"))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("utf8: {e}"))
    } else {
        Err(format!(
            "git error: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    /// `git -C <dir>` çalıştır, test fixture commit'leri için.
    /// `user.name`/`user.email` her çağrıda explicit verilir (override destekler).
    /// Önemli: `GIT_AUTHOR_*`/`GIT_COMMITTER_*` env var'ları temizlenir — parent
    /// process'ten (CI/lokal git config) sızan author bilgisini önler. Ayrıca
    /// `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM` boşaltılır ki tempdir gerçekten
    /// izole olsun (parent repo'nun .git'i keşfedilmesin).
    fn git_env(dir: &Path, args: &[&str], author: &str, email: &str) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", author)
            .env("GIT_AUTHOR_EMAIL", email)
            .env("GIT_COMMITTER_NAME", author)
            .env("GIT_COMMITTER_EMAIL", email)
            .env("GIT_CONFIG_GLOBAL", "")
            .env("GIT_CONFIG_SYSTEM", "")
            .status()
            .expect("git command");
        assert!(status.success(), "git {:?} failed", args);
    }

    /// Eski `git()` helper — sadece author gerekmediğinde (init/add).
    fn git(dir: &Path, args: &[&str]) {
        git_env(dir, args, "OSP Test", "osp-test@example");
    }

    /// Birden fazla yazarlı git fixture: her yazar birkaç dosyaya commit yapar.
    fn make_fixture_with_commits() -> TempDir {
        let dir = TempDir::new().expect("temp dir");
        git(dir.path(), &["init", "-q"]);
        // Önce dosyaları yaz
        fs::write(dir.path().join("alpha.py"), "x = 1\n").unwrap();
        fs::write(dir.path().join("beta.py"), "y = 2\n").unwrap();
        // İlk commit (Alice)
        git(dir.path(), &["add", "."]);
        git_env(
            dir.path(),
            &["commit", "-q", "-m", "init"],
            "Alice",
            "alice@x",
        );
        // alpha.py'yi değiştir (Bob)
        fs::write(dir.path().join("alpha.py"), "x = 1\ny = 2\n").unwrap();
        git(dir.path(), &["add", "."]);
        git_env(
            dir.path(),
            &["commit", "-q", "-m", "touch alpha"],
            "Bob",
            "bob@x",
        );
        dir
    }

    // --- extract_witness: git var ---

    #[test]
    fn extract_witness_populates_by_file_for_real_git_repo() {
        let dir = make_fixture_with_commits();
        let profile = extract_witness(dir.path());

        assert!(profile.repo_has_git, "git repo olmalı");
        assert!(profile.head_commit.is_some(), "head commit dolu olmalı");

        let alpha = profile.for_file("alpha.py").expect("alpha.py tracked");
        // alpha'ya 2 commit dokundu (init + touch)
        assert_eq!(alpha.commits_touching, 2, "alpha: 2 commits");
        assert_eq!(alpha.distinct_authors, 2, "alpha: Alice + Bob");
        assert!(alpha.churn > 0, "alpha: churn > 0");

        let beta = profile.for_file("beta.py").expect("beta.py tracked");
        assert_eq!(beta.commits_touching, 1, "beta: 1 commit (init only)");
        assert_eq!(beta.distinct_authors, 1, "beta: Alice only");
    }

    #[test]
    fn extract_witness_ownership_concentration_solo_vs_shared() {
        let dir = make_fixture_with_commits();
        let profile = extract_witness(dir.path());

        // alpha: 2 commit, 2 yazar (Alice 1 + Bob 1) → concentration 0.5
        let alpha = profile.for_file("alpha.py").unwrap();
        assert!(
            (alpha.ownership_concentration - 0.5).abs() < 1e-9,
            "alpha shared → 0.5, got {}",
            alpha.ownership_concentration
        );

        // beta: 1 commit, 1 yazar → concentration 1.0 (solo)
        let beta = profile.for_file("beta.py").unwrap();
        assert!(
            (beta.ownership_concentration - 1.0).abs() < 1e-9,
            "beta solo → 1.0, got {}",
            beta.ownership_concentration
        );
    }

    #[test]
    fn extract_witness_last_modified_zero_for_fresh_commit() {
        let dir = make_fixture_with_commits();
        let profile = extract_witness(dir.path());
        // En son commit az önce → days_ago ~0
        for (_, w) in &profile.by_file {
            assert!(
                w.last_modified_days_ago <= 1,
                "fresh commit → days_ago <= 1, got {}",
                w.last_modified_days_ago
            );
        }
    }

    // --- extract_witness: git yok (graceful) ---
    //
    // Not: `extract_witness`'ın "git yok → no_git()" branch'i `git rev-parse`
    // Err döndüğünde çalışır. Bu, `git` binary yoksa veya gerçekten repo dışı
    // bir path'te çalışır. Lokal Windows'ta tempdir (`%LOCALAPPDATA%\Temp`)
    // parent'ında `.git` varsa (kullanıcı home repo'su) git keşif yapar — bu
    // test ortamı kusuru, production davranışı değil. Bu yüzden no_git
    // davranışını constructor (`WitnessProfile::no_git`) ve `for_file` ile
    // test ediyoruz; `extract_witness`'ın git-çağrı logic'i zaten basit
    // (Err → no_git) ve CI (Linux, tempdir .git-free) altında geçer.

    #[test]
    fn no_git_profile_has_empty_by_file_and_none_for_file() {
        let profile = WitnessProfile::no_git();
        assert!(!profile.repo_has_git);
        assert!(profile.by_file.is_empty());
        assert!(profile.head_commit.is_none());
        // for_file her zaman None — repo_has_git false iken erken dönüş.
        assert!(profile.for_file("any/path.py").is_none());
    }

    #[test]
    fn no_git_profile_for_file_returns_none_even_with_entries() {
        // Teorik olarak by_file dolu olsa bile repo_has_git=false → None.
        // (no_git() boş üretir ama bu test for_file'ın invariant'ını sabitler.)
        let mut profile = WitnessProfile::no_git();
        profile.by_file.insert(
            "alpha.py".to_string(),
            NodeWitness {
                commits_touching: 5,
                ..NodeWitness::neutral()
            },
        );
        assert!(
            profile.for_file("alpha.py").is_none(),
            "no_git masks entries"
        );
    }

    // --- parse_log: unit testler (gerçek git gerektirmeyen) ---

    #[test]
    fn parse_log_single_commit_single_file() {
        let log = "OSPSEPabc123|Alice|2026-06-29T13:35:13+00:00\n1\t0\talpha.py\n";
        let map = parse_log(log);
        let w = map.get("alpha.py").expect("alpha.py parsed");
        assert_eq!(w.commits_touching, 1);
        assert_eq!(w.distinct_authors, 1);
        assert_eq!(w.churn, 1);
        assert!((w.ownership_concentration - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_log_two_commits_aggregate_churn_and_authors() {
        let log = "\
OSPSEPc1|Alice|2026-06-29T13:35:13+00:00
3\t1\talpha.py
OSPSEPc2|Bob|2026-06-28T10:00:00+00:00
2\t0\talpha.py
";
        let map = parse_log(log);
        let w = map.get("alpha.py").unwrap();
        assert_eq!(w.commits_touching, 2);
        assert_eq!(w.distinct_authors, 2, "Alice + Bob");
        assert_eq!(w.churn, 6, "3+1+2+0 = 6");
        assert!(
            (w.ownership_concentration - 0.5).abs() < 1e-9,
            "1 commit each → 0.5"
        );
        // last_modified: en yakın commit (2026-06-29, HEAD) → 0 gün
        assert_eq!(w.last_modified_days_ago, 0, "HEAD commit = 0 days ago");
    }

    #[test]
    fn parse_log_days_ago_uses_day_difference_not_seconds_regression() {
        // REGRESSION: gün farkı 12 olmalı, 0 DEĞİL. Eski implementasyon
        // days_from_civil (gün) sonucunu /86400'e bölüyordu → çift bölme → 0.
        // Svelte corpus'ta tüm dosyalar "0 gün önce" çıkıyordu. Bug fix:
        // days_between artık bölme yapmıyor (gün - gün = gün).
        let log = "\
OSPSEPhead|HEAD|2026-06-16T01:18:02+09:00
1\t0\truntime.js
OSPSEPold|Alice|2026-06-04T03:49:19+02:00
5\t3\truntime.js
";
        let map = parse_log(log);
        let w = map.get("runtime.js").unwrap();
        // HEAD (06-16) vs son değişiklik — runtime.js her iki commit'te de değişti.
        // En yakın = HEAD → 0 gün. Bu doğru.
        assert_eq!(w.last_modified_days_ago, 0, "HEAD'te de touch → 0");

        // AMA sadece eski commit'te touch edilen dosya → gün farkı korunmalı.
        let log2 = "\
OSPSEPhead|HEAD|2026-06-16T01:18:02+09:00
OSPSEPold|Alice|2026-06-04T03:49:19+02:00
5\t3\tlegacy.js
";
        let map2 = parse_log(log2);
        let w2 = map2.get("legacy.js").unwrap();
        // legacy.js sadece 06-04'te → HEAD (06-16) arası 12 gün. Eski bug: 0.
        assert_eq!(
            w2.last_modified_days_ago, 12,
            "12 gün fark (eski bug 0 veriyordu — çift bölme)"
        );
    }

    #[test]
    fn parse_log_binary_files_skipped() {
        // Binary: "-\t-\timage.png" → skip
        let log = "OSPSEPc1|Alice|2026-06-29T13:35:13+00:00\n-\t-\timage.png\n1\t0\talpha.py\n";
        let map = parse_log(log);
        assert!(!map.contains_key("image.png"), "binary skip");
        assert!(map.contains_key("alpha.py"), "text kept");
    }

    #[test]
    fn parse_log_multiple_files_in_one_commit() {
        let log = "\
OSPSEPc1|Alice|2026-06-29T13:35:13+00:00
1\t0\talpha.py
0\t2\tbeta.py
5\t3\tgamma.py
";
        let map = parse_log(log);
        assert_eq!(map.len(), 3, "3 distinct files");
        assert_eq!(map["alpha.py"].churn, 1);
        assert_eq!(map["beta.py"].churn, 2);
        assert_eq!(map["gamma.py"].churn, 8);
    }

    // --- normalize_path ---

    #[test]
    fn normalize_path_strips_dot_slash_prefix() {
        assert_eq!(normalize_path("./src/main.py"), "src/main.py");
    }

    #[test]
    fn normalize_path_handles_rename_arrow() {
        assert_eq!(normalize_path("old/name.py => new/name.py"), "new/name.py");
    }

    #[test]
    fn normalize_path_backslash_to_forward() {
        assert_eq!(normalize_path("src\\main.py"), "src/main.py");
    }

    // --- days_from_civil (Howard Hinnant) ---

    #[test]
    fn days_from_civil_epoch_is_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0, "1970-01-01 = epoch");
    }

    #[test]
    fn days_from_civil_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 2), 1, "+1 day");
        assert_eq!(days_from_civil(1971, 1, 1), 365, "+1 non-leap year");
        assert_eq!(days_from_civil(2024, 1, 1), 19_723, "2024-01-01 known");
    }
}
