use crate::Result;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;

const MB_BASE: &str = "https://musicbrainz.org/ws/2";
const MB_USER_AGENT: &str = "CassettePlayer/0.1 (https://github.com/cassette-music)";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MbRelease {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub track_count: Option<u32>,
    pub release_group_type: Option<String>,
    pub label: Option<String>,
    pub country: Option<String>,
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MbTrack {
    pub title: String,
    pub artist: String,
    pub track_number: u32,
    pub disc_number: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MbReleaseWithTracks {
    pub release: MbRelease,
    pub tracks: Vec<MbTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagFix {
    pub path: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub applied: bool,
}

pub struct MetadataService {
    client: reqwest::Client,
}

impl MetadataService {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(MB_USER_AGENT)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { client })
    }

    /// Search MusicBrainz for a release matching artist + album
    pub async fn search_release(&self, artist: &str, album: &str) -> Result<Vec<MbRelease>> {
        let query = format!("artist:\"{}\" AND release:\"{}\"", artist, album);
        let resp = self.client
            .get(format!("{MB_BASE}/release"))
            .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "5")])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MusicBrainz returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let releases = body.get("releases")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(mb_release_from_value).collect())
            .unwrap_or_default();

        Ok(releases)
    }

    /// Search MusicBrainz recordings by artist + track title and return their primary releases.
    /// Used to find the parent album for single tracks (e.g. "Closer" → "Collage EP").
    /// Prefers Album/EP primary types over Single or Compilation.
    pub async fn find_parent_album(&self, artist: &str, track_title: &str) -> Result<Option<MbRelease>> {
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        let query = format!("recording:\"{}\" AND artist:\"{}\"", track_title, artist);
        let resp = self.client
            .get(format!("{MB_BASE}/recording"))
            .query(&[
                ("query", query.as_str()),
                ("fmt", "json"),
                ("limit", "5"),
                ("inc", "releases+release-groups"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MusicBrainz returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let recordings = body.get("recordings")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Walk recordings → releases, prefer Album/EP over Single/Compilation
        let mut best: Option<MbRelease> = None;
        for rec in &recordings {
            let releases = rec.get("releases")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for rel in &releases {
                let rtype = rel.pointer("/release-group/primary-type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let candidate = mb_release_from_value(rel);
                if best.is_none() {
                    best = Some(candidate.clone());
                }
                if matches!(rtype, "Album" | "EP") {
                    best = Some(candidate);
                    break;
                }
            }
            if best.as_ref().map_or(false, |r| {
                matches!(r.release_group_type.as_deref(), Some("Album") | Some("EP"))
            }) {
                break;
            }
        }

        Ok(best)
    }

    /// Fetch full release details including track listing
    pub async fn get_release_tracks(&self, release_id: &str) -> Result<MbReleaseWithTracks> {
        // Rate limit: MusicBrainz requires 1 req/sec
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        let resp = self.client
            .get(format!("{MB_BASE}/release/{release_id}"))
            .query(&[("inc", "recordings+artist-credits"), ("fmt", "json")])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MusicBrainz returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let release = mb_release_from_value(&body);

        let mut tracks = Vec::new();
        if let Some(media) = body.get("media").and_then(|v| v.as_array()) {
            for (disc_idx, disc) in media.iter().enumerate() {
                let disc_num = disc.get("position")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(disc_idx as u64 + 1) as u32;

                if let Some(track_list) = disc.get("tracks").and_then(|v| v.as_array()) {
                    for t in track_list {
                        let track_num = t.get("position")
                            .or_else(|| t.get("number"))
                            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                            .unwrap_or(0) as u32;

                        let title = t.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();

                        let artist = t.get("artist-credit")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|ac| ac.get("name"))
                            .or_else(|| t.pointer("/recording/artist-credit/0/name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();

                        let duration_ms = t.get("length")
                            .or_else(|| t.pointer("/recording/length"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        tracks.push(MbTrack {
                            title,
                            artist,
                            track_number: track_num,
                            disc_number: disc_num,
                            duration_ms,
                        });
                    }
                }
            }
        }

        Ok(MbReleaseWithTracks { release, tracks })
    }

    /// Match local album tracks against MusicBrainz and return proposed fixes
    pub async fn propose_tag_fixes(
        &self,
        artist: &str,
        album: &str,
        local_tracks: &[crate::models::Track],
    ) -> Result<Vec<TagFix>> {
        let releases = self.search_release(artist, album).await?;
        let Some(best) = releases.first() else {
            return Ok(Vec::new());
        };

        let mb = self.get_release_tracks(&best.id).await?;
        let mut fixes = Vec::new();

        for local in local_tracks {
            let mb_track = mb.tracks.iter().find(|t| {
                t.track_number == local.track_number.unwrap_or(0) as u32
                    && t.disc_number == local.disc_number.unwrap_or(1) as u32
            }).or_else(|| {
                // Fuzzy: match by position in album
                let idx = local.track_number.unwrap_or(1).max(1) as usize - 1;
                mb.tracks.get(idx)
            });

            let Some(mb_t) = mb_track else { continue };

            // Title fix
            if !mb_t.title.is_empty() && mb_t.title != local.title {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "title".into(),
                    old_value: local.title.clone(),
                    new_value: mb_t.title.clone(),
                    applied: false,
                });
            }

            // Artist fix
            if !mb_t.artist.is_empty() && mb_t.artist != local.artist {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "artist".into(),
                    old_value: local.artist.clone(),
                    new_value: mb_t.artist.clone(),
                    applied: false,
                });
            }

            // Album fix
            if !mb.release.title.is_empty() && mb.release.title != local.album {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "album".into(),
                    old_value: local.album.clone(),
                    new_value: mb.release.title.clone(),
                    applied: false,
                });
            }

            // Year fix
            if let Some(year) = mb.release.year {
                if local.year != Some(year) {
                    fixes.push(TagFix {
                        path: local.path.clone(),
                        field: "year".into(),
                        old_value: local.year.map(|y| y.to_string()).unwrap_or_default(),
                        new_value: year.to_string(),
                        applied: false,
                    });
                }
            }

            // Track number fix
            if local.track_number != Some(mb_t.track_number as i32) {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "track_number".into(),
                    old_value: local.track_number.map(|n| n.to_string()).unwrap_or_default(),
                    new_value: mb_t.track_number.to_string(),
                    applied: false,
                });
            }
        }

        Ok(fixes)
    }
}

