//! Repository snapshot — Git HEAD binding + hierarchical tracked paths + analyzed-scope clean fence.
//!
//! #155 (analyzed-scope clean semantics): garanti edilen şey tüm worktree'nin temizliği
//! değil, **ölçülen içeriğin bağlı olduğu revision tarafından tanımlanabilir olması ve
//! ölçüm/karar penceresinde drift etmemesidir**. Snapshot authority tek SHA değil,
//! repository-root → revision mapping'idir (superproject HEAD → gitlink SHA → submodule
//! tree → analyzed file; `capture_tracked_paths` submodule-aware genişletir).
//!
//! Fence katmanları (`trajectory attempt`):
//! - `validate_analyzed_paths_tracked` — analyzed ⊆ (hiyerarşik) HEAD tracked set,
//! - `validate_analyzed_paths_clean` (pre/post) — analyzed kapsam dirty değil
//!   (modified/untracked/submodule girdisi önek eşleşmesiyle),
//! - `validate_post_attempt_snapshot` — HEAD + tracked-set global eşitlik + post analyzed-scope.
//!
//! Bu modül "atomic snapshot" iddia ETMEZ — pre/post fence ile drift-detected consistent
//! analysis sağlar. Transient ABA (clean A → B → clean A) yakalanmayabilir; controlled
//! temp fixture'da eşzamanlı yazıcı olmadığı için yeterli.
//!
//! `ensure_snapshot_eligible` (global clean) yalnızca clean-bound
//! `--require-clean-snapshot` analyze sözleşmesinde yaşar (B-3 task üretimi).

#![allow(
    dead_code,
    reason = "Faz 8 test-project: wired incrementally across commits"
)]

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// Validated Git commit ID — full 40-char lowercase hex SHA.
///
/// `unknown`, short hash, veya boş değerler fail-closed reddedilir (review P1-3).
/// Snapshot identity için canonical kaynak — analyze DTO, task file HEAD validation,
/// run-evidence envelope aynı newtype'ı kullanır.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitCommitId(String);

impl GitCommitId {
    /// Full 40-char SHA'yı döndürür.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GitCommitId {
    type Error = RepoSnapshotError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = value.trim();
        if value.len() != 40 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(RepoSnapshotError::InvalidRepositoryHead {
                value: value.to_string(),
            });
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}

impl std::fmt::Display for GitCommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Repository snapshot — HEAD + tracked paths + dirty-path set.
///
/// `capture(repo)` Git komutlarıyla bu üç bilgiyi toplar. Snapshot equality
/// (`PartialEq`) yalnızca **clean-bound analyze** sözleşmesinde kullanılır
/// (`--require-clean-snapshot`: pre/post eşitlik fence'i); `trajectory attempt`
/// bunun yerine [`validate_post_attempt_snapshot`]'ı kullanır (HEAD + tracked-set
/// global eşitlik + analyzed-scope içerik fence'i).
///
/// #155 (analyzed-scope clean semantics): `clean` artık global bir boolean değil,
/// `dirty_paths` kümesinden türetilir. Drift fence'i analyzed-scope'ta çalışır:
/// ölçülen dosyaların HEAD-tracked ve değişmemiş olması gerekir; analiz kapsamı
/// dışındaki aktif iş run'ı bloklamaz (gerçek monorepo: modified submodule,
/// ilgisiz untracked dosyalar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    pub head: GitCommitId,
    pub tracked_paths: BTreeSet<String>,
    /// Modified + untracked + submodule girdileri (`git status --porcelain
    /// --untracked-files=all`). Submodule girdisi TEK path'tir (iç dosyalar değil);
    /// kapsam kontrolü önek eşleşmesiyle yapılır.
    pub dirty_paths: BTreeSet<String>,
}

impl RepositorySnapshot {
    /// Repository snapshot'ı capture eder — `git rev-parse HEAD`, `git ls-tree`, `git status`.
    ///
    /// dirty_paths = `git status --porcelain --untracked-files=all` path'leri (sorted).
    /// tracked_paths = `git ls-tree -r --name-only HEAD` çıktısı (sorted).
    pub fn capture(repo: &Path) -> Result<Self, RepoSnapshotError> {
        let head = capture_head(repo)?;
        let tracked_paths = capture_tracked_paths(repo)?;
        let dirty_paths = capture_dirty_paths(repo)?;
        Ok(Self {
            head,
            tracked_paths,
            dirty_paths,
        })
    }

    /// Global temizlik (yalnızca raporlama + clean-bound analyze sözleşmesi için).
    pub fn clean(&self) -> bool {
        self.dirty_paths.is_empty()
    }
}

/// Repository snapshot capture hatası.
#[derive(Debug, thiserror::Error)]
pub enum RepoSnapshotError {
    #[error("invalid repository HEAD (expected full 40-char hex SHA): {value:?}")]
    InvalidRepositoryHead { value: String },
    #[error("git command failed ({command}): {detail}")]
    GitCommandFailed {
        command: &'static str,
        detail: String,
    },
    /// #204 (tur-3 P0): porcelain -z kaydı yeniden adlandırma/copy beyan etmiş ama
    /// ikinci NUL kaydı (eski yol) eksik/boş — bozuk git çıktısı sessizce
    /// yorumlanamaz (fail-closed).
    #[error("malformed git status record: {detail}")]
    MalformedStatusRecord { detail: String },
}

