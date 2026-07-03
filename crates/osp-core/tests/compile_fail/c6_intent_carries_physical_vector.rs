// INV-C6 + INV-C2 compile-fail: ObservedCodeEvidence PhysicalCodeVector bekler.
// ConceptualIntentVector (intent) observed code evidence'ına verilemez — type mismatch.
// "Kod metric'leri (PhysicalCode) ile niyet (ConceptualIntent) karıştırılamaz."
use osp_core::anchoring::types::{
    ConceptualIntentVector, EvidenceStrength, ObservedCodeEvidence, ObservedCodeMetricSource,
};
use osp_core::anchoring::ConceptNodeId;

fn main() {
    let _evidence = ObservedCodeEvidence::new(
        ConceptNodeId("Concept:Auth".into()),
        // HATA: PhysicalCodeVector bekleniyor, ConceptualIntentVector verilmiş.
        // INV-C6 (kod metric ≠ niyet) + INV-C2 (family ayrımı) birleşimi.
        ConceptualIntentVector::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5),
        ObservedCodeMetricSource::Scip,
        EvidenceStrength::one(),
        0,
    );
}
