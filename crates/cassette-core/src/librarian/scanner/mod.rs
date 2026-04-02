pub mod audio;
pub mod hashing;
pub mod integrity;
pub mod walker;

use crate::librarian::config::{LibrarianConfig, ScanMode};
use crate::librarian::db::LibrarianDb;
use crate::librarian::error::{LibrarianError, Result};
use crate::librarian::models::{IntegrityStatus, ScanStats};
use crate::gatekeeper::validation::fingerprint::compute_fingerprint;
use crate::librarian::scanner::audio::{parse_audio_file, to_new_local_file};
use crate::librarian::scanner::hashing::blake3_hash_file;
use crate::librarian::scanner::walker::discover_audio_files;
use futures_util::{stream, StreamExt};
use sanitize_filename::sanitize;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

const CHECKPOINT_FLUSH_INTERVAL: i64 = 100;

pub async fn scan_library(db: &LibrarianDb, config: &LibrarianConfig, run_id: &str) -> Result<ScanStats> {
    let mut stats = ScanStats::default();
    let mut roots = config.library_roots.clone();
    roots.sort();

    for root in roots {
        let root_stats = scan_root(db, config, run_id, &root).await?;
        stats.discovered_files += root_stats.discovered_files;
        stats.scanned_files += root_stats.scanned_files;
        stats.skipped_files += root_stats.skipped_files;
        stats.unreadable_files += root_stats.unreadable_files;
        stats.suspicious_files += root_stats.suspicious_files;
    }

    Ok(stats)
}

