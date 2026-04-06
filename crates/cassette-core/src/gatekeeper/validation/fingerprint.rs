use crate::gatekeeper::error::{GatekeeperError, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub async fn compute_fingerprint(path: &Path) -> Result<String> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || compute_fingerprint_blocking(&p))
        .await
        .map_err(|e| GatekeeperError::FingerprintFailed(e.to_string()))?
}

fn compute_fingerprint_blocking(path: &Path) -> Result<String> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|v| v.to_str()) {
        hint.with_extension(ext);
    }

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| GatekeeperError::FingerprintFailed(e.to_string()))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| GatekeeperError::FingerprintFailed("no decodable track".to_string()))?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| GatekeeperError::FingerprintFailed(e.to_string()))?;

    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut captured = Vec::<f32>::new();
    let target_samples = 22_050 * 2;

    while captured.len() < target_samples {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track.id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        if sample_buf.is_none() {
            sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
        }
        if let Some(ref mut sb) = sample_buf {
            sb.copy_interleaved_ref(decoded);
            captured.extend_from_slice(sb.samples());
        }
    }

    if captured.is_empty() {
        return Err(GatekeeperError::FingerprintFailed(
            "no samples decoded".to_string(),
        ));
    }

    // Local acoustic signature derived from PCM payload.
    let mut hasher = blake3::Hasher::new();
    for sample in captured.iter().step_by(3) {
        hasher.update(&sample.to_le_bytes());
    }
    Ok(hasher.finalize().to_hex().to_string())
}
