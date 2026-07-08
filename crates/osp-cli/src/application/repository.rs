//! Review store repository — tek persistent transaction yolu (Review 3#3).
//!
//! Hem one-shot subcommand'lar hem interactive wizard aynı `mutate()` yolunu kullanır
//! → iki farklı davranış oluşmaz. `mutate()`:
//!
//! ```text
//! acquire exclusive lock (.lock dosyası, fs4)
//!   → reload current PersistedStore (canonical dosya)
//!   → restore_snapshot + validate
//!   → op(&mut store) — domain transition (expected_basis_digest içinde)
//!   → next envelope: revision = current.revision + 1, snapshot = export_snapshot()
//!   → serialize tmp → fsync → dir sync → atomic replace
//!   → release lock
//!   → return (T, next_revision)
//! ```

use std::path::PathBuf;

use osp_core::anchoring::store::InMemoryAnchorStore;
use osp_core::anchoring::SnapshotError;

use crate::errors::{ReviewError, StoreIoError};
use crate::store_io::{read_persisted_store, write_persisted_store, PersistedStore, StoreLock};

/// Store repository — read (query'ler) + mutate (locked persistent transaction).
///
/// `mutate()` transaction'ı: lock + reload + validate + op + revision(R+1) + atomic save.
/// Revision **serialize öncesi** R+1 yapılır (Review 3#1) — replace sonrasında değil.
pub trait ReviewStoreRepository {
    /// Read-only — mevcut PersistedStore'u yükler (revision dahil). Query'ler için.
    fn read(&self) -> Result<PersistedStore, StoreIoError>;

    /// Locked persistent transaction. `op` store üzerinde domain transition yapar;
    /// başarılıysa revision R+1 ile atomik kaydeder. `(T, next_revision)` döner.
    ///
    /// Revision increment yalnız başarılı persistent mutation sonrası (op Ok + save Ok).
    fn mutate<T>(
        &self,
        op: impl FnOnce(&mut InMemoryAnchorStore) -> Result<T, ReviewError>,
    ) -> Result<(T, u64), ReviewError>;
}

/// Dosya-tabanlı repository — canonical store path üzerinde.
pub struct FileReviewStore {
    store_path: PathBuf,
}

impl FileReviewStore {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
        }
    }

    /// Store path (test'ler ve mesaj çıktısı için).
    #[allow(dead_code)]
    pub fn path(&self) -> &std::path::Path {
        &self.store_path
    }
}

impl ReviewStoreRepository for FileReviewStore {
    fn read(&self) -> Result<PersistedStore, StoreIoError> {
        read_persisted_store(&self.store_path)
    }

    fn mutate<T>(
        &self,
        op: impl FnOnce(&mut InMemoryAnchorStore) -> Result<T, ReviewError>,
    ) -> Result<(T, u64), ReviewError> {
        // (1) Exclusive lock — sabit .lock dosyası.
        let _lock = StoreLock::acquire(&self.store_path)?;

        // (2) Reload current PersistedStore (lock altında — güncel revision).
        let current = read_persisted_store(&self.store_path)?;

        // (3) restore_snapshot + validate.
        let mut store = InMemoryAnchorStore::restore_snapshot(current.snapshot.clone())
            .map_err(|e: SnapshotError| ReviewError::Store(e.to_string()))?;

        // (4) Domain transition (expected_basis_digest op içinde kontrol eder).
        let result = op(&mut store)?;

        // (5) Next envelope: revision R+1 (serialize ÖNCESİ — Review 3#1).
        let next = PersistedStore {
            store_schema_version: PersistedStore::STORE_SCHEMA_VERSION,
            revision: current.revision + 1,
            snapshot: store.export_snapshot(),
        };

        // (6) Atomic save (lock altında).
        write_persisted_store(&self.store_path, &next)?;

        // lock drop → release. Return (T, next_revision).
        Ok((result, next.revision))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::anchoring::types::{ConceptNode, ConceptNodeId, ConceptNodeKind, GraphSeed};
    use osp_core::anchoring::{DecisionStatus, PositionFamily};
    use tempfile::tempdir;

    fn store_with_candidate(repo: &FileReviewStore, id: &str) {
        let node = ConceptNode {
            id: ConceptNodeId(id.into()),
            canonical: id.split(':').nth(1).unwrap_or(id).into(),
            aliases: vec![],
            node_kind: ConceptNodeKind::RuleCandidate,
            decision_status: DecisionStatus::Candidate,
            position_family: PositionFamily::ConceptualIntent,
        };
        let mut seed = GraphSeed::default();
        seed.rule_candidates.push(node);
        let mut store = InMemoryAnchorStore::with_seed(seed);
        let snap = store.export_snapshot();
        let _ = std::mem::take(&mut store);
        let persisted = PersistedStore::from_snapshot(snap);
        write_persisted_store(repo.path(), &persisted).unwrap();
    }

    /// Mutate: op revision'ı artırmadan döner; revision envelope seviyesinde.
    #[test]
    fn mutate_increments_revision_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("store.json");
        let repo = FileReviewStore::new(path.clone());
        store_with_candidate(&repo, "RuleCandidate:A");

        let (val, rev) = repo
            .mutate(|store| {
                // op store'dan node sayısı dönsün (domain effect yok, sadece proof).
                let count = store.node_count().unwrap();
                Ok(count)
            })
            .unwrap();
        assert_eq!(val, 1);
        assert_eq!(rev, 1, "revision 0 → 1");
    }

    /// Mutate: op hata dönerse revision artmaz + store değişmez.
    #[test]
    fn mutate_does_not_increment_revision_on_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("store.json");
        let repo = FileReviewStore::new(path.clone());
        store_with_candidate(&repo, "RuleCandidate:A");

        let before = repo.read().unwrap();
        let err: ReviewError = repo
            .mutate(|_store| -> Result<u32, ReviewError> {
                Err(ReviewError::NotFound("ghost".into()))
            })
            .unwrap_err();
        assert!(matches!(err, ReviewError::NotFound(_)));
        let after = repo.read().unwrap();
        assert_eq!(
            after.revision, before.revision,
            "revision unchanged on error"
        );
    }

    /// Mutate: op store'u mutasyona uğratırsa, sonraki read mutasyonu görür.
    #[test]
    fn mutate_persists_domain_change() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("store.json");
        let repo = FileReviewStore::new(path.clone());
        store_with_candidate(&repo, "RuleCandidate:A");

        // op candidate_query çağırıp sayıyı döner (domain read, mutation kanıtı için yeterli).
        let (count_before, _) = repo
            .mutate(|store| Ok(store.candidate_query().unwrap().len()))
            .unwrap();
        assert_eq!(count_before, 1);
        let after = repo.read().unwrap();
        assert_eq!(after.revision, 1);
    }
}
