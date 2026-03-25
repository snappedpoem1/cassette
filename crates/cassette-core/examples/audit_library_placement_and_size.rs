use cassette_core::custodian::validation::{deep_validate_audio, ValidationStatus};
use cassette_core::library::organizer;
use cassette_core::library::read_track_metadata;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
struct MisplacedFinding {
    source_path: String,
    expected_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct SizeFinding {
    path: String,
    status: String,
    file_size: u64,
    duration_ms: Option<u64>,
    bitrate_kbps: Option<u32>,
    reasons: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct AuditReport {
    root: String,
    total_audio_files: usize,
    misplaced_files: usize,
    suspicious_size_files: usize,
    invalid_audio_files: usize,
    misplaced_examples: Vec<MisplacedFinding>,
    suspicious_size_examples: Vec<SizeFinding>,
    invalid_examples: Vec<SizeFinding>,
}

#[derive(Debug, Clone)]
struct AuditConfig {
    max_misplaced_examples: usize,
    max_suspicious_examples: usize,
    max_invalid_examples: usize,
}

impl AuditConfig {
    fn from_env() -> Self {
        let max_misplaced_examples = std::env::var("CASSETTE_AUDIT_MAX_MISPLACED")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(200);
        let max_suspicious_examples = std::env::var("CASSETTE_AUDIT_MAX_SUSPICIOUS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(500);
        let max_invalid_examples = std::env::var("CASSETTE_AUDIT_MAX_INVALID")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(500);

        Self {
            max_misplaced_examples,
            max_suspicious_examples,
            max_invalid_examples,
        }
    }
}

fn is_audio_ext(path: &Path) -> bool {
    const AUDIO_EXTENSIONS: &[&str] = &[
        "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "aiff", "wv", "ape",
    ];
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn normalize_for_compare(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "A:\\music".to_string());
    let root_path = PathBuf::from(&root);

    if !root_path.exists() {
        return Err(format!("Library root does not exist: {root}"));
    }

    let config = AuditConfig::from_env();

    let allowed_formats = vec![
        "flac".to_string(),
        "mp3".to_string(),
        "m4a".to_string(),
        "aac".to_string(),
        "ogg".to_string(),
        "opus".to_string(),
        "wav".to_string(),
        "aiff".to_string(),
    ];

    let mut paths = Vec::<PathBuf>::new();
    for entry in WalkDir::new(&root_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_type().is_file() && is_audio_ext(entry.path()) {
            paths.push(entry.into_path());
        }
    }

    println!("Auditing {} audio files under {}", paths.len(), root_path.display());

    let worker_count = std::env::var("CASSETTE_AUDIT_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|v| v.get())
                .unwrap_or(8)
        })
        .clamp(1, 32);

    let queue = Arc::new(Mutex::new(VecDeque::from(paths)));
    let findings = Arc::new(tokio::sync::Mutex::new(AuditReport {
        root: root.clone(),
        total_audio_files: 0,
        misplaced_files: 0,
        suspicious_size_files: 0,
        invalid_audio_files: 0,
        misplaced_examples: Vec::new(),
        suspicious_size_examples: Vec::new(),
        invalid_examples: Vec::new(),
    }));

    let processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total = {
        let guard = queue.lock().map_err(|_| "queue lock poisoned".to_string())?;
        guard.len()
    };

    let mut tasks = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let findings = Arc::clone(&findings);
        let processed = Arc::clone(&processed);
        let root = root.clone();
        let config = config.clone();
        let allowed_formats = allowed_formats.clone();

        tasks.push(tokio::spawn(async move {
            loop {
                let path = {
                    match queue.lock() {
                        Ok(mut guard) => guard.pop_front(),
                        Err(_) => None,
                    }
                };

                let Some(path) = path else {
                    break;
                };

                let report = deep_validate_audio(&path, &allowed_formats, 1.5, false);
                let index = processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                let mut write = findings.lock().await;
                write.total_audio_files += 1;

                match report.status {
                    ValidationStatus::SuspiciousSmallForDuration | ValidationStatus::ProbableTruncation => {
                        write.suspicious_size_files += 1;
                        if write.suspicious_size_examples.len() < config.max_suspicious_examples {
                            write.suspicious_size_examples.push(SizeFinding {
                                path: path.to_string_lossy().to_string(),
                                status: format!("{:?}", report.status),
                                file_size: report.file_size,
                                duration_ms: report.duration_ms,
                                bitrate_kbps: report.bitrate,
                                reasons: report.reasons.clone(),
                            });
                        }
                    }
                    ValidationStatus::Valid | ValidationStatus::IncompleteMetadata => {}
                    _ => {
                        write.invalid_audio_files += 1;
                        if write.invalid_examples.len() < config.max_invalid_examples {
                            write.invalid_examples.push(SizeFinding {
                                path: path.to_string_lossy().to_string(),
                                status: format!("{:?}", report.status),
                                file_size: report.file_size,
                                duration_ms: report.duration_ms,
                                bitrate_kbps: report.bitrate,
                                reasons: report.reasons.clone(),
                            });
                        }
                    }
                }

                if let Ok(track) = read_track_metadata(&path) {
                    let expected = organizer::canonical_path(&root, &track);
                    if normalize_for_compare(&path) != normalize_for_compare(&expected) {
                        write.misplaced_files += 1;
                        if write.misplaced_examples.len() < config.max_misplaced_examples {
                            write.misplaced_examples.push(MisplacedFinding {
                                source_path: path.to_string_lossy().to_string(),
                                expected_path: expected.to_string_lossy().to_string(),
                            });
                        }
                    }
                }

                if index % 1000 == 0 {
                    println!("Processed {index}/{total}");
                }
            }
        }));
    }

    for task in tasks {
        let _ = task.await;
    }

    let findings = findings.lock().await;
    println!("Total audio files: {}", findings.total_audio_files);
    println!("Misplaced files: {}", findings.misplaced_files);
    println!("Suspicious size files: {}", findings.suspicious_size_files);
    println!("Other invalid audio files: {}", findings.invalid_audio_files);

    let report_path = PathBuf::from("tmp").join("library_audit_report.json");
    if let Some(parent) = report_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&*findings).map_err(|e| e.to_string())?;
    std::fs::write(&report_path, json).map_err(|e| e.to_string())?;

    println!("Report written to {}", report_path.display());

    Ok(())
}