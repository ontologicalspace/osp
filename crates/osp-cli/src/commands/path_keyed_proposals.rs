//! #161 (B5): path-keyed proposals dosyası — v2/v3 zarf + path→id re-bind.
//!
//! v1 format (çıplak JSON array, `Vec<DeltaProposal>`) dokunulmadan yaşamaya
//! devam eder. v2, NodeId alanlarını repo-relative path string olarak taşır;
//! çözümleme yalnızca attempt anındaki baseline `node_paths` üzerinden yapılır.
//!
//! **#199 (v3): yeni-düğüm adreslenebilirliği.** v2'de path'ler yalnız MEVCUT
//! baseline node'larına çözümlenir; yeni node bağlantıları
//! `new_nodes[].connected_to` üzerinden mevcut node'lara kurulur — mevcut→yeni
//! (ve yeni→yeni) kenar TEMSİL EDİLEMEZDİ (run-18: hipotetik import derecesi
//! eksik sayıldı, eşik ayrımı gate re-analysis'e kaydı). v3'te `new_nodes[]`
//! girdileri opsiyonel `path` taşıyabilir ve `new_edges` uçları bu beyan
//! edilmiş path'lere çözümlenir (sentezik ID sözleşmesi:
//! `osp_core::task_measurement::synthetic_new_node_id`). `connected_to`
//! geri-uyumlu kısaltma olarak baseline-only kalır; yeni-düğüm kenarları
//! `new_edges` üzerinden ifade edilir.

#![allow(dead_code, reason = "#161: wired incrementally across commits")]

use std::collections::HashMap;
use std::path::Path;

use osp_core::agent::{
    DeltaProposal, EdgeRef, EntityChangeSpec, NewEdgeSpec, NewNodeSpec, PositionHint,
};
use osp_core::coords::RawPosition;
use osp_core::space::{EdgeKind, NodeId};

use crate::commands::path_bindings::{build_path_to_id, resolve_path, PathBindingError};

/// Path-keyed proposals zarfı (v2).
///
/// `repository_head`: proposal'ın ÜRETİLDİĞİ repo HEAD'i (full 40-char SHA);
/// load sırasında capture edilen snapshot ile exact match doğrulanır —
/// re-bind'ten ÖNCE (R1 P1-1: proposal intent, üretim state'ine bağlıdır;
/// aynı path çifti farklı revision'da farklı structural fact olabilir).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedProposalsFileV2 {
    pub schema_version: u32,
    pub repository_head: String,
    pub proposals: Vec<CliPathKeyedProposal>,
}

