from __future__ import annotations

import argparse
import csv
import hashlib
import json
import logging
import os
import shutil
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path


LOGGER = logging.getLogger("collision_deduper")
MAX_COMPONENT_LENGTH = 96


@dataclass(frozen=True)
class CollisionGroup:
    target_path: Path
    source_paths: tuple[Path, ...]


@dataclass
class HashCacheEntry:
    size: int
    mtime_ns: int
    sha256: str


def configure_logging(verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(level=level, format="%(levelname)s %(message)s")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Resolve exact duplicate collision groups by keeping one file and quarantining the rest."
    )
    parser.add_argument(
        "--manifest-dir",
        default="tmp/active_music_manifest_post_safe",
        help="Directory containing target_collisions.csv from a manifest run.",
    )
    parser.add_argument(
        "--quarantine-root",
        default=r"A:\music\_Cassette_Quarantine\exact_collision_duplicates",
        help="Where redundant duplicates should be moved.",
    )
    parser.add_argument(
        "--output-dir",
        default="tmp/remediation/exact-collision-resolution",
        help="Directory for plans, logs, cache, and checkpoints.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Hashing workers.",
    )
    parser.add_argument(
        "--checkpoint-every",
        type=int,
        default=100,
        help="Write progress metadata every N collision groups.",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Execute the plan.",
    )
    parser.add_argument(
        "--resume",
        action="store_true",
        help="Resume from prior progress files in the output directory.",
    )
    parser.add_argument(
        "--shard-count",
        type=int,
        default=1,
        help="Total number of disjoint shards processing the same manifest.",
    )
    parser.add_argument(
        "--shard-index",
        type=int,
        default=0,
        help="Zero-based shard index for this worker.",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable debug logging.",
    )
    return parser.parse_args()


def read_collision_groups(path: Path) -> list[CollisionGroup]:
    groups: list[CollisionGroup] = []
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            target = Path(row["target_path"])
            sources = tuple(Path(part.strip()) for part in row["source_paths"].split(" | ") if part.strip())
            if len(sources) >= 2:
                groups.append(CollisionGroup(target_path=target, source_paths=sources))
    return groups


def shard_matches(target_path: Path, shard_count: int, shard_index: int) -> bool:
    if shard_count <= 1:
        return True
    digest = hashlib.blake2b(str(target_path).lower().encode("utf-8"), digest_size=8).digest()
    shard_value = int.from_bytes(digest, byteorder="big", signed=False) % shard_count
    return shard_value == shard_index


def cache_key(path: Path) -> str:
    return str(path).lower()


def stat_fingerprint(path: Path) -> tuple[int, int]:
    stat = path.stat()
    return stat.st_size, stat.st_mtime_ns


def load_hash_cache(path: Path) -> dict[str, HashCacheEntry]:
    if not path.exists():
        return {}
    raw = json.loads(path.read_text(encoding="utf-8"))
    return {
        key: HashCacheEntry(
            size=int(value["size"]),
            mtime_ns=int(value["mtime_ns"]),
            sha256=str(value["sha256"]),
        )
        for key, value in raw.items()
    }


def save_hash_cache(path: Path, cache: dict[str, HashCacheEntry]) -> None:
    payload = {key: asdict(value) for key, value in cache.items()}
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def hash_file(path: Path, cache: dict[str, HashCacheEntry]) -> str:
    size, mtime_ns = stat_fingerprint(path)
    key = cache_key(path)
    cached = cache.get(key)
    if cached and cached.size == size and cached.mtime_ns == mtime_ns:
        return cached.sha256

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)

    sha256 = digest.hexdigest()
    cache[key] = HashCacheEntry(size=size, mtime_ns=mtime_ns, sha256=sha256)
    return sha256


def score_keeper(source: Path) -> tuple[int, int, str]:
    parent = source.parent.name.lower()
    penalty = 1 if parent.startswith("disc ") else 0
    return (penalty, len(source.parts), str(source).lower())


def shorten_component(value: str, max_length: int = MAX_COMPONENT_LENGTH) -> str:
    cleaned = "".join(char if char not in '<>:"/\\|?*' else "_" for char in value).strip().rstrip(". ")
    if len(cleaned) <= max_length:
        return cleaned or "root"
    digest = hashlib.blake2b(cleaned.encode("utf-8"), digest_size=6).hexdigest()
    head_length = max(8, max_length - len(digest) - 2)
    return f"{cleaned[:head_length]}__{digest}"


