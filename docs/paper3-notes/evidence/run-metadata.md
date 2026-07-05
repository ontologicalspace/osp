# Paper 3 — Run Metadata (Evidence Freeze)

> **Tek ev** of volatile kanıt bilgileri (commit/tarih/toolchain/sha256). Evidence JSON'lar
> saf deterministik builder çıktısıdır — volatile alan içermez (snapshot drift imkânsız).
> Paper 2 pattern'i (`paper2-draft-v1.md` run-metadata tablosu) izlenmiştir.

## Run metadata

| Parameter | Value |
|---|---|
| OSP commit (frozen evidence, hash₁) | `481690d6f904d312ac08232c4572a13232ef2848` (`481690d`) |
| Branch | `docs/paper3-draft` |
| Frozen date | 2026-07-05 |
| Rust toolchain | `rustc 1.95.0 (59807616e 2026-04-14)` |
| osp-core tests | 450+ (Paper 1/2/3 birleşik; paper3_evidence + paper3_heldout eklendi) |
| Paper 3 trybuild (type-level) | 11 Paper 3'e özgü (kümülatif 18 bağlam) — `tests/anchoring_typelevel.rs` |
| Golden fixtures | 13 (`anchoring.fixture.v1`) |
| Held-out fixtures | 5 (4 held_out + 1 regression_anchored) |
| Snapshot discipline | `PAPER3_FREEZE=1 cargo test -p osp-core --test {paper3_evidence,paper3_heldout} -- --ignored --nocapture` |

## Evidence strata (5 katman)

| Stratum | Amaç | Kanıt | Test |
|---|---|---|---|
| **(1) Type-level trybuild** | INV-C1..C8, INV-P1..P3 compile-time | 11 Paper 3'e özgü (kümülatif 18) | `tests/anchoring_typelevel.rs` |
| **(2) Golden fixture conformance** | 13 fixture pipeline davranışı | `anchoring_mvp.rs` + `anchoring_fixtures.rs` | `cargo test -p osp-core --test anchoring_mvp` |
| **(3) Held-out adversarial** | 5 cümle totoloji-olmayan RQ1 | `held-out-adversarial-fixtures.json` | `paper3_heldout.rs` |
| **(4) E2E binding chain replay** | Uçtan uca zincir (lowering→task) | `e2e-binding-chain-replay.json` | `paper3_evidence.rs` |
| **(5) E2E rejected paths replay** | 4 reddedilen kapı | `e2e-rejected-paths-replay.json` | `paper3_evidence.rs` |

## Evidence JSON dosyaları + sha256

| Dosya | sha256 |
|---|---|
| `e2e-binding-chain-replay.json` | `9890bdc48f0904a4b206b5a7c70b3892726a71912f50787cdf2f7ca123ecb396` |
| `e2e-rejected-paths-replay.json` | `5df6a272598277944abeb8fe4d4bfc9c9611ebbc51b273e596c460a1971278e1` |
| `held-out-adversarial-fixtures.json` | `1b704ff49873f6ce9e2a782b784ab1bef2da1a69d30ec852d1ecb64c4d85f710` |

## Üretim komutları

```bash
# Drift yakalar (normal CI — source tree'yi mutate ETMEZ):
cargo test -p osp-core --test paper3_evidence
cargo test -p osp-core --test paper3_heldout

# Bilinçli dondurma (kod değiştiğinde, evidence güncellenmeli):
PAPER3_FREEZE=1 cargo test -p osp-core --test paper3_evidence -- --ignored --nocapture
PAPER3_FREEZE=1 cargo test -p osp-core --test paper3_heldout -- --ignored --nocapture
```

## Not

- Evidence JSON'lar **saf deterministik builder çıktısıdır** — aynı kod → bayt-bayt aynı JSON.
- Volatile alanlar (commit/tarih/toolchain) burada yaşar, JSON'larda DEĞİL.
- Snapshot testleri `assert_eq!(generated, frozen)` ile drift yakalar; `PAPER3_FREEZE=1` ile
  bilinçli yeniden dondurma. *"Test altına alınmayan invariant ihlal edilir."*
