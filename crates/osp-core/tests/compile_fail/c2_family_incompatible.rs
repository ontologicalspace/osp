// INV-C2 compile-fail: PhysicalCodeVector ve EvidenceVector farklı tipler.
// Compiler karıştırmayı reddeder — family separation type-level.
use osp_core::anchoring::types::{EvidenceVector, PhysicalCodeVector};

fn main() {
    // Bu satır derlenmemeli: EvidenceVector'ü PhysicalCodeVector'e atama (farklı family).
    let _ev: PhysicalCodeVector = EvidenceVector::new(0.9, 0.85, 0.7, 0.6, 0.95);
}
