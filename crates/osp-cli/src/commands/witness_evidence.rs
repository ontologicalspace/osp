//! #205 — `osp witness-evidence --template`: tanık kanıt dosyasının BİÇİMİNİ
//! keşfedilebilir kılan operatör yüzeyi.
//!
//! Tasarım ilkesi (karar-5): **protokol geçerli kanıtın nasıl ifade edileceğini
//! öğretir; kanıt üretmiş gibi davranmaz.** Şablon tek olaydır, kendi WitnessSet
//! değerlendirmesini YAPMAZ (yeni authorization mekanizması değil) ve
//! production-default koşulda tek başına quorum sağlayamaz. Placeholder
//! alanlar gerçek tanıklık iddiası taşımaz.
//!
//! Tek sözleşme (karar-4): şablon ve hata-mesajı örneği `WitnessEvidenceWireV1`
//! DEĞERLERİNDEN serde serialize edilir — wire tipiyle yapısal senkron; ikinci bir
//! JSON şeması tanımlanmaz, `weight` asla üretilmez (`deny_unknown_fields`).
//! Ağırlık tablosu + quorum satırı + worked-example'lar
//! `WitnessKind::default_weight()` ve `WitnessSet::new` sabitlerinden runtime
//! hesaplanır — elle bakımlanan türev YOK (karar kaydı #205).

use std::path::PathBuf;

use clap::Args;
use osp_core::witness::{WitnessKind, WitnessSet};

use super::WitnessEvidenceWireV1;

// ═══════════════════════════════════════════════════════════════════════════════
// Varyant kümesi — exhaustive-match bekçisi (tur-1 P2-1)
// ═══════════════════════════════════════════════════════════════════════════════

/// Tablo/uyarı yüzeylerinde listelenen tüm varyantlar (enum bildirim sırası).
///
/// **Bakım kuralı:** enum'a yeni `WitnessKind` varyantı eklendiğinde bu diziyi de
/// güncelle — bitişik `_witness_kind_index` exhaustive match'i yeni varyantta
/// DERLEME RED'i verir ve seni buraya getirir (artık risk bilinçli kabul:
/// dizi ↔ enum senkronu derleme-zamanı değil, bekçi + pinleyici testle korunur).
const WITNESS_KINDS: [WitnessKind; 4] = [
    WitnessKind::MergeCommit,
    WitnessKind::PRMerged,
    WitnessKind::TrailerReviewed,
    WitnessKind::CoAuthored,
];

/// Derleme-zamanı varyant-kümesi bekçisi — enum'a varyant eklenirse bu match
/// non-exhaustive olur ve derleme düşer. Dönüş değeri dizinin pozisyonuyla birebir
/// (tablo üreticisinde `debug_assert` + test pinler).
const fn _witness_kind_index(kind: WitnessKind) -> usize {
    match kind {
        WitnessKind::MergeCommit => 0,
        WitnessKind::PRMerged => 1,
        WitnessKind::TrailerReviewed => 2,
        WitnessKind::CoAuthored => 3,
    }
}

/// Varyantın wire'daki ismi — elle yazılmaz, serde sözleşmesinden okunur
/// (karar-4: isim senkronu elle listeden değil serialization'dan).
fn witness_kind_wire_name(kind: WitnessKind) -> String {
    let value = serde_json::to_value(kind).expect("WitnessKind serializes to a JSON value");
    value
        .as_str()
        .expect("unit-enum WitnessKind serializes to a JSON string")
        .to_owned()
}

/// Wire'daki izinli `witness_kind` değerleri — hata mesajı ve stderr rehberi
/// için tek kaynak: `"MergeCommit | PRMerged | TrailerReviewed | CoAuthored"`.
pub(super) fn witness_kind_names_joined() -> String {
    WITNESS_KINDS
        .iter()
        .map(|k| witness_kind_wire_name(*k))
        .collect::<Vec<_>>()
        .join(" | ")
}

// ─────────────────────────────────────────────────────────────────────────
// Şablon — wire değerlerinden serialize (tek olay; yetkilendirmez)
// ─────────────────────────────────────────────────────────────────────────

