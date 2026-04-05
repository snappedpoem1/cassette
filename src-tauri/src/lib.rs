pub mod album_resolver;
mod commands;
mod now_playing;
mod pending_recovery;
mod runtime_bootstrap;
mod spotify_history;
pub mod state;

use state::AppState;
use tauri::Manager;

fn append_startup_log(message: &str) {
    use std::io::Write;

    let mut targets = Vec::new();
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        targets.push(
            std::path::PathBuf::from(local_app_data)
                .join("Cassette")
                .join("startup.log"),
        );
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        targets.push(
            std::path::PathBuf::from(app_data)
                .join("dev.cassette.app")
                .join("startup.log"),
        );
    }

    for target in targets {
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)
        {
            let _ = writeln!(file, "{}", message);
        }
    }
}

fn recover_sidecar_db(db_path: &std::path::Path) {
    let Some(parent) = db_path.parent() else {
        return;
    };

    let sidecar = parent.join("cassette_librarian.db");
    if !sidecar.exists() {
        return;
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let backup = parent.join(format!("cassette_librarian.startup_recovery_{stamp}.db"));
    if let Err(error) = std::fs::rename(&sidecar, &backup) {
        eprintln!("[startup] failed to backup librarian sidecar db: {error}");
        return;
    }

    let wal = parent.join("cassette_librarian.db-wal");
    let shm = parent.join("cassette_librarian.db-shm");
    let _ = std::fs::remove_file(wal);
    let _ = std::fs::remove_file(shm);

    eprintln!("[startup] sidecar db recovered to {}", backup.display());
}

#[cfg(desktop)]
fn register_tray_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show_item = MenuItemBuilder::with_id("show", "Show Cassette").build(app)?;
    let play_pause_item = MenuItemBuilder::with_id("play_pause", "Play/Pause").build(app)?;
    let next_item = MenuItemBuilder::with_id("next", "Next Track").build(app)?;
    let prev_item = MenuItemBuilder::with_id("prev", "Previous Track").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&play_pause_item)
        .item(&next_item)
        .item(&prev_item)
        .separator()
        .item(&quit_item)
        .build()?;

    TrayIconBuilder::with_id("cassette-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let state = app.state::<AppState>();
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "play_pause" => {
                    let _ = commands::player::player_toggle(state);
                }
                "next" => {
                    let _ = commands::player::player_next(state);
                }
                "prev" => {
                    let _ = commands::player::player_prev(state);
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(desktop)]
fn register_media_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_shortcuts(["MediaPlayPause", "MediaTrackNext", "MediaTrackPrevious"])?
            .with_handler(|app, shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

                let state = app.state::<AppState>();
                if shortcut.matches(Modifiers::empty(), Code::MediaPlayPause) {
                    let _ = commands::player::player_toggle(state);
                } else if shortcut.matches(Modifiers::empty(), Code::MediaTrackNext) {
                    let _ = commands::player::player_next(state);
                } else if shortcut.matches(Modifiers::empty(), Code::MediaTrackPrevious) {
                    let _ = commands::player::player_prev(state);
                }
            })
            .build(),
    )?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();
    std::panic::set_hook(Box::new(|panic_info| {
        let message = format!("[panic] {panic_info}");
        eprintln!("{message}");
        append_startup_log(&message);
    }));
    append_startup_log("[startup] run() entered");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("app data dir unavailable: {error}"))?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("cassette.db");
            let app_handle = app.handle().clone();
            let app_state = match tauri::async_runtime::block_on(async {
                AppState::new(&db_path, Some(app_handle.clone()))
            }) {
                Ok(state) => state,
                Err(first_error) => {
                    eprintln!("[startup] AppState initialization failed: {first_error}");
                    recover_sidecar_db(&db_path);
                    tauri::async_runtime::block_on(async {
                        AppState::new(&db_path, Some(app_handle))
                    })
                    .map_err(|retry_error| {
                        format!(
                            "failed to initialize app state after sidecar recovery: {retry_error}"
                        )
                    })?
                }
            };
            app.manage(app_state);
            #[cfg(desktop)]
            if let Err(error) = register_media_shortcuts(app) {
                eprintln!("[startup] failed to register media shortcuts: {error}");
            }
            #[cfg(desktop)]
            if let Err(error) = register_tray_menu(app) {
                eprintln!("[startup] failed to register tray menu: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library::get_library_roots,
            commands::library::add_library_root,
            commands::library::remove_library_root,
            commands::library::scan_library,
            commands::library::get_tracks,
            commands::library::search_tracks,
            commands::library::get_albums,
            commands::library::get_album_tracks,
            commands::library::get_artists,
            commands::library::get_track_count,
            commands::queue::get_queue,
            commands::queue::clear_queue,
            commands::queue::add_to_queue,
            commands::queue::queue_tracks,
            commands::player::player_load,
            commands::player::player_play,
            commands::player::player_pause,
            commands::player::player_stop,
            commands::player::player_toggle,
            commands::player::player_next,
            commands::player::player_prev,
            commands::player::player_set_volume,
            commands::player::player_seek,
            commands::player::get_playback_state,
            commands::player::get_now_playing_context,
            commands::player::sync_lastfm_history,
            commands::downloads::start_download,
            commands::downloads::start_song_download,
            commands::downloads::start_album_downloads,
            commands::downloads::start_discography_downloads,
            commands::downloads::start_artist_downloads,
            commands::downloads::cancel_download,
            commands::downloads::build_library_acquisition_queue,
            commands::downloads::start_spotify_missing_batch,
            commands::downloads::get_download_jobs,
            commands::downloads::search_download_metadata,
            commands::downloads::get_artist_discography,
            commands::downloads::get_slskd_transfers,
            commands::downloads::get_candidate_review,
            commands::downloads::get_task_provenance,
            commands::downloads::get_recent_task_results,
            commands::downloads::create_acquisition_request,
            commands::downloads::list_acquisition_requests,
            commands::downloads::get_acquisition_request_timeline,
            commands::downloads::get_request_candidate_review,
            commands::downloads::get_request_lineage,
            commands::downloads::start_backlog_run,
            commands::downloads::stop_backlog_run,
            commands::downloads::get_backlog_status,
            commands::downloads::get_director_debug_stats,
            commands::planner::plan_acquisition,
            commands::planner::get_candidate_set,
            commands::planner::get_request_rationale,
            commands::planner::approve_planned_request,
            commands::planner::reject_planned_request,
            commands::playlists::get_playlists,
            commands::playlists::get_playlist_items,
            commands::playlists::create_playlist,
            commands::playlists::replace_playlist_tracks,
            commands::playlists::delete_playlist,
            commands::playlists::play_playlist,
            commands::import::parse_spotify_history,
            commands::import::import_spotify_desired_tracks,
            commands::import::queue_spotify_albums,
            commands::import::get_spotify_import_status,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::get_config,
            commands::settings::get_provider_statuses,
            commands::settings::save_config,
            commands::settings::persist_effective_config,
            commands::organize::organize_library,
            commands::organize::find_duplicates,
            commands::organize::resolve_duplicate,
            commands::organize::prune_missing_tracks,
            commands::organize::propose_tag_fixes,
            commands::organize::apply_tag_fixes,
            commands::organize::ingest_staging,
        ]);

    if let Err(error) = app.run(tauri::generate_context!()) {
        let message = format!("[startup] tauri runtime failed: {error}");
        eprintln!("{message}");
        append_startup_log(&message);
    }
}
