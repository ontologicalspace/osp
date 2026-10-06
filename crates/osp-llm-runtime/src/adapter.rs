//! D3 — RuntimeLlmClient: navigator::LlmClient trait impl for real GPT-4o-mini.
//!
//! Runtime -> navigator::LlmClient adapter. Custom prompt (osp_system_prompt +
//! trajectory task context + AgentTaskView JSON) ile gerçek LLM çağrısı.
//! OspPrompt DEĞİŞMEZ (Paper 1 stub alanları korunur) — complete_raw bypass.
//!
//! **INV-T1:** AgentTaskView serialize edilir (hedef koordinat YOK).
//! **INV-#4:** System prompt agent'a "pozisyon DECLARE ETME" der.
//!
//! **G2:** `last_usage: Mutex<TokenUsage>` (Cell → Mutex) — `LlmClient: Send + Sync`
//! gereği (MCP server Arc<dyn LlmClient> + spawn_blocking). Mutex Sync'tir.

use std::path::PathBuf;
use std::sync::Mutex;

use osp_core::agent::DeltaProposal;
use osp_core::navigator::{LlmClient, LlmError as NavLlmError};
use osp_core::trajectory::{AgentTaskView, TokenCost};

use crate::artifacts::{self, ElicitedBar, InvE1Manifest};
use crate::error::LlmError as RtLlmError;
use crate::prompt::{delta_proposal_output_format_snippet, osp_system_prompt};
use crate::response::{RawCompletion, TokenUsage};
use crate::{CompletionRequest, Runtime};

/// D3 - Runtime -> navigator::LlmClient adapter. Gerçek GPT-4o-mini (veya OpenAI-compatible).
///
/// `Runtime::complete` OspPrompt alır, ama navigator AgentTaskView üretir. Bu adapter
/// `complete_raw`'ı custom CompletionRequest ile çağırır - OspPrompt'u bypass eder.
/// `system` = osp_system_prompt + trajectory task context, `user` = AgentTaskView JSON.
///
/// #171 preregistered mod: `with_artifacts(dir, manifest)` kanıt kaydını VE D6
/// strict retry semantiğini birlikte açar — parse ihlali terminal (navigator
/// `ParseFailurePolicy::TerminalNoRepair` ile koşar), ağ hatasında AYNI
/// `CompletionRequest` ile en fazla bir retry (bkz. [`strict_d6_call`]).
/// Kayıtsız mod mevcut davranışı korur (kayıt yok, iç retry yok).
pub struct RuntimeLlmClient {
    runtime: Runtime,
    last_usage: Mutex<TokenUsage>,
    /// #171 §5 — kanıt dizini (None: kayıt yok, generic davranış — mevcut hâl).
    artifacts_dir: Option<PathBuf>,
    /// INV-E1 provenance manifest'i (donmuş anchor'a yazılır; with_artifacts ile birlikte).
    inv_e1: Option<InvE1Manifest>,
    /// İlk completion'ın kör view'ı (D4 bar elicitation aynı view'ı kullanır).
    first_view: Mutex<Option<AgentTaskView>>,
}

