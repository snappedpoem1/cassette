pub mod fuzzy;
pub mod isrc;
pub mod metadata;

use crate::librarian::db::LibrarianDb;
use crate::librarian::error::Result;
use crate::librarian::matchers::fuzzy::fuzzy_within_distance;
use crate::librarian::matchers::isrc::isrc_match;
use crate::librarian::matchers::metadata::duration_within_tolerance;
use crate::librarian::models::{DesiredTrack, LocalFile, MatchOutcome, ReconciliationStatus, Track};
use crate::librarian::normalize::{album::normalize_album_title, artist::normalize_artist_name, track::normalize_track_title};
use std::collections::BTreeMap;

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

    let strong_candidates = db
        .strong_match_candidates(&norm_artist, norm_album.as_deref(), &norm_title)
        .await?;

    let mut strong_matches = BTreeMap::new();
    for (track, file) in strong_candidates {
        let isrc_ok = isrc_match(desired.isrc.as_deref(), track.isrc.as_deref());
        let duration_ok = duration_within_tolerance(desired.duration_ms, track.duration_ms, 2000);
        if isrc_ok || duration_ok {
            let reason = if isrc_ok { "isrc+metadata" } else { "strong metadata" };
            upsert_best_candidate(
                &mut strong_matches,
                CandidateMatch::new(track, file, reason),
            );
        }
    }

    if strong_matches.len() == 1 {
        let candidate = strong_matches
            .into_values()
            .next()
            .expect("single strong candidate");
        return Ok(MatchOutcome {
            status: ReconciliationStatus::StrongMatch,
            matched_track_id: Some(candidate.track_id),
            matched_local_file_id: Some(candidate.local_file_id),
            confidence: 0.9,
            reason: candidate.reason.to_string(),
        });
    }

    if strong_matches.len() > 1 {
        let candidate = strong_matches
            .into_values()
            .next()
            .expect("duplicate strong candidate");
        return Ok(MatchOutcome {
            status: ReconciliationStatus::Duplicate,
            matched_track_id: Some(candidate.track_id),
            matched_local_file_id: Some(candidate.local_file_id),
            confidence: 0.65,
            reason: "multiple strong local matches".to_string(),
        });
    }

    let fuzzy = db.fuzzy_candidates_for_artist(&norm_artist).await?;

    let exact_title_candidates = dedupe_candidates(
        fuzzy.iter()
            .filter(|(track, _)| normalize_track_title(&track.title) == norm_title)
            .map(|(track, file)| CandidateMatch::new(track.clone(), file.clone(), "exact title")),
    );
    if let Some(outcome) = resolve_exact_title_candidates(desired, &exact_title_candidates) {
        return Ok(outcome);
    }

    let fuzzy_candidates = dedupe_candidates(
        fuzzy.into_iter()
            .filter(|(track, _)| {
                let track_title = normalize_track_title(&track.title);
                fuzzy_within_distance(&norm_title, &track_title, 2)
            })
            .map(|(track, file)| CandidateMatch::new(track, file, "single bounded fuzzy match")),
    );

    if fuzzy_candidates.len() == 1 {
        let candidate = &fuzzy_candidates[0];
        return Ok(MatchOutcome {
            status: ReconciliationStatus::WeakMatch,
            matched_track_id: Some(candidate.track_id),
            matched_local_file_id: Some(candidate.local_file_id),
            confidence: 0.45,
            reason: candidate.reason.to_string(),
        });
    }

    if fuzzy_candidates.len() > 1 {
        let candidate = &fuzzy_candidates[0];
        return Ok(MatchOutcome {
            status: ReconciliationStatus::ManualReview,
            matched_track_id: Some(candidate.track_id),
            matched_local_file_id: Some(candidate.local_file_id),
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

#[derive(Debug, Clone)]
struct CandidateMatch {
    track_id: i64,
    local_file_id: i64,
    track_number: Option<i64>,
    disc_number: Option<i64>,
    duration_ms: Option<i64>,
    reason: &'static str,
}

impl CandidateMatch {
    fn new(track: Track, file: LocalFile, reason: &'static str) -> Self {
        Self {
            track_id: track.id,
            local_file_id: file.id,
            track_number: track.track_number,
            disc_number: track.disc_number,
            duration_ms: track.duration_ms,
            reason,
        }
    }
}

fn upsert_best_candidate(
    candidates: &mut BTreeMap<i64, CandidateMatch>,
    candidate: CandidateMatch,
) {
    candidates
        .entry(candidate.track_id)
        .and_modify(|current| {
            if candidate.local_file_id < current.local_file_id {
                *current = candidate.clone();
            }
        })
        .or_insert(candidate);
}

fn dedupe_candidates<I>(candidates: I) -> Vec<CandidateMatch>
where
    I: IntoIterator<Item = CandidateMatch>,
{
    let mut by_track = BTreeMap::new();
    for candidate in candidates {
        upsert_best_candidate(&mut by_track, candidate);
    }
    by_track.into_values().collect()
}

fn resolve_exact_title_candidates(
    desired: &DesiredTrack,
    candidates: &[CandidateMatch],
) -> Option<MatchOutcome> {
    if candidates.is_empty() {
        return None;
    }

    let mut filtered = candidates.iter().collect::<Vec<_>>();
    let mut used_metadata = false;

    if let Some(track_number) = desired.track_number {
        let matching = filtered
            .iter()
            .copied()
            .filter(|candidate| candidate.track_number == Some(track_number))
            .collect::<Vec<_>>();
        if !matching.is_empty() {
            filtered = matching;
            used_metadata = true;
        }
    }

    if let Some(disc_number) = desired.disc_number {
        let matching = filtered
            .iter()
            .copied()
            .filter(|candidate| candidate.disc_number == Some(disc_number))
            .collect::<Vec<_>>();
        if !matching.is_empty() {
            filtered = matching;
            used_metadata = true;
        }
    }

    if desired.duration_ms.is_some() {
        let matching = filtered
            .iter()
            .copied()
            .filter(|candidate| duration_within_tolerance(desired.duration_ms, candidate.duration_ms, 2000))
            .collect::<Vec<_>>();
        if !matching.is_empty() {
            filtered = matching;
            used_metadata = true;
        }
    }

    if filtered.len() == 1 {
        let candidate = filtered[0];
        let (status, confidence, reason) = if used_metadata {
            (
                ReconciliationStatus::StrongMatch,
                0.88,
                "exact title match with deterministic metadata tie-break",
            )
        } else {
            (
                ReconciliationStatus::WeakMatch,
                0.5,
                "unique exact title match within artist",
            )
        };
        return Some(MatchOutcome {
            status,
            matched_track_id: Some(candidate.track_id),
            matched_local_file_id: Some(candidate.local_file_id),
            confidence,
            reason: reason.to_string(),
        });
    }

    if filtered.len() > 1 {
        let candidate = filtered[0];
        return Some(MatchOutcome {
            status: ReconciliationStatus::ManualReview,
            matched_track_id: Some(candidate.track_id),
            matched_local_file_id: Some(candidate.local_file_id),
            confidence: 0.25,
            reason: "multiple exact-title candidates remain after metadata tie-break".to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::normalize::{album::normalize_album_title, artist::normalize_artist_name};
    use crate::librarian::models::{IntegrityStatus, NewLocalFile};
    use super::fuzzy::levenshtein;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn levenshtein_distance_is_bounded() {
        assert_eq!(levenshtein("track", "track"), 0);
        assert_eq!(levenshtein("track", "tracks"), 1);
        assert_eq!(levenshtein("song", "s0ng"), 1);
    }

    async fn test_db() -> LibrarianDb {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory db");
        let db = LibrarianDb::from_pool(pool);
        db.migrate().await.expect("migrate");
        db
    }

    async fn insert_local_file(db: &LibrarianDb, track_id: i64, file_path: &str) -> i64 {
        db.upsert_local_file(&NewLocalFile {
            track_id: Some(track_id),
            file_path: file_path.to_string(),
            file_name: std::path::Path::new(file_path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("track.flac")
                .to_string(),
            extension: "flac".to_string(),
            codec: Some("flac".to_string()),
            bitrate: None,
            sample_rate: Some(44100),
            bit_depth: Some(16),
            channels: Some(2),
            duration_ms: Some(273640),
            file_size: 1024,
            file_mtime_ms: Some(1),
            content_hash: None,
            integrity_status: IntegrityStatus::Readable,
            quality_tier: None,
        })
        .await
        .expect("insert local file")
    }

    #[tokio::test]
    async fn exact_title_metadata_tie_break_avoids_manual_review() {
        let db = test_db().await;
        let artist_id = db
            .upsert_artist("Kendrick Lamar", &normalize_artist_name("Kendrick Lamar"))
            .await
            .expect("artist");
        let album_id = db
            .upsert_album(
                artist_id,
                "good kid, m.A.A.d city",
                &normalize_album_title("good kid, m.A.A.d city"),
                None,
            )
            .await
            .expect("album");

        let original_track = db
            .upsert_track(
                artist_id,
                Some(album_id),
                "Bitch, Don’t Kill My Vibe",
                "bitch don’t kill my vibe",
                Some(2),
                Some(1),
                Some(310720),
                Some("USUM71210774"),
            )
            .await
            .expect("original track");
        insert_local_file(&db, original_track, "A:\\music\\Kendrick Lamar\\02 - Bitch, Don’t Kill My Vibe.flac").await;

        let remix_track = db
            .upsert_track(
                artist_id,
                Some(album_id),
                "Bitch, Don’t Kill My Vibe (Remix)",
                "bitch don’t kill my vibe",
                Some(4),
                Some(2),
                Some(278573),
                Some("USUM71302802"),
            )
            .await
            .expect("remix track");
        insert_local_file(&db, remix_track, "A:\\music\\Kendrick Lamar\\04 - Bitch, Don’t Kill My Vibe (Remix).flac").await;

        let desired = DesiredTrack {
            id: 1,
            source_name: "spotify".to_string(),
            source_track_id: Some("712uvW1Vezq8WpQi38v2L9".to_string()),
            source_album_id: Some("3DGQ1iZ9XKUQxAUWjfC34w".to_string()),
            source_artist_id: None,
            artist_name: "Kendrick Lamar".to_string(),
            album_title: Some("good kid, m.A.A.d city (Deluxe)".to_string()),
            track_title: "Bitch, Don't Kill My Vibe".to_string(),
            track_number: Some(2),
            disc_number: Some(1),
            duration_ms: Some(310720),
            isrc: None,
            raw_payload_json: None,
            imported_at: String::new(),
        };

        let outcome = match_desired_track(&db, &desired).await.expect("match");
        assert_eq!(outcome.status, ReconciliationStatus::StrongMatch);
        assert_eq!(outcome.matched_track_id, Some(original_track));
    }

    #[tokio::test]
    async fn duplicate_local_files_for_same_track_still_resolve_cleanly() {
        let db = test_db().await;
        let artist_id = db
            .upsert_artist("Kendrick Lamar", &normalize_artist_name("Kendrick Lamar"))
            .await
            .expect("artist");
        let album_id = db
            .upsert_album(
                artist_id,
                "good kid, m.A.A.d city",
                &normalize_album_title("good kid, m.A.A.d city"),
                None,
            )
            .await
            .expect("album");
        let track_id = db
            .upsert_track(
                artist_id,
                Some(album_id),
                "Backseat Freestyle",
                &normalize_track_title("Backseat Freestyle"),
                Some(3),
                Some(1),
                Some(212653),
                Some("USUM71210777"),
            )
            .await
            .expect("track");
        let first_file = insert_local_file(
            &db,
            track_id,
            "A:\\music\\Kendrick Lamar\\2012 - good kid, m.A.A.d city\\03 - Backseat Freestyle.flac",
        )
        .await;
        insert_local_file(
            &db,
            track_id,
            "A:\\music\\Kendrick Lamar\\good kid, m.A.A.d city\\03 - Backseat Freestyle.flac",
        )
        .await;

        let desired = DesiredTrack {
            id: 1,
            source_name: "spotify".to_string(),
            source_track_id: Some("3aGibUHhQyBsyumYHylw0K".to_string()),
            source_album_id: Some("3DGQ1iZ9XKUQxAUWjfC34w".to_string()),
            source_artist_id: None,
            artist_name: "Kendrick Lamar".to_string(),
            album_title: Some("good kid, m.A.A.d city (Deluxe)".to_string()),
            track_title: "Backseat Freestyle".to_string(),
            track_number: Some(3),
            disc_number: Some(1),
            duration_ms: Some(212653),
            isrc: None,
            raw_payload_json: None,
            imported_at: String::new(),
        };

        let outcome = match_desired_track(&db, &desired).await.expect("match");
        assert_eq!(outcome.status, ReconciliationStatus::StrongMatch);
        assert_eq!(outcome.matched_track_id, Some(track_id));
        assert_eq!(outcome.matched_local_file_id, Some(first_file));
    }
}