def quarantine_path(quarantine_root: Path, source: Path) -> Path:
    artist_component = shorten_component(source.parent.parent.name if len(source.parts) > 2 else "root", 48)
    parent_component = shorten_component(source.parent.name, 64)
    digest = hashlib.blake2b(str(source).lower().encode("utf-8"), digest_size=8).hexdigest()
    file_component = shorten_component(f"{source.stem}__{digest}", 120)
    return quarantine_root / artist_component / parent_component / f"{file_component}{source.suffix}"


def format_error_message(prefix: str, exc: Exception) -> str:
    message = str(exc).replace("\r", " ").replace("\n", " ").strip()
    if len(message) > 180:
        message = f"{message[:177]}..."
    return f"{prefix}:{type(exc).__name__}:{message}" if message else f"{prefix}:{type(exc).__name__}"


def fs_path_str(path: Path) -> str:
    raw = str(path)
    if os.name != "nt":
        return raw
    if raw.startswith("\\\\?\\"):
        return raw
    if raw.startswith("\\\\"):
        return f"\\\\?\\UNC\\{raw[2:]}"
    return f"\\\\?\\{raw}"


def copy_verify_delete(source: Path, target: Path, cache: dict[str, HashCacheEntry]) -> tuple[bool, str]:
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            return False, "target_exists"
        temp_target = target.with_name(f"{target.name}.cassette_tmp")
        if temp_target.exists():
            temp_target.unlink()
        shutil.copy2(fs_path_str(source), fs_path_str(temp_target))
        if hash_file(source, cache) != hash_file(temp_target, cache):
            temp_target.unlink(missing_ok=True)
            return False, "hash_mismatch"
        temp_target.replace(target)
        source.unlink()
        return True, "moved"
    except Exception as exc:
        return False, format_error_message("copy_verify_delete_failed", exc)


def move_file(source: Path, target: Path) -> tuple[bool, str]:
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            return False, "target_exists"
        shutil.move(fs_path_str(source), fs_path_str(target))
        return True, "moved"
    except Exception as exc:
        return False, format_error_message("move_failed", exc)


def load_processed_targets(path: Path) -> set[str]:
    if not path.exists():
        return set()
    with path.open("r", encoding="utf-8", newline="") as handle:
        return {row["target_path"].lower() for row in csv.DictReader(handle)}