/// `DeltaProposal`'un path-keyed DTO karşılığı — alan adları birebir, NodeId → path.
///
/// `deny_unknown_fields` (R1 P1-2): v2 yeni bir wire surface — typo'lanmış
/// structural alan (`removed_edge` gibi) sessizce yutulmak yerine parse error
/// üretir; hata Q4'ten ÖNCE kaybolamaz. v1 `DeltaProposal` serde gevşekliği
/// backward-compat için korunur (çıplak array yolu).
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CliPathKeyedProposal {
    pub new_nodes: Vec<CliPathKeyedNewNodeSpec>,
    pub new_edges: Vec<CliPathKeyedEdgeSpec>,
    pub removed_edges: Vec<CliPathKeyedEdgeRef>,
    pub affected_nodes: Vec<String>,
    pub modified_entities: Vec<CliPathKeyedEntityChange>,
    pub position_hints: Vec<CliPathKeyedPositionHint>,
    pub reasoning: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedNewNodeSpec {
    pub kind: osp_core::space::NodeKind,
    pub initial_mass: f64,
    pub connected_to: Vec<(String, EdgeKind)>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedEdgeSpec {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedEdgeRef {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedEntityChange {
    pub path: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedPositionHint {
    pub path: String,
    pub suggested_raw: RawPosition,
    pub rationale: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// #199: v3 zarfı — yeni-düğüm adreslenebilirliği (path'li new_nodes +
// new_edges uçlarının beyan edilen yeni düğümlere açılması).
// ─────────────────────────────────────────────────────────────────────────────

/// Path-keyed proposals zarfı (v3).
///
/// v2'den farkı: `new_nodes[].path` (opsiyonel — path'siz yeni düğümler v2
/// semantiğinde kalır, adreslenemez) ve `new_edges` uçlarının bu path'lere
/// çözümlenebilmesi. HEAD fence v2 ile aynı.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedProposalsFileV3 {
    pub schema_version: u32,
    pub repository_head: String,
    pub proposals: Vec<CliPathKeyedProposalV3>,
}

/// v3 proposal gövdesi — v2 ile aynı alanlar; yalnızca new_nodes eleman tipi
/// farklı (path taşır). Kenar/ref DTO'ları (EdgeSpec/EdgeRef/EntityChange/
/// PositionHint) v2 ile PAYLAŞILIR — şekil aynı, çözümleme semantiği farklı.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CliPathKeyedProposalV3 {
    pub new_nodes: Vec<CliPathKeyedNewNodeSpecV3>,
    pub new_edges: Vec<CliPathKeyedEdgeSpec>,
    pub removed_edges: Vec<CliPathKeyedEdgeRef>,
    pub affected_nodes: Vec<String>,
    pub modified_entities: Vec<CliPathKeyedEntityChange>,
    pub position_hints: Vec<CliPathKeyedPositionHint>,
    pub reasoning: String,
}

/// v3 yeni-düğüm spec'i — opsiyonel `path` ile proposal-yerel adres edinir.
/// Path'li yeni düğümler `new_edges` uçlarında referanslanabilir; path'siz
/// olanlar v2 davranışındadır (yalnız `connected_to` ile mevcut düğümlere).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliPathKeyedNewNodeSpecV3 {
    pub kind: osp_core::space::NodeKind,
    pub initial_mass: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Yeni→mevcut çıkış-kenarları (geri-uyumlu kısaltma). v3'te opsiyonel:
    /// bağlantısız yeni düğüm meşru (bağlantılar new_edges üzerinden de
    /// kurulabilir).
    #[serde(default)]
    pub connected_to: Vec<(String, EdgeKind)>,
}

/// Path-keyed proposals yükleme hatası.
#[derive(Debug, thiserror::Error)]
pub enum PathKeyedProposalError {
    #[error("failed to read proposals file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse proposals file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error(
        "proposals file object envelope requires schema_version 2 or 3 (found {found}) — \
         plain arrays are v1 (node-id keyed)"
    )]
    UnsupportedEnvelopeVersion { found: String },
    #[error("proposals file object envelope is missing schema_version")]
    MissingEnvelopeVersion,
    #[error(
        "repository HEAD mismatch: proposals file {proposal_head}, repo {repo_head} — \
         v2/v3 proposals re-bind only against the state they were produced on (R1 P1-1)"
    )]
    ProposalRepositoryHeadMismatch {
        proposal_head: String,
        repo_head: String,
    },
    #[error(
        "proposals file must be a JSON array (v1) or an object envelope (v2/v3), found {found}"
    )]
    UnexpectedTopLevelShape { found: String },
    #[error(
        "path-keyed endpoints resolve against measured baseline nodes \
         (v3: or new_nodes[].path declared in the same proposal; v2: connected_to only) — {0}"
    )]
    PathBinding(#[from] PathBindingError),
    #[error(
        "duplicate new_nodes path {path} — each declared new node must carry a unique path (#199)"
    )]
    DuplicateNewNodePath { path: String },
    #[error(
        "new_nodes path {path} is already a measured baseline node — a new node \
         must not shadow an existing path; reference it directly instead (#199)"
    )]
    NewNodePathShadowsBaseline { path: String },
    #[error(
        "new_nodes.connected_to targets new-node path {path} — connected_to resolves \
         baseline nodes only (back-compat shorthand); express new-node edges via \
         new_edges in the v3 envelope (#199)"
    )]
    ConnectedToNewNode { path: String },
    #[error(
        "new_nodes path {path:?} is not a canonical repo-relative path — the measured \
         space's identity axis requires a non-empty, forward-slash, repo-relative path \
         without `./`, `../`, empty or absolute segments (#199 review P1-2: an alias \
         such as `./main.rs` would count as a SECOND node in the hypothetical graph)"
    )]
    InvalidNewNodePath { path: String },
}

