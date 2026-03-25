use crate::gatekeeper::config::PolicySpine;
use crate::gatekeeper::error::{GatekeeperError, Result};
use crate::gatekeeper::mod_types::{AudioTags, JunkFlag, PayloadProbe};
use regex::Regex;

pub fn apply_junk_filters(
    _probe: &PayloadProbe,
    tags: &AudioTags,
    policy: &PolicySpine,
) -> Result<Vec<JunkFlag>> {
    let mut flags = Vec::<JunkFlag>::new();
    let mut haystack = String::new();
    if let Some(v) = &tags.title {
        haystack.push_str(v);
        haystack.push(' ');
    }
    if let Some(v) = &tags.artist {
        haystack.push_str(v);
        haystack.push(' ');
    }
    if let Some(v) = &tags.album {
        haystack.push_str(v);
    }
    let haystack = haystack.to_ascii_lowercase();

    for (pattern, flag) in &policy.junk_filter_patterns {
        let regex = Regex::new(pattern).map_err(|e| GatekeeperError::ConfigError(e.to_string()))?;
        if regex.is_match(&haystack) {
            let allowed = match flag {
                JunkFlag::IsKaraoke => policy.allow_karaoke,
                JunkFlag::IsLiveVersion => policy.allow_live,
                JunkFlag::IsInstrumental => policy.allow_instrumental,
                JunkFlag::IsRemix => policy.allow_remixes,
                JunkFlag::IsDemo => policy.allow_demos,
                JunkFlag::IsUnofficial => policy.allow_unofficial,
                JunkFlag::IsAltVersion => policy.allow_alt_versions,
                JunkFlag::IsInterlude | JunkFlag::IsSkitOrSpeech => {
                    policy.allow_skits_and_interludes
                }
            };
            if !allowed {
                flags.push(*flag);
            }
        }
    }

    Ok(flags)
}
