use cassette_core::db::{
    CandidateReviewItem, TaskExecutionSummary, TrustLedgerGatekeeperAudit, TrustLedgerOperationEvent,
};
use cassette_core::librarian::models::{AcquisitionRequestEvent, AcquisitionRequestRow};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TrustLedgerSummary {
    pub stage: String,
    pub reason_code: String,
    pub headline: String,
    pub detail: String,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrustReasonDistributionEntry {
    pub reason_code: String,
    pub label: String,
    pub count: usize,
    pub stage: String,
}

pub fn trust_reason_label(code: &str) -> &'static str {
    match code {
        "request_recorded" => "Request recorded",
        "planning_in_progress" => "Planning in progress",
        "awaiting_operator_review" => "Waiting on approval",
        "runtime_in_progress" => "Runtime handoff in progress",
        "finalized_to_library" => "Delivered to library",
        "already_in_library" => "Already present",
        "cancelled_by_operator" => "Stopped before handoff",
        "auth_failed" => "Provider authentication failed",
        "rate_limited" => "Provider rate limited",
        "provider_busy" => "Provider busy",
        "provider_unhealthy" => "Provider unhealthy",
        "validation_failed" => "Candidate validation failed",
        "metadata_only" => "Metadata only",
        "no_result" => "No usable result",
        "unsupported" => "Unsupported by provider",
        "identity_mismatch" => "Identity mismatch",
        "quarantined" => "Quarantined by gatekeeper",
        "rejected_by_gatekeeper" => "Rejected by gatekeeper",
        "provider_exhausted" => "Provider stack exhausted",
        _ => "Needs review",
    }
}

pub fn derive_request_trust_summary(
    request: &AcquisitionRequestRow,
    timeline: &[AcquisitionRequestEvent],
    execution: Option<&TaskExecutionSummary>,
    candidate_review: &[CandidateReviewItem],
    operation_events: &[TrustLedgerOperationEvent],
    gatekeeper_audit: &[TrustLedgerGatekeeperAudit],
) -> TrustLedgerSummary {
    let evidence_count = timeline.len()
        + candidate_review.len()
        + operation_events.len()
        + gatekeeper_audit.len()
        + usize::from(execution.is_some())
        + usize::from(request.task_id.is_some());

    let latest_gatekeeper = gatekeeper_audit.last();
    let latest_event_type = timeline.last().map(|event| event.event_type.as_str());

    let stage = if matches!(request.status.as_str(), "failed" | "cancelled") {
        "blocked"
    } else if matches!(request.status.as_str(), "finalized" | "already_present") {
        "completed"
    } else if request.status == "reviewing" {
        "review"
    } else if matches!(request.status.as_str(), "queued" | "submitted" | "in_progress") {
        "runtime"
    } else {
        "planning"
    };

    let reason_code = if let Some(execution) = execution {
        match execution.disposition.as_str() {
            "Finalized" => "finalized_to_library".to_string(),
            "AlreadyPresent" => "already_in_library".to_string(),
            "MetadataOnly" => "metadata_only".to_string(),
            "Cancelled" => "cancelled_by_operator".to_string(),
            "Failed" => execution
                .failure_class
                .as_deref()
                .map(map_failure_class)
                .unwrap_or("provider_exhausted")
                .to_string(),
            _ => request_status_reason(request.status.as_str(), candidate_review),
        }
    } else if let Some(audit) = latest_gatekeeper {
        match audit.decision.to_ascii_lowercase().as_str() {
            "quarantined" => "quarantined".to_string(),
            "rejected" => "rejected_by_gatekeeper".to_string(),
            "admitted" => "runtime_in_progress".to_string(),
            _ => request_status_reason(request.status.as_str(), candidate_review),
        }
    } else if let Some(event_type) = latest_event_type {
        match event_type {
            "review_rejected" => "cancelled_by_operator".to_string(),
            "director_submit_failed" => "provider_exhausted".to_string(),
            _ => request_status_reason(request.status.as_str(), candidate_review),
        }
    } else {
        request_status_reason(request.status.as_str(), candidate_review)
    };

    let headline = trust_reason_label(&reason_code).to_string();
    let detail = match reason_code.as_str() {
        "awaiting_operator_review" => format!(
            "{} candidate(s) recorded and waiting for an explicit review decision.",
            candidate_review.len()
        ),
        "runtime_in_progress" => "Planner selected a path and the runtime is still working through provider, download, or verification steps.".to_string(),
        "planning_in_progress" => "The request is in the planner/runtime pipeline and evidence is still accumulating.".to_string(),
        "finalized_to_library" => execution
            .and_then(|value| value.final_path.clone())
            .map(|path| format!("Cassette finalized this request and recorded the destination at {path}."))
            .unwrap_or_else(|| "Cassette finalized this request and recorded a successful handoff.".to_string()),
        "already_in_library" => "The trust ledger shows the requested music was already present, so no destructive mutation was needed.".to_string(),
        "cancelled_by_operator" => "The request was intentionally stopped before final handoff.".to_string(),
        "auth_failed" => "A configured provider refused authentication, so the request could not continue on that lane.".to_string(),
        "rate_limited" => "A provider asked Cassette to back off, so progress paused instead of hammering the service.".to_string(),
        "provider_busy" => "The provider stack reported temporary load or cooldown pressure and Cassette held the request safely.".to_string(),
        "provider_unhealthy" => "The provider health checks marked this lane unhealthy, so Cassette avoided blind retries.".to_string(),
        "validation_failed" => "Candidates were found, but the evidence did not pass Cassette's validation rules.".to_string(),
        "metadata_only" => "The provider returned metadata context but not an acceptable acquisition candidate.".to_string(),
        "no_result" => "Cassette searched the configured provider lane and did not record a usable match.".to_string(),
        "unsupported" => "The provider lane answered, but not in a way this request can safely use.".to_string(),
        "identity_mismatch" => "The gatekeeper or validation evidence says the bytes found do not confidently match the requested identity.".to_string(),
        "quarantined" => "Gatekeeper admitted the file to evidence review instead of the library because it failed a safety or identity rule.".to_string(),
        "rejected_by_gatekeeper" => "Gatekeeper rejected the file before library mutation, preserving reversibility and auditability.".to_string(),
        "provider_exhausted" => "The configured provider stack was tried without reaching a successful handoff.".to_string(),
        _ => "Cassette recorded the request, but the current trust explanation still needs a more specific reason code.".to_string(),
    };

    TrustLedgerSummary {
        stage: stage.to_string(),
        reason_code,
        headline,
        detail,
        evidence_count,
    }
}

