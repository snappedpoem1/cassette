pub fn quality_score(codec: Option<&str>, bitrate: Option<u32>, bit_depth: Option<u8>) -> i64 {
    let mut score = 0_i64;
    match codec.unwrap_or_default().to_ascii_lowercase().as_str() {
        "flac" | "wav" | "alac" | "aiff" | "dsf" | "dff" => score += 100,
        "mp3" | "aac" | "m4a" | "opus" | "ogg" => score += 40,
        _ => score += 10,
    }

    score += i64::from(bitrate.unwrap_or(0) / 8);
    score += i64::from(bit_depth.unwrap_or(0));
    score
}
