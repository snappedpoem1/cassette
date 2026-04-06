use crate::director::error::MetadataError;
use crate::director::models::{CandidateSelection, TrackTask};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::TagExt;
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem};

pub async fn apply_metadata(
    task: TrackTask,
    selection: CandidateSelection,
) -> Result<(), MetadataError> {
    let path = selection.temp_path.clone();
    let cover_art = download_cover_art_candidates(cover_art_candidates(
        selection.cover_art_url.as_deref(),
        task.target.musicbrainz_release_id.as_deref(),
    ))
    .await;

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

fn cover_art_candidates(
    provider_cover_art_url: Option<&str>,
    musicbrainz_release_id: Option<&str>,
) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(url) = provider_cover_art_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        urls.push(url.to_string());
    }

    if let Some(release_id) = musicbrainz_release_id
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains(':'))
    {
        urls.push(format!(
            "https://coverartarchive.org/release/{release_id}/front-500"
        ));
        urls.push(format!(
            "https://coverartarchive.org/release/{release_id}/front"
        ));
    }

    urls
}

async fn download_cover_art_candidates(urls: Vec<String>) -> Option<Vec<u8>> {
    for url in urls {
        if let Some(bytes) = download_cover_art(&url).await {
            return Some(bytes);
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::cover_art_candidates;

    #[test]
    fn cover_art_candidates_fall_back_to_cover_art_archive_for_musicbrainz_releases() {
        let urls = cover_art_candidates(None, Some("mb-release-123"));
        assert_eq!(
            urls,
            vec![
                "https://coverartarchive.org/release/mb-release-123/front-500".to_string(),
                "https://coverartarchive.org/release/mb-release-123/front".to_string(),
            ]
        );
    }

    #[test]
    fn cover_art_candidates_skip_cover_art_archive_for_non_musicbrainz_release_ids() {
        let urls = cover_art_candidates(Some("https://example.com/cover.jpg"), Some("spotify:123"));
        assert_eq!(urls, vec!["https://example.com/cover.jpg".to_string()]);
    }
}
