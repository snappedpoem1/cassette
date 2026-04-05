pub mod album_resolver;
mod commands;
mod now_playing;
mod pending_recovery;
mod runtime_bootstrap;
mod spotify_history;
pub mod state;

use state::AppState;
use tauri::Manager;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("cassette.db");
            let app_handle = app.handle().clone();
            let app_state = tauri::async_runtime::block_on(async move {
                AppState::new(&db_path, Some(app_handle))
            }).map_err(|e| e.to_string())?;
            app.manage(app_state);
            #[cfg(desktop)]
            register_media_shortcuts(app).map_err(|e| e.to_string())?;
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
            commands::organize::organize_library,
            commands::organize::find_duplicates,
            commands::organize::resolve_duplicate,
            commands::organize::prune_missing_tracks,
            commands::organize::propose_tag_fixes,
            commands::organize::apply_tag_fixes,
            commands::organize::ingest_staging,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cassette");
}
