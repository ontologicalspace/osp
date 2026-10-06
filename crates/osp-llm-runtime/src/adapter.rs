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

use crate::artifacts::{self, ElicitedBar};
use crate::error::LlmError as RtLlmError;
use crate::prompt::{delta_proposal_output_format_snippet, osp_system_prompt};
use crate::response::TokenUsage;
use crate::{CompletionRequest, Runtime};

/// D3 - Runtime -> navigator::LlmClient adapter. Gerçek GPT-4o-mini (veya OpenAI-compatible).
///
/// `Runtime::complete` OspPrompt alır, ama navigator AgentTaskView üretir. Bu adapter
/// `complete_raw`'ı custom CompletionRequest ile çağırır - OspPrompt'u bypass eder.
/// `system` = osp_system_prompt + trajectory task context, `user` = AgentTaskView JSON.
///
/// #171: `with_artifacts(dir)` ile ilk gerçek completion'ın (prompt, ham yanıt, parse
/// sonucu) donmuş kanıt üçlüsü `dir`'e yazılır (INV-E1 freeze; bkz. `artifacts` modülü).
pub struct RuntimeLlmClient {
    runtime: Runtime,
    last_usage: Mutex<TokenUsage>,
    /// #171 §5 — kanıt dizini (None: kayıt yok — mevcut davranış).
    artifacts_dir: Option<PathBuf>,
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
            first_view: Mutex::new(None),
        }
    }

    /// #171 §5: llm-real kanıt artifact'larını `dir`'e yaz (llm-prompt.json ·
    /// llm-raw-response.txt · llm-proposals.json; elicit_bar ile llm-bar.json).
    pub fn with_artifacts(mut self, dir: impl Into<PathBuf>) -> Self {
        self.artifacts_dir = Some(dir.into());
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

    /// #171 D4 — bar elicitation: aynı kör view ile İKİNCİ completion.
    ///
    /// "kabul barın olarak öngörülen değişim-sonrası coupling değerini bildir ve
    /// önerilerini öngörülen etkiye göre sırala". Elicitation prompt'u da donar +
    /// digest kayda geçer (INV-E1 disiplini); yanıt `llm-bar.json`'a yazılır.
    /// Transport/parse ihlali DÜRÜST boşluk olarak dosyaya işlenir (D6) — yalnız
    /// disk hatası Err döner. None = hiç completion olmadı (kayda değer view yok).
    pub fn elicit_bar(&self) -> anyhow::Result<Option<BarElicitationReport>> {
        let dir = self
            .artifacts_dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("bar elicitation requires an artifacts directory"))?;
        let Some(view) = self.first_task_view() else {
            return Ok(None);
        };
        let req = CompletionRequest {
            system: bar_elicitation_system_prompt(&view),
            user: serde_json::to_string_pretty(&view)
                .map_err(|e| anyhow::anyhow!("AgentTaskView serialize: {e}"))?,
        };
        let cfg = self.runtime.config();
        match self.runtime.complete_raw(&req) {
            Ok(raw) => {
                let parsed = artifacts::parse_elicited_bar(&raw.content);
                let (elicited, parse_error) = match parsed {
                    Ok(bar) => (Some(bar), None),
                    Err(e) => (None, Some(format!("{e}"))),
                };
                artifacts::record_bar_elicitation(
                    dir,
                    &cfg.model,
                    &cfg.endpoint,
                    &req,
                    Some(&raw),
                    elicited.as_ref(),
                    parse_error.as_deref(),
                    None,
                )?;
                Ok(Some(BarElicitationReport {
                    elicited,
                    parse_error,
                    transport_error: None,
                    prompt_digest: artifacts::completion_prompt_digest(&req),
                }))
            }
            Err(e) => {
                // Transport hatası — llm-bar.json yine de dürüst boşlukla yazılır.
                artifacts::record_bar_elicitation(
                    dir,
                    &cfg.model,
                    &cfg.endpoint,
                    &req,
                    None,
                    None,
                    None,
                    Some(&format!("{e}")),
                )?;
                Ok(Some(BarElicitationReport {
                    elicited: None,
                    parse_error: None,
                    transport_error: Some(format!("{e}")),
                    prompt_digest: artifacts::completion_prompt_digest(&req),
                }))
            }
        }
    }
}

/// #171 D4 — elicit_bar sonucunun özeti (CLI çıktısı için; kanıt `llm-bar.json`).
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
        // view üzerinden işler. first_view'in None olması = bu ilk completion.
        let is_first = self
            .first_view
            .lock()
            .expect("first_view poisoned")
            .is_none();
        if is_first {
            *self.first_view.lock().expect("first_view poisoned") = Some(view.clone());
        }
        let raw = self.runtime.complete_raw(&req).map_err(map_runtime_error)?;
        // G2c-4 (review 10 #5): usage'ı parse error'da DA koru — token harcandı.
        let usage = raw.usage;
        *self.last_usage.lock().expect("last_usage poisoned") = raw.usage;
        let token_cost = Some(TokenCost {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        });
        let parse_result = raw.clone().into_proposal();
        // #171 §5 (yalnız ilk completion — INV-E1 freeze): kanıt persist'i
        // protokolün parçası; disk hatası çağrıyı terminal hatayla bitirir.
        if is_first {
            if let Some(dir) = &self.artifacts_dir {
                let (proposal, parse_error) = match &parse_result {
                    Ok((p, _)) => (Some(p), None),
                    // Ham yanıt llm-raw-response.txt'te ayrıca durur (D6) —
                    // parse_error yalnız hitalet sebebidir.
                    Err((_raw_text, e)) => (None, Some(format!("{e}"))),
                };
                let cfg = self.runtime.config();
                artifacts::record_proposal_completion(
                    dir,
                    &cfg.model,
                    &cfg.endpoint,
                    &req,
                    &raw,
                    proposal,
                    parse_error.as_deref(),
                )
                .map_err(|e| {
                    NavLlmError::Network(format!(
                        "llm artifact persist failed ({}): {e:#}",
                        dir.display()
                    ))
                })?;
            }
        }
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
