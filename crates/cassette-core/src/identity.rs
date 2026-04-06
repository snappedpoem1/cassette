use std::borrow::Cow;

fn normalize_compatibility_punctuation(input: &str) -> Cow<'_, str> {
    if !input.chars().any(|ch| {
        matches!(
            ch,
            '\u{2018}'
                | '\u{2019}'
                | '\u{201a}'
                | '\u{201b}'
                | '\u{201c}'
                | '\u{201d}'
                | '\u{201e}'
                | '\u{201f}'
                | '\u{2010}'
                | '\u{2011}'
                | '\u{2012}'
                | '\u{2013}'
                | '\u{2014}'
                | '\u{2015}'
                | '\u{2026}'
        )
    }) {
        return Cow::Borrowed(input);
    }

    let normalized = input
        .chars()
        .map(|ch| match ch {
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => '\'',
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' => '"',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' => '-',
            '\u{2026}' => ' ',
            other => other,
        })
        .collect::<String>();
    Cow::Owned(normalized)
}

pub fn normalize_identity_text(input: &str) -> String {
    let normalized = normalize_compatibility_punctuation(input);
    normalized
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn normalize_artist_identity(input: &str) -> String {
    normalize_identity_text(&input.replace('&', " and "))
}

#[cfg(test)]
mod tests {
    use super::{normalize_artist_identity, normalize_identity_text};

    #[test]
    fn identity_normalizer_collapses_unicode_punctuation() {
        assert_eq!(
            normalize_identity_text("Bitch, Don’t Kill My Vibe — Deluxe"),
            "bitch don t kill my vibe deluxe"
        );
    }

    #[test]
    fn artist_normalizer_treats_ampersand_as_and() {
        assert_eq!(
            normalize_artist_identity("Simon & Garfunkel"),
            normalize_artist_identity("Simon and Garfunkel")
        );
    }
}
