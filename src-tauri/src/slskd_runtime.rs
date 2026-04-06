use cassette_core::{db::Db, provider_settings::DownloadConfig};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

const SLSKD_READY_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SlskdRuntimeStatus {
    pub running: bool,
    pub ready: bool,
    pub spawned_by_app: bool,
    pub binary_found: bool,
    pub binary_path: Option<String>,
    pub app_dir: Option<String>,
    pub downloads_dir: Option<String>,
    pub url: String,
    pub message: Option<String>,
}

pub struct SlskdRuntimeManager {
    child: Option<Child>,
    status: SlskdRuntimeStatus,
}

impl Default for SlskdRuntimeManager {
    fn default() -> Self {
        Self {
            child: None,
            status: SlskdRuntimeStatus {
                url: "http://localhost:5030".to_string(),
                ..SlskdRuntimeStatus::default()
            },
        }
    }
}

impl SlskdRuntimeManager {
    pub fn refresh_status(
        &mut self,
        app_handle: Option<&AppHandle>,
        db: &Db,
        download_config: &DownloadConfig,
    ) -> SlskdRuntimeStatus {
        let url = resolve_slskd_url(db, download_config);
        let app_dir = slskd_app_dir();
        let downloads_dir = resolve_slskd_downloads_dir(db, download_config);
        let binary_path = locate_slskd_binary(app_handle);
        let ready = slskd_endpoint_reachable(&url);
        let running = ready || self.child_is_alive();

        self.status = SlskdRuntimeStatus {
            running,
            ready,
            spawned_by_app: self.child.is_some(),
            binary_found: binary_path
                .as_ref()
                .map(|path| path.exists())
                .unwrap_or(false),
            binary_path: binary_path.map(|path| path.display().to_string()),
            app_dir: Some(app_dir.display().to_string()),
            downloads_dir: Some(downloads_dir.display().to_string()),
            url,
            message: if ready {
                Some("slskd endpoint reachable".to_string())
            } else if self.child.is_some() {
                Some("Cassette started slskd; waiting for endpoint".to_string())
            } else {
                Some("slskd is not reachable".to_string())
            },
        };

        self.status.clone()
    }

    pub fn ensure_started(
        &mut self,
        app_handle: Option<&AppHandle>,
        db: &Db,
        download_config: &DownloadConfig,
    ) -> SlskdRuntimeStatus {
        let status = self.refresh_status(app_handle, db, download_config);
        if status.ready {
            return status;
        }

        let Some(binary_path) = locate_slskd_binary(app_handle) else {
            self.status.message = Some("bundled slskd.exe was not found".to_string());
            return self.status.clone();
        };
        if !binary_path.exists() {
            self.status.message = Some(format!(
                "bundled slskd.exe was not found at {}",
                binary_path.display()
            ));
            return self.status.clone();
        }

        let app_dir = slskd_app_dir();
        let downloads_dir = resolve_slskd_downloads_dir(db, download_config);
        if let Err(error) = fs::create_dir_all(&app_dir) {
            self.status.message = Some(format!("failed to create slskd app dir: {error}"));
            return self.status.clone();
        }
        if let Err(error) = fs::create_dir_all(&downloads_dir) {
            self.status.message = Some(format!("failed to create slskd downloads dir: {error}"));
            return self.status.clone();
        }

        let url = resolve_slskd_url(db, download_config);
        let username = read_slskd_setting(db, "slskd_user")
            .or_else(|| download_config.slskd_user.clone())
            .unwrap_or_else(|| "slskd".to_string());
        let password = read_slskd_setting(db, "slskd_pass")
            .or_else(|| download_config.slskd_pass.clone())
            .unwrap_or_else(|| "slskd".to_string());
        let slsk_username = read_slskd_setting(db, "soulseek_username");
        let slsk_password = read_slskd_setting(db, "soulseek_password");

        let log_dir = cassette_log_dir();
        let _ = fs::create_dir_all(&log_dir);
        let stdout = open_log_file(&log_dir.join("slskd.stdout.log")).ok();
        let stderr = open_log_file(&log_dir.join("slskd.stderr.log")).ok();

        let mut command = Command::new(&binary_path);
        command
            .arg("--headless")
            .arg("--no-logo")
            .arg("--app-dir")
            .arg(&app_dir)
            .arg("--downloads")
            .arg(&downloads_dir)
            .arg("--http-port")
            .arg(resolve_slskd_port(&url).to_string())
            .arg("--no-https")
            .arg("--username")
            .arg(&username)
            .arg("--password")
            .arg(&password);

        if let Some(value) = slsk_username.filter(|value| !value.trim().is_empty()) {
            command.arg("--slsk-username").arg(value);
        }
        if let Some(value) = slsk_password.filter(|value| !value.trim().is_empty()) {
            command.arg("--slsk-password").arg(value);
        }
        if let Some(file) = stdout {
            command.stdout(Stdio::from(file));
        }
        if let Some(file) = stderr {
            command.stderr(Stdio::from(file));
        }

        match command.spawn() {
            Ok(child) => {
                info!(
                    path = %binary_path.display(),
                    app_dir = %app_dir.display(),
                    downloads_dir = %downloads_dir.display(),
                    "started bundled slskd runtime"
                );
                self.child = Some(child);
            }
            Err(error) => {
                self.status = SlskdRuntimeStatus {
                    running: false,
                    ready: false,
                    spawned_by_app: false,
                    binary_found: true,
                    binary_path: Some(binary_path.display().to_string()),
                    app_dir: Some(app_dir.display().to_string()),
                    downloads_dir: Some(downloads_dir.display().to_string()),
                    url,
                    message: Some(format!("failed to start bundled slskd: {error}")),
                };
                return self.status.clone();
            }
        }

        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(SLSKD_READY_TIMEOUT_SECS) {
            if slskd_endpoint_reachable(&url) {
                self.status = SlskdRuntimeStatus {
                    running: true,
                    ready: true,
                    spawned_by_app: true,
                    binary_found: true,
                    binary_path: Some(binary_path.display().to_string()),
                    app_dir: Some(app_dir.display().to_string()),
                    downloads_dir: Some(downloads_dir.display().to_string()),
                    url,
                    message: Some("Cassette started bundled slskd".to_string()),
                };
                return self.status.clone();
            }

            if let Some(child) = self.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(exit)) => {
                        self.child = None;
                        self.status = SlskdRuntimeStatus {
                            running: false,
                            ready: false,
                            spawned_by_app: false,
                            binary_found: true,
                            binary_path: Some(binary_path.display().to_string()),
                            app_dir: Some(app_dir.display().to_string()),
                            downloads_dir: Some(downloads_dir.display().to_string()),
                            url,
                            message: Some(format!("bundled slskd exited early: {exit}")),
                        };
                        return self.status.clone();
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!(error = %error, "failed to poll slskd child process");
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(500));
        }

