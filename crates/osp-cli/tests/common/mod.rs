//! Paylaşılan test fixture'ı — gerçek git repo + dışarıda work CWD + HEAD.
//!
//! #173: iki kurucu — `new()` mod-bildirimlidir (güncel Rust adaptöründe 0 kenar;
//! yalnız wiring/rejection testleri), `new_with_use_edges()` gerçek `use` kenarları
//! üretir (main.rs → a.rs + b.rs, coupling 2/3). Graf önermesi taşıyan testler
//! önermeyi `osp analyze` çıktısından ÖLÇEREK pinler (vacuous pass yasak).
//! #172: defter komut testleri (defter_flow.rs) aynı fixture'ı paylaşır.
#![allow(dead_code)]

use std::fs;
use std::process::Command;
use std::sync::Mutex;

use assert_cmd::prelude::*;

/// Serializes `osp trajectory attempt` invocations (defensive — CWD isolation handles
/// the real contamination, but the lock keeps output deterministic under load).
pub static OSP_ATTEMPT_LOCK: Mutex<()> = Mutex::new(());

// ═══════════════════════════════════════════════════════════════════════════════
// HarnessFixture — owns repo tempdir + work tempdir (CWD) + HEAD
// ═══════════════════════════════════════════════════════════════════════════════

/// Bundles the analyzed git repo, the out-of-repo working dir (CWD), and the fixture HEAD.
/// `work` is a separate tempdir so `.osp/` writes and task/proposal files don't dirty
/// the repo's git status (which would fail snapshot eligibility).
pub struct HarnessFixture {
    repo: tempfile::TempDir,
    work: tempfile::TempDir,
    pub head: String,
}