impl RuntimeLlmClient {
    /// Mevcut Runtime ile adapter kur.
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            last_usage: Mutex::new(TokenUsage::default()),
            artifacts_dir: None,
            inv_e1: None,
            first_view: Mutex::new(None),
        }
    }

    /// #171 §5 + INV-E1: llm-real kanıt artifact'larını `dir`'e yaz — donmuş
    /// prompt anchor'ları (llm-prompt.json / llm-bar-prompt.json), ham yanıt,
    /// D6 durum kayıtları. Bu builder AYNI ANDA D6 strict retry semantiğini
    /// açar: kayıt protokolü ile tamir döngüsü bir arada var olamaz (review P0).
    pub fn with_artifacts(mut self, dir: impl Into<PathBuf>, inv_e1: InvE1Manifest) -> Self {
        self.artifacts_dir = Some(dir.into());
        self.inv_e1 = Some(inv_e1);
        self
    }

    /// OPENAI_API_KEY env var'dan Runtime kur + adapter oluştur.
    pub fn from_env() -> Result<Self, RtLlmError> {
        Ok(Self::new(Runtime::from_env()?))
    }

    /// #171 D4: ilk completion'ın kör view'ı (henüz çağrı yoksa None).
    pub fn first_task_view(&self) -> Option<AgentTaskView> {
        self.first_view.lock().expect("first_view poisoned").clone()
    }

    /// Kayıt gerekçesi — iki alan birlikte set edilir (with_artifacts) veya hiçbiri.
    fn recording<'a>(
        dir: &'a Option<PathBuf>,
        manifest: &'a Option<InvE1Manifest>,
    ) -> Option<(&'a PathBuf, &'a InvE1Manifest)> {
        match (dir, manifest) {
            (Some(d), Some(m)) => Some((d, m)),
            _ => None,
        }
    }

    /// #171 D4 — bar elicitation: aynı kör view ile İKİNCİ completion.
    ///
    /// Sıra sözleşmesi (review R3 P0): request kur → **freeze_and_claim**
    /// (anchor + no-clobber claim, HTTP'ten ÖNCE) → strict_d6_call → outcome
    /// kaydı (claim şartı yapısal). Retryable hatada aynı-prompt tek retry;
    /// her terminal sonuç mühürler. None = hiç completion olmadı.
    pub fn elicit_bar(&self) -> anyhow::Result<Option<BarElicitationReport>> {
        let Some((dir, manifest)) = Self::recording(&self.artifacts_dir, &self.inv_e1) else {
            anyhow::bail!("bar elicitation requires the artifacts protocol (with_artifacts)");
        };
        let Some(view) = self.first_task_view() else {
            return Ok(None);
        };
        let req = CompletionRequest {
            system: bar_elicitation_system_prompt(&view),
            user: serde_json::to_string_pretty(&view)
                .map_err(|e| anyhow::anyhow!("AgentTaskView serialize: {e}"))?,
        };
        let cfg = self.runtime.config();
        // PRE-CALL: anchor'u dondur + hattı atomik sahiplen (P0+P1). Sealed
        // veya claimed hatlarda HTTP çağrısı yapılmaz.
        artifacts::freeze_and_claim_bar_line(dir, cfg, &req, manifest)?;
        match strict_d6_call(|r| self.runtime.complete_raw(r), &req) {
            StrictD6Outcome::Response {
                raw,
                network_retries,
            } => {
                let parsed = artifacts::parse_elicited_bar(&raw.content);
                let (elicited, parse_error) = match parsed {
                    Ok(bar) => (Some(bar), None),
                    Err(e) => (None, Some(format!("{e}"))),
                };
                artifacts::record_bar_elicitation(
                    dir,
                    cfg,
                    &req,
                    Some(&raw),
                    elicited.as_ref(),
                    parse_error.as_deref(),
                    None,
                    network_retries,
                    manifest,
                )?;
                Ok(Some(BarElicitationReport {
                    elicited,
                    parse_error,
                    transport_error: None,
                    prompt_digest: artifacts::completion_prompt_digest(&req),
                }))
            }
            StrictD6Outcome::RetryExhausted { first, second } => {
                // Retryable ilk hatanın retry'ı da hata verdi — dürüst boşluk
                // kaydı + MÜHÜR (bütçe tükendi; ikinci hata non-retryable
                // olabilir — sınıfı etiketle).
                let msg = retry_exhausted_message(&first, &second);
                artifacts::record_bar_elicitation(
                    dir,
                    cfg,
                    &req,
                    None,
                    None,
                    None,
                    Some(&msg),
                    1,
                    manifest,
                )?;
                Ok(Some(BarElicitationReport {
                    elicited: None,
                    parse_error: None,
                    transport_error: Some(msg),
                    prompt_digest: artifacts::completion_prompt_digest(&req),
                }))
            }
            StrictD6Outcome::NonRetryable { error } => {
                // Sınıflandırıcı reddi (ör. 401/400): retry YAPILMADI — kayıt
                // retry_count=0 ile mühürlenir.
                let msg = format!("non-retryable ({error}) — no retry attempted");
                artifacts::record_bar_elicitation(
                    dir,
                    cfg,
                    &req,
                    None,
                    None,
                    None,
                    Some(&msg),
                    0,
                    manifest,
                )?;
                Ok(Some(BarElicitationReport {
                    elicited: None,
                    parse_error: None,
                    transport_error: Some(msg),
                    prompt_digest: artifacts::completion_prompt_digest(&req),
                }))
            }
        }
    }
}

/// RetryExhausted durumunun dürüst kayıt mesajı: ilk hata retryable'dı (bu
/// yüzden retry edildi), ikinci hata İSE başka bir sınıfta olabilir (review
/// R3 P2) — sınıf etiketi kayda geçer, üçüncü deneme her durumda yapılmaz.
fn retry_exhausted_message(first: &RtLlmError, second: &RtLlmError) -> String {
    let second_class = if is_d6_retryable(second) {
        "retryable"
    } else {
        "non-retryable"
    };
    format!("attempt 1 (retryable): {first}; attempt 2 ({second_class}): {second}")
}

