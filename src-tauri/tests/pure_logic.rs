#![allow(dead_code)]

#[path = "../src/album_resolver.rs"]
mod album_resolver;
#[path = "../src/now_playing.rs"]
mod now_playing;
#[path = "../src/pending_recovery.rs"]
mod pending_recovery;
#[path = "../src/runtime_bootstrap.rs"]
mod runtime_bootstrap;
#[path = "../src/spotify_history.rs"]
mod spotify_history;
#[path = "../src/trust_ledger.rs"]
mod trust_ledger;

use cassette_core::db::{PendingDirectorTask, TerminalDirectorTaskUpdate};
use cassette_core::director::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};
use cassette_core::librarian::models::AcquisitionRequestRow;
use std::collections::HashMap;

fn pending_task(task_id: &str, progress: &str, updated_at: &str) -> PendingDirectorTask {
    PendingDirectorTask {
        task: TrackTask {
            task_id: task_id.to_string(),
            source: TrackTaskSource::Manual,
            desired_track_id: None,
            source_operation_id: None,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_album_id: None,
                source_artist_id: None,
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(35.0),
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_group_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy: AcquisitionStrategy::ObscureFallbackHeavy,
        },
        strategy: "ObscureFallbackHeavy".to_string(),
        progress: progress.to_string(),
        created_at: updated_at.to_string(),
        updated_at: updated_at.to_string(),
    }
}

#[test]
fn pending_recovery_plan_keeps_newer_retry_and_drops_stale_terminal_row() {
    let pending = vec![
        pending_task("stale-failed", "Queued", "2026-03-27 12:00:00"),
        pending_task("retry-failed", "Queued", "2026-03-27 12:00:03"),
        pending_task("terminal-progress", "Cancelled", "2026-03-27 12:00:04"),
    ];
    let terminal_updates = HashMap::from([
        (
            "stale-failed".to_string(),
            TerminalDirectorTaskUpdate {
                disposition: "Failed".to_string(),
                updated_at: "2026-03-27 12:00:02".to_string(),
            },
        ),
        (
            "retry-failed".to_string(),
            TerminalDirectorTaskUpdate {
                disposition: "Failed".to_string(),
                updated_at: "2026-03-27 12:00:01".to_string(),
            },
        ),
    ]);

    let plan = pending_recovery::build_pending_recovery_plan(pending, &terminal_updates);

    assert_eq!(
        plan.resumable_tasks
            .iter()
            .map(|task| task.task.task_id.as_str())
            .collect::<Vec<_>>(),
        vec!["retry-failed"]
    );
    assert_eq!(
        plan.stale_task_ids,
        vec!["stale-failed".to_string(), "terminal-progress".to_string()]
    );
}

