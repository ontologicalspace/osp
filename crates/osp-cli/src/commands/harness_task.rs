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

/// Path-keyed harness task file envelope (V2 — #161/B5).
///
/// V1 id-keyed binding'in path-keyed karşılığı: `node_id` YOK, sadece `path`.
/// Re-bind (`rebind_task_v2`) attempt anındaki taze baseline `node_paths`'ten
/// path→id çözümleyip V1-equivalent iç temsil üretir; mevcut fence zinciri
/// (HEAD exact, set-equality, Node-only homogeneous) birebir işler. Core
/// (`PredicateScope`, `Task`) değişmez — id, yalnızca bir baseline'ın iç
/// koordinatıdır; path dosyanın kalıcı kimliğidir.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliHarnessTaskFileV2 {
    pub schema_version: u32,
    pub repository_head: String,
    pub scope_bindings: Vec<CliScopeBindingV2>,
    /// Ham task JSON — re-bind sırasında predicate scope `{"Path": p}` →
    /// `{"Node": id}` rewrite edilir, sonra core `Task`'a deserialize edilir.
    pub task: serde_json::Value,
}

/// Path-keyed scope binding (V2 task file declaration).
///
/// `deny_unknown_fields`: yanlışlıkla `node_id` yazan kullanıcı sessiz
/// yutulmak yerine net serde hatası alır (v2 = path-keyed).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliScopeBindingV2 {
    pub path: String,
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
    #[error("task file is missing a numeric schema_version")]
    MissingSchemaVersion,
    #[error("scope binding path {path} not present in analysis node_paths")]
    UnknownScopeBindingPath { path: String },
    #[error(
        "v2 task predicate scope must be Path-keyed ({{\"Path\": \"...\"}}), found {found} — \
         Node-id scopes are v1; ids are not stable across file-set changes (#161/B5)"
    )]
    V2ScopeNotPathKeyed { found: String },
    #[error(
        "v2 task shape error: cannot walk target_predicate_set.predicates[*].predicate.scope ({detail})"
    )]
    V2TaskShape { detail: String },
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
/// 1. Read + peek `schema_version` → dispatch
///    - **v1 (id-keyed):** deserialize `CliHarnessTaskFileV1` → adım 3'ten devam
///    - **v2 (path-keyed, #161/B5):** deserialize `CliHarnessTaskFileV2` → HEAD fence
///      (re-bind'ten ÖNCE, guard sırası korunur) → `rebind_task_v2`: path→id çözümleme
///      + predicate scope rewrite → V1-equivalent iç temsil → adım 3'ten devam
/// 2. (v1) `schema_version == 1` · (v2) peek sırasında == 2
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
    // 1. Read + peek schema_version → dispatch (v1 id-keyed | v2 path-keyed).
    let raw = std::fs::read_to_string(path).map_err(|source| HarnessTaskError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let file = match peek_schema_version(&raw)? {
        1 => serde_json::from_str::<CliHarnessTaskFileV1>(&raw)?,
        2 => {
            let v2: CliHarnessTaskFileV2 = serde_json::from_str(&raw)?;
            // 3a. HEAD fence v2 re-bind'inden ÖNCE — path çözümlemesi yanlış bir
            //     baseline'a karşı yapılmış olmasın (guard sırası: schema → HEAD → resolve).
            if v2.repository_head != snapshot.head.as_str() {
                return Err(HarnessTaskError::RepositoryHeadMismatch {
                    task_head: v2.repository_head,
                    repo_head: snapshot.head.as_str().to_string(),
                });
            }
            rebind_task_v2(v2, node_paths)?
        }
        found => {
            return Err(HarnessTaskError::UnsupportedSchemaVersion { found, expected: 2 });
        }
    };
    validate_and_finalize_harness_task(
        file,
        positional_task_id,
        snapshot,
        node_paths,
        maneuver_override,
    )
}

