// #96 MD-2 W6 compile-fail: external crate `FinalizedNativeTaskClaim` STRUCT
// LITERAL ile kuramaz — private fields. (Constructor private olmasa bile literal
// yolu kapalı; iki katman birlikte "sealed".)
fn main() {
    let claim: osp_core::witness::Claim = unimplemented!();
    let measurement: osp_core::measurement::NativeLegacySubjectMeasurement = unimplemented!();
    // Bu satır derlenmemeli: field'lar private.
    let _carrier = osp_core::task_measurement::FinalizedNativeTaskClaim {
        claim,
        measurement,
    };
}
