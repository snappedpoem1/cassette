use crate::librarian::models::DesiredTrack;
use crate::orchestrator::reconciliation::normalization::normalize_name;
use crate::orchestrator::types::LocalFileMatch;

pub fn compute_metadata_confidence(
    desired: &DesiredTrack,
    local: &LocalFileMatch,
    duration_tolerance_ms: u32,
) -> f32 {
    let mut score = 0.0;

    if normalize_name(&desired.artist_name) == normalize_name(&local.artist_name) {
        score += 0.4;
    }

    if let Some(album) = desired.album_title.as_ref() {
        if !album.trim().is_empty() && normalize_name(album) == normalize_name(&local.album_title) {
            score += 0.3;
        }
    }

    if normalize_name(&desired.track_title) == normalize_name(&local.title) {
        score += 0.2;
    }

    let desired_duration = desired.duration_ms.unwrap_or_default();
    if desired_duration > 0 {
        let delta = (desired_duration - local.duration_ms as i64).abs() as u32;
        if delta <= duration_tolerance_ms {
            score += 0.1;
        }
    }

    score
}
