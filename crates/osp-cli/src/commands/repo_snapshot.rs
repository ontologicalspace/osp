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
/// `capture(repo)` Git komutlarıyla bu üç bilgiyi toplar. İki snapshot'ın `PartialEq`
/// karşılaştırması drift detection için kullanılır (pre/post analysis fence).
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

    // Submodule-aware genişletme (#155 P1-1): initialized submodule'un worktree HEAD'i
    // parent gitlink SHA'sıyla EŞLEŞİYORSA, submodule HEAD tree'sini `<submodule>/`
    // önekiyle tracked sete kat — revision identity hiyerarşiktir
    // (superproject HEAD → gitlink SHA → submodule tree → analyzed file).
    // Eşleşmiyorsa içerik parent revizyonundan farklıdır; `git status` submodule'u
    // dirty listeler → analyzed-scope prefix fence reddeder (aşağıda).
    let long_format = run_git_raw(repo, &["ls-tree", "-r", "HEAD"], "ls-tree")?;
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
        let sub_dir = repo.join(sub_path);
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
        let Ok(nested) = run_git(
            &sub_dir,
            &["ls-tree", "-r", "--name-only", "HEAD"],
            "ls-tree (submodule)",
        ) else {
            continue;
        };
        for p in nested.lines().filter(|l| !l.is_empty()) {
            paths.insert(format!("{sub_path}/{p}"));
        }
    }
    Ok(paths)
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
/// YOKTUR (`quo"te.rs`, `türkçe.rs` olduğu gibi gelir). Klasik porcelain'un quote'lu
/// çıktısında `trim_matches('"')` escape'leri çözmezdi ve fence fail-open olabilirdi.
/// Rename (`R`) ve copy (`C`) kayıtları `<new>\0<old>` şeklinde İKİ NUL'lu kayıttır —
/// her iki uç da kirli kapsama alınır.
pub(crate) fn parse_porcelain_z(raw: &str) -> BTreeSet<String> {
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
        if xy.starts_with('R') || xy.starts_with('C') {
            // Rename/copy: izleyen ek kayıt eski path'tir.
            if let Some(old) = records.next() {
                if !old.is_empty() {
                    paths.insert(old.to_string());
                }
            }
        }
    }
    paths
}

fn capture_dirty_paths(repo: &Path) -> Result<BTreeSet<String>, RepoSnapshotError> {
    // -z: NUL-ayrı, quote/escape'siz, deterministik (#155 P1-2).
    let output = run_git_raw(
        repo,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        "status",
    )?;
    Ok(parse_porcelain_z(&output))
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

/// #155: post-attempt drift fence — saf fonksiyon (test edilebilir exact matrix).
///
/// Sözleşme (#155 kabul kriterleri):
/// - HEAD değişti → red (revision identity bozuldu; global),
/// - tracked-path seti değişti → red (revision tanımladığı dosya seti değişti; global),
/// - analyzed kapsam dirty'leşti → red (ölçülen içerik attempt penceresinde drift etti),
/// - analiz DIŞINDAKİ bir path dirty'leşti → kabul (ölçüm ondan türetilmedi).
pub fn validate_post_attempt_snapshot(
    before: &RepositorySnapshot,
    after: &RepositorySnapshot,
    node_paths: &std::collections::HashMap<u64, String>,
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
    validate_analyzed_paths_clean(node_paths, after, "after attempt")
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
                detail: format!("analyzed path not tracked in HEAD: {path}"),
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
        let parsed = parse_porcelain_z(raw);
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
        let parsed = parse_porcelain_z(tricky);
        assert!(parsed.contains("quo\"te.rs"), "got: {parsed:?}");
        assert!(parsed.contains("türkçe.rs"), "got: {parsed:?}");

        // Rename: `R  new\0old` — iki NUL'lu kayıt, iki uç da kirli kapsamda.
        let rename = "R  src/new.rs\0src/old.rs\0";
        let parsed = parse_porcelain_z(rename);
        assert_eq!(
            parsed,
            BTreeSet::from(["src/new.rs".to_string(), "src/old.rs".to_string()])
        );

        // Boş/çok kısa kayıtlar sessizce atlanır; boş çıktı → boş küme (clean).
        assert!(parse_porcelain_z("").is_empty());
        assert!(parse_porcelain_z("\0\0").is_empty());
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

        // Hepsi clean → geçer.
        validate_post_attempt_snapshot(&mk(&[]), &mk(&[]), &analyzed)
            .expect("clean→clean must pass");

        // Analyzed kapsam dirty'leşti → red.
        let err = validate_post_attempt_snapshot(&mk(&[]), &mk(&["a.rs"]), &analyzed)
            .expect_err("analyzed path dirty after attempt must reject");
        assert!(format!("{err}").contains("after attempt"), "{err}");

        // Analiz DIŞI path dirty'leşti → kabul (ölçüm ondan türetilmedi).
        validate_post_attempt_snapshot(&mk(&[]), &mk(&["notes/local.md"]), &analyzed)
            .expect("out-of-scope drift after attempt must be accepted");

        // HEAD değişti → red.
        let before = mk(&[]);
        let err = validate_post_attempt_snapshot(&before, &head_variant(&before), &analyzed)
            .expect_err("HEAD moved during attempt must reject");
        assert!(format!("{err}").contains("HEAD moved"), "{err}");

        // Tracked set değişti → red.
        let err = validate_post_attempt_snapshot(&before, &tracked_variant(&before), &analyzed)
            .expect_err("tracked set change during attempt must reject");
        assert!(
            format!("{err}").contains("tracked-path set changed"),
            "{err}"
        );
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
}
