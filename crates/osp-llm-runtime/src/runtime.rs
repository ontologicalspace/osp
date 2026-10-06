//! Runtime: stateless HTTP client wrapping an OpenAI-compatible endpoint.

use std::time::Duration;

use osp_core::agent::{DeltaProposal, OspPrompt};
use serde::Serialize;

use crate::error::LlmError;
use crate::prompt::{osp_system_prompt, osp_user_prompt};
use crate::response::{parse_raw, RawCompletion, TokenUsage};

/// Endpoint + model + auth configuration. Cloneable for shared use.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// OpenAI-compatible chat-completion URL.
    pub endpoint: String,
    /// Model id, e.g. `"gpt-4o-mini"`.
    pub model: String,
    /// Bearer token. Required (non-empty) before any call.
    pub api_key: String,
    /// Per-request timeout (HTTP connect + read).
    pub timeout: Duration,
    /// Sampling temperature.
    pub temperature: f32,
    /// Max completion tokens.
    pub max_tokens: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: String::new(),
            timeout: Duration::from_secs(60),
            temperature: 0.3,
            max_tokens: 500,
        }
    }
}

impl RuntimeConfig {
    /// Read the API key from the `OPENAI_API_KEY` env var.
    pub fn with_env_api_key(mut self) -> Result<Self, LlmError> {
        let key = std::env::var("OPENAI_API_KEY").map_err(|_| LlmError::MissingApiKey)?;
        if key.trim().is_empty() {
            return Err(LlmError::MissingApiKey);
        }
        self.api_key = key;
        Ok(self)
    }

    /// #171 D3: key + model/endpoint env overrides in one pass.
    ///
    /// - `OPENAI_API_KEY` — required (empty ⇒ [`LlmError::MissingApiKey`]).
    /// - `OSP_LLM_MODEL` — optional; empty/whitespace ⇒ default (`gpt-4o-mini`).
    /// - `OSP_LLM_ENDPOINT` — optional; empty/whitespace ⇒ default (OpenAI).
    ///
    /// Gerekçe (tasarım notu D3): deney satırlarında `model` alanı dolu olmalı ve
    /// Faz-2 model-karşılaştırması aynı protokolle veri biriktirir. v1 deneyi tek
    /// modelle koşar; override KOŞULMAZ, yalnız yapılandırılabilir.
    pub fn from_env() -> Result<Self, LlmError> {
        Self::from_env_lookup(|name| std::env::var(name).ok())
    }

    /// Testable core — CI offline: env lookups injected, `std::env` never touched.
    fn from_env_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, LlmError> {
        let mut config = Self::default().with_env_api_key_from(|| lookup("OPENAI_API_KEY"))?;
        if let Some(model) = lookup("OSP_LLM_MODEL") {
            let model = model.trim();
            if !model.is_empty() {
                config.model = model.to_string();
            }
        }
        if let Some(endpoint) = lookup("OSP_LLM_ENDPOINT") {
            let endpoint = endpoint.trim();
            if !endpoint.is_empty() {
                config.endpoint = endpoint.to_string();
            }
        }
        Ok(config)
    }

    /// `with_env_api_key`'in testable çekirdeği (lookup enjekte edilir).
    fn with_env_api_key_from(
        mut self,
        lookup: impl FnOnce() -> Option<String>,
    ) -> Result<Self, LlmError> {
        let key = lookup().ok_or(LlmError::MissingApiKey)?;
        if key.trim().is_empty() {
            return Err(LlmError::MissingApiKey);
        }
        self.api_key = key;
        Ok(self)
    }
}

/// One chat message in the request payload.
#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

/// The subset of the chat-completion request body we serialize.
#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

/// A pre-built pair of (system, user) messages — used so callers can measure
/// input size before/after the call without re-serializing.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub system: String,
    pub user: String,
}

impl CompletionRequest {
    /// Build the standard OSP request: system prompt + serialized `OspPrompt`.
    pub fn osp(prompt: &OspPrompt) -> Self {
        Self {
            system: osp_system_prompt().to_string(),
            user: osp_user_prompt(prompt),
        }
    }

    /// Total input size in characters (for RQ5 prompt-size reporting).
    pub fn input_chars(&self) -> usize {
        self.system.len() + self.user.len()
    }
}

/// Stateless OpenAI-compatible runtime (inv #11 — no agent state held).
#[derive(Debug, Clone)]
pub struct Runtime {
    config: RuntimeConfig,
    client: reqwest::blocking::Client,
}

