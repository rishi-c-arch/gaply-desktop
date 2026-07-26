//! Targeted-escalation payloads + gate (Box 2, the PURE half).
//!
//! ⚠️ HONEST BOUNDARY — what ships vs. what works: this module builds and gates
//! the per-finding escalation CALL, but escalation only *functions* once TWO
//! things exist, NEITHER of which is part of this Rust rebuild:
//!   (a) gaply-proxy implements a `task:"escalate_findings"` endpoint
//!       (server-side, Python — out of scope, provider-blind), and
//!   (b) the reviewer-quality spike — does the LLM produce trustworthy, GROUNDED
//!       per-finding verdicts on real papers (the architecture doc's Q12 risk,
//!       still unresolved) — is run and passes.
//! Until both land, the app-crate orchestration degrades every call to
//! "unavailable". This is a structural fact about post-commit behaviour, not a
//! "coming soon" note.
//!
//! PRIVACY (free by construction): an [`EvidenceRecord`] carries NO raw
//! manuscript text — only structured ids/severity/confidence/provenance refs — so
//! a payload built from records is text-free without any filtering. A test pins it.

use serde::Serialize;
use serde_json::{json, Value};

use crate::evidence::EvidenceRecord;
use crate::evidence_store::EscalationOutcome;
use crate::orchestrator::ContextAttachment;
use crate::GaplyError;

/// Prose instruction sent with each batch. SUBJECT TO the pending reviewer-quality
/// spike — the exact wording/schema that yields trustworthy per-finding verdicts
/// is unproven; this is a structural first version, not a validated prompt.
const ESCALATION_INSTRUCTION: &str = "You are adjudicating specific flagged findings from a \
local research-integrity analysis. For EACH finding id provided, return a JSON object in \
`verdicts` with: `finding_id` (echoing one of the ids given), `verdict` (SUPPORTED, REFUTED, \
or UNKNOWN), `rationale`, and `evidence_refs` (a subset of the structured refs supplied for \
that finding). Cite ONLY the ids and refs supplied; if the evidence does not settle it, \
return UNKNOWN. Never invent findings, refs, or manuscript content.";

/// The ids/refs actually sent — the gate validates the reply grounds in exactly these.
pub struct EscalationSent {
    pub finding_ids: Vec<String>,
    pub evidence_refs: Vec<String>,
}

fn enum_text<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|j| j.as_str().map(String::from)).unwrap_or_default()
}

/// Build one escalation batch payload (structured-only; text-free by
/// construction) + the [`EscalationSent`] ids the gate checks against.
pub fn build_escalation_payload(
    batch: &[&EvidenceRecord],
    context: &[ContextAttachment],
) -> (Value, EscalationSent) {
    let mut finding_ids = Vec::new();
    let mut evidence_refs = Vec::new();
    let findings: Vec<Value> = batch
        .iter()
        .map(|r| {
            finding_ids.push(r.id.clone());
            evidence_refs.extend(r.evidence_refs.iter().cloned());
            json!({
                "id": r.id,
                "agent": enum_text(&r.agent),
                "severity": enum_text(&r.severity),
                "confidence": r.confidence,
                "confidence_kind": enum_text(&r.confidence_kind),
                "evidence": r.evidence_refs,
            })
        })
        .collect();
    let ctx: Vec<Value> =
        context.iter().map(|c| json!({ "source": c.source_finding_id, "refs": c.refs })).collect();
    let payload = json!({
        "task": "escalate_findings",
        "instruction": ESCALATION_INSTRUCTION,
        "summary": { "findings": findings, "context": ctx },
    });
    (payload, EscalationSent { finding_ids, evidence_refs })
}

