pub fn plausible_size_for_duration(
    duration_ms: Option<u64>,
    bitrate_kbps: Option<u32>,
    file_size: u64,
    tolerance: f64,
) -> bool {
    let Some(duration_ms) = duration_ms else {
        return true;
    };
    let Some(bitrate_kbps) = bitrate_kbps else {
        return true;
    };

    let seconds = duration_ms as f64 / 1000.0;
    if seconds <= 0.0 {
        return false;
    }

    let expected_bytes = (seconds * bitrate_kbps as f64 * 1000.0 / 8.0).max(1.0);
    let lower = expected_bytes / tolerance.max(1.0);
    let upper = expected_bytes * tolerance.max(1.0);
    let actual = file_size as f64;

    actual >= lower && actual <= upper
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_heuristic_flags_impossible_payload() {
        let ok = plausible_size_for_duration(Some(180_000), Some(320), 7_200_000, 1.5);
        let bad = plausible_size_for_duration(Some(180_000), Some(320), 20_000, 1.5);
        assert!(ok);
        assert!(!bad);
    }
}
