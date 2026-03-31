"""
execute_library_strike.py
Fast, parallel, hash-verified library cleanup executor.

Reads the fresh manifest, splits into:
  - safe rows (no target collisions)
  - collision groups (best-copy-wins logic)

Executes safe rows with a ThreadPoolExecutor (I/O bound, saturates A: drive throughput).
Resolves collisions by picking the largest file, quarantining the losers.
Prunes empty directories after all moves.
"""
from __future__ import annotations

import csv
import hashlib
import io
import os
import shutil
import sys
import threading
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

# Force UTF-8 stdout so Unicode filenames don't crash on Windows cp1252 terminals
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

# ── Config ──────────────────────────────────────────────────────────────────
MANIFEST_CSV = Path("tmp/active_music_manifest_fresh/rename_manifest.csv")
QUARANTINE_ROOT = Path("A:/music/_Cassette_Quarantine")
SKIP_ACTIONS = {"REVIEW", "SKIP"}
WORKERS = 16  # I/O bound — 16 threads drains A: drive queue nicely

# ── Stats ────────────────────────────────────────────────────────────────────
lock = threading.Lock()
stats: dict[str, int] = Counter()


def log(msg: str) -> None:
    with lock:
        print(msg, flush=True)


# ── File ops ─────────────────────────────────────────────────────────────────

