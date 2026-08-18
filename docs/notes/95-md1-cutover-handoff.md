# Handoff — #95 MD-1 Faz 8a (Caller Cutover) — P2-1 MERGED sonrası

**Tarih:** 2026-08-18
**Önceki oturumlar:** plan v1→v6 (5 review turu) → PR #122 (P2-1) → 3 EK PR review turu → merge `be79675`
**Bu dosya:** Yeni oturumda #95 Faz 8a caller cutover ile devam etmek için tüm bağlam.
(P2-1 öncesi dönem notları tarihçe için git geçmişinde: `354f85d`'teki ilk sürüm.)

## Oturum nasıl başlamalı

Kullanıcı bu mesajı iletecek:

> Handoff: #95 MD-1 **Faz 8a** (caller cutover) ile devam ediyoruz.
> Notlar: `docs/notes/95-md1-cutover-handoff.md` — önce onu oku, durum kontrolü yap, plan moduna gir.

**İlk adımlar:** (1) bu dosyayı oku, (2) `git status` + `git log --oneline -5` (main `be79675`+
olmalı), (3) `gh issue view 95` (OPEN — Faz 8a) + `gh issue view 96` (sıradaki),
(4) plan moduna girip **Faz 8a cutover planını** hazırla (v6 §9 önizlemesi aşağıda).

## Mevcut durum (bu oturumun sonunda)

- **P2-1 MERGED** — PR #122 (`be79675`, squash): `subject_authority.rs` + navigator/MCP
  wiring + wire sidecar'ları + 15+ test. 6 review turu geçti (5 plan + 3 EK PR).
- **#95 OPEN** — yalnız Faz 8a kaldı. **#99'e durum yorumu yazıldı** (MD-1 kısımları bitti;
  `measure_task_delta_checked` Faz 8a'ya, MD-2 dual evaluation #96'ya devrolmalı).
- **Kritik yol:** #95-B (Faz 8a) → #96 (MD-2) → #97 (MD-3) → #100 (engine cutover, BLOCKED).
- **#110 MSRV** — ara iş, istenilen noktaya sıkıştırılabilir.

## Dogfood gözlem penceresi kanıtı (2026-08-18, mini)

Gerçek CLI akışı (`osp trajectory attempt`, gerçek analyze + navigator, mock LLM):

**Run A — Completed (harness + auto-approve):** evidence kaydında `subject_authority_drift`
sidecar canlı görüldü:

```text
v1: subject [2], sources [Scip×5] (compatibility projection), Q5 passed,
    downstream Observed{Completed, AcceptAsCompleted}   ← authoritative, gerçek
v2: subject [2] (digest PARITY), sources engine-native
    [TreeSitter, Placeholder, TreeSitter, Heuristic, Heuristic],
    Q5 passed (aynı theta bits), downstream NotCompleted/Reject
    (SourceInsufficient — required_source=Scip karşılanamıyor)
```

**Bu, gerçek üretim akışında canlı MD-2 confound kanıtıdır:** subject ve θ parity
olmasına rağmen downstream V1/V2 arasında diverge ediyor — neden provenance (MD-2),
subject authority DEĞİL. Telemetri farkı doğru atfediyor. **Faz 8a planının en kritik
girdisi budur:** cutover `required_source`'lu task'larda karar yüzeyini değiştirir;
bu senaryo #96 (MD-2) ile koordinasyonu ve Case 2/3 regolden reason-note'larını
zorunlu kılar.

