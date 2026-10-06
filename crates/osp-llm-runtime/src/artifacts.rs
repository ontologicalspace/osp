//! #171 §5/D6 — llm-real producer-channel kanıt artifact'ları (preregistered mod).
//!
//! `trajectory attempt --llm real --llm-artifacts <dir>` çağrısının DONMUŞ
//! prompt'unu, ham yanıtını, parse edilmiş öneriyi ve D4 bar elicitation'ı run
//! dizinine yazar. `--llm-artifacts` = #171 preregistration protokolü: kayıt +
//! D6 strict retry semantiği birlikte açılır (kayıt varken tamir döngüsü
//! çalışması structurally imkânsız — navigator `ParseFailurePolicy::
//! TerminalNoRepair` ile koşar).
//!
//! Dosyalar:
//! - `llm-prompt.json` — ilk proposal completion'ının donmuş anchor'u:
//!   (system, user) + `prompt_digest` + **`completion_identity_digest`**
//!   (model/endpoint/temperature/max_tokens dahil — treatment kimliği) +
//!   INV-E1 provenance bloğu. Freeze disiplini (INV-E1): dosya YOKSA
//!   no-clobber donar; VARSA yeni completion'ın her iki digest'i donmuş
//!   değerlerle AYNI olmalıdır (D6: yalnız aynı-prompt aynı-config ağ
//!   retry'sına izin var; farklı prompt/config = yeni satır değil YASAK).
//! - `llm-raw-response.txt` — ham asistan metni (dönüştürme YOK).
//! - `llm-proposals.json` — parse edilmiş `DeltaProposal` + usage + D6 durumu:
//!   parse ihlali → `proposal: null` + `parse_error` (tamir döngüsü YOK —
//!   terminal, kayıt mühürlenir); transport tükenmesi → `transport_error` +
//!   `network_retry_count` (aynı-prompt retry devam edebilir).
//! - `llm-bar-prompt.json` / `llm-bar-response.json` — D4 bar elicitation'ın
//!   donmuş anchor'u + yanıt kaydı (aynı iki-dosya disiplini; tek dosyada
//!   prompt/response birleşik olamaz — frozen anchor ile değişken kayıt
//!   ayrıdır).
//!
//! Bu dosyalar #178 read-once zincirine GİRMEZ (tüketici değil üretici
//! kanalı); envelope'un `proposals_digest: null` bilinçli boşluğu aynen
//! kalır. Kayıt yalnızca İLK completion'da yapılır (first-freeze):
//! navigator'ın feedback retry'ları prompt'u değiştirir ve donmuş artifact'ı
//! kirletirler — onların kanıt izi attempt envelope'unun evidence dizisindedir.

use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::response::{strip_code_fence, RawCompletion, TokenUsage};
use crate::runtime::{CompletionRequest, RuntimeConfig};

/// `sha256:<64 hex>` — house digest formatı (finalize-run K3 ile aynı).
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Prompt-pair wire — digest tam olarak bu canonical serileştirme üzerindedir
/// (alan sırası sabit: system, user; JSON escaping sınır belirsizliğini kapatır).
#[derive(Serialize)]
struct PromptPair<'a> {
    system: &'a str,
    user: &'a str,
}

/// (system, user) çiftinin donmuş kimliği — `prompt_digest` alanı ve
/// freeze-doğrulamasının birinci eksenidir.
pub fn completion_prompt_digest(req: &CompletionRequest) -> String {
    let pair = PromptPair {
        system: &req.system,
        user: &req.user,
    };
    let canonical = serde_json::to_string(&pair).expect("string-only pair serialize");
    sha256_hex(canonical.as_bytes())
}

/// Completion identity wire — treatment kimliğinin canonical serileştirme
/// sırası: model, endpoint, temperature, max_tokens, prompt_digest.
#[derive(Serialize)]
struct CompletionIdentity<'a> {
    model: &'a str,
    endpoint: &'a str,
    temperature: f32,
    max_tokens: u32,
    prompt_digest: &'a str,
}

