use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone, Default)]
pub struct ProbeReport {
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub duration_ms: Option<u64>,
    pub decode_ok: bool,
}

pub fn probe_audio(path: &std::path::Path) -> Result<ProbeReport, String> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|v| v.to_str()) {
        hint.with_extension(ext);
    }

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;

    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| "no default track".to_string())?;
    if track.codec_params.codec == CODEC_TYPE_NULL {
        return Err("unknown codec".to_string());
    }

    let codec_name = Some(format!("{:?}", track.codec_params.codec));
    let sample_rate = track.codec_params.sample_rate;
    let channels = track.codec_params.channels.map(|c| c.count() as u8);

    let duration_ms = match (track.codec_params.n_frames, track.codec_params.sample_rate) {
        (Some(frames), Some(rate)) if rate > 0 => {
            Some(((frames as f64 / rate as f64) * 1000.0) as u64)
        }
        _ => None,
    };

    let mut decode_ok = false;
    if let Ok(mut decoder) = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()) {
        if let Ok(packet) = format.next_packet() {
            decode_ok = decoder.decode(&packet).is_ok();
        }
    }

    Ok(ProbeReport {
        codec: codec_name,
        sample_rate,
        channels,
        duration_ms,
        decode_ok,
    })
}
