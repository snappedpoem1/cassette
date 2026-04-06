use cassette_core::library::{organizer, read_track_metadata};
use cassette_core::models::Track;
use serde::Deserialize;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "aiff", "wv", "ape",
];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];
const SIDECAR_EXTENSIONS: &[&str] = &["lrc", "txt", "nfo", "cue", "log", "sfv", "m3u", "m3u8"];
const SAFE_QUARANTINE_EXTENSIONS: &[&str] = &["nfo", "log", "sfv"];
const STANDARD_ART_BASENAMES: &[&str] = &[
    "cover.jpg",
    "cover.png",
    "folder.jpg",
    "folder.png",
    "front.jpg",
    "front.png",
    "album.jpg",
    "album.png",
    "artwork.jpg",
    "artwork.png",
];

#[derive(Debug, Default, Deserialize)]
struct AuditReport {
    invalid_examples: Vec<AuditSizeFinding>,
    suspicious_size_examples: Vec<AuditSizeFinding>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditSizeFinding {
    path: String,
    status: String,
    reasons: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DeadspotReport {
    examples: Vec<DeadspotFinding>,
}

#[derive(Debug, Default, Deserialize)]
struct DeadspotFinding {
    path: String,
    reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileKind {
    Audio,
    Image,
    Sidecar,
    Other,
}

impl FileKind {
    fn as_str(self) -> &'static str {
        match self {
            FileKind::Audio => "audio",
            FileKind::Image => "image",
            FileKind::Sidecar => "sidecar",
            FileKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
struct AudioEntry {
    source_path: PathBuf,
    track: Track,
    normalized_artist: String,
    normalized_album: String,
    has_track_number: bool,
    invalid: bool,
    suspicious_size: bool,
    deadspot: bool,
}

#[derive(Debug, Default)]
struct FolderAccumulator {
    child_dirs: BTreeSet<String>,
    audio_paths: Vec<PathBuf>,
    image_paths: Vec<PathBuf>,
    sidecar_paths: Vec<PathBuf>,
    other_paths: Vec<PathBuf>,
}

#[derive(Debug, Serialize)]
struct FolderReportRow {
    folder_path: String,
    relative_path: String,
    audio_count: usize,
    image_count: usize,
    sidecar_count: usize,
    other_count: usize,
    child_dir_count: usize,
    folder_flags: String,
    proposed_action: String,
    apply_eligible: bool,
    notes: String,
}

#[derive(Debug, Serialize)]
struct GroupPlanRow {
    group_key: String,
    audio_count: usize,
    dominant_artist: String,
    dominant_album: String,
    dominant_year: String,
    canonical_folder: String,
    proposed_action: String,
    apply_eligible: bool,
    notes: String,
}

#[derive(Debug, Clone, Serialize)]
struct ManifestRow {
    group_key: String,
    item_type: String,
    action: String,
    apply_eligible: bool,
    source_path: String,
    target_path: String,
    rollback_source: String,
    rollback_target: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct ManifestSummary {
    root: String,
    output_dir: String,
    quarantine_root: String,
    folder_count: usize,
    audio_file_count: usize,
    image_file_count: usize,
    sidecar_file_count: usize,
    other_file_count: usize,
    manifest_rows: usize,
    apply_eligible_rows: usize,
    apply_eligible_groups: usize,
    review_rows: usize,
    collision_rows: usize,
}

#[derive(Debug, Clone)]
struct GroupPlan {
    group_key: String,
    apply_eligible: bool,
    action: String,
    reason: String,
    canonical_folder: Option<PathBuf>,
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| "A:\\music".to_string()));
    let output_dir = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "tmp\\library_cleanup_manifest".to_string()),
    );
    let quarantine_root = PathBuf::from(args.next().unwrap_or_else(|| {
        root.parent()
            .unwrap_or_else(|| Path::new("."))
            .join("_Cassette_Quarantine")
            .to_string_lossy()
            .to_string()
    }));

    if !root.exists() {
        return Err(format!("Library root does not exist: {}", root.display()));
    }

    fs::create_dir_all(&output_dir).map_err(|e| format!("create output dir failed: {e}"))?;

    let audit_report = load_audit_report(Path::new("tmp").join("library_audit_report.json"));
    let deadspot_report =
        load_deadspot_report(Path::new("tmp").join("library_deadspot_report.json"));
    let invalid_map = build_issue_map(&audit_report.invalid_examples);
    let suspicious_map = build_issue_map(&audit_report.suspicious_size_examples);
    let deadspot_map = build_deadspot_map(&deadspot_report.examples);

    let mut folders: BTreeMap<PathBuf, FolderAccumulator> = BTreeMap::new();
    let mut audio_entries: BTreeMap<PathBuf, AudioEntry> = BTreeMap::new();

    for entry in WalkDir::new(&root)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path().to_path_buf();
        if entry.file_type().is_dir() {
            folders.entry(path).or_default();
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let parent = path.parent().unwrap_or(root.as_path()).to_path_buf();
        let folder = folders.entry(parent.clone()).or_default();
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();

        if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            folder.audio_paths.push(path.clone());
            if let Some(audio_entry) =
                build_audio_entry(&path, &invalid_map, &suspicious_map, &deadspot_map)
            {
                audio_entries.insert(path.clone(), audio_entry);
            }
        } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            folder.image_paths.push(path.clone());
        } else if SIDECAR_EXTENSIONS.contains(&ext.as_str()) {
            folder.sidecar_paths.push(path.clone());
        } else {
            folder.other_paths.push(path.clone());
        }

        if let Some(child_name) = parent.file_name().and_then(|value| value.to_str()) {
            let mut cursor = parent.clone();
            while let Some(parent_dir) = cursor.parent() {
                if !parent_dir.starts_with(&root) {
                    break;
                }
                folders
                    .entry(parent_dir.to_path_buf())
                    .or_default()
                    .child_dirs
                    .insert(child_name.to_string());
                cursor = parent_dir.to_path_buf();
                if cursor == root {
                    break;
                }
            }
        }
    }