/// Gate the reply. Per SENT finding: a verdict whose cited `evidence_refs` are ALL
/// grounded in what we sent → verified outcome; a verdict citing ungrounded refs
/// → recorded as gate-REJECTED (`verified=false` = three-state 0, flagged); a
/// finding the reply omits gets NO outcome (the caller leaves it NULL =
/// escalation-unavailable). A verdict for an id we did not send is IGNORED (never
/// invent a finding). `provider`/`provider_model` are RECORDED from the reply's
/// uniform JSON, never branched on.
pub fn gate_escalation_response(
    resp: &Value,
    sent: &EscalationSent,
) -> Result<Vec<(String, EscalationOutcome)>, GaplyError> {
    let provider = resp["provider"].as_str().unwrap_or("").to_string();
    let provider_model = resp["provider_model"].as_str().unwrap_or("").to_string();
    let verdicts = resp["verdicts"].as_array().ok_or_else(|| {
        GaplyError::Validation("escalation schema: missing 'verdicts' array".into())
    })?;

    let mut out = Vec::new();
    for v in verdicts {
        let fid = match v["finding_id"].as_str() {
            Some(s) if sent.finding_ids.iter().any(|k| k == s) => s.to_string(),
            _ => continue, // unknown / absent id → ignore
        };
        let verdict = v["verdict"].as_str().unwrap_or("UNKNOWN").to_string();
        let rationale = v["rationale"].as_str().unwrap_or("").to_string();
        let refs: Vec<String> = v["evidence_refs"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let ungrounded: Vec<String> =
            refs.iter().filter(|r| !sent.evidence_refs.iter().any(|s| s == *r)).cloned().collect();
        let (verified, gate_flags) = if ungrounded.is_empty() {
            (true, Vec::new())
        } else {
            (false, vec![format!("ungrounded_refs_dropped: {}", ungrounded.join(","))])
        };
        out.push((
            fid,
            EscalationOutcome {
                llm_verdict: verdict,
                llm_rationale: rationale,
                verified,
                gate_flags,
                provider: provider.clone(),
                provider_model: provider_model.clone(),
            },
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::FindingSeverity;
    use crate::swarm::AgentKind;

    fn rec(id: &str, agent: AgentKind, conf: f64, refs: &[&str]) -> EvidenceRecord {
        EvidenceRecord::at_source(
            id,
            agent,
            FindingSeverity::Major,
            conf,
            refs.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn payload_is_structured_only_no_manuscript_text() {
        const RAW_SENTINEL: &str = "the quick brown manuscript sentence";
        let r = rec("f1", AgentKind::Plagiarism, 0.85, &["similarity:0.850", "source:corpus"]);
        let (payload, sent) = build_escalation_payload(&[&r], &[]);
        let s = serde_json::to_string(&payload).unwrap();
        assert!(!s.contains(RAW_SENTINEL), "payload must carry no raw manuscript text");
        assert!(s.contains("escalate_findings"));
        assert!(s.contains("similarity:0.850"));
        assert_eq!(sent.finding_ids, vec!["f1"]);
        assert!(sent.evidence_refs.contains(&"source:corpus".to_string()));
    }

    #[test]
    fn gate_accepts_grounded_verdict() {
        let sent = EscalationSent {
            finding_ids: vec!["f1".into()],
            evidence_refs: vec!["similarity:0.850".into()],
        };
        let resp = json!({
            "provider": "openai", "provider_model": "gpt-5.5",
            "verdicts": [{ "finding_id": "f1", "verdict": "REFUTED",
                           "rationale": "match confirmed", "evidence_refs": ["similarity:0.850"] }]
        });
        let out = gate_escalation_response(&resp, &sent).unwrap();
        assert_eq!(out.len(), 1);
        let (fid, o) = &out[0];
        assert_eq!(fid, "f1");
        assert!(o.verified);
        assert_eq!(o.llm_verdict, "REFUTED");
        assert_eq!(o.provider, "openai");
        assert!(o.gate_flags.is_empty());
    }

    #[test]
    fn gate_rejects_ungrounded_refs() {
        let sent =
            EscalationSent { finding_ids: vec!["f1".into()], evidence_refs: vec!["similarity:0.850".into()] };
        let resp = json!({
            "provider": "openai", "provider_model": "gpt-5.5",
            "verdicts": [{ "finding_id": "f1", "verdict": "REFUTED",
                           "rationale": "cites a source we never sent",
                           "evidence_refs": ["evidence:made-up-123"] }]
        });
        let out = gate_escalation_response(&resp, &sent).unwrap();
        let (_, o) = &out[0];
        assert!(!o.verified, "ungrounded refs → gate-rejected (verified=false)");
        assert!(o.gate_flags[0].contains("ungrounded_refs_dropped"));
    }

    #[test]
    fn gate_ignores_verdict_for_unsent_finding() {
        let sent = EscalationSent { finding_ids: vec!["f1".into()], evidence_refs: vec![] };
        let resp = json!({
            "provider": "x", "provider_model": "y",
            "verdicts": [{ "finding_id": "f999", "verdict": "REFUTED", "rationale": "", "evidence_refs": [] }]
        });
        let out = gate_escalation_response(&resp, &sent).unwrap();
        assert!(out.is_empty(), "never invent a finding we didn't send");
    }
}
