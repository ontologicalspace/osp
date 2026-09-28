// #96 MD-2 W6 compile-fail (P0-2 pin): `TaskCommitInput::new` YALNIZ sealed
// `FinalizedNativeTaskClaim` carrier kabul eder. Eski "ayrı &Claim + ayrı &token"
// artifact-mix çağrı şekli (Claim B + token A saldırısı) type-level
// UNREPRESENTABLE — aşağıdaki iki çağrı da derlenmemeli:
//   (a) eski çok-argümanlı şekil (claim ve token ayrı)
//   (b) doğru arity ama 1. argüman plain &Claim (carrier DEĞİL)
// **#100 Faz 8a:** imza 3 argümana daraldı (target/loss_before kalktı) —
// artifact-mix pin'i korunur; loss skaler argümanları artık hiç yok.
fn main() {
    let claim: osp_core::witness::Claim = unimplemented!();
    let omega: osp_core::witness::WitnessSet = unimplemented!();
    let resolver: &dyn osp_core::trajectory::TaskResolver = unimplemented!();
    let token: osp_core::measurement::NativeSubjectMeasurement = unimplemented!();
    // (a) Eski çok-argümanlı şekil derlenmemeli (artifact mix).
    let _ = osp_core::engine::TaskCommitInput::new(&claim, &omega, resolver, &token);
    // (b) Doğru arity ama 1. arg plain &Claim — sealed carrier bekleniyor.
    let _ = osp_core::engine::TaskCommitInput::new(&claim, &omega, resolver);
}