pub fn summarize_reason_distribution(
    summaries: &[TrustLedgerSummary],
    top_n: usize,
) -> Vec<TrustReasonDistributionEntry> {
    let mut counts = std::collections::BTreeMap::<(String, String), usize>::new();
    for summary in summaries {
        *counts
            .entry((summary.reason_code.clone(), summary.stage.clone()))
            .or_default() += 1;
    }

    let mut rows = counts
        .into_iter()
        .map(|((reason_code, stage), count)| TrustReasonDistributionEntry {
            label: trust_reason_label(&reason_code).to_string(),
            reason_code,
            count,
            stage,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.reason_code.cmp(&right.reason_code))
    });
    rows.truncate(top_n);
    rows
}

fn request_status_reason(status: &str, candidate_review: &[CandidateReviewItem]) -> String {
    if status == "reviewing" {
        "awaiting_operator_review".to_string()
    } else if matches!(status, "queued" | "submitted" | "in_progress") {
        if candidate_review.iter().any(|candidate| candidate.is_selected) {
            "runtime_in_progress".to_string()
        } else {
            "planning_in_progress".to_string()
        }
    } else if status == "finalized" {
        "finalized_to_library".to_string()
    } else if status == "already_present" {
        "already_in_library".to_string()
    } else if status == "cancelled" {
        "cancelled_by_operator".to_string()
    } else {
        "request_recorded".to_string()
    }
}

fn map_failure_class(code: &str) -> &'static str {
    match code {
        "auth_failed" => "auth_failed",
        "rate_limited" => "rate_limited",
        "provider_busy" => "provider_busy",
        "provider_unhealthy" => "provider_unhealthy",
        "validation_failed" => "validation_failed",
        "metadata_only" => "metadata_only",
        "no_result" => "no_result",
        "unsupported" => "unsupported",
        "identity_mismatch" => "identity_mismatch",
        "policy_rejected" => "unsupported",
        "search_error" | "acquire_failed" => "provider_exhausted",
        other if other.contains("auth") => "auth_failed",
        other if other.contains("rate") || other.contains("429") => "rate_limited",
        other if other.contains("busy") || other.contains("cooldown") => "provider_busy",
        other if other.contains("health") || other.contains("outage") => "provider_unhealthy",
        other if other.contains("validation") => "validation_failed",
        other if other.contains("identity") => "identity_mismatch",
        _ => "provider_exhausted",
    }
}
