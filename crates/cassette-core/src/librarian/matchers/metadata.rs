pub fn duration_within_tolerance(
    left_ms: Option<i64>,
    right_ms: Option<i64>,
    tolerance_ms: i64,
) -> bool {
    match (left_ms, right_ms) {
        (Some(a), Some(b)) => (a - b).abs() <= tolerance_ms,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strong_metadata_duration_tolerance_behaves_as_expected() {
        assert!(duration_within_tolerance(
            Some(200_000),
            Some(201_500),
            2_000
        ));
        assert!(!duration_within_tolerance(
            Some(200_000),
            Some(205_000),
            2_000
        ));
    }
}
