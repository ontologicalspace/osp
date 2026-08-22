// #96 MD-2 W6 (review tur-7 P1) compile-fail: MEVCUT authority token ctor
// `NativeLegacySubjectMeasurement::new` `pub(crate)` — external crate token
// MINT EDEMEZ (caller-supplied MeasuredRawPosition/revision/digest/epoch ile).
//
// Bu fence kritik: `pub(crate)` → `pub` genişletilirse eski P0-1 forge yüzeyi
// geri açılır ve DİĞER dört fixture yeşil kalırdı (new_characterization_legacy
// hâlâ yoktur) — yalnız BU test yakalar.
fn main() {
    let measured: osp_core::coords::MeasuredRawPosition = unimplemented!();
    let ids: Vec<u64> = unimplemented!();
    let delta: osp_core::measurement::MeasurementDeltaDigest = unimplemented!();
    let revision: osp_core::authorization::SpaceViewRevision = unimplemented!();
    let input: osp_core::authorization::MeasurementInputDigest = unimplemented!();
    let stamp: osp_core::coords::CoreAxisEpochStamp = unimplemented!();
    // Bu satır derlenmemeli: `new` pub(crate) — tek üretici engine producer.
    let _ = osp_core::measurement::NativeLegacySubjectMeasurement::new(
        measured,
        ids,
        delta,
        revision,
        input,
        stamp,
    );
}
