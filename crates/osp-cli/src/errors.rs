//! CLI hata tipleri — store I/O + review application.

use std::path::PathBuf;

/// Store I/O hatası — persistence envelope (lock, atomic replace, serde, schema).
#[derive(Debug, thiserror::Error)]
pub enum StoreIoError {
    #[error("invalid store path (no parent/filename): {0}")]
    InvalidStorePath(PathBuf),
    #[error("cannot acquire store lock at {path}: {source}")]
    LockAcquire {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot read store at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot deserialize store at {path}: {source}")]
    Deserialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot serialize store: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot write tmp file at {path}: {source}")]
    WriteTmp {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot atomically replace {from} → {to}: {source}")]
    AtomicReplace {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Envelope store_schema_version uyumsuz (osp-core `SnapshotError` ayrı — graph-seviye).
    #[error("unsupported store schema version: expected={expected}, found={found}")]
    UnsupportedStoreSchema { expected: u32, found: u32 },
}

/// Review application hatası — domain transition (basis freshness, promotability, store).
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("node not found: {0}")]
    NotFound(String),
    #[error("stale basis: node changed after operator reviewed it (TOCTOU)")]
    StaleBasis,
    #[error("not promotable: {0}")]
    NotPromotable(String),
    /// Store-level hata (osp-core `StoreError` veya `SnapshotError`) sarmalanmış.
    #[error("store error: {0}")]
    Store(String),
    /// Persistence katmanı hatası (lock/atomic replace/serde).
    #[error("persistence error: {0}")]
    Persistence(#[from] StoreIoError),
}

/// Review işleminin sonucu (mutation — revision bilmez; revision envelope seviyesinde).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewMutation {
    pub status: String,
    pub node_id: String,
    pub decision_sequence: u64,
}

/// Persisted review sonucu — domain mutation + persistence revision.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersistedReviewOutput {
    pub mutation: ReviewMutation,
    pub revision: u64,
}
