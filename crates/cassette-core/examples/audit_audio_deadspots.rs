use serde::Serialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
struct DeadspotFinding {
    path: String,
    codec: String,
    sample_rate: u32,
    channels: u32,
    analyzed_seconds: f64,
    peak: f32,
    rms: f32,
    active_ratio: f64,
    longest_silence_seconds: f64,
    reasons: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct DeadspotReport {
    root: String,
    total_audio_files: usize,
    analyzed_files: usize,
    decode_failures: usize,
    timed_out_files: usize,
    probable_silent_placeholders: usize,
    files_with_deadspots: usize,
    examples: Vec<DeadspotFinding>,
}

#[derive(Debug, Clone)]
struct DeadspotConfig {
    silence_threshold: f32,
    min_active_ratio: f64,
    long_silence_seconds: f64,
    max_analyze_seconds: f64,
    max_examples: usize,
    per_file_timeout_seconds: u64,
}

impl DeadspotConfig {
    fn from_env() -> Self {
        let silence_threshold = std::env::var("CASSETTE_DEADSPOT_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0008);
        let min_active_ratio = std::env::var("CASSETTE_DEADSPOT_MIN_ACTIVE_RATIO")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.01);
        let long_silence_seconds = std::env::var("CASSETTE_DEADSPOT_LONG_SILENCE_SECS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(8.0);
        let max_analyze_seconds = std::env::var("CASSETTE_DEADSPOT_MAX_ANALYZE_SECS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(120.0);
        let max_examples = std::env::var("CASSETTE_DEADSPOT_MAX_EXAMPLES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1000);
        let per_file_timeout_seconds = std::env::var("CASSETTE_DEADSPOT_PER_FILE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);

        Self {
            silence_threshold,
            min_active_ratio,
            long_silence_seconds,
            max_analyze_seconds,
            max_examples,
            per_file_timeout_seconds,
        }
    }
}

#[derive(Debug)]
struct TrackStats {
    codec: String,
    sample_rate: u32,
    channels: u32,
    analyzed_seconds: f64,
    peak: f32,
    rms: f32,
    active_ratio: f64,
    longest_silence_seconds: f64,
    reasons: Vec<String>,
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

fn analyze_track(path: &Path, config: &DeadspotConfig) -> Result<Option<TrackStats>, String> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|v| v.to_str()) {
        hint.with_extension(ext);
    }

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| e.to_string())?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .cloned()
        .ok_or_else(|| "no decodable track".to_string())?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| "missing sample rate".to_string())?;

    let channels = track
        .codec_params
        .channels
        .map(|v| v.count() as u32)
        .ok_or_else(|| "missing channel count".to_string())?;

    let track_id = track.id;
    let codec = format!("{:?}", track.codec_params.codec);

    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut total_frames: u64 = 0;
    let mut active_frames: u64 = 0;
    let mut sum_sq: f64 = 0.0;
    let mut total_samples: u64 = 0;
    let mut peak: f32 = 0.0;
    let mut current_silent_run: u64 = 0;
    let mut max_silent_run: u64 = 0;

    let max_frames = if config.max_analyze_seconds > 0.0 {
        (config.max_analyze_seconds * sample_rate as f64) as u64
    } else {
        u64::MAX
    };

    loop {
        if total_frames >= max_frames {
            break;
        }

        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        if sample_buf.is_none() {
            sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
        }

        if let Some(ref mut sb) = sample_buf {
            sb.copy_interleaved_ref(decoded);
            let samples = sb.samples();
            if samples.is_empty() {
                continue;
            }

            for &sample in samples {
                let abs = sample.abs();
                if abs > peak {
                    peak = abs;
                }
                let sq = (sample as f64) * (sample as f64);
                sum_sq += sq;
                total_samples += 1;
            }

            for frame in samples.chunks_exact(channels as usize) {
                let mut frame_peak = 0.0_f32;
                for &sample in frame {
                    let abs = sample.abs();
                    if abs > frame_peak {
                        frame_peak = abs;
                    }
                }

                if frame_peak >= config.silence_threshold {
                    active_frames += 1;
                    if current_silent_run > max_silent_run {
                        max_silent_run = current_silent_run;
                    }
                    current_silent_run = 0;
                } else {
                    current_silent_run += 1;
                }

                total_frames += 1;
                if total_frames >= max_frames {
                    break;
                }
            }
        }
    }

    if current_silent_run > max_silent_run {
        max_silent_run = current_silent_run;
    }

    if total_frames == 0 || total_samples == 0 {
        return Ok(None);
    }

    let analyzed_seconds = total_frames as f64 / sample_rate as f64;
    let rms = (sum_sq / total_samples as f64).sqrt() as f32;
    let active_ratio = active_frames as f64 / total_frames as f64;
    let longest_silence_seconds = max_silent_run as f64 / sample_rate as f64;

    let mut reasons = Vec::<String>::new();
    if active_ratio <= config.min_active_ratio || peak <= config.silence_threshold {
        reasons.push("probable silent placeholder".to_string());
    }
    if longest_silence_seconds >= config.long_silence_seconds {
        reasons.push(format!(
            "contains long silent run ({:.1}s)",
            longest_silence_seconds
        ));
    }

    if reasons.is_empty() {
        return Ok(None);
    }

    Ok(Some(TrackStats {
        codec,
        sample_rate,
        channels,
        analyzed_seconds,
        peak,
        rms,
        active_ratio,
        longest_silence_seconds,
        reasons,
    }))
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

    let cfg = DeadspotConfig::from_env();
    let worker_count = std::env::var("CASSETTE_DEADSPOT_WORKERS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|v| v.get().saturating_mul(2))
                .unwrap_or(16)
        })
        .clamp(1, 64);

