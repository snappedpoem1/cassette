pub const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS artists (
      id INTEGER PRIMARY KEY,
      canonical_name TEXT NOT NULL,
      normalized_name TEXT NOT NULL,
      spotify_id TEXT UNIQUE,
      discogs_id TEXT UNIQUE,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE UNIQUE INDEX IF NOT EXISTS uq_artists_normalized_name ON artists(normalized_name);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS albums (
      id INTEGER PRIMARY KEY,
      artist_id INTEGER NOT NULL REFERENCES artists(id),
      title TEXT NOT NULL,
      normalized_title TEXT NOT NULL,
      release_date DATE,
      spotify_id TEXT UNIQUE,
      discogs_id TEXT UNIQUE,
      cover_art_path TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE UNIQUE INDEX IF NOT EXISTS uq_albums_artist_normalized_title
      ON albums(artist_id, normalized_title);
    CREATE INDEX IF NOT EXISTS idx_albums_artist_id ON albums(artist_id);
    CREATE INDEX IF NOT EXISTS idx_albums_normalized_title ON albums(normalized_title);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS tracks (
      id INTEGER PRIMARY KEY,
      album_id INTEGER REFERENCES albums(id),
      artist_id INTEGER NOT NULL REFERENCES artists(id),
      title TEXT NOT NULL,
      normalized_title TEXT NOT NULL,
      track_number INTEGER,
      disc_number INTEGER,
      duration_ms INTEGER,
      isrc TEXT UNIQUE,
      spotify_id TEXT UNIQUE,
      discogs_id TEXT UNIQUE,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_tracks_artist_id ON tracks(artist_id);
    CREATE INDEX IF NOT EXISTS idx_tracks_album_id ON tracks(album_id);
    CREATE INDEX IF NOT EXISTS idx_tracks_isrc ON tracks(isrc);
    CREATE INDEX IF NOT EXISTS idx_tracks_normalized_title ON tracks(normalized_title);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS local_files (
      id INTEGER PRIMARY KEY,
      track_id INTEGER REFERENCES tracks(id),
      file_path TEXT UNIQUE NOT NULL,
      file_name TEXT NOT NULL,
      extension TEXT NOT NULL,
      codec TEXT,
      bitrate INTEGER,
      sample_rate INTEGER,
      bit_depth INTEGER,
      channels INTEGER,
      duration_ms INTEGER,
      file_size INTEGER,
      file_mtime_ms INTEGER,
      content_hash TEXT,
      acoustid_fingerprint TEXT,
      fingerprint_attempted_at TIMESTAMP,
      fingerprint_error TEXT,
      fingerprint_source_mtime_ms INTEGER,
      integrity_status TEXT NOT NULL,
      quality_tier TEXT,
      last_scanned_at TIMESTAMP,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_local_files_track_id ON local_files(track_id);
    CREATE INDEX IF NOT EXISTS idx_local_files_content_hash ON local_files(content_hash);
    CREATE INDEX IF NOT EXISTS idx_local_files_acoustid_fingerprint ON local_files(acoustid_fingerprint);
    CREATE INDEX IF NOT EXISTS idx_local_files_integrity_status ON local_files(integrity_status);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS scan_checkpoints (
      id INTEGER PRIMARY KEY,
      root_path TEXT NOT NULL UNIQUE,
      last_run_id TEXT,
      last_scanned_path TEXT,
      status TEXT NOT NULL DEFAULT 'pending',
      files_seen INTEGER DEFAULT 0,
      files_indexed INTEGER DEFAULT 0,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_scan_checkpoints_status ON scan_checkpoints(status);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS desired_tracks (
      id INTEGER PRIMARY KEY,
      source_name TEXT NOT NULL,
      source_track_id TEXT,
      source_album_id TEXT,
      source_artist_id TEXT,
      artist_name TEXT NOT NULL,
      album_title TEXT,
      track_title TEXT NOT NULL,
      track_number INTEGER,
      disc_number INTEGER,
      duration_ms INTEGER,
      isrc TEXT,
      raw_payload_json TEXT,
      imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_desired_tracks_source_track_id ON desired_tracks(source_track_id);
    CREATE INDEX IF NOT EXISTS idx_desired_tracks_isrc ON desired_tracks(isrc);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS reconciliation_results (
      id INTEGER PRIMARY KEY,
      desired_track_id INTEGER NOT NULL REFERENCES desired_tracks(id),
      matched_track_id INTEGER REFERENCES tracks(id),
      matched_local_file_id INTEGER REFERENCES local_files(id),
      reconciliation_status TEXT NOT NULL,
      quality_assessment TEXT,
      reason TEXT NOT NULL,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_reconciliation_desired_track_id ON reconciliation_results(desired_track_id);
    CREATE INDEX IF NOT EXISTS idx_reconciliation_status ON reconciliation_results(reconciliation_status);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS delta_queue (
      id INTEGER PRIMARY KEY,
      desired_track_id INTEGER NOT NULL REFERENCES desired_tracks(id),
      action_type TEXT NOT NULL,
      priority INTEGER DEFAULT 0,
      reason TEXT NOT NULL,
      target_quality TEXT,
      source_operation_id TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      claimed_at TIMESTAMP,
      claim_run_id TEXT,
      processed_at TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_delta_queue_action_type ON delta_queue(action_type);
    CREATE INDEX IF NOT EXISTS idx_delta_queue_priority ON delta_queue(priority DESC);
    CREATE INDEX IF NOT EXISTS idx_delta_queue_claim_run_id ON delta_queue(claim_run_id);
    CREATE INDEX IF NOT EXISTS idx_delta_queue_processed_at ON delta_queue(processed_at);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS sync_runs (
      id INTEGER PRIMARY KEY,
      run_id TEXT UNIQUE NOT NULL,
      started_at TIMESTAMP NOT NULL,
      ended_at TIMESTAMP,
      status TEXT NOT NULL,
      phase_reached TEXT NOT NULL,
      files_scanned INTEGER DEFAULT 0,
      files_upserted INTEGER DEFAULT 0,
      desired_tracks_imported INTEGER DEFAULT 0,
      reconciliation_completed BOOLEAN DEFAULT FALSE,
      delta_queue_entries INTEGER DEFAULT 0,
      error_message TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_sync_runs_run_id ON sync_runs(run_id);
    CREATE INDEX IF NOT EXISTS idx_sync_runs_status ON sync_runs(status);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS canonical_artists (
      id INTEGER PRIMARY KEY,
      name TEXT NOT NULL,
      normalized_name TEXT NOT NULL,
      musicbrainz_id TEXT UNIQUE,
      spotify_id TEXT UNIQUE,
      discogs_id TEXT UNIQUE,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_artists_normalized_name
      ON canonical_artists(normalized_name);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS canonical_releases (
      id INTEGER PRIMARY KEY,
      canonical_artist_id INTEGER NOT NULL REFERENCES canonical_artists(id),
      title TEXT NOT NULL,
      normalized_title TEXT NOT NULL,
      release_group_mbid TEXT,
      release_mbid TEXT UNIQUE,
      spotify_id TEXT UNIQUE,
      discogs_id TEXT UNIQUE,
      release_type TEXT,
      year INTEGER,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_releases_artist_normalized_title
      ON canonical_releases(canonical_artist_id, normalized_title);
    CREATE INDEX IF NOT EXISTS idx_canonical_releases_release_group_mbid
      ON canonical_releases(release_group_mbid);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS canonical_recordings (
      id INTEGER PRIMARY KEY,
      canonical_artist_id INTEGER REFERENCES canonical_artists(id),
      canonical_release_id INTEGER REFERENCES canonical_releases(id),
      title TEXT NOT NULL,
      normalized_title TEXT NOT NULL,
      musicbrainz_recording_id TEXT UNIQUE,
      isrc TEXT,
      track_number INTEGER,
      disc_number INTEGER,
      duration_ms INTEGER,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_canonical_recordings_release_id
      ON canonical_recordings(canonical_release_id);
    CREATE INDEX IF NOT EXISTS idx_canonical_recordings_isrc
      ON canonical_recordings(isrc);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS acquisition_requests (
      id INTEGER PRIMARY KEY,
      scope TEXT NOT NULL,
      source_name TEXT NOT NULL,
      source_track_id TEXT,
      source_album_id TEXT,
      source_artist_id TEXT,
      artist TEXT NOT NULL,
      album TEXT,
      title TEXT NOT NULL DEFAULT '',
      normalized_artist TEXT NOT NULL,
      normalized_album TEXT,
      normalized_title TEXT NOT NULL DEFAULT '',
      track_number INTEGER,
      disc_number INTEGER,
      year INTEGER,
      duration_secs REAL,
      isrc TEXT,
      musicbrainz_recording_id TEXT,
      musicbrainz_release_group_id TEXT,
      musicbrainz_release_id TEXT,
      canonical_artist_id INTEGER REFERENCES canonical_artists(id),
      canonical_release_id INTEGER REFERENCES canonical_releases(id),
      strategy TEXT NOT NULL,
      quality_policy TEXT,
      excluded_providers_json TEXT,
      edition_policy TEXT,
      confirmation_policy TEXT NOT NULL DEFAULT 'automatic',
      desired_track_id INTEGER,
      source_operation_id TEXT,
      task_id TEXT UNIQUE,
      request_signature TEXT NOT NULL UNIQUE,
      status TEXT NOT NULL DEFAULT 'pending',
      raw_payload_json TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_acquisition_requests_status
      ON acquisition_requests(status, created_at DESC);
    CREATE INDEX IF NOT EXISTS idx_acquisition_requests_desired_track_id
      ON acquisition_requests(desired_track_id);
    CREATE INDEX IF NOT EXISTS idx_acquisition_requests_source_operation_id
      ON acquisition_requests(source_operation_id);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS acquisition_request_events (
      id INTEGER PRIMARY KEY,
      request_id INTEGER NOT NULL REFERENCES acquisition_requests(id) ON DELETE CASCADE,
      task_id TEXT,
      event_type TEXT NOT NULL,
      status TEXT NOT NULL,
      message TEXT,
      payload_json TEXT,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_acquisition_request_events_request_id
      ON acquisition_request_events(request_id, created_at ASC, id ASC);
    CREATE INDEX IF NOT EXISTS idx_acquisition_request_events_task_id
      ON acquisition_request_events(task_id, created_at ASC, id ASC);
    "#,
];