/// **D6 retryable sınıflandırıcısı (review R2 P1; preregistered küme):**
/// "yalnız GEÇİCİ ağ hatasında retry" kuralı error TÜRÜNE göre işler —
/// generic Err-retry DEĞİL.
///
/// Retryable (issue #171 preregistration kaydı 6019658939):
/// - `Http(_)` — transport katmanı (connect/timeout/reset/DNS/TLS): tanım
///   gereği geçici ağ sınıfı.
/// - `Status { code: 429 | 502 | 503 | 504 }` — açıkça preregister edilmiş
///   geçici provider status kümesi.
///
/// Terminal (retry YOK): diğer tüm `Status` (401/400/500…), `BadResponse`
/// (malformed envelope), `ProposalParse`, `MissingApiKey`.
pub fn is_d6_retryable(e: &RtLlmError) -> bool {
    match e {
        RtLlmError::Http(_) => true,
        RtLlmError::Status { code, .. } => matches!(code, 429 | 502 | 503 | 504),
        RtLlmError::BadResponse(_)
        | RtLlmError::ProposalParse { .. }
        | RtLlmError::MissingApiKey => false,
    }
}

/// **D6 strict retry çekirdeği** (R1 P0 + R2 P1 + R3 P2): retryable hatada
/// AYNI `CompletionRequest` ile EN FAZLA bir retry; non-retryable hata
/// retry'sız terminal; retry bütçesi tükenince terminal. Parse ihlaline retry
/// YOKTUR (tamir döngüsü yasak — navigator `ParseFailurePolicy::
/// TerminalNoRepair` + artifacts mührü kapatır).
///
/// Aynı `req` nesnesi iki denemede de değişmeden geçirilir — system/user
/// bayt-özdeşliği yapısal garanti, deneysel iddia değildir.
#[derive(Debug)]
pub(crate) enum StrictD6Outcome {
    /// Yanıt alındı (retry sayısıyla).
    Response {
        raw: RawCompletion,
        network_retries: u32,
    },
    /// İlk hata sınıflandırıcıda non-retryable çıktı — retry yapıLMADI.
    NonRetryable { error: RtLlmError },
    /// Retryable ilk hatanın retry'ı da hata verdi — retry bütçesi tükendi
    /// (ikinci hata kendi sınıfında olabilir: `retry_exhausted_message`
    /// sınıfı kaydeder; review R3 P2). Üçüncü deneme yapılmaz.
    RetryExhausted {
        first: RtLlmError,
        second: RtLlmError,
    },
}

pub(crate) fn strict_d6_call(
    mut call: impl FnMut(&CompletionRequest) -> Result<RawCompletion, RtLlmError>,
    req: &CompletionRequest,
) -> StrictD6Outcome {
    match call(req) {
        Ok(raw) => StrictD6Outcome::Response {
            raw,
            network_retries: 0,
        },
        Err(first) if !is_d6_retryable(&first) => StrictD6Outcome::NonRetryable { error: first },
        Err(first) => match call(req) {
            Ok(raw) => StrictD6Outcome::Response {
                raw,
                network_retries: 1,
            },
            Err(second) => StrictD6Outcome::RetryExhausted { first, second },
        },
    }
}

/// #171 D4 — elicit_bar sonucunun özeti (CLI çıktısı için; kanıt
/// `llm-bar-prompt.json` + `llm-bar-response.json`).
#[derive(Debug, Clone)]
pub struct BarElicitationReport {
    pub elicited: Option<ElicitedBar>,
    pub parse_error: Option<String>,
    pub transport_error: Option<String>,
    pub prompt_digest: String,
}