    let mut folder_rows = Vec::new();
    let mut group_rows = Vec::new();
    let mut manifest_rows = Vec::new();

    for (folder_path, folder) in &folders {
        let group_key = relative_display(&root, folder_path);
        let folder_audio: Vec<&AudioEntry> = folder
            .audio_paths
            .iter()
            .filter_map(|path| audio_entries.get(path))
            .collect();
        let plan = build_group_plan(&root, folder_path, &folder_audio);
        folder_rows.push(build_folder_row(
            &root,
            folder_path,
            folder,
            &folder_audio,
            &plan,
        ));
        if !folder_audio.is_empty() {
            group_rows.push(build_group_row(&folder_audio, &plan));
        }
        manifest_rows.extend(build_manifest_rows(
            &root,
            &quarantine_root,
            &group_key,
            folder,
            &folder_audio,
            &plan,
        ));
    }

    downgrade_collisions(&mut manifest_rows);
    folder_rows.sort_by(|left, right| left.folder_path.cmp(&right.folder_path));
    group_rows.sort_by(|left, right| left.group_key.cmp(&right.group_key));
    manifest_rows.sort_by(|left, right| {
        (
            left.group_key.as_str(),
            left.item_type.as_str(),
            left.source_path.as_str(),
        )
            .cmp(&(
                right.group_key.as_str(),
                right.item_type.as_str(),
                right.source_path.as_str(),
            ))
    });

    let summary = build_summary(
        &root,
        &output_dir,
        &quarantine_root,
        &folders,
        &manifest_rows,
    );
    write_outputs(
        &output_dir,
        &folder_rows,
        &group_rows,
        &manifest_rows,
        &summary,
    )?;
    println!("Cleanup manifest written to {}", output_dir.display());
    Ok(())
}

fn build_audio_entry(
    path: &Path,
    invalid_map: &HashMap<String, Vec<String>>,
    suspicious_map: &HashMap<String, Vec<String>>,
    deadspot_map: &HashMap<String, Vec<String>>,
) -> Option<AudioEntry> {
    let track = read_track_metadata(path).ok()?;
    let normalized_path = normalize_path_key(path);

    Some(AudioEntry {
        source_path: path.to_path_buf(),
        normalized_artist: normalize_token(best_artist(&track)),
        normalized_album: normalize_token(&track.album),
        has_track_number: track.track_number.unwrap_or_default() > 0,
        invalid: invalid_map.contains_key(&normalized_path),
        suspicious_size: suspicious_map.contains_key(&normalized_path),
        deadspot: deadspot_map.contains_key(&normalized_path),
        track,
    })
}