        self.status = SlskdRuntimeStatus {
            running: self.child_is_alive(),
            ready: false,
            spawned_by_app: self.child.is_some(),
            binary_found: true,
            binary_path: Some(binary_path.display().to_string()),
            app_dir: Some(app_dir.display().to_string()),
            downloads_dir: Some(downloads_dir.display().to_string()),
            url,
            message: Some(
                "bundled slskd started but did not become reachable before timeout".to_string(),
            ),
        };
        self.status.clone()
    }

    pub fn restart(
        &mut self,
        app_handle: Option<&AppHandle>,
        db: &Db,
        download_config: &DownloadConfig,
    ) -> SlskdRuntimeStatus {
        let _ = self.stop();
        self.ensure_started(app_handle, db, download_config)
    }

    pub fn stop(&mut self) -> io::Result<()> {
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            let _ = child.wait();
        }
        self.status.running = false;
        self.status.ready = false;
        self.status.spawned_by_app = false;
        self.status.message = Some("bundled slskd stopped".to_string());
        Ok(())
    }

    fn child_is_alive(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };

        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                self.child = None;
                false
            }
            Err(_) => false,
        }
    }
}

impl Drop for SlskdRuntimeManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn read_slskd_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_slskd_url(db: &Db, download_config: &DownloadConfig) -> String {
    read_slskd_setting(db, "slskd_url")
        .or_else(|| download_config.slskd_url.clone())
        .unwrap_or_else(|| "http://localhost:5030".to_string())
}

fn resolve_slskd_downloads_dir(db: &Db, download_config: &DownloadConfig) -> PathBuf {
    read_slskd_setting(db, "slskd_downloads_dir")
        .or_else(|| download_config.slskd_downloads_dir.clone())
        .map(PathBuf::from)
        .unwrap_or_else(|| slskd_app_dir().join("downloads"))
}

fn resolve_slskd_port(url: &str) -> u16 {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.port_or_known_default())
        .unwrap_or(5030)
}

fn locate_slskd_binary(app_handle: Option<&AppHandle>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app_handle) = app_handle {
        if let Ok(resource_dir) = app_handle.path().resource_dir() {
            candidates.extend(bundled_slskd_binary_candidates(&resource_dir));
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.extend(bundled_slskd_binary_candidates(&current_dir));
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.extend(bundled_slskd_binary_candidates(parent));
            if let Some(grandparent) = parent.parent() {
                candidates.extend(bundled_slskd_binary_candidates(grandparent));
            }
        }
    }

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn bundled_slskd_binary_candidates(base: &Path) -> Vec<PathBuf> {
    vec![
        base.join("binaries").join("slskd").join("slskd.exe"),
        base.join("slskd").join("slskd.exe"),
        base.join("slskd.exe"),
    ]
}

fn slskd_app_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("slskd")
}

fn cassette_log_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Cassette")
        .join("logs")
}

fn open_log_file(path: &Path) -> io::Result<std::fs::File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn slskd_endpoint_reachable(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let port = parsed.port_or_known_default().unwrap_or(5030);
    let Ok(mut addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    addrs.any(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(750)).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{bundled_slskd_binary_candidates, resolve_slskd_port};
    use std::path::Path;

    #[test]
    fn bundled_binary_candidates_cover_repo_and_resource_layouts() {
        let base = Path::new("C:\\Cassette Music");
        let candidates = bundled_slskd_binary_candidates(base);
        assert_eq!(
            candidates[0],
            Path::new("C:\\Cassette Music\\binaries\\slskd\\slskd.exe")
        );
        assert_eq!(
            candidates[1],
            Path::new("C:\\Cassette Music\\slskd\\slskd.exe")
        );
        assert_eq!(candidates[2], Path::new("C:\\Cassette Music\\slskd.exe"));
    }

    #[test]
    fn slskd_port_defaults_to_standard_http_port() {
        assert_eq!(resolve_slskd_port("http://localhost:5030"), 5030);
        assert_eq!(resolve_slskd_port("http://127.0.0.1"), 80);
        assert_eq!(resolve_slskd_port("not-a-url"), 5030);
    }
}
