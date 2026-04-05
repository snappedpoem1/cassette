use cassette_core::director::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};
use cassette_core::metadata::MetadataService;
use cassette_core::provider_settings::DownloadConfig;
use cassette_core::sources::RemoteProviderConfig;

pub const RESOLUTION_FALLBACK_ORDER: [&str; 3] = ["musicbrainz", "itunes", "spotify"];

#[derive(Debug, Clone)]
pub struct ResolvedAlbumTrackTasks {
    pub resolver_source: String,
    pub resolver_release_id: String,
    pub source_album_id: Option<String>,
    pub tasks: Vec<TrackTask>,
}

pub fn metadata_service_from_remote_config(
    config: &RemoteProviderConfig,
) -> Result<MetadataService, String> {
    metadata_service_from_spotify_credentials(
        config.spotify_client_id.clone(),
        config.spotify_client_secret.clone(),
    )
}

pub fn metadata_service_from_download_config(
    config: &DownloadConfig,
) -> Result<MetadataService, String> {
    metadata_service_from_spotify_credentials(
        config.spotify_client_id.clone(),
        config.spotify_client_secret.clone(),
    )
}

pub fn metadata_service_from_spotify_credentials(
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
) -> Result<MetadataService, String> {
    MetadataService::with_spotify(spotify_client_id, spotify_client_secret)
        .map_err(|error| error.to_string())
}

pub fn resolution_fallback_order() -> &'static [&'static str; 3] {
    &RESOLUTION_FALLBACK_ORDER
}

pub async fn resolve_album_track_tasks_from_remote_config(
    config: &RemoteProviderConfig,
    artist: &str,
    album: &str,
    source: TrackTaskSource,
    strategy: AcquisitionStrategy,
) -> Result<ResolvedAlbumTrackTasks, String> {
    let metadata = metadata_service_from_remote_config(config)?;
    resolve_album_track_tasks(&metadata, artist, album, source, strategy).await
}

pub async fn resolve_album_track_tasks_from_download_config(
    config: &DownloadConfig,
    artist: &str,
    album: &str,
    source: TrackTaskSource,
    strategy: AcquisitionStrategy,
) -> Result<ResolvedAlbumTrackTasks, String> {
    let metadata = metadata_service_from_download_config(config)?;
    resolve_album_track_tasks(&metadata, artist, album, source, strategy).await
}

pub async fn resolve_album_track_tasks_from_spotify_credentials(
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
    artist: &str,
    album: &str,
    source: TrackTaskSource,
    strategy: AcquisitionStrategy,
) -> Result<ResolvedAlbumTrackTasks, String> {
    let metadata =
        metadata_service_from_spotify_credentials(spotify_client_id, spotify_client_secret)?;
    resolve_album_track_tasks(&metadata, artist, album, source, strategy).await
}

pub async fn resolve_album_track_tasks(
    metadata: &MetadataService,
    artist: &str,
    album: &str,
    source: TrackTaskSource,
    strategy: AcquisitionStrategy,
) -> Result<ResolvedAlbumTrackTasks, String> {
    let release_with_tracks = metadata
        .resolve_release_with_tracks(artist, album)
        .await
        .map_err(|error| error.to_string())?;

    if release_with_tracks.tracks.is_empty() {
        return Err(format!("No tracks found for {artist} - {album}"));
    }

    let release_title = release_with_tracks.release.title.clone();
    let release_year = release_with_tracks.release.year;
    let (resolver_source, source_album_id, musicbrainz_release_id) =
        classify_release_identity(&release_with_tracks.release.id);

    let mut tasks = release_with_tracks
        .tracks
        .into_iter()
        .map(|track| TrackTask {
            task_id: album_track_task_key(
                artist,
                album,
                track.disc_number,
                track.track_number,
                &track.title,
            ),
            source: source.clone(),
            desired_track_id: None,
            source_operation_id: None,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_album_id: source_album_id.clone(),
                source_artist_id: None,
                source_playlist: None,
                artist: if track.artist.trim().is_empty() {
                    artist.to_string()
                } else {
                    track.artist
                },
                album_artist: Some(artist.to_string()),
                title: track.title,
                album: Some(release_title.clone()),
                track_number: Some(track.track_number),
                disc_number: Some(track.disc_number),
                year: release_year,
                duration_secs: if track.duration_ms > 0 {
                    Some(track.duration_ms as f64 / 1000.0)
                } else {
                    None
                },
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_group_id: None,
                musicbrainz_release_id: musicbrainz_release_id.clone(),
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy,
        })
        .collect::<Vec<_>>();

    tasks.sort_by_key(|task| {
        (
            task.target.disc_number.unwrap_or(0),
            task.target.track_number.unwrap_or(0),
            task.target.title.to_ascii_lowercase(),
        )
    });

    Ok(ResolvedAlbumTrackTasks {
        resolver_source: resolver_source.to_string(),
        resolver_release_id: release_with_tracks.release.id,
        source_album_id,
        tasks,
    })
}

pub fn album_track_task_key(
    artist: &str,
    album: &str,
    disc_number: u32,
    track_number: u32,
    title: &str,
) -> String {
    format!(
        "spotify-album-track::{}::{}::{}::{:02}::{}",
        artist.to_ascii_lowercase(),
        album.to_ascii_lowercase(),
        disc_number,
        track_number,
        normalize_task_component(title),
    )
}

pub fn normalize_task_component(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn classify_release_identity(
    release_id: &str,
) -> (&'static str, Option<String>, Option<String>) {
    if release_id.starts_with("spotify:") {
        return ("spotify", Some(release_id.to_string()), None);
    }
    if release_id.starts_with("itunes:") {
        return ("itunes", Some(release_id.to_string()), None);
    }
    ("musicbrainz", None, Some(release_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{classify_release_identity, resolution_fallback_order};

    #[test]
    fn classifies_release_identity_sources() {
        let spotify = classify_release_identity("spotify:album123");
        assert_eq!(spotify.0, "spotify");
        assert_eq!(spotify.1.as_deref(), Some("spotify:album123"));
        assert!(spotify.2.is_none());

        let itunes = classify_release_identity("itunes:456");
        assert_eq!(itunes.0, "itunes");
        assert_eq!(itunes.1.as_deref(), Some("itunes:456"));
        assert!(itunes.2.is_none());

        let musicbrainz = classify_release_identity("mb-release-1");
        assert_eq!(musicbrainz.0, "musicbrainz");
        assert!(musicbrainz.1.is_none());
        assert_eq!(musicbrainz.2.as_deref(), Some("mb-release-1"));
    }

    #[test]
    fn fallback_order_is_musicbrainz_then_itunes_then_spotify() {
        assert_eq!(
            resolution_fallback_order().as_slice(),
            ["musicbrainz", "itunes", "spotify"]
        );
    }
}
