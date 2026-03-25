-- View all operations in order
SELECT operation_id, module, phase, status, duration_ms, error_message
FROM operation_log
ORDER BY started_at DESC;

-- View all events for an operation
-- Replace :operation_id with an actual operation id in your SQL client
SELECT event_type, target_file_id, target_track_id, timestamp, event_data
FROM operation_events
WHERE operation_id = :operation_id
ORDER BY event_id ASC;

-- Get file lineage by substring match in event payload
-- Replace :needle with file path or file name substring
SELECT ol.module, ol.phase, oe.event_type, oe.timestamp, oe.event_data
FROM operation_events oe
JOIN operation_log ol ON oe.operation_id = ol.operation_id
WHERE oe.event_data LIKE '%' || :needle || '%'
ORDER BY oe.event_id ASC;

-- Count deltas by action type
SELECT action_type, COUNT(*) AS count
FROM delta_queue
GROUP BY action_type;

-- Find failed downloads
SELECT target_track_id, event_data, timestamp
FROM operation_events
WHERE event_type = 'download_failed'
ORDER BY event_id DESC;

-- Check for stalled operations
SELECT operation_id, module, phase, started_at
FROM operation_log
WHERE status = 'in_progress'
  AND datetime(started_at) < datetime('now', '-1 hour');

-- Verify no orphaned successful operations
SELECT operation_id, status
FROM operation_log ol
WHERE NOT EXISTS (
    SELECT 1 FROM operation_events oe WHERE oe.operation_id = ol.operation_id
)
  AND ol.status IN ('success', 'partial_success');
