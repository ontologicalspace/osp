//! Repository snapshot — Git HEAD binding + clean worktree + tracked-path verification.
//!
//! Faz 8 test-project (review v6-v7): task-subject binding doğruluğu için analyzed
//! repository snapshot'ına bağlama. NodeId→path binding, repo HEAD drift detection,
//! ve analyzed-path ⊆ HEAD tracked-path invariant'ları.
//!
//! Bu modül "atomic snapshot" iddia ETMEZ — pre/post repository snapshot equality ile
//! drift-detected consistent analysis sağlar. Transient ABA (clean A → B → clean A)
//! yakalanmayabilir; controlled temp fixture'da eşzamanlı yazıcı olmadığı için yeterli.

#![allow(dead_code, reason = "Faz 8 test-project: wired incrementally across commits")]

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

/// Repository snapshot — HEAD + tracked paths + clean worktree.
///
/// `capture(repo)` Git komutlarıyla bu üç bilgiyi toplar. İki snapshot'ın `PartialEq`
/// karşılaştırması drift detection için kullanılır (pre/post analysis fence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    pub head: GitCommitId,
    pub tracked_paths: BTreeSet<String>,
    pub clean: bool,
}

impl RepositorySnapshot {
    /// Repository snapshot'ı capture eder — `git rev-parse HEAD`, `git ls-tree`, `git status`.
    ///
    /// `clean` = `git status --porcelain --untracked-files=all` çıktısı boş.
    /// tracked_paths = `git ls-tree -r --name-only HEAD` çıktısı (sorted).
    pub fn capture(repo: &Path) -> Result<Self, RepoSnapshotError> {
        let head = capture_head(repo)?;
        let tracked_paths = capture_tracked_paths(repo)?;
        let clean = capture_clean(repo)?;
        Ok(Self {
            head,
            tracked_paths,
            clean,
        })
    }
}

/// Repository snapshot capture hatası.
#[derive(Debug, thiserror::Error)]
pub enum RepoSnapshotError {
    #[error("invalid repository HEAD (expected full 40-char hex SHA): {value:?}")]
    InvalidRepositoryHead { value: String },
    #[error("git command failed ({command}): {detail}")]
    GitCommandFailed { command: &'static str, detail: String },
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
    Ok(output.lines().filter(|l| !l.is_empty()).map(String::from).collect())
}

fn capture_clean(repo: &Path) -> Result<bool, RepoSnapshotError> {
    let output = run_git(
        repo,
        &["status", "--porcelain", "--untracked-files=all"],
        "status",
    )?;
    Ok(output.is_empty())
}

/// Harness eligibility: clean worktree zorunlu (review P0-3).
pub fn ensure_snapshot_eligible(snapshot: &RepositorySnapshot) -> Result<(), RepoSnapshotError> {
    if !snapshot.clean {
        return Err(RepoSnapshotError::GitCommandFailed {
            command: "status",
            detail: "dirty or untracked working tree — harness requires clean worktree".to_string(),
        });
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
