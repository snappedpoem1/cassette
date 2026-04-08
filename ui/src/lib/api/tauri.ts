import { invoke } from '@tauri-apps/api/core';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface Track {
  id: number;
  path: string;
  title: string;
  artist: string;
  album: string;
  album_artist: string;
  track_number: number | null;
  disc_number: number | null;
  year: number | null;
  duration_secs: number;
  sample_rate: number | null;
  bit_depth: number | null;
  bitrate_kbps: number | null;
  format: string;
  file_size: number;
  cover_art_path: string | null;
  isrc: string | null;
  musicbrainz_recording_id: string | null;
  musicbrainz_release_id: string | null;
  canonical_artist_id: number | null;
  canonical_release_id: number | null;
  quality_tier: string | null;
  content_hash: string | null;
  added_at: string;
}

export interface TrackIdentityContext {
  musicbrainz_release_group_id: string | null;
  canonical_release_type: string | null;
  edition_bucket: string | null;
  edition_markers: string[];
}

export interface Album {
  id: number;
  title: string;
  artist: string;
  year: number | null;
  cover_art_path: string | null;
  track_count: number;
  dominant_color_hex: string | null;
}

export interface Artist {
  id: number;
  name: string;
  album_count: number;
  track_count: number;
}

export interface LibraryRoot {
  id: number;
  path: string;
  enabled: boolean;
}

export interface QueueItem {
  id: number;
  track_id: number;
  position: number;
  track: Track | null;
}

export interface PlaybackState {
  current_track: Track | null;
  queue_position: number;
  position_secs: number;
  duration_secs: number;
  is_playing: boolean;
  volume: number;
}

export interface NowPlayingContext {
  artist_name: string;
  album_title: string | null;
  artist_summary: string | null;
  artist_tags: string[];
  listeners: number | null;
  album_art_url: string | null;
  album_summary: string | null;
  lyrics: string | null;
  synced_lyrics: string | null;
  lyrics_source: string | null;
}

export interface ScanProgress {
  scanned: number;
  total: number;
  current_file: string;
  done: boolean;
}

export interface Playlist {
  id: number;
  name: string;
  description: string | null;
  track_count: number;
  created_at: string;
}

export interface PlaylistItem {
  id: number;
  playlist_id: number;
  track_id: number;
  position: number;
  track: Track | null;
}

export interface DownloadJob {
  id: string;
  query: string;
  artist: string;
  title: string;
  album: string | null;
  status: 'Queued' | 'Searching' | 'Downloading' | 'Verifying' | 'Done' | 'Cancelled' | 'Failed';
  provider: string | null;
  progress: number;
  error: string | null;
}

export interface DirectorEvent {
  task_id: string;
  progress: string;
  provider_id: string | null;
  message: string;
}

export interface DirectorTaskResult {
  task_id: string;
  disposition: 'Finalized' | 'AlreadyPresent' | 'MetadataOnly' | 'Cancelled' | 'Failed';
  error: string | null;
  candidate_records?: unknown[];
  provider_searches?: unknown[];
}

export interface ProviderHealthEvent {
  provider_id: string;
  status: 'Unknown' | 'Healthy' | 'Down';
  checked_at: string;
  message: string | null;
}

export interface DownloadConfig {
  library_base: string;
  staging_folder: string;
  // Soulseek
  slskd_url: string | null;
  slskd_user: string | null;
  slskd_pass: string | null;
  slskd_downloads_dir: string | null;
  // Real-Debrid / torrents
  real_debrid_key: string | null;
  jackett_url: string | null;
  jackett_api_key: string | null;
  // Usenet
  nzbgeek_api_key: string | null;
  sabnzbd_url: string | null;
  sabnzbd_api_key: string | null;
  // Streaming
  qobuz_email: string | null;
  qobuz_password: string | null;
  deezer_arl: string | null;
  // Spotify
  spotify_client_id: string | null;
  spotify_client_secret: string | null;
  spotify_access_token: string | null;
  // Enrichment
  genius_token: string | null;
  discogs_token: string | null;
  lastfm_api_key: string | null;
  lastfm_username: string | null;
  // Tools
  ytdlp_path: string | null;
  sevenzip_path: string | null;
}

export type PolicyProfile = 'playback_first' | 'balanced_auto' | 'aggressive_overnight';

