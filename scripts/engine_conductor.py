"""
Cassette Engine Conductor — Phase 2
Query MusicBrainz for full discographies of target artists,
compare against spotify_album_history, insert any missing albums.
Rate limited to 1 req/sec per MB API policy.
"""
import json
import os
import sqlite3
import sys
import time
import urllib.request
import urllib.parse
import urllib.error
from pathlib import Path

MB_BASE = "https://musicbrainz.org/ws/2"
USER_AGENT = "CassetteEngine/1.0 (snappedpoem@gmail.com)"
RATE_LIMIT_SECS = 1.1  # slightly over 1s to stay safe

DB_PATH = os.path.join(os.environ["APPDATA"], "dev.cassette.app", "cassette.db")
ARTIST_FILE = r"A:\music_admin\top_50_artists.json"
OUTPUT_FILE = r"A:\music_admin\conductor_work_orders_2026-03-30.json"

def mb_get(endpoint: str, params: dict) -> dict:
    """Make a rate-limited GET request to MusicBrainz API."""
    params["fmt"] = "json"
    url = f"{MB_BASE}/{endpoint}?{urllib.parse.urlencode(params)}"
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        if e.code == 503:
            print(f"  [MB 503 - rate limited, waiting 3s]")
            time.sleep(3)
            with urllib.request.urlopen(req, timeout=15) as resp:
                return json.loads(resp.read())
        raise

def search_artist_mbid(name: str) -> str | None:
    """Search MusicBrainz for an artist and return their MBID."""
    data = mb_get("artist", {"query": f'artist:"{name}"', "limit": "5"})
    time.sleep(RATE_LIMIT_SECS)
    for artist in data.get("artists", []):
        if artist.get("name", "").lower() == name.lower():
            return artist["id"]
        # Fuzzy match for punctuation differences
        if artist.get("name", "").lower().replace(".", "").replace("!", "") == name.lower().replace(".", "").replace("!", ""):
            return artist["id"]
    # Fall back to first result if score is high enough
    artists = data.get("artists", [])
    if artists and artists[0].get("score", 0) >= 90:
        return artists[0]["id"]
    return None

def get_artist_releases(mbid: str, release_types: list[str] = None) -> list[dict]:
    """Get all release groups for an artist (albums, EPs)."""
    if release_types is None:
        release_types = ["album", "ep"]

    all_releases = []
    offset = 0
    limit = 100

    while True:
        data = mb_get("release-group", {
            "artist": mbid,
            "type": "|".join(release_types),
            "limit": str(limit),
            "offset": str(offset),
        })
        time.sleep(RATE_LIMIT_SECS)

        groups = data.get("release-groups", [])
        for rg in groups:
            title = rg.get("title", "")
            rg_type = rg.get("primary-type", "")
            first_release = rg.get("first-release-date", "")[:4]
            all_releases.append({
                "title": title,
                "type": rg_type,
                "year": first_release,
                "mbid": rg.get("id", ""),
            })

        if len(groups) < limit:
            break
        offset += limit

    return all_releases

def get_existing_albums(db: sqlite3.Connection, artist: str) -> set[str]:
    """Get albums already in spotify_album_history for an artist (normalized lowercase)."""
    cursor = db.execute(
        "SELECT LOWER(album) FROM spotify_album_history WHERE LOWER(artist) = LOWER(?)",
        (artist,)
    )
    return {row[0] for row in cursor.fetchall()}

def normalize_album_title(title: str) -> str:
    """Normalize album title for comparison."""
    return (title.lower()
            .replace("'", "'")
            .replace("\u2019", "'")
            .replace("\u2018", "'")
            .replace("\u201c", '"')
            .replace("\u201d", '"')
            .strip())

def insert_album(db: sqlite3.Connection, artist: str, album: str) -> bool:
    """Insert an album into spotify_album_history if not already present."""
    existing = db.execute(
        "SELECT 1 FROM spotify_album_history WHERE LOWER(artist) = LOWER(?) AND LOWER(album) = LOWER(?)",
        (artist, album)
    ).fetchone()
    if existing:
        return False
    db.execute(
        "INSERT INTO spotify_album_history (artist, album, total_ms, play_count) VALUES (?, ?, 0, 1)",
        (artist, album)
    )
    return True

def main():
    # Load target artists
    with open(ARTIST_FILE) as f:
        data = json.load(f)
    artists = [a["name"] for a in data["artists"]]

    print(f"Conductor: processing {len(artists)} target artists")
    print(f"DB: {DB_PATH}")
    print()

    db = sqlite3.connect(DB_PATH)

    work_orders = []
    total_inserted = 0
    total_already_present = 0
    errors = []

    for i, artist_name in enumerate(artists, 1):
        print(f"[{i:>2}/{len(artists)}] {artist_name}...", end=" ", flush=True)

        # Search for artist MBID
        try:
            mbid = search_artist_mbid(artist_name)
        except Exception as e:
            print(f"ERROR searching: {e}")
            errors.append({"artist": artist_name, "error": str(e)})
            continue

        if not mbid:
            print("NOT FOUND on MusicBrainz")
            errors.append({"artist": artist_name, "error": "not found"})
            continue

        # Get full discography
        try:
            releases = get_artist_releases(mbid)
        except Exception as e:
            print(f"ERROR fetching releases: {e}")
            errors.append({"artist": artist_name, "error": str(e)})
            continue

        # Compare against DB
        existing = get_existing_albums(db, artist_name)

        inserted = 0
        already = 0
        artist_missing = []

        for release in releases:
            title = release["title"]
            normalized = normalize_album_title(title)

            if normalized in existing:
                already += 1
                continue

            # Try inserting
            if insert_album(db, artist_name, title):
                inserted += 1
                artist_missing.append({
                    "album": title,
                    "type": release["type"],
                    "year": release["year"],
                    "mbid": release["mbid"],
                })

        db.commit()
        total_inserted += inserted
        total_already_present += already

        print(f"MB: {len(releases)} releases | DB: {len(existing)} known | +{inserted} new")

        if artist_missing:
            work_orders.append({
                "artist": artist_name,
                "mbid": mbid,
                "total_releases": len(releases),
                "already_in_db": already,
                "newly_inserted": inserted,
                "missing_albums": artist_missing,
            })

    db.close()

    # Write work orders
    output = {
        "timestamp": "2026-03-30",
        "total_artists": len(artists),
        "total_new_albums_inserted": total_inserted,
        "total_already_present": total_already_present,
        "errors": errors,
        "work_orders": work_orders,
    }

    with open(OUTPUT_FILE, "w") as f:
        json.dump(output, f, indent=2)

    print(f"\n=== CONDUCTOR COMPLETE ===")
    print(f"Artists processed: {len(artists)}")
    print(f"New albums inserted into DB: {total_inserted}")
    print(f"Already in DB: {total_already_present}")
    print(f"Errors: {len(errors)}")
    print(f"Work orders written to: {OUTPUT_FILE}")

if __name__ == "__main__":
    main()