def append_rows(path: Path, fieldnames: list[str], rows: list[dict[str, str]]) -> None:
    if not rows:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    file_exists = path.exists()
    with path.open("a", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        if not file_exists:
            writer.writeheader()
        writer.writerows(rows)


def write_progress(path: Path, processed: int, total: int, apply: bool) -> None:
    path.write_text(
        json.dumps(
            {
                "processed_groups": processed,
                "total_groups": total,
                "apply": apply,
                "updated_at": datetime.now().isoformat(),
            },
            indent=2,
        ),
        encoding="utf-8",
    )


def resolve_group(
    group: CollisionGroup,
    apply: bool,
    quarantine_root: Path,
    cache: dict[str, HashCacheEntry],
    workers: int,
) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    plan_rows: list[dict[str, str]] = []
    execution_rows: list[dict[str, str]] = []

    existing_sources = [path for path in group.source_paths if path.exists()]
    if len(existing_sources) < 2:
        plan_rows.append(
            {
                "target_path": str(group.target_path),
                "decision": "SKIP",
                "keeper": "",
                "duplicates": " | ".join(str(path) for path in group.source_paths),
                "reason": "fewer than two sources currently exist",
            }
        )
        return plan_rows, execution_rows

    sizes = {path.stat().st_size for path in existing_sources}
    if len(sizes) != 1:
        plan_rows.append(
            {
                "target_path": str(group.target_path),
                "decision": "SKIP",
                "keeper": "",
                "duplicates": " | ".join(str(path) for path in existing_sources),
                "reason": "size mismatch within collision group",
            }
        )
        return plan_rows, execution_rows

    with ThreadPoolExecutor(max_workers=max(1, workers)) as executor:
        hash_pairs = list(executor.map(lambda path: (path, hash_file(path, cache)), existing_sources))

    unique_hashes = {hash_value for _, hash_value in hash_pairs}
    if len(unique_hashes) != 1:
        plan_rows.append(
            {
                "target_path": str(group.target_path),
                "decision": "SKIP",
                "keeper": "",
                "duplicates": " | ".join(str(path) for path in existing_sources),
                "reason": "non-identical collision group",
            }
        )
        return plan_rows, execution_rows

    keeper = min(existing_sources, key=score_keeper)
    duplicates = [path for path in existing_sources if path != keeper]
    plan_rows.append(
        {
            "target_path": str(group.target_path),
            "decision": "EXACT_DUPLICATE",
            "keeper": str(keeper),
            "duplicates": " | ".join(str(path) for path in duplicates),
            "reason": "all existing sources share identical SHA256",
        }
    )

    if not apply:
        return plan_rows, execution_rows

    if keeper != group.target_path:
        ok, message = copy_verify_delete(keeper, group.target_path, cache)
        execution_rows.append(
            {
                "action": "MOVE_KEEPER_TO_TARGET",
                "source_path": str(keeper),
                "target_path": str(group.target_path),
                "status": "MOVED" if ok else "ERROR",
                "message": message,
            }
        )
        if not ok:
            return plan_rows, execution_rows
    else:
        execution_rows.append(
            {
                "action": "MOVE_KEEPER_TO_TARGET",
                "source_path": str(keeper),
                "target_path": str(group.target_path),
                "status": "SKIPPED",
                "message": "keeper already at target",
            }
        )

    for duplicate in duplicates:
        target = quarantine_path(quarantine_root, duplicate)
        ok, message = move_file(duplicate, target)
        execution_rows.append(
            {
                "action": "QUARANTINE_DUPLICATE",
                "source_path": str(duplicate),
                "target_path": str(target),
                "status": "MOVED" if ok else "ERROR",
                "message": message,
            }
        )

    return plan_rows, execution_rows


def main() -> int:
    args = parse_args()
    configure_logging(args.verbose)

    if args.shard_count < 1:
        raise ValueError("--shard-count must be >= 1")
    if args.shard_index < 0 or args.shard_index >= args.shard_count:
        raise ValueError("--shard-index must be in [0, shard-count)")

    manifest_dir = Path(args.manifest_dir)
    collision_csv = manifest_dir / "target_collisions.csv"
    if not collision_csv.exists():
        raise FileNotFoundError(f"Collision CSV not found: {collision_csv}")

    all_groups = read_collision_groups(collision_csv)
    groups = [
        group
        for group in all_groups
        if shard_matches(group.target_path, args.shard_count, args.shard_index)
    ]
    output_dir = Path(args.output_dir)
    if args.shard_count > 1:
        output_dir = output_dir / f"shard-{args.shard_index:02d}-of-{args.shard_count:02d}"
    output_dir.mkdir(parents=True, exist_ok=True)
    quarantine_root = Path(args.quarantine_root)

    plan_path = output_dir / "exact_collision_plan.csv"
    log_path = output_dir / "exact_collision_execution.csv"
    cache_path = output_dir / "hash_cache.json"
    progress_path = output_dir / "progress.json"

    processed_targets = load_processed_targets(plan_path) if args.resume else set()
    cache = load_hash_cache(cache_path)

    LOGGER.info(
        "Loaded %s/%s collision groups for shard %s/%s",
        len(groups),
        len(all_groups),
        args.shard_index,
        args.shard_count,
    )
    if processed_targets:
        LOGGER.info("Resuming with %s already processed groups", len(processed_targets))

    processed = 0
    for group in groups:
        target_key = str(group.target_path).lower()
        if target_key in processed_targets:
            processed += 1
            continue

        plan_rows, execution_rows = resolve_group(
            group=group,
            apply=args.apply,
            quarantine_root=quarantine_root,
            cache=cache,
            workers=args.workers,
        )
        append_rows(
            plan_path,
            ["target_path", "decision", "keeper", "duplicates", "reason"],
            plan_rows,
        )
        append_rows(
            log_path,
            ["action", "source_path", "target_path", "status", "message"],
            execution_rows,
        )

        processed += 1
        processed_targets.add(target_key)
        if processed % max(1, args.checkpoint_every) == 0 or processed == len(groups):
            save_hash_cache(cache_path, cache)
            write_progress(progress_path, processed=processed, total=len(groups), apply=args.apply)
            LOGGER.info("Processed %s/%s collision groups", processed, len(groups))

    save_hash_cache(cache_path, cache)
    write_progress(progress_path, processed=processed, total=len(groups), apply=args.apply)
    LOGGER.info("Wrote plan to %s", plan_path)
    if log_path.exists():
        LOGGER.info("Wrote execution log to %s", log_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