export interface ProviderStatus {
  id: string;
  label: string;
  configured: boolean;
  summary: string;
  missing_fields: string[];
}

export interface SlskdRuntimeStatus {
  running: boolean;
  ready: boolean;
  spawned_by_app: boolean;
  binary_found: boolean;
  binary_path: string | null;
  app_dir: string | null;
  downloads_dir: string | null;
  url: string;
  message: string | null;
}

export interface DownloadArtistResult {
  id: string;
  name: string;
  sort_name: string | null;
  disambiguation: string | null;
  origin: string | null;
  tags: string[];
  summary: string | null;
  listeners: number | null;
  image_url: string | null;
  source: string;
  mbid: string | null;
  artist_mbid: string | null;
}

export interface DownloadAlbumResult {
  id: string;
  title: string;
  artist: string;
  artist_mbid: string | null;
  year: number | null;
  release_type: string | null;
  track_count: number | null;
  cover_url: string | null;
  source: string;
  mbid: string | null;
  discogs_id: number | null;
}

export interface DownloadMetadataSearchResult {
  artists: DownloadArtistResult[];
  albums: DownloadAlbumResult[];
}

export interface DownloadArtistDiscography {
  artist: DownloadArtistResult;
  albums: DownloadAlbumResult[];
}

export interface AcquisitionQueueReport {
  scope: string;
  requested: number;
  queued: number;
  skipped: number;
  job_ids: string[];
  notes: string[];
}

export interface SpotifyAlbumSummary {
  artist: string;
  album: string;
  total_ms: number;
  play_count: number;
  skip_count: number;
  in_library: boolean;
}

export interface SpotifyImportResult {
  albums: SpotifyAlbumSummary[];
  total_streams: number;
  unique_albums: number;
  already_in_library: number;
}

export interface SpotifyImportStatus {
  album_rows: number;
  last_imported_at: string | null;
}

export interface SpotifyAlbumHistory {
  artist: string;
  album: string;
  total_ms: number;
  play_count: number;
  skip_count: number;
  in_library: boolean;
  imported_at: string;
}

export interface FileMove {
  old_path: string;
  new_path: string;
  track_id: number;
}

export interface OrganizeReport {
  moved: FileMove[];
  skipped: number;
  errors: string[];
}

export interface DuplicateTrack {
  id: number;
  path: string;
  format: string;
  bit_depth: number | null;
  sample_rate: number | null;
  bitrate_kbps: number | null;
  file_size: number;
  is_best: boolean;
}

export interface DuplicateGroup {
  key: string;
  tracks: DuplicateTrack[];
  recommendation: string;
}

export interface TagFix {
  path: string;
  field: string;
  old_value: string;
  new_value: string;
  applied: boolean;
}

export interface CandidateReviewItem {
  task_id: string;
  provider_id: string;
  provider_display_name: string;
  provider_trust_rank: number;
  provider_candidate_id: string;
  outcome: string;
  rejection_reason: string | null;
  is_selected: boolean;
  score_total: number | null;
  candidate_json: string;
  validation_json: string | null;
  score_reason_json: string | null;
}

export interface TaskResultSummary {
  task_id: string;
  disposition: string;
  provider: string;
  error: string | null;
}

export interface BacklogRunStatus {
  running: boolean;
  albums_queued: number;
  albums_skipped: number;
  tracks_submitted: number;
  current_album: string | null;
  errors: string[];
  started_at: string | null;
  finished_at: string | null;
}

export interface ProviderStat {
  provider: string;
  success: number;
  failed: number;
}

export interface DirectorDebugStats {
  pending_count: number;
  provider_stats: ProviderStat[];
  recent_results: TaskResultSummary[];
}

export interface AcquisitionRequest {
  id: number;
  scope: string;
  source: 'SpotifyLibrary' | 'SpotifyHistory' | 'SpotifyPlaylist' | 'Manual';
  source_name: string;
  source_track_id?: string | null;
  source_album_id?: string | null;
  source_artist_id?: string | null;
  artist: string;
  album?: string | null;
  title: string;
  track_number?: number | null;
  disc_number?: number | null;
  year?: number | null;
  duration_secs?: number | null;
  isrc?: string | null;
  musicbrainz_recording_id?: string | null;
  musicbrainz_release_group_id?: string | null;
  musicbrainz_release_id?: string | null;
  canonical_artist_id?: number | null;
  canonical_release_id?: number | null;
  strategy: string;
  quality_policy?: string | null;
  excluded_providers: string[];
  edition_policy?: string | null;
  confirmation_policy: string;
  desired_track_id?: number | null;
  source_operation_id?: string | null;
  task_id?: string | null;
  request_signature?: string | null;
  status: string;
  raw_payload_json?: string | null;
}

