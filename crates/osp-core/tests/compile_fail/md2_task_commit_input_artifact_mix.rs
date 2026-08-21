// #96 MD-2 W6 compile-fail (P0-2 pin): `TaskCommitInput::new` YALNIZ sealed
// `FinalizedNativeTaskClaim` carrier kabul eder. Eski "ayrı &Claim + ayrı &token"
// artifact-mix çağrı şekli (Claim B + token A saldırısı) type-level
// UNREPRESENTABLE — aşağıdaki iki çağrı da derlenmemeli:
//   (a) 6 argüman (eski imza — claim ve token ayrı)
//   (b) 5 argüman ama 1. argüman plain &Claim (carrier DEĞİL)
fn main() {
    let claim: osp_core::witness::Claim = unimplemented!();
    let omega: osp_core::witness::WitnessSet = unimplemented!();
    let resolver: &dyn osp_core::trajectory::TaskResolver = unimplemented!();
    let target = osp_core::coords::RawPosition::default();
    let token: osp_core::measurement::NativeLegacySubjectMeasurement = unimplemented!();
    // (a) Eski 6-argüman şekli derlenmemeli.
    let _ = osp_core::engine::TaskCommitInput::new(
        &claim,
        &omega,
        resolver,
        target,
        1.0,
        &token,
    );
    // (b) 5 argüman ama 1. arg plain &Claim — sealed carrier bekleniyor.
    let _ = osp_core::engine::TaskCommitInput::new(&claim, &omega, resolver, target, 1.0);
}
