//! Provider settings compatibility types.
//!
//! These structs exist for persisted desktop settings and UI status reporting.
//! They do not own runtime acquisition behavior; that lives under `director/providers/`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub library_base: String,
    pub staging_folder: String,
    // Soulseek
    pub slskd_url: Option<String>,
    pub slskd_user: Option<String>,
    pub slskd_pass: Option<String>,
    pub slskd_downloads_dir: Option<String>,
    // Real-Debrid / torrents
    pub real_debrid_key: Option<String>,
    pub jackett_url: Option<String>,
    pub jackett_api_key: Option<String>,
    // Usenet
    pub nzbgeek_api_key: Option<String>,
    pub sabnzbd_url: Option<String>,
    pub sabnzbd_api_key: Option<String>,
    // Streaming
    pub qobuz_email: Option<String>,
    pub qobuz_password: Option<String>,
    pub deezer_arl: Option<String>,
    // Spotify
    pub spotify_client_id: Option<String>,
    pub spotify_client_secret: Option<String>,
    pub spotify_access_token: Option<String>,
    // Enrichment
    pub genius_token: Option<String>,
    pub discogs_token: Option<String>,
    pub lastfm_api_key: Option<String>,
    pub lastfm_username: Option<String>,
    // Tools
    pub ytdlp_path: Option<String>,
    pub sevenzip_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub id: String,
    pub label: String,
    pub configured: bool,
    pub summary: String,
    pub missing_fields: Vec<String>,
}

impl DownloadConfig {
    pub fn from_env() -> Self {
        Self {
            library_base: std::env::var("LIBRARY_BASE").unwrap_or_default(),
            staging_folder: std::env::var("STAGING_FOLDER").unwrap_or_default(),
            slskd_url: std::env::var("SLSKD_URL").ok(),
            slskd_user: std::env::var("SLSKD_USER").ok(),
            slskd_pass: std::env::var("SLSKD_PASSWORD").ok(),
            slskd_downloads_dir: std::env::var("SLSKD_DOWNLOADS_DIR").ok(),
            real_debrid_key: std::env::var("REAL_DEBRID_KEY").ok(),
            jackett_url: std::env::var("JACKETT_URL").ok(),
            jackett_api_key: std::env::var("JACKETT_API_KEY").ok(),
            nzbgeek_api_key: std::env::var("NZBGEEK_API_KEY").ok(),
            sabnzbd_url: std::env::var("SABNZBD_URL").ok(),
            sabnzbd_api_key: std::env::var("SABNZBD_API_KEY").ok(),
            qobuz_email: std::env::var("QOBUZ_EMAIL").ok(),
            qobuz_password: std::env::var("QOBUZ_PASSWORD").ok(),
            deezer_arl: std::env::var("DEEZER_ARL").ok(),
            spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID").ok(),
            spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET").ok(),
            spotify_access_token: std::env::var("SPOTIFY_ACCESS_TOKEN").ok(),
            genius_token: std::env::var("GENIUS_TOKEN").ok(),
            discogs_token: std::env::var("DISCOGS_TOKEN").ok(),
            lastfm_api_key: std::env::var("LASTFM_API_KEY").ok(),
            lastfm_username: std::env::var("LASTFM_USERNAME").ok(),
            ytdlp_path: std::env::var("YTDLP_PATH").ok(),
            sevenzip_path: std::env::var("SEVENZIP_PATH").ok(),
        }
    }

    pub fn provider_statuses(&self) -> Vec<ProviderStatus> {
        vec![
            // ── Streaming / lossless ──────────────────────────────────────
            Self::build_status(
                "qobuz",
                "Qobuz",
                vec![
                    ("email", &self.qobuz_email),
                    ("password", &self.qobuz_password),
                ],
                None,
            ),
            Self::build_status("deezer", "Deezer", vec![("ARL", &self.deezer_arl)], None),
            // ── Torrent / debrid ─────────────────────────────────────────
            Self::build_status(
                "jackett",
                "Jackett (canonical torrent search)",
                vec![
                    ("URL", &self.jackett_url),
                    ("API key", &self.jackett_api_key),
                ],
                Some(vec![(
                    "Real-Debrid key (required for resolve)",
                    &self.real_debrid_key,
                )]),
            ),
            Self::build_status(
                "real_debrid",
                "Real-Debrid (resolver/unrestrict; direct search debug-only)",
                vec![("API key", &self.real_debrid_key)],
                None,
            ),
            // ── Usenet ───────────────────────────────────────────────────
            Self::build_status(
                "usenet",
                "Usenet (NZBGeek + SABnzbd)",
                vec![
                    ("NZBGeek API key", &self.nzbgeek_api_key),
                    ("SABnzbd URL", &self.sabnzbd_url),
                    ("SABnzbd API key", &self.sabnzbd_api_key),
                ],
                None,
            ),
            // ── P2P ──────────────────────────────────────────────────────
            Self::build_status(
                "slskd",
                "Soulseek / slskd",
                vec![
                    ("URL", &self.slskd_url),
                    ("username", &self.slskd_user),
                    ("password", &self.slskd_pass),
                ],
                Some(vec![("downloads dir", &self.slskd_downloads_dir)]),
            ),
            // ── Metadata ─────────────────────────────────────────────────
            Self::build_status(
                "spotify",
                "Spotify",
                vec![
                    ("client ID", &self.spotify_client_id),
                    ("client secret", &self.spotify_client_secret),
                ],
                Some(vec![("access token", &self.spotify_access_token)]),
            ),
            Self::build_status(
                "lastfm",
                "Last.fm",
                vec![],
                Some(vec![
                    ("API key", &self.lastfm_api_key),
                    ("username", &self.lastfm_username),
                ]),
            ),
            // ── Tools ────────────────────────────────────────────────────
            Self::build_status(
                "ytdlp",
                "yt-dlp",
                vec![],
                Some(vec![("binary path", &self.ytdlp_path)]),
            ),
            Self::build_status(
                "sevenzip",
                "7-Zip",
                vec![],
                Some(vec![("binary path", &self.sevenzip_path)]),
            ),
        ]
    }

    fn build_status(
        id: &str,
        label: &str,
        required_fields: Vec<(&str, &Option<String>)>,
        optional_fields: Option<Vec<(&str, &Option<String>)>>,
    ) -> ProviderStatus {
        let missing_fields = required_fields
            .iter()
            .filter_map(|(field, value)| (!Self::is_present(value)).then_some((*field).to_string()))
            .collect::<Vec<_>>();

        let configured = missing_fields.is_empty();
        let optional_ready = optional_fields
            .as_ref()
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|(field, value)| {
                        Self::is_present(value).then_some((*field).to_string())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let summary = if configured {
            if optional_ready.is_empty() {
                "Configured".to_string()
            } else {
                format!("Configured; also found {}", optional_ready.join(", "))
            }
        } else if missing_fields.len() == required_fields.len() {
            "Not configured".to_string()
        } else {
            format!("Partial; missing {}", missing_fields.join(", "))
        };

        ProviderStatus {
            id: id.to_string(),
            label: label.to_string(),
            configured,
            summary,
            missing_fields,
        }
    }

    fn is_present(value: &Option<String>) -> bool {
        value.as_ref().is_some_and(|item| !item.trim().is_empty())
    }
}