export interface AcquisitionRequestListItem {
  id: number;
  scope: string;
  artist: string;
  album: string | null;
  title: string;
  status: string;
  strategy: string;
  musicbrainz_release_group_id?: string | null;
  edition_policy?: string | null;
  task_id: string | null;
  request_signature: string;
  selected_provider: string | null;
  failure_class: string | null;
  final_path: string | null;
  execution_disposition: string | null;
  trust_stage: string;
  trust_reason_code: string;
  trust_detail: string;
  updated_at: string;
  created_at: string;
}

export interface TrustLedgerSummary {
  stage: string;
  reason_code: string;
  headline: string;
  detail: string;
  evidence_count: number;
}

export interface TrustReasonDistributionEntry {
  reason_code: string;
  label: string;
  count: number;
  stage: string;
}

export interface TrustLedgerOperationEvent {
  operation_id: string;
  module: string;
  phase: string;
  event_type: string;
  timestamp: string | null;
  event_data: string | null;
}

export interface TrustLedgerGatekeeperAudit {
  operation_id: string;
  timestamp: string;
  file_path: string;
  decision: string;
  desired_track_id: number | null;
  matched_local_file_id: number | null;
  duration_ms: number;
  notes: string;
}

export interface AcquisitionRequestEvent {
  id: number;
  request_id: number;
  task_id: string | null;
  event_type: string;
  status: string;
  message: string | null;
  payload_json: string | null;
  created_at: string;
}

export interface PlannedAcquisitionResult {
  request: AcquisitionRequestListItem & { status: string };
  identity_lane: PlannerIdentityLane;
  provider_order: string[];
  cached_provider_ids: string[];
  summary: unknown;
  provider_searches: unknown[];
  candidate_review: CandidateReviewItem[];
}

export interface PlannerIdentityLane {
  scope: string;
  musicbrainz_release_group_id?: string | null;
  musicbrainz_release_id?: string | null;
  musicbrainz_recording_id?: string | null;
  canonical_artist_id?: number | null;
  canonical_release_id?: number | null;
  quality_policy?: string | null;
  edition_policy?: string | null;
  confirmation_policy: string;
}

export interface EditionMarkers {
  is_live: boolean;
  is_deluxe: boolean;
  is_remaster: boolean;
  country?: string | null;
  label?: string | null;
  catalog_number?: string | null;
}

export interface EditionEvidence {
  source: string;
  confidence: string;
}

export interface EditionContext {
  policy?: string | null;
  markers: EditionMarkers;
  evidence: EditionEvidence;
}

export interface StoredCandidateSetSummary {
  task_id: string;
  request_signature?: string | null;
  request_strategy?: string | null;
  disposition: string;
  selected_provider?: string | null;
  candidate_count: number;
  provider_count: number;
  updated_at: string;
}

export interface StoredProviderSearchRecord {
  provider_id: string;
  provider_display_name: string;
  provider_trust_rank: number;
  provider_order_index: number;
  outcome: string;
  candidate_count: number;
  error?: string | null;
  retryable: boolean;
  recorded_at: string;
}

export interface ReviewPreflightResult {
  passed: boolean;
  checked_at: string;
  reason_codes: string[];
  selected_candidate_count: number;
  provider_search_count: number;
  provider_success_count: number;
  candidate_count: number;
}

export interface ReviewApprovalPolicy {
  required: boolean;
  token?: string | null;
  low_trust_selected_providers: string[];
}

export interface ReviewContract {
  request: AcquisitionRequest;
  identity_lane: PlannerIdentityLane;
  edition?: EditionContext | null;
  candidate_set?: StoredCandidateSetSummary | null;
  provider_searches: StoredProviderSearchRecord[];
  candidate_review: CandidateReviewItem[];
  preflight: ReviewPreflightResult;
  approval: ReviewApprovalPolicy;
}

export interface TaskExecutionSummary {
  task_id: string;
  disposition: string;
  provider: string | null;
  failure_class: string | null;
  final_path: string | null;
  updated_at: string;
}

