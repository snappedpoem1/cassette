use cassette_lib::state::AppState;
use cassette_core::director::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};
use cassette_core::models::{DownloadJob, DownloadStatus};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|error| error.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn parse_args() -> Result<(String, String, Option<String>), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err(
            "Usage: acquire_cli <artist> <title> [album]\nExample: acquire_cli \"Brand New\" \"Sic Transit Gloria... Glory Fades\" \"Deja Entendu\""
                .to_string(),
        );
    }
    let artist = args[0].trim().to_string();
    let title = args[1].trim().to_string();
    let album = args
        .get(2)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if artist.is_empty() || title.is_empty() {
        return Err("Artist and title must be non-empty.".to_string());
    }
    Ok((artist, title, album))
}

fn status_label(status: &DownloadStatus) -> &'static str {
    match status {
        DownloadStatus::Queued => "Queued",
        DownloadStatus::Searching => "Searching",
        DownloadStatus::Downloading => "Downloading",
        DownloadStatus::Verifying => "Verifying",
        DownloadStatus::Done => "Done",
        DownloadStatus::Cancelled => "Cancelled",
        DownloadStatus::Failed => "Failed",
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let (artist, title, album) = parse_args()?;
    let db_path = app_db_path()?;
    let app_state = AppState::new(&db_path, None).map_err(|error| error.to_string())?;

    let id = format!("job-{}", Uuid::new_v4());
    let query = match album.as_deref() {
        Some(album_name) => format!("{artist} {title} {album_name}"),
        None => format!("{artist} {title}"),
    };

    {
        let mut jobs = app_state
            .download_jobs
            .lock()
            .map_err(|error| error.to_string())?;
        jobs.insert(
            id.clone(),
            DownloadJob {
                id: id.clone(),
                query,
                artist: artist.clone(),
                title: title.clone(),
                album: album.clone(),
                status: DownloadStatus::Queued,
                provider: None,
                progress: 0.0,
                error: None,
            },
        );
    }

    app_state
        .director_submitter
        .submit(TrackTask {
            task_id: id.clone(),
            source: TrackTaskSource::Manual,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_playlist: None,
                artist: artist.clone(),
                album_artist: Some(artist.clone()),
                title: title.clone(),
                album: album.clone(),
                track_number: None,
                disc_number: None,
                year: None,
                duration_secs: None,
                isrc: None,
            },
            strategy: AcquisitionStrategy::Standard,
        })
        .await
        .map_err(|error| error.to_string())?;

    println!("Queued job {id} for {artist} - {title}");

    let mut elapsed = 0u64;
    let mut last_report = String::new();
    let timeout_secs = 180u64;
    while elapsed <= timeout_secs {
        let snapshot = {
            let jobs = app_state
                .download_jobs
                .lock()
                .map_err(|error| error.to_string())?;
            jobs.get(&id).cloned()
        };

        let Some(job) = snapshot else {
            return Err("Job disappeared from in-memory queue.".to_string());
        };

        let provider = job.provider.clone().unwrap_or_else(|| "-".to_string());
        let error_note = job.error.clone().unwrap_or_default();
        let report_line = format!(
            "t={elapsed:>3}s status={} provider={} progress={:.0}% {}",
            status_label(&job.status),
            provider,
            job.progress * 100.0,
            error_note
        );
        if report_line != last_report {
            println!("{report_line}");
            last_report = report_line;
        }

        if matches!(
            job.status,
            DownloadStatus::Done | DownloadStatus::Cancelled | DownloadStatus::Failed
        ) {
            break;
        }

        sleep(Duration::from_secs(5)).await;
        elapsed += 5;
    }

    let final_job = {
        let jobs = app_state
            .download_jobs
            .lock()
            .map_err(|error| error.to_string())?;
        jobs.get(&id).cloned()
    };
    if let Some(job) = final_job {
        println!(
            "Final: status={} provider={} progress={:.0}% error={}",
            status_label(&job.status),
            job.provider.unwrap_or_else(|| "-".to_string()),
            job.progress * 100.0,
            job.error.unwrap_or_default()
        );
    }

    Ok(())
}
