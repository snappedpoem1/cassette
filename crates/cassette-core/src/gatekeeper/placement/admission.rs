use crate::gatekeeper::config::GatekeeperConfig;
use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::{AdmissionDecision, DuplicatePolicyOutcome, QuarantineReason, QualityAssessment};
use crate::gatekeeper::placement::collision::detect_duplicates;
use crate::librarian::models::DesiredTrack;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct AdmissionPaths {
    pub final_path: PathBuf,
    pub quarantine_path: Option<PathBuf>,
}

pub async fn decide_and_place(
    quality: &QualityAssessment,
    fingerprint: &str,
    content_hash: &str,
    source_path: &Path,
    desired_track: Option<&DesiredTrack>,
    db_pool: &sqlx::SqlitePool,
    config: &GatekeeperConfig,
) -> Result<(AdmissionDecision, AdmissionPaths)> {
    if let Some(conflict) = detect_duplicates(
        fingerprint,
        content_hash,
        quality.quality_tier,
        db_pool,
        config,
    )
    .await?
    {
        match conflict.policy_decision {
            DuplicatePolicyOutcome::ReplaceExisting | DuplicatePolicyOutcome::MarkBothKeepBest => {
                let canonical = build_canonical_path(source_path, desired_track, config)?;
                copy_to_path(source_path, &canonical).await?;
                let decision = AdmissionDecision::Admitted {
                    canonical_path: canonical.clone(),
                    confidence: 0.95,
                };
                return Ok((
                    decision,
                    AdmissionPaths {
                        final_path: canonical,
                        quarantine_path: None,
                    },
                ));
            }
            DuplicatePolicyOutcome::ManualReview | DuplicatePolicyOutcome::KeepExisting => {
                let qpath = build_quarantine_path(source_path, QuarantineReason::DuplicateDetected, config)?;
                copy_to_path(source_path, &qpath).await?;
                let decision = AdmissionDecision::Quarantined {
                    reason: QuarantineReason::DuplicateDetected,
                    manual_review_required: true,
                };
                return Ok((
                    decision,
                    AdmissionPaths {
                        final_path: qpath.clone(),
                        quarantine_path: Some(qpath),
                    },
                ));
            }
        }
    }

    let canonical = build_canonical_path(source_path, desired_track, config)?;
    copy_to_path(source_path, &canonical).await?;

    let decision = AdmissionDecision::Admitted {
        canonical_path: canonical.clone(),
        confidence: 0.9,
    };

    Ok((
        decision,
        AdmissionPaths {
            final_path: canonical,
            quarantine_path: None,
        },
    ))
}

async fn copy_to_path(source_path: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::copy(source_path, destination).await?;
    Ok(())
}

fn sanitize_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let ok = ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.');
        out.push(if ok { ch } else { '_' });
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "Unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_canonical_path(
    source_path: &Path,
    desired_track: Option<&DesiredTrack>,
    config: &GatekeeperConfig,
) -> Result<PathBuf> {
    let artist = sanitize_component(
        desired_track
            .map(|d| d.artist_name.as_str())
            .unwrap_or("Unknown Artist"),
    );
    let album = sanitize_component(
        desired_track
            .and_then(|d| d.album_title.as_deref())
            .unwrap_or("Unknown Album"),
    );
    let title = sanitize_component(
        desired_track
            .map(|d| d.track_title.as_str())
            .unwrap_or("Unknown Title"),
    );
    let ext = source_path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("bin");

    let file_name = if let Some(track) = desired_track.and_then(|d| d.track_number) {
        format!("{:02} - {}.{}", track, title, ext)
    } else {
        format!("{}.{}", title, ext)
    };

    Ok(config.canonical_library_root.join(artist).join(album).join(file_name))
}

fn build_quarantine_path(source_path: &Path, reason: QuarantineReason, config: &GatekeeperConfig) -> Result<PathBuf> {
    let name = source_path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown.bin");
    Ok(config.quarantine_root.join(reason.as_dir()).join(name))
}