export interface DeadLetterItem {
  task_id: string;
  artist: string | null;
  title: string | null;
  album: string | null;
  provider: string | null;
  failed_at: string;
  request_json: string | null;
  request_signature: string | null;
}

export interface DeadLetterGroup {
  failure_class: string;
  label: string;
  suggested_fix: string;
  count: number;
  recent_items: DeadLetterItem[];
}

export interface DeadLetterSummary {
  groups: DeadLetterGroup[];
  total_count: number;
}

export interface RequestLineage {
  request: AcquisitionRequest;
  timeline: unknown[];
  execution: TaskExecutionSummary | null;
  provenance: string | null;
  candidate_review: unknown[];
  operation_events: TrustLedgerOperationEvent[];
  gatekeeper_audit: TrustLedgerGatekeeperAudit[];
  trust: TrustLedgerSummary;
}

// ── API ───────────────────────────────────────────────────────────────────────

export const api = {
  // Library
  getLibraryRoots: () => invoke<LibraryRoot[]>('get_library_roots'),
  addLibraryRoot: (path: string) => invoke<void>('add_library_root', { path }),
  removeLibraryRoot: (path: string) => invoke<void>('remove_library_root', { path }),
  scanLibrary: () => invoke<number>('scan_library'),
  getTracks: (limit = 500, offset = 0) => invoke<Track[]>('get_tracks', { limit, offset }),
  getRecentlyFinalizedTracks: (days?: number) =>
    invoke<Track[]>('get_recently_finalized_tracks', { days }),
  getTrackIdentityContext: (trackId: number) =>
    invoke<TrackIdentityContext | null>('get_track_identity_context', { track_id: trackId }),
  searchTracks: (query: string) => invoke<Track[]>('search_tracks', { query }),
  getAlbums: () => invoke<Album[]>('get_albums'),
  getAlbumTracks: (artist: string, album: string) =>
    invoke<Track[]>('get_album_tracks', { artist, album }),
  getArtists: () => invoke<Artist[]>('get_artists'),
  getTrackCount: () => invoke<number>('get_track_count'),

  // Player
  playerLoad: (path: string) => invoke<void>('player_load', { path }),
  playerPlay: () => invoke<void>('player_play'),
  playerPause: () => invoke<void>('player_pause'),
  playerStop: () => invoke<void>('player_stop'),
  playerToggle: () => invoke<void>('player_toggle'),
  playerNext: () => invoke<void>('player_next'),
  playerPrev: () => invoke<void>('player_prev'),
  playerSetVolume: (volume: number) => invoke<void>('player_set_volume', { volume }),
  playerSeek: (secs: number) => invoke<void>('player_seek', { secs }),
  getPlaybackState: () => invoke<PlaybackState>('get_playback_state'),
  getNowPlayingContext: (artist: string, title: string, album?: string) =>
    invoke<NowPlayingContext>('get_now_playing_context', { artist, title, album }),
  syncLastfmHistory: (username?: string, limit?: number) =>
    invoke<number>('sync_lastfm_history', { username, limit }),
  submitLastfmScrobble: (
    trackId: number,
    artist: string,
    title: string,
    album?: string,
    durationSecs?: number,
    positionSecs?: number,
  ) =>
    invoke<boolean>('submit_lastfm_scrobble', {
      trackId,
      artist,
      title,
      album,
      durationSecs,
      positionSecs,
    }),

  // Queue
  getQueue: () => invoke<QueueItem[]>('get_queue'),
  clearQueue: () => invoke<void>('clear_queue'),
  addToQueue: (trackId: number, position?: number) =>
    invoke<void>('add_to_queue', { track_id: trackId, position }),
  queueTracks: (trackIds: number[], startIndex?: number) =>
    invoke<void>('queue_tracks', { track_ids: trackIds, start_index: startIndex }),
  reorderQueue: (trackIds: number[], startIndex = 0) =>
    invoke<void>('queue_tracks', { track_ids: trackIds, start_index: startIndex }),
  removeQueueItem: (position: number, allTrackIds: number[], startIndex = 0) => {
    const remaining = allTrackIds.filter((_, i) => i !== position);
    if (remaining.length === 0) {
      return invoke<void>('clear_queue');
    }
    return invoke<void>('queue_tracks', { track_ids: remaining, start_index: startIndex });
  },

  // Playlists
  getPlaylists: () => invoke<Playlist[]>('get_playlists'),
  getPlaylistItems: (playlistId: number) =>
    invoke<PlaylistItem[]>('get_playlist_items', { playlist_id: playlistId }),
  createPlaylist: (name: string, description: string | null, trackIds: number[]) =>
    invoke<number>('create_playlist', { name, description, track_ids: trackIds }),
  replacePlaylistTracks: (playlistId: number, trackIds: number[]) =>
    invoke<void>('replace_playlist_tracks', { playlist_id: playlistId, track_ids: trackIds }),
  async addTrackToPlaylist(playlistId: number, trackId: number): Promise<void> {
    await invoke<void>('add_track_to_playlist', {
      playlist_id: playlistId,
      track_id: trackId,
    });
  },
  deletePlaylist: (playlistId: number) =>
    invoke<void>('delete_playlist', { playlist_id: playlistId }),
  playPlaylist: (playlistId: number, startIndex?: number) =>
    invoke<void>('play_playlist', { playlist_id: playlistId, start_index: startIndex }),

  // Downloads
  startDownload: (artist: string, title: string, album?: string) =>
    invoke<string>('start_download', { artist, title, album }),
  startAlbumDownloads: (albums: object[]) =>
    invoke<string[]>('start_album_downloads', { albums }),
  startDiscographyDownloads: (
    artist: string,
    artistMbid?: string,
    includeSingles?: boolean,
    includeEps?: boolean,
    includeCompilations?: boolean,
    maxAlbums?: number,
  ) =>
    invoke<AcquisitionQueueReport>('start_discography_downloads', {
      artist,
      artist_mbid: artistMbid,
      include_singles: includeSingles,
      include_eps: includeEps,
      include_compilations: includeCompilations,
      max_albums: maxAlbums,
    }),
  startArtistDownloads: (
    artist: string,
    artistMbid?: string,
    includeSingles?: boolean,
    includeEps?: boolean,
    includeCompilations?: boolean,
    maxAlbums?: number,
  ) =>
    invoke<AcquisitionQueueReport>('start_artist_downloads', {
      artist,
      artist_mbid: artistMbid,
      include_singles: includeSingles,
      include_eps: includeEps,
      include_compilations: includeCompilations,
      max_albums: maxAlbums,
    }),
  buildLibraryAcquisitionQueue: (artistFilter?: string, limit?: number) =>
    invoke<AcquisitionQueueReport>('build_library_acquisition_queue', {
      artist_filter: artistFilter,
      limit,
    }),
  cancelDownload: (taskId: string) => invoke<boolean>('cancel_download', { task_id: taskId }),
  getDownloadJobs: () => invoke<DownloadJob[]>('get_download_jobs'),
  searchDownloadMetadata: (query: string) =>
    invoke<DownloadMetadataSearchResult>('search_download_metadata', { query }),
  getArtistDiscography: (artist: string, artistMbid?: string) =>
    invoke<DownloadArtistDiscography>('get_artist_discography', {
      artist,
      artist_mbid: artistMbid,
    }),
  getSlskdTransfers: () => invoke<object[]>('get_slskd_transfers'),
  getCandidateReview: (taskId: string) =>
    invoke<CandidateReviewItem[]>('get_candidate_review', { task_id: taskId }),
  getTaskProvenance: (taskId: string) =>
    invoke<string | null>('get_task_provenance', { task_id: taskId }),
  getRecentTaskResults: (limit?: number) =>
    invoke<TaskResultSummary[]>('get_recent_task_results', { limit }),
  createAcquisitionRequest: (request: AcquisitionRequest) =>
    invoke('create_acquisition_request', { request }),
  planAcquisition: (request: AcquisitionRequest) =>
    invoke<PlannedAcquisitionResult>('plan_acquisition', { request }),
  getReviewContract: (requestId: number) =>
    invoke<ReviewContract>('get_review_contract', { requestId }),
  approvePlannedRequest: (requestId: number, note?: string, excludedProviderIds?: string[]) =>
    invoke('approve_planned_request', { requestId, note, excludedProviderIds }),
  rejectPlannedRequest: (requestId: number, reason?: string, excludedProviderIds?: string[]) =>
    invoke('reject_planned_request', { requestId, reason, excludedProviderIds }),
  listAcquisitionRequests: (status?: string, limit?: number) =>
    invoke<AcquisitionRequestListItem[]>('list_acquisition_requests', { status, limit }),
  getAcquisitionRequestTimeline: (requestId: number) =>
    invoke<AcquisitionRequestEvent[]>('get_acquisition_request_timeline', { requestId }),
  getRequestCandidateReview: (requestId: number) =>
    invoke<CandidateReviewItem[]>('get_request_candidate_review', { requestId }),
  getRequestLineage: (requestId: number) =>
    invoke<RequestLineage>('get_request_lineage', { requestId }),
  getTrustReasonDistribution: (limit?: number) =>
    invoke<TrustReasonDistributionEntry[]>('get_trust_reason_distribution', { limit }),
  getDeadLetterSummary: (recentLimit?: number) =>
    invoke<DeadLetterSummary>('get_dead_letter_summary', { recentLimit: recentLimit ?? 5 }),
  replayDeadLetter: (taskId: string) => invoke<number>('replay_dead_letter', { taskId }),

  // Import
  parseSpotifyHistory: (path: string) =>
    invoke<SpotifyImportResult>('parse_spotify_history', { path }),
  importSpotifyDesiredTracks: (path: string) =>
    invoke<number>('import_spotify_desired_tracks', { path }),
  queueSpotifyAlbums: (albums: SpotifyAlbumSummary[]) =>
    invoke<number>('queue_spotify_albums', { albums }),
  getSpotifyImportStatus: () =>
    invoke<SpotifyImportStatus>('get_spotify_import_status'),
  getMissingSpotifyAlbums: (limit?: number) =>
    invoke<SpotifyAlbumHistory[]>('get_missing_spotify_albums', { limit }),

  // Library Organization
  organizeLibrary: (dryRun?: boolean) =>
    invoke<OrganizeReport>('organize_library', { dry_run: dryRun }),
  findDuplicates: () => invoke<DuplicateGroup[]>('find_duplicates'),
  resolveDuplicate: (keepTrackId: number, removeTrackIds: number[], deleteFiles?: boolean) =>
    invoke<number>('resolve_duplicate', {
      keep_track_id: keepTrackId,
      remove_track_ids: removeTrackIds,
      delete_files: deleteFiles,
    }),
  pruneMissingTracks: () => invoke<number>('prune_missing_tracks'),
  proposeTagFixes: (artist: string, album: string) =>
    invoke<TagFix[]>('propose_tag_fixes', { artist, album }),
  applyTagFixes: (fixes: TagFix[]) =>
    invoke<number>('apply_tag_fixes', { fixes }),
  ingestStaging: () => invoke<string[]>('ingest_staging'),

  // Settings
  getSetting: (key: string) => invoke<string | null>('get_setting', { key }),
  setSetting: (key: string, value: string) => invoke<void>('set_setting', { key, value }),
  getPolicyProfile: () => invoke<PolicyProfile>('get_policy_profile'),
  setPolicyProfile: (profile: PolicyProfile) =>
    invoke<PolicyProfile>('set_policy_profile', { profile }),
  getConfig: () => invoke<DownloadConfig>('get_config'),
  getProviderStatuses: () => invoke<ProviderStatus[]>('get_provider_statuses'),
  getSlskdRuntimeStatus: () => invoke<SlskdRuntimeStatus>('get_slskd_runtime_status'),
  restartSlskdRuntime: () => invoke<SlskdRuntimeStatus>('restart_slskd_runtime'),
  stopSlskdRuntime: () => invoke<SlskdRuntimeStatus>('stop_slskd_runtime'),
  saveConfig: (config: DownloadConfig) => invoke<void>('save_config', { config }),
  persistEffectiveConfig: () => invoke<void>('persist_effective_config'),

  // Background backlog downloader
  startBacklogRun: (batchSize?: number, limit?: number, operatorDirectSubmit?: boolean) =>
    invoke<BacklogRunStatus>('start_backlog_run', {
      batch_size: batchSize,
      limit,
      operator_direct_submit: operatorDirectSubmit,
    }),
  stopBacklogRun: () => invoke<void>('stop_backlog_run'),
  getBacklogStatus: () => invoke<BacklogRunStatus>('get_backlog_status'),
  getDirectorDebugStats: (limit?: number) =>
    invoke<DirectorDebugStats>('get_director_debug_stats', { limit }),
};