/// Proposals dosyası yükle + re-bind → `Vec<DeltaProposal>`.
///
/// Dispatch: JSON array → v1 (`Vec<DeltaProposal>`, dokunulmaz) · JSON object
/// with `schema_version: 2` → path-keyed v2. Guard sırası: schema → **HEAD
/// fence** (proposal'ın üretim state'i == attempt anındaki snapshot; re-bind'ten
/// ÖNCE) → path→id çözümleme (R1 P1-1).
pub fn load_proposals_file(
    path: &Path,
    snapshot_head: &str,
    node_paths: &HashMap<NodeId, String>,
) -> Result<Vec<DeltaProposal>, PathKeyedProposalError> {
    let raw = std::fs::read_to_string(path).map_err(|source| PathKeyedProposalError::Read {
        path: path.display().to_string(),
        source,
    })?;
    load_proposals_str(&raw, snapshot_head, node_paths)
}

/// #178 review P0-1 (read-once): bayt-temelli çekirdek — çağıran dosyayı TEK
/// okumayla alır; digest ve parse AYNI tampondan (navigator sırasında diskteki
/// dosya değişse bile zarf iddiası doğru kalır).
pub fn load_proposals_str(
    raw: &str,
    snapshot_head: &str,
    node_paths: &HashMap<NodeId, String>,
) -> Result<Vec<DeltaProposal>, PathKeyedProposalError> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    match &value {
        serde_json::Value::Array(_) => Ok(serde_json::from_value(value)?),
        serde_json::Value::Object(_) => {
            // #199: zarf sürümü ÖNCE okunur, sonra tip-li parse — v2/v3 gövde
            // tipleri farklı (new_nodes path alanı). Guard sırası v2 sözleşmesiyle
            // AYNEN: parse → **HEAD fence** → build_path_to_id → re-bind
            // (R1 P1-1: stale artifact'ın yanlış HEAD'i, path'leri bu snapshot'ta
            // çözümlenemese de DAHA ÖNCE reddedilir — review P1-1: fence asla
            // re-bind'in arkasına geçmez).
            let version = value.get("schema_version").and_then(|v| v.as_u64());
            match version {
                Some(2) => {
                    let file: CliPathKeyedProposalsFileV2 = serde_json::from_value(value)?;
                    ensure_head_fence(&file.repository_head, snapshot_head)?;
                    let path_to_id = build_path_to_id(node_paths)?;
                    file.proposals
                        .into_iter()
                        .map(|p| rebind_proposal(p, &path_to_id))
                        .collect()
                }
                Some(3) => {
                    let file: CliPathKeyedProposalsFileV3 = serde_json::from_value(value)?;
                    ensure_head_fence(&file.repository_head, snapshot_head)?;
                    let path_to_id = build_path_to_id(node_paths)?;
                    file.proposals
                        .into_iter()
                        .map(|p| rebind_proposal_v3(p, &path_to_id))
                        .collect()
                }
                Some(v) => Err(PathKeyedProposalError::UnsupportedEnvelopeVersion {
                    found: v.to_string(),
                }),
                None => Err(PathKeyedProposalError::MissingEnvelopeVersion),
            }
        }
        other => Err(PathKeyedProposalError::UnexpectedTopLevelShape {
            found: other.to_string(),
        }),
    }
}

/// HEAD fence — re-bind'ten ÖNCE koşar (v2 sözleşmesi, #199 review P1-1):
/// path identity doğru olsa bile proposal intent, üretildiği state'e bağlıdır
/// (#160 state-identity ilkesinin proposal artefaktındaki karşılığı). Stale
/// artifact'ın yanlış HEAD'i, çözümlenemeyen path'lerden DAHA SPESİFİK hata
/// olarak döner.
fn ensure_head_fence(
    proposal_head: &str,
    snapshot_head: &str,
) -> Result<(), PathKeyedProposalError> {
    if proposal_head != snapshot_head {
        return Err(PathKeyedProposalError::ProposalRepositoryHeadMismatch {
            proposal_head: proposal_head.to_string(),
            repo_head: snapshot_head.to_string(),
        });
    }
    Ok(())
}

