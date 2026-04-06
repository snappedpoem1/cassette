use crate::Result;
use anyhow::anyhow;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rb::{RbConsumer, RbProducer, SpscRb, RB};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const RING_BUF_SAMPLES: usize = 192_000; // ~2 s at 48 kHz stereo

#[derive(Debug)]
pub enum PlayerCommand {
    Load(String),
    Play,
    Pause,
    Stop,
    Seek(f64),
    SetVolume(f32),
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Playing,
    Paused,
    Stopped,
    TrackEnded,
    Error(String),
}

pub struct Player {
    cmd_tx: std::sync::mpsc::SyncSender<PlayerCommand>,
    event_rx: Arc<Mutex<std::sync::mpsc::Receiver<PlayerEvent>>>,
    position_bits: Arc<AtomicU64>,
    duration_bits: Arc<AtomicU64>,
    is_playing: Arc<AtomicBool>,
    volume: Arc<Mutex<f32>>,
}

impl Player {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(32);
        let (evt_tx, evt_rx) = std::sync::mpsc::sync_channel(32);
        let position_bits = Arc::new(AtomicU64::new(0));
        let duration_bits = Arc::new(AtomicU64::new(0));
        let is_playing = Arc::new(AtomicBool::new(false));
        let volume = Arc::new(Mutex::new(0.8f32));

        let pos2 = Arc::clone(&position_bits);
        let dur2 = Arc::clone(&duration_bits);
        let play2 = Arc::clone(&is_playing);
        let vol2 = Arc::clone(&volume);

        std::thread::Builder::new()
            .name("cassette-player".into())
            .spawn(move || {
                player_thread(cmd_rx, evt_tx, pos2, dur2, play2, vol2);
            })
            .expect("spawn player thread");

