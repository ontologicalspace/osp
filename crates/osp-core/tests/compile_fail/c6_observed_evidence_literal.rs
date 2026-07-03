// INV-C6 compile-fail: external crate ObservedCodeEvidence literal construct edemez.
// Field'lar private → harici `ObservedCodeEvidence { ... }` engelli.
// Sadece public smart constructor `new(...)` ile üretilebilir (Patch 1).
// "Observed evidence almak = smart constructor'dan geçmiş olmak."
use osp_core::anchoring::types::{
    EvidenceStrength, ObservedCodeEvidence, ObservedCodeMetricSource, PhysicalCodeVector,
};
use osp_core::anchoring::ConceptNodeId;

fn main() {
    // Bu satır derlenmemeli: field'lar private.
    let _evidence = ObservedCodeEvidence {
        code_entity_id: ConceptNodeId("CodeEntity:X".into()),
        physical_vector: PhysicalCodeVector::new(0.1, 0.2, 0.3, 0.4, 1.0),
        metric_source: ObservedCodeMetricSource::Scip,
        confidence: EvidenceStrength::one(),
        measured_at: 0,
    };
}
