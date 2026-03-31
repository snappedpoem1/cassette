"""
stabilize_db.py - One-shot full cleanup of the Cassette library DB.
Fixes: quarantine leaks, duplicates, empty metadata, title prefixes,
artist-in-title, hash suffixes, qobuz prefixes, filename artist prefixes,
whitespace in paths.
"""
import sqlite3, pathlib, re, shutil
from collections import Counter

DB_PATH = "C:/Users/Admin/AppData/Roaming/dev.cassette.app/cassette.db"
MUSIC_ROOT = pathlib.Path("A:/music")

format_rank = {".flac": 0, ".m4a": 1, ".mp3": 2, ".opus": 3, ".ogg": 4, ".aac": 5}

db = sqlite3.connect(DB_PATH)

# 1. PURGE QUARANTINE
deleted = db.execute("DELETE FROM tracks WHERE path LIKE '%_Cassette_Quarantine%'").rowcount
print(f"1. Purged quarantine: {deleted}")

# 2. DEDUPLICATE
dupes = db.execute(
    "SELECT GROUP_CONCAT(id), GROUP_CONCAT(path, '|') FROM tracks "
    "GROUP BY LOWER(COALESCE(artist,'')), LOWER(COALESCE(album,'')), LOWER(COALESCE(title,'')) "
    "HAVING COUNT(*) > 1"
).fetchall()
dupe_removed = 0
for ids_str, paths_str in dupes:
    ids = [int(i) for i in ids_str.split(",")]
    paths = paths_str.split("|")
    def score(idx):
        p = pathlib.Path(paths[idx])
        size = p.stat().st_size if p.exists() else 0
        fmt = format_rank.get(p.suffix.lower(), 99)
        return (-fmt, size)
    ranked = sorted(range(len(ids)), key=score, reverse=True)
    for idx in ranked[1:]:
        db.execute("DELETE FROM tracks WHERE id = ?", (ids[idx],))
        dupe_removed += 1
print(f"2. Removed duplicates: {dupe_removed}")

# 3. FIX EMPTY ARTIST/ALBUM
rows = db.execute(
    "SELECT id, path, title, artist, album, year FROM tracks "
    "WHERE artist = '' OR artist IS NULL OR album = '' OR album IS NULL"
).fetchall()
meta_fixed = 0
for rid, path, title, artist, album, year in rows:
    try:
        parts = pathlib.Path(path).relative_to(MUSIC_ROOT).parts
    except Exception:
        continue
    if len(parts) < 2:
        continue
    new_artist = artist if artist else parts[0]
    new_album = album
    if not new_album:
        if len(parts) >= 3:
            new_album = re.sub(r"^\d{4}\s*-\s*", "", parts[1])
        else:
            new_album = "Singles"
    new_year = year
    if not new_year and len(parts) >= 3:
        ym = re.match(r"^(\d{4})\s*-", parts[1])
        if ym:
            new_year = ym.group(1)
    db.execute(
        "UPDATE tracks SET artist=?, album=?, year=COALESCE(NULLIF(?,''), year) WHERE id=?",
        (new_artist, new_album, new_year, rid),
    )
    meta_fixed += 1
print(f"3. Fixed empty artist/album: {meta_fixed}")

# 4. STRIP TRACK PREFIXES ("01 - Title" -> "Title")
rows = db.execute("SELECT id, title FROM tracks").fetchall()
prefix_fixed = 0
for rid, title in rows:
    if not title:
        continue
    m = re.match(r"^(?:\d{1,2}-)?\d{1,2}\s*[-\u2013\u2014.]\s+(.+)$", title)
    if m:
        new_title = m.group(1).strip()
        if new_title and new_title != title:
            db.execute("UPDATE tracks SET title=? WHERE id=?", (new_title, rid))
            prefix_fixed += 1
print(f"4. Stripped track prefixes: {prefix_fixed}")

# 5. STRIP ARTIST FROM TITLE
rows = db.execute("SELECT id, title, artist FROM tracks").fetchall()
at_fixed = 0
for rid, title, artist in rows:
    if not title or not artist:
        continue
    prefix = artist + " - "
    if title.startswith(prefix):
        new_title = title[len(prefix):].strip()
        if new_title:
            db.execute("UPDATE tracks SET title=? WHERE id=?", (new_title, rid))
            at_fixed += 1
print(f"5. Stripped artist from title: {at_fixed}")

