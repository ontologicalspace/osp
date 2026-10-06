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
        // Review R2 P1: INV-E1 provenance eşitliği — donmuş anchor'ın "bu prompt
        // ŞU girdilerden üretildi" iddiası da donmuştur; aynı prompt + aynı
        // config + FARKLI girdi bağlamı (baseline/task/osp_revision/files_read)
        // aynı deney satırı OLAMAZ.
        if existing.inv_e1 != doc.inv_e1 {
            anyhow::bail!(
                "{file_name} is already FROZEN with different INV-E1 provenance \
                 (frozen {:?}; new {:?}). The frozen anchor records WHICH inputs \
                 produced this prompt (agent-task view, task, baseline, osp revision, \
                 files read): reusing the directory from a different context would \
                 attach new evidence to someone else's pre-registration. Use a fresh \
                 --llm-artifacts directory.",
                existing.inv_e1,
                doc.inv_e1
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
    /// Kayıt mühürlü mü? D6 matrisi (review R2): HER sonuç kaydı terminaldir —
    /// parse-null, success VE transport-exhausted. "Bir retry" bütçesi
    /// EXPERIMENT-LINE bütçesidir, process-local değil: iki ağ denemesi de
    /// tükendiyse satır tamamlanmıştır; aynı dizinde üçüncü çağrı RED.
    fn is_sealed(&self) -> bool {
        self.parse_error.is_some() || self.proposal.is_some() || self.transport_error.is_some()
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
    /// D6 "retry kayda geçer" — bar completion'ı da kapsar (review R2).
    #[serde(default)]
    network_retry_count: u32,
}

impl BarResponseDoc {
    /// Elicitation mühürlü mü? Proposals ile aynı D6 matrisi (review R2): her
    /// sonuç terminaldir — parse-null, elicited-success VE transport-exhausted.
    /// Transport sonrası yeniden elicitation = yeniden örnekleme (cherry-pick).
    fn is_sealed(&self) -> bool {
        self.parse_error.is_some() || self.elicited.is_some() || self.transport_error.is_some()
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

fn proposals_seal_message(existing: &ProposalsDoc) -> String {
    let outcome = if existing.parse_error.is_some() {
        "parse violation (proposal=null)"
    } else if existing.proposal.is_some() {
        "recorded proposal"
    } else {
        "transport-exhausted (both network attempts failed)"
    };
    format!(
        "llm-proposals.json is SEALED for this directory (outcome: {outcome}): D6's \
         one-retry budget is an EXPERIMENT-LINE budget, not a process budget — a parse \
         violation, a successful record, or transport exhaustion are all terminal for \
         this line. A new completion needs a fresh --llm-artifacts directory."
    )
}

// ── experiment-line claim (P0 sıralama + P1 tek-yazıcı sahipliği) ─────────────

/// No-clobber line-claim kaydı: çağrıdan ÖNCE atomik olarak alınır; outcome
/// kaydı claim'i gerektirir. Bir kez alındığında asla geri bırakılmaz —
/// outcome'suz claim (crash penceresi) "unknown realization" olarak satırı
/// yakar: model örnekleme yapmış OLABİLİR, yeniden koşu = ek örnekleme.
#[derive(Serialize, Deserialize, Debug)]
struct LineClaimDoc {
    schema_version: u32,
    kind: String,
    prompt_digest: String,
    completion_identity_digest: String,
    claimed_at_unix_millis: u128,
    pid: u32,
}

fn claim_doc(kind: &str, doc: &PromptDoc) -> LineClaimDoc {
    LineClaimDoc {
        schema_version: 1,
        kind: kind.to_string(),
        prompt_digest: doc.prompt_digest.clone(),
        completion_identity_digest: doc.completion_identity_digest.clone(),
        claimed_at_unix_millis: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        pid: std::process::id(),
    }
}

fn claim_path(dir: &Path, file_name: &str) -> std::path::PathBuf {
    dir.join(file_name)
}

/// Claim'in varlığı RED mesajı — iki olası anlam: (i) başka bir süreç hattı
/// SAHİPLENMİŞ durumda (concurrent writer; P1), (ii) claim alan süreç outcome
/// yazamadan öldü (crash; unknown realization — P0 penceresi).
fn claimed_message(path: &std::path::Path) -> String {
    format!(
        "experiment line is CLAIMED ({}): the line was claimed before an LLM call — \
         either another process owns it right now (single-writer invariant) or the \
         claiming process died before recording an outcome (the model MAY have \
         sampled; rerunning would add an extra realization). Either way this \
         directory is burned for further calls — use a fresh --llm-artifacts \
         directory.",
        path.display()
    )
}

/// **PRE-CALL (review R3 P0+P1):** proposal completion'ından ÖNCE —
/// (1) outcome kaydı var mı (SEALED → RED); (2) donmuş prompt anchor'U
/// **çağrı öncesi** doğrula-veya-dondur (llm-prompt.json; üç digest kapısı:
/// prompt, completion identity, inv_e1 — farklı prompt/config/provenance en
/// spesifik hata ile RED'lenir); (3) claim var mı (RED — sahipli ya da
/// unknown realization); (4) `llm-line-claim.json`'ı **no-clobber** yayınla —
/// atomik sahiplenme (yarışan ikinci süreç AlreadyExists alır → RED).
///
/// Sıra sözleşmesi: bu fonksiyon dönmeden HTTP çağrısı YAPILMAZ; outcome
/// kayıtları claim'in varlığını YAPISAL olarak gerektirir.
pub fn freeze_and_claim_proposal_line(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    if let Some(existing) = existing_proposals(dir)? {
        if existing.is_sealed() {
            anyhow::bail!("{}", proposals_seal_message(&existing));
        }
    }
    let doc = prompt_doc("llm-proposal-prompt", cfg, req, manifest);
    freeze_prompt(dir, "llm-prompt.json", &doc)?;
    let claim_file = claim_path(dir, "llm-line-claim.json");
    if claim_file.exists() {
        anyhow::bail!("{}", claimed_message(&claim_file));
    }
    let claim = claim_doc("proposal-line-claim", &doc);
    match no_clobber_publish(&claim_file, &serde_json::to_vec_pretty(&claim)?) {
        Ok(()) => Ok(()),
        // Yarış: karşı süreç claim'i bizden önce aldı — single-writer kazandı.
        Err(e) if format!("{e:#}").contains("concurrently") => {
            anyhow::bail!("{}", claimed_message(&claim_file))
        }
        Err(e) => Err(e),
    }
}

/// Bar kanalının PRE-CALL claim'i — aynı sıra sözleşmesi (llm-bar-prompt.json
/// + llm-bar-line-claim.json). Outcome'suz bar claim'i = unknown realization.
pub fn freeze_and_claim_bar_line(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let response_path = dir.join("llm-bar-response.json");
    if response_path.exists() {
        let raw = std::fs::read_to_string(&response_path)?;
        let existing: BarResponseDoc = serde_json::from_str(&raw).map_err(|e| {
            anyhow::anyhow!(
                "llm-bar-response.json exists but is unreadable: {e} — resolve manually"
            )
        })?;
        if existing.is_sealed() {
            anyhow::bail!("{}", bar_seal_message(&existing));
        }
    }
    let doc = prompt_doc("llm-bar-elicitation-prompt", cfg, req, manifest);
    freeze_prompt(dir, "llm-bar-prompt.json", &doc)?;
    let claim_file = claim_path(dir, "llm-bar-line-claim.json");
    if claim_file.exists() {
        anyhow::bail!("{}", claimed_message(&claim_file));
    }
    let claim = claim_doc("bar-line-claim", &doc);
    match no_clobber_publish(&claim_file, &serde_json::to_vec_pretty(&claim)?) {
        Ok(()) => Ok(()),
        Err(e) if format!("{e:#}").contains("concurrently") => {
            anyhow::bail!("{}", claimed_message(&claim_file))
        }
        Err(e) => Err(e),
    }
}

/// Outcome kaydının claim şartı (YAPISAL P0 sıralaması): claim yoksa outcome
/// YAZILAMAZ — "çağrı öncesi donmuştu" iddiası olmayan kanıt üretilmez.
/// Claim'in digest'leri bu completion'ın digest'leriyle eşleşmelidir.
fn require_claim(dir: &Path, claim_file: &str, doc: &PromptDoc) -> anyhow::Result<()> {
    let path = claim_path(dir, claim_file);
    let raw = std::fs::read_to_string(&path).map_err(|_| {
        anyhow::anyhow!(
            "outcome record requires a PRE-CALL line claim ({}): none found — the \
             freeze→claim→HTTP→outcome ordering is structural (#171 review R3 P0); \
             refusing to write post-hoc evidence",
            path.display()
        )
    })?;
    let claim: LineClaimDoc = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("line claim {} is unreadable: {e}", path.display()))?;
    if claim.prompt_digest != doc.prompt_digest
        || claim.completion_identity_digest != doc.completion_identity_digest
    {
        anyhow::bail!(
            "line claim digest mismatch ({}): claimed prompt {}, this completion {} — \
             outcome belongs to a different claim",
            path.display(),
            claim.prompt_digest,
            doc.prompt_digest
        );
    }
    Ok(())
}

/// Outcome yazarı için anchor doğrulaması — VERIFY-ONLY (create YOK): P0
/// sıralamasının record tarafı. Anchor çağrı-öncesi claim tarafından
/// dondurulmuş OLMALIYDI; burada yoksa bu protokol ihlalidir.
fn verify_frozen_anchor(dir: &Path, file_name: &str, doc: &PromptDoc) -> anyhow::Result<()> {
    let path = dir.join(file_name);
    if !path.exists() {
        anyhow::bail!(
            "{file_name} must be frozen BEFORE the call (by freeze_and_claim_*): found \
             none at outcome time — post-hoc anchor creation is forbidden (#171 R3 P0)"
        );
    }
    freeze_prompt(dir, file_name, doc) // var olan anchor ile üç-digest doğrulama
}

/// Outcome kaydı: yanıt ALINMIŞ proposal completion'ı (first-freeze; §5+D6).
/// HER sonuç terminaldir (parse-null / success / transport-exhausted mühürler).
/// Pre-call claim + donmuş anchor şart (P0); disk hatası sessiz unpersisted
/// çağrıya izin vermez.
pub fn record_proposal_outcome(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    raw: &RawCompletion,
    network_retry_count: u32,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let doc = prompt_doc("llm-proposal-prompt", cfg, req, manifest);
    require_claim(dir, "llm-line-claim.json", &doc)?;
    verify_frozen_anchor(dir, "llm-prompt.json", &doc)?;

    if let Some(existing) = existing_proposals(dir)? {
        if existing.is_sealed() {
            anyhow::bail!("{}", proposals_seal_message(&existing));
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

/// Yanıtsız terminal çağrı hatası outcome'u — iki yol (review R2):
/// (i) retryable iken iki deneme de tükendi (`network_retry_count` = 1);
/// (ii) sınıflandırıcı non-retryable buldu (401/400…; retry YAPILMADI, = 0).
/// Her iki durum da kaydı mühürler. Pre-call claim şart (P0).
pub fn record_call_failure(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    call_error: &str,
    network_retry_count: u32,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let doc = prompt_doc("llm-proposal-prompt", cfg, req, manifest);
    require_claim(dir, "llm-line-claim.json", &doc)?;
    verify_frozen_anchor(dir, "llm-prompt.json", &doc)?;
    if let Some(existing) = existing_proposals(dir)? {
        if existing.is_sealed() {
            anyhow::bail!("{}", proposals_seal_message(&existing));
        }
    }
    let record = ProposalsDoc {
        schema_version: 1,
        usage: None,
        proposal: None,
        parse_error: None,
        transport_error: Some(call_error.to_string()),
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

fn bar_seal_message(existing: &BarResponseDoc) -> String {
    let outcome = if existing.parse_error.is_some() {
        "parse violation"
    } else if existing.elicited.is_some() {
        "recorded bar"
    } else {
        "transport-exhausted (both network attempts failed)"
    };
    format!(
        "llm-bar-response.json is SEALED for this directory (outcome: {outcome}): bar \
         re-elicitation after any terminal outcome is resampling — cherry-pick, \
         forbidden by the preregistration protocol. Use a fresh --llm-artifacts \
         directory."
    )
}

/// D4 bar elicitation outcome'u — donmuş anchor (`llm-bar-prompt.json`,
/// pre-call) + yanıt kaydı (`llm-bar-response.json`). Transport hatası ve
/// parse ihlali de DÜRÜST boşluk olarak yazılır; HER sonuç mühürler.
/// Pre-call bar claim şart (P0).
#[allow(clippy::too_many_arguments)]
pub fn record_bar_elicitation(
    dir: &Path,
    cfg: &RuntimeConfig,
    req: &CompletionRequest,
    raw: Option<&RawCompletion>,
    elicited: Option<&ElicitedBar>,
    parse_error: Option<&str>,
    transport_error: Option<&str>,
    network_retry_count: u32,
    manifest: &InvE1Manifest,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let doc = prompt_doc("llm-bar-elicitation-prompt", cfg, req, manifest);
    require_claim(dir, "llm-bar-line-claim.json", &doc)?;
    verify_frozen_anchor(dir, "llm-bar-prompt.json", &doc)?;

    let response_path = dir.join("llm-bar-response.json");
    if response_path.exists() {
        let existing_raw = std::fs::read_to_string(&response_path)?;
        let existing: BarResponseDoc = serde_json::from_str(&existing_raw).map_err(|e| {
            anyhow::anyhow!(
                "llm-bar-response.json exists but is unreadable: {e} — resolve manually"
            )
        })?;
        if existing.is_sealed() {
            anyhow::bail!("{}", bar_seal_message(&existing));
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
        network_retry_count,
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

    /// Üretim akışının test içi birebiri: PRE-CALL claim (anchor + sahiplenme).
    fn claim_proposal(dir: &Path, model: &str, request: &CompletionRequest) {
        freeze_and_claim_proposal_line(dir, &cfg(model), request, &manifest())
            .expect("pre-call claim");
    }

    fn claim_bar(dir: &Path, model: &str, request: &CompletionRequest) {
        freeze_and_claim_bar_line(dir, &cfg(model), request, &manifest())
            .expect("pre-call bar claim");
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

    // ── PRE-CALL: freeze + claim (review R3 P0+P1) ─────────────────────────────

    #[test]
    fn pre_call_claim_writes_anchor_and_claim_before_any_outcome() {
        // R3 P0 pin: anchor + claim, outcome'tan ÖNCE (burada hiç outcome yok)
        // diskte durur — "freeze önce kanıtı" sıralamasının birim karşılığı.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);

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

        let claim = read_json(dir.path(), "llm-line-claim.json");
        assert_eq!(claim["kind"], "proposal-line-claim");
        assert_eq!(
            claim["prompt_digest"],
            completion_prompt_digest(&request).as_str()
        );

        // Outcome henüz YOK — satır claim'li ama kanıt boş (crash penceresi
        // artık görünürdür: outcome'suz claim dizini yakar, aşağıda).
        assert!(!dir.path().join("llm-proposals.json").exists());
    }

    #[test]
    fn claim_without_outcome_burns_line_unknown_realization() {
        // R3 P0 crash penceresi: claim alındı, süreç outcome yazamadan öldü.
        // Model örnekleme yapmış OLABİLİR → satır yeniden koşulamaz (yeniden
        // koşu = ekstrealizasyon; iz artık claim dosyasında durur).
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);

        let err = freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &request, &manifest())
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("CLAIMED") && msg.contains("MAY have sampled"),
            "claim'li satır RED: {msg}"
        );
    }

    #[test]
    fn second_concurrent_claim_is_refused_single_writer() {
        // R3 P1: aynı anda iki süreç aynı hattı sahiplenemez — no-clobber
        // publish atomiktir; ikinci claim AlreadyExists ile RED (sıralı iki
        // çağrı yarışı deterministik çözümler).
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);
        let err = freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &request, &manifest())
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("CLAIMED"),
            "single-writer: {err:#}"
        );
    }

    #[test]
    fn outcome_without_pre_call_claim_is_refused() {
        // R3 P0 yapısal sıralama: claim'siz outcome YAZILAMAZ — çağrı-sonrası
        // "frozen" kanıt üretimi engellenir.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        let err = record_proposal_outcome(
            dir.path(),
            &cfg("m"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("PRE-CALL line claim"),
            "claim şartı: {err:#}"
        );
        let err =
            record_call_failure(dir.path(), &cfg("m"), &request, "x", 0, &manifest()).unwrap_err();
        assert!(
            format!("{err:#}").contains("PRE-CALL line claim"),
            "failure kaydı da claim ister: {err:#}"
        );
        // Bar tarafı aynı şekilde.
        let err = record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &req("sys-bar", "user-view"),
            None,
            None,
            None,
            Some("x"),
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("PRE-CALL line claim"));
    }

    // ── outcome kayıtları (claim'li akış) ─────────────────────────────────────

    #[test]
    fn record_first_outcome_freezes_anchor_and_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        let response = raw(&valid_proposal_json());
        claim_proposal(dir.path(), "m", &request);
        record_proposal_outcome(dir.path(), &cfg("m"), &request, &response, 0, &manifest())
            .unwrap();

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
        claim_proposal(dir.path(), "m", &request);
        record_proposal_outcome(
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

        // D6: parse ihlali TERMINAL — aynı dizinde sonra gelen (başarılı bile
        // olsa) outcome REDDEDİLİR: artifact truth == executed truth.
        let err = record_proposal_outcome(
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
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert!(proposals["proposal"].is_null());
    }

    #[test]
    fn record_success_seals_directory_against_rerun() {
        // Başarı da mühürlüdür: aynı prompt'la yeniden koşu kanıtı ezemez.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);
        record_proposal_outcome(
            dir.path(),
            &cfg("m"),
            &request,
            &raw(&valid_proposal_json()),
            0,
            &manifest(),
        )
        .unwrap();
        // Yeniden koşuş önce CLAIM aşamasında RED (sealed outcome kontrolü).
        let err = freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &request, &manifest())
            .unwrap_err();
        assert!(format!("{err:#}").contains("SEALED"));
    }

    #[test]
    fn transport_exhausted_seals_experiment_line() {
        // R2 P1: "bir retry" EXPERIMENT-LINE bütçesidir — iki retryable deneme
        // de tükendiyse kayıt MÜHÜRLÜ; aynı dizinde üçüncü çağrı (yeni
        // process'ten bile) RED. Process-restart ile bütçe sıfırlanamaz.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);
        record_call_failure(
            dir.path(),
            &cfg("m"),
            &request,
            "attempt 1 (retryable): timeout; attempt 2 (retryable): reset",
            1,
            &manifest(),
        )
        .unwrap();
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert!(proposals["proposal"].is_null());
        assert!(proposals["usage"].is_null(), "yanıt yok — usage bilinmez");
        assert_eq!(proposals["network_retry_count"], 1);
        assert!(proposals["transport_error"].is_string());

        // Yeniden koşuş claim aşamasında RED — HTTP çağrısı yapılmaz.
        let err = freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &request, &manifest())
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("SEALED"),
            "sealed line: {err:#}"
        );
        // Outcome yazarı da (çift katman) RED — kayıt değişmedi.
        let err = record_proposal_outcome(
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
            "transport-exhausted mühür: {err:#}"
        );
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert!(proposals["proposal"].is_null(), "kayıt değişmedi");
    }

    #[test]
    fn non_retryable_failure_records_with_zero_retries_and_seals() {
        // Sınıflandırıcı reddi (ör. 401): retry YAPILMADI → network_retry_count=0;
        // sonuç yine terminal — kayıt mühürlenir.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);
        record_call_failure(
            dir.path(),
            &cfg("m"),
            &request,
            "non-retryable (api status 401: unauthorized) — no retry attempted",
            0,
            &manifest(),
        )
        .unwrap();
        let proposals = read_json(dir.path(), "llm-proposals.json");
        assert_eq!(proposals["network_retry_count"], 0, "retry yapılmadı");
        assert!(proposals["transport_error"].is_string());
        assert!(proposals["proposal"].is_null());
        assert!(
            freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &request, &manifest()).is_err(),
            "401 sonrası da satır mühürlü"
        );
    }

    #[test]
    fn record_different_prompt_refused_by_freeze_at_claim() {
        // Farklı prompt aynı dizinde: CLAIM aşamasında (çağrıdan ÖNCE, outcome
        // daha yokken) RED — donmuş anchor kapısı en spesifik hatayı verir.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);
        let err = freeze_and_claim_proposal_line(
            dir.path(),
            &cfg("m"),
            &req("sys", "DIFFERENT"),
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
    fn same_prompt_different_model_refused_by_identity_freeze_at_claim() {
        // R1 P1: prompt aynı, model farklı → treatment değişimi → RED (claim'de).
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "gpt-4o-mini", &request);
        let err = freeze_and_claim_proposal_line(
            dir.path(),
            &cfg("different-model"),
            &request,
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
    fn claim_refuses_different_inv_e1_provenance() {
        // R2 P1: aynı prompt + aynı completion identity + FARKLI girdi bağlamı
        // → RED (claim'de, çağrıdan önce — donmuş anchor KİMİN girdilerinden
        // üretildiğini taşır).
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys", "user");
        claim_proposal(dir.path(), "m", &request);

        let mut other_baseline = manifest();
        other_baseline.baseline_digest = "f".repeat(40);
        let mut other_task = manifest();
        other_task.task_digest = None;
        let mut other_revision = manifest();
        other_revision.osp_revision = "different-rev".into();

        for (label, m) in [
            ("baseline", other_baseline),
            ("task_digest", other_task),
            ("osp_revision", other_revision),
        ] {
            let err =
                freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &request, &m).unwrap_err();
            let msg = format!("{err:#}");
            assert!(
                msg.contains("INV-E1 provenance"),
                "{label} uyuşmazlığı RED: {msg}"
            );
        }
    }

    #[test]
    fn claim_surfaces_corrupt_frozen_anchor() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path().join("llm-prompt.json"), "not json").unwrap();
        let err =
            freeze_and_claim_proposal_line(dir.path(), &cfg("m"), &req("sys", "user"), &manifest())
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
        claim_bar(dir.path(), "m", &request);

        // R3 P0 bar pin: anchor claim ile ÇAĞRIDAN ÖNCE donar (outcome henüz yok).
        let prompt = read_json(dir.path(), "llm-bar-prompt.json");
        assert_eq!(prompt["kind"], "llm-bar-elicitation-prompt");
        assert!(dir.path().join("llm-bar-line-claim.json").exists());

        record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
            0,
            &manifest(),
        )
        .unwrap();
        let bar = read_json(dir.path(), "llm-bar-response.json");
        assert_eq!(bar["kind"], "llm-bar-elicitation");
        assert_eq!(bar["elicited"]["bar_value"], 0.6);
        assert_eq!(
            bar["prompt_digest"],
            completion_prompt_digest(&request).as_str()
        );

        // R1 P1 regression: elicited kaydı sonrası yeniden örnekleme RED.
        let err = record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("SEALED") && format!("{err:#}").contains("resampling"),
            "seal message: {err:#}"
        );
    }

    #[test]
    fn bar_transport_exhausted_seals_and_retry_count_persists() {
        // R2: bar tarafı da experiment-line bütçesiyle çalışır — transport
        // tükenmesi MÜHÜRLÜ; retry kaydı (network_retry_count) bar completion'ı
        // için de persists ("retry kayda geçer").
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys-bar", "user-view");
        claim_bar(dir.path(), "m", &request);
        record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            None,
            None,
            None,
            Some("attempt 1 (retryable): reset; attempt 2 (retryable): timeout"),
            1,
            &manifest(),
        )
        .unwrap();
        let bar = read_json(dir.path(), "llm-bar-response.json");
        assert_eq!(
            bar["transport_error"],
            "attempt 1 (retryable): reset; attempt 2 (retryable): timeout"
        );
        assert_eq!(bar["network_retry_count"], 1, "D6: retry kayda geçer");
        assert!(bar["elicited"].is_null());

        // Yeniden bar claim'i RED (sealed outcome) — çağrı öncesi.
        assert!(freeze_and_claim_bar_line(dir.path(), &cfg("m"), &request, &manifest()).is_err());
        // Yeniden elicitation (aynı prompt'la bile) = yeniden örnekleme → RED.
        let response = raw(r#"{"bar_value":0.7,"predicted":[]}"#);
        let elicited = parse_elicited_bar(&response.content).unwrap();
        let err = record_bar_elicitation(
            dir.path(),
            &cfg("m"),
            &request,
            Some(&response),
            Some(&elicited),
            None,
            None,
            0,
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("SEALED"),
            "bar transport mührü: {err:#}"
        );
    }

    #[test]
    fn bar_different_prompt_refused_by_freeze_at_claim() {
        // Farklı bar prompt'u: claim aşamasında (outcome yokken) FROZEN RED.
        let dir = tempfile::tempdir().unwrap();
        let request = req("sys-bar", "user-view");
        claim_bar(dir.path(), "m", &request);
        let err = freeze_and_claim_bar_line(
            dir.path(),
            &cfg("m"),
            &req("sys-bar-DIFFERENT", "user-view"),
            &manifest(),
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("already FROZEN"),
            "farklı bar prompt'u red: {err:#}"
        );
    }
}