fn build_group_plan(root: &Path, folder_path: &Path, folder_audio: &[&AudioEntry]) -> GroupPlan {
    let group_key = relative_display(root, folder_path);

    if folder_audio.is_empty() {
        return GroupPlan {
            group_key,
            apply_eligible: false,
            action: "REVIEW_NON_AUDIO_FOLDER".to_string(),
            reason: "No audio files in folder.".to_string(),
            canonical_folder: None,
        };
    }

    if folder_audio.len() == 1 {
        return GroupPlan {
            group_key,
            apply_eligible: false,
            action: "REVIEW_SINGLE_TRACK_FOLDER".to_string(),
            reason: "Single-track folders stay review-only.".to_string(),
            canonical_folder: None,
        };
    }

    let artist_count = distinct_count(
        folder_audio
            .iter()
            .map(|entry| entry.normalized_artist.as_str()),
    );
    let album_count = distinct_count(
        folder_audio
            .iter()
            .map(|entry| entry.normalized_album.as_str()),
    );
    let target_folders: HashSet<PathBuf> = folder_audio
        .iter()
        .map(|entry| {
            organizer::canonical_path(&root.to_string_lossy(), &entry.track)
                .parent()
                .unwrap_or(root)
                .to_path_buf()
        })
        .collect();

    let folder_name = folder_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    let mut reasons = Vec::new();
    if dominant_value(folder_audio.iter().map(|entry| best_artist(&entry.track))).is_empty()
        || dominant_value(folder_audio.iter().map(|entry| entry.track.album.as_str())).is_empty()
    {
        reasons.push("Missing consistent artist/album tags.".to_string());
    }
    if artist_count > 1 {
        reasons.push("Mixed artist tags in folder.".to_string());
    }
    if album_count > 1 {
        reasons.push("Mixed album tags in folder.".to_string());
    }
    if folder_audio.iter().any(|entry| !entry.has_track_number) {
        reasons.push("One or more tracks are missing track numbers.".to_string());
    }
    if folder_audio
        .iter()
        .any(|entry| entry.invalid || entry.suspicious_size || entry.deadspot)
    {
        reasons.push("One or more tracks failed validation or deadspot checks.".to_string());
    }
    if target_folders.len() != 1 {
        reasons.push("Tracks do not converge to one canonical folder.".to_string());
    }
    if is_suspicious_folder_name(folder_name) {
        reasons.push("Folder name is suspicious.".to_string());
    }
    if folder_audio.iter().any(|entry| {
        let expected = organizer::canonical_path(&root.to_string_lossy(), &entry.track);
        expected.exists() && normalize_path_key(&expected) != normalize_path_key(&entry.source_path)
    }) {
        reasons.push("One or more canonical destinations already exist.".to_string());
    }

    let apply_eligible = reasons.is_empty();
    GroupPlan {
        group_key,
        apply_eligible,
        action: if apply_eligible {
            "SAFE_RELOCATE_GROUP".to_string()
        } else {
            "REVIEW_GROUP".to_string()
        },
        reason: if reasons.is_empty() {
            "Folder is complete, consistent, and can move as a group.".to_string()
        } else {
            reasons.join(" ")
        },
        canonical_folder: target_folders.iter().next().cloned(),
    }
}