impl HarnessFixture {
    /// Build a deterministic git fixture: main.rs imports a.rs + b.rs (2 outgoing value imports).
    /// CRLF normalization disabled to prevent dirty-worktree cross-contamination.
    pub fn new() -> Self {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let r = repo.path();
        let git = |args: &[&str]| {
            Command::new("git")
                .args(["-C", r.to_str().unwrap()])
                .args(args)
                .status()
                .expect("git")
        };
        Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(r)
            .status()
            .expect("git init");
        git(&["config", "user.email", "osp-test@example.invalid"]);
        git(&["config", "user.name", "OSP Test"]);
        git(&["config", "core.autocrlf", "false"]);
        git(&["config", "core.eol", "lf"]);
        // Deterministic commit (review P1-5): fixed author/committer dates → stable SHA
        // across machines/clocks.
        let commit = |args: &[&str]| {
            Command::new("git")
                .args(["-C", r.to_str().unwrap()])
                .args(args)
                .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
                .status()
                .expect("git")
        };
        // main.rs imports a + b → 2 outgoing value imports → coupling 2/3 = 0.667.
        // NOT (#173): `mod` bildirimleri güncel Rust adaptöründe kenar ÜRETMEZ
        // (yalnız `use_declaration` sayılır) — bu fixture'ta gerçek coupling 0'dır.
        // Kenar-gerektiren testler `new_with_use_edges()` kullanır.
        fs::write(
            r.join("main.rs"),
            "mod a;\nmod b;\npub fn main() { a::a(); b::b(); }\n",
        )
        .expect("write main.rs");
        fs::write(r.join("a.rs"), "pub fn a() {}\n").expect("write a.rs");
        fs::write(r.join("b.rs"), "pub fn b() {}\n").expect("write b.rs");
        git(&["add", "-A"]);
        commit(&["commit", "-qm", "init"]);
        let head = String::from_utf8(
            Command::new("git")
                .args(["-C", r.to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let work = tempfile::tempdir().expect("work tempdir (CWD)");
        Self { repo, work, head }
    }

    /// #166 fixture'ı: `use`-declaration tabanlı GERÇEK import kenarları.
    /// main.rs → a.rs + b.rs (2 çıkan kenar, coupling 2/3 = 0.6667). Mevcut
    /// `new()`'in `mod` bildirimleri güncel Rust adaptöründe kenar üretmez
    /// (yalnız `use_declaration` sayılır — #173); kenar-gerektiren testler
    /// bu kurucuyu kullanır. `mod` satırları KENAR ÜRETMEZ ama fixture'ı
    /// geçerli bir Rust programı yapar (P2-3) — graf önermesi ayrıca
    /// `use_edge_fixture_graph_premise_holds` ile analizden assert edilir.
    pub fn new_with_use_edges() -> Self {
        let fx = Self::new();
        let r = fx.repo_path();
        fs::write(r.join("a.rs"), "pub struct A;\n").expect("write a.rs");
        fs::write(r.join("b.rs"), "pub struct B;\n").expect("write b.rs");
        fs::write(
            r.join("main.rs"),
            "mod a;\nmod b;\nuse crate::a::A;\nuse crate::b::B;\npub fn main() { let _ = (A, B); }\n",
        )
        .expect("write main.rs");
        // Test tempdir'i (kullanıcı reporu değil): fixture plumbing'inde add -A güvenli.
        let add = Command::new("git")
            .args(["-C", r.to_str().unwrap(), "add", "-A"])
            .status()
            .expect("git add");
        assert!(add.success(), "use-edges add must succeed");
        let commit = Command::new("git")
            .args(["-C", r.to_str().unwrap(), "commit", "-qm", "use-edges"])
            .env("GIT_AUTHOR_DATE", "2001-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00Z")
            .status()
            .expect("git commit (use-edges)");
        assert!(commit.success(), "use-edges commit must succeed");
        let head = String::from_utf8(
            Command::new("git")
                .args(["-C", r.to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        Self {
            repo: fx.repo,
            work: fx.work,
            head,
        }
    }

    pub fn repo_path(&self) -> &std::path::Path {
        self.repo.path()
    }

    pub fn work_path(&self) -> &std::path::Path {
        self.work.path()
    }

    /// Write task JSON to the work CWD, return its path.
    pub fn write_task(&self, value: &serde_json::Value) -> std::path::PathBuf {
        let path = self.work_path().join("task.v1.json");
        fs::write(&path, serde_json::to_string_pretty(value).unwrap()).unwrap();
        path
    }

    /// Write a proposals JSON (one RemoveImport from_node→to_node) to the work CWD.
    pub fn write_proposals(&self, from_node: u64, to_node: u64) -> std::path::PathBuf {
        let proposals = serde_json::json!([{
            "new_nodes": [],
            "new_edges": [],
            "removed_edges": [{"from": from_node, "to": to_node, "kind": "Imports"}],
            "affected_nodes": [from_node],
            "modified_entities": [],
            "position_hints": [],
            "reasoning": "remove import to reduce coupling below threshold"
        }]);
        let path = self.work_path().join("proposals.json");
        fs::write(&path, serde_json::to_string_pretty(&proposals).unwrap()).unwrap();
        path
    }

    /// Write a path-keyed proposals v2 envelope (one RemoveImport from_path→to_path).
    /// #161/B5: NodeId alanları repo-relative path; R1 P1-1 sonrası envelope
    /// `repository_head` taşır — proposal'ın üretildiği state, re-bind'ten önce
    /// exact-match fence'e girer.
    pub fn write_proposals_v2(
        &self,
        head: &str,
        from_path: &str,
        to_path: &str,
    ) -> std::path::PathBuf {
        let proposals = serde_json::json!({
            "schema_version": 2,
            "repository_head": head,
            "proposals": [{
                "removed_edges": [{"from": from_path, "to": to_path, "kind": "Imports"}],
                "affected_nodes": [from_path],
                "reasoning": "remove import to reduce coupling below threshold"
            }]
        });
        let path = self.work_path().join("proposals.v2.json");
        fs::write(&path, serde_json::to_string_pretty(&proposals).unwrap()).unwrap();
        path
    }

    /// Write raw string to the work CWD (for malformed-JSON tests).
    pub fn write_raw_task(&self, name: &str, content: &str) -> std::path::PathBuf {
        let path = self.work_path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    /// Run `osp trajectory attempt` with harness mode + task file. Returns the output.
    /// CWD + --state-dir = work tempdir (outside repo → git status clean, .osp/ isolated,
    /// harness state-dir invariant satisfied — review B-3 P0).
    /// `format` = "human" (default) veya "json" (versioned run envelope).
    pub fn run_attempt(
        &self,
        task_path: &std::path::Path,
        proposals_path: &std::path::Path,
        positional_task_id: u64,
        format: &str,
    ) -> std::process::Output {
        let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        Command::cargo_bin("osp")
            .expect("osp binary")
            .current_dir(self.work_path())
            .arg("trajectory")
            .arg("attempt")
            .arg(positional_task_id.to_string())
            .arg("--repo")
            .arg(self.repo_path())
            .arg("--execution-mode")
            .arg("harness")
            .arg("--witness")
            .arg("harness-auto-approve")
            .arg("--llm")
            .arg("mock")
            .arg("--proposals")
            .arg(proposals_path)
            .arg("--task")
            .arg(task_path)
            .arg("--state-dir")
            .arg(self.work_path()) // harness invariant: state-dir outside repo (P0)
            .arg("--format")
            .arg(format)
            .output()
            .expect("run osp")
    }

    /// Assert the analyzed repo has no `.osp/` state (no side-effects leaked into repo).
    /// Review P1-2 — fail-closed must leave the repo untouched.
    pub fn assert_repo_clean(&self) {
        // git status --porcelain must be empty (no .osp/, no held artifacts).
        let status = Command::new("git")
            .args([
                "-C",
                self.repo_path().to_str().unwrap(),
                "status",
                "--porcelain",
                "--untracked-files=all",
            ])
            .output()
            .expect("git status");
        let porcelain = String::from_utf8_lossy(&status.stdout);
        assert!(
            porcelain.trim().is_empty(),
            "analyzed repo must be clean after attempt (review P1-2), but git status:\n{porcelain}"
        );
        assert!(
            !self.repo_path().join(".osp").exists(),
            "no .osp/ state should leak into analyzed repo (review P1-2)"
        );
    }

    /// Run an arbitrary `osp trajectory attempt` (no task file) — serialized.
    pub fn run_attempt_no_task<F>(&self, build: F) -> std::process::Output
    where
        F: FnOnce(&mut Command) -> &mut Command,
    {
        let _guard = OSP_ATTEMPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = Command::cargo_bin("osp").expect("osp binary");
        cmd.current_dir(self.work_path())
            .arg("trajectory")
            .arg("attempt");
        build(&mut cmd);
        cmd.output().expect("run osp")
    }

    /// #173 review tur-1 (P1-2): fixture'ın ÖLÇÜLEN baseline coupling'i —
    /// `osp analyze` çıktısından main.rs düğümünün coupling değeri. Evidence
    /// `before` bootstrap tohumu (Placeholder 0.7) olduğundan, gerçek graf
    /// geçişi (2/3 → 1/2) bu kanaldan ayrıca doğrulanır: epistemik kaynakları
    /// farklı iki sayı "improvement" kanıtı olarak karşılaştırılamaz.
    pub fn measured_main_coupling(&self) -> f64 {
        let output = Command::cargo_bin("osp")
            .expect("osp binary")
            .current_dir(self.work_path())
            .arg("analyze")
            .arg(self.repo_path())
            .arg("--format")
            .arg("json")
            .output()
            .expect("run osp analyze");
        assert!(
            output.status.success(),
            "analyze must succeed. stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let space: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("analyze json parses: {e}. stdout={stdout}"));
        space["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|n| n["path"].as_str().map(|p| p.ends_with("main.rs")) == Some(true))
            .expect("main.rs node")["coupling"]["value"]
            .as_f64()
            .expect("main.rs coupling value")
    }
}

/// Build a harness task JSON envelope. anchor defaults: Node 0 = "a.rs", 1 = "b.rs",
/// 2 = "main.rs" (analyzer alphabetical ordering).
pub fn task_envelope(head: &str, anchor_node_id: u64) -> serde_json::Value {
    let anchor_path = match anchor_node_id {
        0 => "a.rs",
        1 => "b.rs",
        2 => "main.rs",
        // Submodule fixture: alfabetik keşif sırası a.rs, b.rs, clients/fe/src/main.ts,
        // main.rs → main.rs node 3'e kayar (#156 R2 P1-2: exact kontrat gerçek
        // node kimliğine bağlanır).
        3 => "main.rs",
        _ => "a.rs",
    };
    serde_json::json!({
        "schema_version": 1,
        "repository_head": head,
        "scope_bindings": [{"node_id": anchor_node_id, "expected_path": anchor_path}],
        "task": {
            "id": 7,
            "milestone_id": 1,
            "label": "completed-loop fixture",
            "target_predicate_set": {
                "mode": "All",
                "predicates": [{
                    "predicate": {
                        "metric": "Coupling",
                        "operator": "Le",
                        "threshold": 0.55,
                        "scope": {"Node": anchor_node_id},
                        // Live Contract v1 authority profile (#151 Faz 0): coupling
                        // attempt pipeline'da TreeSitter ölçülür — declare edilmesi zorunlu
                        // (eski null, #96 MD-2 regolden bağlamındaydı; preflight artık
                        // coupling'de declaration'ı şart koşuyor).
                        "required_source": "TreeSitter",
                        "tolerance": 0.0
                    },
                    "weight": null
                }],
                "preferred_vector": {"x": 0.55, "y": 0.6, "z": 0.5, "w": 0.5, "v": 0.3}
            },
            "policy": {
                "predicate_failure_policy": "StrictReject",
                "min_improvement_delta": 0.02,
                "max_axis_regression": 0.15,
                "maneuver_limit": 3,
                "allow_progress_checkpoint": false
            },
            "allowed_operations": ["RemoveImport"],
            "constraints": [],
            "status": "Pending"
        }
    })
}

/// #161/B5: path-keyed harness task envelope (v2). Anchor path doğrudan dosya
/// yoludur; node-id, attempt anındaki taze baseline'a karşı CLI'da çözümlenir.
pub fn task_envelope_v2(head: &str, anchor_path: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 2,
        "repository_head": head,
        "scope_bindings": [{"path": anchor_path}],
        "task": {
            "id": 7,
            "milestone_id": 1,
            "label": "completed-loop fixture (path-keyed v2)",
            "target_predicate_set": {
                "mode": "All",
                "predicates": [{
                    "predicate": {
                        "metric": "Coupling",
                        "operator": "Le",
                        "threshold": 0.55,
                        "scope": {"Path": anchor_path},
                        "required_source": "TreeSitter",
                        "tolerance": 0.0
                    },
                    "weight": null
                }],
                "preferred_vector": {"x": 0.55, "y": 0.6, "z": 0.5, "w": 0.5, "v": 0.3}
            },
            "policy": {
                "predicate_failure_policy": "StrictReject",
                "min_improvement_delta": 0.02,
                "max_axis_regression": 0.15,
                "maneuver_limit": 3,
                "allow_progress_checkpoint": false
            },
            "allowed_operations": ["RemoveImport"],
            "constraints": [],
            "status": "Pending"
        }
    })
}
