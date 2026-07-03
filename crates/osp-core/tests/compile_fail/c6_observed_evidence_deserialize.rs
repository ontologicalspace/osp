// INV-C6 compile-fail (serde boundary): ObservedCodeEvidence Deserialize YOK.
// Private field'lar serde ile reconstruct edilemesin (observed evidence bypass engeli).
// Serialize-only (audit için); trusted restore PR30 paternini izler (AnchorPlan/ConceptGraph).
// "Diskten evidence reconstruct edip INV-C6 boundary'yi bypass etmek imkansız."
use osp_core::anchoring::ObservedCodeEvidence;

fn main() {
    // Bu satır derlenmemeli: ObservedCodeEvidence Deserialize impl'i yok.
    let _evidence: ObservedCodeEvidence = serde_json::from_str("{}").unwrap();
}
