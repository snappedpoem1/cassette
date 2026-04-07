use cassette_core::db::Db;
use cassette_core::sources::{
    fetch_slskd_transfers, qobuz_user_auth_token, RemoteProviderConfig, SlskdConnectionConfig,
};
use serde_json::Value;
use std::path::PathBuf;
use tokio::process::Command;

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

async fn probe_slskd(db: &Db) -> ProbeResult {
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
            detail: format!("auth + transfers OK ({} transfer groups)", items.len()),
        },
        Err(error) => {
            let detail = if error.to_ascii_lowercase().contains("connection")
                || error.to_ascii_lowercase().contains("error sending request")
            {
                "daemon unavailable at configured URL (start app-owned slskd runtime first)"
                    .to_string()
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

    let token = match qobuz_user_auth_token(&provider_config).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return ProbeResult {
                provider: "qobuz",
                status: "SKIP",
                detail: "credentials/token missing".to_string(),
            }
        }
        Err(error) => {
            return ProbeResult {
                provider: "qobuz",
                status: "FAIL",
                detail: format!("auth failed: {error}"),
            }
        }
    };

    if token.is_empty() {
        return ProbeResult {
            provider: "qobuz",
            status: "FAIL",
            detail: "user_auth_token missing".to_string(),
        };
    }

    let client = reqwest::Client::new();
    let search = client
        .get("https://www.qobuz.com/api.json/0.2/catalog/search")
        .query(&[
            ("query", "Brand New"),
            ("limit", "1"),
            ("app_id", app_id),
            ("user_auth_token", token.as_str()),
        ])
        .send()
        .await;
    let Ok(search) = search else {
        return ProbeResult {
            provider: "qobuz",
            status: "FAIL",
            detail: "search request failed".to_string(),
        };
    };
    if !search.status().is_success() {
        return ProbeResult {
            provider: "qobuz",
            status: "FAIL",
            detail: format!("search HTTP {}", search.status()),
        };
    }
    let body = match search.json::<Value>().await {
        Ok(value) => value,
        Err(_) => {
            return ProbeResult {
                provider: "qobuz",
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
        provider: "qobuz",
        status: "OK",
        detail: format!("search OK ({albums} album result(s) in probe)"),
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