#[test]
fn spotify_history_parser_summarizes_and_sorts() {
    let dir =
        std::env::temp_dir().join(format!("cassette-spotify-history-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    let file = dir.join("Streaming_History_Audio_2026.json");
    let payload = r#"
[
  {
    "master_metadata_album_artist_name": "The National",
    "master_metadata_album_album_name": "Boxer",
        "master_metadata_track_name": "Fake Empire",
    "ms_played": 200000,
    "skipped": false
  },
  {
    "master_metadata_album_artist_name": "The National",
    "master_metadata_album_album_name": "Boxer",
        "master_metadata_track_name": "Mistaken for Strangers",
    "ms_played": 100000,
    "skipped": true
  },
  {
    "master_metadata_album_artist_name": "Interpol",
    "master_metadata_album_album_name": "Turn on the Bright Lights",
        "master_metadata_track_name": "Untitled",
    "ms_played": 250000,
    "skipped": false
  }
]
"#;
    std::fs::write(&file, payload).expect("fixture should be written");

    let files = spotify_history::collect_spotify_history_files(&dir)
        .expect("history files should be found");
    assert_eq!(files.len(), 1);
    let entries = spotify_history::parse_spotify_entries(&files).expect("entries should parse");
    assert_eq!(entries.len(), 3);

    let mut library_track_counts = HashMap::new();
    library_track_counts.insert(("the national".to_string(), "boxer".to_string()), 2usize);
    let albums = spotify_history::summarize_spotify_albums(&entries, &library_track_counts);

    assert_eq!(albums.len(), 2);
    assert_eq!(albums[0].artist, "The National");
    assert_eq!(albums[0].album, "Boxer");
    assert_eq!(albums[0].total_ms, 300000);
    assert_eq!(albums[0].play_count, 1);
    assert_eq!(albums[0].skip_count, 1);
    assert!(albums[0].in_library);

    assert_eq!(albums[1].artist, "Interpol");
    assert_eq!(albums[1].play_count, 1);
    assert_eq!(albums[1].skip_count, 0);
    assert!(!albums[1].in_library);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spotify_import_pipeline_persists_to_db() {
    let dir =
        std::env::temp_dir().join(format!("cassette-spotify-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    let file = dir.join("Streaming_History_Audio_2026.json");
    let payload = r#"
[
  {
    "master_metadata_album_artist_name": "Frightened Rabbit",
    "master_metadata_album_album_name": "The Midnight Organ Fight",
        "master_metadata_track_name": "The Modern Leper",
    "ms_played": 123000,
    "skipped": false
  },
  {
    "master_metadata_album_artist_name": "Frightened Rabbit",
    "master_metadata_album_album_name": "The Midnight Organ Fight",
        "master_metadata_track_name": "The Twist",
    "ms_played": 110000,
    "skipped": true
  }
]
"#;
    std::fs::write(&file, payload).expect("fixture should be written");

    let db_path = dir.join("cassette-test.db");
    let db = cassette_core::db::Db::open(&db_path).expect("db should open");

    let files = spotify_history::collect_spotify_history_files(&dir)
        .expect("history files should be found");
    let entries = spotify_history::parse_spotify_entries(&files).expect("entries should parse");
    let library_track_counts = HashMap::new();
    let albums = spotify_history::summarize_spotify_albums(&entries, &library_track_counts);

    let rows = albums
        .iter()
        .map(|album| cassette_core::models::SpotifyAlbumHistory {
            artist: album.artist.clone(),
            album: album.album.clone(),
            total_ms: album.total_ms,
            play_count: album.play_count,
            skip_count: album.skip_count,
            in_library: album.in_library,
            imported_at: String::new(),
        })
        .collect::<Vec<_>>();
    db.replace_spotify_album_history(&rows)
        .expect("history rows should persist");

    let count = db
        .get_spotify_album_history_count()
        .expect("history count should be readable");
    assert_eq!(count, 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parse_lastfm_artist_info_strips_html_and_reads_tags() {
    let json = serde_json::json!({
        "artist": {
            "bio": { "summary": "Great band text <a href=\"https://example\">Read more</a>" },
            "stats": { "listeners": "12345" },
            "tags": { "tag": [{ "name": "indie" }, { "name": "post-punk" }] }
        }
    });

    let parsed = now_playing::parse_lastfm_artist_info(&json).expect("artist info should parse");
    assert_eq!(parsed.summary.as_deref(), Some("Great band text"));
    assert_eq!(parsed.listeners, Some(12345));
    assert_eq!(
        parsed.tags,
        vec!["indie".to_string(), "post-punk".to_string()]
    );
}

#[test]
fn parse_lastfm_album_info_prefers_largest_image() {
    let json = serde_json::json!({
        "album": {
            "wiki": { "summary": "Album summary <a href=\"https://example\">read</a>" },
            "image": [
                { "#text": "small.jpg", "size": "small" },
                { "#text": "", "size": "large" },
                { "#text": "mega.jpg", "size": "mega" }
            ]
        }
    });

    let parsed = now_playing::parse_lastfm_album_info(&json).expect("album info should parse");
    assert_eq!(parsed.summary.as_deref(), Some("Album summary"));
    assert_eq!(parsed.image_url.as_deref(), Some("mega.jpg"));
}

#[test]
fn runtime_bootstrap_creates_runtime_and_sidecar_dbs() {
    let root = std::env::temp_dir().join(format!("cassette-bootstrap-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let db_path = root.join("cassette.db");

    let (_db, _control_db) =
        runtime_bootstrap::open_runtime_and_control_db(&db_path).expect("runtime and sidecar dbs");

    assert!(db_path.exists(), "runtime db should exist");
    assert!(
        runtime_bootstrap::control_db_path_for_runtime(&db_path).exists(),
        "sidecar db should exist"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn shared_album_resolver_uses_expected_fallback_order() {
    assert_eq!(
        album_resolver::resolution_fallback_order().as_slice(),
        ["musicbrainz", "itunes", "spotify"]
    );
}

#[test]
fn downloads_command_album_queue_uses_shared_remote_config_resolver() {
    let source = include_str!("../src/commands/downloads.rs");
    assert!(source.contains("resolve_album_track_tasks_from_remote_config("));
    assert!(!source.contains("resolve_album_track_tasks_with_metadata("));
}

#[test]
fn engine_pipeline_spotify_backlog_uses_shared_remote_config_resolver() {
    let source = include_str!("../src/bin/engine_pipeline_cli.rs");
    assert!(source.contains("resolve_album_track_tasks_from_remote_config("));
    assert!(!source.contains("resolve_album_track_tasks_with_metadata("));
}

#[test]
fn batch_downloader_uses_shared_spotify_credential_resolver() {
    let source = include_str!("../src/bin/batch_download_cli.rs");
    assert!(source.contains("resolve_album_track_tasks_from_spotify_credentials("));
    assert!(!source.contains("resolve_album_track_tasks_with_metadata("));
}

#[test]
fn desktop_setup_registers_media_shortcuts_and_tray_menu() {
    let source = include_str!("../src/lib.rs");

    assert!(source.contains("register_media_shortcuts(app)"));
    assert!(source.contains("register_tray_menu(app)"));
    assert!(source.contains("get_slskd_runtime_status"));
    assert!(source.contains("restart_slskd_runtime"));
    assert!(source.contains(
        "with_shortcuts([\"MediaPlayPause\", \"MediaTrackNext\", \"MediaTrackPrevious\"])"
    ));
    assert!(source.contains("TrayIconBuilder::with_id(\"cassette-tray\")"));
}

#[test]
fn smoke_script_uses_managed_slskd_runtime_probe() {
    let smoke = include_str!("../../scripts/smoke_desktop.ps1");

    assert!(smoke.contains("slskd_runtime_probe_cli"));
    assert!(smoke.contains("Managed slskd runtime ready"));
    assert!(!smoke.contains("slskd localhost:5030"));
}

#[test]
fn slskd_runtime_probe_reuses_runtime_manager_contract() {
    let probe = include_str!("../src/bin/slskd_runtime_probe_cli.rs");

    assert!(probe.contains("runtime.ensure_started(None, &db, &download_config)"));
    assert!(probe.contains("runtime.refresh_status(None, &db, &download_config)"));
    assert!(probe.contains("probe_status.spawned_by_app"));
    assert!(probe.contains("runtime.stop()"));
}

#[test]
fn tray_show_paths_restore_main_window_focus() {
    let source = include_str!("../src/lib.rs");

    assert!(source.contains("\"show\""));
    assert!(source.contains("window.unminimize()"));
    assert!(source.contains("window.show()"));
    assert!(source.contains("window.set_focus()"));
    assert!(source.contains("TrayIconEvent::Click"));
    assert!(source.contains("MouseButton::Left"));
    assert!(source.contains("MouseButtonState::Up"));
}

#[test]
fn command_palette_keyboard_contract_is_present() {
    let source = include_str!("../../ui/src/lib/components/CommandPalette.svelte");

    assert!(source.contains("event.ctrlKey || event.metaKey"));
    assert!(source.contains("event.key.toLowerCase() === 'k'"));
    assert!(source.contains("if (event.key === 'Escape')"));
    assert!(source.contains("if (event.key === 'Enter')"));
    assert!(source.contains("handleGlobalShortcut(event)"));
}

#[test]
fn command_shortcuts_protect_editable_targets() {
    let source = include_str!("../../ui/src/lib/stores/commands.ts");

    assert!(source.contains("function isEditableTarget"));
    assert!(source.contains("target.isContentEditable"));
    assert!(source.contains("tag === 'input' || tag === 'textarea' || tag === 'select'"));
    assert!(source.contains("if (isEditableTarget(event.target))"));
    assert!(source.contains("return false;"));
}

#[test]
fn route_components_do_not_import_desktop_window_or_core_apis() {
    let layout = include_str!("../../ui/src/routes/+layout.svelte");
    let home = include_str!("../../ui/src/routes/+page.svelte");
    let library = include_str!("../../ui/src/routes/library/+page.svelte");
    let downloads = include_str!("../../ui/src/routes/downloads/+page.svelte");
    let settings = include_str!("../../ui/src/routes/settings/+page.svelte");

    for source in [layout, home, library, downloads, settings] {
        assert!(!source.contains("@tauri-apps/api/window"));
        assert!(!source.contains("@tauri-apps/api/core"));
    }
}

#[test]
fn library_route_surfaces_musicbrainz_identity_details() {
    let library = include_str!("../../ui/src/routes/library/+page.svelte");
    let tauri_api = include_str!("../../ui/src/lib/api/tauri.ts");

    assert!(library.contains("MB recording"));
    assert!(library.contains("MB release"));
    assert!(library.contains("MB release group"));
    assert!(library.contains("Edition bucket"));
    assert!(library.contains("Canonical artist"));
    assert!(library.contains("track-inspector"));
    assert!(tauri_api.contains("get_track_identity_context"));
}

#[test]
fn downloads_route_surfaces_release_group_and_edition_policy_hints() {
    let downloads = include_str!("../../ui/src/routes/downloads/+page.svelte");
    let downloads_commands = include_str!("../src/commands/downloads.rs");

    assert!(downloads.contains("requestIdentityMeta"));
    assert!(downloads.contains("policy:"));
    assert!(downloads_commands.contains("musicbrainz_release_group_id"));
    assert!(downloads_commands.contains("edition_policy"));
}

#[test]
fn settings_route_exposes_policy_profile_controls_and_commands() {
    let settings_route = include_str!("../../ui/src/routes/settings/+page.svelte");
    let tauri_api = include_str!("../../ui/src/lib/api/tauri.ts");
    let settings_commands = include_str!("../src/commands/settings.rs");

    assert!(settings_route.contains("Playback-First"));
    assert!(settings_route.contains("Aggressive Overnight"));
    assert!(tauri_api.contains("getPolicyProfile"));
    assert!(tauri_api.contains("setPolicyProfile"));
    assert!(settings_commands.contains("get_policy_profile"));
    assert!(settings_commands.contains("set_policy_profile"));
}

#[test]
fn home_route_carries_while_you_were_away_and_artist_first_copy() {
    let home = include_str!("../../ui/src/routes/+page.svelte");
    let layout = include_str!("../../ui/src/routes/+layout.svelte");

    assert!(home.contains("While you were away"));
    assert!(home.contains("Artist-first collection"));
    assert!(home.contains("Calm automation"));
    assert!(layout.contains("SystemStatusStrip"));
}

#[test]
fn signature_surfaces_wave_three_and_four_are_present() {
    let sidebar = include_str!("../../ui/src/lib/components/Sidebar.svelte");
    let playlists = include_str!("../../ui/src/routes/playlists/+page.svelte");
    let queue = include_str!("../../ui/src/routes/queue/+page.svelte");
    let session = include_str!("../../ui/src/lib/components/SessionComposer.svelte");
    let now_playing = include_str!("../../ui/src/lib/components/NowPlayingShrine.svelte");

    assert!(sidebar.contains("/crates"));
    assert!(sidebar.contains("/now-playing"));
    assert!(playlists.contains("Authorship ritual"));
    assert!(playlists.contains("Variants"));
    assert!(queue.contains("Queue scenes"));
    assert!(queue.contains("Pivot to artist"));
    assert!(session.contains("Branch arc"));
    assert!(session.contains("Export to playlist"));
    assert!(now_playing.contains("Immersion ritual"));
    assert!(now_playing.contains("Provenance"));
}

#[test]
fn calm_automation_wave_is_present() {
    let digest = include_str!("../../ui/src/lib/automation-digest.ts");
    let digest_panel = include_str!("../../ui/src/lib/components/AutomationDigestPanel.svelte");
    let right_sidebar = include_str!("../../ui/src/lib/components/RightSidebar.svelte");
    let workstation = include_str!("../../ui/src/lib/components/WorkstationSurface.svelte");

    assert!(digest.contains("silent"));
    assert!(digest.contains("digest"));
    assert!(digest.contains("soft_attention"));
    assert!(digest.contains("explicit_intervention"));
    assert!(digest_panel.contains("Calm automation"));
    assert!(right_sidebar.contains("Room"));
    assert!(workstation.contains("Digest boundary"));
}

#[test]
fn shell_foundation_uses_persistent_library_rail_and_workstation_deck() {
    let layout = include_str!("../../ui/src/routes/+layout.svelte");
    let shell_store = include_str!("../../ui/src/lib/stores/shell.ts");
    let library_rail = include_str!("../../ui/src/lib/components/LibraryRail.svelte");
    let workstation_deck = include_str!("../../ui/src/lib/components/WorkstationDeck.svelte");
    let workstation_route = include_str!("../../ui/src/routes/workstation/+page.svelte");
    let commands = include_str!("../../ui/src/lib/stores/commands.ts");
    let sidebar = include_str!("../../ui/src/lib/components/Sidebar.svelte");
    let app_css = include_str!("../../ui/src/app.css");

    assert!(shell_store.contains("libraryRailWidth"));
    assert!(shell_store.contains("utilityWellWidth"));
    assert!(shell_store.contains("utilityWellMode"));
    assert!(shell_store.contains("workstationDeckOpen"));
    assert!(shell_store.contains("applyWorkspacePreset"));
    assert!(layout.contains("LibraryRail"));
    assert!(layout.contains("WorkstationDeck"));
    assert!(layout.contains("Resize library rail"));
    assert!(layout.contains("Resize utility well"));
    assert!(library_rail.contains("Browser rail"));
    assert!(library_rail.contains("Preview"));
    assert!(workstation_deck.contains("Workstation deck"));
    assert!(workstation_route.contains("Compatibility surface"));
    assert!(commands.contains("Open Workstation Deck"));
    assert!(commands.contains("Focus Library Rail"));
    assert!(sidebar.contains("shellSurface: true"));
    assert!(app_css.contains("--library-rail-w"));
    assert!(app_css.contains(".app-library"));
    assert!(app_css.contains(".app-shell.utility-collapsed"));
}

#[test]
fn first_selective_breakout_uses_a_persisted_visualizer_window() {
    let capabilities = include_str!("../capabilities/default.json");
    let shell_store = include_str!("../../ui/src/lib/stores/shell.ts");
    let commands = include_str!("../../ui/src/lib/stores/commands.ts");
    let now_playing = include_str!("../../ui/src/lib/components/NowPlaying.svelte");
    let layout = include_str!("../../ui/src/routes/+layout.svelte");
    let visualizer_route = include_str!("../../ui/src/routes/visualizer-window/+page.svelte");

    assert!(capabilities.contains("\"visualizer\""));
    assert!(capabilities.contains("core:window:allow-create"));
    assert!(capabilities.contains("core:window:allow-get-all-windows"));
    assert!(shell_store.contains("VISUALIZER_WINDOW_LABEL"));
    assert!(shell_store.contains("openVisualizerWindow"));
    assert!(shell_store.contains("cassette.shell.visualizerWindowGeometry"));
    assert!(commands.contains("Open Visualizer Window"));
    assert!(now_playing.contains(">Visualizer</button>"));
    assert!(layout.contains("isVisualizerWindowRoute"));
    assert!(visualizer_route.contains("Visualizer breakout proof"));
    assert!(visualizer_route.contains("persistWindowGeometry"));
    assert!(visualizer_route.contains("decorative or preset-driven"));
}

#[test]
fn listening_shell_keeps_workstation_as_the_single_control_doorway() {
    let sidebar = include_str!("../../ui/src/lib/components/Sidebar.svelte");
    let boundary_map = include_str!("../../docs/EXPERIENCE_BOUNDARY_MAP.md");
    let object_model = include_str!("../../docs/OBJECT_MODEL_DECISIONS.md");

    assert!(sidebar.contains("nav-heading\">Workstation"));
    assert!(sidebar.contains("label: 'Workstation'"));
    assert!(!sidebar.contains("label: 'Downloads'"));
    assert!(!sidebar.contains("label: 'Import'"));
    assert!(!sidebar.contains("label: 'History'"));
    assert!(!sidebar.contains("label: 'Tools'"));
    assert!(!sidebar.contains("label: 'Settings'"));
    assert!(boundary_map.contains("single Workstation doorway"));
    assert!(boundary_map.contains("Acquire"));
    assert!(object_model.contains("Playlist is not Crate."));
    assert!(object_model.contains("Session is not Queue Scene."));
}

#[test]
fn player_runtime_listener_advances_or_stops_cleanly_on_track_end() {
    let state = include_str!("../src/state.rs");
    let core_player = include_str!("../../crates/cassette-core/src/player/mod.rs");

    assert!(state.contains("spawn_player_event_listener("));
    assert!(state.contains("PlayerEvent::TrackEnded"));
    assert!(state.contains("handle_track_end("));
    assert!(state.contains("player.load(track.path.clone())"));
    assert!(state.contains("player.pause();"));
    assert!(core_player.contains("recv_event_timeout"));
}

#[test]
fn primary_actions_use_direct_button_labels() {
    let home = include_str!("../../ui/src/routes/+page.svelte");
    let layout = include_str!("../../ui/src/routes/+layout.svelte");
    let queue = include_str!("../../ui/src/routes/queue/+page.svelte");
    let action_rail = include_str!("../../ui/src/lib/components/ContextActionRail.svelte");

    assert!(!home.contains("Open collection"));
    assert!(!home.contains("Open queue"));
    assert!(!home.contains("Open workstation"));
    assert!(home.contains(">Collection</button>"));
    assert!(home.contains(">Queue</button>"));
    assert!(home.contains(">Workstation</button>"));
    assert!(layout.contains("Commands"));
    assert!(queue.contains("'Play queue'"));
    assert!(action_rail.contains("Play now"));
    assert!(action_rail.contains("Add to queue"));
    assert!(action_rail.contains("Get track"));
    assert!(action_rail.contains("Get album"));
    assert!(action_rail.contains("Get releases"));
}

#[test]
fn primary_transport_stops_optimistic_seek_and_volume_mutation() {
    let player_store = include_str!("../../ui/src/lib/stores/player.ts");
    let now_playing = include_str!("../../ui/src/lib/components/NowPlaying.svelte");

    assert!(player_store.contains("export const playerActionError = writable<string | null>(null);"));
    assert!(player_store.contains("Failed to seek playback."));
    assert!(player_store.contains("Failed to change volume."));
    assert!(!player_store.contains("playbackState.update((s) => ({ ...s, position_secs: secs }))"));
    assert!(!player_store.contains("playbackState.update((s) => ({ ...s, volume: v }))"));
    assert!(now_playing.contains("playerActionError"));
    assert!(now_playing.contains("np-runtime-hint-error"));
}

#[test]
fn shell_and_primary_surfaces_show_bounded_load_or_action_failures() {
    let shell_store = include_str!("../../ui/src/lib/stores/shell.ts");
    let layout = include_str!("../../ui/src/routes/+layout.svelte");
    let queue_store = include_str!("../../ui/src/lib/stores/queue.ts");
    let queue_panel = include_str!("../../ui/src/lib/components/QueuePanel.svelte");
    let library_store = include_str!("../../ui/src/lib/stores/library.ts");
    let library_route = include_str!("../../ui/src/routes/library/+page.svelte");
    let downloads_store = include_str!("../../ui/src/lib/stores/downloads.ts");
    let downloads_route = include_str!("../../ui/src/routes/downloads/+page.svelte");

    assert!(shell_store.contains("export const shellActionError = writable<string | null>(null);"));
    assert!(layout.contains("shell-action-banner"));
    assert!(queue_store.contains("export const queueLoadError = writable<string | null>(null);"));
    assert!(queue_panel.contains("Queue unavailable"));
    assert!(library_store.contains("export const libraryLoadError = writable<string | null>(null);"));
    assert!(library_route.contains("Library unavailable"));
    assert!(downloads_store.contains("export const downloadsSurfaceError = writable<string | null>(null);"));
    assert!(downloads_route.contains("surfaceLoadError"));
    assert!(downloads_route.contains("panel-note-error"));
}

#[test]
fn import_route_unifies_spotify_history_and_direct_track_intake() {
    let import_route = include_str!("../../ui/src/routes/import/+page.svelte");
    let import_commands = include_str!("../src/commands/import.rs");
    let tauri_api = include_str!("../../ui/src/lib/api/tauri.ts");

    assert!(import_route.contains("Spotify intake"));
    assert!(import_route.contains("Album backlog from streaming history"));
    assert!(import_route.contains("Direct desired-track JSON"));
    assert!(import_route.contains("same identity-first desired-state pipeline"));
    assert!(tauri_api.contains("importSpotifyDesiredTracks"));
    assert!(import_commands.contains("import_spotify_desired_tracks"));
}

#[test]
fn spotify_intake_and_album_queue_use_canonical_operator_story() {
    let import_commands = include_str!("../src/commands/import.rs");
    let download_commands = include_str!("../src/commands/downloads.rs");

    assert!(import_commands.contains("import_desired_spotify_json"));
    assert!(import_commands.contains("queue_album_tracks("));
    assert!(import_commands.contains("TrackTaskSource::SpotifyLibrary"));
    assert!(download_commands.contains("plan_acquisition(state.clone(), request).await"));
}

#[test]
fn planner_identity_lane_carries_release_group_and_edition_policy() {
    let planner_commands = include_str!("../src/commands/planner.rs");

    assert!(planner_commands
        .contains("musicbrainz_release_group_id: request.musicbrainz_release_group_id.clone()"));
    assert!(planner_commands
        .contains("musicbrainz_release_group_id: request.musicbrainz_release_group_id.clone(),"));
    assert!(planner_commands.contains("edition_policy: request.edition_policy.clone()"));
    assert!(planner_commands.contains("apply_edition_policy_filter_to_records"));
}

#[test]
fn queue_boundary_enforces_richer_identity_contract() {
    let download_commands = include_str!("../src/commands/downloads.rs");

    assert!(download_commands.contains("validate_request_identity_contract(&task, &request)?"));
    assert!(download_commands.contains("request contract violation for task"));
    assert!(download_commands.contains("source_track_id was not preserved"));
    assert!(download_commands.contains("source_album_id was not preserved"));
    assert!(download_commands.contains("musicbrainz_release_group_id was not preserved"));
}

fn sample_request(status: &str) -> AcquisitionRequestRow {
    AcquisitionRequestRow {
        id: 1,
        scope: "track".to_string(),
        source_name: "manual".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist: "Artist".to_string(),
        album: Some("Album".to_string()),
        title: "Song".to_string(),
        normalized_artist: "artist".to_string(),
        normalized_album: Some("album".to_string()),
        normalized_title: "song".to_string(),
        track_number: Some(1),
        disc_number: Some(1),
        year: Some(2024),
        duration_secs: Some(200.0),
        isrc: None,
        musicbrainz_recording_id: None,
        musicbrainz_release_group_id: None,
        musicbrainz_release_id: None,
        canonical_artist_id: None,
        canonical_release_id: None,
        strategy: "Standard".to_string(),
        quality_policy: None,
        excluded_providers_json: None,
        edition_policy: None,
        confirmation_policy: "auto".to_string(),
        desired_track_id: Some(42),
        source_operation_id: None,
        task_id: Some("task-1".to_string()),
        request_signature: "sig-1".to_string(),
        status: status.to_string(),
        raw_payload_json: None,
        created_at: "2026-04-06 00:00:00".to_string(),
        updated_at: "2026-04-06 00:00:00".to_string(),
    }
}

#[test]
fn trust_ledger_maps_failed_execution_reason_codes() {
    let request = sample_request("failed");
    let execution = cassette_core::db::TaskExecutionSummary {
        task_id: "task-1".to_string(),
        disposition: "Failed".to_string(),
        provider: Some("qobuz".to_string()),
        failure_class: Some("rate_limited".to_string()),
        final_path: None,
        updated_at: "2026-04-06 00:00:00".to_string(),
    };

    let summary = trust_ledger::derive_request_trust_summary(
        &request,
        &[],
        Some(&execution),
        &[],
        &[],
        &[],
    );

    assert_eq!(summary.stage, "blocked");
    assert_eq!(summary.reason_code, "rate_limited");
}

#[test]
fn trust_ledger_prefers_gatekeeper_decision_when_runtime_result_missing() {
    let request = sample_request("in_progress");
    let audit = cassette_core::db::TrustLedgerGatekeeperAudit {
        operation_id: "op-1".to_string(),
        timestamp: "2026-04-06T00:00:00Z".to_string(),
        file_path: "C:\\Music\\Artist\\Song.flac".to_string(),
        decision: "Quarantined".to_string(),
        desired_track_id: Some(42),
        matched_local_file_id: None,
        duration_ms: 45,
        notes: "identity mismatch".to_string(),
    };

    let summary = trust_ledger::derive_request_trust_summary(
        &request,
        &[],
        None,
        &[],
        &[],
        &[audit],
    );

    assert_eq!(summary.reason_code, "quarantined");
    assert_eq!(summary.headline, "Quarantined by gatekeeper");
}
