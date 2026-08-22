// #96 MD-2 W6 (review tur-5/6) compile-fail: external crate `FinalizedNativeTaskClaim`
// elle KURAMAZ. Constructor private (`fn new` — yalnız `finalize` aynı modülde
// çağırır; "sealed" iddiası crate içinde de gerçek — review tur-5 P2).
//
// Non-forgeability evidence katmanları:
// - Module privacy (yalnız finalize üretir): BU test (external erişim engeli).
// - finalize subject-binding negatifleri: engine.rs unit test'leri
//   (md2_finalize_rejects_same_delta_different_affected_nodes_token_mix +
//   md2_finalize_rejects_same_raw_bits_different_legacy_subject).
fn main() {
    let claim: osp_core::witness::Claim = unimplemented!();
    let measured: osp_core::coords::MeasuredRawPosition = unimplemented!();
    let ids: Vec<u64> = unimplemented!();
    let delta: osp_core::measurement::MeasurementDeltaDigest = unimplemented!();
    let revision: osp_core::authorization::SpaceViewRevision = unimplemented!();
    let input: osp_core::authorization::MeasurementInputDigest = unimplemented!();
    // Bu satır derlenmemeli: `new` private — carrier yalnız finalize ürünü.
    let _ = osp_core::task_measurement::FinalizedNativeTaskClaim::new(
        claim,
        measured,
        ids,
        delta,
        revision,
        input,
    );
}