**Run B — Held (production witness): BAŞARISIZ, tasarım gereği:** CLI
`FilesystemPendingAuthorizationStore` (CrossProcess) + engine hâlâ `Ephemeral`
space identity üretiyor (persisted identity lifecycle "Commit 4" — engine.rs:2283;
INV-T9 #72 D3 kuralı fail-closed). **CLI'de Held yüzeyi bugün structurally kapalı**
(bizim regresyonumuz değil; completed_loop'da da Held e2e testi yok). Held sidecar
kapsamı: navigator unit testi (`md1_held_pending_authorization_…`) + MCP e2e
(`md1_held_response_carries_drift_sidecar_additively`). Faz 8a planında bu kısıt
not edilmeli (cutover Held testlemesi bu kısıt altında kalır).

## P2-1'den devralınan varlıklar (Faz 8a'da kullanılacak/kaldırılacak)

1. **`crates/osp-core/src/subject_authority.rs`** — MD-1 compatibility semantics'in tek evi:
   - `derive_v1_legacy_measurement_subject` (ordered union; dual pinning)
   - `produce_legacy_subject_measurement` → `LegacySubjectMeasurement` (private fields,
     tek üretici) — **Faz 8a'da kaldırma hedefi**
   - `legacy_compatibility_projection` (uniform-Scip; navigator bağımlılığı yok)
   - `observe_subject_authority_drift` → `SubjectAuthorityDriftObservationDraft`
     (private alanlar + accessor'lar + Clone'suz; consuming `finalize`) → final
     `SubjectAuthorityDriftObservation` (serde'li, untrusted telemetry DTO)
   - `v1_downstream_from_engine_commit_error` — 14 `EngineCommitError` varyantı explicit
     (surviving: Syntax/Vision/RuleViolation; diğerleri None)
   - Eligibility contract: comparison-surviving (Evaluated/Held/Rejected/retryable
     Q4-Q6) → emit; TaskValidation/VisionContextInvalid/SystemFailure → yok
2. **Taşıyıcılar:** `TrajectoryEvidence.subject_authority_drift`
   (`#[serde(default)]`, untrusted/outbound telemetry — task identity kontrolü
   consumer'a bırakılmış, doc'ta), `PendingAuthorization.subject_authority_drift`
   (validate_internal identity-bound), `RevisionRequired.try_with_subject_authority_drift`
   (checked builder). Digest preimage'lerine hiçbiri girmez.
3. **Test envanteri:** `tests/subject_authority_drift_observation.rs` (8 — 002 θ
   cross-pin'leri #92 golden'larıyla bit-exact), navigator md1 testleri (2),
   authorization wire/identity testleri (3), MCP sidecar e2e (3: Held/Q4/Q6),
   engine unit axis TCB (2), modül unit (8). Corpus'a case EKLENMEDİ.
4. **Plan v6** — `docs/notes/faz8-p2-migration-decisions.md` "P2-1 implementation"
   section'ı + invariants INV-T3 note güncel.

## Faz 8a planı için kapsam önizlemesi (plan v6 §9 — detay yeni plan turunda)

- Caller cutover: navigator/MCP ölçümü `measure_task_delta` tek authority'ya kesilir
  (V1 producer + `compute_raw_from_delta` legacy yolu kaldırılır).
- `affected_nodes` → impact hint/telemetry (measurement subject DEĞİL).
- Compatibility yüzeyi kaldırma: producer'lar + `compute_raw_from_delta`/
  `try_compute_raw_from_delta` + gözlemin V1 lane'i (#100 listesiyle hizala).
- Case 2/3 expected semantic-change regolden — **eski golden + yeni golden + reason
  note**; dogfood'taki MD-2 confound senaryosu reason-note malzemesi.
- LLM prompt güncellemesi (affected_nodes advisory hint dili) + downstream
  Q5/predicate güncellemeleri.
- **#99'un `measure_task_delta_checked` public boundary'i bu plana devrolmalı.**
- Dikkat: CLI Held/D3 kısıtı (yukarıda) + `#92` drift sınıflandırması
  (NoDrift/SourceLabelOnly/PredicateResultDrift/PolicyDecisionDrift/MutationDecisionDrift).

## Plan/PR dersleri (birikmiş — yeni turlarda uygula)

- **Conventional-commit + issue ref TUZAĞI (bu oturumda yakalandı):** `fix(core,mcp): #95 …`
  squash merge'de GitHub tarafından `fix #95` closing keyword'ü yorumlandı → #95
  yanlışlıkla kapandı (yeniden açıldı). **Kural:** issue referansından önce scope
  parantezi KULLANMA — `feat: #95 …` güvenli, `feat(core): #95 …` değil.
- Plan disiplini: 3-5 review turu normal; "production SEMANTIC değişikliği yok" de,
  refactor'ları işaretle; elle fixture sabiti YAZMA (probe-then-freeze); exact
  snapshot dondur; `Option` sınıflandırması yalnız ulaşilan yüzeyde; enum equality
  (debug-string değil); doğrulama komutlarında `||`/`2>/dev/null` YOK.
- Corpus'a case eklerken ID-bazlı `case_by_id`; class find DEĞİL.
- Sentinel testleri: corpus'un `direct-*` case'lerinde role-bearing node yok → Q5
  NotEvaluated → downstream quadruple erişilemez; inline fixture gerekçesiyle
  yazıldı (aynı gerekçe Faz 8a testlerinde de geçerli olabilir).
- GitHub PR yazarı kendi PR'ını onaylayamaz — onay metni PR yorumu olarak kaydedildi.
- `index.scip` (16.4 MB) repo kökünde lokal; yeniden üretim: rust-toolchain.toml'u
  geçici taşı + `MSYS_NO_PATHCONV=1 docker run --rm -v "P:/Work/SoftwarePhysics:/repo"
  -w /repo sourcegraph/scip-rust:latest scip-rust --output /repo/index.scip`.

## Ortam notları (Windows)

- Shell her sıfırlanmada `export PATH="$HOME/.cargo/bin:$PATH"` gerekebilir.
- `grep` ZCode function'ı — `command grep` kullan.
- `python` yok; JSON için `node -e` (temp path'ler `C:/Users/ervol/...` formatında).
- **ASLA `git add -A`** — kullanıcı untracked kişisel notları var
  (`docs/notes/planlama-tasarım-eskiz.txt`, `proje-adaylari.md`, `sohbet-konu.txt`,
  `docs/osp-*.md`, `docs/design/`, `dump.scip`, `crates/osp-desktop/gen/`).
- Dogfood temp fixture: `C:/Users/ervol/AppData/Local/Temp/osp-md1-dogfood/`
  (silinmeye hazır; task/proposal/repo şablonları `crates/osp-cli/tests/completed_loop.rs`
  mirror'idir).

## Sıradaki iş önerisi

1. **#95 Faz 8a caller cutover** — bu handoff ile yeni plandan başlanmalı (yukarıdaki
   önizleme + #100 scope'u ile hizala).
2. Sonra #96 (MD-2) — dogfood MD-2 confound kanıtı bu işin characterization input'u.
3. Ara iş: #110 MSRV.
