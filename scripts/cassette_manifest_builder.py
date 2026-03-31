from __future__ import annotations

import argparse
import csv
import hashlib
import json
import logging
import os
import re
import shutil
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable

LOGGER = logging.getLogger("cassette_manifest_builder")

AUDIO_EXTENSIONS = {
    ".flac",
    ".mp3",
    ".m4a",
    ".aac",
    ".wav",
    ".ogg",
    ".opus",
    ".alac",
    ".wma",
    ".aiff",
    ".wv",
    ".ape",
}

IMAGE_EXTENSIONS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".gif"}
SIDECAR_EXTENSIONS = {".lrc", ".txt", ".nfo", ".cue", ".log", ".sfv", ".m3u", ".m3u8"}

KNOWN_ART_NAMES = {
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "album.jpg",
    "album.jpeg",
    "album.png",
}

TRACK_PREFIX_RE = re.compile(r"^(?P<num>\d{1,2})(?:-(?P<disc_track>\d{2}))?[\s._-]+(?P<title>.+)$")
DISC_TRACK_RE = re.compile(r"^(?P<disc>\d{2})-(?P<track>\d{2})[\s._-]+(?P<title>.+)$")
YEAR_PREFIX_RE = re.compile(r"^\((?P<year>\d{4})\)\s+(?P<album>.+)$")
YEAR_SUFFIX_RE = re.compile(r"^(?P<album>.+?)\s+\((?P<year>\d{4})\)$")
YEAR_HYPHEN_RE = re.compile(r"^(?P<year>\d{4})\s+-\s+(?P<album>.+)$")
LONG_NUMERIC_SUFFIX_RE = re.compile(r"[_-]\d{8,}$")


@dataclass(frozen=True)
class AudioTagData:
    artist: str = ""
    album_artist: str = ""
    album: str = ""
    title: str = ""
    year: str = ""
    track_number: int | None = None
    disc_number: int | None = None
    source: str = "path"


@dataclass(frozen=True)
class FileInventoryRow:
    source_path: str
    relative_path: str
    parent_rel: str
    file_name: str
    extension: str
    kind: str
    guessed_artist: str
    guessed_album: str
    guessed_title: str
    guessed_year: str
    guessed_track_number: str
    guessed_disc_number: str
    metadata_source: str
    flags: str


@dataclass(frozen=True)
class FolderRecord:
    relative_path: str
    folder_name: str
    parent_rel: str
    depth: int
    audio_count: int
    image_count: int
    sidecar_count: int
    other_count: int
    child_folder_count: int
    folder_flags: str
    classification: str


@dataclass(frozen=True)
class GroupPlan:
    group_key: str
    parent_rel: str
    item_class: str
    artist: str
    album: str
    year: str
    confidence: str
    action: str
    proposed_folder_rel: str
    notes: str
    audio_count: int
    metadata_sources: str


@dataclass
class ManifestRow:
    action: str
    confidence: str
    source_path: str
    target_path: str
    rollback_source: str
    rollback_target: str
    item_type: str
    reason: str
    group_key: str