fn run_git(repo: &Path, args: &[&str], command: &'static str) -> Result<String, RepoSnapshotError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| RepoSnapshotError::GitCommandFailed {
            command,
            detail: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(RepoSnapshotError::GitCommandFailed {
            command,
            detail: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    String::from_utf8(output.stdout)
        .map_err(|e| RepoSnapshotError::GitCommandFailed {
            command,
            detail: e.to_string(),
        })
        .map(|s| s.trim().to_string())
}

fn capture_head(repo: &Path) -> Result<GitCommitId, RepoSnapshotError> {
    let head_str = run_git(repo, &["rev-parse", "HEAD"], "rev-parse HEAD")?;
    GitCommitId::try_from(head_str)
}

fn capture_tracked_paths(repo: &Path) -> Result<BTreeSet<String>, RepoSnapshotError> {
    // Parent HEAD tree (düz dosyalar). Submodule içeride DEĞİLDİR: parent `ls-tree -r`
    // yalnızca gitlink'i (`160000 commit <sha>\t<path>`) listeler — nested dosyalar
    // parent HEAD tree'sinde yok (#155 P1-1: bu yüzden analyzed nested path'ler
    // `validate_analyzed_paths_tracked`'den geçemiyordu).
    let mut paths: BTreeSet<String> =
        run_git(repo, &["ls-tree", "-r", "--name-only", "HEAD"], "ls-tree")?
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();

    // Submodule-aware genişletme (#155 P1-1; #204 tur-4 P1-1 ile RECURSIVE):
    // revision identity hiyerarşiktir ve her derinlikte aynı sözleşmeyle doğrulanır.
    let long_format = run_git_raw(repo, &["ls-tree", "-r", "HEAD"], "ls-tree")?;
    expand_gitlinks_recursive(repo, "", &long_format, &mut paths)?;
    Ok(paths)
}

/// (#204 tur-4 P1-1) Nested gitlink traversal — `capture_tracked_paths`'in recursive
/// yardımcısı. Her seviyede submodule HEAD'i o seviyenin gitlink SHA'sıyla EŞLEŞİYORSA
/// o seviyenin HEAD tree'si `<prefix>/` önekiyle tracked sete katılır; eşleşmiyorsa
/// içerik parent revizyonundan farklıdır (`git status` submodule'u dirty listeler →
/// önek fence'i reddeder) — o seviyenin nested path'leri yetkili kabul EDİLMEZ.
/// Uninitialized submodule (worktree boş) genişletilmez: analyzer da içeriği göremez.
fn expand_gitlinks_recursive(
    dir: &Path,
    prefix: &str,
    long_format: &str,
    paths: &mut BTreeSet<String>,
) -> Result<(), RepoSnapshotError> {
    for line in long_format.lines() {
        let Some((meta, sub_path)) = line.split_once('\t') else {
            continue;
        };
        let mut parts = meta.split_whitespace();
        let (Some(mode), Some(kind), Some(gitlink_sha)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if mode != "160000" || kind != "commit" {
            continue;
        }
        let full_prefix = if prefix.is_empty() {
            sub_path.to_string()
        } else {
            format!("{prefix}/{sub_path}")
        };
        let sub_dir = dir.join(sub_path);
        if !sub_dir.join(".git").exists() {
            // Uninitialized submodule: worktree boş → analyzer nested dosya bulamaz;
            // tracked sete katmak anlamsız (analiz edilecek içerik yok).
            continue;
        }
        let Ok(sub_head) = run_git(
            &sub_dir,
            &["rev-parse", "HEAD"],
            "rev-parse HEAD (submodule)",
        ) else {
            continue;
        };
        if sub_head != gitlink_sha {
            // HEAD ≠ gitlink: submodule içeriği parent'ın bağladığı revizyondan farklı.
            // Porcelain bunu dirty girdi olarak raporlar → prefix fence devreye girer.
            continue;
        }
        let nested_names = run_git(
            &sub_dir,
            &["ls-tree", "-r", "--name-only", "HEAD"],
            "ls-tree (submodule)",
        )?;
        for p in nested_names.lines().filter(|l| !l.is_empty()) {
            paths.insert(format!("{full_prefix}/{p}"));
        }
        // Bir seviye daha derin gitlink'ler (nested-in-nested submodule'lar) aynı
        // HEAD==gitlink sözleşmesiyle açılır — recursive detached analiz (tur-4 P0)
        // `module/nested/src/leaf.cs` gibi path'leri ölçebilir; tracked set de aynı
        // derinliği görmek zorunda, yoksa `validate_analyzed_paths_tracked` geçerli
        // node'ları yanlış reddederdi.
        let nested_long = run_git_raw(&sub_dir, &["ls-tree", "-r", "HEAD"], "ls-tree (submodule)")?;
        expand_gitlinks_recursive(&sub_dir, &full_prefix, &nested_long, paths)?;
    }
    Ok(())
}

/// `run_git`'in trim'siz varyantı — porcelain gibi BAŞTAKİ boşluk anlamlı olan
/// çıktılar için (`run_git` tüm çıktıyı trim'ler ve XY durum boşluğunu yutardı;
/// #155 parse hatasının kök nedeni buydu).
fn run_git_raw(
    repo: &Path,
    args: &[&str],
    command: &'static str,
) -> Result<String, RepoSnapshotError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| RepoSnapshotError::GitCommandFailed {
            command,
            detail: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(RepoSnapshotError::GitCommandFailed {
            command,
            detail: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    String::from_utf8(output.stdout).map_err(|e| RepoSnapshotError::GitCommandFailed {
        command,
        detail: e.to_string(),
    })
}

/// `git status --porcelain=v1 -z` kayıtlarını dirty-path kümesine çevirir (saf fonksiyon).
///
/// `-z` formatı (#155 P1-2): kayıtlar NUL ile ayrılır, path'ler HAM'dır — quote/C-escape
/// YOKTUR (`quo"te.rs", `türkçe.rs` olduğu gibi gelir). Klasik porcelain'un quote'lu
/// çıktısında `trim_matches('"')` escape'leri çözmezdi ve fence fail-open olabilirdi.
/// Rename (`R`) ve copy (`C`) kayıtları `<new>\0<old>` şeklinde İKİ NUL'lu kayıttır —
/// her iki uç da kirli kapsama alınır.
///
/// **#204 (tur-3 P0): iki-sütunlu rename/copy tanıma.** R/C hem X (index) hem Y
/// (worktree) sütununda görünebilir (`R ` index-side, ` R` worktree-side — örn.
/// `mv a b && git add -N b` tam olarak ` R b\0a\0` üretir). Eski kod yalnız X'e
/// bakıyordu: worktree-side rename'de ikinci kayıt tüketilmiyor, eski yol ya
/// sessizce düşüyor (kısa ad) ya da bir sonraki iterasyonda BAŞLIKSIZ kayıt sanılıp
/// çöp bir dirty-path olarak kümeye giriyordu (uzun ad). Artık XY'nin herhangi bir
/// sütununda R/C varsa ikinci kayıt ZORUNLUDUR; eksik/boş kayıt fail-closed red
/// (yorumlanamaz çıktı sessizce atlanamaz).
pub(crate) fn parse_porcelain_z(raw: &str) -> Result<BTreeSet<String>, RepoSnapshotError> {
    let mut paths = BTreeSet::new();
    let mut records = raw.split('\0');
    while let Some(record) = records.next() {
        // Kayıt en az `XY ` + 1 karakter path.
        if record.len() < 4 {
            continue;
        }
        let xy = &record[..2];
        let path = &record[3..];
        if path.is_empty() {
            continue;
        }
        paths.insert(path.to_string());
        let rename_or_copy = xy.contains('R') || xy.contains('C');
        if rename_or_copy {
            // Rename/copy: izleyen ek kayıt eski path'tir — beyan edilen iki-NUL'lu
            // şeklin ZORUNLU parçası; eksik/boş ise çıktı bozuktur (fail-closed).
            match records.next() {
                Some(old) if !old.is_empty() => {
                    paths.insert(old.to_string());
                }
                _ => {
                    return Err(RepoSnapshotError::MalformedStatusRecord {
                        detail: format!(
                            "rename/copy record {xy:?} for {path:?} is missing its \
                             second NUL record (the old path) — refusing to interpret a \
                             truncated git status stream"
                        ),
                    });
                }
            }
        }
    }
    Ok(paths)
}

fn capture_dirty_paths(repo: &Path) -> Result<BTreeSet<String>, RepoSnapshotError> {
    // -z: NUL-ayrı, quote/escape'siz, deterministik (#155 P1-2).
    // --ignore-submodules=none (#156 R2 P1-1): `submodule.<name>.ignore = dirty/all`
    // config'i submodule worktree değişikliklerini status'tan GİZLER — flag bunu
    // override eder. Aksi halde HEAD == gitlink durumunda nested tracked dosyalar
    // sete girer, analyzer değiştirilmiş içeriği okur ama dirty kümesi boş kalır:
    // "HEAD'e bağlı" iddia edilen ölçüm commit'siz içerikten üretilebilir (epistemik
    // bypass). Yeni invariant'ın "repository config davranışı değiştiremez" koşulu.
    let output = run_git_raw(
        repo,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        "status",
    )?;
    parse_porcelain_z(&output)
}

/// Harness eligibility: clean worktree zorunlu (review P0-3).
///
/// #155: yalnızca clean-bound `--require-clean-snapshot` analyze sözleşmesi için
/// kullanılır (task üretimi tam HEAD bağlanır). `trajectory attempt` akışı bunun
/// yerine [`validate_analyzed_paths_clean`] kullanır (analyzed-scope fence).
pub fn ensure_snapshot_eligible(snapshot: &RepositorySnapshot) -> Result<(), RepoSnapshotError> {
    if !snapshot.clean() {
        return Err(RepoSnapshotError::GitCommandFailed {
            command: "status",
            detail: "dirty or untracked working tree — harness requires clean worktree".to_string(),
        });
    }
    Ok(())
}

/// #155: analyzed-scope clean fence — ölçülen dosyalar HEAD-tracked ve değişmemiş
/// olmalıdır; analiz kapsamı DIŞINDAKİ dirty/untracked dosyalar run'ı bloklamaz.
///
/// İki red durumu (educational, path listeli):
/// - analyzed path dirty kümesinde (modified / untracked),
/// - dirty path analyzed path'in EBEVEYNİ (submodule girdisi — `--porcelain`
///   submodule'u tek path olarak raporlar; içeriği ölçüme giriyorsa kapsam
///   etkilenmiş demektir).
pub fn validate_analyzed_paths_clean(
    node_paths: &std::collections::HashMap<u64, String>,
    snapshot: &RepositorySnapshot,
    phase: &'static str,
) -> Result<(), RepoSnapshotError> {
    for path in node_paths.values() {
        if snapshot.dirty_paths.contains(path) {
            return Err(RepoSnapshotError::GitCommandFailed {
                command: "status",
                detail: format!(
                    "analyzed path is modified or untracked ({phase}): {path} — \
                     commit/stash it (a measurement bound to HEAD must not depend on \
                     uncommitted content)"
                ),
            });
        }
        for dirty in &snapshot.dirty_paths {
            if path.starts_with(&format!("{dirty}/")) {
                return Err(RepoSnapshotError::GitCommandFailed {
                    command: "status",
                    detail: format!(
                        "analyzed path falls under a dirty submodule/directory ({phase}): \
                         {path} under {dirty} — the nested content changed; commit/stash \
                         the submodule work or scope the task outside it"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// #204: kirli-ağaç fence politikası — default (Strict) veya opt-in declared-dirty.
///
/// - **Strict** (default; davranış #155 ile birebir aynı): ölçülen dosyalar
///   HEAD-tracked ve değişmemiş olmalı; analiz kapsamı DIŞINDAKİ dirty/untracked
///   dosyalar run'ı bloklamaz.
/// - **DeclaredUniverse** (opt-in `--allow-declared-dirty`): worktree'deki TÜM
///   kirli yollar (staged+unstaged+untracked+intent-to-add; rename her iki uç)
///   beyan edilen evrenin (`new_nodes[].path ∪ modified_entities`) içinde olmalı.
///   Beyan dışı TEK kirli yol → red. Bu, default'un analyzed-scope gevşekliğinden
///   bilinçli olarak DAHA KATIDIR: "temiz ağaç" bu koşulun boş-beyan özel hâlidir.
///   Ignored dosyalar kapsam DIŞIDIR (`git status --porcelain` onları listelemez).
#[derive(Debug, Clone, Copy)]
pub enum DirtyPolicy<'a> {
    Strict,
    DeclaredUniverse(&'a BTreeSet<String>),
}

/// #204: global `D ⊆ A` fence'i — dirty-path kümesi beyan edilen evrenin alt kümesi
/// olmalı; ihlalde sıralı yol listesi + remedya ile educational red (saf fonksiyon).
///
/// Bilinçli sözleşme (issue #204 karar kaydı): fence yalnız YOL-ÜYELİĞİ bağlar —
/// kirli dosya içeriğinin koşu penceresinde kararlılığı iddia EDİLMEZ (aday içerik
/// kimliği realization zincirinin patch-digest + candidate-head alanındadır).
/// Beyan fazladan temiz yol içerebilir (`D ⊆ A`, `D = A` DEĞİL).
pub fn validate_dirty_paths_declared(
    dirty_paths: &BTreeSet<String>,
    declared: &BTreeSet<String>,
    phase: &'static str,
) -> Result<(), RepoSnapshotError> {
    let undeclared: Vec<&str> = dirty_paths
        .difference(declared)
        .map(|p| p.as_str())
        .collect();
    if undeclared.is_empty() {
        return Ok(());
    }
    Err(RepoSnapshotError::GitCommandFailed {
        command: "status",
        detail: format!(
            "undeclared dirty path(s) ({phase}): {} of {} dirty path(s) are outside the \
             declared universe (new_nodes[].path ∪ modified_entities): [{}] — declare \
             them in the spec, stash them, or commit them; the declared-dirty mode \
             admits only the declared universe (ignored files are out of scope)",
            undeclared.len(),
            dirty_paths.len(),
            undeclared.join(", ")
        ),
    })
}

/// #204: dirty fence dispatcher — politikaya göre Strict (#155 analyzed-scope) veya
/// DeclaredUniverse (global `D ⊆ A`) kontrolünü çalıştırır. Ölçüm kaynağı her iki
/// modda da sabit-SHA detached worktree / temiz baseline sözleşmesine tabidir;
/// bu fence yalnızca "koşu hangi worktree durumunda istendi" bütünlüğünü korur.
pub fn enforce_dirty_policy(
    node_paths: &std::collections::HashMap<u64, String>,
    snapshot: &RepositorySnapshot,
    phase: &'static str,
    policy: &DirtyPolicy<'_>,
) -> Result<(), RepoSnapshotError> {
    match policy {
        DirtyPolicy::Strict => validate_analyzed_paths_clean(node_paths, snapshot, phase),
        DirtyPolicy::DeclaredUniverse(declared) => {
            validate_dirty_paths_declared(&snapshot.dirty_paths, declared, phase)
        }
    }
}

/// #155: post-attempt drift fence — saf fonksiyon (test edilebilir exact matrix).
///
/// Sözleşme (#155 kabul kriterleri + #204 policy):
/// - HEAD değişti → red (revision identity bozuldu; global),
/// - tracked-path seti değişti → red (revision tanımladığı dosya seti değişti; global),
/// - analyzed kapsam dirty'leşti → red (Strict) / kirli set beyanı aştı → red
///   (DeclaredUniverse — #204 opt-in; ölçüm ondan türetilmese de beyan edilen
///   çalışma-ağacı durumu koşu penceresinde ihlal edilmemeli),
/// - analiz DIŞINDAKİ bir path dirty'leşti → Strict'te kabul (ölçüm ondan türetilmedi).
pub fn validate_post_attempt_snapshot(
    before: &RepositorySnapshot,
    after: &RepositorySnapshot,
    node_paths: &std::collections::HashMap<u64, String>,
    policy: &DirtyPolicy<'_>,
) -> Result<(), RepoSnapshotError> {
    if after.head != before.head {
        return Err(RepoSnapshotError::GitCommandFailed {
            command: "rev-parse HEAD",
            detail: "repository HEAD moved during trajectory attempt — \
                     analysis-run consistency violated"
                .to_string(),
        });
    }
    if after.tracked_paths != before.tracked_paths {
        return Err(RepoSnapshotError::GitCommandFailed {
            command: "ls-tree",
            detail: "repository tracked-path set changed during trajectory attempt — \
                     analysis-run consistency violated"
                .to_string(),
        });
    }
    enforce_dirty_policy(node_paths, after, "after attempt", policy)
}

/// Analyzed path ⊆ HEAD tracked-path invariant (review P1-2).
///
/// Analyzer'ın ürettiği `node_paths` içinde HEAD tree'de tracked olmayan path varsa
/// fail-closed (ignored/generated dosyalar `git status --porcelain` ile yakalanmayabilir).
pub fn validate_analyzed_paths_tracked(
    node_paths: &std::collections::HashMap<u64, String>,
    snapshot: &RepositorySnapshot,
) -> Result<(), RepoSnapshotError> {
    for path in node_paths.values() {
        if !snapshot.tracked_paths.contains(path) {
            return Err(RepoSnapshotError::GitCommandFailed {
                command: "ls-tree",
                detail: format!(
                    "analyzed path not tracked in HEAD: {path} — commit/add it \
                     (git add + commit) or exclude it from the analysis scope; a \
                     measurement bound to HEAD cannot depend on uncommitted files"
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod analyzed_scope_fence_tests {
    //! #155: analyzed-scope clean semantics — dirty kümesi parse + kapsam fence'i.

    use super::*;
    use std::collections::HashMap;

    fn snapshot_with_dirty(dirty: &[&str]) -> RepositorySnapshot {
        RepositorySnapshot {
            head: GitCommitId::try_from("a".repeat(40)).unwrap(),
            tracked_paths: BTreeSet::new(),
            dirty_paths: dirty.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn paths(entries: &[(u64, &str)]) -> HashMap<u64, String> {
        entries.iter().map(|(k, v)| (*k, v.to_string())).collect()
    }

    #[test]
    fn porcelain_z_parse_pins_real_git_output_shapes() {
        // Gerçek `git status --porcelain=v1 -z` çıktı biçimleri birebir pinlenir
        // (#155 P1-2 review: önceki test parse'ı değil hazır set'i sınıyordu).
        // -z'de path'ler HAMDIR: quote/C-escape yoktur.
        let raw = " M a.rs\0?? e2e/new.spec.ts\0 m clients/nexus-backoffice\0";
        let parsed = parse_porcelain_z(raw).expect("plain records must parse");
        assert_eq!(
            parsed,
            BTreeSet::from([
                "a.rs".to_string(),
                "e2e/new.spec.ts".to_string(),
                "clients/nexus-backoffice".to_string(),
            ])
        );

        // Quote/unicode path'ler klasik porcelain'da escape edilirdi ("t\303\274rk...");
        // -z'de kayıpsız gelir — fail-open kapanır.
        let tricky = " M quo\"te.rs\0 M türkçe.rs\0";
        let parsed = parse_porcelain_z(tricky).expect("tricky records must parse");
        assert!(parsed.contains("quo\"te.rs"), "got: {parsed:?}");
        assert!(parsed.contains("türkçe.rs"), "{parsed:?}");

        // Rename: `R  new\0old` — iki NUL'lu kayıt, iki uç da kirli kapsamda.
        let rename = "R  src/new.rs\0src/old.rs\0";
        let parsed = parse_porcelain_z(rename).expect("index-side rename must parse");
        assert_eq!(
            parsed,
            BTreeSet::from(["src/new.rs".to_string(), "src/old.rs".to_string()])
        );

        // #204 (tur-3 P0): WORKTREE-side rename ` R new\0old` — ikinci sütundaki R.
        // Gerçek üretim: `mv a b && git add -N b` → ` R b\0a\0`. Eski kod bu şekli
        // tanımıyordu: eski yol ya düşüyor ya da başlıksız kayıt sanılıp ÇÖP path
        // olarak kümeye giriyordu. Her iki uç artık kümede.
        let worktree_rename = " R src/new.rs\0src/old.rs\0";
        let parsed = parse_porcelain_z(worktree_rename).expect("worktree-side rename must parse");
        assert_eq!(
            parsed,
            BTreeSet::from(["src/new.rs".to_string(), "src/old.rs".to_string()])
        );

        // Rename/copy beyan edilmiş ama ikinci NUL kaydı eksik/boş → fail-closed red
        // (bozuk git çıktısı sessizce yorumlanamaz).
        let err = parse_porcelain_z("R  src/new.rs\0").expect_err("truncated rename must reject");
        assert!(
            format!("{err}").contains("missing its second NUL record"),
            "{err}"
        );
        let err = parse_porcelain_z(" R src/new.rs\0src/old.rs\0R  x.rs\0\0")
            .expect_err("empty old-path record must reject");
        assert!(format!("{err}").contains("rename/copy record"), "{err}");

        // Boş/çok kısa kayıtlar sessizce atlanır; boş çıktı → boş küme (clean).
        assert!(parse_porcelain_z("").expect("empty input").is_empty());
        assert!(parse_porcelain_z("\0\0")
            .expect("null-only input")
            .is_empty());
        assert!(snapshot_with_dirty(&[]).clean()); // clean() derived: boş küme → clean
    }

    #[test]
    fn post_fence_matrix_exact_contract() {
        // #155 kabul kriterleri — saf fonksiyonda exact matrix (timing-flaky E2E yerine):
        // analyzed clean→dirty RED · unrelated clean→dirty PASS · HEAD changed RED ·
        // tracked set changed RED.
        let mk = |dirty: &[&str]| RepositorySnapshot {
            head: GitCommitId::try_from("b".repeat(40)).unwrap(),
            tracked_paths: BTreeSet::from(["a.rs".to_string(), "b.rs".to_string()]),
            dirty_paths: dirty.iter().map(|s| s.to_string()).collect(),
        };
        let head_variant = |snap: &RepositorySnapshot| RepositorySnapshot {
            head: GitCommitId::try_from("c".repeat(40)).unwrap(),
            ..snap.clone()
        };
        let tracked_variant = |snap: &RepositorySnapshot| {
            let mut tracked = snap.tracked_paths.clone();
            tracked.insert("c.rs".to_string());
            RepositorySnapshot {
                tracked_paths: tracked,
                ..snap.clone()
            }
        };
        let analyzed = paths(&[(0, "a.rs"), (1, "b.rs")]);
        let strict = DirtyPolicy::Strict;

        // Hepsi clean → geçer.
        validate_post_attempt_snapshot(&mk(&[]), &mk(&[]), &analyzed, &strict)
            .expect("clean→clean must pass");

        // Analyzed kapsam dirty'leşti → red.
        let err = validate_post_attempt_snapshot(&mk(&[]), &mk(&["a.rs"]), &analyzed, &strict)
            .expect_err("analyzed path dirty after attempt must reject");
        assert!(format!("{err}").contains("after attempt"), "{err}");

        // Analiz DIŞI path dirty'leşti → kabul (ölçüm ondan türetilmedi).
        validate_post_attempt_snapshot(&mk(&[]), &mk(&["notes/local.md"]), &analyzed, &strict)
            .expect("out-of-scope drift after attempt must be accepted");

        // HEAD değişti → red.
        let before = mk(&[]);
        let err =
            validate_post_attempt_snapshot(&before, &head_variant(&before), &analyzed, &strict)
                .expect_err("HEAD moved during attempt must reject");
        assert!(format!("{err}").contains("HEAD moved"), "{err}");

        // Tracked set değişti → red.
        let err =
            validate_post_attempt_snapshot(&before, &tracked_variant(&before), &analyzed, &strict)
                .expect_err("tracked set change during attempt must reject");
        assert!(
            format!("{err}").contains("tracked-path set changed"),
            "{err}"
        );
    }

    /// #204 opt-in post-fence: kirli set beyanı aşarsa (pencerede beyan-dışı yol
    /// doğduysa) red; beyan içinde kalıyorsa kabul. HEAD/tracked eşitliği her iki
    /// modda da koşar (yukarıdaki Strict matrisiyle aynı global koşullar).
    #[test]
    fn post_fence_declared_policy_matrix() {
        let mk = |dirty: &[&str]| RepositorySnapshot {
            head: GitCommitId::try_from("b".repeat(40)).unwrap(),
            tracked_paths: BTreeSet::from(["a.rs".to_string(), "b.rs".to_string()]),
            dirty_paths: dirty.iter().map(|s| s.to_string()).collect(),
        };
        let analyzed = paths(&[(0, "a.rs"), (1, "b.rs")]);
        let declared: BTreeSet<String> = ["a.rs", "src/new.cs", "notes/local.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let policy = DirtyPolicy::DeclaredUniverse(&declared);

        // Başlangıçta beyan-dışı kirli yok → post aynı kalırsa geçer (analyzed path
        // kirli OLSA BİLE — beyandaysa opt-in izin verir; ölçüm sabit-SHA'dan gelir).
        validate_post_attempt_snapshot(&mk(&["a.rs"]), &mk(&["a.rs"]), &analyzed, &policy)
            .expect("declared dirty set unchanged must pass");

        // Pencerede beyan-dışı yeni kirli yol → red (Strict'te kabul edilen sınıf:
        // "notes/local2.md" analiz dışı — opt-in'de çalışma-ağacı bütünlüğü ihlali).
        let err =
            validate_post_attempt_snapshot(&mk(&[]), &mk(&["notes/local2.md"]), &analyzed, &policy)
                .expect_err("undeclared dirty growth must reject in declared mode");
        let msg = format!("{err}");
        assert!(msg.contains("undeclared dirty path"), "{msg}");
        assert!(msg.contains("notes/local2.md"), "{msg}");
        assert!(msg.contains("after attempt"), "{msg}");

        // HEAD hareketi opt-in modda da reddedilir (ölçüm pinned SHA'ya bağlı).
        let before = mk(&["a.rs"]);
        let head_variant = RepositorySnapshot {
            head: GitCommitId::try_from("c".repeat(40)).unwrap(),
            ..before.clone()
        };
        let err = validate_post_attempt_snapshot(&before, &head_variant, &analyzed, &policy)
            .expect_err("HEAD move must reject in declared mode too");
        assert!(format!("{err}").contains("HEAD moved"), "{err}");
    }

    /// #204: global `D ⊆ A` fence'i — exact matris (saf fonksiyon).
    #[test]
    fn declared_fence_exact_contract() {
        let set = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();

        // Geçiş: dirty ⊆ declared (untracked + modified + intent-to-add karışık;
        // hepsi porcelain dirty kümesinde aynı üyeliğe tabidir).
        validate_dirty_paths_declared(
            &set(&["src/a.rs", "src/b.rs", "gen/new.cs"]),
            &set(&[
                "src/a.rs",
                "src/b.rs",
                "gen/new.cs",
                "clean-but-declared.rs",
            ]),
            "before attempt",
        )
        .expect("dirty ⊆ declared must pass (extra clean declarations allowed: D ⊆ A ≠ D = A)");

        // Boş beyan + kirli yol = "temiz ağaç" özel hâli → red.
        let err = validate_dirty_paths_declared(&set(&["src/a.rs"]), &set(&[]), "before attempt")
            .expect_err("empty declaration with dirty paths must reject");
        assert!(format!("{err}").contains("undeclared dirty path"), "{err}");

        // Tek suçlu: yol listelenir + sayı + remedya taşınır.
        let err = validate_dirty_paths_declared(
            &set(&["src/a.rs", "scratch/notes.md", "src/b.rs"]),
            &set(&["src/a.rs", "src/b.rs"]),
            "before attempt",
        )
        .expect_err("single undeclared path must reject with the path listed");
        let msg = format!("{err}");
        assert!(msg.contains("1 of 3"), "{msg}");
        assert!(msg.contains("scratch/notes.md"), "{msg}");
        assert!(msg.contains("stash"), "{msg}");
        assert!(msg.contains("modified_entities"), "{msg}");

        // Rename iki uç: yeni uç beyanda, eski uç DEĞİL → red (eski yol da dirty).
        let err = validate_dirty_paths_declared(
            &set(&["src/new.rs", "src/old.rs"]),
            &set(&["src/new.rs"]),
            "before attempt",
        )
        .expect_err("rename old end must be declared too");
        assert!(format!("{err}").contains("src/old.rs"), "{err}");

        // Süper-set beyanla rename iki ucu da geçer.
        validate_dirty_paths_declared(
            &set(&["src/new.rs", "src/old.rs"]),
            &set(&["src/new.rs", "src/old.rs"]),
            "before attempt",
        )
        .expect("both rename ends declared must pass");
    }

    /// #204: dispatcher — aynı snapshot+kapsamda politika sonucu değişir
    /// (Strict analyzed-scope gevşekliği ≠ DeclaredUniverse global katılığı).
    #[test]
    fn dirty_policy_dispatcher_routes_strict_and_declared() {
        let snap = snapshot_with_dirty(&["notes/local.md"]);
        let node_paths = paths(&[(0, "a.rs")]);

        // Strict: analiz dışı kirli → kabul (#155 sözleşmesi aynen).
        enforce_dirty_policy(&node_paths, &snap, "test", &DirtyPolicy::Strict)
            .expect("out-of-scope dirt must pass under Strict");

        // Declared: aynı durum, beyanda yok → red (bilinçli daha katı).
        let empty: BTreeSet<String> = BTreeSet::new();
        let err = enforce_dirty_policy(
            &node_paths,
            &snap,
            "test",
            &DirtyPolicy::DeclaredUniverse(&empty),
        )
        .expect_err("undeclared dirt must fail under DeclaredUniverse");
        assert!(format!("{err}").contains("undeclared dirty path"), "{err}");
    }

    #[test]
    fn fence_passes_when_dirty_paths_are_outside_analysis_scope() {
        let snap = snapshot_with_dirty(&[
            "clients/nexus-backoffice",
            "notes/local.md",
            "docs/untracked-card.md",
        ]);
        let node_paths = paths(&[(0, "services/ai/src/A.cs"), (1, "services/ai/src/B.cs")]);
        validate_analyzed_paths_clean(&node_paths, &snap, "test")
            .expect("dirty paths outside the analyzed scope must not block the run");
    }

    #[test]
    fn fence_rejects_modified_analyzed_path_with_actionable_message() {
        let snap = snapshot_with_dirty(&["services/ai/src/A.cs"]);
        let node_paths = paths(&[(0, "services/ai/src/A.cs")]);
        let err = validate_analyzed_paths_clean(&node_paths, &snap, "before attempt")
            .expect_err("modified analyzed path must fail the fence");
        let msg = format!("{err}");
        assert!(
            msg.contains("analyzed path is modified or untracked"),
            "{msg}"
        );
        assert!(msg.contains("services/ai/src/A.cs"), "{msg}");
        assert!(msg.contains("before attempt"), "{msg}");
    }

    #[test]
    fn fence_rejects_analyzed_path_under_dirty_submodule() {
        // `--porcelain` submodule'u TEK path olarak raporlar; analyzed path onun
        // altındaysa kapsam etkilenmiştir (Faz 1 ilk run'ın Nexus vakası).
        let snap = snapshot_with_dirty(&["clients/nexus-backoffice"]);
        let node_paths = paths(&[(0, "clients/nexus-backoffice/src/main.ts")]);
        let err = validate_analyzed_paths_clean(&node_paths, &snap, "before attempt")
            .expect_err("analyzed path under a dirty submodule must fail the fence");
        let msg = format!("{err}");
        assert!(msg.contains("dirty submodule/directory"), "{msg}");
        assert!(msg.contains("clients/nexus-backoffice"), "{msg}");
    }

    #[test]
    fn fence_prefix_is_directory_scoped_not_string_prefix() {
        // "services/ai-src" dirty iken "services/ai/src/A.cs" analyze ediliyorsa
        // bu ÖNEK KARMAŞASLIĞI olmamalı: yalnız "{dirty}/" altı etkilenir.
        let snap = snapshot_with_dirty(&["services/ai-src"]);
        let node_paths = paths(&[(0, "services/ai/src/A.cs")]);
        validate_analyzed_paths_clean(&node_paths, &snap, "test")
            .expect("directory-scoped prefix must not false-positive on sibling names");
    }

    /// #204 (tur-3 P0) GERÇEK git repro: `mv a b && git add -N b` üretim akışı
    /// porcelain'da worktree-side rename getirir (incelemeci ` R b\0a\0` gördü).
    /// Sözleşme: git hangi XY şeklini seçerse seçsin, eski yol dahil HER İKİ uç
    /// dirty kümesinde kalmalı — eski uç düşerse/çöp-girerse global `D ⊆ A`
    /// fence'i yanlış karar verirdi.
    #[test]
    fn worktree_side_rename_real_git_repro() {
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
            out.stdout
        };
        let _ = run(&["init", "-q"]);
        let _ = run(&["config", "user.email", "t@t"]);
        let _ = run(&["config", "user.name", "t"]);
        std::fs::write(dir.path().join("a.rs"), "fn main() {}\n").unwrap();
        let _ = run(&["add", "."]);
        let _ = run(&["commit", "-qm", "init"]);
        // İncelemecinin repro dizisi: taşı + intent-to-add.
        std::fs::rename(dir.path().join("a.rs"), dir.path().join("b.rs")).unwrap();
        let _ = run(&["add", "-N", "b.rs"]);
        let raw = run(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ]);
        let parsed =
            parse_porcelain_z(&String::from_utf8_lossy(&raw)).expect("real git output must parse");
        assert!(
            parsed.contains("a.rs") && parsed.contains("b.rs"),
            "rename ends must both stay dirty: parsed = {parsed:?}"
        );
    }

    /// #204 (tur-4 P1-1 / re-review P2): İKİ SEVİYELİ gerçek submodule —
    /// recursive gitlink traversal'ın doğrudan regresyonu. Temiz zincirde
    /// (parent→mod1→mod2, her seviyede HEAD==gitlink) nested yaprak tracked
    /// sete girer; nested HEAD gitlink'ten sapınca o seviyenin path'leri yetkili
    /// KABUL EDİLMEZ (setten düşer).
    #[test]
    fn nested_gitlinks_tracked_two_levels_real_git() {
        let parent = tempfile::tempdir().unwrap();
        let mod1_src = tempfile::tempdir().unwrap();
        let mod2_src = tempfile::tempdir().unwrap();
        let git = |cwd: &std::path::Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
                // file:// submodule kurulumu (git 2.38+ protokol kısıtı) —
                // test ortamı açar; capture kendisi yalnız okur.
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
                .env("GIT_CONFIG_VALUE_0", "always")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?} in {} failed: {}",
                cwd.display(),
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let init = |cwd: &std::path::Path| {
            git(cwd, &["init", "-q"]);
            git(cwd, &["config", "user.email", "t@t"]);
            git(cwd, &["config", "user.name", "t"]);
        };
        let file_url =
            |p: &std::path::Path| format!("file:///{}", p.to_str().unwrap().replace('\\', "/"));

        // En iç: mod2 (src/leaf.cs).
        init(mod2_src.path());
        std::fs::create_dir_all(mod2_src.path().join("src")).unwrap();
        std::fs::write(mod2_src.path().join("src/leaf.cs"), "class Leaf {}\n").unwrap();
        git(mod2_src.path(), &["add", "-A"]);
        git(mod2_src.path(), &["commit", "-qm", "mod2 init"]);
        // Orta: mod1 (src/a.ts + İÇİNDE mod2 submodule).
        init(mod1_src.path());
        std::fs::create_dir_all(mod1_src.path().join("src")).unwrap();
        std::fs::write(mod1_src.path().join("src/a.ts"), "export const a = 1;\n").unwrap();
        git(mod1_src.path(), &["add", "-A"]);
        git(
            mod1_src.path(),
            &["submodule", "add", &file_url(mod2_src.path()), "mod2"],
        );
        git(mod1_src.path(), &["add", "-A"]);
        git(mod1_src.path(), &["commit", "-qm", "mod1 init"]);
        // Parent: main.rs + İÇİNDE mod1 submodule; recursive init.
        init(parent.path());
        std::fs::write(parent.path().join("main.rs"), "fn main() {}\n").unwrap();
        git(parent.path(), &["add", "-A"]);
        git(
            parent.path(),
            &["submodule", "add", &file_url(mod1_src.path()), "mod1"],
        );
        git(
            parent.path(),
            &["submodule", "update", "--init", "--recursive"],
        );
        git(parent.path(), &["add", "-A"]);
        git(parent.path(), &["commit", "-qm", "parent init"]);
        assert!(
            parent.path().join("mod1/mod2/src/leaf.cs").exists(),
            "two-level recursive init expected"
        );

        // Temiz zincir: İKİ seviye de açılır — nested yaprak tracked sette.
        let snap = RepositorySnapshot::capture(parent.path()).expect("clean capture");
        assert!(
            snap.tracked_paths.contains("mod1/mod2/src/leaf.cs"),
            "two-level nested leaf must be tracked: {:?}",
            snap.tracked_paths
        );
        assert!(snap.tracked_paths.contains("mod1/src/a.ts"));

        // Sapma: mod2'yi parent worktree'inde ileri taşı (yeni commit) →
        // mod2 HEAD ≠ mod1'in bağladığı gitlink → o seviyenin nested path'leri
        // yetkili kabul EDİLMEZ (recursive expansion o düğümde durur).
        std::fs::write(
            parent.path().join("mod1/mod2/src/leaf2.cs"),
            "class L2 {}\n",
        )
        .unwrap();
        git(parent.path().join("mod1/mod2").as_path(), &["add", "-A"]);
        git(
            parent.path().join("mod1/mod2").as_path(),
            &["commit", "-qm", "mod2 drift"],
        );
        let drifted = RepositorySnapshot::capture(parent.path()).expect("drifted capture");
        assert!(
            !drifted.tracked_paths.contains("mod1/mod2/src/leaf.cs"),
            "drifted nested level must lose tracked authority: {:?}",
            drifted.tracked_paths
        );
        assert!(
            drifted.tracked_paths.contains("mod1/src/a.ts"),
            "temiz üst seviye yetkili kalır"
        );
    }
}
