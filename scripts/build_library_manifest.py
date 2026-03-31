from __future__ import annotations

import argparse
import csv
import hashlib
import json
import logging
import shutil
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path
from typing import Any


LOGGER = logging.getLogger("cassette_manifest")

SUPPORTED_AUDIO_EXTENSIONS = {
    ".flac",
    ".mp3",
    ".m4a",
    ".aac",
    ".ogg",
    ".opus",
    ".wav",
    ".aiff",
    ".wv",
    ".ape",
}

SIDECAR_EXTENSIONS = {
    ".lrc",
    ".txt",
    ".nfo",
    ".cue",
    ".log",
    ".sfv",
    ".m3u",
    ".m3u8",
}

IMAGE_EXTENSIONS = {
    ".jpg",
    ".jpeg",
    ".png",
    ".webp",
    ".bmp",
    ".gif",
}

STANDARD_ART_NAMES = {
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
    "discart.jpg",
    "discart.png",
}

NUMBER_ONLY_FOLDERS = {"00", "01", "02", "03", "04", "05", "06", "07", "08", "09"}
IMPORTER_MARKERS = (" [flac]", " [16b-", " [24b-", " [lossless]", " [mp3]")
TRASH_PARENT_MARKERS = (
    "adjacent jams",
    "heavy remixes",
    "indie alt rock",
    "post-hardcore",
    "electronic bass",
    "hip-hop",
)
HASHY_SUFFIX_LENGTH = 12


@dataclass(frozen=True)
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
class FolderFlagRow:
    folder_path: str
    relative_path: str
    audio_count: int
    image_count: int
    sidecar_count: int
    other_count: int
    flags: str


def configure_logging(verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(level=level, format="%(levelname)s %(message)s")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build and optionally execute a high-confidence Cassette library manifest."
    )
    parser.add_argument(
        "--audit-report",
        default="tmp/library_audit_report.json",
        help="Path to library audit report JSON.",
    )
    parser.add_argument(
        "--deadspot-report",
        default="tmp/library_deadspot_report.json",
        help="Path to deadspot report JSON.",
    )
    parser.add_argument(
        "--output-root",
        default="tmp/remediation",
        help="Root directory for manifest outputs.",
    )
    parser.add_argument(
        "--run-name",
        default="",
        help="Optional run name suffix for deterministic output grouping.",
    )
    parser.add_argument(
        "--quarantine-root",
        default="",
        help="Optional quarantine root. Defaults to <library_root>\\\\_Cassette_Quarantine.",
    )
    parser.add_argument(
        "--apply-safe",
        action="store_true",
        help="Execute only SAFE_* manifest rows using copy-verify-delete.",
    )
    parser.add_argument(
        "--limit-safe",
        type=int,
        default=0,
        help="Optional limit on how many SAFE_* rows to execute.",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable debug logging.",
    )
    return parser.parse_args()


def read_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Required input file not found: {path}")
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        return json.load(handle)


def ensure_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def canonical_text(path: str) -> str:
    return str(Path(path)).lower()


def path_parts_lower(path: Path) -> tuple[str, ...]:
    return tuple(part.lower() for part in path.parts)


def contains_unknown_component(path: str) -> bool:
    lowered = canonical_text(path)
    return "unknown artist" in lowered or "unknown album" in lowered


def has_hashy_suffix(name: str) -> bool:
    stem = Path(name).stem
    if "_" not in stem and "-" not in stem:
        return False
    tail = stem.rsplit("_", 1)[-1]
    if tail == stem:
        tail = stem.rsplit("-", 1)[-1]
    return tail.isdigit() and len(tail) >= HASHY_SUFFIX_LENGTH


def is_importer_style_source(source: Path) -> bool:
    parent_name = source.parent.name.lower()
    grandparent_name = source.parent.parent.name.lower() if source.parent.parent != source.parent else ""
    relative_depth = len(source.parts)

    if relative_depth < 3:
        return False
    if any(marker in parent_name for marker in IMPORTER_MARKERS):
        return True
    if parent_name.startswith("(") and ")" in parent_name:
        return True
    if parent_name[:4].isdigit() and " - " in parent_name:
        return True
    if " - " in parent_name and "(" in parent_name and ")" in parent_name:
        return True
    if grandparent_name.startswith("(") and ")" in grandparent_name:
        return True
    if "disc " in parent_name and any(marker in grandparent_name for marker in IMPORTER_MARKERS):
        return True
    return False