@dataclass(frozen=True)
class CollisionRow:
    target_path: str
    count: int
    source_paths: str
    actions: str


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def setup_logging(verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(level=level, format="%(levelname)s %(message)s")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a dry-run-first Cassette library cleanup manifest."
    )
    parser.add_argument("source_root", help="Library root to scan.")
    parser.add_argument(
        "--output-dir",
        default="tmp/cassette_manifest_output",
        help="Directory for CSV/JSON/TXT outputs.",
    )
    parser.add_argument(
        "--layout",
        choices=("legacy-organizer", "custodian", "year-prefix"),
        default="legacy-organizer",
        help=(
            "Target path layout. "
            "legacy-organizer = Artist/Album (Year)/NN[-NN] - Title.ext, "
            "custodian = Artist/Album/NN - Title.ext, "
            "year-prefix = Artist/Year - Album/NN - Title.ext"
        ),
    )
    parser.add_argument(
        "--tags",
        choices=("auto", "always", "never"),
        default="auto",
        help="Tag reading mode. auto uses mutagen if installed.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=min(32, (os.cpu_count() or 8)),
        help="Concurrent metadata workers.",
    )
    parser.add_argument(
        "--execute-safe",
        action="store_true",
        help="Execute SAFE_RENAME_ALBUM and SAFE_RENAME_ART rows.",
    )
    parser.add_argument(
        "--execute-sidecars",
        action="store_true",
        help="Execute QUARANTINE_SIDECAR rows in addition to safe renames.",
    )
    parser.add_argument(
        "--quarantine-folder-name",
        default="_Cassette_Quarantine",
        help="Folder name created under source root for quarantined sidecars.",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging.")
    return parser.parse_args()


def now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def sanitize_component(value: str, fallback: str = "Unknown") -> str:
    if not value:
        return fallback
    sanitized = LONG_NUMERIC_SUFFIX_RE.sub("", value)
    sanitized = sanitized.replace("/", "~").replace("\\", "~").replace(":", "-")
    sanitized = re.sub(r'[<>\"|?*]', "", sanitized)
    sanitized = re.sub(r"\s+", " ", sanitized).strip().rstrip(". ")
    upper = sanitized.upper()
    if upper in {"CON", "PRN", "AUX", "NUL", "COM1", "LPT1"}:
        sanitized = f"_{sanitized}"
    return sanitized or fallback


def path_depth(relative_path: str) -> int:
    if relative_path in ("", "."):
        return 0
    return len(Path(relative_path).parts)


def split_number_component(raw_value: str) -> int | None:
    if not raw_value:
        return None
    match = re.search(r"\d{1,2}", raw_value)
    if match:
        return int(match.group(0))
    return None


def normalize_title_from_name(file_name: str) -> str:
    stem = Path(file_name).stem
    stem = LONG_NUMERIC_SUFFIX_RE.sub("", stem)
    disc_match = DISC_TRACK_RE.match(stem)
    if disc_match:
        stem = disc_match.group("title")
    else:
        track_match = TRACK_PREFIX_RE.match(stem)
        if track_match:
            stem = track_match.group("title")
    stem = re.sub(r"\s+", " ", stem).strip()
    return sanitize_component(stem or "Untitled", fallback="Untitled")


def infer_from_parent(parent_rel: str, file_name: str) -> AudioTagData:
    parts = Path(parent_rel).parts if parent_rel not in ("", ".") else ()
    artist = parts[-2] if len(parts) >= 2 else ""
    album_folder = parts[-1] if len(parts) >= 1 else ""
    year = ""
    album = album_folder

    for pattern in (YEAR_PREFIX_RE, YEAR_SUFFIX_RE, YEAR_HYPHEN_RE):
        match = pattern.match(album_folder)
        if match:
            year = match.group("year")
            album = match.group("album")
            break

    disc_number = None
    track_number = None
    stem = Path(file_name).stem
    disc_match = DISC_TRACK_RE.match(stem)
    if disc_match:
        disc_number = int(disc_match.group("disc"))
        track_number = int(disc_match.group("track"))
    else:
        track_match = TRACK_PREFIX_RE.match(stem)
        if track_match:
            track_number = int(track_match.group("num"))

    return AudioTagData(
        artist=sanitize_component(artist, fallback=""),
        album_artist="",
        album=sanitize_component(album, fallback=""),
        title=normalize_title_from_name(file_name),
        year=year,
        track_number=track_number,
        disc_number=disc_number,
        source="path",
    )


def parse_mutagen_tags(file_path: Path) -> AudioTagData | None:
    try:
        from mutagen import File as MutagenFile
    except ImportError:
        return None

    try:
        tagged = MutagenFile(file_path, easy=True)
    except Exception as exc:
        LOGGER.debug("mutagen failed for %s: %s", file_path, exc)
        return None

    if tagged is None:
        return None

    tags = tagged.tags or {}

    def first_value(*keys: str) -> str:
        for key in keys:
            value = tags.get(key)
            if isinstance(value, list) and value:
                return str(value[0]).strip()
            if isinstance(value, str):
                return value.strip()
        return ""

    year = first_value("date", "originaldate", "year")
    year_match = re.search(r"\b(\d{4})\b", year)
    year = year_match.group(1) if year_match else ""

    track_number = split_number_component(first_value("tracknumber"))
    disc_number = split_number_component(first_value("discnumber"))

    return AudioTagData(
        artist=sanitize_component(first_value("artist"), fallback=""),
        album_artist=sanitize_component(first_value("albumartist"), fallback=""),
        album=sanitize_component(first_value("album"), fallback=""),
        title=sanitize_component(first_value("title"), fallback=""),
        year=year,
        track_number=track_number,
        disc_number=disc_number,
        source="tags",
    )


def read_audio_metadata(file_path: Path, relative_path: Path, tags_mode: str) -> FileInventoryRow:
    extension = file_path.suffix.lower()
    parent_rel = str(relative_path.parent) if relative_path.parent != Path(".") else "."
    fallback = infer_from_parent(parent_rel, file_path.name)

    tag_data: AudioTagData | None = None
    if tags_mode != "never":
        tag_data = parse_mutagen_tags(file_path)
        if tags_mode == "always" and tag_data is None:
            LOGGER.debug("tag read unavailable for %s under --tags always", file_path)

    effective = tag_data or fallback
    artist = effective.album_artist or effective.artist
    flags: list[str] = []

    if tag_data is None:
        flags.append("PATH_INFERENCE_ONLY")
    if not artist:
        flags.append("MISSING_ARTIST")
    if not effective.album:
        flags.append("MISSING_ALBUM")
    if not effective.title:
        flags.append("MISSING_TITLE")
    if effective.track_number is None:
        flags.append("MISSING_TRACK_NUMBER")
    if effective.year == "":
        flags.append("MISSING_YEAR")

    return FileInventoryRow(
        source_path=str(file_path),
        relative_path=str(relative_path),
        parent_rel=parent_rel,
        file_name=file_path.name,
        extension=extension,
        kind="audio",
        guessed_artist=artist,
        guessed_album=effective.album,
        guessed_title=effective.title or normalize_title_from_name(file_path.name),
        guessed_year=effective.year,
        guessed_track_number=f"{effective.track_number:02}" if effective.track_number is not None else "",
        guessed_disc_number=f"{effective.disc_number:02}" if effective.disc_number is not None else "",
        metadata_source=effective.source,
        flags="; ".join(sorted(flags)),
    )


def read_non_audio_inventory(file_path: Path, relative_path: Path) -> FileInventoryRow:
    extension = file_path.suffix.lower()
    parent_rel = str(relative_path.parent) if relative_path.parent != Path(".") else "."
    flags: list[str] = []
    if extension in IMAGE_EXTENSIONS:
        kind = "image"
        if file_path.name.lower() in KNOWN_ART_NAMES:
            flags.append("STANDARD_ART")
        if LONG_NUMERIC_SUFFIX_RE.search(file_path.stem):
            flags.append("HASHY_ART_NAME")
    elif extension in SIDECAR_EXTENSIONS:
        kind = "sidecar"
        flags.append("SIDECAR_FILE")
    else:
        kind = "other"
        flags.append("UNCOMMON_FILETYPE")

    return FileInventoryRow(
        source_path=str(file_path),
        relative_path=str(relative_path),
        parent_rel=parent_rel,
        file_name=file_path.name,
        extension=extension,
        kind=kind,
        guessed_artist="",
        guessed_album="",
        guessed_title="",
        guessed_year="",
        guessed_track_number="",
        guessed_disc_number="",
        metadata_source="none",
        flags="; ".join(sorted(flags)),
    )


def discover_files(source_root: Path) -> list[Path]:
    files: list[Path] = []
    for path in sorted(source_root.rglob("*")):
        if path.is_file():
            files.append(path)
    return files


def build_inventory(source_root: Path, tags_mode: str, workers: int) -> list[FileInventoryRow]:
    files = discover_files(source_root)
    LOGGER.info("Discovered %s files under %s", len(files), source_root)

    def classify(file_path: Path) -> FileInventoryRow:
        relative_path = file_path.relative_to(source_root)
        if file_path.suffix.lower() in AUDIO_EXTENSIONS:
            return read_audio_metadata(file_path, relative_path, tags_mode)
        return read_non_audio_inventory(file_path, relative_path)

    with ThreadPoolExecutor(max_workers=max(1, workers)) as executor:
        rows = list(executor.map(classify, files))

    rows.sort(key=lambda row: row.source_path.lower())
    return rows


def build_folder_records(source_root: Path, inventory: list[FileInventoryRow]) -> list[FolderRecord]:
    stats: dict[str, dict[str, object]] = defaultdict(
        lambda: {
            "audio": 0,
            "image": 0,
            "sidecar": 0,
            "other": 0,
            "flags": set(),
            "children": set(),
        }
    )

    all_dirs = {".": True}
    for row in inventory:
        parent = row.parent_rel
        all_dirs[parent] = True
        current = Path(parent)
        while current != Path(".") and str(current) not in ("", "."):
            all_dirs[str(current)] = True
            current = current.parent
        bucket = stats[parent]
        bucket[row.kind] = int(bucket[row.kind]) + 1
        if row.flags:
            for flag in row.flags.split("; "):
                if flag:
                    cast_flags = bucket["flags"]
                    assert isinstance(cast_flags, set)
                    cast_flags.add(flag)

        child = Path(parent).name if parent not in ("", ".") else ""
        parent_of_parent = str(Path(parent).parent) if parent not in ("", ".") else ""
        if parent_of_parent == "":
            parent_of_parent = "."
        if child:
            cast_children = stats[parent_of_parent]["children"]
            assert isinstance(cast_children, set)
            cast_children.add(child)

    folder_records: list[FolderRecord] = []
    for relative_path in sorted(all_dirs.keys(), key=lambda item: (path_depth(item), item.lower())):
        folder_name = source_root.name if relative_path == "." else Path(relative_path).name
        parent_rel = "" if relative_path == "." else str(Path(relative_path).parent)
        if parent_rel == "":
            parent_rel = "."
        bucket = stats[relative_path]
        flags = set(bucket["flags"])
        audio_count = int(bucket["audio"])
        image_count = int(bucket["image"])
        sidecar_count = int(bucket["sidecar"])
        other_count = int(bucket["other"])
        child_folder_count = len(bucket["children"])

        if audio_count >= 3:
            classification = "ALBUMISH"
            flags.add("LIKELY_ALBUM")
        elif audio_count == 1:
            classification = "SINGLEISH"
            flags.add("SINGLE_TRACK_FOLDER")
        elif audio_count == 0 and (sidecar_count > 0 or image_count > 0):
            classification = "JUNKISH"
            flags.add("NO_AUDIO_CONTENT")
        else:
            classification = "NEUTRAL"

        if folder_name.lower() == "unknown album":
            flags.add("UNKNOWN_ALBUM_FOLDER")
        if re.fullmatch(r"\d+", folder_name):
            flags.add("NUMBER_ONLY_FOLDER")
        if re.fullmatch(r".{1,2}", folder_name):
            flags.add("VERY_SHORT_FOLDER_NAME")
        if folder_name.startswith("."):
            flags.add("DOT_PREFIX_FOLDER")
        if YEAR_PREFIX_RE.match(folder_name):
            flags.add("YEAR_PREFIX_FORMAT")
        if YEAR_SUFFIX_RE.match(folder_name):
            flags.add("YEAR_SUFFIX_FORMAT")
        if YEAR_HYPHEN_RE.match(folder_name):
            flags.add("YEAR_HYPHEN_FORMAT")

        folder_records.append(
            FolderRecord(
                relative_path=relative_path,
                folder_name=folder_name,
                parent_rel=parent_rel,
                depth=path_depth(relative_path),
                audio_count=audio_count,
                image_count=image_count,
                sidecar_count=sidecar_count,
                other_count=other_count,
                child_folder_count=child_folder_count,
                folder_flags="; ".join(sorted(flags)),
                classification=classification,
            )
        )

    return folder_records


def choose_mode_confidence(
    item_class: str,
    metadata_sources: Counter[str],
    audio_rows: list[FileInventoryRow],
) -> tuple[str, str, list[str]]:
    notes: list[str] = []
    path_inference_only = metadata_sources.get("path", 0) == len(audio_rows)
    missing_track_numbers = any(not row.guessed_track_number for row in audio_rows)
    missing_album = any(not row.guessed_album for row in audio_rows)
    missing_artist = any(not row.guessed_artist for row in audio_rows)

    if path_inference_only:
        notes.append("All audio targets derived from path inference.")
    if missing_track_numbers:
        notes.append("At least one track is missing a track number.")
    if missing_album:
        notes.append("At least one track is missing album metadata.")
    if missing_artist:
        notes.append("At least one track is missing artist metadata.")

    if item_class == "single":
        if path_inference_only or missing_artist:
            return "REVIEW", "LOW", notes
        if missing_track_numbers:
            notes.append("Single normalized by title-only target.")
        return "RENAME_SINGLE", "MEDIUM", notes

    if path_inference_only or missing_album or missing_artist:
        return "REVIEW", "LOW", notes
    if missing_track_numbers:
        return "RENAME_ALBUM", "MEDIUM", notes
    return "SAFE_RENAME_ALBUM", "HIGH", notes


def build_target_folder_rel(layout: str, artist: str, album: str, year: str, item_class: str) -> str:
    artist_component = sanitize_component(artist, fallback="Unknown Artist")
    album_component = sanitize_component(album, fallback="Unknown Album")
    if item_class == "single":
        return str(Path(artist_component) / "Singles")
    if layout == "custodian":
        return str(Path(artist_component) / album_component)
    if layout == "year-prefix":
        album_folder = f"{year} - {album_component}" if year else album_component
        return str(Path(artist_component) / album_folder)
    album_folder = f"{album_component} ({year})" if year else album_component
    return str(Path(artist_component) / album_folder)


def pick_dominant(values: Iterable[str]) -> str:
    filtered = [value for value in values if value]
    if not filtered:
        return ""
    counts = Counter(filtered)
    return sorted(counts.items(), key=lambda item: (-item[1], item[0].lower()))[0][0]


def build_group_plans(inventory: list[FileInventoryRow], layout: str) -> list[GroupPlan]:
    by_parent: dict[str, list[FileInventoryRow]] = defaultdict(list)
    for row in inventory:
        by_parent[row.parent_rel].append(row)

    plans: list[GroupPlan] = []
    for parent_rel in sorted(by_parent.keys(), key=lambda item: (path_depth(item), item.lower())):
        rows = by_parent[parent_rel]
        audio_rows = sorted((row for row in rows if row.kind == "audio"), key=lambda row: row.source_path.lower())
        if not audio_rows:
            continue

        item_class = "single" if len(audio_rows) == 1 else "album"
        dominant_artist = pick_dominant(row.guessed_artist for row in audio_rows)
        dominant_album = pick_dominant(row.guessed_album for row in audio_rows)
        dominant_year = pick_dominant(row.guessed_year for row in audio_rows)
        metadata_sources = Counter(row.metadata_source for row in audio_rows)

        action, confidence, notes = choose_mode_confidence(item_class, metadata_sources, audio_rows)
        if item_class == "single" and not dominant_album:
            dominant_album = audio_rows[0].guessed_title

        proposed_folder_rel = build_target_folder_rel(
            layout,
            dominant_artist,
            dominant_album,
            dominant_year,
            item_class,
        )
        plans.append(
            GroupPlan(
                group_key=parent_rel,
                parent_rel=parent_rel,
                item_class=item_class,
                artist=dominant_artist,
                album=dominant_album,
                year=dominant_year,
                confidence=confidence,
                action=action,
                proposed_folder_rel=proposed_folder_rel,
                notes="; ".join(notes) if notes else "Group plan generated.",
                audio_count=len(audio_rows),
                metadata_sources=json.dumps(dict(sorted(metadata_sources.items())), sort_keys=True),
            )
        )

    return plans


def build_audio_target_name(plan: GroupPlan, row: FileInventoryRow, layout: str) -> str:
    ext = row.extension.lower()
    title = sanitize_component(row.guessed_title or normalize_title_from_name(row.file_name), fallback="Untitled")
    if plan.item_class == "single":
        if plan.year:
            return sanitize_component(f"{plan.year} - {title}", fallback="Untitled") + ext
        return title + ext

    if row.guessed_disc_number and row.guessed_disc_number != "01" and layout == "legacy-organizer":
        if row.guessed_track_number:
            return f"{row.guessed_disc_number}-{row.guessed_track_number} - {title}{ext}"
    if row.guessed_track_number:
        return f"{row.guessed_track_number} - {title}{ext}"
    return f"{title}{ext}"


def paths_equivalent(source_path: str, target_path: str) -> bool:
    try:
        source = Path(source_path).resolve(strict=False)
        target = Path(target_path).resolve(strict=False)
        return str(source).lower() == str(target).lower()
    except Exception:
        return source_path.lower() == target_path.lower()


def build_manifest(
    source_root: Path,
    inventory: list[FileInventoryRow],
    plans: list[GroupPlan],
    quarantine_folder_name: str,
    layout: str,
) -> list[ManifestRow]:
    rows: list[ManifestRow] = []
    plan_by_parent = {plan.parent_rel: plan for plan in plans}

    image_groups: dict[str, list[FileInventoryRow]] = defaultdict(list)
    for row in inventory:
        if row.kind == "image":
            image_groups[row.parent_rel].append(row)

    standard_art_source: dict[str, str] = {}
    for parent_rel, group in image_groups.items():
        standard = sorted(
            (row for row in group if "STANDARD_ART" in row.flags),
            key=lambda row: row.file_name.lower(),
        )
        if len(standard) == 1:
            standard_art_source[parent_rel] = standard[0].source_path

    for row in inventory:
        plan = plan_by_parent.get(row.parent_rel)
        action = "SKIP"
        confidence = "LOW"
        target_path = row.source_path
        reason = "No action generated."
        group_key = row.parent_rel

        if plan and row.kind == "audio":
            target_folder = source_root / plan.proposed_folder_rel
            target_name = build_audio_target_name(plan, row, layout)
            target_path = str(target_folder / target_name)
            action = plan.action
            confidence = plan.confidence
            reason = f"{plan.item_class} normalized using {plan.confidence.lower()}-confidence {row.metadata_source} metadata."
        elif plan and row.kind == "image":
            target_folder = source_root / plan.proposed_folder_rel
            if standard_art_source.get(row.parent_rel) == row.source_path:
                target_path = str(target_folder / "cover.jpg")
                action = "SAFE_RENAME_ART" if plan.action.startswith("SAFE_") else "RENAME_ART"
                confidence = plan.confidence
                reason = "Single standard artwork file normalized to cover.jpg."
            else:
                action = "REVIEW"
                target_path = row.source_path
                reason = "Artwork is non-standard or ambiguous within its folder."
        elif row.kind == "sidecar":
            quarantine_rel = str(
                Path(quarantine_folder_name)
                / sanitize_component(row.parent_rel.replace("\\", "__").replace("/", "__"), fallback="root")
            )
            target_path = str(source_root / quarantine_rel / row.file_name)
            action = "QUARANTINE_SIDECAR"
            confidence = "MEDIUM"
            reason = "Sidecar moved to quarantine review bucket."
        elif row.kind == "other":
            action = "REVIEW"
            target_path = row.source_path
            reason = "Uncommon file type requires review."

        if action not in {"REVIEW", "SKIP"} and paths_equivalent(row.source_path, target_path):
            action = "SKIP"
            confidence = "LOW"
            reason = "Already matches the proposed canonical target."

        rows.append(
            ManifestRow(
                action=action,
                confidence=confidence,
                source_path=row.source_path,
                target_path=target_path,
                rollback_source=target_path,
                rollback_target=row.source_path,
                item_type=row.kind,
                reason=reason,
                group_key=group_key,
            )
        )

    return rows


def detect_collisions(manifest_rows: list[ManifestRow]) -> list[CollisionRow]:
    grouped: dict[str, list[ManifestRow]] = defaultdict(list)
    for row in manifest_rows:
        if row.action not in {"SKIP", "REVIEW"} and row.target_path:
            grouped[row.target_path.lower()].append(row)

    collisions: list[CollisionRow] = []
    for target_key in sorted(grouped.keys()):
        group = grouped[target_key]
        if len(group) > 1:
            collisions.append(
                CollisionRow(
                    target_path=group[0].target_path,
                    count=len(group),
                    source_paths=" | ".join(item.source_path for item in group),
                    actions=" | ".join(item.action for item in group),
                )
            )
    return collisions


def build_duplicate_quarantine_target(
    source_root: Path,
    quarantine_folder_name: str,
    source_path: str,
) -> str:
    source = Path(source_path)
    try:
        relative_parent = source.parent.relative_to(source_root)
        bucket = sanitize_component(
            str(relative_parent).replace("\\", "__").replace("/", "__"),
            fallback="root",
        )
    except ValueError:
        bucket = "root"
    target_dir = source_root / quarantine_folder_name / "duplicate_collisions" / bucket
    return str(target_dir / source.name)


def resolve_exact_duplicate_collisions(
    manifest_rows: list[ManifestRow],
    source_root: Path,
    quarantine_folder_name: str,
) -> None:
    grouped: dict[str, list[ManifestRow]] = defaultdict(list)
    for row in manifest_rows:
        if row.action.startswith("SAFE_") and row.target_path:
            grouped[row.target_path.lower()].append(row)

    hash_cache: dict[str, tuple[int, str] | None] = {}

    def fingerprint(path_text: str) -> tuple[int, str] | None:
        if path_text in hash_cache:
            return hash_cache[path_text]
        path = Path(path_text)
        if not path.exists() or not path.is_file():
            hash_cache[path_text] = None
            return None
        result = (path.stat().st_size, hash_file(path))
        hash_cache[path_text] = result
        return result

    for target_key, rows in grouped.items():
        if len(rows) <= 1:
            continue
        if not all(row.action in {"SAFE_RENAME_ALBUM", "SAFE_RENAME_ART"} for row in rows):
            continue

        fingerprints = [fingerprint(row.source_path) for row in rows]
        if any(value is None for value in fingerprints):
            continue
        if len(set(fingerprints)) != 1:
            continue

        keeper = min(rows, key=lambda row: (row.source_path.lower(), row.item_type, row.target_path.lower()))
        for row in rows:
            if row is keeper:
                row.reason = f"{row.reason} Exact duplicate collision resolved; keeper selected deterministically."
                continue

            row.action = "SAFE_QUARANTINE_DUPLICATE"
            row.confidence = "HIGH"
            row.reason = "Exact duplicate collision detected; duplicate quarantined."
            row.target_path = build_duplicate_quarantine_target(
                source_root=source_root,
                quarantine_folder_name=quarantine_folder_name,
                source_path=row.source_path,
            )
            row.rollback_source = row.target_path
            row.rollback_target = row.source_path


def downgrade_collisions(manifest_rows: list[ManifestRow], collisions: list[CollisionRow]) -> None:
    conflict_targets = {row.target_path.lower() for row in collisions}
    for row in manifest_rows:
        if row.target_path.lower() in conflict_targets and row.action.startswith("SAFE_"):
            row.action = "REVIEW"
            row.confidence = "LOW"
            row.reason = f"{row.reason} Target collision detected."


def write_csv(path: Path, rows: list[dict], fieldnames: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow(row)


def write_summary(
    path: Path,
    source_root: Path,
    layout: str,
    tags_mode: str,
    plans: list[GroupPlan],
    manifest_rows: list[ManifestRow],
    collisions: list[CollisionRow],
    execute_safe: bool,
    execute_sidecars: bool,
) -> None:
    action_counts = Counter(row.action for row in manifest_rows)
    confidence_counts = Counter(row.confidence for row in manifest_rows)
    plan_counts = Counter(plan.action for plan in plans)

    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        handle.write("CASSETTE MANIFEST DEBRIEF\n")
        handle.write("=" * 72 + "\n\n")
        handle.write(f"source_root: {source_root}\n")
        handle.write(f"layout: {layout}\n")
        handle.write(f"tags_mode: {tags_mode}\n")
        handle.write(f"execute_safe: {execute_safe}\n")
        handle.write(f"execute_sidecars: {execute_sidecars}\n\n")

        handle.write("PLAN COUNTS\n")
        handle.write("-" * 72 + "\n")
        for key, value in sorted(plan_counts.items()):
            handle.write(f"{key}: {value}\n")
        handle.write("\n")

        handle.write("MANIFEST ACTION COUNTS\n")
        handle.write("-" * 72 + "\n")
        for key, value in sorted(action_counts.items()):
            handle.write(f"{key}: {value}\n")
        handle.write("\n")

        handle.write("MANIFEST CONFIDENCE COUNTS\n")
        handle.write("-" * 72 + "\n")
        for key, value in sorted(confidence_counts.items()):
            handle.write(f"{key}: {value}\n")
        handle.write("\n")

        handle.write("TARGET COLLISIONS\n")
        handle.write("-" * 72 + "\n")
        handle.write(f"collision_groups: {len(collisions)}\n")
        for row in collisions[:25]:
            handle.write(f"{row.count}x -> {row.target_path}\n")
        handle.write("\n")

        handle.write("NEXT MOVES\n")
        handle.write("-" * 72 + "\n")
        handle.write("1. Review REVIEW rows before any live run.\n")
        handle.write("2. Review target collisions before enabling --execute-safe.\n")
        handle.write("3. Tag-less or path-inferred groups should stay review-only.\n")
        handle.write("4. Sidecars are quarantined by plan, not deleted.\n")


def execute_manifest(
    manifest_rows: list[ManifestRow],
    execute_safe: bool,
    execute_sidecars: bool,
) -> list[dict]:
    execution_log: list[dict] = []
    for row in manifest_rows:
        should_execute = row.action in {
            "SAFE_RENAME_ALBUM",
            "SAFE_RENAME_ART",
            "SAFE_QUARANTINE_DUPLICATE",
        }
        if row.action == "QUARANTINE_SIDECAR" and execute_sidecars:
            should_execute = True

        if not should_execute:
            execution_log.append(
                {
                    "status": "SKIPPED",
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "action": row.action,
                    "message": "Not eligible for execution.",
                }
            )
            continue

        if not execute_safe:
            execution_log.append(
                {
                    "status": "DRY_RUN",
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "action": row.action,
                    "message": "Execution disabled.",
                }
            )
            continue

        source = Path(row.source_path)
        target = Path(row.target_path)
        if not source.exists():
            execution_log.append(
                {
                    "status": "ERROR",
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "action": row.action,
                    "message": "Source missing.",
                }
            )
            continue
        if target.exists():
            execution_log.append(
                {
                    "status": "ERROR",
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "action": row.action,
                    "message": "Target already exists.",
                }
            )
            continue

        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            shutil.move(str(source), str(target))
            execution_log.append(
                {
                    "status": "MOVED",
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "action": row.action,
                    "message": "Move completed.",
                }
            )
        except Exception as exc:
            execution_log.append(
                {
                    "status": "ERROR",
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "action": row.action,
                    "message": f"Move failed: {exc}",
                }
            )

    return execution_log


def maybe_normalize_tags_mode(tags_mode: str) -> str:
    if tags_mode == "never":
        return tags_mode
    try:
        import mutagen  # noqa: F401
    except ImportError:
        if tags_mode == "always":
            raise RuntimeError("mutagen is required for --tags always but is not installed.")
        LOGGER.info("mutagen not installed; falling back to path inference")
        return "never"
    return tags_mode


def main() -> int:
    args = parse_args()
    setup_logging(args.verbose)

    source_root = Path(args.source_root).expanduser().resolve()
    output_dir = Path(args.output_dir).expanduser().resolve()
    if not source_root.exists():
        raise FileNotFoundError(f"Source root does not exist: {source_root}")

    tags_mode = maybe_normalize_tags_mode(args.tags)
    output_dir.mkdir(parents=True, exist_ok=True)

    inventory = build_inventory(source_root, tags_mode, args.workers)
    folder_records = build_folder_records(source_root, inventory)
    plans = build_group_plans(inventory, args.layout)
    manifest_rows = build_manifest(
        source_root,
        inventory,
        plans,
        args.quarantine_folder_name,
        args.layout,
    )
    resolve_exact_duplicate_collisions(
        manifest_rows,
        source_root=source_root,
        quarantine_folder_name=args.quarantine_folder_name,
    )
    collisions = detect_collisions(manifest_rows)
    downgrade_collisions(manifest_rows, collisions)
    execution_log = execute_manifest(
        manifest_rows,
        execute_safe=args.execute_safe,
        execute_sidecars=args.execute_sidecars,
    )

    write_csv(
        output_dir / "files_inventory.csv",
        [asdict(row) for row in inventory],
        [
            "source_path",
            "relative_path",
            "parent_rel",
            "file_name",
            "extension",
            "kind",
            "guessed_artist",
            "guessed_album",
            "guessed_title",
            "guessed_year",
            "guessed_track_number",
            "guessed_disc_number",
            "metadata_source",
            "flags",
        ],
    )
    write_csv(
        output_dir / "folder_records.csv",
        [asdict(row) for row in folder_records],
        [
            "relative_path",
            "folder_name",
            "parent_rel",
            "depth",
            "audio_count",
            "image_count",
            "sidecar_count",
            "other_count",
            "child_folder_count",
            "folder_flags",
            "classification",
        ],
    )
    write_csv(
        output_dir / "group_plans.csv",
        [asdict(row) for row in plans],
        [
            "group_key",
            "parent_rel",
            "item_class",
            "artist",
            "album",
            "year",
            "confidence",
            "action",
            "proposed_folder_rel",
            "notes",
            "audio_count",
            "metadata_sources",
        ],
    )
    write_csv(
        output_dir / "rename_manifest.csv",
        [asdict(row) for row in manifest_rows],
        [
            "action",
            "confidence",
            "source_path",
            "target_path",
            "rollback_source",
            "rollback_target",
            "item_type",
            "reason",
            "group_key",
        ],
    )
    write_csv(
        output_dir / "rollback_manifest.csv",
        [
            {
                "source_path": row.rollback_source,
                "target_path": row.rollback_target,
                "action": row.action,
            }
            for row in manifest_rows
            if row.action not in {"SKIP", "REVIEW"}
        ],
        ["source_path", "target_path", "action"],
    )
    write_csv(
        output_dir / "target_collisions.csv",
        [asdict(row) for row in collisions],
        ["target_path", "count", "source_paths", "actions"],
    )
    write_csv(
        output_dir / "execution_log.csv",
        execution_log,
        ["status", "source_path", "target_path", "action", "message"],
    )
    write_summary(
        output_dir / "manifest_debrief.txt",
        source_root,
        args.layout,
        tags_mode,
        plans,
        manifest_rows,
        collisions,
        args.execute_safe,
        args.execute_sidecars,
    )

    metadata = {
        "generated_at": now_iso(),
        "source_root": str(source_root),
        "output_dir": str(output_dir),
        "layout": args.layout,
        "tags_mode": tags_mode,
        "workers": args.workers,
        "execute_safe": args.execute_safe,
        "execute_sidecars": args.execute_sidecars,
        "counts": {
            "inventory_rows": len(inventory),
            "folder_records": len(folder_records),
            "group_plans": len(plans),
            "manifest_rows": len(manifest_rows),
            "collision_groups": len(collisions),
        },
    }
    (output_dir / "run_metadata.json").write_text(
        json.dumps(metadata, indent=2),
        encoding="utf-8",
    )

    LOGGER.info("Wrote manifest outputs to %s", output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
