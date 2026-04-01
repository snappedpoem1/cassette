use crate::director::error::MetadataError;
use crate::director::models::{CandidateSelection, TrackTask};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem};
use lofty::prelude::{Accessor, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::TagExt;

pub async fn apply_metadata(
    task: TrackTask,
    selection: CandidateSelection,
) -> Result<(), MetadataError> {
    let path = selection.temp_path.clone();
    let cover_art = match selection.cover_art_url.as_deref() {
        Some(url) => download_cover_art(url).await,
        None => None,
    };

    tokio::task::spawn_blocking(move || apply_metadata_blocking(task, selection, cover_art))
        .await
        .map_err(|error| MetadataError::TagWrite {
            path,
            message: error.to_string(),
        })?
}

fn apply_metadata_blocking(
    task: TrackTask,
    selection: CandidateSelection,
    cover_art: Option<Vec<u8>>,
) -> Result<(), MetadataError> {
    let path = selection.temp_path.clone();
    let mut tagged = Probe::open(&path)
        .map_err(|error| MetadataError::TagWrite {
            path: path.clone(),
            message: error.to_string(),
        })?
        .read()
        .map_err(|error| MetadataError::TagWrite {
            path: path.clone(),
            message: error.to_string(),
        })?;

    let primary_tag_type = tagged.file_type().primary_tag_type();
    if tagged.tag(primary_tag_type).is_none() {
        tagged.insert_tag(Tag::new(primary_tag_type));
    }

    let tag = if let Some(tag) = tagged.tag_mut(primary_tag_type) {
        tag
    } else if let Some(tag) = tagged.first_tag_mut() {
        tag
    } else {
        return Err(MetadataError::TagWrite {
            path: path.clone(),
            message: "no writable tag container available".to_string(),
        });
    };

    tag.set_artist(task.target.artist.clone());
    if let Some(album) = &task.target.album {
        tag.set_album(album.clone());
    }
    tag.set_title(task.target.title.clone());
    if let Some(track_number) = task.target.track_number {
        tag.set_track(track_number);
    }
    if let Some(disc_number) = task.target.disc_number {
        tag.set_disk(disc_number);
    }
    if let Some(year) = task.target.year.and_then(|value| u32::try_from(value).ok()) {
        tag.set_year(year);
    }
    tag.set_comment(format!(
        "Cassette provenance: provider={}, task_id={}, score={}",
        selection.provider_id, task.task_id, selection.score.total
    ));
    tag.insert(TagItem::new(
        ItemKey::Comment,
        ItemValue::Text(format!(
            "Selected by Cassette Director from {}",
            selection.provider_id
        )),
    ));
    if let Some(bytes) = cover_art {
        if let Some(picture) = build_cover_picture(bytes) {
            tag.push_picture(picture);
        }
    }

    tag.save_to_path(&path, lofty::config::WriteOptions::default())
        .map_err(|error: lofty::error::LoftyError| MetadataError::TagWrite {
            path,
            message: error.to_string(),
        })
}

async fn download_cover_art(url: &str) -> Option<Vec<u8>> {
    let response = reqwest::Client::new().get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let bytes = response.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > 15 * 1024 * 1024 {
        return None;
    }

    Some(bytes.to_vec())
}

fn build_cover_picture(bytes: Vec<u8>) -> Option<Picture> {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(MimeType::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(MimeType::Jpeg)
    } else {
        None
    };

    mime.map(|mime_type| {
        Picture::new_unchecked(PictureType::CoverFront, Some(mime_type), None, bytes)
    })
}
