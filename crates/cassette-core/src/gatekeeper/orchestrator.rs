use crate::gatekeeper::config::GatekeeperConfig;
use crate::gatekeeper::database::audit_log::insert_audit_entry;
use crate::gatekeeper::database::enrichment_queue::enqueue_enrichment;
use crate::gatekeeper::database::local_files::upsert_local_file;
use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::{
    AdmissionDecision, AudioTags, AuditLogEntry, IngressOutcome, NextAction, QuarantineReason,
};
use crate::gatekeeper::placement::admission::decide_and_place;
use crate::gatekeeper::placement::quarantine::move_to_quarantine;
use crate::gatekeeper::validation::fingerprint::compute_fingerprint;
use crate::gatekeeper::validation::identity_proof::prove_identity;
use crate::gatekeeper::validation::junk_filter::apply_junk_filters;
use crate::gatekeeper::validation::payload_probe::probe_payload;
use crate::gatekeeper::validation::quality_assess::assess_quality;
use crate::librarian::models::DesiredTrack;
use chrono::Utc;
use lofty::prelude::{Accessor, TaggedFileExt};
use std::path::Path;
use uuid::Uuid;

pub async fn ingest_single_file(
    db_pool: &sqlx::SqlitePool,
    config: &GatekeeperConfig,
    source_path: &Path,
    desired_track: Option<&DesiredTrack>,
) -> Result<IngressOutcome> {
    let operation_id = Uuid::new_v4().to_string();
    let started = Utc::now();

    let probe = probe_payload(source_path).await?;
    let quality = assess_quality(&probe, config, false)?;
    let fingerprint = compute_fingerprint(source_path).await?;
    let content_hash = compute_content_hash(source_path).await?;
    let (identity, matched_local_file_id) = prove_identity(
        &fingerprint,
        &probe,
        &quality,
        &content_hash,
        desired_track,
        db_pool,
    )
    .await?;

    let tags = read_audio_tags(source_path);
    let junk_flags = apply_junk_filters(&probe, &tags, &config.policy_spine)?;

    let (decision, final_path, next_action) = if config.reject_junk_files && !junk_flags.is_empty()
    {
        let qpath = move_to_quarantine(
            source_path,
            &config.quarantine_root,
            AdmissionDecision::Quarantined {
                reason: QuarantineReason::JunkFilterTriggered,
                manual_review_required: true,
            },
        )
        .await?;
        (
            AdmissionDecision::Quarantined {
                reason: QuarantineReason::JunkFilterTriggered,
                manual_review_required: true,
            },
            qpath,
            NextAction::ManualReview {
                reason: "Junk policy triggered".to_string(),
            },
        )
    } else if config.require_identity_match
        && identity.acoustid_confidence < config.fingerprint_confidence_floor
    {
        let qpath = move_to_quarantine(
            source_path,
            &config.quarantine_root,
            AdmissionDecision::Quarantined {
                reason: QuarantineReason::IdentityMismatch,
                manual_review_required: true,
            },
        )
        .await?;
        (
            AdmissionDecision::Quarantined {
                reason: QuarantineReason::IdentityMismatch,
                manual_review_required: true,
            },
            qpath,
            NextAction::ManualReview {
                reason: "Identity confidence below floor".to_string(),
            },
        )
    } else if config.reject_below_floor
        && (!quality.passes_bitrate_floor || !quality.passes_sample_rate_floor)
    {
        let qpath = move_to_quarantine(
            source_path,
            &config.quarantine_root,
            AdmissionDecision::Quarantined {
                reason: QuarantineReason::QualityBelowFloor,
                manual_review_required: false,
            },
        )
        .await?;
        (
            AdmissionDecision::Quarantined {
                reason: QuarantineReason::QualityBelowFloor,
                manual_review_required: false,
            },
            qpath,
            NextAction::None,
        )
    } else {
        let (decision, paths) = decide_and_place(
            &quality,
            &fingerprint,
            &content_hash,
            source_path,
            desired_track,
            db_pool,
            config,
        )
        .await?;

        let next = match decision {
            AdmissionDecision::Admitted { .. } if config.enrichment_queue_enabled => {
                NextAction::TriggerEnrichment {
                    local_file_id: 0,
                    track_id: None,
                }
            }
            AdmissionDecision::Quarantined {
                manual_review_required: true,
                ..
            } => NextAction::ManualReview {
                reason: "Duplicate or policy review".to_string(),
            },
            _ => NextAction::None,
        };

        (decision, paths.final_path, next)
    };

    let mut action = next_action;
    if let AdmissionDecision::Admitted { .. } = decision {
        let local_file_id = upsert_local_file(
            db_pool,
            &final_path,
            &identity,
            &quality,
            desired_track.map(|d| d.id),
        )
        .await?;
        if config.enrichment_queue_enabled {
            enqueue_enrichment(
                db_pool,
                local_file_id,
                desired_track.map(|d| d.id),
                "gatekeeper_ingest",
            )
            .await?;
            action = NextAction::TriggerEnrichment {
                local_file_id,
                track_id: desired_track.map(|d| d.id),
            };
        }
    }

    let ended = Utc::now();
    let duration_ms = (ended - started).num_milliseconds().max(0) as u64;

    let audit = AuditLogEntry {
        operation_id: operation_id.clone(),
        timestamp: ended,
        file_path: source_path.to_path_buf(),
        decision: decision.clone(),
        identity_proof: Some(identity.clone()),
        quality_assessment: quality.clone(),
        junk_flags: junk_flags.clone(),
        desired_track_id: desired_track.map(|d| d.id),
        matched_local_file_id,
        duration_ms,
        notes: "gatekeeper ingest".to_string(),
    };

    insert_audit_entry(db_pool, &audit).await?;

    Ok(IngressOutcome {
        file_path: source_path.to_path_buf(),
        decision,
        identity_proof: Some(identity),
        quality_assessment: quality,
        junk_flags,
        audit_log: audit,
        next_action: action,
    })
}

fn read_audio_tags(path: &Path) -> AudioTags {
    let tagged = lofty::read_from_path(path).ok();
    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut isrc = None;
    let mut track_number = None;
    let mut disc_number = None;

    if let Some(tagged) = tagged {
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            title = tag.title().map(|v| v.to_string());
            artist = tag.artist().map(|v| v.to_string());
            album = tag.album().map(|v| v.to_string());
            track_number = tag.track();
            disc_number = tag.disk();
            isrc = None;
        }
    }

    AudioTags {
        title,
        artist,
        album,
        isrc,
        track_number,
        disc_number,
    }
}

async fn compute_content_hash(path: &Path) -> Result<String> {
    let data = tokio::fs::read(path).await?;
    Ok(blake3::hash(&data).to_hex().to_string())
}