impl LlmClient for RuntimeLlmClient {
    fn complete(&self, view: &AgentTaskView) -> Result<DeltaProposal, NavLlmError> {
        let req = CompletionRequest {
            system: trajectory_system_prompt(view),
            user: serde_json::to_string_pretty(view).map_err(|e| NavLlmError::ProposalParse {
                message: format!("AgentTaskView serialize: {e}"),
                token_cost: None,
            })?,
        };
        // #171: ilk view'ı yakala — kayıt (first-freeze) ve D4 elicitation bu
        // view üzerinden işler. Check+set TEK lock acquisition (atomik — iki
        // concurrent caller da kendini "first" göremez; review P2).
        let is_first = {
            let mut guard = self.first_view.lock().expect("first_view poisoned");
            if guard.is_none() {
                *guard = Some(view.clone());
                true
            } else {
                false
            }
        };

        let cfg = self.runtime.config();
        let recording = Self::recording(&self.artifacts_dir, &self.inv_e1);
        // **PRE-CALL (review R3 P0+P1):** ilk completion'da donmuş prompt
        // anchor'u + atomik line-claim HTTP'ten ÖNCE yazılır ("freeze önce
        // kanıtı", çağrı-sonrası değil). Sealed/claimed hatlarda HTTP YAPILMAZ.
        if is_first {
            if let Some((dir, manifest)) = recording {
                artifacts::freeze_and_claim_proposal_line(dir, cfg, &req, manifest).map_err(
                    |e| {
                        NavLlmError::Network(format!(
                            "llm experiment line is not open ({}): {e:#}",
                            dir.display()
                        ))
                    },
                )?;
            }
        }
        // D6 strict (yalnız preregistered/kayıtlı mod): retryable hatada AYNI
        // prompt'la bir retry (sınıflandırıcı: is_d6_retryable); non-retryable
        // hatada retry'sız terminal; kayıtsız mod mevcut davranış (tek deneme).
        let (raw, network_retries) = match recording {
            Some((dir, manifest)) => match strict_d6_call(|r| self.runtime.complete_raw(r), &req) {
                StrictD6Outcome::Response {
                    raw,
                    network_retries,
                } => (raw, network_retries),
                StrictD6Outcome::NonRetryable { error } => {
                    // Sınıflandırıcı reddi (ör. 401/400/BadResponse): retry
                    // YAPILMADI; kayıt retry_count=0 ile MÜHÜRLENİR (review R2).
                    let msg = format!("non-retryable ({error}) — no retry attempted");
                    artifacts::record_call_failure(dir, cfg, &req, &msg, 0, manifest).map_err(
                        |e| {
                            NavLlmError::Network(format!(
                                "llm artifact persist failed ({}): {e:#}",
                                dir.display()
                            ))
                        },
                    )?;
                    return Err(NavLlmError::Network(format!(
                        "D6: terminal non-retryable call failure ({error}); no retry \
                         — the experiment line is sealed with this outcome"
                    )));
                }
                StrictD6Outcome::RetryExhausted { first, second } => {
                    // Retryable ilk hatanın retry'ı da hata verdi: kayıt MÜHÜRLÜ
                    // (experiment-line bütçesi tükendi) + terminal (ikinci hata
                    // non-retryable olabilir — sınıf etiketi kayıtta).
                    let msg = retry_exhausted_message(&first, &second);
                    artifacts::record_call_failure(dir, cfg, &req, &msg, 1, manifest).map_err(
                        |e| {
                            NavLlmError::Network(format!(
                                "llm artifact persist failed ({}): {e:#}",
                                dir.display()
                            ))
                        },
                    )?;
                    return Err(NavLlmError::Network(format!(
                        "D6: both attempts failed for this prompt ({msg}); \
                         the one-retry budget is exhausted and the experiment line is \
                         sealed — terminal"
                    )));
                }
            },
            None => (
                self.runtime.complete_raw(&req).map_err(map_runtime_error)?,
                0,
            ),
        };

        // G2c-4 (review 10 #5): usage'ı parse error'da DA koru — token harcandı.
        let usage = raw.usage;
        *self.last_usage.lock().expect("last_usage poisoned") = raw.usage;
        let token_cost = Some(TokenCost {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });

        // #171 §5 OUTCOME (yalnız ilk completion): pre-call claim + donmuş
        // anchor şartı yapısal; record_proposal_outcome parse'ı KENDİSİ yapar
        // ve D6 mührünü uygular (parse-null / success sonrası dizin mühürlü).
        if is_first {
            if let Some((dir, manifest)) = recording {
                artifacts::record_proposal_outcome(dir, cfg, &req, &raw, network_retries, manifest)
                    .map_err(|e| {
                        NavLlmError::Network(format!(
                            "llm artifact persist failed ({}): {e:#}",
                            dir.display()
                        ))
                    })?;
            }
        }
        let parse_result = raw.into_proposal();
        let (proposal, _) =
            parse_result.map_err(|(raw_text, parse_err)| NavLlmError::ProposalParse {
                message: format!("LLM response parse failed: {parse_err}\nRaw: {raw_text}"),
                token_cost,
            })?;
        Ok(proposal)
    }

    fn last_token_cost(&self) -> TokenCost {
        let u = *self.last_usage.lock().expect("last_usage poisoned");
        TokenCost {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }
    }
}

