//! OpenAI chat-completion response parsing.

use serde::{Deserialize, Serialize};

use osp_core::agent::DeltaProposal;

use crate::error::LlmError;

/// Real tokenizer counts reported by the API (RQ5 benchmark data).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Raw assistant reply + token counts. Always available once the HTTP call
/// succeeds — proposal parsing is a separate step.
#[derive(Debug, Clone)]
pub struct RawCompletion {
    /// Token counts from `response.usage`.
    pub usage: TokenUsage,
    /// Raw assistant text (kept for diagnostics / hallucination analysis).
    pub content: String,
}

impl RawCompletion {
    /// Try to parse the assistant text as a `DeltaProposal`. Returns the raw
    /// text alongside the parse error on failure so callers can inspect /
    /// log / feed it back into a retry.
    pub fn into_proposal(self) -> Result<(DeltaProposal, TokenUsage), (String, serde_json::Error)> {
        let json = strip_code_fence(self.content.trim());
        serde_json::from_str::<DeltaProposal>(&json)
            .map(|p| (p, self.usage))
            .map_err(|e| (self.content, e))
    }
}

/// One successful completion: parsed proposal + token usage + raw assistant text.
#[derive(Debug, Clone)]
pub struct Completion {
    /// The assistant's message content parsed as a DeltaProposal.
    pub proposal: DeltaProposal,
    /// Token counts from `response.usage`.
    pub usage: TokenUsage,
    /// Raw assistant text (kept for diagnostics / hallucination analysis).
    pub raw_content: String,
}

// Wire types for the subset of the OpenAI response we read. We deliberately do
// not model the full schema — only the fields consumed by the runtime.

#[derive(Debug, Deserialize)]
pub(super) struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub(super) struct Choice {
    pub message: Message,
}

#[derive(Debug, Deserialize)]
pub(super) struct Message {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Parse the response envelope into a [`RawCompletion`] (usage + content).
/// Never fails on proposal-shape grounds — only on response-envelope shape.
pub(super) fn parse_raw(body: &str) -> Result<RawCompletion, LlmError> {
    let resp: ChatCompletionResponse = serde_json::from_str(body)
        .map_err(|e| LlmError::BadResponse(format!("envelope parse: {e}")))?;
    let choice = resp
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LlmError::BadResponse("no choices in response".into()))?;
    Ok(RawCompletion {
        usage: TokenUsage {
            prompt_tokens: resp.usage.prompt_tokens,
            completion_tokens: resp.usage.completion_tokens,
            total_tokens: resp.usage.total_tokens,
        },
        content: choice.message.content,
    })
}

/// Parse a raw HTTP response body into a [`Completion`] (envelope + proposal).
///
/// Proposal parsing uses [`RawCompletion::into_proposal`]; a schema mismatch
/// returns [`LlmError::ProposalParse`] with the raw assistant text attached.
pub(super) fn parse_completion(body: &str) -> Result<Completion, LlmError> {
    let raw = parse_raw(body)?;
    match raw.into_proposal() {
        Ok((proposal, usage)) => Ok(Completion {
            proposal,
            usage,
            raw_content: String::new(), // already moved into proposal parse
        }),
        Err((content, source)) => Err(LlmError::ProposalParse { raw: content, source }),
    }
}

/// Remove a leading/trailing ``` or ```json fence if present.
fn strip_code_fence(s: &str) -> String {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```") {
        // skip optional language tag on the opening fence line
        let rest = rest.trim_start_matches(['j', 's', 'o', 'n']).trim_start_matches('\n');
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim().to_string();
        }
        return rest.trim().to_string();
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_completion_extracts_proposal_and_usage() {
        let body = r#"{
            "choices": [{
                "message": {
                    "content": "{\"new_nodes\":[],\"new_edges\":[],\"modified_entities\":[],\"position_hints\":[],\"reasoning\":\"no change needed\"}"
                }
            }],
            "usage": {"prompt_tokens": 155, "completion_tokens": 42, "total_tokens": 197}
        }"#;
        let c = parse_completion(body).unwrap();
        assert_eq!(c.usage.prompt_tokens, 155);
        assert_eq!(c.usage.completion_tokens, 42);
        assert_eq!(c.proposal.reasoning, "no change needed");
        assert!(c.proposal.new_nodes.is_empty());
    }

    #[test]
    fn parse_completion_strips_json_code_fence() {
        let body = r#"{
            "choices": [{
                "message": {
                    "content": "```json\n{\"reasoning\":\"fenced\"}\n```"
                }
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let c = parse_completion(body).unwrap();
        assert_eq!(c.proposal.reasoning, "fenced");
    }

    #[test]
    fn parse_completion_proposal_parse_error_carries_raw() {
        let body = r#"{
            "choices": [{"message": {"content": "not json at all"}}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        }"#;
        let err = parse_completion(body).unwrap_err();
        match err {
            LlmError::ProposalParse { raw, .. } => assert_eq!(raw, "not json at all"),
            other => panic!("expected ProposalParse, got {other:?}"),
        }
    }

    #[test]
    fn parse_completion_missing_choices_is_bad_response() {
        let body = r#"{"choices":[],"usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}"#;
        assert!(matches!(
            parse_completion(body),
            Err(LlmError::BadResponse(_))
        ));
    }

    #[test]
    fn strip_code_fence_plain_json_untouched() {
        assert_eq!(strip_code_fence(r#"{"a":1}"#), r#"{"a":1}"#);
    }
}
