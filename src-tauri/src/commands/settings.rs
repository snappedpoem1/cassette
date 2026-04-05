use crate::state::AppState;
use cassette_core::db::Db;
use cassette_core::provider_settings::{DownloadConfig, ProviderStatus};
use tauri::State;

const MASKED_SECRET: &str = "********";

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Option<String> {
    state.db.lock().unwrap().get_setting(&key).unwrap_or(None)
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) {
    let _ = state.db.lock().unwrap().set_setting(&key, &value);
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> DownloadConfig {
    let mut config = load_config(&state);
    mask_secret_fields(&mut config);
    config
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, config: DownloadConfig) {
    let db = state.db.lock().unwrap();
    persist_config(&db, &config, true);
}

#[tauri::command]
pub fn persist_effective_config(state: State<'_, AppState>) {
    let effective = load_config(&state);
    let db = state.db.lock().unwrap();
    persist_config(&db, &effective, false);
}

#[tauri::command]
pub fn get_provider_statuses(state: State<'_, AppState>) -> Vec<ProviderStatus> {
    load_config(&state).provider_statuses()
}

fn load_config(state: &AppState) -> DownloadConfig {
    let db = state.db.lock().unwrap();
    let mut config = state.download_config.clone();

    if let Ok(Some(value)) = db.get_setting("library_base") {
        config.library_base = value;
    }
    if let Ok(Some(value)) = db.get_setting("staging_folder") {
        config.staging_folder = value;
    }

    config.slskd_url = read_optional_setting(&db, "slskd_url", config.slskd_url);
    config.slskd_user = read_optional_setting(&db, "slskd_user", config.slskd_user);
    config.slskd_pass = read_optional_setting(&db, "slskd_pass", config.slskd_pass);

    config.real_debrid_key =
        read_optional_setting(&db, "real_debrid_key", config.real_debrid_key);

    config.qobuz_email = read_optional_setting(&db, "qobuz_email", config.qobuz_email);
    config.qobuz_password =
        read_optional_setting(&db, "qobuz_password", config.qobuz_password);

    config.deezer_arl = read_optional_setting(&db, "deezer_arl", config.deezer_arl);

    config.spotify_client_id =
        read_optional_setting(&db, "spotify_client_id", config.spotify_client_id);
    config.spotify_client_secret = read_optional_setting(
        &db,
        "spotify_client_secret",
        config.spotify_client_secret,
    );
    config.spotify_access_token = read_optional_setting(
        &db,
        "spotify_access_token",
        config.spotify_access_token,
    );
    config.genius_token = read_optional_setting(&db, "genius_token", config.genius_token);

    config.jackett_url = read_optional_setting(&db, "jackett_url", config.jackett_url);
    config.jackett_api_key = read_optional_setting(&db, "jackett_api_key", config.jackett_api_key);

    config.slskd_downloads_dir =
        read_optional_setting(&db, "slskd_downloads_dir", config.slskd_downloads_dir);

    config.nzbgeek_api_key =
        read_optional_setting(&db, "nzbgeek_api_key", config.nzbgeek_api_key);
    config.sabnzbd_url = read_optional_setting(&db, "sabnzbd_url", config.sabnzbd_url);
    config.sabnzbd_api_key =
        read_optional_setting(&db, "sabnzbd_api_key", config.sabnzbd_api_key);

    config.discogs_token = read_optional_setting(&db, "discogs_token", config.discogs_token);
    config.lastfm_api_key = read_optional_setting(&db, "lastfm_api_key", config.lastfm_api_key);
    config.lastfm_username =
        read_optional_setting(&db, "lastfm_username", config.lastfm_username);

    config.ytdlp_path = read_optional_setting(&db, "ytdlp_path", config.ytdlp_path);
    config.sevenzip_path = read_optional_setting(&db, "sevenzip_path", config.sevenzip_path);

    config
}

fn read_optional_setting(db: &Db, key: &str, current: Option<String>) -> Option<String> {
    db.get_setting(key).ok().flatten().or(current)
}

fn mask_secret_fields(config: &mut DownloadConfig) {
    config.slskd_pass = config.slskd_pass.as_ref().map(|_| MASKED_SECRET.to_string());
    config.real_debrid_key = config
        .real_debrid_key
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.qobuz_password = config
        .qobuz_password
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.deezer_arl = config.deezer_arl.as_ref().map(|_| MASKED_SECRET.to_string());
    config.spotify_client_secret = config
        .spotify_client_secret
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.spotify_access_token = config
        .spotify_access_token
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.genius_token = config.genius_token.as_ref().map(|_| MASKED_SECRET.to_string());
    config.jackett_api_key = config
        .jackett_api_key
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.nzbgeek_api_key = config
        .nzbgeek_api_key
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.sabnzbd_api_key = config
        .sabnzbd_api_key
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.discogs_token = config
        .discogs_token
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
    config.lastfm_api_key = config
        .lastfm_api_key
        .as_ref()
        .map(|_| MASKED_SECRET.to_string());
}

fn persist_string_setting(db: &Db, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        let _ = db.delete_setting(key);
        return;
    }

    let _ = db.set_setting(key, trimmed);
}