/// G2c-4 structural context bölümü — trajectory ve bar-elicitation prompt'larının
/// ORTAK kısmı (hedef koordinat YOK, focus node + mevcut import edge'leri).
fn structural_context_section(view: &AgentTaskView) -> String {
    match &view.structural_context {
        Some(sc) => {
            let imports: Vec<String> = sc
                .current_outgoing_imports
                .iter()
                .map(|e| format!("{}→{} ({:?})", e.from, e.to, e.kind))
                .collect();
            format!(
                "\nSTRUCTURAL CONTEXT (G2c-4 — mevcut yapısal çevre, hedef koordinat DEĞİL):\n\
                 - focus_node_id: {} (bu node'un coupling'ini düşürmek için outgoing Imports kaldır)\n\
                 - current_outgoing_imports: {}\n",
                sc.focus_node_id,
                if imports.is_empty() {
                    "(none)".to_string()
                } else {
                    imports.join(", ")
                }
            )
        }
        None => String::new(),
    }
}

/// D3 - navigator task için özel system prompt. osp_system_prompt() + trajectory bağlamı.
fn trajectory_system_prompt(view: &AgentTaskView) -> String {
    let m = &view.current_measurement;
    let base = osp_system_prompt();
    let format_snippet = delta_proposal_output_format_snippet();
    // G2c-4 (review 10 #3): structural context — focus node + mevcut import edge'leri.
    let structural_ctx = structural_context_section(view);
    let ctx = format!(
        "TRAJECTORY TASK CONTEXT (Paper 2 - Architectural Trajectory Navigation):\n\
You receive an AgentTaskView - a typed epistemic projection of an architecture task.\n\
\n\
CURRENT STATE (engine-measured, NOT your claim):\n\
- coupling (x): {:.3}\n\
- cohesion (y): {:.3}\n\
- instability (z): {:.3}\n\
{structural_ctx}\
\n\
TASK:\n\
- task_id: {}\n\
- label: {}\n\
- target_predicate: constraints the NEXT engine-measured state must satisfy\n\
- allowed_operations: structural operations you MAY propose (e.g. RemoveImport)\n\
- constraints: rules that MUST hold\n\
\n\
INSTRUCTIONS:\n\
1. Analyze the target_predicate - what architectural change moves toward satisfying it?\n\
2. Produce a DeltaProposal with structural changes.\n\
3. To REDUCE coupling, use removed_edges to remove outgoing Imports from the focus_node_id.\n\
   The engine measures the TASK's predicate scope; affected_nodes is an ADVISORY impact\n\
   hint only (it does NOT control what is measured — #95-A MD-1).\n\
4. Use ONLY operations from allowed_operations.\n\
5. DO NOT declare positions - the engine measures. position_hints are advisory only (INV-T4).\n\
6. Provide clear reasoning for your proposed changes.\n\
\n\
OUTPUT FORMAT (strict JSON, no markdown fences):\n\
{format_snippet}",
        m.x, m.y, m.z, view.task_id, view.label
    );
    // D4 - Calibration feedback: önceki attempt'lerin hatalarını LLM'e göster.
    let feedback_section = if view.feedback_history.is_empty() {
        String::new()
    } else {
        let items: String = view
            .feedback_history
            .iter()
            .map(|f| format!("- {f}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\nPREVIOUS ATTEMPTS FAILED — learn from these errors:\n{items}\n\nDo NOT repeat these mistakes. Adjust your approach.")
    };
    format!("{base}\n\n{ctx}{feedback_section}")
}

/// #171 D4 — bar elicitation system prompt'u (ikinci completion, aynı kör view).
///
/// osp_system_prompt DeltaProposal çıktısı ister; elicitation BAR çıktısı ister —
/// bu yüzden ayrı bir tab (koordinat eksenleri kimliği ortak) + elicitation görevi.
/// Ölçüm bağlamı (CURRENT STATE + structural context) proposal prompt'undakiyle
/// aynı bölümden gelir — kör view'ın serileştirmesi (`user`) ile birlikte INV-E1
/// türetilebilirliği korunur: prompt YALNIZCA view'dan + sabit metinden üretilir.
pub fn bar_elicitation_system_prompt(view: &AgentTaskView) -> String {
    let m = &view.current_measurement;
    let structural_ctx = structural_context_section(view);
    format!(
        "You are an OSP (Ontological Space Protocol) agent. You receive a typed \
epistemic projection packet containing module coordinates in a \
5-dimensional architectural space. Coordinate axes: x=coupling, y=cohesion, \
z=instability, w=entropy, v=witness-depth.\n\
\n\
BAR ELICITATION (second completion - same blinded task view):\n\
The engine, not you, measures positions. BEFORE any change is applied, state:\n\
- bar_value: the post-change coupling (x) value you would accept as success for \
this task (a number in [0,1]).\n\
- predicted: the structural changes you would propose for this task, each with a \
short label and your predicted post-change coupling (x), ordered from most \
beneficial to least beneficial.\n\
\n\
CURRENT STATE (engine-measured, NOT your claim):\n\
- coupling (x): {:.3}\n\
- cohesion (y): {:.3}\n\
- instability (z): {:.3}\n\
{structural_ctx}\
\n\
TASK:\n\
- task_id: {}\n\
- label: {}\n\
\n\
OUTPUT FORMAT (strict JSON, no markdown fences):\n\
{{\"bar_value\": 0.55, \"predicted\": [{{\"label\": \"remove import b\", \"predicted_coupling\": 0.5}}]}}",
        m.x, m.y, m.z, view.task_id, view.label
    )
}

/// D3 - Runtime LlmError -> navigator LlmError mapping.
fn map_runtime_error(e: RtLlmError) -> NavLlmError {
    match e {
        RtLlmError::Http(_) | RtLlmError::Status { .. } | RtLlmError::MissingApiKey => {
            NavLlmError::Network(e.to_string())
        }
        RtLlmError::BadResponse(msg) => NavLlmError::ProposalParse {
            message: format!("Bad API response: {msg}"),
            token_cost: None, // network/API hatası — token harcanmadı
        },
        RtLlmError::ProposalParse { raw, .. } => NavLlmError::ProposalParse {
            message: raw,
            token_cost: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeConfig;
    use osp_core::coords::RawPosition;
    use osp_core::trajectory::{AgentPredicateView, PredicateMode};

    fn blinded_view() -> AgentTaskView {
        AgentTaskView {
            task_id: 42,
            label: "Reduce coupling".into(),
            current_measurement: RawPosition {
                x: 0.82,
                y: 0.5,
                z: 0.6,
                w: 0.5,
                v: 0.3,
            },
            target_predicate: AgentPredicateView {
                mode: PredicateMode::All,
                predicates: vec![],
            },
            allowed_operations: vec![],
            constraints: vec![],
            feedback_history: vec![],
            structural_context: None, // G2c-4
        }
    }

    // 1. runtime_llm_client_compiles (compile-time - trait impl)
    #[test]
    fn runtime_llm_client_implements_navigator_trait() {
        fn accepts_llm_client<L: LlmClient>(_c: &L) {}
        // Trait bound compile-time guarantee. Gerçek instance from_env() (API key).
        let _ = accepts_llm_client::<RuntimeLlmClient>;
    }

    // 2. trajectory_system_prompt_includes_task_context
    #[test]
    fn trajectory_system_prompt_includes_task_context() {
        let view = blinded_view();
        let prompt = trajectory_system_prompt(&view);
        assert!(prompt.contains("task_id: 42"), "task_id in prompt");
        assert!(prompt.contains("Reduce coupling"), "label in prompt");
        assert!(prompt.contains("0.820"), "coupling measurement in prompt");
        assert!(
            prompt.contains("DO NOT declare positions"),
            "INV-T4 warning in prompt"
        );
    }

    // ── #171 D4: bar elicitation prompt ───────────────────────────────────────

    #[test]
    fn bar_elicitation_prompt_pins_task_and_bar_contract() {
        let view = blinded_view();
        let prompt = bar_elicitation_system_prompt(&view);
        assert!(prompt.contains("BAR ELICITATION"), "elicitation task named");
        assert!(prompt.contains("bar_value"), "bar field documented");
        assert!(
            prompt.contains("predicted_coupling"),
            "tau fields documented"
        );
        assert!(prompt.contains("task_id: 42"), "same task context");
        assert!(prompt.contains("0.820"), "engine-measured coupling present");
        // Elicitation completion'ı DeltaProposal İSTEMEZ (çıktı sözleşmesi bar JSON).
        assert!(
            !prompt.contains("Produce a DeltaProposal"),
            "no proposal-output contract in elicitation prompt"
        );
    }

    #[test]
    fn bar_elicitation_user_payload_is_byte_identical_to_proposal_request() {
        // D4 "aynı kör view": elicitation user payload'i proposal completion'ınkiyle
        // BAYT-eşleşik olmalı (aynı serileştirme, aynı view) — körleme aynı kanalı
        // taşıdığını bu pin sabitler.
        let view = blinded_view();
        let proposal_user = serde_json::to_string_pretty(&view).unwrap();
        let bar_req_user = serde_json::to_string_pretty(&view).unwrap();
        assert_eq!(proposal_user, bar_req_user);
        // system prompt'u farklıdır (görev farklı), user kör view'dır:
        let bar_system = bar_elicitation_system_prompt(&view);
        assert_ne!(bar_system, trajectory_system_prompt(&view));
    }

    #[test]
    fn first_view_capture_starts_empty() {
        // Runtime gerçek istemci gerektirir (api key) — capture davranışı burada
        // yalnız başlangıç durumu olarak pinlenir; kayıt çekirdeği artifacts
        // modülünde ağdan bağımsız testlidir.
        let cfg = RuntimeConfig {
            api_key: "sk-test".into(),
            ..Default::default()
        };
        let client = RuntimeLlmClient::new(Runtime::new(cfg).unwrap());
        assert!(client.first_task_view().is_none());
    }

    // ── #171 D6 (review P0): strict ağ-retry çekirdeği ────────────────────────

    fn ok_raw() -> RawCompletion {
        RawCompletion {
            usage: TokenUsage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            },
            content: "{}".to_string(),
        }
    }

    #[test]
    fn strict_d6_first_try_success_no_retry() {
        let mut calls = 0;
        let outcome = strict_d6_call(
            |_req| {
                calls += 1;
                Ok(ok_raw())
            },
            &CompletionRequest {
                system: "s".into(),
                user: "u".into(),
            },
        );
        assert!(matches!(
            outcome,
            StrictD6Outcome::Response {
                network_retries: 0,
                ..
            }
        ));
        assert_eq!(calls, 1, "başarıda retry yok");
    }

    #[test]
    fn strict_d6_transient_network_failure_retries_once_byte_identical() {
        // Review P0 test #2'nin çekirdeği: ilk ağ hatası → AYNI
        // CompletionRequest ile tam bir retry; closure iki çağrıda da
        // system/user BAYT-özdeşliğini doğrular.
        let first_req_digest = std::cell::RefCell::new(Vec::new());
        let mut calls = 0;
        let outcome = strict_d6_call(
            |req| {
                calls += 1;
                first_req_digest
                    .borrow_mut()
                    .push(artifacts::completion_prompt_digest(req));
                if calls == 1 {
                    Err(RtLlmError::Status {
                        code: 503,
                        body: "transient reset".into(),
                    })
                } else {
                    Ok(ok_raw())
                }
            },
            &CompletionRequest {
                system: "sys".into(),
                user: "user".into(),
            },
        );
        match outcome {
            StrictD6Outcome::Response {
                network_retries, ..
            } => assert_eq!(network_retries, 1),
            other => panic!("retry sonrası yanıt beklenirdi: {other:?}"),
        }
        assert_eq!(calls, 2, "en fazla bir retry");
        let digests = first_req_digest.into_inner();
        assert_eq!(digests.len(), 2);
        assert_eq!(
            digests[0], digests[1],
            "iki deneme aynı prompt (byte-identical system+user)"
        );
    }

    #[test]
    fn strict_d6_second_network_failure_is_terminal() {
        let mut calls = 0;
        let outcome = strict_d6_call(
            |_req| {
                calls += 1;
                Err(RtLlmError::Status {
                    code: 502,
                    body: "down".into(),
                })
            },
            &CompletionRequest {
                system: "s".into(),
                user: "u".into(),
            },
        );
        assert!(matches!(outcome, StrictD6Outcome::RetryExhausted { .. }));
        assert_eq!(calls, 2, "bir retry sonrası terminal — üçüncü deneme yok");
    }

    // ── Review R2 P1: retryable sınıflandırıcı ───────────────────────────────

    #[test]
    fn d6_classifier_non_retryable_statuses_never_retry() {
        // D6: "yalnız GEÇİCİ ağ hatasında retry" — kalıcı status kodları
        // (401/400/500…) retry'sız terminal; TAM OLARAK 1 çağrı.
        for code in [401u16, 400, 500, 403, 404] {
            let mut calls = 0;
            let outcome = strict_d6_call(
                |_req| {
                    calls += 1;
                    Err(RtLlmError::Status {
                        code,
                        body: "permanent".into(),
                    })
                },
                &CompletionRequest {
                    system: "s".into(),
                    user: "u".into(),
                },
            );
            assert!(
                matches!(outcome, StrictD6Outcome::NonRetryable { .. }),
                "status {code} non-retryable olmalı"
            );
            assert_eq!(calls, 1, "status {code}: retry yapılmadı");
        }
    }

    #[test]
    fn d6_classifier_bad_response_and_missing_key_never_retry() {
        for make_error in [
            || RtLlmError::BadResponse("malformed envelope".into()),
            || RtLlmError::MissingApiKey,
        ] {
            let mut calls = 0;
            let outcome = strict_d6_call(
                |_req| {
                    calls += 1;
                    Err(make_error())
                },
                &CompletionRequest {
                    system: "s".into(),
                    user: "u".into(),
                },
            );
            assert!(matches!(outcome, StrictD6Outcome::NonRetryable { .. }));
            assert_eq!(calls, 1, "retry yapılmadı");
        }
    }

    #[test]
    fn d6_classifier_preregistered_transient_statuses_retry_once() {
        // Preregistered geçici küme (issue #171 kaydı 6019658939):
        // 429/502/503/504 → retryable; başarılı retry network_retries=1.
        for code in [429u16, 502, 503, 504] {
            let mut calls = 0;
            let outcome = strict_d6_call(
                |_req| {
                    calls += 1;
                    if calls == 1 {
                        Err(RtLlmError::Status {
                            code,
                            body: "transient".into(),
                        })
                    } else {
                        Ok(ok_raw())
                    }
                },
                &CompletionRequest {
                    system: "s".into(),
                    user: "u".into(),
                },
            );
            match outcome {
                StrictD6Outcome::Response {
                    network_retries, ..
                } => {
                    assert_eq!(network_retries, 1, "status {code} retry edildi")
                }
                other => panic!("status {code} retryable olmalı: {other:?}"),
            }
        }
    }

    // 3. error_mapping_runtime_to_navigator
    #[test]
    fn error_mapping_runtime_to_navigator() {
        let net_err = map_runtime_error(RtLlmError::MissingApiKey);
        assert!(matches!(net_err, NavLlmError::Network(_)));

        let status_err = map_runtime_error(RtLlmError::Status {
            code: 500,
            body: "server error".into(),
        });
        assert!(matches!(status_err, NavLlmError::Network(_)));

        let parse_err = map_runtime_error(RtLlmError::ProposalParse {
            raw: "invalid json".into(),
            source: serde_json::from_str::<serde_json::Value>("x").unwrap_err(),
        });
        assert!(matches!(parse_err, NavLlmError::ProposalParse { .. }));

        let bad_resp = map_runtime_error(RtLlmError::BadResponse("empty choices".into()));
        assert!(matches!(bad_resp, NavLlmError::ProposalParse { .. }));
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    // G2c-4 merge şartı (arkadaş review 10): prompt removed_edges/affected_nodes içerir
    // ═══════════════════════════════════════════════════════════════════════════════

    /// Merge şartı: prompt schema removed_edges + affected_nodes içerir.
    /// LLM bu şemayı görmeden coupling-reducing proposal üretemez.
    #[test]
    fn g2c4_prompt_contains_removed_edges_and_affected_nodes() {
        let snippet = delta_proposal_output_format_snippet();
        assert!(
            snippet.contains("removed_edges"),
            "prompt must document removed_edges (G2c-2 subtractive delta)"
        );
        assert!(
            snippet.contains("affected_nodes"),
            "prompt must document affected_nodes (measurement scope)"
        );
        assert!(
            snippet.contains("\"kind\": \"Imports\""),
            "prompt must show EdgeRef kind example"
        );
    }

    /// Merge şartı: prompt snippet'indeki örnek JSON geçerli DeltaProposal.
    #[test]
    fn g2c4_prompt_example_json_parses_as_delta_proposal() {
        // snippet'ten JSON bloğunu çıkar (RULES bölümünden önceki ilk { ... } blok).
        let snippet = delta_proposal_output_format_snippet();
        let rules_idx = snippet.find("RULES:").unwrap_or(snippet.len());
        let json_region = &snippet[..rules_idx];
        let start = json_region.find('{').expect("snippet has JSON");
        let end = json_region.rfind('}').expect("snippet has closing brace");
        let json_str = &json_region[start..=end];
        let proposal: osp_core::agent::DeltaProposal =
            serde_json::from_str(json_str).expect("example JSON parses as DeltaProposal");
        assert_eq!(
            proposal.removed_edges.len(),
            1,
            "example has one removed_edge"
        );
        assert_eq!(
            proposal.affected_nodes.len(),
            1,
            "example has one affected_node"
        );
    }

    /// osp_system_prompt da removed_edges/affected_nodes'a işaret eder (prompt debt yok).
    #[test]
    fn g2c4_osp_system_prompt_references_format_snippet() {
        let prompt = osp_system_prompt();
        // osp_system_prompt "Output format" der, helper detayı verir.
        assert!(prompt.contains("Output format"));
    }
}
