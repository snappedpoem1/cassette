from __future__ import annotations

import argparse
import csv
import hashlib
import logging
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path


LOGGER = logging.getLogger("cassette_collision_resolver")

HIGH_CONFIDENCE_COLLISION_REASON = (
    "album normalized using high-confidence tags metadata. Target collision detected."
)


@dataclass(frozen=True)
class CollisionMember:
    source_path: Path
    target_path: Path


@dataclass(frozen=True)
class CollisionDecision:
    group_target: str
    source_path: str
    destination_path: str
    status: str
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Resolve manifest collision groups when every source is byte-identical."
    )
    parser.add_argument("manifest_csv", help="Path to rename_manifest.csv")
    parser.add_argument(
        "--quarantine-root",
        required=True,
        help="Quarantine root for redundant duplicates.",
    )
    parser.add_argument(
        "--output-csv",
        required=True,
        help="Decision log output CSV.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Hash workers.",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply the safe collision resolutions.",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable debug logging.",
    )
    return parser.parse_args()


def configure_logging(verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(level=level, format="%(levelname)s %(message)s")


def ensure_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def load_groups(path: Path) -> dict[str, list[CollisionMember]]:
    groups: dict[str, list[CollisionMember]] = defaultdict(list)
    with path.open("r", newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            if row.get("action") != "REVIEW":
                continue
            if row.get("reason") != HIGH_CONFIDENCE_COLLISION_REASON:
                continue
            target = row["target_path"]
            groups[target].append(
                CollisionMember(
                    source_path=Path(row["source_path"]),
                    target_path=Path(target),
                )
            )
    return {target: members for target, members in groups.items() if len(members) > 1}


def compare_member(member: CollisionMember) -> tuple[CollisionMember, str]:
    return member, hash_file(member.source_path)


def resolve_group(
    target_path: str,
    members: list[CollisionMember],
    quarantine_root: Path,
    workers: int,
) -> list[CollisionDecision]:
    existing_members = [member for member in members if member.source_path.exists()]
    if len(existing_members) < 2:
        return [
            CollisionDecision(
                group_target=target_path,
                source_path=str(member.source_path),
                destination_path="",
                status="skipped",
                message="fewer than two existing sources in group",
            )
            for member in members
        ]

    if Path(target_path).exists():
        return [
            CollisionDecision(
                group_target=target_path,
                source_path=str(member.source_path),
                destination_path=target_path,
                status="skipped",
                message="target already exists",
            )
            for member in existing_members
        ]

    sizes = {member.source_path.stat().st_size for member in existing_members}
    if len(sizes) != 1:
        return [
            CollisionDecision(
                group_target=target_path,
                source_path=str(member.source_path),
                destination_path="",
                status="review_conflict",
                message="size mismatch within collision group",
            )
            for member in existing_members
        ]

    with ThreadPoolExecutor(max_workers=max(1, workers)) as executor:
        hashed = list(executor.map(compare_member, existing_members))

    hashes = {digest for _, digest in hashed}
    if len(hashes) != 1:
        return [
            CollisionDecision(
                group_target=target_path,
                source_path=str(member.source_path),
                destination_path="",
                status="review_conflict",
                message="hash mismatch within collision group",
            )
            for member in existing_members
        ]

    keeper = sorted(existing_members, key=lambda member: str(member.source_path).lower())[0]
    results = [
        CollisionDecision(
            group_target=target_path,
            source_path=str(keeper.source_path),
            destination_path=target_path,
            status="safe_keep_move",
            message="identical collision group; keep deterministic source",
        )
    ]
    for member in sorted(existing_members[1:], key=lambda item: str(item.source_path).lower()):
        relative_parent = member.source_path.drive.rstrip(":") or "root"
        quarantine_path = quarantine_root.joinpath(
            relative_parent, *member.source_path.parent.parts[1:], member.source_path.name
        )
        results.append(
            CollisionDecision(
                group_target=target_path,
                source_path=str(member.source_path),
                destination_path=str(quarantine_path),
                status="safe_quarantine_duplicate",
                message="identical collision group; redundant duplicate",
            )
        )
    return results


def apply_decisions(decisions: list[CollisionDecision]) -> list[CollisionDecision]:
    applied: list[CollisionDecision] = []
    for decision in decisions:
        if decision.status not in {"safe_keep_move", "safe_quarantine_duplicate"}:
            applied.append(decision)
            continue

        source = Path(decision.source_path)
        destination = Path(decision.destination_path)
        ensure_directory(destination.parent)
        if destination.exists():
            applied.append(
                CollisionDecision(
                    group_target=decision.group_target,
                    source_path=decision.source_path,
                    destination_path=decision.destination_path,
                    status="skipped",
                    message="destination already exists during apply",
                )
            )
            continue

        source.replace(destination)
        applied.append(
            CollisionDecision(
                group_target=decision.group_target,
                source_path=decision.source_path,
                destination_path=decision.destination_path,
                status="moved",
                message=decision.message,
            )
        )
    return applied


def write_csv(path: Path, rows: list[CollisionDecision]) -> None:
    ensure_directory(path.parent)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["group_target", "source_path", "destination_path", "status", "message"],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {
                    "group_target": row.group_target,
                    "source_path": row.source_path,
                    "destination_path": row.destination_path,
                    "status": row.status,
                    "message": row.message,
                }
            )


def main() -> int:
    args = parse_args()
    configure_logging(args.verbose)

    manifest_path = Path(args.manifest_csv)
    quarantine_root = Path(args.quarantine_root)
    output_csv = Path(args.output_csv)

    groups = load_groups(manifest_path)
    LOGGER.info("Loaded %s high-confidence collision groups", len(groups))

    decisions: list[CollisionDecision] = []
    for index, (target_path, members) in enumerate(sorted(groups.items()), start=1):
        if index % 1000 == 0:
            LOGGER.info("Processed %s/%s collision groups", index, len(groups))
        decisions.extend(resolve_group(target_path, members, quarantine_root, args.workers))

    if args.apply:
        decisions = apply_decisions(decisions)

    write_csv(output_csv, decisions)

    counts: dict[str, int] = {}
    for row in decisions:
        counts[row.status] = counts.get(row.status, 0) + 1
    LOGGER.info("Decision counts: %s", counts)
    LOGGER.info("Wrote collision decision log to %s", output_csv)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
