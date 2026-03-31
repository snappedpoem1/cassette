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
      integrity_status TEXT NOT NULL,
      quality_tier TEXT,
      last_scanned_at TIMESTAMP,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_local_files_track_id ON local_files(track_id);
    CREATE INDEX IF NOT EXISTS idx_local_files_content_hash ON local_files(content_hash);
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
];