    let mut paths = Vec::<PathBuf>::new();
    for entry in WalkDir::new(&root_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && is_audio_ext(entry.path()) {
            paths.push(entry.into_path());
        }
    }

    let total = paths.len();
    println!("Analyzing {total} audio files with {worker_count} workers...");

    let queue = Arc::new(Mutex::new(VecDeque::from(paths)));
    let processed = Arc::new(AtomicUsize::new(0));
    let report = Arc::new(tokio::sync::Mutex::new(DeadspotReport {
        root: root.clone(),
        total_audio_files: total,
        ..DeadspotReport::default()
    }));

    let mut tasks = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let report = Arc::clone(&report);
        let processed = Arc::clone(&processed);
        let cfg = cfg.clone();

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

                let result = tokio::task::spawn_blocking({
                    let path = path.clone();
                    let cfg = cfg.clone();
                    move || analyze_track(&path, &cfg).map(|found| (path, found))
                });

                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(cfg.per_file_timeout_seconds),
                    result,
                )
                .await;

                let idx = processed.fetch_add(1, Ordering::Relaxed) + 1;
                if idx % 500 == 0 {
                    println!("Processed {idx}/{total}");
                }

                let Ok(joined) = result else {
                    let mut write = report.lock().await;
                    write.timed_out_files += 1;
                    continue;
                };

                let Ok(Ok((path, found))) = joined else {
                    let mut write = report.lock().await;
                    write.decode_failures += 1;
                    continue;
                };

                let mut write = report.lock().await;
                write.analyzed_files += 1;

                if let Some(found) = found {
                    let is_placeholder = found
                        .reasons
                        .iter()
                        .any(|r| r.contains("silent placeholder"));
                    let has_deadspot = found.reasons.iter().any(|r| r.contains("long silent run"));

                    if is_placeholder {
                        write.probable_silent_placeholders += 1;
                    }
                    if has_deadspot {
                        write.files_with_deadspots += 1;
                    }

                    if write.examples.len() < cfg.max_examples {
                        write.examples.push(DeadspotFinding {
                            path: path.to_string_lossy().to_string(),
                            codec: found.codec,
                            sample_rate: found.sample_rate,
                            channels: found.channels,
                            analyzed_seconds: found.analyzed_seconds,
                            peak: found.peak,
                            rms: found.rms,
                            active_ratio: found.active_ratio,
                            longest_silence_seconds: found.longest_silence_seconds,
                            reasons: found.reasons,
                        });
                    }
                }
            }
        }));
    }

    for task in tasks {
        let _ = task.await;
    }

    let report = report.lock().await;
    println!("Analyzed files: {}", report.analyzed_files);
    println!("Decode failures: {}", report.decode_failures);
    println!("Timed out files: {}", report.timed_out_files);
    println!(
        "Probable silent placeholders: {}",
        report.probable_silent_placeholders
    );
    println!("Files with deadspots: {}", report.files_with_deadspots);
    println!("Flagged examples captured: {}", report.examples.len());

    let report_path = PathBuf::from("tmp").join("library_deadspot_report.json");
    if let Some(parent) = report_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&*report).map_err(|e| e.to_string())?;
    std::fs::write(&report_path, json).map_err(|e| e.to_string())?;

    println!("Report written to {}", report_path.display());
    Ok(())
}
