use crate::custodian::CustodianConfig;
use crate::librarian::LibrarianConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationConfig {
    pub exact_match_confidence: f32,
    pub strong_match_floor: f32,
    pub fuzzy_match_floor: f32,
    pub duration_tolerance_ms: u32,
    pub prefer_lossless: bool,
    pub acceptable_lossy_bitrate: u32,
    pub minimum_bitrate: u32,
    pub detect_by_fingerprint: bool,
    pub detect_by_content_hash: bool,
    pub cache_ttl_hours: u32,
}

impl Default for ReconciliationConfig {
    fn default() -> Self {
        Self {
            exact_match_confidence: 1.0,
            strong_match_floor: 0.85,
            fuzzy_match_floor: 0.70,
            duration_tolerance_ms: 2_000,
            prefer_lossless: true,
            acceptable_lossy_bitrate: 256,
            minimum_bitrate: 128,
            detect_by_fingerprint: true,
            detect_by_content_hash: true,
            cache_ttl_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    pub librarian: LibrarianConfig,
    pub custodian: CustodianConfig,
    pub reconciliation: ReconciliationConfig,
    pub desired_state_sources: Vec<String>,
    pub run_librarian: bool,
    pub run_custodian: bool,
    pub run_reconciliation: bool,
    pub library_roots: Vec<PathBuf>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        let librarian = LibrarianConfig::default();
        let library_roots = librarian.library_roots.clone();
        Self {
            librarian,
            custodian: CustodianConfig::default(),
            reconciliation: ReconciliationConfig::default(),
            desired_state_sources: vec!["spotify_export".to_string(), "user_playlists".to_string()],
            run_librarian: true,
            run_custodian: true,
            run_reconciliation: true,
            library_roots,
        }
    }
}
