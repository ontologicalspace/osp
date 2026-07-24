//! INV-T9 — Validated canonical numeric tag newtype'ları.
//!
//! **reviewer P1:** Önceki tasarımda bu tipler ham `u8` alias'ıydı (`pub type
//! CanonicalNodeKind = u8`). Bu, `kind = 255` gibi imkânsız canonical değerlerin
//! oluşturulmasına izin veriyordu ve domain enum'a yeni varyant eklendiğinde
//! compiler mapping'in güncellenmesini zorunlu kılmıyordu.
//!
//! Bu modül her domain enum için private-field validated newtype + exhaustive
//! `TryFrom<&DomainEnum>` sağlar. Yeni varyant eklendiğinde compiler mapping'in
//! güncellenmesini zorunlu kılar (match exhaustive, `#[non_exhaustive]` DEĞİL).
//!
//! **Serde (reviewer P1-1):** newtype'lar `u8` olarak serialize edilir. Deserialize
//! GEÇERSİZ tag'i REDDER — custom `Deserialize → TryFrom<u8>` zinciri `VALID_TAGS` set
//! dışındaki değeri `CanonicalizationError::InvalidCanonicalTag` ile reddeder (örn diskten
//! `kind = 255`). Diskten yüklenen artifact construction'a varmadan valide edilir;
//! imkânsız tag üretilemez ve korunamaz.

use crate::coords::MetricSource;
use crate::space::{NodeClassification, NodeRole};
use crate::trajectory::{ComparisonOp, PredicateAxis};
use crate::vision::VisionSource;

// ═══════════════════════════════════════════════════════════════════════════════
// Macro — her domain enum için validated newtype üretir.
//
// Tek makro tekrarı önler ve tutarlı API sağlar: `as_u8()`, `TryFrom<&Domain>`,
// `Serialize`/`Deserialize` (transparent u8), `Debug`/`Clone`/`Copy`/`PartialEq`/`Eq`/`Hash`.
// ═══════════════════════════════════════════════════════════════════════════════

