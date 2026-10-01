//! #161 (B5): path-keyed proposals dosyası — v2 zarf + path→id re-bind.
//!
//! v1 format (çıplak JSON array, `Vec<DeltaProposal>`) dokunulmadan yaşamaya
//! devam eder. v2, NodeId alanlarını repo-relative path string olarak taşır;
//! çözümleme yalnızca attempt anındaki baseline `node_paths` üzerinden yapılır.
//!
//! **Yeni-node referansı yok:** v2'de path'ler yalnız MEVCUT baseline node'larına
//! çözümlenir. Yeni node bağlantıları `new_nodes[].connected_to` üzerinden mevcut
//! node'lara kurulur (run 7 AddNode deseni); `new_edges` üzerinden henüz var
//! olmayan dosyaya referans → `UnknownProposalPath` fail-closed.

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
#[derive(Debug, serde::Deserialize)]
pub struct CliPathKeyedProposalsFileV2 {
    pub schema_version: u32,
    pub proposals: Vec<CliPathKeyedProposal>,
}

/// `DeltaProposal`'un path-keyed DTO karşılığı — alan adları birebir, NodeId → path.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct CliPathKeyedProposal {
    pub new_nodes: Vec<CliPathKeyedNewNodeSpec>,
    pub new_edges: Vec<CliPathKeyedEdgeSpec>,
    pub removed_edges: Vec<CliPathKeyedEdgeRef>,
    pub affected_nodes: Vec<String>,
    pub modified_entities: Vec<CliPathKeyedEntityChange>,
    pub position_hints: Vec<CliPathKeyedPositionHint>,
    pub reasoning: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct CliPathKeyedNewNodeSpec {
    pub kind: osp_core::space::NodeKind,
    pub initial_mass: f64,
    pub connected_to: Vec<(String, EdgeKind)>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CliPathKeyedEdgeSpec {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, serde::Deserialize)]
pub struct CliPathKeyedEdgeRef {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, serde::Deserialize)]
pub struct CliPathKeyedEntityChange {
    pub path: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct CliPathKeyedPositionHint {
    pub path: String,
    pub suggested_raw: RawPosition,
    pub rationale: String,
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
        "proposals file object envelope requires schema_version 2 (found {found}) — \
         plain arrays are v1 (node-id keyed)"
    )]
    UnsupportedEnvelopeVersion { found: String },
    #[error("proposals file object envelope is missing schema_version")]
    MissingEnvelopeVersion,
    #[error("proposals file must be a JSON array (v1) or an object envelope (v2), found {found}")]
    UnexpectedTopLevelShape { found: String },
    #[error("path-keyed proposals resolve only existing baseline nodes; use connected_to for new-node links — {0}")]
    PathBinding(#[from] PathBindingError),
}

/// Proposals dosyası yükle + re-bind → `Vec<DeltaProposal>`.
///
/// Dispatch: JSON array → v1 (`Vec<DeltaProposal>`, dokunulmaz) · JSON object
/// with `schema_version: 2` → path-keyed v2 (baseline'a karşı çözümle).
pub fn load_proposals_file(
    path: &Path,
    node_paths: &HashMap<NodeId, String>,
) -> Result<Vec<DeltaProposal>, PathKeyedProposalError> {
    let raw = std::fs::read_to_string(path).map_err(|source| PathKeyedProposalError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    match &value {
        serde_json::Value::Array(_) => Ok(serde_json::from_value(value)?),
        serde_json::Value::Object(_) => {
            let file: CliPathKeyedProposalsFileV2 = match value.get("schema_version") {
                Some(v) if v.as_u64() == Some(2) => serde_json::from_value(value)?,
                Some(v) => {
                    return Err(PathKeyedProposalError::UnsupportedEnvelopeVersion {
                        found: v.to_string(),
                    })
                }
                None => return Err(PathKeyedProposalError::MissingEnvelopeVersion),
            };
            let path_to_id = build_path_to_id(node_paths)?;
            file.proposals
                .into_iter()
                .map(|p| rebind_proposal(p, &path_to_id))
                .collect()
        }
        other => Err(PathKeyedProposalError::UnexpectedTopLevelShape {
            found: other.to_string(),
        }),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::space::EdgeKind;

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
        let out = load_proposals_file(&p, &node_paths()).unwrap();
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
        let out = load_proposals_file(&p, &node_paths()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].removed_edges[0].from, 2, "main.rs → id 2");
        assert_eq!(out[0].removed_edges[0].to, 1, "b.rs → id 1");
        assert_eq!(out[0].affected_nodes, vec![2]);
    }

    #[test]
    fn v2_new_node_connected_to_resolves() {
        let raw = r#"{
            "schema_version": 2,
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
        let out = load_proposals_file(&p, &node_paths()).unwrap();
        assert_eq!(out[0].new_nodes.len(), 1);
        assert_eq!(
            out[0].new_nodes[0].connected_to,
            vec![(0, EdgeKind::Imports)]
        );
    }

    #[test]
    fn v2_unknown_path_fails_closed_with_field_context() {
        let raw = r#"{
            "schema_version": 2,
            "proposals": [{
                "removed_edges": [{"from": "stale.rs", "to": "b.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, &node_paths()).unwrap_err();
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
            "proposals": [{
                "new_edges": [{"from": "not-yet-a-file.rs", "to": "a.rs", "kind": "Imports"}]
            }]
        }"#;
        let p = write("proposals.v2.json", raw);
        let err = load_proposals_file(&p, &node_paths()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("connected_to"), "guidance in message: {msg}");
        assert!(msg.contains("not-yet-a-file.rs"), "path in message: {msg}");
    }

    #[test]
    fn object_envelope_without_schema_version_rejected() {
        let p = write("proposals.json", r#"{"proposals": []}"#);
        assert!(matches!(
            load_proposals_file(&p, &node_paths()).unwrap_err(),
            PathKeyedProposalError::MissingEnvelopeVersion
        ));
    }

    #[test]
    fn object_envelope_wrong_version_rejected() {
        let p = write(
            "proposals.json",
            r#"{"schema_version": 3, "proposals": []}"#,
        );
        assert!(matches!(
            load_proposals_file(&p, &node_paths()).unwrap_err(),
            PathKeyedProposalError::UnsupportedEnvelopeVersion { .. }
        ));
    }

    #[test]
    fn scalar_top_level_rejected() {
        let p = write("proposals.json", r#""nope""#);
        assert!(matches!(
            load_proposals_file(&p, &node_paths()).unwrap_err(),
            PathKeyedProposalError::UnexpectedTopLevelShape { .. }
        ));
    }
}