fn build_folder_row(
    root: &Path,
    folder_path: &Path,
    folder: &FolderAccumulator,
    folder_audio: &[&AudioEntry],
    plan: &GroupPlan,
) -> FolderReportRow {
    let folder_name = folder_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut flags = Vec::new();
    if folder_name.eq_ignore_ascii_case("Unknown Album") {
        flags.push("UNKNOWN_ALBUM");
    }
    if is_suspicious_folder_name(folder_name) {
        flags.push("SUSPICIOUS_FOLDER_NAME");
    }
    if folder.audio_paths.len() > 0 && folder.sidecar_paths.len() > 0 {
        flags.push("MIXED_AUDIO_AND_SIDECARS");
    }
    if folder.audio_paths.len() == 1 {
        flags.push("SINGLE_TRACK_FOLDER");
    }
    if folder.audio_paths.len() >= 3 {
        flags.push("LIKELY_ALBUM");
    }
    if folder_audio.iter().any(|entry| entry.invalid) {
        flags.push("HAS_INVALID_AUDIO");
    }
    if folder_audio.iter().any(|entry| entry.deadspot) {
        flags.push("HAS_DEADSPOT_AUDIO");
    }
    if folder_audio.iter().any(|entry| !entry.has_track_number) {
        flags.push("MISSING_TRACK_NUMBERS");
    }

    FolderReportRow {
        folder_path: folder_path.to_string_lossy().to_string(),
        relative_path: relative_display(root, folder_path),
        audio_count: folder.audio_paths.len(),
        image_count: folder.image_paths.len(),
        sidecar_count: folder.sidecar_paths.len(),
        other_count: folder.other_paths.len(),
        child_dir_count: folder.child_dirs.len(),
        folder_flags: flags.join("; "),
        proposed_action: plan.action.clone(),
        apply_eligible: plan.apply_eligible,
        notes: plan.reason.clone(),
    }
}

fn build_group_row(folder_audio: &[&AudioEntry], plan: &GroupPlan) -> GroupPlanRow {
    GroupPlanRow {
        group_key: plan.group_key.clone(),
        audio_count: folder_audio.len(),
        dominant_artist: dominant_value(folder_audio.iter().map(|entry| best_artist(&entry.track))),
        dominant_album: dominant_value(folder_audio.iter().map(|entry| entry.track.album.as_str())),
        dominant_year: dominant_year(folder_audio)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        canonical_folder: plan
            .canonical_folder
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        proposed_action: plan.action.clone(),
        apply_eligible: plan.apply_eligible,
        notes: plan.reason.clone(),
    }
}

