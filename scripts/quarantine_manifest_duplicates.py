from __future__ import annotations

import argparse
import csv
import hashlib
import logging
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


LOGGER = logging.getLogger("cassette_duplicate_quarantine")

HIGH_CONFIDENCE_COLLISION_REASON = (
    "album normalized using high-confidence tags metadata. Target collision detected."
)


@dataclass(frozen=True)
class ReviewRow:
    source_path: Path
    target_path: Path
    action: str
    reason: str


@dataclass(frozen=True)
class DuplicateDecision:
    source_path: str
    target_path: str
    quarantine_path: str
    status: str
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Hash-verify and quarantine exact duplicates from a manifest review lane."
    )
    parser.add_argument("manifest_csv", help="Path to rename_manifest.csv")
    parser.add_argument(
        "--quarantine-root",
        required=True,
        help="Directory where exact duplicates should be quarantined.",
    )
    parser.add_argument(
        "--output-csv",
        required=True,
        help="CSV log path for decisions and moves.",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Hash comparison workers.",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Move exact duplicates into quarantine. Default is dry run.",
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


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def load_review_rows(path: Path) -> list[ReviewRow]:
    rows: list[ReviewRow] = []
    with path.open("r", newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            if row.get("action") != "REVIEW":
                continue
            if row.get("reason") != HIGH_CONFIDENCE_COLLISION_REASON:
                continue
            rows.append(
                ReviewRow(
                    source_path=Path(row["source_path"]),
                    target_path=Path(row["target_path"]),
                    action=row["action"],
                    reason=row["reason"],
                )
            )
    return rows


def compare_row(row: ReviewRow, quarantine_root: Path) -> DuplicateDecision:
    source = row.source_path
    target = row.target_path

    if not source.exists():
        return DuplicateDecision(
            source_path=str(source),
            target_path=str(target),
            quarantine_path="",
            status="skipped",
            message="source missing",
        )
    if not target.exists():
        return DuplicateDecision(
            source_path=str(source),
            target_path=str(target),
            quarantine_path="",
            status="skipped",
            message="target missing",
        )
    if source.stat().st_size != target.stat().st_size:
        return DuplicateDecision(
            source_path=str(source),
            target_path=str(target),
            quarantine_path="",
            status="review_conflict",
            message="different size",
        )
    if hash_file(source) != hash_file(target):
        return DuplicateDecision(
            source_path=str(source),
            target_path=str(target),
            quarantine_path="",
            status="review_conflict",
            message="different hash",
        )

    relative_parent = source.drive.rstrip(":") or "root"
    safe_parts = [relative_parent] + list(source.parent.parts[1:])
    quarantine_path = quarantine_root.joinpath(*safe_parts, source.name)
    return DuplicateDecision(
        source_path=str(source),
        target_path=str(target),
        quarantine_path=str(quarantine_path),
        status="exact_duplicate",
        message="same size and sha256",
    )


def ensure_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def move_exact_duplicates(decisions: list[DuplicateDecision], apply: bool) -> list[DuplicateDecision]:
    results: list[DuplicateDecision] = []
    for decision in decisions:
        if decision.status != "exact_duplicate":
            results.append(decision)
            continue

        if not apply:
            results.append(
                DuplicateDecision(
                    source_path=decision.source_path,
                    target_path=decision.target_path,
                    quarantine_path=decision.quarantine_path,
                    status="dry_run",
                    message=decision.message,
                )
            )
            continue

        source = Path(decision.source_path)
        quarantine_path = Path(decision.quarantine_path)
        ensure_directory(quarantine_path.parent)
        if quarantine_path.exists():
            results.append(
                DuplicateDecision(
                    source_path=decision.source_path,
                    target_path=decision.target_path,
                    quarantine_path=decision.quarantine_path,
                    status="skipped",
                    message="quarantine target exists",
                )
            )
            continue

        source.replace(quarantine_path)
        results.append(
            DuplicateDecision(
                source_path=decision.source_path,
                target_path=decision.target_path,
                quarantine_path=decision.quarantine_path,
                status="moved",
                message=decision.message,
            )
        )
    return results


def write_csv(path: Path, rows: list[DuplicateDecision]) -> None:
    ensure_directory(path.parent)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["source_path", "target_path", "quarantine_path", "status", "message"],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {
                    "source_path": row.source_path,
                    "target_path": row.target_path,
                    "quarantine_path": row.quarantine_path,
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

    review_rows = load_review_rows(manifest_path)
    LOGGER.info("Loaded %s high-confidence collision review rows", len(review_rows))

    with ThreadPoolExecutor(max_workers=max(1, args.workers)) as executor:
        decisions = list(executor.map(lambda row: compare_row(row, quarantine_root), review_rows))

    final_rows = move_exact_duplicates(decisions, apply=args.apply)
    write_csv(output_csv, final_rows)

    counts: dict[str, int] = {}
    for row in final_rows:
        counts[row.status] = counts.get(row.status, 0) + 1
    LOGGER.info("Decision counts at %s: %s", datetime.now().isoformat(), counts)
    LOGGER.info("Wrote duplicate decision log to %s", output_csv)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
