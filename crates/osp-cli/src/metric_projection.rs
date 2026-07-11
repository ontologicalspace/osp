// Test modülü yazılana kadar dead-code (accessor/newtype production'da henüz read yok).
#![allow(dead_code)]

//! Analysis metric projection — axis-granular metric draft (NOT core evidence).
//!
//! Bu modül core'un tam 5-axis evidence tipini ÜRETMEZ (bkz. anchoring/types.rs);
//! yalnız analyzer ölçümlerini kabul edilen axis'lere projekte eder.
//! INV-C6: eksik veri tam veri gibi gösterilmez (entropy/witness_depth üretilmez).
//!
//! **GUARD KURALI (N1):** bu dosyada anchoring/types.rs'teki tam evidence/vector tip adları
//! yorumda bile geçmez — tests/architecture_guards.rs substring kontrolü. Dolaylama kullanılır.
//!
//! # C1 doğrulama sırası
//! value → confidence → coverage doğrulaması source admission'dan ÖNCE. Placeholder + NaN
//! sessizce skip edilmez → InvalidMetric error. Tutarlılık > kullanılabilirlik.

use std::collections::BTreeMap;

use osp_analyzer::contract::AnalysisResult;
use osp_core::anchoring::types::{ConceptNodeId, ObservedCodeMetricSource};
use osp_core::coords::MetricSource;
use osp_core::space::NodeId;

// ═══════════════════════════════════════════════════════════════════════════════
// Axis model
// ═══════════════════════════════════════════════════════════════════════════════

/// Physical code axis (5-axis core uzayı — Paper 1 sabit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PhysicalCodeAxis {
    Coupling,
    Cohesion,
    Instability,
    Entropy,
    WitnessDepth,
}

impl PhysicalCodeAxis {
    /// Deterministik sort sırası (enum declaration'a bağımlı değil).
    pub(crate) const fn stable_rank(self) -> u8 {
        match self {
            Self::Coupling => 0,
            Self::Cohesion => 1,
            Self::Instability => 2,
            Self::Entropy => 3,
            Self::WitnessDepth => 4,
        }
    }
}

/// 5-elemanlı sabit axis alanı bitset (BTreeSet/Ord gerektirmez, sıralama bağımlılığı yok).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AxisSet(u8);

impl AxisSet {
    const COUPLING: u8 = 1 << 0;
    const COHESION: u8 = 1 << 1;
    const INSTABILITY: u8 = 1 << 2;
    const ENTROPY: u8 = 1 << 3;
    const WITNESS_DEPTH: u8 = 1 << 4;

    pub(crate) const fn from_axes(axes: &[PhysicalCodeAxis]) -> Self {
        let mut bits = 0u8;
        let mut i = 0;
        while i < axes.len() {
            bits |= match axes[i] {
                PhysicalCodeAxis::Coupling => Self::COUPLING,
                PhysicalCodeAxis::Cohesion => Self::COHESION,
                PhysicalCodeAxis::Instability => Self::INSTABILITY,
                PhysicalCodeAxis::Entropy => Self::ENTROPY,
                PhysicalCodeAxis::WitnessDepth => Self::WITNESS_DEPTH,
            };
            i += 1;
        }
        Self(bits)
    }

