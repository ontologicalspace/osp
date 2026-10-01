//! #161 (B5): path-keyed binding altyapısı — path→id deterministik çözümleme.
//!
//! Node id, sıralı dosya listesi üzerindeki enumerasyon indeksidir
//! (`osp-analyzer/src/pipeline.rs`) — dosya seti değişince kayar, yön tahmin
//! edilemez. Path dosyanın kalıcı kimliğidir; bu modül path→id yönünde
//! **çözümleme** yapar (v1 `CliScopeBinding` yalnızca id→path **doğrulama**
//! yapardı).
//!
//! Çözümleme kaynağı: attempt anında üretilen taze `AnalysisResult.node_paths`
//! (id→path). Bu, Live Contract v1.1'in "S_applied == S_candidate (re-bind
//! before apply)" ilkesinin task/proposal tarafındaki karşılığıdır: path-keyed
//! dosya, id-keyed iç temsile attempt anındaki baseline'a karşı re-bind edilir.

#![allow(dead_code, reason = "#161: wired incrementally across commits")]

use std::collections::HashMap;

use osp_core::space::NodeId;

/// Path→id haritası kurma hatası.
#[derive(Debug, thiserror::Error)]
pub enum PathBindingError {
    /// `node_paths` değer uzayı injective değil — analyze bijection invariant'i
    /// (CLI tarafı defensive yansıması; normalde imkânsız).
    #[error("analysis node_paths is not path-injective: duplicate path {path}")]
    NotPathInjective { path: String },
    /// Path, attempt anındaki baseline'da çözümlenemedi.
    #[error("path {path} not present in analysis node_paths{context}")]
    UnknownPath { path: String, context: String },
}

/// `node_paths` (id→path) ters çevirimi → path→id.
///
/// Bijection defensive doğrulaması: iki farklı id aynı path'i taşıyorsa
/// fail-closed (analyze tarafındaki `validate_node_paths_bijection` invariant'inin
/// CLI tarafı yansıması — normalde erişilemez, ama invariant CLI'ye de aittir).
pub fn build_path_to_id(
    node_paths: &HashMap<NodeId, String>,
) -> Result<HashMap<String, NodeId>, PathBindingError> {
    let mut map: HashMap<String, NodeId> = HashMap::with_capacity(node_paths.len());
    for (id, path) in node_paths {
        if let Some(existing) = map.insert(path.clone(), *id) {
            if existing != *id {
                return Err(PathBindingError::NotPathInjective { path: path.clone() });
            }
        }
    }
    Ok(map)
}

/// Tek path çözümle — unknown path fail-closed.
///
/// `context` hata mesajında alan konumunu taşır (örn. ` (removed_edges.from)`),
/// boş string = bağlam yok.
pub fn resolve_path(
    path_to_id: &HashMap<String, NodeId>,
    path: &str,
    context: &str,
) -> Result<NodeId, PathBindingError> {
    path_to_id
        .get(path)
        .copied()
        .ok_or_else(|| PathBindingError::UnknownPath {
            path: path.to_string(),
            context: context.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn np(pairs: &[(NodeId, &str)]) -> HashMap<NodeId, String> {
        pairs.iter().map(|(i, p)| (*i, p.to_string())).collect()
    }

    #[test]
    fn build_path_to_id_inverts_node_paths() {
        let map = build_path_to_id(&np(&[(0, "a.rs"), (2, "main.rs")])).unwrap();
        assert_eq!(map.get("a.rs"), Some(&0));
        assert_eq!(map.get("main.rs"), Some(&2));
        assert!(!map.contains_key("b.rs"));
    }

    #[test]
    fn build_path_to_id_rejects_duplicate_paths() {
        // Defensive: analyze bijection invariant'i bozulmuş olsaydı.
        let err = build_path_to_id(&np(&[(0, "a.rs"), (1, "a.rs")])).unwrap_err();
        assert!(matches!(err, PathBindingError::NotPathInjective { .. }));
    }

    #[test]
    fn resolve_path_unknown_fails_closed_with_context() {
        let map = build_path_to_id(&np(&[(0, "a.rs")])).unwrap();
        let err = resolve_path(&map, "gone.rs", " (scope_bindings)").unwrap_err();
        match err {
            PathBindingError::UnknownPath { path, context } => {
                assert_eq!(path, "gone.rs");
                assert_eq!(context, " (scope_bindings)");
            }
            other => panic!("expected UnknownPath, got {other:?}"),
        }
    }
}
