#[allow(dead_code)]
#[path = "../slskd_runtime.rs"]
mod slskd_runtime;

use cassette_core::db::Db;
use cassette_core::provider_settings::DownloadConfig;
use serde::Serialize;
use slskd_runtime::SlskdRuntimeStatus;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct ProbeOutput {
    initial_status: SlskdRuntimeStatus,
    probe_status: SlskdRuntimeStatus,
    stopped_after_probe: bool,
    stop_error: Option<String>,
}

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|error| error.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn print_output(output: &ProbeOutput, emit_json: bool) {
    if emit_json {
        println!(
            "{}",
            serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }

    println!(
        "initial_ready={} probe_ready={} spawned_by_probe={} stopped_after_probe={} url={} message={}",
        output.initial_status.ready,
        output.probe_status.ready,
        output.probe_status.spawned_by_app,
        output.stopped_after_probe,
        output.probe_status.url,
        output
            .probe_status
            .message
            .clone()
            .unwrap_or_else(|| "none".to_string())
    );
    if let Some(error) = &output.stop_error {
        println!("stop_error={error}");
    }
}

fn main() -> Result<(), String> {
    let emit_json = std::env::args().any(|arg| arg == "--json");
    let leave_running = std::env::args().any(|arg| arg == "--leave-running");

    let db = Db::open(&app_db_path()?).map_err(|error| error.to_string())?;
    let download_config = DownloadConfig::from_env();
    let mut runtime = slskd_runtime::SlskdRuntimeManager::default();

    let initial_status = runtime.refresh_status(None, &db, &download_config);
    let probe_status = runtime.ensure_started(None, &db, &download_config);

    let mut stopped_after_probe = false;
    let mut stop_error = None;
    if probe_status.ready && probe_status.spawned_by_app && !leave_running {
        match runtime.stop() {
            Ok(()) => {
                stopped_after_probe = true;
            }
            Err(error) => {
                stop_error = Some(error.to_string());
            }
        }
    }

    let output = ProbeOutput {
        initial_status,
        probe_status,
        stopped_after_probe,
        stop_error,
    };
    print_output(&output, emit_json);

    if output.probe_status.ready {
        Ok(())
    } else {
        Err(output
            .probe_status
            .message
            .clone()
            .unwrap_or_else(|| "slskd runtime probe failed".to_string()))
    }
}
