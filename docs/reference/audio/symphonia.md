# Symphonia
> Audio format probing, codec detection, duration extraction, and playback decoding
**Status:** Proven Working
**Code:** `crates/cassette-core/src/director/validation.rs`, player module

## What It Does
Provides low-level audio format analysis used in two contexts:
1. **Validation:** `validate_candidate()` probes downloaded files to verify they are valid audio, extract duration, and detect codec.
2. **Playback:** The player module uses Symphonia for audio decoding during playback.
3. **Gatekeeper/Custodian:** A richer validation stack exists but is not active in the Tauri runtime.

## External Dependencies
- **symphonia** Rust crate (format probing, codec detection, audio decoding)
- No external services or network calls

## Authentication & Credentials
None. Symphonia is a local-only library.

## Data Flow
### Validation Probe
1. Read first 256 bytes of file for magic byte detection
2. Magic byte mapping: `fLaC` -> flac, `ID3` -> mp3, `OggS` -> ogg (`OpusHead` -> opus), `RIFF`+`WAVE` -> wav, `ftyp` -> m4a
3. Create `Hint` with file extension
4. Wrap file in `MediaSourceStream`
5. Call `get_probe().format()` to get format reader
6. Extract `default_track()` codec params: `n_frames` and `sample_rate`
7. Calculate duration: `n_frames / sample_rate`

### Quality Classification
Classification is by file extension (not codec analysis):
- **Lossless:** flac, wav, aiff, wv, ape
- **Lossy:** mp3, aac, m4a, ogg, opus

## Capabilities
- Magic byte format detection (pre-probe sanity check)
- Full format probing via Symphonia's probe system
- Duration extraction from codec parameters
- Codec identification
- Quality classification (lossless vs lossy)
- Audio decoding for playback

## Limitations & Known Issues
- Quality classification uses file extension, not actual codec analysis (an m4a file could contain ALAC lossless but is classified as lossy)
- Magic byte detection only covers common formats; uncommon containers may not be identified
- No bitrate extraction (could distinguish 128kbps from 320kbps MP3 but doesn't)
- Duration calculation depends on `n_frames` being present in codec params, which not all formats guarantee
- Gatekeeper/custodian validation stack is built but not wired into the Tauri runtime

## Untapped Potential
Symphonia supports significantly more than what Cassette currently uses:
- Full audio decoding (sample-level access, not just probing)
- Sample-accurate seeking
- Codec-specific metadata (FLAC StreamInfo for bit depth/sample rate, MP3 bitrate mode detection VBR/CBR)
- Gapless playback information
- ReplayGain tag reading
- Multiple track/stream handling within a single container
- Format-specific metadata (Vorbis comments in OGG, ID3 in MP3)

## Code Map
| Symbol | Location | Purpose |
|---|---|---|
| `validate_candidate()` | `crates/cassette-core/src/director/validation.rs` | Format probe + duration + codec detection |
| Magic byte detection | `crates/cassette-core/src/director/validation.rs` | First-256-byte format sniffing |
| Quality classification | `crates/cassette-core/src/director/validation.rs` | Extension-based lossless/lossy categorization |
| Player decoding | Player module | Audio playback via Symphonia decoders |
