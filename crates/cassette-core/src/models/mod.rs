use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanProgress {
    pub scanned: u64,
    pub total: u64,
    pub current_file: String,
    pub done: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub year: Option<i32>,
    pub duration_secs: f64,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub format: String,
    pub file_size: u64,
    pub cover_art_path: Option<String>,
    pub isrc: Option<String>,
    pub musicbrainz_recording_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub quality_tier: Option<String>,
    pub content_hash: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub cover_art_path: Option<String>,
    pub track_count: usize,
    pub dominant_color_hex: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub album_count: usize,
    pub track_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryRoot {
    pub id: i64,
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i64,
    pub track_id: i64,
    pub position: i64,
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackState {
    pub current_track: Option<Track>,
    pub queue_position: usize,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub is_playing: bool,
    pub volume: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NowPlayingContext {
    pub artist_name: String,
    pub album_title: Option<String>,
    pub artist_summary: Option<String>,
    pub artist_tags: Vec<String>,
    pub listeners: Option<u64>,
    pub album_art_url: Option<String>,
    pub album_summary: Option<String>,
    pub lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    pub lyrics_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Queued,
    Searching,
    Downloading,
    Verifying,
    Done,
    Cancelled,
    Failed,
}

impl Default for DownloadStatus {
    fn default() -> Self {
        Self::Queued
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: String,
    pub query: String,
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub status: DownloadStatus,
    pub provider: Option<String>,
    pub progress: f32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadArtistResult {
    pub id: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub disambiguation: Option<String>,
    pub origin: Option<String>,
    pub tags: Vec<String>,
    pub summary: Option<String>,
    pub listeners: Option<u64>,
    pub image_url: Option<String>,
    pub source: String,
    pub mbid: Option<String>,
    pub artist_mbid: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadAlbumResult {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_mbid: Option<String>,
    pub year: Option<i32>,
    pub release_type: Option<String>,
    pub track_count: Option<u32>,
    pub cover_url: Option<String>,
    pub source: String,
    pub mbid: Option<String>,
    pub discogs_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadMetadataSearchResult {
    pub artists: Vec<DownloadArtistResult>,
    pub albums: Vec<DownloadAlbumResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadArtistDiscography {
    pub artist: DownloadArtistResult,
    pub albums: Vec<DownloadAlbumResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AcquisitionQueueReport {
    pub scope: String,
    pub requested: usize,
    pub queued: usize,
    pub skipped: usize,
    pub job_ids: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub track_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: i64,
    pub playlist_id: i64,
    pub track_id: i64,
    pub position: i64,
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpotifyAlbumHistory {
    pub artist: String,
    pub album: String,
    pub total_ms: u64,
    pub play_count: u32,
    pub skip_count: u32,
    pub in_library: bool,
    pub imported_at: String,
}