/// Tek path-keyed proposal → id-keyed `DeltaProposal`.
fn rebind_proposal(
    p: CliPathKeyedProposal,
    path_to_id: &HashMap<String, NodeId>,
) -> Result<DeltaProposal, PathKeyedProposalError> {
    let new_nodes = p
        .new_nodes
        .into_iter()
        .map(|n| {
            let connected_to = n
                .connected_to
                .into_iter()
                .map(|(path, kind)| {
                    Ok((
                        resolve_path(path_to_id, &path, " (new_nodes.connected_to)")?,
                        kind,
                    ))
                })
                .collect::<Result<Vec<_>, PathBindingError>>()?;
            Ok(NewNodeSpec {
                kind: n.kind,
                initial_mass: n.initial_mass,
                connected_to,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let new_edges = p
        .new_edges
        .into_iter()
        .map(|e| {
            Ok(NewEdgeSpec {
                from: resolve_path(path_to_id, &e.from, " (new_edges.from)")?,
                to: resolve_path(path_to_id, &e.to, " (new_edges.to)")?,
                kind: e.kind,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let removed_edges = p
        .removed_edges
        .into_iter()
        .map(|e| {
            Ok(EdgeRef {
                from: resolve_path(path_to_id, &e.from, " (removed_edges.from)")?,
                to: resolve_path(path_to_id, &e.to, " (removed_edges.to)")?,
                kind: e.kind,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let affected_nodes = p
        .affected_nodes
        .iter()
        .map(|path| resolve_path(path_to_id, path, " (affected_nodes)"))
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let modified_entities = p
        .modified_entities
        .into_iter()
        .map(|m| {
            Ok(EntityChangeSpec {
                node_id: resolve_path(path_to_id, &m.path, " (modified_entities.path)")?,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let position_hints = p
        .position_hints
        .into_iter()
        .map(|h| {
            Ok(PositionHint {
                node_id: resolve_path(path_to_id, &h.path, " (position_hints.path)")?,
                suggested_raw: h.suggested_raw,
                rationale: h.rationale,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    Ok(DeltaProposal {
        new_nodes,
        new_edges,
        removed_edges,
        affected_nodes,
        modified_entities,
        position_hints,
        reasoning: p.reasoning,
    })
}

/// #199: v3 endpoint çözümlemesi — baseline ölçülmüş düğümü VEYA aynı
/// proposal'da path beyan edilen yeni düğüm. Yeni düğüm → sentezik delta-node
/// ID'si (`synthetic_new_node_id`); core Q4 kenar-uç kuralı delta üyeliğini
/// kabul eder, hipotetik uzaya düğüm önce girer kenar sonra (insert sırası)
/// → mevcut→yeni ve yeni→yeni kenarlar coupling ölçümünde sayılır.
fn resolve_endpoint_v3(
    path_to_id: &HashMap<String, NodeId>,
    new_paths: &HashMap<String, usize>,
    path: &str,
    context: &str,
) -> Result<NodeId, PathBindingError> {
    if let Some(id) = path_to_id.get(path) {
        return Ok(*id);
    }
    if let Some(index) = new_paths.get(path) {
        return Ok(osp_core::task_measurement::synthetic_new_node_id(*index));
    }
    Err(PathBindingError::UnknownPath {
        path: path.to_string(),
        context: context.to_string(),
    })
}

/// Tek v3 path-keyed proposal → id-keyed `DeltaProposal`.
///
/// v2'den farkları (#199):
/// - `new_nodes[].path` beyanları proposal-yerel adres alanı kurar (dup +
///   baseline-gölgeleme reddi fail-closed);
/// - `new_edges` uçları bu adres alanına da çözümlenir (sentezik ID);
/// - `connected_to` baseline-only kalır (geri-uyumlu kısaltma — yeni-düğüm
///   hedefi `ConnectedToNewNode` reddiyle `new_edges`'e yönlendirilir);
/// - `removed_edges`/`affected_nodes`/`modified_entities`/`position_hints`
///   baseline-only (kaldırma ve ölçüm-scope danışma yalnız ölçülmüş gerçekler).
fn rebind_proposal_v3(
    p: CliPathKeyedProposalV3,
    path_to_id: &HashMap<String, NodeId>,
) -> Result<DeltaProposal, PathKeyedProposalError> {
    // 1. Beyan edilen yeni-düğüm path'leri → index (sentezik ID kaynağı).
    //    Sıra: kanoniklik → baseline gölgeleme → duplicate (review P1-2: alias
    //    reddi en spesifik hata olarak önce döner).
    let mut new_paths: HashMap<String, usize> = HashMap::with_capacity(p.new_nodes.len());
    for (index, n) in p.new_nodes.iter().enumerate() {
        if let Some(path) = &n.path {
            if osp_analyzer::language::RepoRelativePath::from_repo_relative_str(path).is_none() {
                return Err(PathKeyedProposalError::InvalidNewNodePath { path: path.clone() });
            }
            if path_to_id.contains_key(path) {
                return Err(PathKeyedProposalError::NewNodePathShadowsBaseline {
                    path: path.clone(),
                });
            }
            if new_paths.insert(path.clone(), index).is_some() {
                return Err(PathKeyedProposalError::DuplicateNewNodePath { path: path.clone() });
            }
        }
    }

    // 2. new_nodes → core NewNodeSpec (connected_to baseline-only).
    let new_nodes = p
        .new_nodes
        .into_iter()
        .map(|n| {
            let connected_to = n
                .connected_to
                .into_iter()
                .map(|(path, kind)| {
                    if new_paths.contains_key(&path) {
                        return Err(PathKeyedProposalError::ConnectedToNewNode { path });
                    }
                    Ok((
                        resolve_path(path_to_id, &path, " (new_nodes.connected_to)")?,
                        kind,
                    ))
                })
                .collect::<Result<Vec<_>, PathKeyedProposalError>>()?;
            Ok(NewNodeSpec {
                kind: n.kind,
                initial_mass: n.initial_mass,
                connected_to,
            })
        })
        .collect::<Result<Vec<_>, PathKeyedProposalError>>()?;

    // 3. new_edges — uçlar baseline ∪ beyan edilen yeni düğümler (#199).
    let new_edges = p
        .new_edges
        .into_iter()
        .map(|e| {
            Ok(NewEdgeSpec {
                from: resolve_endpoint_v3(path_to_id, &new_paths, &e.from, " (new_edges.from)")?,
                to: resolve_endpoint_v3(path_to_id, &new_paths, &e.to, " (new_edges.to)")?,
                kind: e.kind,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;

    // 4. removed_edges / affected_nodes / modified_entities / position_hints —
    //    v2 çözümlemesi aynen (baseline-only).
    let removed_edges = p
        .removed_edges
        .into_iter()
        .map(|e| {
            Ok(EdgeRef {
                from: resolve_path(path_to_id, &e.from, " (removed_edges.from)")?,
                to: resolve_path(path_to_id, &e.to, " (removed_edges.to)")?,
                kind: e.kind,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let affected_nodes = p
        .affected_nodes
        .iter()
        .map(|path| resolve_path(path_to_id, path, " (affected_nodes)"))
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let modified_entities = p
        .modified_entities
        .into_iter()
        .map(|m| {
            Ok(EntityChangeSpec {
                node_id: resolve_path(path_to_id, &m.path, " (modified_entities.path)")?,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    let position_hints = p
        .position_hints
        .into_iter()
        .map(|h| {
            Ok(PositionHint {
                node_id: resolve_path(path_to_id, &h.path, " (position_hints.path)")?,
                suggested_raw: h.suggested_raw,
                rationale: h.rationale,
            })
        })
        .collect::<Result<Vec<_>, PathBindingError>>()?;
    Ok(DeltaProposal {
        new_nodes,
        new_edges,
        removed_edges,
        affected_nodes,
        modified_entities,
        position_hints,
        reasoning: p.reasoning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::space::EdgeKind;

    /// Snapshot HEAD — v2 fence'inin karşılaştırdığı "attempt anındaki repo HEAD".
    const V2_HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

    fn node_paths() -> HashMap<NodeId, String> {
        [
            (0u64, "a.rs".to_string()),
            (1u64, "b.rs".to_string()),
            (2u64, "main.rs".to_string()),
        ]
        .into_iter()
        .collect()
    }

    fn write(name: &str, content: &str) -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.keep().join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn v1_plain_array_loads_unchanged() {
        let raw = r#"[{
            "new_nodes": [],
            "new_edges": [],
            "removed_edges": [{"from": 0, "to": 1, "kind": "Imports"}],
            "affected_nodes": [0],
            "modified_entities": [],
            "position_hints": [],
            "reasoning": "v1 stays"
        }]"#;
        let p = write("proposals.json", raw);
        // v1 array HEAD beyanı taşımaz — fence yalnız v2 envelope'a aittir.
        let out = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].removed_edges[0].from, 0);
        assert_eq!(out[0].removed_edges[0].to, 1);
        assert_eq!(out[0].removed_edges[0].kind, EdgeKind::Imports);
        assert_eq!(out[0].affected_nodes, vec![0]);
    }

    #[test]
    fn v2_paths_resolve_to_ids() {
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [],
                "new_edges": [],
                "removed_edges": [{"from": "main.rs", "to": "b.rs", "kind": "Imports"}],
                "affected_nodes": ["main.rs"],
                "modified_entities": [],
                "position_hints": [],
                "reasoning": "remove import to reduce coupling"
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let out = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].removed_edges[0].from, 2, "main.rs → id 2");
        assert_eq!(out[0].removed_edges[0].to, 1, "b.rs → id 1");
        assert_eq!(out[0].affected_nodes, vec![2]);
    }

    #[test]
    fn v2_new_node_connected_to_resolves() {
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [{
                    "kind": "Module",
                    "initial_mass": 1.0,
                    "connected_to": [["a.rs", "Imports"]]
                }],
                "new_edges": [],
                "removed_edges": [],
                "affected_nodes": [],
                "modified_entities": [],
                "position_hints": [],
                "reasoning": "run-7 AddNode pattern"
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let out = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap();
        assert_eq!(out[0].new_nodes.len(), 1);
        assert_eq!(
            out[0].new_nodes[0].connected_to,
            vec![(0, EdgeKind::Imports)]
        );
    }

    #[test]
    fn v2_proposal_head_mismatch_rejected_before_path_rebind() {
        // R1 P1-1: HEAD=A'da üretilmiş proposal HEAD=B'ye sessizce re-bind EDİLEMEZ —
        // fence re-bind'ten önce; path'ler geçerli olsa bile reddedilir.
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "ffffffffffffffffffffffffffffffffffffffff",
            "proposals": [{
                "removed_edges": [{"from": "main.rs", "to": "b.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        assert!(matches!(
            err,
            PathKeyedProposalError::ProposalRepositoryHeadMismatch { .. }
        ));
    }

    #[test]
    fn v2_typo_structural_field_rejected() {
        // R1 P1-2: "removed_edge" (tekil) sessizce yutulmak yerine parse error —
        // hata Q4 Syntax Gate'ten ÖNCE kaybolamaz.
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "removed_edge": [{"from": "main.rs", "to": "b.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        assert!(matches!(err, PathKeyedProposalError::Parse(_)));
    }

    #[test]
    fn v2_nested_unknown_field_rejected() {
        // R1 P1-2: nested DTO'da id-dünya kalıntısı (node_id) net serde hatası.
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [{
                    "kind": "Module",
                    "initial_mass": 1.0,
                    "node_id": 7,
                    "connected_to": [["a.rs", "Imports"]]
                }]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        assert!(matches!(err, PathKeyedProposalError::Parse(_)));
    }

    #[test]
    fn v2_envelope_unknown_field_rejected() {
        // R1 P1-2: envelope'un kendisi de strict.
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "extra": true,
            "proposals": []
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        assert!(matches!(err, PathKeyedProposalError::Parse(_)));
    }

    #[test]
    fn v2_unknown_path_fails_closed_with_field_context() {
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "removed_edges": [{"from": "stale.rs", "to": "b.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        match err {
            PathKeyedProposalError::PathBinding(PathBindingError::UnknownPath {
                path,
                context,
            }) => {
                assert_eq!(path, "stale.rs");
                assert_eq!(context, " (removed_edges.from)");
            }
            other => panic!("expected UnknownPath, got {other:?}"),
        }
    }

    #[test]
    fn v2_new_node_reference_in_new_edges_rejected_with_guidance() {
        // Yeni dosya baseline'ta yok → UnknownProposalPath; mesaj connected_to'ya yönlendirir.
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_edges": [{"from": "not-yet-a-file.rs", "to": "a.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("connected_to"), "guidance in message: {msg}");
        assert!(msg.contains("not-yet-a-file.rs"), "path in message: {msg}");
    }

    #[test]
    fn object_envelope_without_schema_version_rejected() {
        let p = write("proposals.json", r#"{"proposals": []}"#);
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::MissingEnvelopeVersion
        ));
    }

    #[test]
    fn object_envelope_wrong_version_rejected() {
        // #199: 3 artık geçerli zarf sürümü — bilinmeyen sürüm 4 reddedilir.
        let p = write(
            "proposals.json",
            r#"{"schema_version": 4, "proposals": []}"#,
        );
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::UnsupportedEnvelopeVersion { .. }
        ));
    }

    #[test]
    fn scalar_top_level_rejected() {
        let p = write("proposals.json", r#""nope""#);
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::UnexpectedTopLevelShape { .. }
        ));
    }

    // ── #199: v3 — yeni-düğüm adreslenebilirliği ────────────────────────────────

    #[test]
    fn v3_existing_to_new_edge_resolves_to_synthetic_id() {
        // run-18 deseni: mevcut düğüm, taşınan tipin yeni düğümüne import kazanır.
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 1.0, "path": "newmod.rs"}],
                "new_edges": [{"from": "main.rs", "to": "newmod.rs", "kind": "Imports"}],
                "reasoning": "existing gains import on new node"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        let out = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap();
        assert_eq!(out.len(), 1);
        // main.rs → id 2; newmod.rs → sentezik ID 10_000 (index 0).
        assert_eq!(out[0].new_edges[0].from, 2, "main.rs → id 2");
        assert_eq!(
            out[0].new_edges[0].to,
            osp_core::task_measurement::synthetic_new_node_id(0),
            "newmod.rs → sentezik delta-node ID'si"
        );
        assert_eq!(out[0].new_edges[0].kind, EdgeKind::Imports);
    }

    #[test]
    fn v3_new_to_new_edge_resolves_between_synthetic_ids() {
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [
                    {"kind": "Module", "initial_mass": 1.0, "path": "n1.rs"},
                    {"kind": "Module", "initial_mass": 1.0, "path": "n2.rs"}
                ],
                "new_edges": [{"from": "n2.rs", "to": "n1.rs", "kind": "Imports"}],
                "reasoning": "new file imports another new file"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        let out = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap();
        assert_eq!(out[0].new_edges[0].from, 10_001, "n2.rs → sentezik index 1");
        assert_eq!(out[0].new_edges[0].to, 10_000, "n1.rs → sentezik index 0");
    }

    #[test]
    fn v3_duplicate_new_node_path_rejected() {
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [
                    {"kind": "Module", "initial_mass": 1.0, "path": "n.rs"},
                    {"kind": "Module", "initial_mass": 1.0, "path": "n.rs"}
                ],
                "reasoning": "dup"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::DuplicateNewNodePath { .. }
        ));
    }

    #[test]
    fn v3_new_node_path_shadowing_baseline_rejected() {
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 1.0, "path": "a.rs"}],
                "reasoning": "shadow"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::NewNodePathShadowsBaseline { .. }
        ));
    }

    #[test]
    fn v3_connected_to_new_node_rejected_with_new_edges_guidance() {
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [
                    {"kind": "Module", "initial_mass": 1.0, "path": "n1.rs"},
                    {"kind": "Module", "initial_mass": 1.0, "path": "n2.rs",
                     "connected_to": [["n1.rs", "Imports"]]}
                ],
                "reasoning": "wrong channel"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        assert!(matches!(
            err,
            PathKeyedProposalError::ConnectedToNewNode { .. }
        ));
    }

    #[test]
    fn v3_unknown_endpoint_still_fails_closed() {
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 1.0, "path": "n.rs"}],
                "new_edges": [{"from": "main.rs", "to": "ghost.rs", "kind": "Imports"}],
                "reasoning": "unknown target"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        let err = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err();
        match err {
            PathKeyedProposalError::PathBinding(PathBindingError::UnknownPath { path, .. }) => {
                assert_eq!(path, "ghost.rs");
            }
            other => panic!("expected UnknownPath, got {other:?}"),
        }
    }

    #[test]
    fn v3_pathless_new_node_keeps_v2_semantics() {
        // Path'siz yeni düğüm v3 zarfında da adreslenemez — connected_to ile
        // mevcut düğüme bağlanır (v2 davranışının korunması).
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "0123456789abcdef0123456789abcdef01234567",
            "proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 1.0,
                              "connected_to": [["a.rs", "Imports"]]}],
                "reasoning": "run-7 pattern, unaddressed"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        let out = load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap();
        assert_eq!(
            out[0].new_nodes[0].connected_to,
            vec![(0, EdgeKind::Imports)]
        );
    }

    #[test]
    fn v3_head_fence_applies_before_rebind() {
        // Review P1-1: stale artifact bilerek ÇÖZÜMLENEMEYEN endpoint da taşıyor
        // (gone.rs baseline'da yok) — fence önceliği ancak böyle kanıtlanır;
        // rebind-valid bir proposal'da UnknownPath hiç tetiklenmezdi.
        let raw = r#"{
            "schema_version": 3,
            "repository_head": "ffffffffffffffffffffffffffffffffffffffff",
            "proposals": [{
                "new_nodes": [{"kind": "Module", "initial_mass": 1.0, "path": "n.rs"}],
                "new_edges": [{"from": "gone.rs", "to": "n.rs", "kind": "Imports"}],
                "reasoning": "fence precedence"
            }]
        }"#;
        let p = write("proposals.v3.json", raw);
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::ProposalRepositoryHeadMismatch { .. }
        ));
    }

    #[test]
    fn v2_head_fence_precedence_survives_v3_refactor() {
        // Review P1-1: v1→v3 dispatch refactor'u v2 precedence'ını bozmamalı —
        // yanlış HEAD + stale path → Mismatch (UnknownPath DEĞİL).
        let raw = r#"{
            "schema_version": 2,
            "repository_head": "ffffffffffffffffffffffffffffffffffffffff",
            "proposals": [{
                "removed_edges": [{"from": "stale.rs", "to": "b.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        assert!(matches!(
            load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
            PathKeyedProposalError::ProposalRepositoryHeadMismatch { .. }
        ));
    }

    #[test]
    fn v3_non_canonical_new_node_path_rejected() {
        // Review P1-2: `./main.rs` baseline `main.rs`'in alias'ı — raw-string
        // duplicate/gölgeleme kontrolleri bunu yakalamazdı; hipotetik grafta
        // İKİNCİ bir düğüm olarak sayılırdı.
        for bad in ["./main.rs", "", "../x.rs", "/tmp/x.rs", "src\\x.rs"] {
            let raw = format!(
                r#"{{"schema_version": 3,
                    "repository_head": "0123456789abcdef0123456789abcdef01234567",
                    "proposals": [{{"new_nodes": [{{"kind": "Module", "initial_mass": 1.0,
                     "path": {bad:?}}}], "reasoning": "alias"}}]}}"#
            );
            let p = write("proposals.v3.json", &raw);
            assert!(
                matches!(
                    load_proposals_file(&p, V2_HEAD, &node_paths()).unwrap_err(),
                    PathKeyedProposalError::InvalidNewNodePath { .. }
                ),
                "{bad:?} must be rejected as non-canonical"
            );
        }
    }
}