/// **Completion identity digest (review P1):** prompt BİRBİRİMİYLE aynı olsa
/// bile model/endpoint/temperature/max_tokens farklıysa bu aynı deney
/// completion'ı DEĞİLDİR — treatment kimliğinin parçasıdır (design note §3
/// `model`/`temperature` satır alanları). Freeze doğrulaması her iki digest'i
/// de karşılaştırır; retry için ikisi de eşit olmalıdır.
pub fn completion_identity_digest(cfg: &RuntimeConfig, prompt_digest: &str) -> String {
    let identity = CompletionIdentity {
        model: &cfg.model,
        endpoint: &cfg.endpoint,
        temperature: cfg.temperature,
        max_tokens: cfg.max_tokens,
        prompt_digest,
    };
    let canonical = serde_json::to_string(&identity).expect("identity serialize");
    sha256_hex(canonical.as_bytes())
}

// ── INV-E1 provenance manifest ─────────────────────────────────────────────────

/// INV-E1 (v2 tasarım notu): "freeze adımında prompt'u üreten girdi listesi
/// kayda geçer — 'yalnızca şu girdiler' iddiası after-the-fact doğrulanabilir."
/// Çağıran (CLI) bu manifesti donmuş anchor'a yazar; `agent_task_view_digest`
/// kayıt anında `req.user`'ın (serileştirilmiş kör view) digest'inden hesaplanır.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvE1Manifest {
    /// `--task` dosyasının read-once digest'i (`sha256:<64 hex>`); legacy
    /// hardcoded task → `None` (dürüst boşluk).
    pub task_digest: Option<String>,
    /// Baseline kimliği — attempt anındaki analyzed repo HEAD (exact sha).
    pub baseline_digest: String,
    /// OSP binary revizyonu (build.rs `OSP_GIT_REVISION`).
    pub osp_revision: String,
    /// Prompt üreticisinin okuduğu dosyalar. Motorun prompt'u YALNIZCA engine
    /// state'inden (AgentTaskView) üretmesi nedeniyle v1'de daima BOŞ —
    /// körleme prosedürünün pozitif kaydı ("analist artifact'ı okunMADI").
    pub files_read: Vec<String>,
}

// ── donmuş prompt anchor'ları (llm-prompt.json / llm-bar-prompt.json) ──────────

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PromptDoc {
    schema_version: u32,
    kind: String,
    model: String,
    endpoint: String,
    temperature: f32,
    max_tokens: u32,
    system: String,
    user: String,
    prompt_digest: String,
    completion_identity_digest: String,
    inv_e1: InvE1Doc,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct InvE1Doc {
    agent_task_view_digest: String,
    task_digest: Option<String>,
    baseline_digest: String,
    osp_revision: String,
    files_read: Vec<String>,
}

impl InvE1Doc {
    fn from_manifest(manifest: &InvE1Manifest, req: &CompletionRequest) -> Self {
        Self {
            agent_task_view_digest: sha256_hex(req.user.as_bytes()),
            task_digest: manifest.task_digest.clone(),
            baseline_digest: manifest.baseline_digest.clone(),
            osp_revision: manifest.osp_revision.clone(),
            files_read: manifest.files_read.clone(),
        }
    }
}

/// Freeze çekirdeği: dosya yoksa no-clobber dondurur; varsa donmuş anchor ile
/// yeni completion'ın **prompt + completion-identity** digest'leri
/// karşılaştırılır (ikisi de eşit = D6 aynı-prompt aynı-config retry; prompt
/// farkı = INV-E1 ihlali; config farkı = treatment değişimi — ikisi de RED).
fn freeze_prompt(dir: &Path, file_name: &str, doc: &PromptDoc) -> anyhow::Result<()> {
    let path = dir.join(file_name);
    if path.exists() {
        let frozen = std::fs::read_to_string(&path)?;
        let existing: PromptDoc = serde_json::from_str(&frozen).map_err(|e| {
            anyhow::anyhow!(
                "{file_name} exists but is unreadable ({}): {e} — the frozen prompt \
                 anchor must stay parseable; resolve manually (do not delete blindly: \
                 it is the pre-registration anchor for this directory)",
                path.display()
            )
        })?;
        if existing.prompt_digest != doc.prompt_digest {
            anyhow::bail!(
                "{file_name} is already FROZEN for this directory with a different prompt \
                 (frozen {}, new {}). The pre-registered prompt cannot change mid-experiment \
                 (#171 INV-E1 / D6): a different prompt is a NEW experiment line, not a retry \
                 — use a fresh --llm-artifacts directory.",
                existing.prompt_digest,
                doc.prompt_digest
            );
        }
        if existing.completion_identity_digest != doc.completion_identity_digest {
            anyhow::bail!(
                "{file_name} is already FROZEN with a different completion identity \
                 (frozen model={}, endpoint={}, temperature={}, max_tokens={}; new \
                 model={}, endpoint={}, temperature={}, max_tokens={}). Model/endpoint/\
                 sampling config is part of the TREATMENT identity (#171 §3 model/temperature \
                 rows): a config change is a new experiment line — use a fresh \
                 --llm-artifacts directory.",
                existing.model,
                existing.endpoint,
                existing.temperature,
                existing.max_tokens,
                doc.model,
                doc.endpoint,
                doc.temperature,
                doc.max_tokens
            );
        }
        return Ok(());
    }
    let payload = serde_json::to_vec_pretty(doc)?;
    no_clobber_publish(&path, &payload)
}

// ── llm-proposals.json (D6 durum kaydı) ────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Default)]
struct ProposalsDoc {
    schema_version: u32,
    usage: Option<TokenUsage>,
    proposal: Option<serde_json::Value>,
    parse_error: Option<String>,
    transport_error: Option<String>,
    network_retry_count: u32,
}

impl ProposalsDoc {
    /// Kayıt mühürlü mü? D6 matrisi: parse-null (terminal) ve success
    /// (kanıt tamam) sonrası üzerine yazılamaz; YALNIZ transport tükenmesi
    /// (network_retry yolu) sonrası aynı-prompt retry kaydı güncelleyebilir.
    fn is_sealed(&self) -> bool {
        self.parse_error.is_some() || self.proposal.is_some()
    }
}

// ── llm-bar-response.json (D4 elicitation yanıt kaydı) ─────────────────────────

/// Elicitasyon öngörüsü: tek bir aday değişikliğin etiketi + öngörülen
/// değişim-sonrası coupling. Sıra (vec index) = öngörülen etki sırası (τ).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictedEffect {
    pub label: String,
    pub predicted_coupling: f64,
}