def is_trash_context(source: Path) -> bool:
    lowered = path_parts_lower(source.parent)
    return any(part in TRASH_PARENT_MARKERS for part in lowered)


def quarantine_target(quarantine_root: Path, library_root: Path, source: Path, bucket: str) -> Path:
    relative_parent = source.parent.relative_to(library_root)
    safe_parent = "__".join(relative_parent.parts) if relative_parent.parts else "root"
    return quarantine_root / bucket / safe_parent / source.name


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def compare_existing_target(source: Path, target: Path) -> tuple[bool | None, str]:
    if not target.is_file():
        return None, "target exists but is not a file"
    if source.stat().st_size != target.stat().st_size:
        return False, "target exists with different size"
    try:
        if hash_file(source) == hash_file(target):
            return True, "target exists with identical content"
        return False, "target exists with different hash"
    except OSError as exc:
        return None, f"hash compare failed: {exc}"


def copy_verify_delete(source: Path, target: Path) -> tuple[bool, str]:
    ensure_directory(target.parent)
    if target.exists():
        return False, "target_exists"

    temp_target = target.with_name(f"{target.name}.cassette_tmp")
    if temp_target.exists():
        temp_target.unlink()

    shutil.copy2(source, temp_target)
    source_hash = hash_file(source)
    temp_hash = hash_file(temp_target)
    if source_hash != temp_hash:
        temp_target.unlink(missing_ok=True)
        return False, "hash_mismatch"

    temp_target.replace(target)
    source.unlink()
    return True, "moved"


def classify_deadspot(example: dict[str, Any]) -> tuple[str, str]:
    reasons = [str(reason) for reason in example.get("reasons", [])]
    if any("probable silent placeholder" in reason.lower() for reason in reasons):
        return "SAFE_QUARANTINE_DEADSPOT", "probable silent placeholder"
    return "REVIEW", "deadspot requires human review"


def build_folder_flag_rows(library_root: Path) -> list[FolderFlagRow]:
    folder_counts: dict[Path, dict[str, int]] = defaultdict(
        lambda: {"audio": 0, "image": 0, "sidecar": 0, "other": 0}
    )

    for path in library_root.rglob("*"):
        if not path.is_file():
            continue
        counts = folder_counts[path.parent]
        suffix = path.suffix.lower()
        if suffix in SUPPORTED_AUDIO_EXTENSIONS:
            counts["audio"] += 1
        elif suffix in IMAGE_EXTENSIONS:
            counts["image"] += 1
        elif suffix in SIDECAR_EXTENSIONS:
            counts["sidecar"] += 1
        else:
            counts["other"] += 1

    rows: list[FolderFlagRow] = []
    for folder, counts in sorted(folder_counts.items(), key=lambda item: str(item[0]).lower()):
        relative = folder.relative_to(library_root)
        flags: list[str] = []
        name_lower = folder.name.lower()
        if name_lower in NUMBER_ONLY_FOLDERS or folder.name.isdigit():
            flags.append("NUMBER_ONLY_FOLDER")
        if len(folder.name) <= 2:
            flags.append("VERY_SHORT_FOLDER")
        if folder.name.startswith("."):
            flags.append("DOT_PREFIX_FOLDER")
        if counts["audio"] == 0 and counts["sidecar"] > 0:
            flags.append("SIDECAR_ONLY_FOLDER")
        if counts["audio"] == 1:
            flags.append("SINGLE_TRACK_FOLDER")
        if counts["audio"] > 0 and counts["sidecar"] > 0:
            flags.append("MIXED_AUDIO_AND_SIDECARS")
        if sum(counts.values()) > 0 and counts["audio"] == 0 and counts["image"] > 0:
            flags.append("ART_ONLY_FOLDER")
        if flags:
            rows.append(
                FolderFlagRow(
                    folder_path=str(folder),
                    relative_path=str(relative),
                    audio_count=counts["audio"],
                    image_count=counts["image"],
                    sidecar_count=counts["sidecar"],
                    other_count=counts["other"],
                    flags="; ".join(sorted(set(flags))),
                )
            )
    return rows


