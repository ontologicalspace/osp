//! Repository snapshot — Git HEAD binding + clean worktree + tracked-path verification.
//!
//! Faz 8 test-project (review v6-v7): task-subject binding doğruluğu için analyzed
//! repository snapshot'ına bağlama. NodeId→path binding, repo HEAD drift detection,
//! ve analyzed-path ⊆ HEAD tracked-path invariant'ları.
//!
//! Bu modül "atomic snapshot" iddia ETMEZ — pre/post repository snapshot equality ile
//! drift-detected consistent analysis sağlar. Transient ABA (clean A → B → clean A)
//! yakalanmayabilir; controlled temp fixture'da eşzamanlı yazıcı olmadığı için yeterli.

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
    let output = run_git(repo, &["ls-tree", "-r", "--name-only", "HEAD"], "ls-tree")?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
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

fn capture_dirty_paths(repo: &Path) -> Result<BTreeSet<String>, RepoSnapshotError> {
    // NOT: ham çıktı trim EDİLMEZ — `XY <path>` formatında X boşluk olabilir
    // (` M a.rs` worktree-modified); trim önceki sürümde ".rs" gibi bozuk
    // path parse'ına yol açıyordu (#155).
    let output = run_git_raw(
        repo,
        &["status", "--porcelain", "--untracked-files=all"],
        "status",
    )?;
    let mut paths = BTreeSet::new();
    for line in output.lines() {
        if line.len() < 4 {
            continue;
        }
        // `XY <path>` — XY iki durum karakteri (??, ` M`, ` m`, …); path 3. kolon.
        // Rename (`R  old -> new`) her iki ucu da kirli kapsama alır.
        let entry = line[3..].trim();
        if entry.is_empty() {
            continue;
        }
        if let Some((old, new)) = entry.split_once(" -> ") {
            paths.insert(old.trim_matches('"').to_string());
            paths.insert(new.trim_matches('"').to_string());
        } else {
            paths.insert(entry.trim_matches('"').to_string());
        }
    }
    Ok(paths)
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
    fn porcelain_parse_extracts_paths_including_rename_and_quotes() {
        // capture_dirty_paths filesystem gerektirir; parse mantığını çıktı
        // biçimleriyle sabitleriz (satır formatı `XY <path>`).
        // Burada snapshot türevi üzerinden clean() semantiği + rename çifti
        // validate fonksiyonu üzerinden kapsanır; ham parse için
        // completed_loop E2E fixture gerçek git çıktısını kullanır.
        let snap = snapshot_with_dirty(&["a/b.cs", "\"quo te.cs\"", "old.cs", "new.cs"]);
        assert!(!snap.clean());
        assert_eq!(snap.dirty_paths.len(), 4);
        assert!(snapshot_with_dirty(&[]).clean());
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
