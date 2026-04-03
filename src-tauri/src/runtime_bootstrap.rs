use anyhow::Result;
use cassette_core::{db::Db, librarian::db::LibrarianDb};
use std::path::{Path, PathBuf};

pub fn control_db_path_for_runtime(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cassette_librarian.db")
}

pub fn open_runtime_and_control_db(db_path: &Path) -> Result<(Db, LibrarianDb)> {
    let db = Db::open(db_path)?;
    let control_db_path = control_db_path_for_runtime(db_path);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let control_db = runtime.block_on(async { LibrarianDb::connect(&control_db_path).await })?;
    runtime.block_on(async { converge_canonical_identity(&db, &control_db).await })?;
    Ok((db, control_db))
}

async fn converge_canonical_identity(db: &Db, control_db: &LibrarianDb) -> Result<()> {
    let artists = db.list_canonical_artists()?;
    let mut artist_id_map = std::collections::HashMap::<i64, i64>::new();

    for artist in artists {
        let converged_id = control_db
            .upsert_canonical_artist(
                &artist.name,
                artist.musicbrainz_id.as_deref(),
                artist.spotify_id.as_deref(),
                artist.discogs_id.as_deref(),
            )
            .await?;
        artist_id_map.insert(artist.id, converged_id);
    }

    let releases = db.list_canonical_releases()?;
    let mut release_id_map = std::collections::HashMap::<i64, i64>::new();

    for release in releases {
        let Some(&converged_artist_id) = artist_id_map.get(&release.canonical_artist_id) else {
            continue;
        };
        let converged_id = control_db
            .upsert_canonical_release(
                converged_artist_id,
                &release.title,
                release.release_group_mbid.as_deref(),
                release.release_mbid.as_deref(),
                release.release_type.as_deref(),
                release.year.map(i64::from),
                release.spotify_id.as_deref(),
                release.discogs_id.as_deref(),
            )
            .await?;
        release_id_map.insert(release.id, converged_id);
    }

    let recordings = db.list_canonical_recordings()?;
    for recording in recordings {
        let converged_artist_id = recording
            .canonical_artist_id
            .and_then(|id| artist_id_map.get(&id).copied());
        let converged_release_id = recording
            .canonical_release_id
            .and_then(|id| release_id_map.get(&id).copied());
        control_db
            .upsert_canonical_recording(
                converged_artist_id,
                converged_release_id,
                &recording.title,
                recording.musicbrainz_recording_id.as_deref(),
                recording.isrc.as_deref(),
                recording.track_number.map(i64::from),
                recording.disc_number.map(i64::from),
                recording.duration_secs.map(|value| (value * 1000.0) as i64),
            )
            .await?;
    }

    Ok(())
}
