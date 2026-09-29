# Dogfood Run D — Cold-Start Onay Akışı Canlı Uçtan Uca Kanıtı

**Tarih:** 2026-09-29. Branch `feat/run-d-cold-start` (main `2fd3854` üzerinden — #100
Faz 8a engine cutover merge SONRASI). Başlangıç yeşil: fmt + clippy `-D warnings` +
40 suite / 0 fail (main üzerinde doğrulandı).

**Amaç:** #97 MD-3 cold-start onay akışını (`ColdStartPolicy::RequireOperatorApproval`)
**#100 V2 engine cutover zinciri üzerinde** canlı binary ile uçtan uca koşmak — Run A/B/C
kalıbı (gerçek analyze + gerçek motor, disposable Temp fixture, envelope kanıt kayıtları).
MCP contract test'leri (`crates/osp-mcp/tests/md3_cold_start_approval.rs`) zinciri in-process
kanıtlıyordu; Run D aynı zinciri **canlı süreç + canlı JSON-RPC** ile tekrarlar.

## Çalıştırma aracı seçimi (tasarım kararı)

- **Canlı araç: `osp-mcp` binary** (`--mode operator --llm mock`), rmcp 0.8 stdio
  transport, MCP JSON-RPC ile sürülür (Node.js sürücü — Temp, repoya commit edilmez;
  tam kaynak bu dosyada). Operator mode tek süreçte tüm zinciri taşır: task_add
  (operator-only) → submit_delta (agent yüzeyi) → approve_cold_start (operator-only).
