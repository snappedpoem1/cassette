use crate::gatekeeper::error::{GatekeeperError, Result};
use crate::gatekeeper::mod_types::PayloadProbe;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::path::Path;

pub async fn probe_payload(path: &Path) -> Result<PayloadProbe> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || probe_payload_blocking(&p))
        .await
        .map_err(|e| GatekeeperError::DecodeFailed(e.to_string()))?
}

fn probe_payload_blocking(path: &Path) -> Result<PayloadProbe> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    if file_size == 0 {
        return Err(GatekeeperError::PayloadValidationFailed(
            "zero-byte file".to_string(),
        ));
    }

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|v| v.to_str()) {
        hint.with_extension(ext);
    }

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| GatekeeperError::DecodeFailed(e.to_string()))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| GatekeeperError::PayloadValidationFailed("no decodable track".to_string()))?
        .clone();

    let codec = format!("{:?}", track.codec_params.codec);
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| GatekeeperError::PayloadValidationFailed("sample rate missing".to_string()))?;
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u8)
        .ok_or_else(|| GatekeeperError::PayloadValidationFailed("channel count missing".to_string()))?;

    let duration_ms = match (track.codec_params.n_frames, track.codec_params.sample_rate) {
        (Some(frames), Some(rate)) if rate > 0 => ((frames as f64 / rate as f64) * 1000.0) as u64,
        _ => 0,
    };
    if duration_ms == 0 {
        return Err(GatekeeperError::PayloadValidationFailed(
            "duration resolved to zero".to_string(),
        ));
    }

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| GatekeeperError::DecodeFailed(e.to_string()))?;

    let packet = format
        .next_packet()
        .map_err(|e| GatekeeperError::DecodeFailed(e.to_string()))?;
    if decoder.decode(&packet).is_err() {
        return Err(GatekeeperError::DecodeFailed(
            "first-frame decode failed".to_string(),
        ));
    }

    let bitrate = if duration_ms > 0 {
        ((file_size as f64 * 8.0) / (duration_ms as f64 / 1000.0) / 1000.0) as u32
    } else {
        0
    };

    let bit_depth = track.codec_params.bits_per_sample.unwrap_or(16).clamp(1, 32) as u8;

    if duration_ms > 0 {
        let bytes_per_sec = file_size as f64 / (duration_ms as f64 / 1000.0);
        if bytes_per_sec < 64.0 {
            return Err(GatekeeperError::PayloadValidationFailed(
                "impossible size/duration ratio".to_string(),
            ));
        }
    }

    Ok(PayloadProbe {
        codec,
        bitrate,
        sample_rate,
        bit_depth,
        channels,
        duration_ms,
        file_size,
    })
}
