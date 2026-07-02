// INV-C8 compile-fail: external crate AnchorPlan literal construct edemez.
// Field'lar pub(crate) → harici `AnchorPlan { ... }` engelli.
// "AnchorPlan almak = canon gate'ten geçmiş olmak."
use osp_core::anchoring::types::{AnchorCandidate, AnchorPlan, ConceptPacketId};
use osp_core::anchoring::{AnchorDecisionKind, ThresholdBand};

fn main() {
    let _plan = AnchorPlan {
        packet_id: ConceptPacketId("pkt:x".into()),
        candidates: vec![],
        decision: AnchorDecisionKind::MarkUnanchored,
        threshold_band: ThresholdBand::Unanchored,
        requires_operator_review: false,
        negative_assertions: vec![],
        redirects: vec![],
    };
}