def build_manifest(
    audit_report: dict[str, Any],
    deadspot_report: dict[str, Any],
    library_root: Path,
    quarantine_root: Path,
) -> tuple[list[ManifestRow], list[dict[str, str]], list[FolderFlagRow]]:
    manifest_rows: list[ManifestRow] = []
    invalid_paths: set[str] = set()
    deadspot_paths: set[str] = set()
    collisions: list[dict[str, str]] = []

    for example in sorted(audit_report.get("invalid_examples", []), key=lambda item: item["path"].lower()):
        source = Path(example["path"])
        invalid_paths.add(str(source))
        target = quarantine_target(quarantine_root, library_root, source, "invalid")
        reason = f"{example.get('status', 'Invalid')}: {'; '.join(example.get('reasons', []))}"
        manifest_rows.append(
            ManifestRow(
                action="SAFE_QUARANTINE_INVALID",
                confidence="HIGH",
                source_path=str(source),
                target_path=str(target),
                rollback_source=str(target),
                rollback_target=str(source),
                item_type="audio",
                reason=reason,
                group_key=str(source.parent),
            )
        )

    for example in sorted(deadspot_report.get("examples", []), key=lambda item: item["path"].lower()):
        source = Path(example["path"])
        deadspot_paths.add(str(source))
        action, reason_prefix = classify_deadspot(example)
        target = quarantine_target(quarantine_root, library_root, source, "deadspots")
        manifest_rows.append(
            ManifestRow(
                action=action,
                confidence="MEDIUM" if action.startswith("SAFE_") else "LOW",
                source_path=str(source),
                target_path=str(target),
                rollback_source=str(target),
                rollback_target=str(source),
                item_type="audio",
                reason=f"{reason_prefix}: {'; '.join(example.get('reasons', []))}",
                group_key=str(source.parent),
            )
        )

    misplaced_groups: dict[str, list[dict[str, str]]] = defaultdict(list)
    for example in audit_report.get("misplaced_examples", []):
        misplaced_groups[example["expected_path"]].append(example)

    for target_path in sorted(misplaced_groups, key=str.lower):
        group = sorted(misplaced_groups[target_path], key=lambda item: item["source_path"].lower())
        if len(group) > 1:
            collisions.append(
                {
                    "target_path": target_path,
                    "count": str(len(group)),
                    "source_paths": " | ".join(item["source_path"] for item in group),
                }
            )

        for example in group:
            source = Path(example["source_path"])
            target = Path(example["expected_path"])
            source_key = str(source)

            if source_key in invalid_paths or source_key in deadspot_paths:
                continue

            action = "REVIEW"
            confidence = "LOW"
            reasons: list[str] = []

            if not source.exists():
                manifest_rows.append(
                    ManifestRow(
                        action="SKIP_MISSING_SOURCE",
                        confidence="LOW",
                        source_path=str(source),
                        target_path=str(target),
                        rollback_source=str(target),
                        rollback_target=str(source),
                        item_type="audio",
                        reason="source already missing from disk; audit report is stale",
                        group_key=str(target.parent),
                    )
                )
                continue

            if len(group) > 1:
                reasons.append("target collision")
            if contains_unknown_component(str(target)):
                reasons.append("expected path contains unknown component")
            if is_trash_context(source):
                reasons.append("source is in a trash/adjacent-jams context")
            if not is_importer_style_source(source):
                reasons.append("source folder shape is not a known importer pattern")

            if not reasons and target.exists():
                is_same, compare_reason = compare_existing_target(source, target)
                if is_same:
                    duplicate_target = quarantine_target(
                        quarantine_root,
                        library_root,
                        source,
                        "duplicates",
                    )
                    manifest_rows.append(
                        ManifestRow(
                            action="SAFE_QUARANTINE_DUPLICATE",
                            confidence="HIGH",
                            source_path=str(source),
                            target_path=str(duplicate_target),
                            rollback_source=str(duplicate_target),
                            rollback_target=str(source),
                            item_type="audio",
                            reason=compare_reason,
                            group_key=str(target.parent),
                        )
                    )
                    continue
                reasons.append(compare_reason)

            if not reasons and not target.exists():
                action = "SAFE_RENAME"
                confidence = "HIGH"
                reasons.append("importer-style source with unique canonical target")

            manifest_rows.append(
                ManifestRow(
                    action=action,
                    confidence=confidence,
                    source_path=str(source),
                    target_path=str(target),
                    rollback_source=str(target),
                    rollback_target=str(source),
                    item_type="audio",
                    reason="; ".join(reasons),
                    group_key=str(target.parent),
                )
            )

    folder_flag_rows = build_folder_flag_rows(library_root)
    sidecar_rows = build_sidecar_rows(library_root, quarantine_root)
    manifest_rows.extend(sidecar_rows)

    manifest_rows.sort(key=lambda row: (row.action, row.target_path.lower(), row.source_path.lower()))
    return manifest_rows, collisions, folder_flag_rows