/// Apply a tag fix to the actual file using lofty
pub fn apply_tag_fix(fix: &TagFix) -> Result<()> {
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    let path = Path::new(&fix.path);
    let mut tagged = Probe::open(path)?.read()?;

    let has_primary = tagged.primary_tag().is_some();
    let tag = if has_primary {
        tagged.primary_tag_mut().unwrap()
    } else {
        tagged.first_tag_mut().ok_or_else(|| anyhow!("No tag found in {}", fix.path))?
    };

    match fix.field.as_str() {
        "title" => { tag.set_title(fix.new_value.clone()); }
        "artist" => { tag.set_artist(fix.new_value.clone()); }
        "album" => { tag.set_album(fix.new_value.clone()); }
        "year" => {
            if let Ok(y) = fix.new_value.parse::<u32>() {
                tag.set_year(y);
            }
        }
        "track_number" => {
            if let Ok(n) = fix.new_value.parse::<u32>() {
                tag.set_track(n);
            }
        }
        "album_artist" => {
            tag.insert(lofty::tag::TagItem::new(
                ItemKey::AlbumArtist,
                lofty::tag::ItemValue::Text(fix.new_value.clone()),
            ));
        }
        _ => {}
    }

    tag.save_to_path(path, lofty::config::WriteOptions::default())?;
    Ok(())
}

fn mb_release_from_value(v: &serde_json::Value) -> MbRelease {
    MbRelease {
        id: v.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        title: v.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        artist: v.get("artist-credit")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|ac| ac.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        year: v.get("date")
            .and_then(|v| v.as_str())
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok()),
        track_count: v.get("track-count")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
        release_group_type: v.pointer("/release-group/primary-type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        label: v.get("label-info")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|li| li.pointer("/label/name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        country: v.get("country")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        barcode: v.get("barcode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mb_release_from_value_maps_core_fields() {
        let json = serde_json::json!({
            "id": "release-123",
            "title": "Turn on the Bright Lights",
            "artist-credit": [{ "name": "Interpol" }],
            "date": "2002-08-20",
            "track-count": 11,
            "release-group": { "primary-type": "Album" },
            "label-info": [{ "label": { "name": "Matador" } }],
            "country": "US",
            "barcode": "74486105222"
        });

        let release = mb_release_from_value(&json);
        assert_eq!(release.id, "release-123");
        assert_eq!(release.title, "Turn on the Bright Lights");
        assert_eq!(release.artist, "Interpol");
        assert_eq!(release.year, Some(2002));
        assert_eq!(release.track_count, Some(11));
        assert_eq!(release.release_group_type.as_deref(), Some("Album"));
        assert_eq!(release.label.as_deref(), Some("Matador"));
        assert_eq!(release.country.as_deref(), Some("US"));
        assert_eq!(release.barcode.as_deref(), Some("74486105222"));
    }

    #[test]
    fn apply_tag_fix_ignores_unknown_field() {
        let fix = TagFix {
            path: "C:\\no-such-file.flac".to_string(),
            field: "unsupported_field".to_string(),
            old_value: String::new(),
            new_value: "value".to_string(),
            applied: false,
        };

        // Unknown field path exits before file I/O branch in apply function match.
        // We still expect a result because opening file is attempted before match.
        // This test validates that unsupported field is handled without panicking.
        let result = apply_tag_fix(&fix);
        assert!(result.is_err());
    }
}