/// Şablonun tek örnek olayı — yalnızca BİÇİMİ öğreten iskelet (karar-5).
///
/// Placeholder'lar (`id`/`source`/`actor`) gerçek tanıklık iddiası TAŞIMAZ;
/// operator her alanı gerçek kanıtla değiştirir. `claim` tek bağlanabilir
/// alandır: `--artifact` verildiyse kaydın gerçek claim_id'si, yoksa 0.
fn template_events(claim: u64) -> Vec<WitnessEvidenceWireV1> {
    vec![WitnessEvidenceWireV1 {
        id: 1,
        source: "replace-me".to_string(),
        witness_kind: WitnessKind::MergeCommit,
        actor: 2,
        claim,
    }]
}

/// Hata mesajlarındaki tek-satır örnek — kök wire biçimiyle birebir
/// (JSON array; tur-3 notu B). Şablonun kendisiyle aynı üreticiden (tek kaynak).
pub(super) fn witness_evidence_example_line() -> String {
    serde_json::to_string(&template_events(0)).expect("template events serialize")
}

// ─────────────────────────────────────────────────────────────────────────
// Tablo + worked-example'ler — sabitlerden runtime hesap (karar-2)
// ─────────────────────────────────────────────────────────────────────────

/// Tür/ağırlık tablosu + production-default quorum satırı.
///
/// Tüm sayılar sabitlerden runtime hesaplanır (`default_weight` + `WitnessSet::new`
/// production default'u); elle bakımlanan metin YOK. Quorum satırı "production
/// default" niteliğiyle sunulur — resume quorum parametrelerini **artifact'tan**
/// alır (operator düşüremez); özelleştirilmiş politikalara genelleme yapılmaz.
pub(super) fn witness_weight_table_text() -> String {
    let defaults = WitnessSet::new(vec![]); // production default (§4.2)
    let mut text = String::from("witness_kind weights (WitnessKind::default_weight, §4.1):");
    for (i, kind) in WITNESS_KINDS.iter().enumerate() {
        // Bekçi ↔ dizi pozisyon bütünlüğü (tur-1 P2-1) — sıra bozulursa anında görünür.
        debug_assert_eq!(_witness_kind_index(*kind), i);
        text.push_str(&format!(
            "\n  {:<16} {:.1}",
            witness_kind_wire_name(*kind),
            kind.default_weight()
        ));
    }
    text.push_str(&format!(
        "\nquorum (production default): at least {} distinct non-author approvers AND \
         total weight >= {:.1} — resume evaluates with the artifact's recorded quorum \
         parameters, which the operator cannot lower",
        defaults.min_approvers, defaults.quorum_threshold
    ));
    text
}

/// İki worked-example — toplamlar VE karşılaştırma sonuçları (≥ / <) sabitlerden
/// runtime hesaplanır; ✓/✗ biçimindeki hüküm hardcoded DEĞİL (tur-1 P2-4).
/// Approver sayısı da sabitten: örnekler kasıtlı olarak minimal quorum setidir
/// (min_approvers olay, hepsi distinct actor) — kod-review P2-1.
pub(super) fn witness_worked_examples_text() -> String {
    fn pair(a: WitnessKind, b: WitnessKind) -> String {
        let defaults = WitnessSet::new(vec![]); // production default (§4.2)
        let sum = a.default_weight() + b.default_weight();
        let (cmp, verdict) = if sum >= defaults.quorum_threshold {
            (
                ">=",
                format!(
                    "meets the production-default support threshold (with {} distinct \
                     approvers)",
                    defaults.min_approvers
                ),
            )
        } else {
            (
                "<",
                format!(
                    "held: insufficient support ({} distinct approvers, weights sum below \
                     the threshold)",
                    defaults.min_approvers
                ),
            )
        };
        format!(
            "  {} + {} = {:.1} {} {:.1} — {}",
            witness_kind_wire_name(a),
            witness_kind_wire_name(b),
            sum,
            cmp,
            defaults.quorum_threshold,
            verdict
        )
    }
    format!(
        "worked examples (computed from the weights above):\n{}\n{}",
        pair(WitnessKind::MergeCommit, WitnessKind::TrailerReviewed),
        pair(WitnessKind::TrailerReviewed, WitnessKind::CoAuthored),
    )
}

/// `osp trajectory resume --help` ek metni — tablo + worked-example'lar +
/// şablon komutuna işaret. `#[command(after_help = …)]` runtime üretir;
/// wiring'i gerçek binary üzerinde subprocess testi kanıtlar (karar-2).
pub(super) fn witness_resume_after_help() -> String {
    format!(
        "Witness evidence (--witness-evidence):\n{}\n\n{}\n\nGenerate a strict-parse-passing \
         single-event skeleton with `osp witness-evidence --template \
         [--artifact <pending.json>]` — the template does not by itself satisfy quorum.",
        witness_weight_table_text(),
        witness_worked_examples_text()
    )
}

