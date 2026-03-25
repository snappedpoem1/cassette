pub const INVARIANT_TRIGGERS: &[&str] = &[
    r#"
    CREATE TRIGGER IF NOT EXISTS file_uniqueness_check
    BEFORE INSERT ON local_files
    FOR EACH ROW
    WHEN (NEW.content_hash IS NOT NULL AND (
      SELECT COUNT(*) FROM local_files WHERE content_hash = NEW.content_hash AND id != NEW.id
    ) > 0)
    BEGIN
      SELECT RAISE(ABORT, 'File already exists (duplicate content hash)');
    END;
    "#,
    r#"
    CREATE TRIGGER IF NOT EXISTS track_album_integrity
    BEFORE INSERT ON tracks
    FOR EACH ROW
    WHEN (NEW.album_id IS NOT NULL AND (
      SELECT COUNT(*) FROM albums WHERE id = NEW.album_id
    ) = 0)
    BEGIN
      SELECT RAISE(ABORT, 'Album does not exist');
    END;
    "#,
    r#"
    CREATE TRIGGER IF NOT EXISTS operation_end_clears_locks
    AFTER UPDATE ON operation_log
    FOR EACH ROW
    WHEN (OLD.status = 'in_progress' AND NEW.status IN ('success', 'failed', 'rolled_back'))
    BEGIN
      DELETE FROM file_locks WHERE operation_id = NEW.operation_id;
    END;
    "#,
];