/// V1-equivalent dosya üzerinde fence zinciri (adım 2-10; v2 → rebind sonrası buraya gelir).
fn validate_and_finalize_harness_task(
    file: CliHarnessTaskFileV1,
    positional_task_id: TaskId,
    snapshot: &RepositorySnapshot,
    node_paths: &HashMap<NodeId, String>,
    maneuver_override: Option<u32>,
) -> Result<Task, HarnessTaskError> {
    // 2. schema_version (v2 re-bind'i zaten 1 sabitlemiştir — defensive yeniden doğrulama).
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

/// Ham task JSON'undan `schema_version` peek (dispatch anahtarı).
fn peek_schema_version(raw: &str) -> Result<u32, HarnessTaskError> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    match value.get("schema_version") {
        Some(v) if v.is_u64() => Ok(v.as_u64().expect("checked is_u64") as u32),
        Some(_) | None => Err(HarnessTaskError::MissingSchemaVersion),
    }
}

/// V2 path-keyed task dosyasını V1-equivalent iç temsile re-bind et (#161/B5).
///
/// 1. `node_paths`'ten path→id haritası (bijection defensive)
/// 2. predicate scope rewrite: `{"Path": p}` → `{"Node": id}` (başka varyant → reddi;
///    v2 = Path-only) + rewritten Value → core `Task` deserialize
/// 3. binding path set ≡ predicate path set (erken net mesaj; id düzeyindeki mevcut
///    set-equality fence'i finalize'ta çift güvence olarak yeniden işler)
/// 4. V1-equivalent `{node_id, expected_path}` binding'leri (unknown path fail-closed)
fn rebind_task_v2(
    v2: CliHarnessTaskFileV2,
    node_paths: &HashMap<NodeId, String>,
) -> Result<CliHarnessTaskFileV1, HarnessTaskError> {
    use crate::commands::path_bindings::{build_path_to_id, resolve_path};

    let path_to_id = build_path_to_id(node_paths).map_err(path_binding_error)?;

    let mut task_value = v2.task;
    let predicates = task_value
        .get_mut("target_predicate_set")
        .and_then(|tps| tps.get_mut("predicates"))
        .and_then(|ps| ps.as_array_mut())
        .ok_or_else(|| HarnessTaskError::V2TaskShape {
            detail: "target_predicate_set.predicates missing or not an array".into(),
        })?;
    let mut predicate_paths = std::collections::BTreeSet::new();
    for (i, predicate) in predicates.iter_mut().enumerate() {
        let scope = predicate
            .get_mut("predicate")
            .and_then(|pr| pr.get_mut("scope"))
            .ok_or_else(|| HarnessTaskError::V2TaskShape {
                detail: format!("predicates[{i}].predicate.scope missing"),
            })?;
        let path = take_path_scope(scope)?;
        let id = resolve_path(&path_to_id, &path, " (task predicate scope)")
            .map_err(path_binding_error)?;
        predicate_paths.insert(path);
        *scope = serde_json::json!({ "Node": id });
    }

    let binding_paths: std::collections::BTreeSet<String> =
        v2.scope_bindings.iter().map(|b| b.path.clone()).collect();
    if binding_paths != predicate_paths {
        return Err(HarnessTaskError::ScopeBindingSetMismatch);
    }

    let task: Task = serde_json::from_value(task_value)?;

    let scope_bindings = v2
        .scope_bindings
        .into_iter()
        .map(|b| {
            let node_id = resolve_path(&path_to_id, &b.path, " (scope_bindings)")
                .map_err(path_binding_error)?;
            Ok(CliScopeBinding {
                node_id,
                expected_path: b.path,
            })
        })
        .collect::<Result<Vec<_>, HarnessTaskError>>()?;

    Ok(CliHarnessTaskFileV1 {
        schema_version: 1,
        repository_head: v2.repository_head,
        scope_bindings,
        task,
    })
}