fn build_manifest_rows(
    root: &Path,
    quarantine_root: &Path,
    group_key: &str,
    folder: &FolderAccumulator,
    folder_audio: &[&AudioEntry],
    plan: &GroupPlan,
) -> Vec<ManifestRow> {
    let mut rows = Vec::new();
    let mut standard_images = folder
        .image_paths
        .iter()
        .filter(|path| is_standard_art(path))
        .collect::<Vec<_>>();
    standard_images.sort();
    let safe_art_path = if plan.apply_eligible && standard_images.len() == 1 {
        Some(standard_images[0].clone())
    } else {
        None
    };

    for entry in folder_audio {
        let expected = organizer::canonical_path(&root.to_string_lossy(), &entry.track);
        if normalize_path_key(&entry.source_path) == normalize_path_key(&expected) {
            rows.push(ManifestRow {
                group_key: group_key.to_string(),
                item_type: FileKind::Audio.as_str().to_string(),
                action: "IN_PLACE".to_string(),
                apply_eligible: false,
                source_path: entry.source_path.to_string_lossy().to_string(),
                target_path: expected.to_string_lossy().to_string(),
                rollback_source: expected.to_string_lossy().to_string(),
                rollback_target: entry.source_path.to_string_lossy().to_string(),
                reason: "Already at canonical path.".to_string(),
            });
        } else {
            rows.push(ManifestRow {
                group_key: group_key.to_string(),
                item_type: FileKind::Audio.as_str().to_string(),
                action: if plan.apply_eligible {
                    "SAFE_MOVE_AUDIO".to_string()
                } else {
                    "REVIEW_AUDIO".to_string()
                },
                apply_eligible: plan.apply_eligible,
                source_path: entry.source_path.to_string_lossy().to_string(),
                target_path: expected.to_string_lossy().to_string(),
                rollback_source: expected.to_string_lossy().to_string(),
                rollback_target: entry.source_path.to_string_lossy().to_string(),
                reason: plan.reason.clone(),
            });
        }
    }

    for image_path in &folder.image_paths {
        let source_path = image_path.to_string_lossy().to_string();
        if safe_art_path
            .as_ref()
            .map(|path| path == image_path)
            .unwrap_or(false)
        {
            let target_folder = plan
                .canonical_folder
                .clone()
                .unwrap_or_else(|| root.to_path_buf());
            let ext = image_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("jpg")
                .to_ascii_lowercase();
            let target_path = target_folder.join(format!("cover.{ext}"));
            rows.push(ManifestRow {
                group_key: group_key.to_string(),
                item_type: FileKind::Image.as_str().to_string(),
                action: "SAFE_MOVE_ART".to_string(),
                apply_eligible: true,
                source_path: source_path.clone(),
                target_path: target_path.to_string_lossy().to_string(),
                rollback_source: target_path.to_string_lossy().to_string(),
                rollback_target: source_path,
                reason: "Single standard artwork file moves with the safe group.".to_string(),
            });
        } else {
            rows.push(ManifestRow {
                group_key: group_key.to_string(),
                item_type: FileKind::Image.as_str().to_string(),
                action: "REVIEW_IMAGE".to_string(),
                apply_eligible: false,
                source_path: source_path.clone(),
                target_path: String::new(),
                rollback_source: String::new(),
                rollback_target: source_path,
                reason: "Artwork stays review-only unless the group is safe and the art file is unambiguous.".to_string(),
            });
        }
    }

    for sidecar_path in &folder.sidecar_paths {
        let source_path = sidecar_path.to_string_lossy().to_string();
        let ext = sidecar_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if plan.apply_eligible && SAFE_QUARANTINE_EXTENSIONS.contains(&ext.as_str()) {
            let relative_parent = sidecar_path
                .parent()
                .and_then(|value| value.strip_prefix(root).ok())
                .unwrap_or_else(|| Path::new(""));
            let target_path = quarantine_root
                .join("sidecars")
                .join(relative_parent)
                .join(sidecar_path.file_name().unwrap_or_default());
            rows.push(ManifestRow {
                group_key: group_key.to_string(),
                item_type: FileKind::Sidecar.as_str().to_string(),
                action: "SAFE_QUARANTINE_SIDECAR".to_string(),
                apply_eligible: true,
                source_path: source_path.clone(),
                target_path: target_path.to_string_lossy().to_string(),
                rollback_source: target_path.to_string_lossy().to_string(),
                rollback_target: source_path,
                reason: "Low-value sidecar quarantined alongside a safe group.".to_string(),
            });
        } else {
            rows.push(ManifestRow {
                group_key: group_key.to_string(),
                item_type: FileKind::Sidecar.as_str().to_string(),
                action: "REVIEW_SIDECAR".to_string(),
                apply_eligible: false,
                source_path: source_path.clone(),
                target_path: String::new(),
                rollback_source: String::new(),
                rollback_target: source_path,
                reason: "Lyrics, cues, and text sidecars stay review-only by default.".to_string(),
            });
        }
    }

    for other_path in &folder.other_paths {
        let source_path = other_path.to_string_lossy().to_string();
        rows.push(ManifestRow {
            group_key: group_key.to_string(),
            item_type: FileKind::Other.as_str().to_string(),
            action: "REVIEW_OTHER".to_string(),
            apply_eligible: false,
            source_path: source_path.clone(),
            target_path: String::new(),
            rollback_source: String::new(),
            rollback_target: source_path,
            reason: "Unknown file type requires review.".to_string(),
        });
    }

    rows
}

fn downgrade_collisions(rows: &mut [ManifestRow]) {
    let mut targets: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if row.apply_eligible && !row.target_path.is_empty() {
            targets
                .entry(normalize_path_key(Path::new(&row.target_path)))
                .or_default()
                .push(index);
        }
    }

    let mut conflict_indexes = BTreeSet::new();
    for indexes in targets.values() {
        if indexes.len() > 1 {
            conflict_indexes.extend(indexes.iter().copied());
        }
    }

    for index in conflict_indexes {
        let row = &mut rows[index];
        row.apply_eligible = false;
        row.action = "REVIEW_COLLISION".to_string();
        row.reason = format!("Collision on target path: {}", row.target_path);
    }
}

