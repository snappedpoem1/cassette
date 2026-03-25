pub const SCHEMA_VERSION: u32 = 3;

pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "migration_001_core_schema_metadata",
        r#"
        CREATE TABLE IF NOT EXISTS artists (
            id INTEGER PRIMARY KEY,
            canonical_name TEXT,
            normalized_name TEXT
        );
        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY,
            artist_id INTEGER,
            title TEXT,
            normalized_title TEXT
        );
        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY,
            album_id INTEGER,
            artist_id INTEGER,
            title TEXT,
            normalized_title TEXT
        );
        CREATE TABLE IF NOT EXISTS local_files (
            id INTEGER PRIMARY KEY,
            track_id INTEGER,
            file_path TEXT UNIQUE,
            content_hash TEXT,
            integrity_status TEXT,
            quality_tier TEXT
        );
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TIMESTAMP NOT NULL,
            migration_name TEXT NOT NULL
        );
        "#,
    ),
    (
        "migration_002_operations_and_locks",
        r#"
        CREATE TABLE IF NOT EXISTS operation_log (
            operation_id TEXT PRIMARY KEY,
            module TEXT NOT NULL,
            phase TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TIMESTAMP NOT NULL,
            ended_at TIMESTAMP,
            duration_ms INTEGER,
            error_message TEXT,
            error_context_json TEXT,
            files_affected INTEGER DEFAULT 0,
            tracks_affected INTEGER DEFAULT 0,
            metadata_json TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_operation_log_module ON operation_log(module);
        CREATE INDEX IF NOT EXISTS idx_operation_log_status ON operation_log(status);
        CREATE INDEX IF NOT EXISTS idx_operation_log_started_at ON operation_log(started_at DESC);

        CREATE TABLE IF NOT EXISTS file_locks (
            lock_id INTEGER PRIMARY KEY,
            file_path TEXT UNIQUE NOT NULL,
            locked_by TEXT NOT NULL,
            operation_id TEXT REFERENCES operation_log(operation_id),
            acquired_at TIMESTAMP NOT NULL,
            timeout_at TIMESTAMP NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_file_locks_operation_id ON file_locks(operation_id);
        CREATE INDEX IF NOT EXISTS idx_file_locks_timeout ON file_locks(timeout_at);
        "#,
    ),
    (
        "migration_003_operation_events_and_invariants",
        r#"
        CREATE TABLE IF NOT EXISTS operation_events (
            event_id INTEGER PRIMARY KEY,
            operation_id TEXT NOT NULL REFERENCES operation_log(operation_id),
            event_type TEXT NOT NULL,
            target_file_id INTEGER REFERENCES local_files(id),
            target_track_id INTEGER REFERENCES tracks(id),
            before_state_json TEXT,
            after_state_json TEXT,
            event_data TEXT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_operation_events_operation_id ON operation_events(operation_id);
        CREATE INDEX IF NOT EXISTS idx_operation_events_target_file ON operation_events(target_file_id);
        CREATE INDEX IF NOT EXISTS idx_operation_events_target_track ON operation_events(target_track_id);

        CREATE TABLE IF NOT EXISTS invariant_violations (
            violation_id INTEGER PRIMARY KEY,
            invariant_name TEXT NOT NULL,
            violating_record_json TEXT NOT NULL,
            attempted_operation_id TEXT REFERENCES operation_log(operation_id),
            detected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            resolved BOOLEAN DEFAULT FALSE
        );
        "#,
    ),
];