        Self {
            cmd_tx,
            event_rx: Arc::new(Mutex::new(evt_rx)),
            position_bits,
            duration_bits,
            is_playing,
            volume,
        }
    }

    fn send(&self, cmd: PlayerCommand) {
        if self.cmd_tx.try_send(cmd).is_err() {
            tracing::warn!("[player] command channel full — command dropped");
        }
    }

    pub fn load(&self, path: String) {
        self.send(PlayerCommand::Load(path));
    }
    pub fn play(&self) {
        self.send(PlayerCommand::Play);
    }
    pub fn pause(&self) {
        self.send(PlayerCommand::Pause);
    }
    pub fn stop(&self) {
        self.send(PlayerCommand::Stop);
    }
    pub fn seek(&self, secs: f64) {
        self.send(PlayerCommand::Seek(secs));
    }

    pub fn set_volume(&self, vol: f32) {
        *self.volume.lock().unwrap() = vol.clamp(0.0, 1.0);
    }

    pub fn toggle(&self) {
        if self.is_playing.load(Ordering::Relaxed) {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn position_secs(&self) -> f64 {
        f64::from_bits(self.position_bits.load(Ordering::Relaxed))
    }

    pub fn duration_secs(&self) -> f64 {
        f64::from_bits(self.duration_bits.load(Ordering::Relaxed))
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn volume(&self) -> f32 {
        *self.volume.lock().unwrap()
    }

    /// Non-blocking drain of pending events
    pub fn drain_events(&self) -> Vec<PlayerEvent> {
        let rx = self.event_rx.lock().unwrap();
        let mut events = Vec::new();
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        events
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

// ── Player Thread ─────────────────────────────────────────────────────────────

fn player_thread(
    cmd_rx: std::sync::mpsc::Receiver<PlayerCommand>,
    evt_tx: std::sync::mpsc::SyncSender<PlayerEvent>,
    position_bits: Arc<AtomicU64>,
    duration_bits: Arc<AtomicU64>,
    is_playing: Arc<AtomicBool>,
    volume: Arc<Mutex<f32>>,
) {
    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            let _ = evt_tx.try_send(PlayerEvent::Error("No audio output device".into()));
            return;
        }
    };

    // Shared ring buffer between decode thread and CPAL callback
    let ring = SpscRb::<f32>::new(RING_BUF_SAMPLES);
    let (rb_prod, rb_cons) = (ring.producer(), ring.consumer());
    let rb_prod = Arc::new(Mutex::new(rb_prod));
    let rb_cons = Arc::new(Mutex::new(rb_cons));

    // Stream config
    let config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = evt_tx.try_send(PlayerEvent::Error(format!("Audio config error: {e}")));
            return;
        }
    };
    let _channels = config.channels() as usize;

    let vol_cb = Arc::clone(&volume);
    let play_cb = Arc::clone(&is_playing);
    let cons_cb = Arc::clone(&rb_cons);

    let stream = match device.build_output_stream(
        &config.config(),
        move |data: &mut [f32], _| {
            let playing = play_cb.load(Ordering::Relaxed);
            let vol = *vol_cb.lock().unwrap();
            if playing {
                let cons = cons_cb.lock().unwrap();
                let n = cons.read(data).unwrap_or(0);
                drop(cons);
                for s in &mut data[..n] {
                    *s *= vol;
                }
                for s in &mut data[n..] {
                    *s = 0.0;
                }
            } else {
                for s in data.iter_mut() {
                    *s = 0.0;
                }
            }
        },
        |e| eprintln!("[player] stream error: {e}"),
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            let _ = evt_tx.try_send(PlayerEvent::Error(format!("Stream error: {e}")));
            return;
        }
    };

    let _ = stream.play();

    let mut current_path: Option<String> = None;
    let mut decode_thread: Option<std::thread::JoinHandle<()>> = None;
    let decode_stop = Arc::new(AtomicBool::new(false));

    loop {
        // Collect all pending commands (blocking wait when idle, non-blocking when playing)
        let cmd = if is_playing.load(Ordering::Relaxed) {
            cmd_rx.try_recv().ok()
        } else {
            cmd_rx.recv().ok()
        };

        match cmd {
            Some(PlayerCommand::Quit) | None => break,

            Some(PlayerCommand::Load(path)) => {
                // Stop existing decode thread
                decode_stop.store(true, Ordering::Relaxed);
                if let Some(t) = decode_thread.take() {
                    let _ = t.join();
                }
                decode_stop.store(false, Ordering::Relaxed);
                is_playing.store(false, Ordering::Relaxed);
                position_bits.store(0f64.to_bits(), Ordering::Relaxed);

                current_path = Some(path.clone());
                // Probe duration
                if let Ok(dur) = probe_duration(&path) {
                    duration_bits.store(dur.to_bits(), Ordering::Relaxed);
                }

                // Spawn decode thread
                let prod = Arc::clone(&rb_prod);
                let stop = Arc::clone(&decode_stop);
                let pos = Arc::clone(&position_bits);
                let play = Arc::clone(&is_playing);
                let evt = evt_tx.clone();
                let p = path.clone();

                decode_thread = Some(
                    std::thread::Builder::new()
                        .name("cassette-decode".into())
                        .spawn(move || decode_loop(p, prod, stop, pos, play, evt, None))
                        .expect("spawn decode thread"),
                );

                is_playing.store(true, Ordering::Relaxed);
                let _ = evt_tx.try_send(PlayerEvent::Playing);
            }

            Some(PlayerCommand::Play) => {
                if current_path.is_some() {
                    is_playing.store(true, Ordering::Relaxed);
                    let _ = evt_tx.try_send(PlayerEvent::Playing);
                }
            }

            Some(PlayerCommand::Pause) => {
                is_playing.store(false, Ordering::Relaxed);
                let _ = evt_tx.try_send(PlayerEvent::Paused);
            }

            Some(PlayerCommand::Stop) => {
                decode_stop.store(true, Ordering::Relaxed);
                if let Some(t) = decode_thread.take() {
                    let _ = t.join();
                }
                decode_stop.store(false, Ordering::Relaxed);
                is_playing.store(false, Ordering::Relaxed);
                position_bits.store(0f64.to_bits(), Ordering::Relaxed);
                current_path = None;
                let _ = evt_tx.try_send(PlayerEvent::Stopped);
            }

            Some(PlayerCommand::SetVolume(v)) => {
                *volume.lock().unwrap() = v.clamp(0.0, 1.0);
            }

            Some(PlayerCommand::Seek(secs)) => {
                if let Some(ref path) = current_path {
                    // Stop current decode thread
                    decode_stop.store(true, Ordering::Relaxed);
                    if let Some(t) = decode_thread.take() {
                        let _ = t.join();
                    }
                    decode_stop.store(false, Ordering::Relaxed);

                    // Drain ring buffer so stale audio doesn't play after seek
                    {
                        let cons = rb_cons.lock().unwrap();
                        let mut drain = [0f32; 4096];
                        while cons.read(&mut drain).unwrap_or(0) > 0 {}
                    }

                    // Update position immediately
                    position_bits.store(secs.to_bits(), Ordering::Relaxed);

                    // Restart decode from the target position
                    let prod = Arc::clone(&rb_prod);
                    let stop = Arc::clone(&decode_stop);
                    let pos = Arc::clone(&position_bits);
                    let play = Arc::clone(&is_playing);
                    let evt = evt_tx.clone();
                    let p = path.clone();
                    let was_playing = is_playing.load(Ordering::Relaxed);

                    decode_thread = Some(
                        std::thread::Builder::new()
                            .name("cassette-decode".into())
                            .spawn(move || decode_loop(p, prod, stop, pos, play, evt, Some(secs)))
                            .expect("spawn decode thread"),
                    );

                    if was_playing {
                        is_playing.store(true, Ordering::Relaxed);
                    }
                }
            }
        }

        // Small sleep to avoid burning CPU when idle
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Unified decode loop. Pass `seek_to = Some(secs)` to seek before starting playback,
/// or `None` to play from the current position (typically the beginning after a fresh load).
fn decode_loop(
    path: String,
    prod: Arc<Mutex<rb::Producer<f32>>>,
    stop: Arc<AtomicBool>,
    position_bits: Arc<AtomicU64>,
    is_playing: Arc<AtomicBool>,
    evt_tx: std::sync::mpsc::SyncSender<PlayerEvent>,
    seek_to: Option<f64>,
) {
    use symphonia::core::formats::{SeekMode, SeekTo};
    use symphonia::core::units::Time;

    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            let _ = evt_tx.try_send(PlayerEvent::Error(format!("Cannot open file: {e}")));
            return;
        }
    };

    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
    {
        hint.with_extension(ext);
    }

    let probed = match symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) {
        Ok(p) => p,
        Err(e) => {
            let _ = evt_tx.try_send(PlayerEvent::Error(format!("Format probe failed: {e}")));
            return;
        }
    };

    let mut format = probed.format;
    let track = match format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
    {
        Some(t) => t.clone(),
        None => {
            let _ = evt_tx.try_send(PlayerEvent::Error("No audio track found".into()));
            return;
        }
    };

    let track_id = track.id;
    let time_base = track.codec_params.time_base;

    let mut decoder = match symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
    {
        Ok(d) => d,
        Err(e) => {
            let _ = evt_tx.try_send(PlayerEvent::Error(format!("Decoder error: {e}")));
            return;
        }
    };

    if let Some(seek_secs) = seek_to {
        let seek_time = Time::new(seek_secs as u64, seek_secs.fract());
        if let Err(e) = format.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: seek_time,
                track_id: Some(track_id),
            },
        ) {
            let _ = evt_tx.try_send(PlayerEvent::Error(format!("Seek failed: {e}")));
            return;
        }
        decoder.reset();
    }

    let mut sample_buf: Option<symphonia::core::audio::SampleBuffer<f32>> = None;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        if !is_playing.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                let _ = evt_tx.try_send(PlayerEvent::TrackEnded);
                break;
            }
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        if let Some(tb) = time_base {
            let ts = packet.ts();
            let secs = tb.calc_time(ts).seconds as f64 + tb.calc_time(ts).frac;
            position_bits.store(secs.to_bits(), Ordering::Relaxed);
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        if sample_buf.is_none() {
            sample_buf = Some(symphonia::core::audio::SampleBuffer::<f32>::new(
                decoded.capacity() as u64,
                spec,
            ));
        }
        if let Some(ref mut sb) = sample_buf {
            sb.copy_interleaved_ref(decoded);
            let samples = sb.samples();

            // Write to ring buffer, spin if full
            let prod = prod.lock().unwrap();
            let mut written = 0;
            while written < samples.len() {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                match prod.write(&samples[written..]) {
                    Ok(n) => written += n,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(1)),
                }
            }
        }
    }
}

fn probe_duration(path: &str) -> Result<f64> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
    {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| anyhow!("{e}"))?;
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no track"))?;
    if let (Some(tb), Some(frames)) = (track.codec_params.time_base, track.codec_params.n_frames) {
        let t = tb.calc_time(frames);
        return Ok(t.seconds as f64 + t.frac);
    }
    Ok(0.0)
}