/// Scope object'ten `{"Path": "..."}` değerini al (rewrite öncesi guard).
fn take_path_scope(scope: &mut serde_json::Value) -> Result<String, HarnessTaskError> {
    if !scope.is_object() {
        return Err(HarnessTaskError::V2ScopeNotPathKeyed {
            found: scope.to_string(),
        });
    }
    let obj = scope.as_object_mut().expect("is_object checked above");
    if obj.len() != 1 || !obj.contains_key("Path") {
        let found = obj
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "<empty scope object>".into());
        return Err(HarnessTaskError::V2ScopeNotPathKeyed { found });
    }
    match obj.get("Path").expect("len==1 and key checked above") {
        serde_json::Value::String(s) => Ok(s.clone()),
        other => Err(HarnessTaskError::V2ScopeNotPathKeyed {
            found: format!("Path={other}"),
        }),
    }
}

/// `PathBindingError` → `HarnessTaskError` (task tarafı eşlemesi).
fn path_binding_error(err: crate::commands::path_bindings::PathBindingError) -> HarnessTaskError {
    use crate::commands::path_bindings::PathBindingError;
    match err {
        PathBindingError::UnknownPath { path, .. } => {
            HarnessTaskError::UnknownScopeBindingPath { path }
        }
        PathBindingError::NotPathInjective { path } => HarnessTaskError::V2TaskShape {
            detail: format!("analysis node_paths not path-injective at {path}"),
        },
    }
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
    use osp_core::coords::RawPosition;
    use osp_core::trajectory::{
        ComparisonOp, MetricPredicate, PredicateAxis, PredicateMode, PredicateScope, TaskPolicy,
        TaskStatus, WeightedPredicate,
    };
    use osp_core::trajectory::{OpKind, PredicateSet, Task};

    fn snap(head: &str) -> RepositorySnapshot {
        RepositorySnapshot {
            head: head.to_string().try_into().unwrap(),
            tracked_paths: std::collections::BTreeSet::from(["src/a.rs".to_string()]),
            dirty_paths: std::collections::BTreeSet::new(),
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
                        required_source: None, // #96: native-dürüst (Scip-gereklilik legacy projeksiyon kalıntısı)
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
                            "required_source": null,
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

    // ── #161 (B5): path-keyed V2 task dosyası ──────────────────────────────────

    const V2_HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

    fn v2_envelope(
        scope_bindings: serde_json::Value,
        predicate_scope: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 2,
            "repository_head": V2_HEAD,
            "scope_bindings": scope_bindings,
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
                            "scope": predicate_scope,
                            "required_source": null,
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
        })
    }

    fn load_v2(
        env: &serde_json::Value,
        node_paths: &HashMap<NodeId, String>,
    ) -> Result<Task, HarnessTaskError> {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_string(env).unwrap()).unwrap();
        // Snapshot = gerçek repo HEAD (V2_HEAD); env içindeki repository_head testin
        // manipüle edeceği tarafa (görev yazarının beyanı) aittir.
        let snapshot = snap(V2_HEAD);
        load_and_validate_harness_task(tmp.path(), 7, &snapshot, node_paths, None)
    }

    #[test]
    fn v2_happy_path_resolves_paths_to_current_baseline_ids() {
        let env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        let task = load_v2(&env, &node_paths_with(1, "src/a.rs")).expect("v2 loads");
        assert_eq!(task.id, 7);
        assert_eq!(
            task.target_predicate_set.predicates[0].predicate.scope,
            PredicateScope::Node(1),
            "Path → current baseline id re-bind"
        );
    }

    #[test]
    fn v2_rebinds_path_to_current_analysis_node_id() {
        // R1 P2-1: garanti = "aynı exact repository snapshot içinde id'leri elle
        // bilmek gerekmez; path, güncel analysis NodeId'sine bind edilir" (burada
        // src/z.rs aynı snapshot içinde keşfedilmiş → a.rs'nin id'si 1'den 5'e kaymış).
        // Cross-commit task portability DEĞİL: dosya seti commit'ler arası değişirse
        // Git HEAD de değişir ve HEAD fence re-bind'ten ÖNCE reddeder.
        let env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        let mut np = HashMap::new();
        np.insert(5u64, "src/a.rs".to_string());
        np.insert(6u64, "src/z.rs".to_string());
        let task = load_v2(&env, &np).expect("path binds to current analysis id");
        assert_eq!(
            task.target_predicate_set.predicates[0].predicate.scope,
            PredicateScope::Node(5)
        );
    }

    #[test]
    fn v2_unknown_binding_path_fails_closed() {
        let env = v2_envelope(
            serde_json::json!([{"path": "src/gone.rs"}]),
            serde_json::json!({"Path": "src/gone.rs"}),
        );
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(
            err,
            HarnessTaskError::UnknownScopeBindingPath { ref path } if path == "src/gone.rs"
        ));
    }

    #[test]
    fn v2_unknown_predicate_scope_path_fails_closed() {
        let env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/other.rs"}),
        );
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(
            err,
            HarnessTaskError::UnknownScopeBindingPath { ref path } if path == "src/other.rs"
        ));
    }

    #[test]
    fn v2_node_scope_rejected_v2_is_path_only() {
        let env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Node": 1}),
        );
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(err, HarnessTaskError::V2ScopeNotPathKeyed { .. }));
    }

    #[test]
    fn v2_path_set_mismatch_rejected() {
        let mut np = HashMap::new();
        np.insert(1u64, "src/a.rs".to_string());
        np.insert(2u64, "src/b.rs".to_string());
        let env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/b.rs"}),
        );
        let err = load_v2(&env, &np).unwrap_err();
        assert!(matches!(err, HarnessTaskError::ScopeBindingSetMismatch));
    }

    #[test]
    fn v2_head_mismatch_rejected_before_rebind() {
        let mut env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        env["repository_head"] = serde_json::json!("f".repeat(40));
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        // Guard sırası: schema → HEAD → resolve. HEAD fence re-bind'ten önce patlar;
        // path'ler doğru olsa bile yanlış baseline'a karşı çözülmez.
        assert!(matches!(
            err,
            HarnessTaskError::RepositoryHeadMismatch { .. }
        ));
    }

    #[test]
    fn v2_binding_with_node_id_field_rejected() {
        // deny_unknown_fields: yanlışlıkla id-keyed yazan kullanıcı sessizce
        // yutulmak yerine net serde hatası alır.
        let env = v2_envelope(
            serde_json::json!([{"node_id": 1, "path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(err, HarnessTaskError::Parse(_)));
    }

    #[test]
    fn v2_duplicate_binding_paths_reach_v1_duplicate_fence() {
        // İki binding aynı path → path set eşitliği BTreeSet dedupe'iyle geçer, ama
        // re-bind aynı node_id'yi iki kez üretir → finalize'taki mevcut
        // DuplicateScopeBinding fence'i yakalar (v2 kendi fence'ini icat etmez).
        let env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}, {"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(
            err,
            HarnessTaskError::DuplicateScopeBinding { node_id: 1 }
        ));
    }

    #[test]
    fn missing_schema_version_rejected() {
        let mut env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        env.as_object_mut().unwrap().remove("schema_version");
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(err, HarnessTaskError::MissingSchemaVersion));
    }

    #[test]
    fn unknown_schema_version_rejected() {
        let mut env = v2_envelope(
            serde_json::json!([{"path": "src/a.rs"}]),
            serde_json::json!({"Path": "src/a.rs"}),
        );
        env["schema_version"] = serde_json::json!(3);
        let err = load_v2(&env, &node_paths_with(1, "src/a.rs")).unwrap_err();
        assert!(matches!(
            err,
            HarnessTaskError::UnsupportedSchemaVersion { found: 3, .. }
        ));
    }
}