def sha256(path: Path, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            buf = f.read(chunk)
            if not buf:
                break
            h.update(buf)
    return h.hexdigest()


def move_verified(src: Path, dst: Path) -> tuple[bool, str]:
    """Copy src -> dst with hash verify, then delete src. Atomic-ish via tmp rename."""
    if not src.exists():
        return False, "src_missing"
    if dst.exists():
        return False, "dst_exists"

    dst.parent.mkdir(parents=True, exist_ok=True)
    tmp = dst.with_suffix(dst.suffix + ".~cassette")

    try:
        shutil.copy2(src, tmp)
        if sha256(src) != sha256(tmp):
            tmp.unlink(missing_ok=True)
            return False, "hash_mismatch"
        tmp.replace(dst)
        src.unlink()
        return True, "ok"
    except Exception as exc:
        tmp.unlink(missing_ok=True)
        return False, str(exc)


# ── Load manifest ─────────────────────────────────────────────────────────────

@dataclass
class Row:
    action: str
    source: Path
    target: Path
    reason: str


def load_pending(manifest: Path) -> list[Row]:
    rows: list[Row] = []
    with manifest.open(encoding="utf-8", errors="replace", newline="") as f:
        for r in csv.DictReader(f):
            action = r["action"]
            if action in SKIP_ACTIONS:
                continue
            src = Path(r["source_path"])
            if not src.exists():
                continue
            rows.append(Row(
                action=action,
                source=src,
                target=Path(r["target_path"]),
                reason=r.get("reason", ""),
            ))
    return rows


# ── Collision resolution ──────────────────────────────────────────────────────

def resolve_collision_group(target: Path, candidates: list[Row]) -> list[tuple[Path, Path, str]]:
    """
    Pick the largest file as the keeper.
    If sizes tie, prefer the one whose source path looks most canonical
    (shorter path = less nesting = more likely to be the clean copy).
    Returns list of (src, dst, label) moves:
      - winner  -> target
      - losers  -> quarantine
    """
    def score(row: Row) -> tuple[int, int]:
        size = row.source.stat().st_size if row.source.exists() else 0
        return (size, -len(str(row.source)))  # bigger size wins, shorter path breaks ties

    ranked = sorted(candidates, key=score, reverse=True)
    winner = ranked[0]
    losers = ranked[1:]

    moves: list[tuple[Path, Path, str]] = []

    # Winner goes to canonical target
    moves.append((winner.source, target, "collision_winner"))

    # Losers go to quarantine/collision_dupes/<relative_path>/<filename>
    for loser in losers:
        try:
            rel = loser.source.relative_to(Path("A:/music"))
        except ValueError:
            rel = Path(loser.source.name)
        q_dst = QUARANTINE_ROOT / "collision_dupes" / rel
        moves.append((loser.source, q_dst, "collision_loser"))

    return moves


# ── Execute one row ───────────────────────────────────────────────────────────

def execute_row(row: Row) -> tuple[str, str]:
    ok, msg = move_verified(row.source, row.target)
    action_tag = row.action
    if ok:
        with lock:
            stats["moved"] += 1
            stats[action_tag] += 1
        log(f"  OK  {row.source.name}  ->  {row.target.parent.name}/{row.target.name}")
    else:
        with lock:
            stats["errors"] += 1
        log(f"  ERR [{msg}] {row.source}")
    return ("ok" if ok else "err"), msg


# ── Prune empties ─────────────────────────────────────────────────────────────

def prune_empty_dirs(root: Path) -> int:
    removed = 0
    # Bottom-up so parents become empty after children are removed
    for dirpath, dirnames, filenames in os.walk(root, topdown=False):
        p = Path(dirpath)
        if p == root:
            continue
        try:
            if not any(p.iterdir()):
                p.rmdir()
                removed += 1
                log(f" DEL  empty dir: {p}")
        except Exception:
            pass
    return removed


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    print(f"Loading manifest: {MANIFEST_CSV}")
    all_rows = load_pending(MANIFEST_CSV)
    print(f"Pending rows (source exists): {len(all_rows)}")

    # Split safe vs collision
    target_counts = Counter(r.target for r in all_rows)
    collision_targets = {t for t, c in target_counts.items() if c > 1}

    safe_rows = [r for r in all_rows if r.target not in collision_targets]
    collision_rows_by_target: dict[Path, list[Row]] = defaultdict(list)
    for r in all_rows:
        if r.target in collision_targets:
            collision_rows_by_target[r.target].append(r)

    print(f"Safe rows: {len(safe_rows)}")
    print(f"Collision groups: {len(collision_rows_by_target)}")
    print(f"Workers: {WORKERS}")
    print()

    # ── Phase 1: parallel safe moves ─────────────────────────────────────────
    print("=" * 60)
    print("PHASE 1: Safe moves")
    print("=" * 60)
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        futures = {pool.submit(execute_row, row): row for row in safe_rows}
        for fut in as_completed(futures):
            try:
                fut.result()
            except Exception as exc:
                row = futures[fut]
                log(f"  EXC {row.source}: {exc}")
                with lock:
                    stats["errors"] += 1

    print()
    print(f"Phase 1 done — moved: {stats['moved']}, errors: {stats['errors']}")
    print()

    # ── Phase 2: collision resolution ────────────────────────────────────────
    print("=" * 60)
    print("PHASE 2: Collision resolution")
    print("=" * 60)
    for target, candidates in sorted(collision_rows_by_target.items(), key=lambda kv: str(kv[0])):
        print(f"\n  TARGET: {target}")
        moves = resolve_collision_group(target, candidates)
        for src, dst, label in moves:
            if not src.exists():
                log(f"    SKIP [{label}] src missing: {src.name}")
                continue
            ok, msg = move_verified(src, dst)
            tag = "WIN " if label == "collision_winner" else "DUPE"
            if ok:
                log(f"    {tag}  {src.name}  ({src.stat().st_size if src.exists() else '?'} bytes)")
                with lock:
                    stats["moved"] += 1
                    stats[f"collision_{label}"] += 1
            else:
                log(f"    ERR [{msg}] {src}")
                with lock:
                    stats["errors"] += 1

    print()
    print(f"Phase 2 done — collision winners: {stats.get('collision_collision_winner', 0)}, "
          f"dupes quarantined: {stats.get('collision_collision_loser', 0)}")
    print()

    # ── Phase 3: prune empty dirs ─────────────────────────────────────────────
    print("=" * 60)
    print("PHASE 3: Pruning empty directories")
    print("=" * 60)
    removed = prune_empty_dirs(Path("A:/music"))
    print(f"Removed {removed} empty directories")
    print()

    # ── Final summary ─────────────────────────────────────────────────────────
    print("=" * 60)
    print("DONE")
    print("=" * 60)
    total_moved = stats["moved"]
    total_errors = stats["errors"]
    print(f"  Total moved:    {total_moved}")
    print(f"  Total errors:   {total_errors}")
    print()
    by_action = {k: v for k, v in stats.items() if k not in ("moved", "errors")}
    for k, v in sorted(by_action.items()):
        print(f"  {k}: {v}")

    if total_errors:
        sys.exit(1)


if __name__ == "__main__":
    main()
