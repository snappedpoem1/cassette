use crate::gatekeeper::mod_types::JunkFlag;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySpine {
    pub allow_karaoke: bool,
    pub allow_live: bool,
    pub allow_instrumental: bool,
    pub allow_remixes: bool,
    pub allow_demos: bool,
    pub allow_unofficial: bool,
    pub allow_alt_versions: bool,
    pub allow_skits_and_interludes: bool,
    pub junk_filter_patterns: Vec<(String, JunkFlag)>,
}

impl Default for PolicySpine {
    fn default() -> Self {
        Self {
            allow_karaoke: false,
            allow_live: false,
            allow_instrumental: false,
            allow_remixes: false,
            allow_demos: false,
            allow_unofficial: false,
            allow_alt_versions: false,
            allow_skits_and_interludes: false,
            junk_filter_patterns: vec![
                ("karaoke".to_string(), JunkFlag::IsKaraoke),
                ("\\blive\\b".to_string(), JunkFlag::IsLiveVersion),
                ("instrumental".to_string(), JunkFlag::IsInstrumental),
                ("remix".to_string(), JunkFlag::IsRemix),
                ("demo".to_string(), JunkFlag::IsDemo),
                ("unofficial|bootleg".to_string(), JunkFlag::IsUnofficial),
                (
                    "alt\\.? version|alternate".to_string(),
                    JunkFlag::IsAltVersion,
                ),
                (
                    "interlude|skit|speech".to_string(),
                    JunkFlag::IsSkitOrSpeech,
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum HashAlgorithm {
    Blake3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatekeeperConfig {
    pub canonical_library_root: PathBuf,
    pub staging_root: PathBuf,
    pub quarantine_root: PathBuf,
    pub enrichment_queue_enabled: bool,

    pub bitrate_floor_lossy: u32,
    pub bitrate_floor_lossless: u32,
    pub sample_rate_floor: u32,
    pub supported_codecs: Vec<String>,

    pub reject_below_floor: bool,
    pub reject_unsupported_codec: bool,

    pub fingerprint_confidence_floor: f32,
    pub require_identity_match: bool,
    pub allow_fuzzy_match: bool,

    pub policy_spine: PolicySpine,
    pub reject_junk_files: bool,
    pub allow_junk_if_requested: bool,

    pub on_duplicate_keep_best: bool,
    pub on_duplicate_allow_both: bool,
    pub on_duplicate_require_review: bool,

    pub verify_after_copy: bool,
    pub verify_hash_algorithm: HashAlgorithm,

    pub audit_log_enabled: bool,
    pub audit_manifest_dir: PathBuf,
    pub tracing_level: String,

    pub max_concurrent_ingest: usize,
    pub fingerprint_timeout_ms: u64,
    pub decode_timeout_ms: u64,
}

impl Default for GatekeeperConfig {
    fn default() -> Self {
        Self {
            canonical_library_root: PathBuf::from("A:\\music_sorted"),
            staging_root: PathBuf::from("A:\\music_staging"),
            quarantine_root: PathBuf::from("A:\\music_quarantine"),
            enrichment_queue_enabled: true,
            bitrate_floor_lossy: 128,
            bitrate_floor_lossless: 0,
            sample_rate_floor: 44_100,
            supported_codecs: vec![
                "flac".to_string(),
                "mp3".to_string(),
                "m4a".to_string(),
                "aac".to_string(),
                "opus".to_string(),
                "ogg".to_string(),
                "wav".to_string(),
                "aiff".to_string(),
            ],
            reject_below_floor: false,
            reject_unsupported_codec: true,
            fingerprint_confidence_floor: 0.7,
            require_identity_match: false,
            allow_fuzzy_match: true,
            policy_spine: PolicySpine::default(),
            reject_junk_files: true,
            allow_junk_if_requested: true,
            on_duplicate_keep_best: true,
            on_duplicate_allow_both: false,
            on_duplicate_require_review: false,
            verify_after_copy: true,
            verify_hash_algorithm: HashAlgorithm::Blake3,
            audit_log_enabled: true,
            audit_manifest_dir: PathBuf::from("A:\\music_admin\\gatekeeper_runs"),
            tracing_level: "info".to_string(),
            max_concurrent_ingest: 4,
            fingerprint_timeout_ms: 30_000,
            decode_timeout_ms: 10_000,
        }
    }
}
