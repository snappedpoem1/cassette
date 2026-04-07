use cassette_core::db::Db;
use cassette_core::sources::{
    fetch_slskd_transfers, RemoteProviderConfig, SlskdConnectionConfig,
};
use cassette_core::provider_settings::DownloadConfig;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;

#[allow(dead_code)]
#[path = "../slskd_runtime.rs"]
mod slskd_runtime;

#[derive(Debug)]
struct ProbeResult {
    provider: &'static str,
    status: &'static str,
    detail: String,
}

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|error| error.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn present(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn slskd_local_config_credentials() -> Option<(String, String)> {
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    let path = PathBuf::from(local_app_data).join("slskd").join("slskd.yml");
    let raw = fs::read_to_string(path).ok()?;

    let mut in_web = false;
    let mut in_auth = false;
    let mut username = None;
    let mut password = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let is_top_level = !line.starts_with(' ') && trimmed.ends_with(':');
        if is_top_level {
            in_web = trimmed == "web:";
            in_auth = false;
            continue;
        }

        if in_web && trimmed == "authentication:" {
            in_auth = true;
            continue;
        }

        if in_auth {
            if let Some(value) = trimmed.strip_prefix("username:") {
                username = Some(value.trim().to_string());
                continue;
            }
            if let Some(value) = trimmed.strip_prefix("password:") {
                password = Some(value.trim().to_string());
                continue;
            }
            if !line.starts_with("    ") {
                in_auth = false;
            }
        }
    }

    match (username, password) {
        (Some(user), Some(pass)) if !user.is_empty() && !pass.is_empty() => Some((user, pass)),
        _ => None,
    }
}

async fn probe_slskd(db: &Db) -> ProbeResult {
    let download_config = DownloadConfig::from_env();
    let mut runtime = slskd_runtime::SlskdRuntimeManager::default();
    let runtime_status = runtime.ensure_started(None, db, &download_config);
    if !runtime_status.ready {
        return ProbeResult {
            provider: "slskd",
            status: "FAIL",
            detail: runtime_status
                .message
                .unwrap_or_else(|| "managed slskd runtime failed to start".to_string()),
        };
    }

    let config = SlskdConnectionConfig {
        url: present(db.get_setting("slskd_url").ok().flatten())
            .unwrap_or_else(|| "http://localhost:5030".to_string()),
        username: present(db.get_setting("slskd_user").ok().flatten())
            .unwrap_or_else(|| "slskd".to_string()),
        password: present(db.get_setting("slskd_pass").ok().flatten())
            .unwrap_or_else(|| "slskd".to_string()),
        api_key: present(db.get_setting("slskd_api_key").ok().flatten()),
    };

    match fetch_slskd_transfers(&config).await {
        Ok(items) => ProbeResult {
            provider: "slskd",
            status: "OK",
            detail: format!(
                "managed runtime + auth + transfers OK ({} transfer groups)",
                items.len()
            ),
        },
        Err(error) => {
            if error.contains("HTTP 403") && !runtime_status.spawned_by_app {
                if let Some((fallback_user, fallback_pass)) = slskd_local_config_credentials() {
                    let fallback_config = SlskdConnectionConfig {
                        username: fallback_user.clone(),
                        password: fallback_pass.clone(),
                        ..config.clone()
                    };
                    if let Ok(items) = fetch_slskd_transfers(&fallback_config).await {
                        let _ = db.set_setting("slskd_user", &fallback_user);
                        let _ = db.set_setting("slskd_pass", &fallback_pass);
                        return ProbeResult {
                            provider: "slskd",
                            status: "OK",
                            detail: format!(
                                "auth repaired from local slskd.yml and transfers verified ({} transfer groups)",
                                items.len()
                            ),
                        };
                    }
                }
            }

            let detail = if error.to_ascii_lowercase().contains("connection")
                || error.to_ascii_lowercase().contains("error sending request")
            {
                "daemon unavailable at configured URL (start app-owned slskd runtime first)"
                    .to_string()
            } else if error.contains("HTTP 403") {
                "daemon reachable but credentials rejected (HTTP 403)".to_string()
            } else {
                error
            };
            ProbeResult {
                provider: "slskd",
                status: "FAIL",
                detail,
            }
        }
    }
}

