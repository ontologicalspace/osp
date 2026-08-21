// #96 MD-2 W6 compile-fail (P0-1 pin): `new_characterization_legacy` TAMAMEN
// kaldırıldı (eski doc-hidden pub ctor — production'da authority tipini formlama
// yüzeyi idi). External crate bu metodu ADIYLA çağıramaz; native authority
// üretiminin tek yolu engine producer + finalize.
//
// Bu pin geri getirilirse (ör. #100 öncesi geçici characterization yüzeyi)
// derleme HATASI verir — bilinçli regression fence.
fn main() {
    let measured: osp_core::coords::MeasuredRawPosition = unimplemented!();
    let ids: Vec<u64> = unimplemented!();
    let delta: osp_core::measurement::MeasurementDeltaDigest = unimplemented!();
    let revision: osp_core::authorization::SpaceViewRevision = unimplemented!();
    let input: osp_core::authorization::MeasurementInputDigest = unimplemented!();
    let stamp: osp_core::coords::CoreAxisEpochStamp = unimplemented!();
    // Bu satır derlenmemeli: metod yok (kaldırıldı).
    let _ = osp_core::measurement::NativeLegacySubjectMeasurement::new_characterization_legacy(
        measured,
        ids,
        delta,
        revision,
        input,
        stamp,
    );
}