pub async fn backfill_missing_fingerprints(
    db: &LibrarianDb,
    limit: usize,
    concurrency: usize,
) -> Result<usize> {
    if limit == 0 || concurrency == 0 {
        return Ok(0);
    }

    let candidates = db.list_local_files_missing_fingerprint(limit).await?;
    let db = db.clone();

    let outcomes = stream::iter(candidates.into_iter().map(|local_file| {
        let db = db.clone();
        async move {
            let path = PathBuf::from(&local_file.file_path);
            if !path.exists() {
                let error = "file_missing".to_string();
                db.mark_local_file_fingerprint_failure(
                    local_file.id,
                    local_file.file_mtime_ms,
                    &error,
                )
                .await?;
                debug!(
                    path = %path.display(),
                    local_file_id = local_file.id,
                    "suppressed fingerprint backfill for missing file"
                );
                return Ok::<usize, LibrarianError>(0);
            }

            match compute_fingerprint(&path).await {
                Ok(fingerprint) => {
                    db.set_local_file_fingerprint(
                        local_file.id,
                        &fingerprint,
                        local_file.file_mtime_ms,
                    )
                    .await?;
                    Ok(1)
                }
                Err(error) => {
                    let error_message = error.to_string();
                    db.mark_local_file_fingerprint_failure(
                        local_file.id,
                        local_file.file_mtime_ms,
                        &error_message,
                    )
                    .await?;
                    warn!(
                        path = %path.display(),
                        local_file_id = local_file.id,
                        error = %error_message,
                        "failed to compute acoustid fingerprint during backfill"
                    );
                    Ok(0)
                }
            }
        }
    }))
    .buffer_unordered(concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut backfilled = 0usize;
    for outcome in outcomes {
        backfilled += outcome?;
    }

    Ok(backfilled)
}

async fn scan_root(
    db: &LibrarianDb,
    config: &LibrarianConfig,
    run_id: &str,
    root: &Path,
) -> Result<ScanStats> {
    let root_path = root.to_string_lossy().to_string();
    let mut files = discover_audio_files(&[PathBuf::from(root)], &config.scan_behavior);
    files.sort();

    let checkpoint = db.get_scan_checkpoint(&root_path).await?;
    let resume_after = match config.scan_mode {
        ScanMode::Resume => checkpoint
            .as_ref()
            .and_then(|value| value.last_scanned_path.clone())
            .filter(|_| checkpoint.as_ref().is_some_and(|value| value.status == "in_progress")),
        _ => None,
    };
    let starting_files_seen = match config.scan_mode {
        ScanMode::Resume => checkpoint
            .as_ref()
            .filter(|value| value.status == "in_progress")
            .map(|value| value.files_seen)
            .unwrap_or(0),
        _ => 0,
    };
    let starting_files_indexed = match config.scan_mode {
        ScanMode::Resume => checkpoint
            .as_ref()
            .filter(|value| value.status == "in_progress")
            .map(|value| value.files_indexed)
            .unwrap_or(0),
        _ => 0,
    };

    let files = if let Some(last_scanned_path) = resume_after.as_deref() {
        files
            .into_iter()
            .filter(|path| path.to_string_lossy().as_ref() > last_scanned_path)
            .collect::<Vec<_>>()
    } else {
        files
    };

    info!(
        root = %root.display(),
        mode = config.scan_mode.as_str(),
        discovered = files.len(),
        "librarian discovered audio files for root"
    );

    db.upsert_scan_checkpoint(
        &root_path,
        run_id,
        resume_after.as_deref(),
        "in_progress",
        starting_files_seen,
        starting_files_indexed,
    )
    .await?;

    let mut stats = ScanStats {
        discovered_files: files.len() as u64,
        ..Default::default()
    };
    let mut files_seen = starting_files_seen;
    let mut files_indexed = starting_files_indexed;
    let mut last_scanned_path = resume_after;

    for path in files {
        let metadata = tokio::fs::metadata(&path).await?;
        let file_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let file_mtime_ms = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|value| i64::try_from(value.as_millis()).ok());
        let path_string = path.to_string_lossy().to_string();

        if matches!(config.scan_mode, ScanMode::Resume | ScanMode::DeltaOnly) {
            if let Some(existing) = db.get_local_file_scan_state(&path_string).await? {
                if existing.file_size == file_size && existing.file_mtime_ms == file_mtime_ms {
                    stats.skipped_files += 1;
                    files_seen += 1;
                    last_scanned_path = Some(path_string.clone());
                    maybe_flush_checkpoint(
                        db,
                        &root_path,
                        run_id,
                        last_scanned_path.as_deref(),
                        files_seen,
                        files_indexed,
                    )
                    .await?;
                    continue;
                }
            }
        }

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

        let mut local_file = to_new_local_file(&path, file_size, file_mtime_ms, parsed.clone(), hash.clone());

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

        let local_file_id = db.upsert_local_file(&local_file).await.map_err(|error| {
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
        files_seen += 1;
        files_indexed += 1;
        last_scanned_path = Some(path_string.clone());

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

        maybe_flush_checkpoint(
            db,
            &root_path,
            run_id,
            last_scanned_path.as_deref(),
            files_seen,
            files_indexed,
        )
        .await?;

        if stats.scanned_files % 500 == 0 {
            println!(
                "Librarian scan progress [{} {}]: indexed={} skipped={} unreadable={} suspicious={}",
                config.scan_mode.as_str(),
                root.display(),
                stats.scanned_files,
                stats.skipped_files,
                stats.unreadable_files,
                stats.suspicious_files
            );
        }
    }

    db.upsert_scan_checkpoint(
        &root_path,
        run_id,
        last_scanned_path.as_deref(),
        "completed",
        files_seen,
        files_indexed,
    )
    .await?;

    Ok(stats)
}

async fn maybe_flush_checkpoint(
    db: &LibrarianDb,
    root_path: &str,
    run_id: &str,
    last_scanned_path: Option<&str>,
    files_seen: i64,
    files_indexed: i64,
) -> Result<()> {
    if files_seen == 0 || files_seen % CHECKPOINT_FLUSH_INTERVAL != 0 {
        return Ok(());
    }

    db.upsert_scan_checkpoint(
        root_path,
        run_id,
        last_scanned_path,
        "in_progress",
        files_seen,
        files_indexed,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{backfill_missing_fingerprints, scan_library};
    use crate::librarian::{LibrarianConfig, LibrarianDb, ScanMode};
    use crate::librarian::models::{IntegrityStatus, NewLocalFile};
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_db() -> LibrarianDb {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory pool");
        let db = LibrarianDb::from_pool(pool);
        db.migrate().await.expect("migrate");
        db
    }

    fn write_audio_stub(path: &std::path::Path) {
        std::fs::write(path, b"not-audio").expect("write audio stub");
    }

    fn write_wav_stub(path: &std::path::Path) {
        let sample_rate = 44_100_u32;
        let channels = 1_u16;
        let bits_per_sample = 16_u16;
        let duration_samples = sample_rate;
        let data_len = duration_samples * u32::from(channels) * u32::from(bits_per_sample / 8);
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8);
        let block_align = channels * (bits_per_sample / 8);
        let riff_len = 36 + data_len;

        let mut bytes = Vec::<u8>::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_len.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(bytes.len() + data_len as usize, 0_u8);
        std::fs::write(path, bytes).expect("write wav stub");
    }

    #[tokio::test]
    async fn delta_only_skips_unchanged_files() {
        let db = setup_db().await;
        let root = tempdir().expect("root");
        write_audio_stub(&root.path().join("01 - One.mp3"));
        write_audio_stub(&root.path().join("02 - Two.mp3"));

        let mut config = LibrarianConfig::default();
        config.enable_content_hashing = false;
        config.library_roots = vec![root.path().to_path_buf()];
        config.scan_mode = ScanMode::Full;

        let first = scan_library(&db, &config, "run-1").await.expect("first scan");
        assert_eq!(first.scanned_files, 2);
        assert_eq!(first.skipped_files, 0);

        config.scan_mode = ScanMode::DeltaOnly;
        let second = scan_library(&db, &config, "run-2").await.expect("second scan");
        assert_eq!(second.scanned_files, 0);
        assert_eq!(second.skipped_files, 2);
    }

    #[tokio::test]
    async fn resume_continues_after_checkpoint_path() {
        let db = setup_db().await;
        let root = tempdir().expect("root");
        let first = root.path().join("01 - One.mp3");
        let second = root.path().join("02 - Two.mp3");
        let third = root.path().join("03 - Three.mp3");
        write_audio_stub(&first);
        write_audio_stub(&second);
        write_audio_stub(&third);

        let mut config = LibrarianConfig::default();
        config.enable_content_hashing = false;
        config.library_roots = vec![root.path().to_path_buf()];
        config.scan_mode = ScanMode::Full;
        scan_library(&db, &config, "seed").await.expect("seed scan");

        let root_string = root.path().to_string_lossy().to_string();
        let second_string = second.to_string_lossy().to_string();
        let third_string = third.to_string_lossy().to_string();
        db.upsert_scan_checkpoint(
            &root_string,
            "resume-seed",
            Some(&second_string),
            "in_progress",
            2,
            2,
        )
        .await
        .expect("checkpoint");

        std::fs::write(&third, b"changed-audio").expect("touch file");
        config.scan_mode = ScanMode::Resume;
        let resumed = scan_library(&db, &config, "resume-run")
            .await
            .expect("resume scan");
        assert_eq!(resumed.scanned_files, 1);

        let checkpoint = db
            .get_scan_checkpoint(&root_string)
            .await
            .expect("checkpoint lookup")
            .expect("checkpoint row");
        assert_eq!(checkpoint.status, "completed");
        assert_eq!(checkpoint.last_scanned_path.as_deref(), Some(third_string.as_str()));
    }

    #[tokio::test]
    async fn fingerprint_backfill_persists_missing_fingerprint() {
        let db = setup_db().await;
        let root = tempdir().expect("root");
        let path = root.path().join("01 - One.wav");
        write_wav_stub(&path);

        db.upsert_local_file(&NewLocalFile {
            track_id: None,
            file_path: path.to_string_lossy().to_string(),
            file_name: "01 - One.wav".to_string(),
            extension: "wav".to_string(),
            codec: Some("wav".to_string()),
            bitrate: Some(1411),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            channels: Some(1),
            duration_ms: Some(1_000),
            file_size: std::fs::metadata(&path).expect("metadata").len() as i64,
            file_mtime_ms: None,
            content_hash: None,
            acoustid_fingerprint: None,
            integrity_status: IntegrityStatus::Readable,
            quality_tier: None,
        })
        .await
        .expect("insert local file");

        let backfilled = backfill_missing_fingerprints(&db, 10, 2)
            .await
            .expect("backfill fingerprints");
        assert_eq!(backfilled, 1);

        let fingerprint: Option<String> = sqlx::query_scalar(
            "SELECT acoustid_fingerprint FROM local_files WHERE file_path = ?1",
        )
        .bind(path.to_string_lossy().to_string())
        .fetch_one(db.pool())
        .await
        .expect("fingerprint lookup");
        assert!(fingerprint.is_some());
    }

    #[tokio::test]
    async fn fingerprint_backfill_marks_failure_and_suppresses_repeat_for_unchanged_file() {
        let db = setup_db().await;
        let missing_path = "C:/definitely-missing/ghost.wav".to_string();

        db.upsert_local_file(&NewLocalFile {
            track_id: None,
            file_path: missing_path.clone(),
            file_name: "ghost.wav".to_string(),
            extension: "wav".to_string(),
            codec: Some("wav".to_string()),
            bitrate: Some(1411),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            channels: Some(2),
            duration_ms: Some(1_000),
            file_size: 1_024,
            file_mtime_ms: Some(1234),
            content_hash: None,
            acoustid_fingerprint: None,
            integrity_status: IntegrityStatus::Readable,
            quality_tier: None,
        })
        .await
        .expect("insert local file");

        let first = backfill_missing_fingerprints(&db, 10, 2)
            .await
            .expect("first backfill");
        let second = backfill_missing_fingerprints(&db, 10, 2)
            .await
            .expect("second backfill");

        assert_eq!(first, 0);
        assert_eq!(second, 0);

        let remaining = db
            .list_local_files_missing_fingerprint(10)
            .await
            .expect("remaining rows");
        assert!(remaining.is_empty());

        let fingerprint_error: Option<String> = sqlx::query_scalar(
            "SELECT fingerprint_error FROM local_files WHERE file_path = ?1",
        )
        .bind(missing_path)
        .fetch_one(db.pool())
        .await
        .expect("fingerprint error lookup");
        assert_eq!(fingerprint_error.as_deref(), Some("file_missing"));
    }

    #[tokio::test]
    async fn local_file_mtime_change_clears_stale_fingerprint_state() {
        let db = setup_db().await;
        let root = tempdir().expect("root");
        let path = root.path().join("01 - Changed.wav");
        write_wav_stub(&path);
        let path_string = path.to_string_lossy().to_string();

        db.upsert_local_file(&NewLocalFile {
            track_id: None,
            file_path: path_string.clone(),
            file_name: "01 - Changed.wav".to_string(),
            extension: "wav".to_string(),
            codec: Some("wav".to_string()),
            bitrate: Some(1411),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            channels: Some(1),
            duration_ms: Some(1_000),
            file_size: std::fs::metadata(&path).expect("metadata").len() as i64,
            file_mtime_ms: Some(100),
            content_hash: None,
            acoustid_fingerprint: Some("stale-fingerprint".to_string()),
            integrity_status: IntegrityStatus::Readable,
            quality_tier: None,
        })
        .await
        .expect("insert local file");

        sqlx::query(
            "UPDATE local_files
             SET fingerprint_attempted_at = CURRENT_TIMESTAMP,
                 fingerprint_error = 'old-error',
                 fingerprint_source_mtime_ms = 100
             WHERE file_path = ?1",
        )
        .bind(&path_string)
        .execute(db.pool())
        .await
        .expect("seed stale fingerprint state");

        db.upsert_local_file(&NewLocalFile {
            track_id: None,
            file_path: path_string.clone(),
            file_name: "01 - Changed.wav".to_string(),
            extension: "wav".to_string(),
            codec: Some("wav".to_string()),
            bitrate: Some(1411),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            channels: Some(1),
            duration_ms: Some(1_000),
            file_size: std::fs::metadata(&path).expect("metadata").len() as i64,
            file_mtime_ms: Some(200),
            content_hash: None,
            acoustid_fingerprint: None,
            integrity_status: IntegrityStatus::Readable,
            quality_tier: None,
        })
        .await
        .expect("update local file");

        let row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<i64>)>(
            "SELECT acoustid_fingerprint, fingerprint_error, fingerprint_source_mtime_ms
             FROM local_files
             WHERE file_path = ?1",
        )
        .bind(path_string)
        .fetch_one(db.pool())
        .await
        .expect("state lookup");

        assert_eq!(row.0, None);
        assert_eq!(row.1, None);
        assert_eq!(row.2, None);
    }
}