async fn qobuz_login_token(app_id: &str, email: &str, password: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let login = client
        .post("https://www.qobuz.com/api.json/0.2/user/login")
        .form(&[("email", email), ("password", password), ("app_id", app_id)])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !login.status().is_success() {
        return Err(format!("HTTP {}", login.status()));
    }

    let body = login
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    let token = body
        .get("user_auth_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if token.is_empty() {
        return Err("user_auth_token missing from login response".to_string());
    }
    Ok(token)
}

async fn qobuz_search_with_token(app_id: &str, token: &str) -> Result<usize, String> {
    let client = reqwest::Client::new();
    let search = client
        .get("https://www.qobuz.com/api.json/0.2/catalog/search")
        .query(&[
            ("query", "Brand New"),
            ("limit", "1"),
            ("app_id", app_id),
            ("user_auth_token", token),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !search.status().is_success() {
        return Err(format!("HTTP {}", search.status()));
    }

    let body = search
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    Ok(body
        .pointer("/albums/items")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0))
}

async fn probe_qobuz(db: &Db) -> ProbeResult {
    let provider_config = RemoteProviderConfig {
        qobuz_email: present(db.get_setting("qobuz_email").ok().flatten()),
        qobuz_password: present(db.get_setting("qobuz_password").ok().flatten()),
        qobuz_password_hash: present(db.get_setting("qobuz_password_hash").ok().flatten()),
        qobuz_app_id: present(db.get_setting("qobuz_app_id").ok().flatten()),
        qobuz_app_secret: present(db.get_setting("qobuz_app_secret").ok().flatten()),
        qobuz_user_auth_token: present(db.get_setting("qobuz_user_auth_token").ok().flatten()),
        qobuz_secrets: present(db.get_setting("qobuz_secrets").ok().flatten()),
        ..RemoteProviderConfig::default()
    };

    let Some(app_id) = provider_config.qobuz_app_id.as_deref() else {
        return ProbeResult {
            provider: "qobuz",
            status: "SKIP",
            detail: "qobuz_app_id missing".to_string(),
        };
    };

    if let Some(existing_token) = provider_config.qobuz_user_auth_token.as_deref() {
        match qobuz_search_with_token(app_id, existing_token).await {
            Ok(albums) => {
                return ProbeResult {
                    provider: "qobuz",
                    status: "OK",
                    detail: format!(
                        "search OK with existing token ({albums} album result(s) in probe)"
                    ),
                }
            }
            Err(error) => {
                if !error.contains("401") && !error.contains("403") {
                    return ProbeResult {
                        provider: "qobuz",
                        status: "FAIL",
                        detail: format!("search failed with stored token: {error}"),
                    };
                }
            }
        }
    }

    let Some(email) = provider_config.qobuz_email.as_deref() else {
        return ProbeResult {
            provider: "qobuz",
            status: "FAIL",
            detail: "stored token invalid and qobuz_email missing for refresh".to_string(),
        };
    };

    let mut attempted_modes = Vec::new();
    let mut passwords = Vec::new();
    if let Some(value) = provider_config.qobuz_password.as_deref() {
        passwords.push(("plain", value));
    }
    if let Some(value) = provider_config.qobuz_password_hash.as_deref() {
        passwords.push(("hash", value));
    }

    if passwords.is_empty() {
        return ProbeResult {
            provider: "qobuz",
            status: "FAIL",
            detail: "stored token invalid and no qobuz_password/qobuz_password_hash available"
                .to_string(),
        };
    }

    for (mode, password) in passwords {
        attempted_modes.push(mode);
        let token = match qobuz_login_token(app_id, email, password).await {
            Ok(token) => token,
            Err(_) => continue,
        };

        match qobuz_search_with_token(app_id, &token).await {
            Ok(albums) => {
                if let Err(error) = db.set_setting("qobuz_user_auth_token", &token) {
                    return ProbeResult {
                        provider: "qobuz",
                        status: "FAIL",
                        detail: format!(
                            "refreshed token via {mode} credentials, but failed to persist token: {error}"
                        ),
                    };
                }
                return ProbeResult {
                    provider: "qobuz",
                    status: "OK",
                    detail: format!(
                        "search OK after token refresh via {mode} credentials ({albums} album result(s) in probe)"
                    ),
                };
            }
            Err(_) => continue,
        }
    }

    let _ = db.set_setting("qobuz_user_auth_token", "");
    ProbeResult {
        provider: "qobuz",
        status: "FAIL",
        detail: format!(
            "auth refresh failed after retrying {} credential mode(s); stale token cleared",
            attempted_modes.len()
        ),
    }
}

async fn probe_deezer(db: &Db) -> ProbeResult {
    let arl = present(db.get_setting("deezer_arl").ok().flatten());
    let Some(arl) = arl else {
        return ProbeResult {
            provider: "deezer",
            status: "SKIP",
            detail: "deezer_arl missing".to_string(),
        };
    };

    let mut headers = reqwest::header::HeaderMap::new();
    let cookie = format!("arl={arl}");
    let header_value = match reqwest::header::HeaderValue::from_str(&cookie) {
        Ok(value) => value,
        Err(_) => {
            return ProbeResult {
                provider: "deezer",
                status: "FAIL",
                detail: "invalid cookie header".to_string(),
            }
        }
    };
    headers.insert(reqwest::header::COOKIE, header_value);

    let client = match reqwest::Client::builder().default_headers(headers).build() {
        Ok(client) => client,
        Err(_) => {
            return ProbeResult {
                provider: "deezer",
                status: "FAIL",
                detail: "client build failed".to_string(),
            }
        }
    };

    let search = client
        .get("https://api.deezer.com/search/album")
        .query(&[("q", "Brand New"), ("limit", "1")])
        .send()
        .await;
    let Ok(search) = search else {
        return ProbeResult {
            provider: "deezer",
            status: "FAIL",
            detail: "search request failed".to_string(),
        };
    };
    if !search.status().is_success() {
        return ProbeResult {
            provider: "deezer",
            status: "FAIL",
            detail: format!("search HTTP {}", search.status()),
        };
    }
    let body = match search.json::<Value>().await {
        Ok(value) => value,
        Err(_) => {
            return ProbeResult {
                provider: "deezer",
                status: "FAIL",
                detail: "search JSON parse failed".to_string(),
            }
        }
    };
    let albums = body
        .get("data")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    ProbeResult {
        provider: "deezer",
        status: "OK",
        detail: format!("search OK ({albums} album result(s) in probe)"),
    }
}

async fn probe_spotify(db: &Db) -> ProbeResult {
    let access_token = present(db.get_setting("spotify_access_token").ok().flatten());
    let client_id = present(db.get_setting("spotify_client_id").ok().flatten());
    let client_secret = present(db.get_setting("spotify_client_secret").ok().flatten());

    let client = reqwest::Client::new();
    let token = if let Some(token) = access_token {
        token
    } else {
        let (Some(client_id), Some(client_secret)) = (client_id, client_secret) else {
            return ProbeResult {
                provider: "spotify",
                status: "SKIP",
                detail: "token and client credentials missing".to_string(),
            };
        };
        let token_resp = client
            .post("https://accounts.spotify.com/api/token")
            .basic_auth(client_id, Some(client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await;
        let Ok(token_resp) = token_resp else {
            return ProbeResult {
                provider: "spotify",
                status: "FAIL",
                detail: "token request failed".to_string(),
            };
        };
        if !token_resp.status().is_success() {
            return ProbeResult {
                provider: "spotify",
                status: "FAIL",
                detail: format!("token HTTP {}", token_resp.status()),
            };
        }
        let body = match token_resp.json::<Value>().await {
            Ok(value) => value,
            Err(_) => {
                return ProbeResult {
                    provider: "spotify",
                    status: "FAIL",
                    detail: "token JSON parse failed".to_string(),
                }
            }
        };
        body.get("access_token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };

    if token.is_empty() {
        return ProbeResult {
            provider: "spotify",
            status: "FAIL",
            detail: "access token missing".to_string(),
        };
    }

    let search = client
        .get("https://api.spotify.com/v1/search")
        .bearer_auth(token)
        .query(&[("q", "Brand New"), ("type", "album"), ("limit", "1")])
        .send()
        .await;
    let Ok(search) = search else {
        return ProbeResult {
            provider: "spotify",
            status: "FAIL",
            detail: "search request failed".to_string(),
        };
    };
    if !search.status().is_success() {
        return ProbeResult {
            provider: "spotify",
            status: "FAIL",
            detail: format!("search HTTP {}", search.status()),
        };
    }
    let body = match search.json::<Value>().await {
        Ok(value) => value,
        Err(_) => {
            return ProbeResult {
                provider: "spotify",
                status: "FAIL",
                detail: "search JSON parse failed".to_string(),
            }
        }
    };
    let albums = body
        .pointer("/albums/items")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    ProbeResult {
        provider: "spotify",
        status: "OK",
        detail: format!("search OK ({albums} album result(s) in probe)"),
    }
}

async fn probe_usenet(db: &Db) -> ProbeResult {
    let api_key = present(db.get_setting("nzbgeek_api_key").ok().flatten());
    let usenet_host = present(db.get_setting("usenet_host").ok().flatten());
    let (Some(api_key), Some(_usenet_host)) = (api_key, usenet_host) else {
        return ProbeResult {
            provider: "usenet",
            status: "SKIP",
            detail: "nzbgeek_api_key and/or usenet_host missing".to_string(),
        };
    };

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.nzbgeek.info/api")
        .query(&[
            ("t", "search"),
            ("cat", "3000"),
            ("q", "Brand New"),
            ("apikey", api_key.as_str()),
            ("o", "json"),
            ("limit", "1"),
        ])
        .send()
        .await;
    let Ok(resp) = resp else {
        return ProbeResult {
            provider: "usenet",
            status: "FAIL",
            detail: "search request failed".to_string(),
        };
    };
    if !resp.status().is_success() {
        return ProbeResult {
            provider: "usenet",
            status: "FAIL",
            detail: format!("search HTTP {}", resp.status()),
        };
    }
    ProbeResult {
        provider: "usenet",
        status: "OK",
        detail: "nzbgeek search endpoint reachable".to_string(),
    }
}

async fn probe_ytdlp() -> ProbeResult {
    let output = Command::new("yt-dlp").arg("--version").output().await;
    let Ok(output) = output else {
        return ProbeResult {
            provider: "yt-dlp",
            status: "SKIP",
            detail: "yt-dlp not installed/in PATH".to_string(),
        };
    };
    if !output.status.success() {
        return ProbeResult {
            provider: "yt-dlp",
            status: "FAIL",
            detail: format!("exit code {:?}", output.status.code()),
        };
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    ProbeResult {
        provider: "yt-dlp",
        status: "OK",
        detail: format!("installed ({version})"),
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let db = Db::open(&app_db_path()?).map_err(|error| error.to_string())?;

    let results = vec![
        probe_slskd(&db).await,
        probe_qobuz(&db).await,
        probe_deezer(&db).await,
        probe_spotify(&db).await,
        probe_usenet(&db).await,
        probe_ytdlp().await,
    ];

    println!("{:<10} {:<6} detail", "provider", "status");
    for result in results {
        println!(
            "{:<10} {:<6} {}",
            result.provider, result.status, result.detail
        );
    }

    Ok(())
}