macro_rules! canonical_tag_newtype {
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        domain: $domain:path;
        // $(Variant => tag,)*  — exhaustive mapping
        $( $(#[$vmeta:meta])* $variant:ident => $tag:expr, )*
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize,
        )]
        pub struct $name(u8);

        impl $name {
            /// Stable numeric tag (canonical encoding için).
            pub fn as_u8(&self) -> u8 {
                self.0
            }

            /// Valid tag set'i — deserialize validation için.
            const VALID_TAGS: &'static [u8] = &[ $($tag),* ];

            /// Tag bu newtype için geçerli mi?
            fn is_valid_tag(tag: u8) -> bool {
                Self::VALID_TAGS.contains(&tag)
            }
        }

        impl TryFrom<&$domain> for $name {
            type Error = $crate::authorization::CanonicalizationError;

            fn try_from(value: &$domain) -> Result<Self, Self::Error> {
                // Exhaustive match — domain enum'a yeni varyant eklenirse compiler
                // bu match'i güncellemeye zorlar.
                let tag = match value {
                    $( $(#[$vmeta])* <$domain>::$variant => $tag, )*
                };
                Ok(Self(tag))
            }
        }

        /// **reviewer P1-1:** `TryFrom<u8>` — deserialize validation.
        /// Geçersiz tag (örn 255) reddedilir. Diskten yüklenen artifact korunur.
        impl TryFrom<u8> for $name {
            type Error = $crate::authorization::CanonicalizationError;

            fn try_from(tag: u8) -> Result<Self, Self::Error> {
                if Self::is_valid_tag(tag) {
                    Ok(Self(tag))
                } else {
                    Err($crate::authorization::CanonicalizationError::InvalidCanonicalTag {
                        type_name: stringify!($name),
                        tag,
                    })
                }
            }
        }

        /// **reviewer P1-1:** Custom Deserialize — `TryFrom<u8>` üzerinden.
        /// Derived `#[serde(transparent)]` geçersiz tag'lere izin veriyordu.
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let tag = u8::deserialize(deserializer)?;
                $name::try_from(tag).map_err(serde::de::Error::custom)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

canonical_tag_newtype! {
    /// Node kind canonical tag — `NodeKind`'ın 9 varyantının stable numeric temsili.
    pub struct CanonicalNodeKind;
    domain: crate::space::NodeKind;
    Module => 0,
    Concept => 1,
    Feature => 2,
    Bug => 3,
    Rule => 4,
    Agent => 5,
    Intent => 6,
    Claim => 7,
    Witness => 8,
}

canonical_tag_newtype! {
    /// Edge kind canonical tag — `EdgeKind`'ın 8 varyantının stable numeric temsili.
    pub struct CanonicalEdgeKind;
    domain: crate::space::EdgeKind;
    Imports => 0,
    Calls => 1,
    DependsOn => 2,
    PartOf => 3,
    DerivesFrom => 4,
    Witnesses => 5,
    Approves => 6,
    Violates => 7,
}

canonical_tag_newtype! {
    /// Node classification canonical tag — 9 varyant.
    pub struct CanonicalNodeClassification;
    domain: NodeClassification;
    Production => 0,
    Test => 1,
    Fixture => 2,
    Migration => 3,
    Config => 4,
    Script => 5,
    Generated => 6,
    Documentation => 7,
    Unknown => 8,
}

canonical_tag_newtype! {
    /// Node role canonical tag — 6 varyant.
    pub struct CanonicalNodeRole;
    domain: NodeRole;
    TypeSurface => 0,
    Core => 1,
    Adapter => 2,
    Utility => 3,
    Runtime => 4,
    Support => 5,
}

canonical_tag_newtype! {
    /// Predicate axis canonical tag — 8 varyant (5 raw + 2 derived + Custom).
    pub struct PredicateAxisTag;
    domain: PredicateAxis;
    Coupling => 0,
    Cohesion => 1,
    Instability => 2,
    Entropy => 3,
    WitnessDepth => 4,
    RiskScore => 5,
    MainSequenceDistance => 6,
    Custom => 7,
}

canonical_tag_newtype! {
    /// Comparison operator canonical tag — 6 varyant.
    pub struct ComparisonOpTag;
    domain: ComparisonOp;
    Lt => 0,
    Le => 1,
    Gt => 2,
    Ge => 3,
    Eq => 4,
    Ne => 5,
}

canonical_tag_newtype! {
    /// Metric source canonical tag — 5 varyant.
    ///
    /// **INV-T9 #70:** `Mixed=4` yalnız heterojen aggregation çıktısıdır. Authorization
    /// wire representation — coords katmanı `descriptor_id()` ayrı stable byte ID kullanır.
    pub struct CanonicalMetricSourceTag;
    domain: MetricSource;
    TreeSitter => 0,
    Scip => 1,
    Placeholder => 2,
    Heuristic => 3,
    Mixed => 4,
}

canonical_tag_newtype! {
    /// **INV-T9 Step 4b:** Vision source canonical tag — 5 varyant.
    /// `EffectiveVisionGateContext` için claim-specific vision provenance.
    pub struct CanonicalVisionSourceTag;
    domain: VisionSource;
    None => 0,
    GlobalDefault => 1,
    BuiltinRole => 2,
    RoleProfile => 3,
    UserLoaded => 4,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Predicate mode tag — PredicateMode enum (trajectory.rs) yerine u8 tag.
//
// PredicateMode: All / Any / Weighted. trajectory.rs'de enum olarak tanımlı.
// ═══════════════════════════════════════════════════════════════════════════════

canonical_tag_newtype! {
    /// Predicate birleştirme modu canonical tag.
    pub struct PredicateModeTag;
    domain: crate::trajectory::PredicateMode;
    All => 0,
    Any => 1,
    Weighted => 2,
}

// ═══════════════════════════════════════════════════════════════════════════════
// PredicateSetResultTag — INV-T9 #70 Commit 4b Faz 5 (review v8 P1-4)
//
// PredicateSetResult: Completed / SourceInsufficient / NotCompleted (trajectory.rs).
// PredicateGate değerlendirme sonucu — basis'te canonical predicate evaluation basis
// field'ı olarak taşınır. Pinned numeric tag (enum ordinal/serde adı DEĞİL).
// ═══════════════════════════════════════════════════════════════════════════════

canonical_tag_newtype! {
    /// **INV-T9 #70 Commit 4b Faz 5 (review v8 P1-4):** PredicateSetResult canonical tag.
    /// Pinned numeric — predicate evaluation sonucu (basis/restore validator'da kullanılır).
    pub struct PredicateSetResultTag;
    domain: crate::trajectory::PredicateSetResult;
    Completed => 0,
    SourceInsufficient => 1,
    NotCompleted => 2,
}

// ═══════════════════════════════════════════════════════════════════════════════
// WitnessIndependencePolicy — yeni enum (omega'dan türetilmez, Strict varsayılan).
//
// reviewer: witness independence policy canonical basis'e bağlı olmalı. Omega
// (WitnessSet) şu an independence taşımıyor — Strict varsayılan. Gelecekte omega
// genişletilirse TryFrom eklenir.
// ═══════════════════════════════════════════════════════════════════════════════

/// Witness independence policy — witness quorum hesabının independence kuralı.
///
/// OSP production politikası `Strict`'ir: aynı author + aynı source + aynı claim
/// triple'i dedup edilir (inv #1 + #2). `Loose` yalnız author dedup, `None` dedup yok.
///
/// **Not:** Bu enum yeni — omega (WitnessSet) henüz independence taşımıyor.
/// `CanonicalWitnessPolicy::try_from(omega)` Strict varsayılan kullanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct WitnessIndependencePolicyTag(u8);

impl WitnessIndependencePolicyTag {
    /// Strict (varsayılan production politikası) — OSP inv #1+#2 dedup.
    pub const STRICT: Self = Self(0);
    /// Loose — yalnız author dedup.
    pub const LOOSE: Self = Self(1);
    /// None — dedup yok (test/kalibrasyon).
    pub const NONE: Self = Self(2);

    /// Valid tag set'i — deserialize validation için.
    const VALID_TAGS: &'static [u8] = &[0, 1, 2];

    /// Stable numeric tag.
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl Default for WitnessIndependencePolicyTag {
    fn default() -> Self {
        Self::STRICT
    }
}

/// **reviewer P1-1:** `TryFrom<u8>` — WitnessIndependencePolicyTag için deserialize validation.
impl TryFrom<u8> for WitnessIndependencePolicyTag {
    type Error = crate::authorization::CanonicalizationError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        if Self::VALID_TAGS.contains(&tag) {
            Ok(Self(tag))
        } else {
            Err(
                crate::authorization::CanonicalizationError::InvalidCanonicalTag {
                    type_name: "WitnessIndependencePolicyTag",
                    tag,
                },
            )
        }
    }
}

/// **reviewer P1-1:** Custom Deserialize — `TryFrom<u8>` üzerinden (makro dışı newtype).
impl<'de> serde::Deserialize<'de> for WitnessIndependencePolicyTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let tag = u8::deserialize(deserializer)?;
        Self::try_from(tag).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for WitnessIndependencePolicyTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match *self {
            Self::STRICT => "strict",
            Self::LOOSE => "loose",
            Self::NONE => "none",
            _ => "unknown",
        };
        write!(f, "WitnessIndependencePolicyTag({name})")
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PredicateFailurePolicyTag — TaskPolicy.predicate_failure_policy için canonical tag.
//
// trajectory.rs'de PredicateFailurePolicy enum olarak tanımlı (StrictReject vb.).
// ═══════════════════════════════════════════════════════════════════════════════

canonical_tag_newtype! {
    /// Predicate failure policy canonical tag — predicate başarısız olduğunda
    /// mutation kararını belirler (Reject / AcceptImprovement / OperatorApproval).
    pub struct PredicateFailurePolicyTag;
    domain: crate::trajectory::PredicateFailurePolicy;
    StrictReject => 0,
    AcceptImprovement => 1,
    OperatorApproval => 2,
}

// ═══════════════════════════════════════════════════════════════════════════════
// INV-T9 #70 Faz 5 Adım 1 (P1-3) — Reverse projection: Tag → Domain
//
// Faz 5 restore evaluator'ın ihtiyaç duyduğu dört dönüşüm. `canonical_tag_newtype!`
// makrosuna blanket reverse From eklenmez — yalnızca bu dört tag'e yetki verilir.
// Diğer tag aileleri (NodeKind, EdgeKind, vb.) restore semantiği ortaya çıktığında
// ayrı değerlendirilir.
//
// **Neden `From` infallible?** Tag newtype construction/deserialization sınırında
// geçersiz `u8` reddeder (`TryFrom<u8>` + custom Deserialize → `VALID_TAGS` set
// dışı değer = `CanonicalizationError`). Domain enum varyant set'i ile tag set'i
// birebir örtüşür, bu yüzden reverse dönüşüm kayıpsız ve infallible'dır.
// `unreachable!` arm'ı `VALID_TAGS` invariant'ının bilinçliAssert'idir.
//
// **`MetricSource::Mixed`:** measured evidence'ında geçerli aggregation çıktısıdır
// (`aggregate_source`). Guards consumption-side (predicate evaluate, axis
// construction) — enum-construction-side DEĞİL. Reverse `From` `Mixed` üretebilir.
// ═══════════════════════════════════════════════════════════════════════════════

impl From<PredicateAxisTag> for PredicateAxis {
    fn from(tag: PredicateAxisTag) -> Self {
        match tag.as_u8() {
            0 => PredicateAxis::Coupling,
            1 => PredicateAxis::Cohesion,
            2 => PredicateAxis::Instability,
            3 => PredicateAxis::Entropy,
            4 => PredicateAxis::WitnessDepth,
            5 => PredicateAxis::RiskScore,
            6 => PredicateAxis::MainSequenceDistance,
            7 => PredicateAxis::Custom,
            _ => unreachable!("PredicateAxisTag VALID_TAGS invariant"),
        }
    }
}

impl From<ComparisonOpTag> for ComparisonOp {
    fn from(tag: ComparisonOpTag) -> Self {
        match tag.as_u8() {
            0 => ComparisonOp::Lt,
            1 => ComparisonOp::Le,
            2 => ComparisonOp::Gt,
            3 => ComparisonOp::Ge,
            4 => ComparisonOp::Eq,
            5 => ComparisonOp::Ne,
            _ => unreachable!("ComparisonOpTag VALID_TAGS invariant"),
        }
    }
}

impl From<CanonicalMetricSourceTag> for MetricSource {
    fn from(tag: CanonicalMetricSourceTag) -> Self {
        match tag.as_u8() {
            0 => MetricSource::TreeSitter,
            1 => MetricSource::Scip,
            2 => MetricSource::Placeholder,
            3 => MetricSource::Heuristic,
            4 => MetricSource::Mixed,
            _ => unreachable!("CanonicalMetricSourceTag VALID_TAGS invariant"),
        }
    }
}

impl From<PredicateModeTag> for crate::trajectory::PredicateMode {
    fn from(tag: PredicateModeTag) -> Self {
        match tag.as_u8() {
            0 => crate::trajectory::PredicateMode::All,
            1 => crate::trajectory::PredicateMode::Any,
            2 => crate::trajectory::PredicateMode::Weighted,
            _ => unreachable!("PredicateModeTag VALID_TAGS invariant"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{EdgeKind, NodeKind};

    #[test]
    fn canonical_node_kind_maps_all_domain_variants() {
        let cases = [
            (NodeKind::Module, 0u8),
            (NodeKind::Concept, 1),
            (NodeKind::Feature, 2),
            (NodeKind::Bug, 3),
            (NodeKind::Rule, 4),
            (NodeKind::Agent, 5),
            (NodeKind::Intent, 6),
            (NodeKind::Claim, 7),
            (NodeKind::Witness, 8),
        ];
        for (kind, expected) in cases {
            let tag = CanonicalNodeKind::try_from(&kind).unwrap();
            assert_eq!(
                tag.as_u8(),
                expected,
                "NodeKind::{kind:?} should map to {expected}"
            );
        }
    }

    #[test]
    fn canonical_edge_kind_maps_all_domain_variants() {
        let cases = [
            (EdgeKind::Imports, 0u8),
            (EdgeKind::Calls, 1),
            (EdgeKind::DependsOn, 2),
            (EdgeKind::PartOf, 3),
            (EdgeKind::DerivesFrom, 4),
            (EdgeKind::Witnesses, 5),
            (EdgeKind::Approves, 6),
            (EdgeKind::Violates, 7),
        ];
        for (kind, expected) in cases {
            let tag = CanonicalEdgeKind::try_from(&kind).unwrap();
            assert_eq!(
                tag.as_u8(),
                expected,
                "EdgeKind::{kind:?} should map to {expected}"
            );
        }
    }

    #[test]
    fn canonical_node_classification_maps_all_domain_variants() {
        let cases = [
            (NodeClassification::Production, 0u8),
            (NodeClassification::Test, 1),
            (NodeClassification::Fixture, 2),
            (NodeClassification::Migration, 3),
            (NodeClassification::Config, 4),
            (NodeClassification::Script, 5),
            (NodeClassification::Generated, 6),
            (NodeClassification::Documentation, 7),
            (NodeClassification::Unknown, 8),
        ];
        for (cls, expected) in cases {
            let tag = CanonicalNodeClassification::try_from(&cls).unwrap();
            assert_eq!(tag.as_u8(), expected);
        }
    }

    #[test]
    fn canonical_node_role_maps_all_domain_variants() {
        let cases = [
            (NodeRole::TypeSurface, 0u8),
            (NodeRole::Core, 1),
            (NodeRole::Adapter, 2),
            (NodeRole::Utility, 3),
            (NodeRole::Runtime, 4),
            (NodeRole::Support, 5),
        ];
        for (role, expected) in cases {
            let tag = CanonicalNodeRole::try_from(&role).unwrap();
            assert_eq!(tag.as_u8(), expected);
        }
    }

    #[test]
    fn predicate_axis_tag_maps_all_domain_variants() {
        let cases = [
            (PredicateAxis::Coupling, 0u8),
            (PredicateAxis::Cohesion, 1),
            (PredicateAxis::Instability, 2),
            (PredicateAxis::Entropy, 3),
            (PredicateAxis::WitnessDepth, 4),
            (PredicateAxis::RiskScore, 5),
            (PredicateAxis::MainSequenceDistance, 6),
            (PredicateAxis::Custom, 7),
        ];
        for (axis, expected) in cases {
            let tag = PredicateAxisTag::try_from(&axis).unwrap();
            assert_eq!(tag.as_u8(), expected);
        }
    }

    #[test]
    fn comparison_op_tag_maps_all_domain_variants() {
        let cases = [
            (ComparisonOp::Lt, 0u8),
            (ComparisonOp::Le, 1),
            (ComparisonOp::Gt, 2),
            (ComparisonOp::Ge, 3),
            (ComparisonOp::Eq, 4),
            (ComparisonOp::Ne, 5),
        ];
        for (op, expected) in cases {
            let tag = ComparisonOpTag::try_from(&op).unwrap();
            assert_eq!(tag.as_u8(), expected);
        }
    }

    #[test]
    fn canonical_metric_source_tag_maps_all_domain_variants() {
        let cases = [
            (MetricSource::TreeSitter, 0u8),
            (MetricSource::Scip, 1),
            (MetricSource::Placeholder, 2),
            (MetricSource::Heuristic, 3),
            (MetricSource::Mixed, 4),
        ];
        for (src, expected) in cases {
            let tag = CanonicalMetricSourceTag::try_from(&src).unwrap();
            assert_eq!(tag.as_u8(), expected);
        }
    }

    #[test]
    fn witness_independence_policy_tag_defaults_to_strict() {
        let tag = WitnessIndependencePolicyTag::default();
        assert_eq!(tag, WitnessIndependencePolicyTag::STRICT);
        assert_eq!(tag.as_u8(), 0);
    }

    #[test]
    fn canonical_tag_newtypes_serialize_as_transparent_u8() {
        let tag = CanonicalNodeKind::try_from(&NodeKind::Module).unwrap();
        let json = serde_json::to_string(&tag).unwrap();
        // transparent → raw u8
        assert_eq!(json, "0");
        let back: CanonicalNodeKind = serde_json::from_str("0").unwrap();
        assert_eq!(back, tag);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // INV-T9 #70 Faz 5 Adım 1 (P1-3) — PredicateMode forward + 4 round-trip
    //
    // Forward test canonical byte sözleşmesini sabitler; round-trip test reverse
    // projection tutarlılığını pinler. İkisi farklı şeyi kanıtlar: round-trip tek
    // başına simetrik hatayı (All↔Any karşılıklı değişimi) yakalayamaz.
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn predicate_mode_tag_maps_all_domain_variants() {
        // EKSİK forward test — Adım 1 ile aynı atomik değişiklikte tamamlanır.
        let cases = [
            (crate::trajectory::PredicateMode::All, 0u8),
            (crate::trajectory::PredicateMode::Any, 1),
            (crate::trajectory::PredicateMode::Weighted, 2),
        ];
        for (mode, expected) in cases {
            let tag = PredicateModeTag::try_from(&mode).unwrap();
            assert_eq!(tag.as_u8(), expected);
        }
    }

    #[test]
    fn predicate_axis_tag_round_trips_through_domain() {
        let cases = [
            PredicateAxis::Coupling,
            PredicateAxis::Cohesion,
            PredicateAxis::Instability,
            PredicateAxis::Entropy,
            PredicateAxis::WitnessDepth,
            PredicateAxis::RiskScore,
            PredicateAxis::MainSequenceDistance,
            PredicateAxis::Custom,
        ];
        for axis in cases {
            let tag = PredicateAxisTag::try_from(&axis).unwrap();
            let restored: PredicateAxis = tag.into();
            assert_eq!(restored, axis, "round-trip failed for {axis:?}");
        }
    }

    #[test]
    fn comparison_op_tag_round_trips_through_domain() {
        let cases = [
            ComparisonOp::Lt,
            ComparisonOp::Le,
            ComparisonOp::Gt,
            ComparisonOp::Ge,
            ComparisonOp::Eq,
            ComparisonOp::Ne,
        ];
        for op in cases {
            let tag = ComparisonOpTag::try_from(&op).unwrap();
            let restored: ComparisonOp = tag.into();
            assert_eq!(restored, op, "round-trip failed for {op:?}");
        }
    }

    #[test]
    fn canonical_metric_source_tag_round_trips_through_domain() {
        // Mixed dahil — measured evidence'ında geçerli aggregation çıktısıdır
        // (guards consumption-side, enum-construction-side DEĞİL).
        let cases = [
            MetricSource::TreeSitter,
            MetricSource::Scip,
            MetricSource::Placeholder,
            MetricSource::Heuristic,
            MetricSource::Mixed,
        ];
        for src in cases {
            let tag = CanonicalMetricSourceTag::try_from(&src).unwrap();
            let restored: MetricSource = tag.into();
            assert_eq!(restored, src, "round-trip failed for {src:?}");
        }
    }

    #[test]
    fn predicate_mode_tag_round_trips_through_domain() {
        let cases = [
            crate::trajectory::PredicateMode::All,
            crate::trajectory::PredicateMode::Any,
            crate::trajectory::PredicateMode::Weighted,
        ];
        for mode in cases {
            let tag = PredicateModeTag::try_from(&mode).unwrap();
            let restored: crate::trajectory::PredicateMode = tag.into();
            assert_eq!(restored, mode, "round-trip failed for {mode:?}");
        }
    }
}
