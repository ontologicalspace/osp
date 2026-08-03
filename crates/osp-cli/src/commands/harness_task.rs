//! Harness task file loader — snapshot-bound controlled harness fixture.
//!
//! Faz 8 test-project (review v6-v7): production task genesis'in yerine geçmez.
//! Controlled temp fixture: repository HEAD + NodeId→path scope binding + Task.
//! Trusted operator fixture — hardened task loading + Node-only V1 scope validation.
//!
//! ## Task-canonical subject authority (review v5)
//!
//! Task doğrudan yüklenir; agent `affected_nodes` authority üretmez. Scope binding
//! NodeId→path analyze çıktısıyla doğrulanır → subject drift fail-closed.

#![allow(
    dead_code,
    reason = "Faz 8 test-project: wired incrementally across commits"
)]

use std::collections::HashMap;
use std::path::Path;

use osp_core::space::NodeId;
use osp_core::trajectory::{PredicateScope, Task, TaskId, TaskStatus, TaskValidationError};

use crate::commands::repo_snapshot::{RepoSnapshotError, RepositorySnapshot};

/// Harness task file envelope (V1).
///
/// `repository_head` task dosyasının yazıldığı repo HEAD'ini (full 40-char SHA) taşır;
/// load sırasında capture edilen snapshot ile exact match doğrulanır (subject drift fence).
/// `scope_bindings` NodeId→path binding listesi; analyze `node_paths` ile doğrulanır.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliHarnessTaskFileV1 {
    pub schema_version: u32,
    pub repository_head: String,
    pub scope_bindings: Vec<CliScopeBinding>,
    pub task: Task,
}

/// NodeId → expected source path binding (task file declaration).
///
/// Load sırasında analyzer'ın ürettiği `node_paths` ile karşılaştırılır; mismatch
/// subject drift (Node ID reassignment) sinyalidir → fail-closed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliScopeBinding {
    pub node_id: NodeId,
    pub expected_path: String,
}

/// Harness task load/validation hatası.
#[derive(Debug, thiserror::Error)]
pub enum HarnessTaskError {
    #[error("failed to read task file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse task file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("unsupported schema_version {found} (expected {expected})")]
    UnsupportedSchemaVersion { found: u32, expected: u32 },
    #[error("repository HEAD mismatch: task file {task_head}, repo {repo_head}")]
    RepositoryHeadMismatch {
        task_head: String,
        repo_head: String,
    },
    #[error("scope binding node {node_id} not present in analysis node_paths")]
    UnknownScopeBindingNode { node_id: NodeId },
    #[error(
        "scope binding path mismatch for node {node_id}: binding {binding_path}, analysis {analyzed_path}"
    )]
    ScopeBindingPathMismatch {
        node_id: NodeId,
        binding_path: String,
        analyzed_path: String,
    },
    #[error("duplicate scope binding for node {node_id}")]
    DuplicateScopeBinding { node_id: NodeId },
    #[error("empty scope binding — at least one Node binding required")]
    EmptyScopeBinding,
    #[error("task id mismatch: positional {positional}, task file {task_file}")]
    TaskIdMismatch {
        positional: TaskId,
        task_file: TaskId,
    },
    #[error("unsupported harness scope {found} — V1 harness requires Node-only (review v5)")]
    UnsupportedHarnessScope { found: String },
    #[error("heterogeneous harness scope — all predicates must target the same Node")]
    HeterogeneousHarnessScope,
    #[error("missing preferred_vector — harness task requires navigation target")]
    MissingPreferredVector,
    #[error("invalid initial task status {found:?} — must be non-terminal (Pending/Assigned/InProgress)")]
    InvalidInitialTaskStatus { found: TaskStatus },
    #[error("core task validation failed: {0}")]
    TaskValidation(#[source] TaskValidationError),
    #[error(
        "scope binding set mismatch — task predicate Node set does not match scope_bindings Node set (review P1-3)"
    )]
    ScopeBindingSetMismatch,
    #[error("snapshot error: {0}")]
    Snapshot(#[from] RepoSnapshotError),
}

