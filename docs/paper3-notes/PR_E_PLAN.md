# PR E Plan — Entity Resolution Core + Persistence Contract (yeni oturum için)

> **Dal:** `feat/entity-resolution-core` (main `6a8a923` üstünde — PR #59 merged)
> **Scope:** osp-core + minimal osp-cli persistence envelope migration
> **2 tur plan review sonucu: implementation-ready**

## Özet

`CodeEntityCandidate ─ResolvesTo→ CodeEntity` identity resolution core contract + atomik
transition + snapshot persistence. **Dar V1:** identity-resolution core (yok: projection,
evidence provider migration, operator CLI command — her biri ayrı PR). Tur 2 review 4 P1 + 5 P2
düzeltileri + 4 son nokta entegre: store-owned identity binding, scheme case-policy, status
semantics, basis-pinned Created/Reused outcome, snapshot schema v2, explicit migration,
PresentedResolutionBasis::compile, deterministic entity ID derivation.

## Ontolojik sözleşme (tur 1 + tur 2)

```
node identity ≠ physical code identity
ResolvesTo ≠ promotion (identity resolution, acceptance değil)
evidence resolution ≠ evidence duplication (PR F: CodeIdentityKey merkezi)
```

```
CodeEntityCandidate:<key>  ──RESOLVES_TO──▶  CodeEntity:<derived-id>
  (operator-accepted provenance)              (live canonical identity)
```

İki node aynı `CodeIdentityKey`'i taşır (store-owned binding), ama ayrı immutable graph
identity'lerdir. Resolution, `DecisionStatus` acceptance lane'i ile karışmaz (D15 iki-lane).

## Tur 2 review kararları (4 P1 + 5 P2 zorunlu)

1. **P1-A — `CodeIdentityKey` sahipliği:** `ConceptNode` alanı DEĞİL; store-owned
   `CodeIdentityBinding` katmanı (universal node büyümez; yalnız PhysicalCode node'ları key taşır;
   PR F provider migration doğrudan indeksi kullanır; snapshot validator R2/R7'yi doğrular).
2. **P1-B — `CodeIdentityScheme` case-policy:** `AnalysisPathV1` tek başına yetmez; path case
   policy (CaseSensitive / AsciiCaseInsensitive) scheme'in parçası. Core canonical identity scheme
   tipi eklenir; mevcut CLI scheme/policy adoption ayrı bridge PR'sinde (PR E `graph init` node'ları
   otomatik binding taşımaz — gap HANDOFF'ta).
3. **P1-C — Status semantics:** source candidate pre/post + target entity initial + edge status
   normatif (aşağıda). Resolution acceptance lane'iyle karışmaz.
4. **P1-D — N:1 reuse vs create:** `ResolutionOutcome { Created, Reused }`; `PresentedResolutionBasis`
   target'ı pinler (create→reuse sessiz dönüşümü YOK; `StaleResolutionTarget`).
5. **P2-A — `CodeIdentityKey` serde smart constructor:** custom Deserialize `new()` üzerinden
   (derive bypass YOK; boş/whitespace/NUL/control reject).
6. **P2-B — "live CodeEntity" predicate:** `is_live_code_identity()` (Candidate | Accepted; değil:
   Rejected/Deprecated/SupersededAccepted). R7 validator predicate.
7. **P2-C — Cycle kontrolü redundant:** R3+R4 korunursa cycle yapısal imkansız; cycle scan
   defense-in-depth (merkezde değil). Daha değerli: malformed `CodeEntity→CodeEntity` reject.
8. **P2-D — Edge status + explanation:** `ResolvesTo` edge `DecisionStatus::Accepted` +
   `Some(reason)` (gate'ten geçmez, store üretir; INV-C7 explanation zorunlu, validator doğrular).
9. **P2-E — `AnchorStore` trait yüzeyi:** `apply_resolution` + `resolution_ledger` +
   `resolution_basis_view` + `resolution_target_for_identity` metodları.

## Son 4 nokta (tur 2 final düzeltmeler)

10. **Başlık düzeltme:** "4 P2" → "5 P2" (P2-A/B/C/D/E = 5 madde).
11. **Explicit `PersistedStoreV1 → PersistedStoreV2` migration** (nokta 2): envelope seviyesinde
    açık `PersistedStoreV1` + `TryFrom<PersistedStoreV1> for PersistedStoreV2`; ardından core
    `restore_snapshot` validation. `#[serde(default)]` DEĞİL — kontrollü, test edilebilir migration.
12. **`PresentedResolutionBasis::compile`** (nokta 3): Accepted candidate için ayrı compile yolu
    (mevcut `PresentedBasis::compile` yalnız `candidate_query()` → Candidate node bulur; Accepted
    source için kullanılamaz). `resolution_basis_view` canonical pre-state compiler.
13. **`CodeIdentityKey → CodeEntityId` deterministic derivation** (nokta 4):
    `derive_resolved_code_entity_id(&CodeIdentityKey) -> ConceptNodeId` (domain tag + scheme +
    case policy + canonical key). Aynı key → aynı proposed ID; farklı scheme/policy → farklı domain.

---

## Status politikası (tur 2 P1-C — normatif)

```
Source precondition:
  CodeEntityCandidate + Accepted + PhysicalCode
Source after resolution:
  Accepted olarak değişmeden kalır (history provenance)
Target CodeEntity (Created):
  initial = Candidate (otomatik mainline'a alınmaz; kendi review süreci)
Target CodeEntity (Reused):
  existing live CodeEntity (is_live_code_identity)
ResolvesTo edge:
  Accepted + required explanation (INV-C7)
ResolutionRecord:
  committed identity-resolution provenance (DecisionRecord yerine geçmez)
```

Avantaj: resolution acceptance lane'iyle karışmaz; candidate önce normal operator review'dan geçer;
yeni CodeEntity otomatik Accepted olmaz; INV-C13 genişletilmez.

---

## Entity ID derivation (tur 2 nokta 4 — normative)

```rust
/// Deterministic CodeIdentityKey → CodeEntity ID derivation.
///
/// Hash/input materyali:
///   domain tag (osp:code-entity:v1) + scheme variant + case policy + canonical key
///
/// Invariantlar:
///   aynı CodeIdentityKey → aynı proposed CodeEntity ID
///   farklı scheme/policy → farklı identity domain (collision imkansız)
fn derive_resolved_code_entity_id(key: &CodeIdentityKey) -> ConceptNodeId;
```

**Apply sırasında collision politikası:**
- ID boş (Created) → yeni entity oluştur
- aynı ID + aynı key + uygun canlı entity + basis `Reuse` → reuse
- aynı ID + farklı material/key → `EntityIdentityCollision` error
- basis `Create` iken target sonradan oluşmuş → `StaleResolutionTarget` (create→reuse sessiz dönüşüm YOK)

Create basis bu ID'yi pinler (`PresentedResolutionTarget::Create { proposed_entity_id }`).

---

## osp-core değişiklikleri

### A. `CodeIdentityScheme` + `CodeIdentityKey` (yeni identity.rs veya types.rs)

```rust
/// Path case normalization policy (core canonical identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CodePathCasePolicy {
    CaseSensitive,
    AsciiCaseInsensitive,
}

/// Physical code identity scheme (tur 2 P1-B — case-policy scheme'in parçası).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub enum CodeIdentityScheme {
    AnalysisPathV1 { case_policy: CodePathCasePolicy },
}

/// Physical code identity key — node ID ≠ physical code identity.
///
/// İki ayrı ontolojik node (CodeEntityCandidate + CodeEntity) aynı fiziksel kod varlığına
/// gönderme yapar. Evidence (PR F) bu key üzerinden çözülür; node ID'ye kopyalanmaz.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct CodeIdentityKey {
    scheme: CodeIdentityScheme,
    key: String,
}

impl CodeIdentityKey {
    /// Smart constructor — boş/whitespace/NUL/control reject (tur 2 P2-A).
    pub fn new(
        scheme: CodeIdentityScheme,
        key: impl Into<String>,
    ) -> Result<Self, CodeIdentityKeyError>;

    /// Deterministic entity ID derivation (tur 2 nokta 4).
    pub fn derive_entity_id(&self) -> ConceptNodeId {
        derive_resolved_code_entity_id(self)
    }
}

/// Custom Deserialize — `new()` üzerinden (tur 2 P2-A; derive bypass YOK).
impl<'de> serde::Deserialize<'de> for CodeIdentityKey { /* DTO → new() */ }

fn derive_resolved_code_entity_id(key: &CodeIdentityKey) -> ConceptNodeId;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CodeIdentityKeyError {
    #[error("identity key boş/whitespace olamaz")]
    Empty,
    #[error("identity key NUL/control character içeremez")]
    ControlCharacter,
}
```

### B. `CodeIdentityBinding` — store-owned binding katmanı (tur 2 P1-A)

```rust
/// Node ↔ physical code identity binding (store-owned; ConceptNode alanı DEĞİL).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodeIdentityBinding {
    pub node_id: ConceptNodeId,
    pub identity_key: CodeIdentityKey,
}
```

Store içinde: `code_identity_bindings: BTreeMap<ConceptNodeId, CodeIdentityKey>`.
Snapshot'ta: `code_identity_bindings: Vec<CodeIdentityBinding>` (deterministik sort by node_id).
Yalnız PhysicalCode node'ları binding taşır; universal `ConceptNode` büyümez.

### C. `ConceptEdgeKind::ResolvesTo` (mod.rs) — 16. variant

`is_high_stake` true (INV-C7 explanation zorunlu). Doc comment "16 = 15 ontolojik + 1 meta".

### D. `ResolutionOutcome` + `PresentedResolutionTarget` (review.rs)

```rust
/// Resolution outcome — Created (yeni entity) veya Reused (mevcut live entity).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome", content = "payload")]
pub enum ResolutionOutcome {
    Created { entity_id: ConceptNodeId },
    Reused { entity_id: ConceptNodeId },
}

/// Basis target — operator'ın gördüğü outcome pinlenir (tur 2 P1-D).
/// Create: proposed_entity_id deterministic derivation'dan.
/// Reuse: mevcut live entity ID + digest (freshness için).
#[derive(Debug, Clone, PartialEq)]
pub enum PresentedResolutionTarget {
    Create { proposed_entity_id: ConceptNodeId },
    Reuse { entity_id: ConceptNodeId, entity_digest: NodeDigest },
}
```

### E. `PresentedResolutionBasis` + `ResolutionApplication` (review.rs — SupersedeSession mirror)

```rust
/// Resolution basis — store'dan derlenir (INV-C12 freshness). Target outcome pinlenir.
///
/// Mevcut PresentedBasis::compile Candidate-only (candidate_query); Accepted source için
/// ayrı compile yolu (tur 2 nokta 3). compile doğrular:
///   node exists + node_kind == CodeEntityCandidate + PhysicalCode + Accepted
///   + identity binding exists + target outcome (Create/Reuse)
pub struct PresentedResolutionBasis {
    candidate_id: ConceptNodeId,
    candidate_digest: NodeDigest,
    identity_key: CodeIdentityKey,
    target: PresentedResolutionTarget,  // create vs reuse pin
    compiled_at: SystemTime,
}

impl PresentedResolutionBasis {
    /// Canonical pre-state compiler (tur 2 nokta 3). Accepted candidate için ayrı yol.
    pub fn compile<S: AnchorStore>(
        store: &S, candidate_id: &ConceptNodeId,
    ) -> Result<Self, ResolutionError>;
}

/// Opaque application — yalnız Session üretir (private fields + pub(crate) new + no Deserialize).
pub struct ResolutionApplication {
    candidate_id: ConceptNodeId,
    entity_id: ConceptNodeId,
    identity_key: CodeIdentityKey,
    outcome: ResolutionOutcome,
    basis: PresentedResolutionBasis,
    reason: NonEmptyExplanation,
    session_id: SessionId,
    operator: OperatorId,
    resolved_at: SystemTime,
}
```

### F. `ResolutionRecord` (review.rs — SupersedeRecord mirror)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolutionRecord {
    pub seq: u64,                              // global audit_seq (3 ledger union)
    pub session_id: SessionId,
    pub operator: OperatorId,
    pub candidate_id: ConceptNodeId,
    pub entity_id: ConceptNodeId,
    pub identity_key: CodeIdentityKey,
    pub outcome: ResolutionOutcome,
    pub reason: NonEmptyExplanation,
    pub candidate_digest: u64,                 // raw u64 (NodeDigest Serialize-only; domain alanı)
    pub entity_digest: u64,
    pub basis_fingerprint: [u8; 32],           // domain-tag osp:resolution-basis:v1
    pub at: SystemTime,
}
```

### G. `CodeEntityResolutionSession` (review.rs — SupersedeSession mirror)

```rust
pub struct CodeEntityResolutionSession {
    session_id: SessionId,
    operator: OperatorId,
    opened_at: SystemTime,
    resolutions: u64,
}

impl CodeEntityResolutionSession {
    pub fn open_for_operator(operator: OperatorId) -> Self;
    pub fn resolve<S: AnchorStore>(
        &self, store: &mut S, candidate_id: ConceptNodeId,
        basis: PresentedResolutionBasis, reason: NonEmptyExplanation,
    ) -> Result<ResolutionRecord, ResolutionError>;
}
```

### H. `AnchorStore` trait yüzeyi (tur 2 P2-E)

```rust
// Yeni trait metodları (mevcut apply_decision/apply_supersede yanına):
fn apply_resolution(
    &mut self, application: ResolutionApplication,
) -> Result<ResolutionRecord, Self::Error>;

fn resolution_ledger(&self) -> Vec<ResolutionRecord>;

fn resolution_basis_view(
    &self, candidate: &ConceptNodeId,
) -> Result<ResolutionBasisView, Self::Error>;

fn resolution_target_for_identity(
    &self, key: &CodeIdentityKey,
) -> Result<Option<ConceptNode>, Self::Error>;  // N:1 reuse lookup
```

### I. `DecisionStatus::is_live_code_identity()` predicate (tur 2 P2-B)

```rust
impl DecisionStatus {
    /// R7 "live CodeEntity" predicate (tur 2 P2-B).
    pub const fn is_live_code_identity(self) -> bool {
        matches!(self, Self::Candidate | Self::Accepted)
    }
}
```

### J. `apply_resolution` store transition (store.rs — 14-step, tur 2 final)

Lane-sensitive (candidate proposal edges hariç):

```
1.  Basis candidate/application endpoint match (ResolutionBasisMismatch)
2.  Candidate existence
3.  Candidate kind == CodeEntityCandidate
4.  Candidate family == PhysicalCode
5.  Candidate status == Accepted
6.  Candidate digest freshness (StaleResolutionBasis — INV-C12)
7.  Candidate identity binding exists and matches basis (R2)
8.  Candidate has no committed outgoing ResolvesTo (R6)
9.  Recompute target selection from current state
10. Basis-pinned Create/Reuse outcome still matches (StaleResolutionTarget)
11. Reuse target kind/family/status/key validation (R4, R5, R7)
12. Entity ID/material collision check (EntityIdentityCollision)
13. audit_sequence checked_add (AuditSequenceExhausted)
14. No-fallible mutation block:
      - Created: yeni CodeEntity node (Candidate, PhysicalCode) + CodeIdentityBinding (her iki node)
      - Reused: CodeIdentityBinding (candidate; entity zaten sahip)
      - ConceptEdge { from: candidate, to: entity, kind: ResolvesTo,
                      decision_status: Accepted, explanation: Some(reason) }
      - self.audit_seq = next_seq; ResolutionRecord; self.resolution_ledger.push(record)
```

### K. Snapshot validation (store.rs `validate_snapshot` — INV-C16 triangulation)

`AnchorStoreSnapshot` + `resolution_records: Vec<ResolutionRecord>` + `code_identity_bindings: Vec<CodeIdentityBinding>`.

Validation parallel to supersede section:
1. Resolution record → node existence (her iki endpoint)
2. Status forward integrity (source Accepted kalır; target Created=Candidate/Reused=live)
3. R2: binding key equality (candidate + entity aynı key)
4. R3: source yalnız CodeEntityCandidate; R4: target yalnız CodeEntity
5. R5: her iki uç PhysicalCode family
6. R6: candidate başına ≤1 outgoing committed ResolvesTo
7. R7: same key için ≤1 live CodeEntity
8. Three-way: committed ResolvesTo edge ↔ record ↔ binding key
9. INV-C7: committed ResolvesTo explanation non-empty
10. audit_seq density: decision + supersede + resolution union (3 ledger)
11. Cycle scan (defense-in-depth — R3+R4 ile yapısal imkansız; tur 2 P2-C)

### L. ConceptGraphSnapshot — SCHEMA_VERSION additive

`ConceptGraphSnapshot::SCHEMA_VERSION = 1` kalır (additive enum variant + yeni ledger fields;
`ResolvesTo` varyantı outer store v2 ile sınırlandırılmış). Asıl format bump CLI envelope'da (M maddesi).

---

## osp-cli değişiklikleri (minimal persistence envelope migration)

### M. `PersistedStoreV1 → PersistedStoreV2` explicit migration (tur 2 P1-F + nokta 2)

```rust
// store_io.rs — explicit v1/v2 envelope tipleri (#[serde(default)] DEĞİL)

/// Legacy v1 store envelope (decision + supersede ledger).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedStoreV1 {
    graph: ConceptGraphSnapshot,
    decision_records: Vec<DecisionRecord>,
    supersede_records: Vec<SupersedeRecord>,
    audit_sequence: u64,
}

/// Current v2 store envelope (+ resolution ledger + identity bindings).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedStore {
    pub graph: ConceptGraphSnapshot,
    pub decision_records: Vec<DecisionRecord>,
    pub supersede_records: Vec<SupersedeRecord>,
    pub resolution_records: Vec<ResolutionRecord>,
    pub code_identity_bindings: Vec<CodeIdentityBinding>,
    pub audit_sequence: u64,
    pub store_schema_version: u32,  // = 2
}

