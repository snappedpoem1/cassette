pub fn sanitize_component(input: &str) -> String {
    let replaced = input
        .replace('/', "~")
        .replace('\\', "~")
        .replace(':', "-");

    let mut value = sanitize_filename::sanitize(&replaced);
    if value.is_empty() {
        value = "Unknown".to_string();
    }

    let upper = value.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "LPT1"];
    if reserved.contains(&upper.as_str()) {
        return format!("_{value}");
    }

    value.trim_end_matches('.').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_invalid_windows_chars_and_slashes() {
        assert_eq!(sanitize_component("AC/DC"), "AC~DC");
        assert!(sanitize_component("Album: Remaster").contains("Album- Remaster"));
    }
}
