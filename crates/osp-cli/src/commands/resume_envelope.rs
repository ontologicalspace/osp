//! #164 — Resume run envelope: `osp trajectory resume` truth-surface çıktısı.
//!
//! Attempt'in `run_envelope.rs` mirror'ı: resume sonucunu versioned JSON olarak
//! sunar (result kind + quorum snapshot). Run envelope'ından ayrı şekil — resume
//! LLM'siz operatör akışıdır; evidence/execution_measurement alanları yok
//! (karar zinciri attempt anında koşuldu; resume yalnız fence + witness + apply).

use osp_core::engine::ResumeHeldOutcome;

/// Resume result kind (snake_case wire).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliResumeResultKind {
    /// Quorum karşılandı → kayıtlı delta uygulandı (exit 0).
    Applied,
    /// Quorum hâlâ yetersiz → artifact geçerli kalır (exit 10).
    StillHeld,
    /// Explicit witness ret (exit 11).
    Rejected,
}

/// Quorum anlık görüntüsü — resume karıtının kanıtı.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliResumeQuorumSnapshot {
    pub approvers: usize,
    pub required_approvers: usize,
    pub support: f64,
    pub required_support: f64,
}

/// Serializable resume result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliResumeResult {
    pub kind: CliResumeResultKind,
    pub witness: CliResumeQuorumSnapshot,
}

/// Versioned resume envelope — V1.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliResumeEnvelopeV1 {
    pub schema_version: u32,
    /// Resume edilen artifact path'i.
    pub artifact: String,
    pub task_id: u64,
    pub claim_id: u64,
    pub attempt_num: u64,
    pub result: CliResumeResult,
}

/// Build V1 resume envelope from engine outcome + artifact identity alanları.
pub fn build_resume_envelope_v1(
    outcome: &ResumeHeldOutcome,
    artifact: &std::path::Path,
    task_id: u64,
    claim_id: u64,
    attempt_num: u64,
) -> CliResumeEnvelopeV1 {
    let (kind, witness) = match outcome {
        ResumeHeldOutcome::Applied { snapshot, .. } => {
            (CliResumeResultKind::Applied, clone_snapshot(snapshot))
        }
        ResumeHeldOutcome::StillHeld { snapshot, .. } => {
            (CliResumeResultKind::StillHeld, clone_snapshot(snapshot))
        }
        ResumeHeldOutcome::Rejected { snapshot, .. } => {
            (CliResumeResultKind::Rejected, clone_snapshot(snapshot))
        }
    };
    CliResumeEnvelopeV1 {
        schema_version: 1,
        artifact: artifact.display().to_string(),
        task_id,
        claim_id,
        attempt_num,
        result: CliResumeResult { kind, witness },
    }
}

fn clone_snapshot(snapshot: &osp_core::witness::WitnessQuorumSnapshot) -> CliResumeQuorumSnapshot {
    CliResumeQuorumSnapshot {
        approvers: snapshot.approvers,
        required_approvers: snapshot.required_approvers,
        support: snapshot.support,
        required_support: snapshot.required_support,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osp_core::witness::WitnessQuorumSnapshot;

    fn snapshot() -> WitnessQuorumSnapshot {
        WitnessQuorumSnapshot {
            approvers: 2,
            required_approvers: 2,
            support: 2.0,
            required_support: 1.5,
        }
    }

    #[test]
    fn applied_envelope_serializes_snake_case() {
        let outcome = ResumeHeldOutcome::Applied {
            resulting_sequence: 1,
            snapshot: snapshot(),
        };
        let envelope = build_resume_envelope_v1(
            &outcome,
            std::path::Path::new(".osp/pending-authorizations/task-7.json"),
            7,
            42,
            1,
        );
        let json = serde_json::to_string(&envelope).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["task_id"], 7);
        assert_eq!(v["claim_id"], 42);
        assert_eq!(v["result"]["kind"], "applied");
        assert_eq!(v["result"]["witness"]["approvers"], 2);
        assert_eq!(v["result"]["witness"]["required_support"], 1.5);
    }

    #[test]
    fn still_held_envelope_serializes_snake_case() {
        let outcome = ResumeHeldOutcome::StillHeld {
            reason: osp_core::witness::WitnessHoldReason::MinApproversNotMet {
                distinct: 1,
                required: 2,
            },
            snapshot: WitnessQuorumSnapshot {
                approvers: 1,
                required_approvers: 2,
                support: 1.0,
                required_support: 1.5,
            },
        };
        let envelope = build_resume_envelope_v1(&outcome, std::path::Path::new("a.json"), 7, 42, 1);
        let json = serde_json::to_string(&envelope).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["result"]["kind"], "still_held");
        assert_eq!(v["result"]["witness"]["approvers"], 1);
    }
}