/// D4 bar elicitation'ın parse edilmiş yanıtı: kabul barı (öngörülen
/// değişim-sonrası coupling) + önerilerin öngörülen-etki sıralaması (τ).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElicitedBar {
    pub bar_value: f64,
    pub predicted: Vec<PredictedEffect>,
}

/// Elicitation yanıtını parse et (code-fence toleransı proposal yoluyla aynı).
pub fn parse_elicited_bar(content: &str) -> Result<ElicitedBar, serde_json::Error> {
    let json = strip_code_fence(content.trim());
    serde_json::from_str::<ElicitedBar>(&json)
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct BarResponseDoc {
    schema_version: u32,
    kind: String,
    prompt_digest: String,
    completion_identity_digest: String,
    usage: Option<TokenUsage>,
    raw_response: Option<String>,
    elicited: Option<ElicitedBar>,
    parse_error: Option<String>,
    transport_error: Option<String>,
}

impl BarResponseDoc {
    /// Elicitation mühürlü mü? Proposals ile aynı D6 matrisi: parse-null ve
    /// elicited-success sonrası yeniden örnekleme YASAK (cherry-pick: "beğen
    /// edilene dek bar yeniden istenmez"); yalnız transport tükenmesi
    /// retry'a açık.
    fn is_sealed(&self) -> bool {
        self.parse_error.is_some() || self.elicited.is_some()
    }
}

// ── üst-düzey kayıt API'leri ───────────────────────────────────────────────────

/// Donmuş prompt anchor'u için ortak doküman kur (proposal ve bar aynı şekil).
fn prompt_doc(
    kind: &str,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    manifest: &InvE1Manifest,
) -> PromptDoc {
    let prompt_digest = completion_prompt_digest(req);
    PromptDoc {
        schema_version: 1,
        kind: kind.to_string(),
        model: cfg.model.clone(),
        endpoint: cfg.endpoint.clone(),
        temperature: cfg.temperature,
        max_tokens: cfg.max_tokens,
        system: req.system.clone(),
        user: req.user.clone(),
        prompt_digest: prompt_digest.clone(),
        completion_identity_digest: completion_identity_digest(cfg, &prompt_digest),
        inv_e1: InvE1Doc::from_manifest(manifest, req),
    }
}

/// Mevcut llm-proposals.json durumunu oku (yoksa None) — D6 mühür kapısı.
fn existing_proposals(dir: &Path) -> anyhow::Result<Option<ProposalsDoc>> {
    let path = dir.join("llm-proposals.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    let doc: ProposalsDoc = serde_json::from_str(&raw).map_err(|e| {
        anyhow::anyhow!(
            "llm-proposals.json exists but is unreadable ({}): {e} — resolve manually \
             (the D6 outcome record must stay parseable)",
            path.display()
        )
    })?;
    Ok(Some(doc))
}

