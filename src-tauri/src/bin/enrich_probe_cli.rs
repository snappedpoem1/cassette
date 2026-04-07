use cassette_core::db::Db;
use cassette_core::librarian::enrich::discogs::DiscogsClient;
use cassette_core::librarian::enrich::lastfm::LastFmClient;
use std::path::PathBuf;

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::env::var(key.to_ascii_uppercase())
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
}

fn default_db_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(appdata)
        .join("dev.cassette.app")
        .join("cassette.db")
}

fn parse_args() -> (Option<PathBuf>, usize) {
    let args: Vec<String> = std::env::args().collect();
    let mut db_path: Option<PathBuf> = None;
    let mut limit: usize = 10;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--db-path" => {
                i += 1;
                if i < args.len() {
                    db_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--limit" => {
                i += 1;
                if i < args.len() {
                    if let Ok(n) = args[i].parse::<usize>() {
                        limit = n;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (db_path, limit)
}

#[tokio::main]
async fn main() {
    let (db_path_arg, limit) = parse_args();
    let db_path = db_path_arg.unwrap_or_else(default_db_path);

    println!("DB path: {}", db_path.display());

    let db = match Db::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open DB at {}: {}", db_path.display(), e);
            std::process::exit(1);
        }
    };

    let discogs_token = read_setting(&db, "discogs_token");
    let lastfm_api_key = read_setting(&db, "lastfm_api_key");

    println!(
        "Discogs token: {}",
        if discogs_token.is_some() { "configured" } else { "not configured" }
    );
    println!(
        "Last.fm API key: {}",
        if lastfm_api_key.is_some() { "configured" } else { "not configured" }
    );
    println!();

    let discogs_client = DiscogsClient::new(discogs_token);
    let lastfm_client = LastFmClient::new(lastfm_api_key);

    let mut sample = Vec::new();
    let page_size: i64 = 1_000;
    let mut offset: i64 = 0;

    while sample.len() < limit {
        let tracks = match db.get_tracks(page_size, offset) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to fetch tracks: {}", e);
                std::process::exit(1);
            }
        };

        if tracks.is_empty() {
            break;
        }

        for track in tracks {
            if track.artist.trim().is_empty() || track.album.trim().is_empty() {
                continue;
            }
            sample.push(track);
            if sample.len() >= limit {
                break;
            }
        }

        offset += page_size;
    }

    let http_client = reqwest::Client::new();
    let mut discogs_hits = 0usize;
    let mut lastfm_hits = 0usize;
    let total = sample.len();

    for track in &sample {
        println!(
            "Track: {:?} \u{2014} {} / {}",
            track.title, track.artist, track.album
        );

        let discogs_result = discogs_client
            .fetch_release_context(&http_client, &track.artist, &track.album)
            .await;

        match &discogs_result {
            Some(ctx) => {
                discogs_hits += 1;
                println!(
                    "  Discogs: release_id={} year={} genres=[{}] country={}",
                    ctx.release_id,
                    ctx.year
                        .map(|y| y.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                    ctx.genres.join(", "),
                    ctx.country.as_deref().unwrap_or("None"),
                );
            }
            None => {
                println!("  Discogs: None");
            }
        }

        let lastfm_result = lastfm_client
            .fetch_artist_context(&http_client, &track.artist)
            .await;

        match &lastfm_result {
            Some(ctx) => {
                lastfm_hits += 1;
                println!(
                    "  Last.fm:  tags=[{}] listeners={}",
                    ctx.tags.join(", "),
                    ctx.listeners
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                );
            }
            None => {
                println!("  Last.fm:  None");
            }
        }

        println!("---");
    }

    println!(
        "Summary: {} tracks probed | Discogs hits: {}/{} | Last.fm hits: {}/{}",
        total, discogs_hits, total, lastfm_hits, total
    );
}