    pub(crate) const fn contains(self, axis: PhysicalCodeAxis) -> bool {
        let flag = match axis {
            PhysicalCodeAxis::Coupling => Self::COUPLING,
            PhysicalCodeAxis::Cohesion => Self::COHESION,
            PhysicalCodeAxis::Instability => Self::INSTABILITY,
            PhysicalCodeAxis::Entropy => Self::ENTROPY,
            PhysicalCodeAxis::WitnessDepth => Self::WITNESS_DEPTH,
        };
        self.0 & flag != 0
    }

    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Validated scalar newtypes (C3 — type invariant, convention değil)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MetricAxisValue(f64);

impl MetricAxisValue {
    pub(crate) fn new(value: f64) -> Result<Self, MetricScalarViolation> {
        if !value.is_finite() {
            return Err(MetricScalarViolation::NonFinite);
        }
        if value < 0.0 {
            return Err(MetricScalarViolation::BelowMinimum);
        }
        if value > 1.0 {
            return Err(MetricScalarViolation::AboveMaximum);
        }
        Ok(Self(value))
    }
    pub(crate) const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MetricConfidence(f64);

impl MetricConfidence {
    pub(crate) fn new(value: f64) -> Result<Self, MetricScalarViolation> {
        if !value.is_finite() {
            return Err(MetricScalarViolation::NonFinite);
        }
        if value < 0.0 {
            return Err(MetricScalarViolation::BelowMinimum);
        }
        if value > 1.0 {
            return Err(MetricScalarViolation::AboveMaximum);
        }
        Ok(Self(value))
    }
    pub(crate) const fn get(self) -> f64 {
        self.0
    }
    pub(crate) const fn is_zero(self) -> bool {
        self.0 == 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MetricCoverage(f64);

impl MetricCoverage {
    pub(crate) fn new(value: f64) -> Result<Self, MetricScalarViolation> {
        if !value.is_finite() {
            return Err(MetricScalarViolation::NonFinite);
        }
        if value < 0.0 {
            return Err(MetricScalarViolation::BelowMinimum);
        }
        if value > 1.0 {
            return Err(MetricScalarViolation::AboveMaximum);
        }
        Ok(Self(value))
    }
    pub(crate) const fn get(self) -> f64 {
        self.0
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Projection model (private fields + accessors)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectedMetricProvenance {
    source: ObservedCodeMetricSource,
    confidence: MetricConfidence,
    coverage: MetricCoverage,
}

impl ProjectedMetricProvenance {
    pub(crate) fn source(&self) -> ObservedCodeMetricSource {
        self.source
    }
    pub(crate) fn confidence(&self) -> MetricConfidence {
        self.confidence
    }
    pub(crate) fn coverage(&self) -> MetricCoverage {
        self.coverage
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectedCodeMetric {
    node_id: ConceptNodeId,
    axis: PhysicalCodeAxis,
    value: MetricAxisValue,
    provenance: ProjectedMetricProvenance,
}

impl ProjectedCodeMetric {
    pub(crate) fn node_id(&self) -> &ConceptNodeId {
        &self.node_id
    }
    pub(crate) fn axis(&self) -> PhysicalCodeAxis {
        self.axis
    }
    pub(crate) fn value(&self) -> MetricAxisValue {
        self.value
    }
    pub(crate) fn provenance(&self) -> &ProjectedMetricProvenance {
        &self.provenance
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AnalysisMetricProjection {
    pub metrics: Vec<ProjectedCodeMetric>,
    pub report: MetricProjectionReport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MetricProjectionReport {
    pub module_nodes_seen: usize,
    pub input_axis_values: usize, // N6: == module_nodes_seen × 3 (başarılı projection)
    pub projected_axis_values: usize,
    pub skipped_placeholder: usize,
    pub skipped_heuristic: usize,
    pub skipped_zero_confidence: usize,
    pub analyzer_provided_axes: AxisSet,
    pub analyzer_unavailable_axes: AxisSet,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Source admission + error taksonomisi
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetricSourceRejection {
    PlaceholderSource,
    HeuristicNotAdmitted,
}

/// Exhaustive match → yeni MetricSource compile-time zorlanır.
fn admit_metric_source(source: MetricSource) -> Result<ObservedCodeMetricSource, MetricSourceRejection> {
    match source {
        MetricSource::TreeSitter => Ok(ObservedCodeMetricSource::TreeSitter),
        MetricSource::Scip => Ok(ObservedCodeMetricSource::Scip),
        MetricSource::Placeholder => Err(MetricSourceRejection::PlaceholderSource),
        MetricSource::Heuristic => Err(MetricSourceRejection::HeuristicNotAdmitted),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvalidMetricField {
    Value,
    Confidence,
    Coverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetricScalarViolation {
    NonFinite,
    BelowMinimum,
    AboveMaximum,
}

/// Metric projection error — bridge-level (sessiz skip YOK).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MetricProjectionError {
    #[error("missing projected identity for analysis node {analysis_node_id}")]
    MissingProjectedIdentity { analysis_node_id: NodeId },
    #[error("missing module metrics for analysis node {analysis_node_id}")]
    MissingModuleMetrics { analysis_node_id: NodeId },
    #[error("duplicate projected axis: node={node_id:?}, axis={axis:?}")]
    DuplicateProjectedAxis {
        node_id: ConceptNodeId,
        axis: PhysicalCodeAxis,
    },
    #[error("invalid metric: node={node_id:?}, axis={axis:?}, field={field:?}, violation={violation:?}")]
    InvalidMetric {
        node_id: ConceptNodeId,
        axis: PhysicalCodeAxis,
        field: InvalidMetricField,
        violation: MetricScalarViolation,
    },
}

// ═══════════════════════════════════════════════════════════════════════════════
// project_code_metrics — C1 doğrulama sırası
// ═══════════════════════════════════════════════════════════════════════════════

/// Analyzer ModuleMetrics → axis-granular metric draft projection.
/// Identity PR A'dan tüketilir (scheme/policy YOK — R1).
/// Yalnız coupling/cohesion/instability; entropy/witness_depth üretilmez (INV-C6).
pub(crate) fn project_code_metrics(
    analysis: &AnalysisResult,
    identity_index: &crate::analysis_bridge::AnalysisProjectionIndex,
) -> Result<AnalysisMetricProjection, MetricProjectionError> {
    // Analyzer capability: 3 axis sağlar, 2 sağlamaz.
    let analyzer_provided_axes =
        AxisSet::from_axes(&[PhysicalCodeAxis::Coupling, PhysicalCodeAxis::Cohesion, PhysicalCodeAxis::Instability]);
    let analyzer_unavailable_axes =
        AxisSet::from_axes(&[PhysicalCodeAxis::Entropy, PhysicalCodeAxis::WitnessDepth]);

    let mut metrics: Vec<ProjectedCodeMetric> = Vec::new();
    let mut report = MetricProjectionReport {
        analyzer_provided_axes,
        analyzer_unavailable_axes,
        ..Default::default()
    };

    // Deterministik node sırası.
    let mut node_ids: Vec<NodeId> = analysis.space.nodes.keys().copied().collect();
    node_ids.sort();
    // Yalnızca Module node'ları (PR A ile aynı filtre).
    let module_node_ids: Vec<NodeId> = node_ids
        .into_iter()
        .filter(|&id| analysis.space.nodes[&id].kind == osp_core::space::NodeKind::Module)
        .collect();

    // Duplicate (ConceptNodeId, axis) detection — String key (ConceptNodeId Ord değil).
    let mut seen: BTreeMap<(String, u8), ()> = BTreeMap::new();

    for node_id in &module_node_ids {
        report.module_nodes_seen += 1;

        // Identity — PR A'dan tüket (R1).
        let concept_id = identity_index
            .concept_node_id_for(*node_id)
            .ok_or(MetricProjectionError::MissingProjectedIdentity {
                analysis_node_id: *node_id,
            })?;

        // Module metrics — MissingModuleMetrics typed error.
        let module_metrics = analysis.module_metrics.get(node_id).ok_or(
            MetricProjectionError::MissingModuleMetrics {
                analysis_node_id: *node_id,
            },
        )?;

        // 3 axis: coupling, cohesion, instability.
        for (axis, metric_value) in [
            (PhysicalCodeAxis::Coupling, &module_metrics.coupling),
            (PhysicalCodeAxis::Cohesion, &module_metrics.cohesion),
            (PhysicalCodeAxis::Instability, &module_metrics.instability),
        ] {
            report.input_axis_values += 1;

            // C1: doğrulama ÖNCE (value → confidence → coverage), source admission'dan önce.
            let value = MetricAxisValue::new(metric_value.value).map_err(|v| {
                MetricProjectionError::InvalidMetric {
                    node_id: concept_id.clone(),
                    axis,
                    field: InvalidMetricField::Value,
                    violation: v,
                }
            })?;
            let confidence = MetricConfidence::new(metric_value.confidence).map_err(|v| {
                MetricProjectionError::InvalidMetric {
                    node_id: concept_id.clone(),
                    axis,
                    field: InvalidMetricField::Confidence,
                    violation: v,
                }
            })?;
            let coverage = MetricCoverage::new(metric_value.coverage).map_err(|v| {
                MetricProjectionError::InvalidMetric {
                    node_id: concept_id.clone(),
                    axis,
                    field: InvalidMetricField::Coverage,
                    violation: v,
                }
            })?;

            // Source admission (Placeholder/Heuristic skip — N4: Heuristic defensive policy).
            let source = match admit_metric_source(metric_value.source) {
                Ok(s) => s,
                Err(MetricSourceRejection::PlaceholderSource) => {
                    report.skipped_placeholder += 1;
                    continue;
                }
                Err(MetricSourceRejection::HeuristicNotAdmitted) => {
                    report.skipped_heuristic += 1;
                    continue;
                }
            };

            // Zero-confidence omission (MetricConfidence::new Ok ama is_zero).
            if confidence.is_zero() {
                report.skipped_zero_confidence += 1;
                continue;
            }

            // Duplicate (ConceptNodeId, axis) — many-to-one collision (N7 plan #7).
            let dedup_key = (concept_id.0.clone(), axis.stable_rank());
            if !seen.insert(dedup_key, ()).is_none() {
                return Err(MetricProjectionError::DuplicateProjectedAxis {
                    node_id: concept_id.clone(),
                    axis,
                });
            }

            metrics.push(ProjectedCodeMetric {
                node_id: concept_id.clone(),
                axis,
                value,
                provenance: ProjectedMetricProvenance {
                    source,
                    confidence,
                    coverage,
                },
            });
            report.projected_axis_values += 1;
        }
    }

    // Deterministik sort: (node_id, axis.stable_rank).
    metrics.sort_by_key(|m| (m.node_id.0.clone(), m.axis.stable_rank()));

    Ok(AnalysisMetricProjection { metrics, report })
}