/// İlk llm-real proposal completion'ını kaydet (first-freeze; §5 + D6).
///
/// - `parse_error = Some` ⇒ `proposal: null` dürüst boşluk — **terminal**: bu
///   dizinde proposals kaydı MÜHÜRLÜ, artık üzerine yazılamaz.
/// - Başarı ⇒ kanıt tamam — aynı şekilde mühürlü.
/// - Transport tükenmesi ([`record_transport_failure`]) ⇒ tek açık durum.
///
/// Disk hatası çağıranı hatayla bırakır: kanıt persist'i protokolün parçasıdır,
/// sessiz unpersisted gerçek çağrı olmaz.
pub fn record_proposal_completion(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    raw: &RawCompletion,
    network_retry_count: u32,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let doc = prompt_doc("llm-proposal-prompt", cfg, req, manifest);
    freeze_prompt(dir, "llm-prompt.json", &doc)?;

    if let Some(existing) = existing_proposals(dir)? {
        if existing.is_sealed() {
            anyhow::bail!(
                "llm-proposals.json is SEALED for this directory (outcome: {}): D6 has no \
                 repair loop — a parse violation or a successful record is terminal for \
                 this experiment line. A new completion needs a fresh --llm-artifacts \
                 directory.",
                if existing.parse_error.is_some() {
                    "parse violation (proposal=null)"
                } else {
                    "recorded proposal"
                }
            );
        }
    }

    let parse_result = raw.clone().into_proposal();
    let (proposal, parse_error) = match parse_result {
        Ok((p, _)) => (serde_json::to_value(&p).ok(), None),
        // Ham yanıt llm-raw-response.txt'te ayrıca durur (D6) — parse_error
        // yalnız hitalet sebebidir.
        Err((_raw_text, e)) => (None, Some(format!("{e}"))),
    };
    let record = ProposalsDoc {
        schema_version: 1,
        usage: Some(raw.usage),
        proposal,
        parse_error,
        transport_error: None,
        network_retry_count,
    };
    atomic_replace(&dir.join("llm-raw-response.txt"), raw.content.as_bytes())?;
    write_proposals_doc(dir, &record)?;
    Ok(())
}

/// D6 transport tükenmesi kaydı — iki ağ denemesi de yanıt alamadı: `usage`
/// bilinmiyor (null), `proposal` null, `transport_error` + retry sayısı
/// kayıtta. Bu TEK durum aynı-prompt ağ retry'sına açıktır (yeni süreçte
/// [`record_proposal_completion`] transport-null kaydın üzerine yazabilir).
pub fn record_transport_failure(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    transport_error: &str,
    network_retry_count: u32,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let doc = prompt_doc("llm-proposal-prompt", cfg, req, manifest);
    freeze_prompt(dir, "llm-prompt.json", &doc)?;
    if let Some(existing) = existing_proposals(dir)? {
        if existing.is_sealed() {
            anyhow::bail!(
                "llm-proposals.json is SEALED for this directory — no further records \
                 (D6 terminal outcome)"
            );
        }
    }
    let record = ProposalsDoc {
        schema_version: 1,
        usage: None,
        proposal: None,
        parse_error: None,
        transport_error: Some(transport_error.to_string()),
        network_retry_count,
    };
    write_proposals_doc(dir, &record)
}

fn write_proposals_doc(dir: &Path, record: &ProposalsDoc) -> anyhow::Result<()> {
    atomic_replace(
        &dir.join("llm-proposals.json"),
        &serde_json::to_vec_pretty(record)?,
    )
}