fn persist_optional_setting(db: &Db, key: &str, value: &Option<String>, preserve_mask: bool) {
    let Some(trimmed) = value
        .as_ref()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
    else {
        let _ = db.delete_setting(key);
        return;
    };

    if preserve_mask && trimmed == MASKED_SECRET {
        return;
    }

    let _ = db.set_setting(key, trimmed);
}

fn persist_config(db: &Db, config: &DownloadConfig, preserve_mask: bool) {
    persist_string_setting(db, "library_base", &config.library_base);
    persist_string_setting(db, "staging_folder", &config.staging_folder);

    persist_optional_setting(db, "slskd_url", &config.slskd_url, false);
    persist_optional_setting(db, "slskd_user", &config.slskd_user, false);
    persist_optional_setting(db, "slskd_pass", &config.slskd_pass, preserve_mask);

    persist_optional_setting(db, "real_debrid_key", &config.real_debrid_key, preserve_mask);

    persist_optional_setting(db, "qobuz_email", &config.qobuz_email, false);
    persist_optional_setting(db, "qobuz_password", &config.qobuz_password, preserve_mask);

    persist_optional_setting(db, "deezer_arl", &config.deezer_arl, preserve_mask);

    persist_optional_setting(db, "spotify_client_id", &config.spotify_client_id, false);
    persist_optional_setting(
        db,
        "spotify_client_secret",
        &config.spotify_client_secret,
        preserve_mask,
    );
    persist_optional_setting(
        db,
        "spotify_access_token",
        &config.spotify_access_token,
        preserve_mask,
    );
    persist_optional_setting(db, "genius_token", &config.genius_token, preserve_mask);

    persist_optional_setting(db, "jackett_url", &config.jackett_url, false);
    persist_optional_setting(db, "jackett_api_key", &config.jackett_api_key, preserve_mask);

    persist_optional_setting(db, "slskd_downloads_dir", &config.slskd_downloads_dir, false);

    persist_optional_setting(db, "nzbgeek_api_key", &config.nzbgeek_api_key, preserve_mask);
    persist_optional_setting(db, "sabnzbd_url", &config.sabnzbd_url, false);
    persist_optional_setting(db, "sabnzbd_api_key", &config.sabnzbd_api_key, preserve_mask);

    persist_optional_setting(db, "discogs_token", &config.discogs_token, preserve_mask);
    persist_optional_setting(db, "lastfm_api_key", &config.lastfm_api_key, preserve_mask);
    persist_optional_setting(db, "lastfm_username", &config.lastfm_username, false);

    persist_optional_setting(db, "ytdlp_path", &config.ytdlp_path, false);
    persist_optional_setting(db, "sevenzip_path", &config.sevenzip_path, false);
}
