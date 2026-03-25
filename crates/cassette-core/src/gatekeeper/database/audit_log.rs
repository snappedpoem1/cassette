use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::{AdmissionDecision, AuditLogEntry};

pub async fn insert_audit_entry(db_pool: &sqlx::SqlitePool, entry: &AuditLogEntry) -> Result<()> {
    let quality_json = serde_json::to_string(&entry.quality_assessment)?;
    let identity_json = serde_json::to_string(&entry.identity_proof)?;
    let junk_json = serde_json::to_string(&entry.junk_flags)?;

    sqlx::query(
        "INSERT INTO gatekeeper_audit_log (
            operation_id,
            timestamp,
            file_path,
            decision,
            desired_track_id,
            matched_local_file_id,
            duration_ms,
            notes,
            quality_json,
            identity_json,
            junk_flags_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )
    .bind(&entry.operation_id)
    .bind(entry.timestamp.to_rfc3339())
    .bind(entry.file_path.to_string_lossy().to_string())
    .bind(decision_label(&entry.decision))
    .bind(entry.desired_track_id)
    .bind(entry.matched_local_file_id)
    .bind(entry.duration_ms as i64)
    .bind(&entry.notes)
    .bind(quality_json)
    .bind(identity_json)
    .bind(junk_json)
    .execute(db_pool)
    .await?;

    Ok(())
}

fn decision_label(decision: &AdmissionDecision) -> &'static str {
    match decision {
        AdmissionDecision::Admitted { .. } => "admitted",
        AdmissionDecision::Quarantined { .. } => "quarantined",
        AdmissionDecision::Rejected { .. } => "rejected",
    }
}
