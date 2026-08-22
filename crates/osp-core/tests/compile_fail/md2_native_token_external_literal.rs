// #96 MD-2 W6 (review tur-7 P1) compile-fail: authority token STRUCT LITERAL
// ile external kurulamaz — private fields ("private fields + tek engine
// producer" garantisi; ctor private olmasa bile literal yolu kapalı).
fn main() {
    let measured: osp_core::coords::MeasuredRawPosition = unimplemented!();
    let ids: Vec<u64> = unimplemented!();
    let binding: osp_core::measurement::LegacySubjectBindingDigest = unimplemented!();
    let delta: osp_core::measurement::MeasurementDeltaDigest = unimplemented!();
    let revision: osp_core::authorization::SpaceViewRevision = unimplemented!();
    let input: osp_core::authorization::MeasurementInputDigest = unimplemented!();
    let stamp: osp_core::coords::CoreAxisEpochStamp = unimplemented!();
    // Bu satır derlenmemeli: tüm field'lar private.
    let _token = osp_core::measurement::NativeLegacySubjectMeasurement {
        measured,
        legacy_subject_ids: ids,
        legacy_subject_binding: binding,
        delta_digest: delta,
        base_revision: revision,
        measurement_input_digest: input,
        axis_epoch_stamp: stamp,
    };
}
