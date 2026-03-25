#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub database_url: String,
    pub max_lock_timeout_ms: u64,
    pub stalled_operation_timeout_secs: u64,
    pub enable_invariant_checks: bool,
    pub enable_operation_event_logging: bool,
    pub audit_log_retention_days: u64,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite://cassette.db".to_string(),
            max_lock_timeout_ms: 300_000,
            stalled_operation_timeout_secs: 3_600,
            enable_invariant_checks: true,
            enable_operation_event_logging: true,
            audit_log_retention_days: 90,
        }
    }
}
