//! #96 MD-2 W6 — sealed carrier + authority yüzeyi compile-fail (trybuild) suite.
//!
//! **P0-1/P0-2 regression fence'leri** (review tur-4/5 kapanışlarının pin'i):
//!
//! 1. `FinalizedNativeTaskClaim` external crate'ten elle KURULAMAZ:
//!    - constructor private (`fn new` — yalnız `finalize` üretir)
//!    - struct literal kapalı (private fields)
//! 2. `NativeLegacySubjectMeasurement::new_characterization_legacy` YOK
//!    (kaldırılan P0-1 yüzeyi — geri getirilirse bu test derleme hatası verir).
//! 3. `TaskCommitInput::new` yalnız sealed carrier kabul eder — "Claim B +
//!    token A" artifact-mix çağrı şekli type-level unrepresentable.
//!
//! Runtime karşılıkları: engine.rs unit test'leri (finalize subject-binding
//! negatifleri + verifier 5-check negatifleri) + integration native flow.

#[test]
fn md2_sealed_carrier_compile_fail_boundaries() {
    let t = trybuild::TestCases::new();
    // Carrier elle kurulamaz (ctor private).
    t.compile_fail("tests/compile_fail/md2_carrier_external_construct.rs");
    // Carrier struct literal ile kurulamaz (private fields).
    t.compile_fail("tests/compile_fail/md2_carrier_external_literal.rs");
    // Kaldırılan characterization ctor geri gelmez (P0-1 fence).
    t.compile_fail("tests/compile_fail/md2_characterization_ctor_removed.rs");
    // Artifact-mix çağrı şekli temsil edilemez (P0-2 fence).
    t.compile_fail("tests/compile_fail/md2_task_commit_input_artifact_mix.rs");
}
