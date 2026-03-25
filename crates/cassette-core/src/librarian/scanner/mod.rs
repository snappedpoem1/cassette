pub mod audio;
pub mod hashing;
pub mod integrity;
pub mod walker;

use crate::librarian::config::LibrarianConfig;
use crate::librarian::db::LibrarianDb;
use crate::librarian::error::{LibrarianError, Result};
use crate::librarian::models::{IntegrityStatus, ScanStats};
use crate::librarian::scanner::audio::{parse_audio_file, to_new_local_file};
use crate::librarian::scanner::hashing::blake3_hash_file;
use crate::librarian::scanner::walker::discover_audio_files;
use sanitize_filename::sanitize;
use tracing::{debug, info, warn};

pub async fn scan_library(db: &LibrarianDb, config: &LibrarianConfig) -> Result<ScanStats> {
    let files = discover_audio_files(&config.library_roots, &config.scan_behavior);
    info!(discovered = files.len(), "librarian discovered audio files");

    let mut stats = ScanStats {
        discovered_files: files.len() as u64,
        ..Default::default()
    };

    for path in files {
        let metadata = tokio::fs::metadata(&path).await?;
        let file_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);

        let parsed = parse_audio_file(&path, &config.quality)?;
        let hash = if config.enable_content_hashing {
            match blake3_hash_file(&path).await {
                Ok(value) => Some(value),
                Err(error) => {
                    warn!(path = %path.display(), error = %error, "failed to hash file");
                    None
                }
            }
        } else {
            None
        };

        let mut local_file = to_new_local_file(&path, file_size, parsed.clone(), hash.clone());

        if let (Some(artist), Some(title), Some(norm_artist), Some(norm_title)) = (
            parsed.artist.as_deref(),
            parsed.title.as_deref(),
            parsed.normalized_artist.as_deref(),
            parsed.normalized_title.as_deref(),
        ) {
            let artist_id = db.upsert_artist(artist, norm_artist).await?;
            let album_id = if let (Some(album), Some(norm_album)) =
                (parsed.album.as_deref(), parsed.normalized_album.as_deref())
            {
                Some(db.upsert_album(artist_id, album, norm_album, None).await?)
            } else {
                None
            };

            let safe_title = sanitize(title);
            let track_id = db
                .upsert_track(
                    artist_id,
                    album_id,
                    &safe_title,
                    norm_title,
                    parsed.track_number,
                    parsed.disc_number,
                    parsed.duration_ms,
                    parsed.isrc.as_deref(),
                )
                .await?;
            local_file.track_id = Some(track_id);
        }

        let local_file_id = db
            .upsert_local_file(&local_file)
            .await
            .map_err(|error| {
                LibrarianError::DatabaseError(sqlx::Error::Protocol(format!(
                    "local_file upsert failed for {}: {error}",
                    path.display()
                )))
            })?;
        debug!(
            local_file_id,
            integrity = local_file.integrity_status.as_str(),
            hash = ?hash,
            "indexed local file"
        );

        stats.scanned_files += 1;
        if matches!(local_file.integrity_status, IntegrityStatus::Unreadable) {
            stats.unreadable_files += 1;
        }
        if matches!(local_file.integrity_status, IntegrityStatus::Suspicious) {
            stats.suspicious_files += 1;
        }

        if let Some(hash) = local_file.content_hash.as_deref() {
            let duplicates = db.find_duplicate_hashes(hash).await?;
            if duplicates.len() > 1 {
                debug!(hash, duplicates = duplicates.len(), "detected duplicate content hash");
            }
        }

        if stats.scanned_files % 500 == 0 {
            println!(
                "Librarian scan progress: {}/{} (unreadable: {}, suspicious: {})",
                stats.scanned_files,
                stats.discovered_files,
                stats.unreadable_files,
                stats.suspicious_files
            );
        }
    }

    Ok(stats)
}