impl Runtime {
    /// Construct from config. Errors only if the HTTP client cannot be built.
    pub fn new(config: RuntimeConfig) -> Result<Self, LlmError> {
        if config.api_key.trim().is_empty() {
            return Err(LlmError::MissingApiKey);
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .build()?;
        Ok(Self { config, client })
    }

    /// Convenience: env-derived config (`OPENAI_API_KEY` + #171 D3
    /// `OSP_LLM_MODEL`/`OSP_LLM_ENDPOINT` overrides).
    pub fn from_env() -> Result<Self, LlmError> {
        Self::new(RuntimeConfig::from_env()?)
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Run one OSP-prompt completion and parse the result into a `DeltaProposal`.
    ///
    /// Use [`complete_raw`] when you only need token counts or want to inspect
    /// the assistant text before parsing — this method fails if the model
    /// returns a schema-violating response.
    pub fn complete(&self, prompt: &OspPrompt) -> Result<(DeltaProposal, TokenUsage), LlmError> {
        let req = CompletionRequest::osp(prompt);
        let raw = self.complete_raw(&req)?;
        raw.into_proposal()
            .map_err(|(raw, source)| LlmError::ProposalParse { raw, source })
    }

    /// Lower-level entrypoint: run an arbitrary (system, user) pair and return
    /// the raw assistant text + token usage. Never fails on proposal-shape
    /// grounds — used by the benchmark to compare OSP vs raw source-dump
    /// prompts with identical HTTP plumbing.
    pub fn complete_raw(&self, req: &CompletionRequest) -> Result<RawCompletion, LlmError> {
        let body = ChatRequest {
            model: &self.config.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: &req.system,
                },
                ChatMessage {
                    role: "user",
                    content: &req.user,
                },
            ],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };
        let serialized = serde_json::to_vec(&body)
            .map_err(|e| LlmError::BadResponse(format!("request serialize: {e}")))?;

        tracing::debug!(
            model = %self.config.model,
            input_chars = req.input_chars(),
            "sending completion request"
        );

        let resp = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(&self.config.api_key)
            .header("Content-Type", "application/json")
            .body(serialized)
            .send()?;

        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(LlmError::Status {
                code: status.as_u16(),
                body: text,
            });
        }
        parse_raw(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::agent::{OspPrompt, OutputContract};
    use osp_core::space::TimeLayer;
    use osp_core::vision::VisionVector;

    #[test]
    fn completion_request_osp_serializes_prompt() {
        let prompt = OspPrompt {
            vision: VisionVector::default(),
            time_ref: TimeLayer::default(),
            permissions: Default::default(),
            output_contract: OutputContract::default(),
        };
        let req = CompletionRequest::osp(&prompt);
        assert!(req.user.contains("OspPrompt:"));
        assert!(req.user.contains("\"vision\""));
        assert!(req.input_chars() > 0);
    }

    #[test]
    fn runtime_new_rejects_empty_api_key() {
        let cfg = RuntimeConfig::default(); // empty key
        assert!(matches!(Runtime::new(cfg), Err(LlmError::MissingApiKey)));
    }

    #[test]
    fn config_with_env_api_key_missing_errors() {
        // Ensure the var is absent for this test.
        std::env::remove_var("OPENAI_API_KEY");
        assert!(matches!(
            RuntimeConfig::default().with_env_api_key(),
            Err(LlmError::MissingApiKey)
        ));
    }

    // ── #171 D3: OSP_LLM_MODEL / OSP_LLM_ENDPOINT override (CI offline — lookup inject) ──

    fn envmap<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn config_from_env_applies_model_and_endpoint_overrides() {
        let cfg = RuntimeConfig::from_env_lookup(envmap(&[
            ("OPENAI_API_KEY", "sk-test"),
            ("OSP_LLM_MODEL", "gpt-4.1-mini"),
            (
                "OSP_LLM_ENDPOINT",
                "http://localhost:9999/v1/chat/completions",
            ),
        ]))
        .unwrap();
        assert_eq!(cfg.model, "gpt-4.1-mini");
        assert_eq!(cfg.endpoint, "http://localhost:9999/v1/chat/completions");
        assert_eq!(cfg.api_key, "sk-test");
    }

    #[test]
    fn config_from_env_defaults_when_overrides_absent() {
        let cfg = RuntimeConfig::from_env_lookup(envmap(&[("OPENAI_API_KEY", "sk-test")])).unwrap();
        assert_eq!(cfg.model, "gpt-4o-mini");
        assert_eq!(cfg.endpoint, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn config_from_env_empty_override_falls_back_to_default() {
        // Boş/whitespace override = unset (sessiz boş model kimliği tuzağı).
        let cfg = RuntimeConfig::from_env_lookup(envmap(&[
            ("OPENAI_API_KEY", "sk-test"),
            ("OSP_LLM_MODEL", "   "),
            ("OSP_LLM_ENDPOINT", ""),
        ]))
        .unwrap();
        assert_eq!(cfg.model, "gpt-4o-mini");
        assert_eq!(cfg.endpoint, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn config_from_env_missing_key_errors() {
        assert!(matches!(
            RuntimeConfig::from_env_lookup(envmap(&[("OSP_LLM_MODEL", "x")])),
            Err(LlmError::MissingApiKey)
        ));
        assert!(matches!(
            RuntimeConfig::from_env_lookup(envmap(&[("OPENAI_API_KEY", "  ")])),
            Err(LlmError::MissingApiKey)
        ));
    }
}