- **CLI (`osp trajectory attempt`) yüzeyi yapısal KAPALI** (Run B emsali — Held yüzeyi
  orada da kapalıydı): harness task formatı (`harness_task.rs`)
  `validate_scope_binding_set_equality` ile task predicate Node set ≡ `scope_bindings`
  Node set ≡ analyze `node_paths` (base node'lar) eşitliği zorunlu kılar; cold-start
  task'ının predicate scope'u **base'te olmayan** delta-introduced node'a bağlıdır
  (10_000+). Bu yüzden CLI run-envelope üretimi cold-start için temsil edilemez;
  kanıt yüzeyi MCP envelope'larıdır. `CliRunResultKind::AwaitingColdStartApproval` +
  exit 14 map'i birim testlerle pinli yaşamaya devam eder.

## Planlanan zincir / hedef kanıtlar

1. **Task ekle** — `osp_task_add` (operator, INV-T2): id 7, Coupling ≤ −1.0 scope
   `Node(10_000)` (base'te yok → delta ile girer → daima NotCompleted),
   `preferred_vector: Some` (**#100 sonrası dikkat (a)**: V2 loss hedefi task
   snapshot'ından derive edilir — NoPreferredVector→Reject fail-closed dalı değil,
   gerçek derive dalı egzersiz edilir), policy `AcceptImprovement` +
   `RequireOperatorApproval` + `allow_progress_checkpoint: true`.
2. **Askı ×2** — `osp_submit_delta` (tek `new_nodes[Module]`; allocator sözleşmesi
   10_000+): her ikisi `SuspendedColdStart` döner (`commit_state: cold_start_suspended`,
   `mainline_mutation: not_applied`, `next_action: operator_approval`) → amaç: iki
   bağımsız askı (claim A + B).
3. **Onay (A)** — `osp_approve_cold_start` (operator otoritesi): `AcceptAsColdStart`
   + `apply_target: Sandbox` + evidence 8 alan + `improvement_claimed: false` (INV-T6)
   + `mainline_promotion: not_available` (INV-T8). Bu onay uzayı ilerletir (t_c +1).
4. **Planlanan fakat canlıda ulaşılamayan adım — stale fail-closed (B).**
   İki eşzamanlı suspension gerektirir: A onaylanıp uzay ilerleyince B'nin onayı
   `stale_binding` (#96 5-fence replay) ile fail-closed düşmeli. MCP claim_id
   çakışması (**F1 → #133**) nedeniyle Run D sırasında canlıda
   gerçekleştirilemedi; davranış motor-seviyesinde test-pinned
   (`md3_approve_cold_start_stale_space_fails_closed`).
5. **Tek kullanım (A')** — tüketilmiş askının ikinci onayı: `unknown_suspension`.
6. **INV-T2 canlı pin** — agent-mode süreçte `osp_approve_cold_start` →
   `OperatorCapabilityRequired` (operator tool çağrılamaz; #131'deki per-tool pin
   adayının canlı karşılığı).

> **Not (planned vs observed evidence):** Adım 4 canlıda F1 (#133) nedeniyle
> gerçekleştirilemedi (canlı gözlem: `unknown_suspension` — askı kaydı zaten
> ezilmiş/tüketilmişti). Gerçek gözlenen zincir aşağıdaki "Kanıt" bölümünde
> verilmiştir; plan ile gözlem ayrı tutulur. **(#133 sonrası bu adım canlı
> koştu — "Ek" bölümüne bakınız.)**

## Fixture

`C:/Users/ervol/AppData/Local/Temp/osp-97-rund/` (disposable; Run C kalıbı):
`main.py` + `utils.py` (contract test fixture içeriğiyle aynı). Eski dogfood Temp
fixture'larındaki durable kayıtlar #95-B/#100 bilinçli epoch'ları nedeniyle okunamaz —
fixture'lar her run'da yeniden üretilir (bilinçli epoch kararlarının kabul koşulu buydu).

## Kanıt (canlı çalıştırma — 2026-09-29)

**Çalıştırma:** `osp-mcp` (main `2fd3854`, `cargo build -p osp-mcp` debug) — rmcp 0.8
stdio JSON-RPC (protocol `2025-06-18` negotiate). Sürücü: Node.js (`rund-driver.mjs` +
`rund-post-approve-probe.mjs`, Temp — repoya commit edilmez; payload'lar bu bölümde
tam olarak verilidir). Oturum 1: operator mode zinciri; oturum 2: onay-sonrası sonda;
oturum 3: agent mode INV-T2 reddi. Kanıt dosyaları `C:/Users/ervol/AppData/Local/Temp/
osp-97-rund/evidence/rund-*.json` (disposable; kritik alanlar aşağıda donduruldu).

**Tool registry (operator mode, `rund-00-tools-list.json`):** 9 tool — `osp_trajectory_init`,
`osp_task_add`, `osp_check_predicate`, `osp_analyze_workspace`, `osp_get_agent_task_view`,
`osp_approve_cold_start`, `osp_submit_delta`, `osp_get_attempt_history`, `osp_run_task`.

### 1. Askı — `osp_submit_delta` (oturum 1, task 7, tek `new_nodes[Module]`)

```json
{ "ok": true, "schema_version": "osp.mcp.v1", "tool": "osp_submit_delta",
  "result": {
    "commit_result": "SuspendedColdStart",
    "task_id": 7, "claim_id": 1,
    "baseline_unavailable_reason": "AllMembersIntroducedByDelta { members: [10000] }",
    "commit_state": "cold_start_suspended",
    "mainline_mutation": "not_applied",
    "next_action": "operator_approval" },
  "invariants_checked": ["INV-T6", "INV-T7", "INV-T8"] }
```

### 2. Operatör onayı — `osp_approve_cold_start` (claim 1)

```json
{ "ok": true, "tool": "osp_approve_cold_start",
  "result": {
    "commit_result": "AcceptAsColdStart",
    "apply_target": "Sandbox",
    "mainline_promotion": "not_available",
    "repositioned": [10000], "t_c": 1, "applied": true,
    "evidence": {
      "task_id": 7, "claim_id": 1,
      "baseline_reason": { "AllMembersIntroducedByDelta": { "members": [10000] } },
      "subject_digest": "[32 byte]", "measurement_context_digest": "[32 byte]",
      "base_space_view_revision": { "content_digest": "[32 byte]", "sequence": 0,
                                    "view_id": { "Ephemeral": 0 } },
      "policy": "RequireOperatorApproval",
      "operator_id": "op-rund", "authorization_id": "APR-97-RUND-1",
      "improvement_claimed": false } },
  "invariants_checked": ["INV-T2", "INV-T6", "INV-T8", "INV-T9"] }
```

Issue #97'nin 8 alanı + audit bağlamı (`base_space_view_revision`) canlı wire'da;
`improvement_claimed` ctor sabiti false (INV-T6); Mainline promotion yok (INV-T8);
Sandbox apply node 10_000'i gerçekten işledi (`repositioned: [10000]`, `t_c: 1`).

### 3. Tek kullanım — tüketilmiş askının ikinci onayı

```json
{ "result": { "error": "unknown_suspension", "applied": false, "retryable": false,
              "detail": "..." } }
```

### 4. INV-T2 canlı pin — agent mode'da `osp_approve_cold_start`

```json
{ "ok": false, "error_code": "OPERATOR_CAPABILITY_REQUIRED",
  "message": "tool 'osp_approve_cold_start' requires operator mode — agent mode denied (INV-T2)",
  "invariants_checked": ["INV-T2"], "recoverable": true }
```

(Aynı reddin tool-gate mekanizması `gate_operator_tool` — #131'deki per-tool pin
adayının canlı karşılığı.)

### 5. Onay sonrası aynı delta (oturum 2: submit → approve → submit, TEK süreç)

Motor süreç-bellekli olduğundan üç çağrı da aynı server sürecinde koştu. Üçüncü
çağrıda uzay artık node 10_000'i içeriyor → baseline **Available** → cold-start
ASLA; normal V2 değerlendirmesi:

```json
{ "result": {
    "attempt_outcome": { "gate_decision": "PassedAll",
                         "predicate_completion": "NotCompleted",
                         "mutation_decision": "Reject", "witness_status": null },
    "apply_target": "NotApplied",
    "loss_after": 0.5678908345800274,
    "measured_after": { "coupling": { "source": "TreeSitter", "value": 0.0 },
                        "cohesion": { "source": "Placeholder", "value": 0.5 },
                        "entropy": { "source": "Heuristic", "value": 0.46153846153846156 },
                        "instability": { "source": "TreeSitter", "value": 0.5 },
                        "witness_depth": { "source": "Heuristic", "value": 0.3496052731635571 } } } }
```

**#100 V2 cutover'in canlı bonus kanıtı:** cold-start tüketildikten sonra akış
`evaluate_task_gate_v2`'nin derived-loss yoluna girer (Available baseline +
preferred_vector Some → `trajectory_loss(after, preferred)`); skaler caller girdisi
yok. Onaylanmış node'un base ölçümü canlı görülüyor (coupling TreeSitter 0.0 —
bağlantısız module).

### Bulgular (dogfood amaçlı yüzey geri bildirimi)

**F1 — `osp_submit_delta` claim_id sabit (1) → eşzamanlı askılar birbirini ezer.**
`server.rs:890` `try_new(proposal, raw, task, 1, 1)` — claim_id (ve agent id) 1
hardcode. Canlı: iki submit aynı `claim_id: 1` döndürdü; motorun in-flight
`suspended_cold_starts` map'i claim_id ile anahtarlı → ikinci askı İLKİNİ EZER.
Farklı task'ların askıları da çapraz çakışır (task A'nın daveti task B submit'iyle
düşer → `TaskMismatch` fail-closed). Güvenlik delili değil (her yol fail-closed);
kullanılabilirlik kusuru. Motor eşzamanlı askıları destekler (engine testleri ayrı
id'lerle). **Takip: issue #133** (server-ömürlü monotonic ClaimId sayacı + test
adayları).

**F2 — canlı MCP yüzeyinde `stale_binding` fiilen erişilemez.** Zincirin 5. adımı
(askıdayken uzay ilerlerse stale fail-closed) planlandığı gibi koşamadı: F1 iki eşzamanlı
askıyı engelliyor; uzayı ilerletebilen tek eylem cold-start onayının kendisi (kendi
kaydını tüketir) ve witness'lı mainline commit boş omega ile Held'de kalır (production
witness policy). Canlı sonuç: ikinci askının "onayı" `unknown_suspension` oldu (kayıt
zaten ezilmiş/tüketilmiş). Davranış motor-seviyesinde test-pinned
(`md3_approve_cold_start_stale_space_fails_closed`); #133 düzelince canlı doğrulama
mümkün olur.

## Ek — #133 sonrası canlı stale zinciri (2026-09-29)

**F1/F2 kapanışı** (issue #133, branch `feat/133-mcp-claim-id`):
`submit_delta_attempt` claim id'yi artık server-ömürlü monotonic sayaçtan alır
(`Workspace.next_claim_id`, AtomicU64, ilk submit 1 — Run D kanıtlarıyla wire-uyumlu).
Run D'nin "planlanan fakat canlıda ulaşılamayan" 4. adımı aynı gün **canlı koştu** —
tek operator server oturumu (motor süreç-bellekli), Run D sürücü kalıbı
(`Temp/osp-133/`, aynı fixture; task 7 ve delta payload'ları birebir):

1. submit A → `SuspendedColdStart`, **claim_id 1**
2. submit B → `SuspendedColdStart`, **claim_id 2** — F1 kapanışı: eşzamanlı askılar
   benzersiz id alır (pre-fix ikisi de 1'di ve B'nin kaydı A'nınkini eziyordu)
3. approve A → `AcceptAsColdStart`, `applied: true`, `t_c: 1`
4. **approve B → `stale_binding` fail-closed — F2 kapanışı.** `detail` 5-fence
   kanıtı taşır: askı `SpaceViewRevision { Ephemeral(0), sequence: 0 }` anına bağlı,
   onay anında uzay `Ephemeral(1), sequence: 1`'de (A'nın onayı ilerletti);
   `applied: false`, `retryable: false`
5. approve B tekrar → yine `stale_binding` (kayıt yerinde — motor-parite:
   `md3_approve_cold_start_stale_space_fails_closed`)
6. approve A tekrar → `unknown_suspension` (tek kullanım korunur)

Agent id 1 bilinçli kaldı: MCP tek agent lane — `AgentId` gönderen özneyi,
`ClaimId` attempt'i adlandırır; aynı yüzeyden ikinci submit agent kimliğini
değiştirmez. Contract pin'leri: `crates/osp-mcp/tests/md3_cold_start_approval.rs`
(`mcp_concurrent_cold_start_suspensions_unique_claim_ids_second_stale` +
`mcp_cross_task_cold_start_suspensions_do_not_clobber`). Kanıt dosyaları
`Temp/osp-133/evidence/r133-*.json` (disposable; kritik alanlar yukarıda donuk).


## Sonuç

**Zincir canlı doğrulandı:** (1) askı (`SuspendedColdStart`, mutation yok, typed
baseline reason), (2) operatör onayı (`AcceptAsColdStart` + Sandbox apply + evidence
8 alan + INV-T6/T8 pin'leri), (3) tek kullanım (`unknown_suspension`), (4) INV-T2
agent-mode reddi, (5) onay sonrası normal V2 derived-loss yolu — tamamı #100 Faz 8a
cutover zinciri üzerinden, gerçek binary + gerçek analyze + gerçek motor. MCP
contract test'lerinin **ana cold-start zinciri** (askı → onay → tek kullanım) canlı
süreçte birebir doğrulandı; **stale-binding kolu #133 nedeniyle canlıda koşulamadı
ve yalnız motor-seviyesinde test-pinned kaldı** (F2) — gözlem ile plan arasındaki
sapma F1/F2 bulguları olarak kayda geçti.

Dogfood geri bildirimi: **F1** (issue #133 — claim-id clobber), **F2** (yüzey
karakterizasyonu: canlı stale_binding erişilemez; motor-seviye kanıt mevcut). CLI
run-envelope yüzeyi cold-start için yapısal kapalı (yukarıda) — Run B emsali.
**F1/F2 aynı gün #133 ile kapatıldı — canlı stale zinciri "Ek" bölümünde.**

Run A/B/C/D seti tamam: A (Completed, MD-2 confound canlı), B (Held — CLI yüzey
kapalı, ProcessLocal store), C (divergent fixture, subject cutover), **D (cold-start
onay akışı + V2 cutover canlı)**.


## Ortam notları (Windows)

`export PATH="$HOME/.cargo/bin:$PATH"`; `command grep`; `node -e` (python yok — Türkçe
kesme işaretli metinlerde inline `node -e` quoting kırılır, script'ler temp dosyaya).
ASLA `git add -A`. CI parity: fmt + clippy `-D warnings` + aynı target setinde tam paket.