fn build_summary(
    root: &Path,
    output_dir: &Path,
    quarantine_root: &Path,
    folders: &BTreeMap<PathBuf, FolderAccumulator>,
    rows: &[ManifestRow],
) -> ManifestSummary {
    let mut audio_file_count = 0;
    let mut image_file_count = 0;
    let mut sidecar_file_count = 0;
    let mut other_file_count = 0;
    for folder in folders.values() {
        audio_file_count += folder.audio_paths.len();
        image_file_count += folder.image_paths.len();
        sidecar_file_count += folder.sidecar_paths.len();
        other_file_count += folder.other_paths.len();
    }

    let apply_eligible_rows = rows.iter().filter(|row| row.apply_eligible).count();
    let apply_eligible_groups = rows
        .iter()
        .filter(|row| row.apply_eligible)
        .map(|row| row.group_key.as_str())
        .collect::<HashSet<_>>()
        .len();
    let collision_rows = rows
        .iter()
        .filter(|row| row.action == "REVIEW_COLLISION")
        .count();

    ManifestSummary {
        root: root.to_string_lossy().to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        quarantine_root: quarantine_root.to_string_lossy().to_string(),
        folder_count: folders.len(),
        audio_file_count,
        image_file_count,
        sidecar_file_count,
        other_file_count,
        manifest_rows: rows.len(),
        apply_eligible_rows,
        apply_eligible_groups,
        review_rows: rows.len().saturating_sub(apply_eligible_rows),
        collision_rows,
    }
}

fn write_outputs(
    output_dir: &Path,
    folder_rows: &[FolderReportRow],
    group_rows: &[GroupPlanRow],
    manifest_rows: &[ManifestRow],
    summary: &ManifestSummary,
) -> Result<(), String> {
    write_csv(
        &output_dir.join("folder_report.csv"),
        &[
            "folder_path",
            "relative_path",
            "audio_count",
            "image_count",
            "sidecar_count",
            "other_count",
            "child_dir_count",
            "folder_flags",
            "proposed_action",
            "apply_eligible",
            "notes",
        ],
        folder_rows
            .iter()
            .map(|row| {
                vec![
                    row.folder_path.clone(),
                    row.relative_path.clone(),
                    row.audio_count.to_string(),
                    row.image_count.to_string(),
                    row.sidecar_count.to_string(),
                    row.other_count.to_string(),
                    row.child_dir_count.to_string(),
                    row.folder_flags.clone(),
                    row.proposed_action.clone(),
                    row.apply_eligible.to_string(),
                    row.notes.clone(),
                ]
            })
            .collect(),
    )?;

    write_csv(
        &output_dir.join("group_plan.csv"),
        &[
            "group_key",
            "audio_count",
            "dominant_artist",
            "dominant_album",
            "dominant_year",
            "canonical_folder",
            "proposed_action",
            "apply_eligible",
            "notes",
        ],
        group_rows
            .iter()
            .map(|row| {
                vec![
                    row.group_key.clone(),
                    row.audio_count.to_string(),
                    row.dominant_artist.clone(),
                    row.dominant_album.clone(),
                    row.dominant_year.clone(),
                    row.canonical_folder.clone(),
                    row.proposed_action.clone(),
                    row.apply_eligible.to_string(),
                    row.notes.clone(),
                ]
            })
            .collect(),
    )?;

    write_csv(
        &output_dir.join("manifest_rows.csv"),
        &[
            "group_key",
            "item_type",
            "action",
            "apply_eligible",
            "source_path",
            "target_path",
            "rollback_source",
            "rollback_target",
            "reason",
        ],
        manifest_rows
            .iter()
            .map(|row| {
                vec![
                    row.group_key.clone(),
                    row.item_type.clone(),
                    row.action.clone(),
                    row.apply_eligible.to_string(),
                    row.source_path.clone(),
                    row.target_path.clone(),
                    row.rollback_source.clone(),
                    row.rollback_target.clone(),
                    row.reason.clone(),
                ]
            })
            .collect(),
    )?;

    fs::write(
        output_dir.join("manifest_summary.json"),
        serde_json::to_string_pretty(summary).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write manifest_summary.json failed: {e}"))?;

    fs::write(
        output_dir.join("manifest_debrief.txt"),
        render_debrief(summary, manifest_rows),
    )
    .map_err(|e| format!("write manifest_debrief.txt failed: {e}"))?;

    Ok(())
}

fn render_debrief(summary: &ManifestSummary, rows: &[ManifestRow]) -> String {
    let mut action_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows {
        *action_counts.entry(row.action.as_str()).or_default() += 1;
    }

    let mut output = String::new();
    output.push_str("Cassette Library Cleanup Manifest Debrief\n");
    output.push_str("=======================================\n\n");
    output.push_str(&format!("Root: {}\n", summary.root));
    output.push_str(&format!("Output: {}\n", summary.output_dir));
    output.push_str(&format!("Quarantine: {}\n\n", summary.quarantine_root));
    output.push_str(&format!("Folders scanned: {}\n", summary.folder_count));
    output.push_str(&format!("Audio files: {}\n", summary.audio_file_count));
    output.push_str(&format!("Image files: {}\n", summary.image_file_count));
    output.push_str(&format!("Sidecars: {}\n", summary.sidecar_file_count));
    output.push_str(&format!("Other files: {}\n\n", summary.other_file_count));
    output.push_str(&format!("Manifest rows: {}\n", summary.manifest_rows));
    output.push_str(&format!(
        "Apply-eligible rows: {}\n",
        summary.apply_eligible_rows
    ));
    output.push_str(&format!(
        "Apply-eligible groups: {}\n",
        summary.apply_eligible_groups
    ));
    output.push_str(&format!("Review rows: {}\n", summary.review_rows));
    output.push_str(&format!("Collision rows: {}\n\n", summary.collision_rows));
    output.push_str("Action counts\n");
    output.push_str("-------------\n");
    for (action, count) in action_counts {
        output.push_str(&format!("{action}: {count}\n"));
    }
    output.push_str("\nNext steps\n");
    output.push_str("----------\n");
    output.push_str("1. Inspect group_plan.csv before applying anything.\n");
    output
        .push_str("2. Only SAFE_* rows are apply-eligible; REVIEW_* rows need human decisions.\n");
    output
        .push_str("3. Apply with scripts/apply_cleanup_manifest.ps1 after checking collisions.\n");
    output
}

fn load_audit_report(path: PathBuf) -> AuditReport {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<AuditReport>(&content).ok())
        .unwrap_or_default()
}

