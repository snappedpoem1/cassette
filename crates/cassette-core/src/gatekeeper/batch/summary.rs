use crate::gatekeeper::mod_types::{AdmissionDecision, BatchIngestOutcome, IngressOutcome};

pub fn summarize(outcomes: Vec<IngressOutcome>) -> BatchIngestOutcome {
    let total_files = outcomes.len();
    let mut admitted = 0;
    let mut quarantined = 0;
    let mut rejected = 0;
    let mut duplicates_detected = 0;

    for outcome in &outcomes {
        match &outcome.decision {
            AdmissionDecision::Admitted { .. } => admitted += 1,
            AdmissionDecision::Quarantined { reason, .. } => {
                quarantined += 1;
                if matches!(reason, crate::gatekeeper::mod_types::QuarantineReason::DuplicateDetected) {
                    duplicates_detected += 1;
                }
            }
            AdmissionDecision::Rejected { .. } => rejected += 1,
        }
    }

    BatchIngestOutcome {
        total_files,
        admitted,
        quarantined,
        rejected,
        duplicates_detected,
        audit_entries: outcomes,
    }
}