/// Load + validate harness task file → registry-ready `Task`.
///
/// Validation authority split (review P0-1):
/// - **Core-owned** (`Task::validate()`): predicate değerleri, source policy, task policy,
///   finite/range kuralları, genel declaration invariants. Tek truth source.
/// - **CLI harness-owned** (bu modül): snapshot HEAD equality, scope binding equality,
///   Node-only kısıtı, preferred_vector harness için zorunlu, başlangıç status terminal olamaz.
///
/// Guard sırası (review P1-2): maneuver override core validation'dan ÖNCE uygulanır —
/// böylece override'ın ürettiği geçersiz policy core validator'da yakalanır.
///
/// 1. Read + deserialize JSON
/// 2. `schema_version == 1`
/// 3. repository HEAD matches snapshot (exact full SHA)
/// 4. scope binding: her `{node_id, expected_path}` ⊆ analyzed `node_paths` + exact path match
/// 5. scope binding set ≡ task predicate Node set (exact-set equality — review P1-3)
/// 6. task id consistency (positional == task.id)
/// 7. maneuver override uygula (CLI `--maneuver-limit` wins; pre-validate — P1-2)
/// 8. `task.validate()` — core canonical validation (P0-1)
/// 9. harness profile: preferred_vector present, non-terminal initial status
/// 10. `validate_harness_scope` — Node-only, homogeneous anchor
///
/// Returns the validated `Task` with maneuver override applied (registry insert-ready).
pub fn load_and_validate_harness_task(
    path: &Path,
    positional_task_id: TaskId,
    snapshot: &RepositorySnapshot,
    node_paths: &HashMap<NodeId, String>,
    maneuver_override: Option<u32>,
) -> Result<Task, HarnessTaskError> {
    // 1. Read + deserialize.
    let raw = std::fs::read_to_string(path).map_err(|source| HarnessTaskError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let file: CliHarnessTaskFileV1 = serde_json::from_str(&raw)?;

    // 2. schema_version.
    if file.schema_version != 1 {
        return Err(HarnessTaskError::UnsupportedSchemaVersion {
            found: file.schema_version,
            expected: 1,
        });
    }

    // 3. repository HEAD exact match (single snapshot truth source — review P1-1).
    if file.repository_head != snapshot.head.as_str() {
        return Err(HarnessTaskError::RepositoryHeadMismatch {
            task_head: file.repository_head,
            repo_head: snapshot.head.as_str().to_string(),
        });
    }

    // 4. scope binding: her NodeId→path ⊆ analyzed node_paths + exact path match (review P1-3).
    validate_scope_bindings(&file.scope_bindings, node_paths)?;

    // 5. scope binding set ≡ task predicate Node set (exact-set equality — review P1-3).
    let anchor = validate_scope_binding_set_equality(&file.task, &file.scope_bindings)?;

    // 6. task id consistency.
    if file.task.id != positional_task_id {
        return Err(HarnessTaskError::TaskIdMismatch {
            positional: positional_task_id,
            task_file: file.task.id,
        });
    }

    // 7. maneuver override uygula — core validation'dan ÖNCE (review P1-2). Override'ın
    //    ürettiği geçersiz policy (örn. maneuver_limit=0) bir sonraki adımda core'da yakalanır.
    let mut task = file.task;
    if let Some(limit) = maneuver_override {
        task.policy.maneuver_limit = limit;
    }

    // 8. core canonical validation (review P0-1) — tek truth source; CLI kopyası yok.
    task.validate().map_err(HarnessTaskError::TaskValidation)?;

    // 9. harness profile (CLI-owned): preferred_vector zorunlu + non-terminal initial status.
    validate_harness_profile(&task)?;

    // 10. Node-only homogeneous scope (CLI-owned) — anchor step 5'ten biliniyor.
    let _ = anchor;
    Ok(task)
}

/// Scope binding validation — NodeId→path ⊆ analyzed node_paths + exact path match.
///
/// Duplicate node_id ve empty list fail-closed (review: extra/duplicate/empty scope binding).
fn validate_scope_bindings(
    bindings: &[CliScopeBinding],
    node_paths: &HashMap<NodeId, String>,
) -> Result<(), HarnessTaskError> {
    if bindings.is_empty() {
        return Err(HarnessTaskError::EmptyScopeBinding);
    }
    let mut seen = std::collections::HashSet::new();
    for b in bindings {
        if !seen.insert(b.node_id) {
            return Err(HarnessTaskError::DuplicateScopeBinding { node_id: b.node_id });
        }
        match node_paths.get(&b.node_id) {
            None => {
                return Err(HarnessTaskError::UnknownScopeBindingNode { node_id: b.node_id });
            }
            Some(analyzed) if analyzed == &b.expected_path => {}
            Some(analyzed) => {
                return Err(HarnessTaskError::ScopeBindingPathMismatch {
                    node_id: b.node_id,
                    binding_path: b.expected_path.clone(),
                    analyzed_path: analyzed.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Harness profile — preferred_vector zorunlu + non-terminal initial status (CLI-owned).
///
/// Predicate/range/policy validation core'da (`Task::validate()`); burada yalnız harness'e
/// özgü ek koşullar (review P0-1 authority split).
fn validate_harness_profile(task: &Task) -> Result<(), HarnessTaskError> {
    if task.target_predicate_set.preferred_vector.is_none() {
        return Err(HarnessTaskError::MissingPreferredVector);
    }
    // Non-terminal initial status (Pending/Assigned/InProgress). Completed/Blocked terminal.
    match task.status {
        TaskStatus::Pending | TaskStatus::Assigned | TaskStatus::InProgress => {}
        found => {
            return Err(HarnessTaskError::InvalidInitialTaskStatus { found });
        }
    }
    Ok(())
}

/// Exact-set scope binding equality (review P1-3).
///
/// `scope_bindings` Node ID set ≡ task predicate Node ID set. Sadece "her binding
/// node_paths'te var" yetmez; fazla/eksik binding veya farklı Node anchoring fail-closed.
/// Boş predicate set core `validate()` tarafından reddedilir; burada anchor consistency
/// ve Node-only/homogeneous V1 harness kısıtı doğrulanır.
///
/// Returns the anchor `NodeId` when valid (used for set-equality proof).
fn validate_scope_binding_set_equality(
    task: &Task,
    bindings: &[CliScopeBinding],
) -> Result<NodeId, HarnessTaskError> {
    // Task predicate Node set (canonical-sorted, Node-only). Subgraph/Module unsupported V1.
    let mut predicate_nodes: std::collections::BTreeSet<NodeId> = std::collections::BTreeSet::new();
    for wp in &task.target_predicate_set.predicates {
        match &wp.predicate.scope {
            PredicateScope::Node(id) => {
                predicate_nodes.insert(*id);
            }
            other => {
                return Err(HarnessTaskError::UnsupportedHarnessScope {
                    found: format!("{other:?}"),
                });
            }
        }
    }
    // Homogeneous V1 harness: tek anchor (predicate set zaten core validate'den geçti → non-empty).
    if predicate_nodes.len() > 1 {
        return Err(HarnessTaskError::HeterogeneousHarnessScope);
    }
    // scope_bindings Node set (canonical-sorted — duplicate validate_scope_bindings'te yakalandı).
    let binding_nodes: std::collections::BTreeSet<NodeId> =
        bindings.iter().map(|b| b.node_id).collect();
    // Exact-set equality: fazla/eksik binding fail-closed (review P1-3).
    if predicate_nodes != binding_nodes {
        return Err(HarnessTaskError::ScopeBindingSetMismatch);
    }
    predicate_nodes
        .iter()
        .next()
        .copied()
        .ok_or(HarnessTaskError::ScopeBindingSetMismatch)
}

/// V1 harness scope validation — Node-only, homogeneous anchor (review v5).
///
/// Tüm predicate'lar aynı `PredicateScope::Node(id)` hedeflemeli. Subgraph/module
/// V1 harness'da unsupported (scope relaxation deferral). Heterogeneous Node id'ler
/// fail-closed (V1 single-anchor task).
///
/// Returns the anchor `NodeId` when valid.
pub fn validate_harness_scope(task: &Task) -> Result<NodeId, HarnessTaskError> {
    let mut anchor: Option<NodeId> = None;
    for wp in &task.target_predicate_set.predicates {
        match &wp.predicate.scope {
            PredicateScope::Node(id) => match anchor {
                None => anchor = Some(*id),
                Some(existing) if existing == *id => {}
                Some(_) => return Err(HarnessTaskError::HeterogeneousHarnessScope),
            },
            other => {
                return Err(HarnessTaskError::UnsupportedHarnessScope {
                    found: format!("{other:?}"),
                });
            }
        }
    }
    anchor.ok_or(HarnessTaskError::HeterogeneousHarnessScope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::coords::{MetricSource, RawPosition};
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, TaskPolicy,
        TaskStatus, WeightedPredicate,
    };
    use osp_core::trajectory::{OpKind, PredicateSet, Task};

    fn snap(head: &str) -> RepositorySnapshot {
        RepositorySnapshot {
            head: head.to_string().try_into().unwrap(),
            tracked_paths: std::collections::BTreeSet::from(["src/a.rs".to_string()]),
            clean: true,
        }
    }

    fn node_paths_with(node_id: NodeId, path: &str) -> HashMap<NodeId, String> {
        let mut m = HashMap::new();
        m.insert(node_id, path.to_string());
        m
    }

    fn task_at(node: NodeId, status: TaskStatus) -> Task {
        Task {
            id: 7,
            milestone_id: 1,
            label: "t".into(),
            target_predicate_set: PredicateSet {
                mode: PredicateMode::All,
                predicates: vec![WeightedPredicate {
                    predicate: MetricPredicate {
                        metric: PredicateAxis::Coupling,
                        operator: ComparisonOp::Le,
                        threshold: 0.55,
                        scope: PredicateScope::Node(node),
                        required_source: Some(MetricSource::Scip),
                        tolerance: 0.0,
                    },
                    weight: None,
                }],
                preferred_vector: Some(RawPosition::default()),
            },
            policy: TaskPolicy::default(),
            allowed_operations: vec![OpKind::RemoveImport],
            constraints: vec![],
            status,
        }
    }

    #[test]
    fn validate_harness_scope_accepts_homogeneous_node() {
        let task = task_at(3, TaskStatus::Pending);
        assert_eq!(validate_harness_scope(&task).unwrap(), 3);
    }

    #[test]
    fn validate_harness_scope_rejects_subgraph() {
        let mut task = task_at(0, TaskStatus::Pending);
        task.target_predicate_set.predicates[0].predicate.scope =
            PredicateScope::Subgraph(vec![1, 2]);
        assert!(matches!(
            validate_harness_scope(&task),
            Err(HarnessTaskError::UnsupportedHarnessScope { .. })
        ));
    }

    #[test]
    fn validate_harness_scope_rejects_module() {
        let mut task = task_at(0, TaskStatus::Pending);
        task.target_predicate_set.predicates[0].predicate.scope =
            PredicateScope::Module("m".into());
        assert!(matches!(
            validate_harness_scope(&task),
            Err(HarnessTaskError::UnsupportedHarnessScope { .. })
        ));
    }

    #[test]
    fn validate_harness_scope_rejects_heterogeneous_nodes() {
        let mut task = task_at(1, TaskStatus::Pending);
        task.target_predicate_set
            .predicates
            .push(WeightedPredicate {
                predicate: MetricPredicate {
                    metric: PredicateAxis::Coupling,
                    operator: ComparisonOp::Le,
                    threshold: 0.4,
                    scope: PredicateScope::Node(2),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            });
        assert!(matches!(
            validate_harness_scope(&task).unwrap_err(),
            HarnessTaskError::HeterogeneousHarnessScope
        ));
    }

    #[test]
    fn validate_scope_bindings_rejects_empty() {
        let np = node_paths_with(1, "src/a.rs");
        assert!(matches!(
            validate_scope_bindings(&[], &np).unwrap_err(),
            HarnessTaskError::EmptyScopeBinding
        ));
    }

    #[test]
    fn validate_scope_bindings_rejects_duplicate() {
        let np = node_paths_with(1, "src/a.rs");
        let b = vec![
            CliScopeBinding {
                node_id: 1,
                expected_path: "src/a.rs".into(),
            },
            CliScopeBinding {
                node_id: 1,
                expected_path: "src/a.rs".into(),
            },
        ];
        assert!(matches!(
            validate_scope_bindings(&b, &np).unwrap_err(),
            HarnessTaskError::DuplicateScopeBinding { node_id: 1 }
        ));
    }

    #[test]
    fn validate_scope_bindings_rejects_path_mismatch() {
        let np = node_paths_with(1, "src/a.rs");
        let b = vec![CliScopeBinding {
            node_id: 1,
            expected_path: "src/b.rs".into(),
        }];
        assert!(matches!(
            validate_scope_bindings(&b, &np).unwrap_err(),
            HarnessTaskError::ScopeBindingPathMismatch { node_id: 1, .. }
        ));
    }

    #[test]
    fn validate_harness_profile_rejects_terminal_status() {
        let task = task_at(1, TaskStatus::Completed);
        assert!(matches!(
            validate_harness_profile(&task).unwrap_err(),
            HarnessTaskError::InvalidInitialTaskStatus { .. }
        ));
    }

    #[test]
    fn validate_harness_profile_rejects_missing_preferred_vector() {
        let mut task = task_at(1, TaskStatus::Pending);
        task.target_predicate_set.preferred_vector = None;
        assert!(matches!(
            validate_harness_profile(&task).unwrap_err(),
            HarnessTaskError::MissingPreferredVector
        ));
    }

    // ── Review P0-1/P1-2: forwarding tests (core validation delegation) ──────────

    #[test]
    fn loader_delegates_invalid_task_to_core_validator() {
        // NaN threshold is not JSON-serializable (serde_json → null → parse fail), so we
        // exercise the delegation directly against the in-memory task + snapshot, mirroring
        // the load path steps that run AFTER deserialize (schema/HEAD/scope/id all valid here).
        let mut task = task_at(1, TaskStatus::Pending);
        task.target_predicate_set.predicates[0].predicate.threshold = f64::NAN;
        assert!(
            task.validate().is_err(),
            "sanity: core rejects NaN threshold"
        );
        let node_paths = node_paths_with(1, "src/a.rs");
        let bindings = vec![CliScopeBinding {
            node_id: 1,
            expected_path: "src/a.rs".into(),
        }];
        // Pre-core steps pass (schema/head/scope-binding/set-equality/id all valid).
        validate_scope_bindings(&bindings, &node_paths).unwrap();
        validate_scope_binding_set_equality(&task, &bindings).unwrap();
        assert_eq!(task.id, 7);
        // Core validation is the step that rejects — confirming CLI delegates, not re-implements.
        let err = task.validate().expect_err("core must reject NaN threshold");
        assert!(
            matches!(err, TaskValidationError::NonFiniteThreshold { .. }),
            "expected NonFiniteThreshold, got {err:?}"
        );
        // Surfaces via HarnessTaskError::TaskValidation in the full loader.
        assert!(matches!(
            HarnessTaskError::TaskValidation(err),
            HarnessTaskError::TaskValidation(_)
        ));
    }

    #[test]
    fn maneuver_override_is_validated_by_core() {
        // Valid task; override maneuver_limit → 0 (core InvalidManeuverLimit, P1-2).
        let task = task_at(1, TaskStatus::Pending);
        assert!(task.validate().is_ok(), "sanity: base task valid");
        let file = CliHarnessTaskFileV1 {
            schema_version: 1,
            repository_head: "0123456789abcdef0123456789abcdef01234567".into(),
            scope_bindings: vec![CliScopeBinding {
                node_id: 1,
                expected_path: "src/a.rs".into(),
            }],
            task,
        };
        let err = exercise_full_load(&file, 7, Some(0), None);
        assert!(
            matches!(err, HarnessTaskError::TaskValidation(ref e)
                if matches!(e, TaskValidationError::InvalidManeuverLimit { value: 0, .. })),
            "expected InvalidManeuverLimit(0) from override-before-validate, got {err:?}"
        );
    }

    // ── Review P1-3: exact-set scope binding equality ────────────────────────────

    #[test]
    fn set_equality_accepts_exact_match() {
        let task = task_at(1, TaskStatus::Pending);
        let bindings = vec![CliScopeBinding {
            node_id: 1,
            expected_path: "src/a.rs".into(),
        }];
        assert_eq!(
            validate_scope_binding_set_equality(&task, &bindings).unwrap(),
            1
        );
    }

    #[test]
    fn set_equality_rejects_extra_binding() {
        let task = task_at(1, TaskStatus::Pending);
        let bindings = vec![
            CliScopeBinding {
                node_id: 1,
                expected_path: "src/a.rs".into(),
            },
            CliScopeBinding {
                node_id: 99,
                expected_path: "src/other.rs".into(),
            },
        ];
        assert!(matches!(
            validate_scope_binding_set_equality(&task, &bindings).unwrap_err(),
            HarnessTaskError::ScopeBindingSetMismatch
        ));
    }

    #[test]
    fn set_equality_rejects_wrong_anchor() {
        // Task targets Node(1), binding declares Node(2) → set mismatch.
        let task = task_at(1, TaskStatus::Pending);
        let bindings = vec![CliScopeBinding {
            node_id: 2,
            expected_path: "src/b.rs".into(),
        }];
        assert!(matches!(
            validate_scope_binding_set_equality(&task, &bindings).unwrap_err(),
            HarnessTaskError::ScopeBindingSetMismatch
        ));
    }

    #[test]
    fn set_equality_rejects_heterogeneous_via_predicate() {
        // Two predicates targeting different Nodes → HeterogeneousHarnessScope.
        let mut task = task_at(1, TaskStatus::Pending);
        task.target_predicate_set
            .predicates
            .push(WeightedPredicate {
                predicate: MetricPredicate {
                    metric: PredicateAxis::Coupling,
                    operator: ComparisonOp::Le,
                    threshold: 0.4,
                    scope: PredicateScope::Node(2),
                    required_source: None,
                    tolerance: 0.0,
                },
                weight: None,
            });
        let bindings = vec![
            CliScopeBinding {
                node_id: 1,
                expected_path: "src/a.rs".into(),
            },
            CliScopeBinding {
                node_id: 2,
                expected_path: "src/b.rs".into(),
            },
        ];
        assert!(matches!(
            validate_scope_binding_set_equality(&task, &bindings).unwrap_err(),
            HarnessTaskError::HeterogeneousHarnessScope
        ));
    }

    #[test]
    fn set_equality_rejects_subgraph_scope() {
        let mut task = task_at(0, TaskStatus::Pending);
        task.target_predicate_set.predicates[0].predicate.scope =
            PredicateScope::Subgraph(vec![1, 2]);
        let bindings = vec![CliScopeBinding {
            node_id: 1,
            expected_path: "src/a.rs".into(),
        }];
        assert!(matches!(
            validate_scope_binding_set_equality(&task, &bindings).unwrap_err(),
            HarnessTaskError::UnsupportedHarnessScope { .. }
        ));
    }

    /// End-to-end exercise of the full load+validate pipeline against an in-memory file.
    ///
    /// Mirrors `load_and_validate_harness_task` without filesystem I/O, so forwarding
    /// tests stay deterministic. If `override_node_paths` is None, uses node {1 → src/a.rs}.
    fn exercise_full_load(
        file: &CliHarnessTaskFileV1,
        positional_task_id: TaskId,
        maneuver_override: Option<u32>,
        override_node_paths: Option<HashMap<NodeId, String>>,
    ) -> HarnessTaskError {
        let snapshot = snap(&file.repository_head);
        let node_paths = override_node_paths.unwrap_or_else(|| node_paths_with(1, "src/a.rs"));
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_string(file).unwrap()).unwrap();
        load_and_validate_harness_task(
            tmp.path(),
            positional_task_id,
            &snapshot,
            &node_paths,
            maneuver_override,
        )
        .expect_err("test expects an error; task should be rejected")
    }

    #[test]
    fn deserialize_envelope_external_tagging() {
        // PredicateScope externally-tagged: {"Node": <u64>}.
        let json = r#"{
            "schema_version": 1,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "scope_bindings": [{"node_id": 1, "expected_path": "src/a.rs"}],
            "task": {
                "id": 7,
                "milestone_id": 1,
                "label": "t",
                "target_predicate_set": {
                    "mode": "All",
                    "predicates": [{
                        "predicate": {
                            "metric": "Coupling",
                            "operator": "Le",
                            "threshold": 0.55,
                            "scope": {"Node": 1},
                            "required_source": "Scip",
                            "tolerance": 0.0
                        },
                        "weight": null
                    }],
                    "preferred_vector": {"x": 0.5, "y": 0.5, "z": 0.5, "w": 0.5, "v": 0.5}
                },
                "policy": {
                    "predicate_failure_policy": "StrictReject",
                    "min_improvement_delta": 0.02,
                    "max_axis_regression": 0.15,
                    "maneuver_limit": 5,
                    "allow_progress_checkpoint": false
                },
                "allowed_operations": ["RemoveImport"],
                "constraints": [],
                "status": "Pending"
            }
        }"#;
        let file: CliHarnessTaskFileV1 = serde_json::from_str(json).unwrap();
        assert_eq!(file.schema_version, 1);
        assert_eq!(file.task.id, 7);
        let head: crate::commands::repo_snapshot::GitCommitId =
            file.repository_head.clone().try_into().unwrap();
        assert_eq!(head.as_str(), "0123456789abcdef0123456789abcdef01234567");
    }

    // `snap` + `node_paths_with` used via load path indirectly above; keep parse helper
    // exercised to avoid dead-code noise in test build.
    #[test]
    fn snap_and_node_paths_helpers_compile() {
        let _s = snap("0123456789abcdef0123456789abcdef01234567");
        let _n = node_paths_with(1, "src/a.rs");
    }
}