fn load_deadspot_report(path: PathBuf) -> DeadspotReport {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<DeadspotReport>(&content).ok())
        .unwrap_or_default()
}

fn build_issue_map(findings: &[AuditSizeFinding]) -> HashMap<String, Vec<String>> {
    findings
        .iter()
        .map(|finding| {
            let mut reasons = Vec::new();
            reasons.push(finding.status.clone());
            reasons.extend(finding.reasons.clone());
            (normalize_path_key(Path::new(&finding.path)), reasons)
        })
        .collect()
}

fn build_deadspot_map(findings: &[DeadspotFinding]) -> HashMap<String, Vec<String>> {
    findings
        .iter()
        .map(|finding| {
            (
                normalize_path_key(Path::new(&finding.path)),
                finding.reasons.clone(),
            )
        })
        .collect()
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|value| {
            if value.as_os_str().is_empty() {
                Some(".".to_string())
            } else {
                Some(value.to_string_lossy().to_string())
            }
        })
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn normalize_path_key(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn best_artist(track: &Track) -> &str {
    if !track.album_artist.trim().is_empty() {
        track.album_artist.as_str()
    } else {
        track.artist.as_str()
    }
}

fn dominant_value<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut originals = BTreeMap::new();
    for value in values {
        let key = normalize_token(value);
        if key.is_empty() {
            continue;
        }
        *counts.entry(key.clone()).or_default() += 1;
        originals
            .entry(key)
            .or_insert_with(|| value.trim().to_string());
    }
    counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        .and_then(|(key, _)| originals.get(&key).cloned())
        .unwrap_or_default()
}

fn dominant_year(folder_audio: &[&AudioEntry]) -> Option<i32> {
    let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
    for year in folder_audio.iter().filter_map(|entry| entry.track.year) {
        if year > 0 {
            *counts.entry(year).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        .map(|(year, _)| year)
}

fn distinct_count<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len()
}

fn is_suspicious_folder_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() <= 2 {
        return true;
    }
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return true;
    }
    trimmed.starts_with('.')
}

fn is_standard_art(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    STANDARD_ART_BASENAMES.contains(&file_name.as_str())
}

fn write_csv(path: &Path, header: &[&str], rows: Vec<Vec<String>>) -> Result<(), String> {
    let mut content = String::new();
    content.push_str(
        &header
            .iter()
            .map(|value| csv_escape(value))
            .collect::<Vec<_>>()
            .join(","),
    );
    content.push('\n');
    for row in rows {
        content.push_str(
            &row.iter()
                .map(|value| csv_escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        content.push('\n');
    }
    fs::write(path, content).map_err(|e| format!("write {} failed: {e}", path.display()))
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
