use crate::gatekeeper::config::GatekeeperConfig;
use crate::gatekeeper::error::{GatekeeperError, Result};
use crate::gatekeeper::mod_types::{PayloadProbe, QualityAssessment, QualityTier};

pub fn assess_quality(
    probe: &PayloadProbe,
    config: &GatekeeperConfig,
    upgrade_from_local: bool,
) -> Result<QualityAssessment> {
    let is_lossless = probe.codec.to_ascii_lowercase().contains("flac")
        || probe.codec.to_ascii_lowercase().contains("pcm")
        || probe.codec.to_ascii_lowercase().contains("alac");

    let bitrate_floor = if is_lossless {
        config.bitrate_floor_lossless
    } else {
        config.bitrate_floor_lossy
    };

    let passes_bitrate_floor = is_lossless || probe.bitrate >= bitrate_floor;
    let passes_sample_rate_floor = probe.sample_rate >= config.sample_rate_floor;

    let quality_tier = if is_lossless {
        QualityTier::Lossless
    } else if probe.bitrate >= 256 {
        QualityTier::LossyHi
    } else if probe.bitrate >= 128 {
        QualityTier::LossyAcceptable
    } else {
        QualityTier::BelowFloor
    };

    if config.reject_below_floor && (!passes_bitrate_floor || !passes_sample_rate_floor) {
        return Err(GatekeeperError::QualityAssessmentFailed(
            "file below configured quality floor".to_string(),
        ));
    }

    Ok(QualityAssessment {
        passes_bitrate_floor,
        passes_sample_rate_floor,
        is_lossless,
        quality_tier,
        upgrade_from_local,
    })
}
