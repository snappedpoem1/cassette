use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub library_base: String,
    pub staging_folder: String,
    pub slskd_url: Option<String>,
    pub slskd_user: Option<String>,
    pub slskd_pass: Option<String>,
    pub real_debrid_key: Option<String>,
    pub jackett_url: Option<String>,
    pub jackett_api_key: Option<String>,
    pub qobuz_email: Option<String>,
    pub qobuz_password: Option<String>,
    pub deezer_arl: Option<String>,
    pub spotify_client_id: Option<String>,
    pub spotify_client_secret: Option<String>,
    pub spotify_access_token: Option<String>,
    pub genius_token: Option<String>,
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
            real_debrid_key: std::env::var("REAL_DEBRID_KEY").ok(),
            jackett_url: std::env::var("JACKETT_URL").ok(),
            jackett_api_key: std::env::var("JACKETT_API_KEY").ok(),
            qobuz_email: std::env::var("QOBUZ_EMAIL").ok(),
            qobuz_password: std::env::var("QOBUZ_PASSWORD").ok(),
            deezer_arl: std::env::var("DEEZER_ARL").ok(),
            spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID").ok(),
            spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET").ok(),
            spotify_access_token: std::env::var("SPOTIFY_ACCESS_TOKEN").ok(),
            genius_token: std::env::var("GENIUS_TOKEN").ok(),
        }
    }

    pub fn provider_statuses(&self) -> Vec<ProviderStatus> {
        vec![
            Self::build_status(
                "slskd",
                "Soulseek / slskd",
                vec![
                    ("URL", &self.slskd_url),
                    ("username", &self.slskd_user),
                    ("password", &self.slskd_pass),
                ],
                None,
            ),
            Self::build_status(
                "real_debrid",
                "Real-Debrid",
                vec![("API key", &self.real_debrid_key)],
                None,
            ),
            Self::build_status(
                "deezer",
                "Deezer",
                vec![("ARL", &self.deezer_arl)],
                None,
            ),
            Self::build_status(
                "qobuz",
                "Qobuz",
                vec![
                    ("email", &self.qobuz_email),
                    ("password", &self.qobuz_password),
                ],
                None,
            ),
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
                "genius",
                "Genius",
                vec![("access token", &self.genius_token)],
                None,
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