# 6. FIX HASH SUFFIX FILES
rows = db.execute("SELECT id, path FROM tracks").fetchall()
hash_fixed = 0
for rid, path in rows:
    p = pathlib.Path(path)
    if re.search(r"_\d{12,}$", p.stem):
        new_stem = re.sub(r"_\d{12,}$", "", p.stem)
        new_path = p.with_name(new_stem + p.suffix)
        if p.exists() and not new_path.exists():
            p.rename(new_path)
            db.execute("UPDATE tracks SET path=? WHERE id=?", (str(new_path), rid))
            hash_fixed += 1
print(f"6. Fixed hash-suffix files: {hash_fixed}")

# 7. FIX QOBUZ PREFIX FILES
rows = db.execute("SELECT id, path FROM tracks WHERE path LIKE '%qobuz-%'").fetchall()
qobuz_fixed = 0
for rid, path in rows:
    p = pathlib.Path(path)
    m = re.match(r"^qobuz-\d+-(.+)$", p.stem)
    if m:
        new_path = p.with_name(m.group(1) + p.suffix)
        if p.exists() and not new_path.exists():
            p.rename(new_path)
            db.execute("UPDATE tracks SET path=? WHERE id=?", (str(new_path), rid))
            qobuz_fixed += 1
print(f"7. Fixed qobuz-prefix files: {qobuz_fixed}")

# 8. FIX FILENAME ARTIST PREFIX
rows = db.execute("SELECT id, path, artist FROM tracks").fetchall()
fname_fixed = 0
for rid, path, artist in rows:
    if not artist:
        continue
    p = pathlib.Path(path)
    prefix = artist + " - "
    if p.stem.startswith(prefix):
        new_name = p.stem[len(prefix):] + p.suffix
        new_path = p.with_name(new_name)
        if p.exists() and not new_path.exists() and new_name.strip():
            p.rename(new_path)
            db.execute("UPDATE tracks SET path=? WHERE id=?", (str(new_path), rid))
            fname_fixed += 1
print(f"8. Fixed artist-prefix filenames: {fname_fixed}")

# 9. FIX PATH WHITESPACE
rows = db.execute("SELECT id, path FROM tracks").fetchall()
ws_fixed = 0
for rid, path in rows:
    p = pathlib.Path(path)
    try:
        parts = p.relative_to(MUSIC_ROOT).parts
    except Exception:
        continue
    if any(part != part.strip() for part in parts):
        clean_parts = [part.strip() for part in parts]
        clean_path = MUSIC_ROOT.joinpath(*clean_parts)
        if p.exists() and not clean_path.exists():
            clean_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(p), str(clean_path))
            db.execute("UPDATE tracks SET path=? WHERE id=?", (str(clean_path), rid))
            ws_fixed += 1
        elif clean_path.exists() and p != clean_path:
            db.execute("UPDATE tracks SET path=? WHERE id=?", (str(clean_path), rid))
            ws_fixed += 1
print(f"9. Fixed whitespace paths: {ws_fixed}")

db.commit()

# FINAL REPORT
total = db.execute("SELECT COUNT(*) FROM tracks").fetchone()[0]
bad_a = db.execute("SELECT COUNT(*) FROM tracks WHERE artist='' OR artist IS NULL").fetchone()[0]
bad_al = db.execute("SELECT COUNT(*) FROM tracks WHERE album='' OR album IS NULL").fetchone()[0]
bad_t = db.execute("SELECT COUNT(*) FROM tracks WHERE title='' OR title IS NULL").fetchone()[0]
ql = db.execute("SELECT COUNT(*) FROM tracks WHERE path LIKE '%_Cassette_Quarantine%'").fetchone()[0]
dg = db.execute(
    "SELECT COUNT(*) FROM ("
    "SELECT 1 FROM tracks "
    "GROUP BY LOWER(COALESCE(artist,'')), LOWER(COALESCE(album,'')), LOWER(COALESCE(title,'')) "
    "HAVING COUNT(*) > 1)"
).fetchone()[0]
print(f"\n{'='*50}")
print(f"FINAL: {total} tracks")
print(f"  Missing artist:  {bad_a}")
print(f"  Missing album:   {bad_al}")
print(f"  Missing title:   {bad_t}")
print(f"  Quarantine leak: {ql}")
print(f"  Dupe groups:     {dg}")
db.close()