/// D4 bar elicitation kaydı — iki dosya: donmuş anchor
/// (`llm-bar-prompt.json`) + yanıt kaydı (`llm-bar-response.json`).
/// Transport hatası ve parse ihlali de DÜRÜST boşluk olarak yazılır; parse
/// ihlali ve elicited-success sonrası yanıt kaydı mühürlenir (yeniden
/// örnekleme = cherry-pick, yasak).
#[allow(clippy::too_many_arguments)]
pub fn record_bar_elicitation(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    raw: Option<&RawCompletion>,
    elicited: Option<&ElicitedBar>,
    parse_error: Option<&str>,
    transport_error: Option<&str>,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let doc = prompt_doc("llm-bar-elicitation-prompt", cfg, req, manifest);
    freeze_prompt(dir, "llm-bar-prompt.json", &doc)?;

    let response_path = dir.join("llm-bar-response.json");
    if response_path.exists() {
        let existing_raw = std::fs::read_to_string(&response_path)?;
        let existing: BarResponseDoc = serde_json::from_str(&existing_raw).map_err(|e| {
            anyhow::anyhow!(
                "llm-bar-response.json exists but is unreadable: {e} — resolve manually"
            )
        })?;
        if existing.is_sealed() {
            anyhow::bail!(
                "llm-bar-response.json is SEALED for this directory (outcome: {}): bar \
                 re-elicitation after a parse violation or a recorded bar is resampling \
                 — cherry-pick, forbidden by the preregistration protocol. Use a fresh \
                 --llm-artifacts directory.",
                if existing.parse_error.is_some() {
                    "parse violation"
                } else {
                    "recorded bar"
                }
            );
        }
    }

    let record = BarResponseDoc {
        schema_version: 1,
        kind: "llm-bar-elicitation".to_string(),
        prompt_digest: doc.prompt_digest.clone(),
        completion_identity_digest: doc.completion_identity_digest.clone(),
        usage: raw.map(|r| r.usage),
        raw_response: raw.map(|r| r.content.clone()),
        elicited: elicited.cloned(),
        parse_error: parse_error.map(str::to_string),
        transport_error: transport_error.map(str::to_string),
    };
    atomic_replace(&response_path, &serde_json::to_vec_pretty(&record)?)
}

// ── atomik yazım çekirdekleri ──────────────────────────────────────────────────

/// Unique same-dir temp (`create_new`) → write+sync → rename-replace.
/// Düz `fs::write`'ın truncate penceresini kapatır (osp-cli stage_temp emsali).
fn atomic_replace(out: &Path, payload: &[u8]) -> anyhow::Result<()> {
    let tmp = stage_temp(out, payload)?;
    if let Err(e) = std::fs::rename(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        anyhow::bail!("rename failed for {}: {e}", out.display());
    }
    Ok(())
}

