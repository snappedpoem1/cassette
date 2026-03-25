pub mod fuzzy;
pub mod isrc;
pub mod metadata;

use crate::librarian::db::LibrarianDb;
use crate::librarian::error::Result;
use crate::librarian::matchers::fuzzy::fuzzy_within_distance;
use crate::librarian::matchers::isrc::isrc_match;
use crate::librarian::matchers::metadata::duration_within_tolerance;
use crate::librarian::models::{DesiredTrack, MatchOutcome, ReconciliationStatus};
use crate::librarian::normalize::{album::normalize_album_title, artist::normalize_artist_name, track::normalize_track_title};

pub async fn match_desired_track(db: &LibrarianDb, desired: &DesiredTrack) -> Result<MatchOutcome> {
    if let Some(isrc) = desired.isrc.as_deref() {
        if let Some(track) = db.find_track_by_isrc(isrc).await? {
            let files = db.list_local_files_for_track(track.id).await?;
            let matched_file = files.first().map(|f| f.id);
            return Ok(MatchOutcome {
                status: ReconciliationStatus::ExactMatch,
                matched_track_id: Some(track.id),
                matched_local_file_id: matched_file,
                confidence: 1.0,
                reason: "ISRC match".to_string(),
            });
        }
    }

    let norm_artist = normalize_artist_name(&desired.artist_name);
    let norm_album = desired.album_title.as_deref().map(normalize_album_title);
    let norm_title = normalize_track_title(&desired.track_title);

    let strong = db
        .strong_match_candidates(&norm_artist, norm_album.as_deref(), &norm_title)
        .await?;

    let mut strong_matches = Vec::new();
    for (track, file) in strong {
        let isrc_ok = isrc_match(desired.isrc.as_deref(), track.isrc.as_deref());
        let duration_ok = duration_within_tolerance(desired.duration_ms, track.duration_ms, 2000);
        if isrc_ok || duration_ok {
            strong_matches.push((track.id, file.id, if isrc_ok { "isrc+metadata" } else { "strong metadata" }));
        }
    }

    if strong_matches.len() == 1 {
        let (track_id, local_file_id, reason) = strong_matches[0];
        return Ok(MatchOutcome {
            status: ReconciliationStatus::StrongMatch,
            matched_track_id: Some(track_id),
            matched_local_file_id: Some(local_file_id),
            confidence: 0.9,
            reason: reason.to_string(),
        });
    }

    if strong_matches.len() > 1 {
        return Ok(MatchOutcome {
            status: ReconciliationStatus::Duplicate,
            matched_track_id: Some(strong_matches[0].0),
            matched_local_file_id: Some(strong_matches[0].1),
            confidence: 0.65,
            reason: "multiple strong local matches".to_string(),
        });
    }

    let fuzzy = db.fuzzy_candidates_for_artist(&norm_artist).await?;
    let mut fuzzy_hit: Option<(i64, i64)> = None;
    let mut fuzzy_count = 0usize;
    for (track, file) in fuzzy {
        if fuzzy_within_distance(&norm_title, &track.normalized_title, 2) {
            fuzzy_count += 1;
            if fuzzy_hit.is_none() {
                fuzzy_hit = Some((track.id, file.id));
            }
        }
    }

    if fuzzy_count == 1 {
        let (track_id, file_id) = fuzzy_hit.expect("present");
        return Ok(MatchOutcome {
            status: ReconciliationStatus::WeakMatch,
            matched_track_id: Some(track_id),
            matched_local_file_id: Some(file_id),
            confidence: 0.45,
            reason: "single bounded fuzzy match".to_string(),
        });
    }

    if fuzzy_count > 1 {
        return Ok(MatchOutcome {
            status: ReconciliationStatus::ManualReview,
            matched_track_id: fuzzy_hit.map(|v| v.0),
            matched_local_file_id: fuzzy_hit.map(|v| v.1),
            confidence: 0.2,
            reason: "multiple fuzzy candidates require manual review".to_string(),
        });
    }

    Ok(MatchOutcome {
        status: ReconciliationStatus::Missing,
        matched_track_id: None,
        matched_local_file_id: None,
        confidence: 0.0,
        reason: "no local match".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::fuzzy::levenshtein;

    #[test]
    fn levenshtein_distance_is_bounded() {
        assert_eq!(levenshtein("track", "track"), 0);
        assert_eq!(levenshtein("track", "tracks"), 1);
        assert_eq!(levenshtein("song", "s0ng"), 1);
    }
}