def build_sidecar_rows(library_root: Path, quarantine_root: Path) -> list[ManifestRow]:
    rows: list[ManifestRow] = []
    for path in sorted(library_root.rglob("*"), key=lambda candidate: str(candidate).lower()):
        if not path.is_file():
            continue
        suffix = path.suffix.lower()
        if suffix not in SIDECAR_EXTENSIONS and suffix not in IMAGE_EXTENSIONS:
            continue

        sibling_audio = any(
            sibling.is_file() and sibling.suffix.lower() in SUPPORTED_AUDIO_EXTENSIONS
            for sibling in path.parent.iterdir()
        )
        if suffix in IMAGE_EXTENSIONS and path.name.lower() in STANDARD_ART_NAMES:
            continue

        action = "REVIEW"
        confidence = "LOW"
        reasons: list[str] = []
        item_type = "sidecar" if suffix in SIDECAR_EXTENSIONS else "image"
        target = quarantine_target(quarantine_root, library_root, path, "sidecars")

        if suffix == ".nfo":
            action = "SAFE_QUARANTINE_SIDECAR"
            confidence = "HIGH"
            reasons.append("nfo file is never part of canonical music library")
        elif suffix == ".lrc" and not sibling_audio:
            action = "SAFE_QUARANTINE_SIDECAR"
            confidence = "HIGH"
            reasons.append("orphan lyric sidecar without sibling audio")
        elif suffix in IMAGE_EXTENSIONS and path.name.lower() not in STANDARD_ART_NAMES and has_hashy_suffix(path.name):
            action = "SAFE_QUARANTINE_SIDECAR"
            confidence = "MEDIUM"
            reasons.append("hashy image artifact")
        elif suffix in SIDECAR_EXTENSIONS:
            reasons.append("sidecar retained for manual review")
        else:
            reasons.append("non-standard image retained for manual review")

        rows.append(
            ManifestRow(
                action=action,
                confidence=confidence,
                source_path=str(path),
                target_path=str(target),
                rollback_source=str(target),
                rollback_target=str(path),
                item_type=item_type,
                reason="; ".join(reasons),
                group_key=str(path.parent),
            )
        )
    return rows


def write_csv(path: Path, rows: list[dict[str, Any]], fieldnames: list[str]) -> None:
    ensure_directory(path.parent)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow(row)


def write_summary(
    path: Path,
    library_root: Path,
    manifest_rows: list[ManifestRow],
    collisions: list[dict[str, str]],
    folder_flag_rows: list[FolderFlagRow],
    apply_safe: bool,
) -> None:
    action_counts = Counter(row.action for row in manifest_rows)
    confidence_counts = Counter(row.confidence for row in manifest_rows)
    folder_flag_counts = Counter()
    for row in folder_flag_rows:
        for flag in row.flags.split("; "):
            folder_flag_counts[flag] += 1

    with path.open("w", encoding="utf-8") as handle:
        handle.write("CASSETTE LIBRARY MANIFEST DEBRIEF\n")
        handle.write("=" * 72 + "\n\n")
        handle.write(f"Library root: {library_root}\n")
        handle.write(f"Apply safe mode: {apply_safe}\n")
        handle.write(f"Generated at: {datetime.now().isoformat()}\n\n")

        handle.write("MANIFEST ACTION COUNTS\n")
        handle.write("-" * 72 + "\n")
        for key, value in sorted(action_counts.items()):
            handle.write(f"{key}: {value}\n")
        handle.write("\n")

        handle.write("CONFIDENCE COUNTS\n")
        handle.write("-" * 72 + "\n")
        for key, value in sorted(confidence_counts.items()):
            handle.write(f"{key}: {value}\n")
        handle.write("\n")

        handle.write("TOP FOLDER FLAGS\n")
        handle.write("-" * 72 + "\n")
        for flag, count in folder_flag_counts.most_common(20):
            handle.write(f"{flag}: {count}\n")
        handle.write("\n")

        handle.write("TARGET COLLISIONS\n")
        handle.write("-" * 72 + "\n")
        handle.write(f"Collision groups: {len(collisions)}\n")
        for collision in collisions[:25]:
            handle.write(f"{collision['count']}x -> {collision['target_path']}\n")
        handle.write("\n")

        handle.write("EXECUTION POLICY\n")
        handle.write("-" * 72 + "\n")
        handle.write("SAFE_* actions are eligible for copy-verify-delete execution.\n")
        handle.write("REVIEW rows are intentionally blocked from automatic mutation.\n")
        handle.write("Unknown-target rows stay out of the executable lane.\n")