/// No-clobber publish (freeze): temp + sync + `hard_link` (`fs::rename` hedefi
/// REPLACE eder — freeze'e uygun değil; osp-cli canonical-store emsali).
fn no_clobber_publish(out: &Path, payload: &[u8]) -> anyhow::Result<()> {
    let tmp = stage_temp(out, payload)?;
    match std::fs::hard_link(&tmp, out) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Yarış penceresi: başka process aynı anda dondurdu — çağıranın
            // verify dalı bunu digest karşılaştırmasıyla çözer; burada kalan
            // temp temizlenir ve mevcut dosya güvenilebilir kabul EDİLEMEZ,
            // bu yüzden hata verip doğrulama dalına bırak.
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!(
                "frozen prompt anchor appeared concurrently while publishing {}; re-run \
                 to verify the frozen digests match",
                out.display()
            );
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("no-clobber publish failed for {}: {e}", out.display());
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

fn stage_temp(out: &Path, payload: &[u8]) -> anyhow::Result<std::path::PathBuf> {
    let dir = out
        .parent()
        .ok_or_else(|| anyhow::anyhow!("artifact path {} has no parent", out.display()))?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    for suffix in 0..=64u32 {
        let name = match suffix {
            0 => format!(".llm-artifacts-tmp-{pid}-{millis}"),
            n => format!(".llm-artifacts-tmp-{pid}-{millis}-{n}"),
        };
        let candidate = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(payload).and_then(|_| file.sync_all()) {
                    let _ = std::fs::remove_file(&candidate);
                    anyhow::bail!("temp write failed for {}: {e}", out.display());
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => anyhow::bail!("temp open failed for {}: {e}", out.display()),
        }
    }
    anyhow::bail!("temp collision budget exhausted for {}", out.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(model: &str) -> RuntimeConfig {
        RuntimeConfig {
            model: model.to_string(),
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: "sk-test".into(),
            timeout: std::time::Duration::from_secs(1),
            temperature: 0.3,
            max_tokens: 500,
        }
    }

    fn manifest() -> InvE1Manifest {
        InvE1Manifest {
            task_digest: Some(format!("sha256:{}", "1a".repeat(32))),
            baseline_digest: "0".repeat(40),
            osp_revision: "test-rev".into(),
            files_read: vec![],
        }
    }

    fn req(system: &str, user: &str) -> CompletionRequest {
        CompletionRequest {
            system: system.to_string(),
            user: user.to_string(),
        }
    }

    fn raw(content: &str) -> RawCompletion {
        RawCompletion {
            usage: TokenUsage {
                prompt_tokens: 11,
                completion_tokens: 7,
                total_tokens: 18,
            },
            content: content.to_string(),
        }
    }

    fn valid_proposal_json() -> String {
        r#"{"new_nodes":[],"new_edges":[],"removed_edges":[{"from":0,"to":1,"kind":"Imports"}],"affected_nodes":[0],"modified_entities":[],"position_hints":[],"reasoning":"r"}"#
            .to_string()
    }

    fn read_json(dir: &Path, name: &str) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    }

    // ── digest'ler ────────────────────────────────────────────────────────────

    #[test]
    fn prompt_digest_known_answer_vector() {
        // serde_json::to_string(PromptPair) === {"system":"s","user":"u"}
        // (alan sırası sabit) — sha256 KAT py ile bağımsız hesaplandı.
        let d = completion_prompt_digest(&req("s", "u"));
        assert_eq!(
            d,
            "sha256:f47adadc6fb3ce18459e9a4cf1453a8f924d48a7bd3eef53e8f7ec31a137ebbf"
        );
    }

    #[test]
    fn prompt_digest_deterministic_and_input_sensitive() {
        let a = completion_prompt_digest(&req("s", "u"));
        assert_eq!(a, completion_prompt_digest(&req("s", "u")));
        assert_ne!(a, completion_prompt_digest(&req("s", "v")));
        assert_ne!(a, completion_prompt_digest(&req("t", "u")));
        // Concatenation sınırı: JSON escaping sınırı netleştirir ("ab"+"c" ≠ "a"+"bc").
        assert_ne!(
            completion_prompt_digest(&req("ab", "c")),
            completion_prompt_digest(&req("a", "bc"))
        );
    }

    #[test]
    fn completion_identity_digest_known_answer_vector() {
        // {"model":"m","endpoint":"e","temperature":0.3,"max_tokens":500,
        //  "prompt_digest":"sha256:ab…"} — py ile bağımsız hesaplandı.
        let mut c = cfg("m");
        c.endpoint = "e".into();
        let pd = format!("sha256:{}", "ab".repeat(32));
        assert_eq!(
            completion_identity_digest(&c, &pd),
            "sha256:278bd7c5cd8bc972869d103f949ea2f64151420ba5c9a08ccb137a121aee1f5d"
        );
    }

    #[test]
    fn completion_identity_digest_binds_model_and_sampling() {
        // Review P1: aynı prompt (digest) + farklı model/config → farklı kimlik.
        let base = cfg("gpt-4o-mini");
        let pd = completion_prompt_digest(&req("s", "u"));
        let id1 = completion_identity_digest(&base, &pd);

        let mut other_model = cfg("gpt-4.1-mini");
        other_model.endpoint = base.endpoint.clone();
        assert_ne!(id1, completion_identity_digest(&other_model, &pd));

        let mut other_temp = base.clone();
        other_temp.temperature = 0.7;
        assert_ne!(id1, completion_identity_digest(&other_temp, &pd));

        let mut other_tokens = base.clone();
        other_tokens.max_tokens = 1000;
        assert_ne!(id1, completion_identity_digest(&other_tokens, &pd));

        let mut other_endpoint = base.clone();
        other_endpoint.endpoint = "http://localhost:1".into();
        assert_ne!(id1, completion_identity_digest(&other_endpoint, &pd));
    }

    #[test]
    fn inv_e1_doc_carries_view_digest_and_blindness_record() {
        let d = InvE1Doc::from_manifest(&manifest(), &req("sys", "user-view"));
        assert_eq!(
            d.agent_task_view_digest,
            "sha256:992e38202478bb595829b0caba4fe81ed8cb6e306a305b06f999121bb4da82c2",
            "view digest = sha256(user payload) KAT"
        );
        assert!(
            d.files_read.is_empty(),
            "procedural blindness pozitif kaydı"
        );
        assert_eq!(d.osp_revision, "test-rev");
    }

    // ── record_proposal_completion ────────────────────────────────────────────

    #[test]
    fn record_first_completion_freezes_anchor_and_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        let response = raw(&valid_proposal_json());
        record_proposal_completion(dir.path(), &cfg("m"), &request, &response, 0, &manifest())
            .unwrap();

        let prompt = read_json(dir.path(), "llm-prompt.json");
        assert_eq!(prompt["kind"], "llm-proposal-prompt");
        assert_eq!(prompt["model"], "m");
        assert_eq!(prompt["temperature"], 0.3);
        assert_eq!(prompt["max_tokens"], 500);
        assert_eq!(
            prompt["prompt_digest"],
            completion_prompt_digest(&request).as_str()
        );
        assert_eq!(
            prompt["completion_identity_digest"],
            completion_identity_digest(&cfg("m"), &completion_prompt_digest(&request)).as_str()
        );
        assert_eq!(prompt["inv_e1"]["osp_revision"], "test-rev");
        assert_eq!(
            prompt["inv_e1"]["files_read"].as_array().map(Vec::len),
            Some(0)
        );

        let raw_text = std::fs::read_to_string(dir.path().join("llm-raw-response.txt")).unwrap();
        assert_eq!(raw_text, valid_proposal_json());

        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert_eq!(proposals["usage"]["total_tokens"], 18);
        assert_eq!(proposals["network_retry_count"], 0);
        assert_eq!(proposals["proposal"]["reasoning"], "r");
    }

    #[test]
    fn record_parse_failure_is_honest_null_and_seals_directory() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &request,
            &raw("not json at all"),
            0,
            &manifest(),
        )
        .unwrap();
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert!(
            proposals["proposal"].is_null(),
            "D6: tamir yok, null dürüst"
        );
        assert!(proposals["parse_error"].is_string());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("llm-raw-response.txt")).unwrap(),
            "not json at all"
        );

        // D6 (review P0): parse ihlali TERMINAL — aynı dizinde sonra gelen
        // (başarılı bile olsa) kayıt REDDEDİLİR: artifact truth == executed truth.
        let err = record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("SEALED"),
            "parse sonrası mühür: {err:#}"
        );
        // Kayıt değişmedi:
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert!(proposals["proposal"].is_null());
    }

    #[test]
    fn record_success_seals_directory_against_rerun() {
        // Başarı da mühürlüdür: aynı prompt'la yeniden koşu kanıtı ezemez.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap();
        let err = record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("SEALED"));
    }

    #[test]
    fn record_transport_failure_then_same_prompt_retry_succeeds() {
        // D6'ın TEK açık durumu: iki ağ denemesi de tükenmiş → transport-null
        // kaydı; aynı-prompt retry başarıyla gelirse kayıt güncellenir.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        record_transport_failure(
            dir.path(),
            &cfg("m"),
            &request,
            "attempt 1: connect timeout; attempt 2: connection reset",
            1,
            &manifest(),
        )
        .unwrap();
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert!(proposals["proposal"].is_null());
        assert!(proposals["usage"].is_null(), "yanıt yok — usage bilinmez");
        assert_eq!(proposals["network_retry_count"], 1);
        assert!(proposals["transport_error"].is_string());

        // Aynı-prompt retry (D6): kayıt güncellenebilir.
        record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap();
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert_eq!(proposals["proposal"]["reasoning"], "r");
        assert!(proposals["transport_error"].is_null());
    }

    #[test]
    fn record_different_prompt_refused_by_freeze() {
        let dir = tempfile::tempdir().unwrap();
        record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &req("sys", "user"),
            &raw("first"),
            0,
            &manifest(),
        )
        .unwrap();
        let err = record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &req("sys", "DIFFERENT"),
            &raw("second"),
            0,
            &manifest(),
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("already FROZEN") && msg.contains("INV-E1"),
            "educational freeze error, got: {msg}"
        );
    }

    #[test]
    fn record_same_prompt_different_model_refused_by_identity_freeze() {
        // Review P1: prompt aynı, model farklı → treatment değişimi → RED.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        record_proposal_completion(
            dir.path(),
            &cfg("gpt-4o-mini"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap();
        let err = record_proposal_completion(
            dir.path(),
            &cfg("different-model"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("completion identity"),
            "identity fence mesajı: {msg}"
        );
    }

    #[test]
    fn record_corrupt_frozen_prompt_surfaces_read_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("llm-prompt.json"), "not json").unwrap();
        let err = record_proposal_completion(
            dir.path(),
            &cfg("m"),
            &req("sys", "user"),
            &raw("r"),
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("unreadable"),
            "corrupt freeze anchor is a loud error"
        );
    }

    // ── bar elicitation ───────────────────────────────────────────────────────

    #[test]
    fn parse_elicited_bar_plain_and_fenced() {
        let plain = r#"{"bar_value":0.55,"predicted":[{"label":"remove b import","predicted_coupling":0.5}]}"#;
        let bar = parse_elicited_bar(plain).unwrap();
        assert_eq!(bar.bar_value, 0.55);
        assert_eq!(bar.predicted.len(), 1);
        assert_eq!(bar.predicted[0].label, "remove b import");

        let fenced = format!("```json\n{plain}\n```");
        assert_eq!(parse_elicited_bar(&fenced).unwrap(), bar);
        assert!(parse_elicited_bar("nope").is_err());
    }

    #[test]
    fn bar_prompt_freezes_once_response_is_separate_record() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys-bar", "user-view");
        let response = raw(r#"{"bar_value":0.6,"predicted":[]}"#);
        let elicited = parse_elicited_bar(&response.content).unwrap();
        record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
            &manifest(),
        )
        .unwrap();

        let prompt = read_json(dir.path(), "llm-bar-prompt.json");
        assert_eq!(prompt["kind"], "llm-bar-elicitation-prompt");
        let bar = read_json(dir.path(), "llm-bar-response.json");
        assert_eq!(bar["kind"], "llm-bar-elicitation");
        assert_eq!(bar["elicited"]["bar_value"], 0.6);
        assert_eq!(
            bar["prompt_digest"],
            completion_prompt_digest(&request).as_str()
        );

        // Review P1 regression: elicited kaydı sonrası yeniden örnekleme RED.
        let err = record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("SEALED") && format!("{err:#}").contains("resampling"),
            "seal message: {err:#}"
        );
    }

    #[test]
    fn bar_transport_failure_then_same_prompt_retry_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys-bar", "user-view");
        record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            None,
            None,
            None,
            Some("connection reset"),
            &manifest(),
        )
        .unwrap();
        let bar = read_json(dir.path(), "llm-bar-response.json");
        assert_eq!(bar["transport_error"], "connection reset");
        assert!(bar["elicited"].is_null());

        // Aynı bar prompt'uyla retry → açık durum, kayıt güncellenir.
        let response = raw(r#"{"bar_value":0.7,"predicted":[]}"#);
        let elicited = parse_elicited_bar(&response.content).unwrap();
        record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
            &manifest(),
        )
        .unwrap();
        let bar = read_json(dir.path(), "llm-bar-response.json");
        assert_eq!(bar["elicited"]["bar_value"], 0.7);
    }

    #[test]
    fn bar_different_prompt_refused_by_freeze() {
        let dir = tempfile::tempdir().unwrap();
        record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &req("sys-bar", "user-view"),
            None,
            None,
            None,
            Some("x"),
            &manifest(),
        )
        .unwrap();
        let err = record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &req("sys-bar-DIFFERENT", "user-view"),
            None,
            None,
            None,
            Some("x"),
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("already FROZEN"),
            "farklı bar prompt'u red: {err:#}"
        );
    }
}
