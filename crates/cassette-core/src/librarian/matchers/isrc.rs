pub fn isrc_match(lhs: Option<&str>, rhs: Option<&str>) -> bool {
    match (lhs, rhs) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isrc_matching_is_case_insensitive() {
        assert!(isrc_match(Some("USRC17607839"), Some("usrc17607839")));
        assert!(!isrc_match(Some("USRC17607839"), Some("USRC17607840")));
    }
}