// ─────────────────────────────────────────────────────────────────────────
// `osp witness-evidence --template` — kalıcı etkisiz şablon yüzeyi (karar-3)
// ─────────────────────────────────────────────────────────────────────────

/// `osp witness-evidence` — #205: tanık kanıt dosyasının biçimini öğretir.
#[derive(Args, Debug)]
pub struct WitnessEvidenceArgs {
    /// Strict-parse geçen TEK-olaylı şablon bas (stdout: saf JSON — redirect ile
    /// dosyaya yazılır; rehberlik stderr'e gider). Şablon quorum SAĞLAMAZ ve
    /// placeholder değerler gerçek tanıklık iddiası taşımaz (karar-5).
    /// Zorunlu bayrak: `#[arg(long)]` bool tek başına zorunlu kılmaz (tur-3 A).
    #[arg(long, required = true)]
    pub template: bool,
    /// Pending authorization artifact — verilirse şablonun `claim` alanı kaydın
    /// gerçek claim_id'siyle dolar ve stderr'de askı kimliği gösterilir.
    /// Yalnız OKUNUR: fence/receipt/kalıcı etki üretilmez (karar-3).
    #[arg(long)]
    pub artifact: Option<PathBuf>,
}

/// Şablon üretimi — read-only operatör yüzeyi. stdout'ta yalnız şablon JSON'u
/// (redirect edilebilir); bütün açıklama/uyarılar stderr'de.
pub fn run_witness_evidence(args: WitnessEvidenceArgs) -> anyhow::Result<()> {
    // --artifact: askı kimliğini oku — yalnız claim bağlama + bağlam (yazma YOK).
    let claim = match &args.artifact {
        Some(path) => {
            let envelope = match osp_core::authorization::load_pending_authorization(path) {
                Ok(envelope) => envelope,
                Err(e) => {
                    eprintln!(
                        "✗ System failure: pending authorization artifact {}: {e}",
                        path.display()
                    );
                    std::process::exit(super::exit_codes::SYSTEM_FAILURE);
                }
            };
            let record = envelope.record();
            eprintln!(
                "pending suspension: task {} / claim {} / attempt {} — every evidence \
                 event's `claim` must equal this claim id",
                record.task_id,
                record.claim_id,
                record.attempt_num.get()
            );
            record.claim_id
        }
        None => {
            eprintln!(
                "no --artifact given: template uses claim 0 — replace it with the pending \
                 artifact's claim id (claim-binding is fail-closed: the resume fence and \
                 evaluate both reject foreign-claim evidence, #216)"
            );
            0
        }
    };

    // stdout: saf JSON (redirect ile kanıt dosyasına). Tek olay — yetkilendirmez.
    let json =
        serde_json::to_string_pretty(&template_events(claim)).expect("template events serialize");
    println!("{json}");

    // stderr: rehberlik — stdout JSON'unu bozmaz.
    eprintln!();
    eprintln!("witness evidence template — strict wire, ONE event shown; the template");
    eprintln!("does NOT satisfy quorum: duplicate the event for each real witness and");
    eprintln!("replace every placeholder (evidence does NOT accumulate across resumes).");
    eprintln!(
        "fields: id: u64 | source: string | witness_kind: one of {} | actor: u64 | claim: u64",
        witness_kind_names_joined()
    );
    eprintln!("  id     — unique per event in this file (plain integer, not a string)");
    eprintln!("  source — the real evidence source, e.g. \"PR #42\", a commit sha,");
    eprintln!("           or \"trailer:Reviewed-by:<...>\" (operator-asserted)");
    eprintln!("  actor  — 2 is a PLACEHOLDER: the real witness's agent id. It must differ");
    eprintln!("           from the claim author — an actor equal to the author is INVALID");
    eprintln!("           evidence (exit 20), not a hold. The artifact does not record the");
    eprintln!("           author, so OSP cannot pick a collision-free placeholder for you.");
    eprintln!(
        "  claim  — {}",
        if args.artifact.is_some() {
            "bound to the artifact's claim id".to_string()
        } else {
            "0 placeholder — replace with the artifact's claim id".to_string()
        }
    );
    eprintln!();
    eprintln!("{}", witness_weight_table_text());
    eprintln!();
    eprintln!("{}", witness_worked_examples_text());
    eprintln!();
    eprintln!("usage: osp witness-evidence --template --artifact <pending.json> > witnesses.json");
    eprintln!("then:  osp trajectory resume <pending.json> --repo <repo> \\");
    eprintln!("         --witness-evidence witnesses.json --evidence-trust operator-asserted \\");
    eprintln!("         --state-dir <state-dir>");
    eprintln!("`--evidence-trust operator-asserted` is REQUIRED: OSP does not verify the");
    eprintln!("events against reality — the operator vouches for them (recorded in the receipt).");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testler — kabul kriterleri 1-3 + karar-5 kilitleri
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::load_witness_evidence;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("osp205-{name}-{}.json", std::process::id()))
    }

    fn write_and_load(
        name: &str,
        json: &str,
    ) -> anyhow::Result<Vec<osp_core::witness::EvidenceEvent>> {
        let path = temp_path(name);
        std::fs::write(&path, json).expect("write temp evidence");
        let result = load_witness_evidence(&path);
        let _ = std::fs::remove_file(&path);
        result
    }

    // (1) Kabul kriteri 1: şablon dosyaya yazılır → strict parse GEÇER.
    // Ayrıca P1-1 kilidi: olay başına TAM 5 wire alanı, `weight` YOK.
    #[test]
    fn template_roundtrips_strict_wire_with_exactly_five_fields() {
        let json = serde_json::to_string_pretty(&template_events(7)).unwrap();
        let events = write_and_load("roundtrip", &json).expect("template passes the strict wire");
        assert_eq!(events.len(), 1, "şablon TEK olay (karar-5)");
        assert_eq!(events[0].claim, 7);
        assert_eq!(events[0].actor, 2);
        assert_eq!(
            events[0].weight,
            WitnessKind::MergeCommit.default_weight(),
            "weight wire'dan gelmez — kind'ın kalibre değerinden türetilir"
        );

        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_array().expect("root is a JSON array")[0]
            .as_object()
            .expect("event is an object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["actor", "claim", "id", "source", "witness_kind"],
            "şablon wire tipinden serialize — fazlası/eksik yok"
        );
    }

    // (2) Karar-5 (tur-2 P2-1 hâliyle): Held iddiası YOK — yalnız quorum KOŞULU.
    // Production default min 2 distinct non-author approver; tek olay 1 < 2.
    // Gerçek Held/exit-10 disposition'u integration testinde kanıtlanır.
    #[test]
    fn template_single_event_cannot_meet_production_default_quorum() {
        let events = write_and_load(
            "quorum",
            &serde_json::to_string_pretty(&template_events(0)).unwrap(),
        )
        .expect("strict parse");
        // 999: şablonun placeholder actor'ü (2) DEĞİL — author çakışması olmadığı
        // varsayımı altında canonicalize events'i dışlamaz. #217: worked-example
        // kanıtları WitnessKind::default_weight SABİTLERİNDEN üretilir — sayısal
        // geçerlik yapısal (validate_numerics'in başarısız olması imkânsız).
        let canon = WitnessSet::new(events)
            .canonicalize_for(999)
            .expect("worked-example evidence derives from constants — numerically valid");
        assert_eq!(canon.approver_count(), 1);
        assert_eq!(canon.min_approvers(), 2);
        assert!(
            canon.approver_count() < canon.min_approvers(),
            "tek olay production-default Q1 koşulunu yapısal olarak karşılayamaz"
        );
    }

    // (3) Kabul kriteri 2: tablo her varyantı serde-ismiyle + sabit ağırlığıyla
    // listeler; quorum satırı production-default niteliği taşır. Dizi ↔ bekçi
    // pozisyon bütünlüğü de burada pinlenir (tur-1 P2-1).
    #[test]
    fn weight_table_lists_every_kind_with_its_constant() {
        let table = witness_weight_table_text();
        for (i, kind) in WITNESS_KINDS.iter().enumerate() {
            assert_eq!(_witness_kind_index(*kind), i, "dizi ↔ bekçi pozisyonu");
            // Kod-review P2-2: ad ↔ ağırlık AYNI SATIRDA doğrulanır — ayrı
            // contains kontrolleri türün adını bir satırda, ağırlığını başka
            // satırda bulup yanlış eşleşmeyi geçirirdi.
            let name = witness_kind_wire_name(*kind);
            let row = table
                .lines()
                .find(|l| l.contains(&name))
                .unwrap_or_else(|| panic!("tablo {name} satırını listelemeli:\n{table}"));
            assert!(
                row.contains(&format!("{:.1}", kind.default_weight())),
                "{name} satırı KENDİ ağırlığını taşımalı (satır: {row})"
            );
        }
        let defaults = WitnessSet::new(vec![]);
        assert!(
            table.contains(&format!(
                "at least {} distinct non-author approvers",
                defaults.min_approvers
            )),
            "quorum satırı production-default min_approvers taşır: {table}"
        );
        assert!(
            table.contains(&format!(">= {:.1}", defaults.quorum_threshold)),
            "quorum satırı production-default eşiği taşır: {table}"
        );
        assert!(
            table.contains("production default"),
            "quorum satırı niteliğini açıklar (artifact'tan gelen parametrelere genellemez)"
        );
    }

    // (4) Tur-1 P2-4: worked-example toplamları VE ≥/< hükümleri runtime
    // aritmetikle birebir — ağırlıklar değişse metin kendini yeniden hesaplar.
    #[test]
    fn worked_examples_match_runtime_arithmetic() {
        let text = witness_worked_examples_text();
        let theta = WitnessSet::new(vec![]).quorum_threshold;
        for (a, b) in [
            (WitnessKind::MergeCommit, WitnessKind::TrailerReviewed),
            (WitnessKind::TrailerReviewed, WitnessKind::CoAuthored),
        ] {
            let sum = a.default_weight() + b.default_weight();
            let cmp = if sum >= theta { ">=" } else { "<" };
            let expected = format!(
                "{} + {} = {:.1} {} {:.1}",
                witness_kind_wire_name(a),
                witness_kind_wire_name(b),
                sum,
                cmp,
                theta
            );
            assert!(
                text.contains(&expected),
                "worked-example runtime aritmetikle birebir olmalı: {expected} ∈ {text}"
            );
        }
        // Kod-review P2-1: approver sayısı da min_approvers sabitinden türetilir.
        assert!(
            text.contains(&format!(
                "{} distinct approvers",
                WitnessSet::new(vec![]).min_approvers
            )),
            "worked-example approver sayısı sabitten gelmeli: {text}"
        );
    }

    // (5) Kabul kriteri 3 — run-26'nın üç denemesi (string id / string actor /
    // bilinmeyen kind): mutlu-zincir şablonundan TEK bozulma. Hata mesajı tipli
    // alan listesi + kök-form örnek satırı taşır; serde'nin kendi detayı korunur.
    #[test]
    fn malformed_evidence_matrix_carries_typed_fields_and_example() {
        let base = serde_json::to_value(template_events(0))
            .unwrap()
            .as_array()
            .unwrap()[0]
            .clone();
        let example = witness_evidence_example_line();

        let cases: Vec<(&str, serde_json::Value, &str)> = vec![
            ("string id", serde_json::json!("one"), "id"),
            ("string actor", serde_json::json!("two"), "actor"),
            (
                "unknown kind",
                serde_json::json!("HumanReview"),
                "witness_kind",
            ),
        ];
        for (name, bad_value, field) in cases {
            let mut event = base.clone();
            event[field] = bad_value;
            let file = serde_json::to_string_pretty(&serde_json::json!([event])).unwrap();
            let err = write_and_load("matrix", &file)
                .expect_err("tek-bozulma mutlu-zinciri strict parse'ta RED olmalı");
            let msg = format!("{err:#}");
            assert!(
                msg.contains("id: u64") && msg.contains("actor: u64") && msg.contains("claim: u64"),
                "{name}: mesaj tipli alan listesi taşımıyor: {msg}"
            );
            assert!(
                msg.contains(&witness_kind_names_joined()),
                "{name}: mesaj izinli witness_kind setini listelemeli: {msg}"
            );
            assert!(
                msg.contains(&example),
                "{name}: mesaj kök-form örnek satırı taşımalı: {msg}"
            );
            let serde_detail = match field {
                "witness_kind" => "unknown variant",
                _ => "invalid type: string",
            };
            assert!(
                msg.contains(serde_detail),
                "{name}: serde'nin kendi detayı korunmalı ({serde_detail}): {msg}"
            );
        }
    }
}