def execute_manifest_rows(
    manifest_rows: list[ManifestRow],
    apply_safe: bool,
    limit_safe: int,
) -> list[dict[str, str]]:
    execution_rows: list[dict[str, str]] = []
    executed_safe = 0

    for row in manifest_rows:
        source = Path(row.source_path)
        target = Path(row.target_path)
        should_execute = row.action.startswith("SAFE_")

        if not should_execute:
            execution_rows.append(
                {
                    "status": "SKIPPED",
                    "action": row.action,
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "message": "review-only row",
                }
            )
            continue

        if limit_safe and executed_safe >= limit_safe:
            execution_rows.append(
                {
                    "status": "SKIPPED",
                    "action": row.action,
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "message": "safe execution limit reached",
                }
            )
            continue

        if not apply_safe:
            execution_rows.append(
                {
                    "status": "DRY_RUN",
                    "action": row.action,
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "message": "apply-safe disabled",
                }
            )
            continue

        if not source.exists():
            execution_rows.append(
                {
                    "status": "SKIPPED",
                    "action": row.action,
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "message": "source already absent; audit input is stale",
                }
            )
            continue

        ok, message = copy_verify_delete(source, target)
        execution_rows.append(
            {
                "status": "MOVED" if ok else "ERROR",
                "action": row.action,
                "source_path": row.source_path,
                "target_path": row.target_path,
                "message": message,
            }
        )
        if ok:
            executed_safe += 1

    return execution_rows


def main() -> int:
    args = parse_args()
    configure_logging(args.verbose)

    audit_path = Path(args.audit_report)
    deadspot_path = Path(args.deadspot_report)
    audit_report = read_json(audit_path)
    deadspot_report = read_json(deadspot_path)

    library_root = Path(audit_report["root"])
    if not library_root.exists():
        raise FileNotFoundError(f"Library root from audit report does not exist: {library_root}")

    quarantine_root = (
        Path(args.quarantine_root)
        if args.quarantine_root
        else library_root / "_Cassette_Quarantine"
    )

    run_stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    run_name = f"{run_stamp}-{args.run_name}" if args.run_name else f"{run_stamp}-manifest"
    output_dir = Path(args.output_root) / run_name
    ensure_directory(output_dir)

    LOGGER.info("Building manifest from %s", audit_path)
    manifest_rows, collisions, folder_flag_rows = build_manifest(
        audit_report=audit_report,
        deadspot_report=deadspot_report,
        library_root=library_root,
        quarantine_root=quarantine_root,
    )

    execution_rows = execute_manifest_rows(
        manifest_rows=manifest_rows,
        apply_safe=args.apply_safe,
        limit_safe=args.limit_safe,
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
        output_dir / "target_collisions.csv",
        collisions,
        ["target_path", "count", "source_paths"],
    )
    write_csv(
        output_dir / "folder_flags.csv",
        [asdict(row) for row in folder_flag_rows],
        [
            "folder_path",
            "relative_path",
            "audio_count",
            "image_count",
            "sidecar_count",
            "other_count",
            "flags",
        ],
    )
    write_csv(
        output_dir / "execution_log.csv",
        execution_rows,
        ["status", "action", "source_path", "target_path", "message"],
    )
    write_summary(
        output_dir / "manifest_debrief.txt",
        library_root=library_root,
        manifest_rows=manifest_rows,
        collisions=collisions,
        folder_flag_rows=folder_flag_rows,
        apply_safe=args.apply_safe,
    )

    LOGGER.info("Manifest output written to %s", output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