impl PersistedStore {
    pub const STORE_SCHEMA_VERSION: u32 = 2;
}

/// Explicit v1 → v2 migration (tur 2 nokta 2 — kontrollü, test edilebilir).
impl TryFrom<PersistedStoreV1> for PersistedStore {
    type Error = StoreIoError;
    fn try_from(v1: PersistedStoreV1) -> Result<Self, Self::Error> {
        // v1 store yüklenirse:
        //   code_identity_bindings = []
        //   resolution_records = []
        //   audit_sequence mevcut değeri korunur
        //   graph ve eski iki ledger değişmeden migrate edilir
        //   envelope store_schema_version = 2 olarak yeniden yazılır
        Ok(Self {
            graph: v1.graph,
            decision_records: v1.decision_records,
            supersede_records: v1.supersede_records,
            resolution_records: Vec::new(),
            code_identity_bindings: Vec::new(),
            audit_sequence: v1.audit_sequence,
            store_schema_version: Self::STORE_SCHEMA_VERSION,
        })
    }
}
```

Read path: version dispatch (v1 → TryFrom → v2 → core `restore_snapshot` validation).
Persistence round-trip testleri: v2 write → read → restore validation; v1 read → migration → v2.

**YOK:** CLI command (`osp review resolve-code-entity`), evidence provider migration, projection.

---

## 10 invariant (R1-R10)

- **R1:** Node ID immutable (ID HashMap key; promotion ID rewrite YOK)
- **R2:** Candidate ve Entity aynı `CodeIdentityKey`'i taşır (binding katmanı)
- **R3:** `ResolvesTo` source yalnız `CodeEntityCandidate`
- **R4:** `ResolvesTo` target yalnız `CodeEntity`
- **R5:** İki uç da PhysicalCode family
- **R6:** Candidate başına en fazla bir outgoing `ResolvesTo`
- **R7:** Aynı `CodeIdentityKey` için en fazla bir canlı CodeEntity (`is_live_code_identity`; N:1 destekler)
- **R8:** Resolution edge immutable (acyclic — R3+R4 ile yapısal garanti; cycle defense-in-depth)
- **R9:** Target selection/optional creation + identity binding + committed edge + audit record atomiktir (INV-C16; Created/Reused outcome)
- **R10:** Resolution evidence üretmez, strength değiştirmez (PR F evidence migration)

---

## Kabul kriterleri (~22)

1. `CodeIdentityScheme` (case-policy dahil) + `CodeIdentityKey` (smart constructor + custom Deserialize + `derive_entity_id`)
2. `CodeIdentityBinding` store-owned katmanı (ConceptNode alanı DEĞİL)
3. `ConceptEdgeKind::ResolvesTo` (16. variant; high-stake)
4. `ResolutionOutcome { Created, Reused }` + `PresentedResolutionTarget` (basis-pinned)
5. `PresentedResolutionBasis::compile` (Accepted candidate ayrı compile yolu; tur 2 nokta 3)
6. `PresentedResolutionBasis` + opaque `ResolutionApplication` (SupersedeSession mirror)
7. `ResolutionRecord` (audit; basis_fingerprint domain-tag `osp:resolution-basis:v1`)
8. `CodeEntityResolutionSession` (crate-private authority issuer; atomik `resolve`)
9. `AnchorStore` trait: `apply_resolution` + `resolution_ledger` + `resolution_basis_view` + `resolution_target_for_identity`
10. `apply_resolution` 14-step (lane-sensitive; no fallible path after mutation marker)
11. Status politikası: source Accepted kalır; target Created=Candidate/Reused=live; edge Accepted+explanation
12. `DecisionStatus::is_live_code_identity()` predicate (R7)
13. `derive_resolved_code_entity_id` deterministic (tur 2 nokta 4) + collision politikası
14. R1-R10 invariantları
15. N:1 reuse (R7; same key → existing live entity; no duplicate)
16. Basis target pin (create→reuse sessiz dönüşüm YOK; `StaleResolutionTarget`)
17. Snapshot validation INV-C16 triangulation (3 ledger union audit_seq density)
18. `PersistedStoreV1 → PersistedStoreV2` explicit migration (TryFrom; tur 2 nokta 2)
19. `STORE_SCHEMA_VERSION: 1 → 2` + persistence round-trip testleri
20. Typed error (anyhow YOK; `ResolutionError` + `StoreError` + `StoreIoError` variants)
21. `ConceptGraphSnapshot::SCHEMA_VERSION = 1` additive (backward-compat)
22. osp-cli command YOK (evidence/provider/projection ayrı PR)

---

## Test matrisi (~25)

### Identity type (5)
```
code_identity_key_valid_construction
code_identity_key_empty_reject
code_identity_key_control_character_reject
code_identity_scheme_case_policy_participates_in_equality
code_identity_key_custom_deserialize_validates
derive_entity_id_deterministic_same_key_same_id
derive_entity_id_different_scheme_different_domain
```

### Created target (4)
```
accepted_candidate_resolves_to_newly_created_entity
entity_initial_status_pinned_candidate
resolves_to_edge_accepted_with_explanation
record_outcome_created
```

### Reused target (5)
```
second_candidate_same_key_resolves_to_existing_entity
no_duplicate_entity_created_on_reuse
record_outcome_reused
different_key_entity_cannot_be_reused
stale_reused_target_digest_rejects
target_appeared_after_create_basis_rejects  # StaleResolutionTarget
entity_identity_collision_rejects           # farklı material aynı ID
```

### Failure atomikliği (6)
```
stale_candidate_digest_rejects
wrong_candidate_kind_rejects
wrong_family_rejects
candidate_not_accepted_rejects
already_resolved_candidate_rejects  # R6
duplicate_live_entity_corruption_rejects  # R7 violation
audit_sequence_overflow_rejects
every_failure_leaves_graph_bindings_ledgers_audit_seq_unchanged
```

### Snapshot adversarial (8)
```
record_without_edge_rejects
edge_without_record_rejects
record_edge_endpoint_mismatch_rejects
binding_key_mismatch_rejects  # R2
wrong_source_kind_rejects  # R3
wrong_target_kind_rejects  # R4 (CodeEntity→CodeEntity malformed)
duplicate_outgoing_resolution_rejects  # R6
two_live_entities_same_key_rejects  # R7
resolution_record_outcome_inconsistent_rejects
audit_density_across_three_ledgers
deterministic_export_ordering
```

### Persistence migration (3)
```
v1_store_migrates_to_v2_empty_resolution_ledger
v2_store_round_trip_restore_validation
v1_store_audit_sequence_preserved
```

### Type boundary (compile-fail, 2)
```
c16_resolution_application_literal  # opaque application struct literal engelli
c16_resolution_application_deserialize  # Deserialize YOK
```

---

## Uygulama sırası

0. `ConceptEdgeKind::ResolvesTo` + `is_high_stake` (mod.rs)
1. `CodeIdentityScheme` + `CodePathCasePolicy` + `CodeIdentityKey` + `CodeIdentityKeyError` + `derive_resolved_code_entity_id` (identity.rs)
2. `CodeIdentityBinding` (types.rs)
3. `DecisionStatus::is_live_code_identity()` (mod.rs)
4. `ResolutionOutcome` + `PresentedResolutionTarget` + `PresentedResolutionBasis::compile` + `ResolutionApplication` (review.rs)
5. `ResolutionRecord` + `ResolutionError` (review.rs)
6. `CodeEntityResolutionSession` (review.rs)
7. `AnchorStore` trait metodları + `InMemoryAnchorStore::apply_resolution` (store.rs)
8. `AnchorStoreSnapshot` + `code_identity_bindings` + `resolution_records` (store.rs)
9. `validate_snapshot` INV-C16 triangulation (store.rs)
10. `PersistedStoreV1` + `PersistedStoreV2` + `TryFrom` migration (store_io.rs)
11. Compile-fail fixture'ları (c16_resolution_application_*)
12. Workspace validation
13. HANDOFF/STATUS/run-metadata güncelleme

---

## PR serisi (PR E sonrası)

- **PR F — Evidence identity migration:** `ObservedCodeEvidence.code_entity_id` → `code_identity_key`;
  provider `CodeIdentityKey` merkezi; `CodeIdentityResolver` + `ResolvedCodeEvidenceProvider`
  node-facing adapter. E1-E8 evidence invariantları.
- **PR G — Lineage-aware effective projection:** `Concept → Candidate → Entity` derived `ImplementedBy`
  (read-only; tarihsel `ExpectedImplementation` korunur).
- **CLI PR:** `osp review resolve-code-entity <candidate>` (core transition sabitlendikten sonra).

---

## HANDOFF bullet'leri (PR E sonrası)

- **CLI scheme adoption gap:** PR E core canonical identity scheme ekler ama mevcut `graph init`
  node'ları otomatik `CodeIdentityBinding` taşımaz (CLI scheme/policy adoption ayrı bridge PR).
- **Evidence ownership PR F'de:** `CodeIdentityKey` provider merkezi; node-facing adapter geçici.
- **Projection PR G'de:** derived `ImplementedBy` (read-only); tarihsel `ExpectedImplementation` korunur.
- **CLI surface:** `osp review resolve-code-entity` core transition sabitlendikten sonra.
- **N:1 cardinality V1'de desteklenir (R7);** V1 test 1:1 ile başlayabilir.
- **`ConceptGraphSnapshot::SCHEMA_VERSION = 1` additive;** asıl format bump CLI envelope v2.

---

## run-metadata: current protocol — compile-fail 26→28 (c16 resolution application); frozen snapshot untouched.
