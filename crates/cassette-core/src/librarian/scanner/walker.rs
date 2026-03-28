use crate::librarian::config::ScanBehavior;
use crate::sources::is_audio_path as is_audio_file;
use std::path::PathBuf;

pub fn discover_audio_files(roots: &[PathBuf], behavior: &ScanBehavior) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(root)
            .follow_links(behavior.follow_symlinks)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if behavior.ignore_hidden_files {
                let hidden = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .map(|v| v.starts_with('.'))
                    .unwrap_or(false);
                if hidden {
                    continue;
                }
            }
            if entry.file_type().is_file() && is_audio_file(path) {
                out.push(path.to_path_buf());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn classifies_audio_extensions() {
        assert!(is_audio_file(Path::new("x.flac")));
        assert!(is_audio_file(Path::new("x.MP3")));
        assert!(!is_audio_file(Path::new("x.txt")));
    }
}
