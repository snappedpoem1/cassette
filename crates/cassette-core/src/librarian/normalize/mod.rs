pub mod album;
pub mod artist;
pub mod track;

pub fn normalize_text(input: &str) -> String {
    let mut out = input.to_ascii_lowercase();
    for ch in ['.', ',', '!', '?', '-', '_', ':', ';', '\'', '"', '(', ')', '[', ']', '{', '}', '/'] {
        out = out.replace(ch, " ");
    }
    out = out.replace("featuring", "feat");
    out = out.replace("feat.", "feat");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_title_suffixes(input: &str) -> String {
    let mut value = normalize_text(input);
    let suffixes = ["radio edit", "original mix", "extended mix", "remix", "mix"];
    for suffix in suffixes {
        if value.ends_with(suffix) {
            value = value.trim_end_matches(suffix).trim().to_string();
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_punctuation_and_whitespace() {
        let actual = normalize_text("  A.B,   C!?  ");
        assert_eq!(actual, "a b c");
    }

    #[test]
    fn normalizes_featuring_variants() {
        let actual = normalize_text("Artist Featuring Guest feat. Other");
        assert_eq!(actual, "artist feat guest feat other");
    }

    #[test]
    fn strips_common_title_suffixes() {
        let actual = normalize_title_suffixes("My Song [Radio Edit]");
        assert_eq!(actual, "my song");
    }
}
