use crate::models::{AudioState, YTVideoInfo};
use crate::ytdlp_installer::YTDLPInstaller;
use rodio::{buffer::SamplesBuffer, OutputStream, Sink, Source};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tauri::{AppHandle, Emitter};
use std::sync::mpsc as std_mpsc;

// Symphonia imports for direct decoding + fast seeking
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;



// Custom audio source that wraps Symphonia for low-memory streaming + fast seeking
struct SymphoniaSource {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    current_buf: Vec<i16>,
    buf_index: usize,
}

impl SymphoniaSource {
    /// Create a new SymphoniaSource from a file path
    fn new(path: &str) -> Result<Self, String> {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Set format hint from file extension
        let mut hint = Hint::new();
        if let Some(ext) = std::path::Path::new(path).extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        // Probe the format
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| format!("Failed to probe audio format: {}", e))?;

        let format_reader = probed.format;

        // Get the default audio track
        let track = format_reader.default_track()
            .ok_or("No audio track found")?;

        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        let channels = codec_params.channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);

        println!("🎵 SymphoniaSource: {}ch, {}Hz", channels, sample_rate);

        // Create decoder
        let decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            sample_rate,
            channels,
            current_buf: Vec::new(),
            buf_index: 0,
        })
    }

    /// Create a SymphoniaSource from a Read stream (non-seekable, forward-only playback)
    fn from_reader(reader: impl std::io::Read + Send + Sync + 'static) -> Result<Self, String> {
        let start = Instant::now();

        let read_only = ReadOnlySource::new(reader);
        let mss = MediaSourceStream::new(Box::new(read_only), Default::default());

        let hint = Hint::new();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| format!("Failed to probe stream: {}", e))?;

        let format_reader = probed.format;
        let track = format_reader.default_track().ok_or("No audio track found")?;
        let track_id = track.id;
        let codec_params = track.codec_params.clone();
        let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);

        println!("🎵 SymphoniaSource from stream: {}ch, {}Hz (ready in {:.0}ms)",
            channels, sample_rate, start.elapsed().as_millis());

        let decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        Ok(Self {
            format_reader, decoder, track_id, sample_rate, channels,
            current_buf: Vec::new(), buf_index: 0,
        })
    }

    /// Create a new SymphoniaSource and seek to a position (FAST - uses seek tables)
    fn seek_to_time(path: &str, position_secs: f64) -> Result<Self, String> {
        let mut source = Self::new(path)?;

        // Use Coarse seeking - uses FLAC seek tables for O(1) seeking
        let seek_time = Time {
            seconds: position_secs as u64,
            frac: position_secs.fract(),
        };

        source.format_reader
            .seek(SeekMode::Coarse, SeekTo::Time { time: seek_time, track_id: None })
            .map_err(|e| format!("Failed to seek: {}", e))?;

        // Reset decoder state after seeking
        source.decoder.reset();

        // Clear any stale buffer
        source.current_buf.clear();
        source.buf_index = 0;

        Ok(source)
    }

    /// Decode the next packet into the internal buffer
    fn decode_next_packet(&mut self) -> bool {
        loop {
            let packet = match self.format_reader.next_packet() {
                Ok(p) => p,
                Err(_) => return false, // End of stream or error
            };

            // Skip packets from other tracks
            if packet.track_id() != self.track_id {
                continue;
            }

            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(_) => return false,
            };

            // Convert decoded audio to interleaved i16 samples
            let spec = *decoded.spec();
            let capacity = decoded.capacity() as u64;

            if capacity == 0 {
                continue;
            }

            let mut sample_buf = SampleBuffer::<i16>::new(capacity, spec);
            sample_buf.copy_interleaved_ref(decoded);

            self.current_buf = sample_buf.samples().to_vec();
            self.buf_index = 0;
            return true;
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        // If buffer is exhausted, decode the next packet
        if self.buf_index >= self.current_buf.len() {
            if !self.decode_next_packet() {
                return None; // End of stream
            }
        }

        let sample = self.current_buf[self.buf_index];
        self.buf_index += 1;
        Some(sample)
    }
}

impl Source for SymphoniaSource {
    fn current_frame_len(&self) -> Option<usize> {
        if self.buf_index < self.current_buf.len() {
            Some(self.current_buf.len() - self.buf_index)
        } else {
            None
        }
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

// Helper function to build YouTube bypass arguments
// Start with no bypass, escalate if needed
fn build_youtube_bypass_args() -> Vec<String> {
    // Start with no bypass - just use normal yt-dlp
    // The ytdlp_manager already handles the escalation through multiple methods if needed
    // For audio playback, we start simple and let yt-dlp work normally
    println!("🎯 Using normal yt-dlp for audio playback (no bypass by default)");
    Vec::new()
}

// Commands that can be sent to the audio thread
enum AudioCommand {
    Play(YTVideoInfo),
    PlayFromFile(YTVideoInfo, String), // track, file_path
    TogglePlayPause,
    Pause,
    Stop,
    Seek(f64), // position in seconds
    SetVolume(f32),
    SetPlaybackRate(f32),
}

pub struct AudioManager {
    state: Arc<Mutex<AudioState>>,
    command_tx: mpsc::UnboundedSender<AudioCommand>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    state_change_rx: Arc<Mutex<std_mpsc::Receiver<()>>>,
    track_ended_rx: Arc<Mutex<std_mpsc::Receiver<()>>>,
}

impl AudioManager {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (state_change_tx, state_change_rx) = std_mpsc::channel();
        let (track_ended_tx, track_ended_rx) = std_mpsc::channel();
        let state = Arc::new(Mutex::new(AudioState::default()));

        // Spawn dedicated audio thread
        let state_clone = Arc::clone(&state);
        std::thread::spawn(move || {
            audio_thread(command_rx, state_clone, state_change_tx, track_ended_tx);
        });

        Self {
            state,
            command_tx,
            app_handle: Arc::new(Mutex::new(None)),
            state_change_rx: Arc::new(Mutex::new(state_change_rx)),
            track_ended_rx: Arc::new(Mutex::new(track_ended_rx)),
        }
    }

    pub async fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().await = Some(handle.clone());

        // Spawn a task to listen for state changes and emit events
        let state = Arc::clone(&self.state);
        let state_change_rx = Arc::clone(&self.state_change_rx);
        let track_ended_rx = Arc::clone(&self.track_ended_rx);
        let handle_clone = handle.clone();

        tokio::spawn(async move {
            loop {
                // Check for state change notifications (non-blocking)
                let has_change = {
                    let rx = state_change_rx.lock().await;
                    rx.try_recv().is_ok()
                };

                if has_change {
                    let current_state = state.lock().await.clone();
                    let _ = handle.emit("playback-state-changed", current_state);
                }

                // Check for track-ended notifications
                let track_ended = {
                    let rx = track_ended_rx.lock().await;
                    rx.try_recv().is_ok()
                };

                if track_ended {
                    println!("🔔 Emitting track-ended event");
                    let _ = handle_clone.emit("track-ended", ());
                }

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });
    }

    pub async fn play(&self, track: YTVideoInfo) -> Result<(), String> {
        println!("🎵 Playing track: {}", track.title);

        // Update state immediately for UI feedback
        {
            let mut state = self.state.lock().await;
            state.current_track = Some(track.clone());
            state.is_loading = true;
            state.is_playing = false;
            state.current_position = 0.0;
            state.duration = track.duration as f64;
        }

        self.emit_state_change().await;

        // Send play command to audio thread
        self.command_tx
            .send(AudioCommand::Play(track))
            .map_err(|_| "Audio thread disconnected".to_string())?;

        Ok(())
    }

    pub async fn play_from_file(&self, track: YTVideoInfo, file_path: String) -> Result<(), String> {
        println!("🎵 Playing track from file: {} ({})", track.title, file_path);

        // Update state immediately for UI feedback
        {
            let mut state = self.state.lock().await;
            state.current_track = Some(track.clone());
            state.is_loading = true;
            state.is_playing = false;
            state.current_position = 0.0;
            state.duration = track.duration as f64;
        }

        self.emit_state_change().await;

        // Send play from file command to audio thread
        self.command_tx
            .send(AudioCommand::PlayFromFile(track, file_path))
            .map_err(|_| "Audio thread disconnected".to_string())?;

        Ok(())
    }

    pub async fn toggle_play_pause(&self) -> Result<(), String> {
        self.command_tx
            .send(AudioCommand::TogglePlayPause)
            .map_err(|_| "Audio thread disconnected".to_string())?;
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), String> {
        self.command_tx
            .send(AudioCommand::Pause)
            .map_err(|_| "Audio thread disconnected".to_string())?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.command_tx
            .send(AudioCommand::Stop)
            .map_err(|_| "Audio thread disconnected".to_string())?;
        Ok(())
    }

    pub async fn seek(&self, position: f64) -> Result<(), String> {
        let duration = self.state.lock().await.duration;
        let position = position.min(duration).max(0.0);

        // Send seek command to audio thread
        self.command_tx
            .send(AudioCommand::Seek(position))
            .map_err(|_| "Audio thread disconnected".to_string())?;

        Ok(())
    }

    pub async fn set_volume(&self, volume: f32) -> Result<(), String> {
        let volume = volume.max(0.0).min(1.0);

        // Update state
        self.state.lock().await.volume = volume;

        // Send to audio thread
        self.command_tx
            .send(AudioCommand::SetVolume(volume))
            .map_err(|_| "Audio thread disconnected".to_string())?;

        self.emit_state_change().await;
        Ok(())
    }

    pub async fn set_playback_rate(&self, rate: f32) -> Result<(), String> {
        let rate = rate.max(0.25).min(2.0);

        // Update state
        self.state.lock().await.playback_rate = rate;

        // Send to audio thread
        self.command_tx
            .send(AudioCommand::SetPlaybackRate(rate))
            .map_err(|_| "Audio thread disconnected".to_string())?;

        self.emit_state_change().await;
        Ok(())
    }

    pub async fn get_state(&self) -> AudioState {
        self.state.lock().await.clone()
    }

    async fn emit_state_change(&self) {
        let app_guard = self.app_handle.lock().await;
        if let Some(handle) = app_guard.as_ref() {
            let state = self.state.lock().await;
            let _ = handle.emit("playback-state-changed", state.clone());
        }
    }
}

// Audio playback constants
const SAMPLE_RATE: u32 = 44100;
const CHANNELS: u16 = 2;

// Tracks playback position using elapsed time
struct PlaybackTimer {
    start_instant: Option<Instant>,
    start_position: f64,
    playback_rate: f32,
}

impl PlaybackTimer {
    fn new() -> Self {
        Self {
            start_instant: None,
            start_position: 0.0,
            playback_rate: 1.0,
        }
    }

    fn start(&mut self, position: f64, rate: f32) {
        self.start_instant = Some(Instant::now());
        self.start_position = position;
        self.playback_rate = rate;
    }

    fn pause(&mut self) -> f64 {
        let position = self.current_position();
        self.start_position = position; // Save current position so resume works correctly
        self.start_instant = None;
        position
    }

    fn seek(&mut self, position: f64) {
        self.start_position = position;
        if self.start_instant.is_some() {
            self.start_instant = Some(Instant::now());
        }
    }

    fn set_rate(&mut self, rate: f32) {
        // Update position before changing rate
        if self.start_instant.is_some() {
            self.start_position = self.current_position();
            self.start_instant = Some(Instant::now());
        }
        self.playback_rate = rate;
    }

    fn current_position(&self) -> f64 {
        match self.start_instant {
            Some(start) => {
                let elapsed = start.elapsed().as_secs_f64();
                self.start_position + (elapsed * self.playback_rate as f64)
            }
            None => self.start_position,
        }
    }

    fn is_playing(&self) -> bool {
        self.start_instant.is_some()
    }

    fn stop(&mut self) {
        self.start_instant = None;
        self.start_position = 0.0;
    }
}

// The dedicated audio thread - owns OutputStream and Sink
fn audio_thread(
    mut command_rx: mpsc::UnboundedReceiver<AudioCommand>,
    state: Arc<Mutex<AudioState>>,
    state_change_tx: std_mpsc::Sender<()>,
    track_ended_tx: std_mpsc::Sender<()>,
) {
    // Create audio output stream once for this thread
    let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
        eprintln!("❌ Failed to create audio output");
        return;
    };
    println!("✅ Audio output stream created");

    let mut current_sink: Option<Sink> = None;
    let mut current_samples: Option<Vec<i16>> = None; // Store samples for seeking (memory-based playback)
    let mut current_file_path: Option<String> = None; // Store file path for seeking (file-based playback)
    let mut position_timer = PlaybackTimer::new(); // Track playback position
    let mut last_position_update = Instant::now();

    // Process commands with polling to allow periodic position updates
    loop {
        // Try to receive a command (non-blocking)
        let command = command_rx.try_recv().ok();

        // Check if track has ended (sink is empty)
        if let Some(sink) = &current_sink {
            if sink.empty() && position_timer.is_playing() {
                println!("🏁 Track ended (sink empty)");
                position_timer.stop();
                // Keep current_samples so we can restart the track if user presses play

                let mut state_guard = state.blocking_lock();
                let duration = state_guard.duration;
                state_guard.is_playing = false;
                state_guard.current_position = duration; // Set to exact duration
                drop(state_guard);

                // Emit both state change and track-ended event
                let _ = state_change_tx.send(());
                let _ = track_ended_tx.send(()); // Notify that track ended for auto-play

                current_sink = None; // Clear sink to stop the empty check, but samples remain
            }
        }

        // Periodically update position in state (every 500ms)
        if position_timer.is_playing() && last_position_update.elapsed() > std::time::Duration::from_millis(500) {
            let current_pos = position_timer.current_position();
            let duration = state.blocking_lock().duration;

            // Don't exceed duration
            let clamped_pos = current_pos.min(duration);

            {
                let mut state_guard = state.blocking_lock();
                state_guard.current_position = clamped_pos;
                // Set download_progress based on playback type
                // 1.0 = downloaded (file/memory), 0.0 = streaming (no seeking)
                state_guard.download_progress = if current_file_path.is_some() || current_samples.is_some() {
                    1.0
                } else {
                    0.0
                };
            }
            let _ = state_change_tx.send(());
            last_position_update = Instant::now();
        }

        let Some(command) = command else {
            // No command, sleep briefly and continue loop for position updates
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        };

        match command {
            AudioCommand::Play(track) => {
                // Stop current playback
                if let Some(sink) = current_sink.take() {
                    sink.stop();
                }
                current_samples = None;
                current_file_path = None;

                let video_url = format!("https://www.youtube.com/watch?v={}", track.id);
                println!("📥 Getting audio URL from yt-dlp...");

                // Get yt-dlp path
                let ytdlp_path = YTDLPInstaller::get_ytdlp_path();

                // Build bypass arguments
                let bypass_args = build_youtube_bypass_args();

                // Build complete argument list to get the direct audio URL
                let mut ytdlp_args = vec![
                    "-f".to_string(),
                    "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio".to_string(),
                    "-g".to_string(), // Get URL only
                    "--no-warnings".to_string(),
                ];
                ytdlp_args.extend(bypass_args);
                ytdlp_args.push(video_url.clone());

                let args_refs: Vec<&str> = ytdlp_args.iter().map(|s| s.as_str()).collect();

                // Get the audio URL
                let ytdlp_output = match Command::new(&ytdlp_path)
                    .args(&args_refs)
                    .output()
                {
                    Ok(output) => output,
                    Err(e) => {
                        eprintln!("❌ Failed to run yt-dlp: {}", e);
                        continue;
                    }
                };

                if !ytdlp_output.status.success() {
                    eprintln!("❌ yt-dlp failed to get audio URL");
                    continue;
                }

                let audio_url = String::from_utf8_lossy(&ytdlp_output.stdout).trim().to_string();

                if audio_url.is_empty() {
                    eprintln!("❌ No audio URL returned from yt-dlp");
                    continue;
                }

                println!("✅ Got audio URL, starting hybrid streaming...");

                // Start streaming playback (instant, no seeking)
                let stream_response = match reqwest::blocking::get(&audio_url) {
                    Ok(resp) => resp,
                    Err(e) => {
                        eprintln!("❌ Failed to stream audio: {}", e);
                        {
                            let mut state_guard = state.blocking_lock();
                            state_guard.is_loading = false;
                        }
                        let _ = state_change_tx.send(());
                        continue;
                    }
                };

                let symphonia_result = SymphoniaSource::from_reader(stream_response);

                match symphonia_result {
                    Ok(source) => {
                        println!("✅ Streaming started (instant playback)");

                        let Ok(sink) = Sink::try_new(&stream_handle) else {
                            eprintln!("❌ Failed to create sink");
                            continue;
                        };

                        let (volume, rate) = {
                            let state_guard = state.blocking_lock();
                            (state_guard.volume, state_guard.playback_rate)
                        };

                        sink.set_volume(volume);
                        sink.set_speed(rate);
                        sink.append(source.convert_samples::<f32>());
                        sink.play();

                        current_sink = Some(sink);

                        // Start position timer
                        position_timer.start(0.0, rate);
                        last_position_update = Instant::now();

                        // Update state
                        {
                            let mut state_guard = state.blocking_lock();
                            state_guard.is_loading = false;
                            state_guard.is_playing = true;
                            state_guard.current_position = 0.0;
                            state_guard.download_progress = 0.0; // 0.0 = streaming, seeking disabled
                        }
                        let _ = state_change_tx.send(());

                        println!("▶️ Streaming: {} (instant play, no seeking)", track.title);
                    }
                    Err(e) => {
                        eprintln!("⚠️ Stream failed: {}", e);
                        eprintln!("❌ Cannot play this track");
                        {
                            let mut state_guard = state.blocking_lock();
                            state_guard.is_loading = false;
                            state_guard.is_playing = false;
                        }
                        let _ = state_change_tx.send(());
                    }
                }
            }
            AudioCommand::PlayFromFile(track, file_path) => {
                // Stop current playback
                if let Some(sink) = current_sink.take() {
                    sink.stop();
                }
                current_samples = None;
                current_file_path = None;


                println!("📥 Playing from local file: {}", file_path);

                // Try SymphoniaSource first (low memory + fast seeking)
                let symphonia_result = SymphoniaSource::new(&file_path);

                match symphonia_result {
                    Ok(source) => {
                        println!("✅ SymphoniaSource created (low memory streaming)");

                        let Ok(sink) = Sink::try_new(&stream_handle) else {
                            eprintln!("❌ Failed to create sink");
                            continue;
                        };

                        let (volume, rate) = {
                            let state_guard = state.blocking_lock();
                            (state_guard.volume, state_guard.playback_rate)
                        };

                        sink.set_volume(volume);
                        sink.set_speed(rate);
                        sink.append(source.convert_samples::<f32>());
                        sink.play();

                        current_sink = Some(sink);
                        current_file_path = Some(file_path.clone());

                        position_timer.start(0.0, rate);
                        last_position_update = Instant::now();

                        {
                            let mut state_guard = state.blocking_lock();
                            state_guard.is_loading = false;
                            state_guard.is_playing = true;
                            state_guard.current_position = 0.0;
                            state_guard.download_progress = 1.0; // File fully available
                        }
                        let _ = state_change_tx.send(());

                        println!("▶️ Streaming: {} (LOW MEMORY + FAST SEEK)", track.title);
                    }
                    Err(e) => {
                        // Fallback to memory mode using ffmpeg
                        eprintln!("⚠️ SymphoniaSource failed: {}", e);
                        println!("📥 Falling back to memory mode (ffmpeg)...");

                        let ffmpeg_output = match Command::new("ffmpeg")
                            .args(&[
                                "-i", &file_path,
                                "-f", "s16le",
                                "-acodec", "pcm_s16le",
                                "-ar", &SAMPLE_RATE.to_string(),
                                "-ac", &CHANNELS.to_string(),
                                "-loglevel", "error",
                                "pipe:1",
                            ])
                            .stdout(Stdio::piped())
                            .stderr(Stdio::null())
                            .output()
                        {
                            Ok(output) => output,
                            Err(e) => {
                                eprintln!("❌ Failed to run ffmpeg: {}", e);
                                continue;
                            }
                        };

                        if !ffmpeg_output.status.success() {
                            eprintln!("❌ ffmpeg conversion failed");
                            continue;
                        }

                        let pcm_bytes = ffmpeg_output.stdout;
                        if pcm_bytes.is_empty() {
                            eprintln!("❌ No audio data from ffmpeg");
                            continue;
                        }

                        let samples: Vec<i16> = pcm_bytes
                            .chunks_exact(2)
                            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                            .collect();

                        current_samples = Some(samples.clone());

                        let source = SamplesBuffer::new(CHANNELS, SAMPLE_RATE, samples);

                        let Ok(sink) = Sink::try_new(&stream_handle) else {
                            eprintln!("❌ Failed to create sink");
                            continue;
                        };

                        let (volume, rate) = {
                            let state_guard = state.blocking_lock();
                            (state_guard.volume, state_guard.playback_rate)
                        };

                        sink.set_volume(volume);
                        sink.set_speed(rate);
                        sink.append(source.convert_samples::<f32>());
                        sink.play();

                        current_sink = Some(sink);

                        position_timer.start(0.0, rate);
                        last_position_update = Instant::now();

                        {
                            let mut state_guard = state.blocking_lock();
                            state_guard.is_loading = false;
                            state_guard.is_playing = true;
                            state_guard.current_position = 0.0;
                            state_guard.download_progress = 1.0; // Fully in memory
                        }
                        let _ = state_change_tx.send(());

                        println!("▶️ Playing from memory: {} (FALLBACK)", track.title);
                    }
                }
            }
            AudioCommand::Seek(position) => {
                // Only allow seeking for downloaded tracks (file-based or memory-based)
                // Streaming tracks don't support seeking
                if current_file_path.is_none() && current_samples.is_none() {
                    println!("⏩ Seeking not available - track must be downloaded for full controls");
                    continue; // Don't stop playback, just ignore the seek
                }

                // Stop current playback
                if let Some(sink) = current_sink.take() {
                    sink.stop();
                }

                // Handle file-based seeking (Symphonia fast seek using seek tables)
                if let Some(file_path) = &current_file_path {
                    let seek_start = Instant::now();
                    println!("⏩ Seeking to {:.1}s...", position);

                    // Use SymphoniaSource::seek_to_time - FAST (uses FLAC seek tables)
                    let source = match SymphoniaSource::seek_to_time(file_path, position) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("❌ Failed to seek: {}", e);
                            continue;
                        }
                    };

                    let Ok(sink) = Sink::try_new(&stream_handle) else {
                        eprintln!("❌ Failed to create sink for seek");
                        continue;
                    };

                    let (volume, rate) = {
                        let state_guard = state.blocking_lock();
                        (state_guard.volume, state_guard.playback_rate)
                    };

                    sink.set_volume(volume);
                    sink.set_speed(rate);
                    sink.append(source.convert_samples::<f32>());
                    sink.play();

                    current_sink = Some(sink);

                    position_timer.start(position, rate);
                    last_position_update = Instant::now();

                    {
                        let mut state_guard = state.blocking_lock();
                        state_guard.current_position = position;
                        state_guard.is_playing = true;
                    }
                    let _ = state_change_tx.send(());

                    let seek_ms = seek_start.elapsed().as_secs_f64() * 1000.0;
                    println!("⏩ Seeked to {:.1}s - took {:.1}ms", position, seek_ms);
                }
                // Handle memory-based seeking (for non-downloaded tracks)
                else if let Some(samples) = &current_samples {
                    // Calculate sample index from position
                    let sample_index = (position * SAMPLE_RATE as f64 * CHANNELS as f64) as usize;
                    let sample_index = sample_index.min(samples.len());

                    // Get samples from position onwards
                    let remaining_samples: Vec<i16> = samples[sample_index..].to_vec();

                    if remaining_samples.is_empty() {
                        println!("⏩ Seek position at end of track");
                        continue;
                    }

                    // Create source from remaining samples
                    let source = SamplesBuffer::new(CHANNELS, SAMPLE_RATE, remaining_samples);

                    // Create new sink
                    let Ok(sink) = Sink::try_new(&stream_handle) else {
                        eprintln!("❌ Failed to create sink for seek");
                        continue;
                    };

                    // Get current settings from state
                    let (volume, rate) = {
                        let state_guard = state.blocking_lock();
                        (state_guard.volume, state_guard.playback_rate)
                    };

                    sink.set_volume(volume);
                    sink.set_speed(rate);
                    sink.append(source.convert_samples::<f32>());
                    sink.play();

                    current_sink = Some(sink);

                    // Update position timer
                    position_timer.start(position, rate);
                    last_position_update = Instant::now();

                    // Update state
                    {
                        let mut state_guard = state.blocking_lock();
                        state_guard.current_position = position;
                        state_guard.is_playing = true;
                    }
                    let _ = state_change_tx.send(());

                    println!("⏩ Seeked to {:.1}s (memory-based)", position);
                }
            }
            AudioCommand::TogglePlayPause => {
                let state_guard = state.blocking_lock();
                let is_playing = state_guard.is_playing;
                let duration = state_guard.duration;
                let current_pos = position_timer.current_position();
                let rate = state_guard.playback_rate;
                let volume = state_guard.volume;
                drop(state_guard);

                // Check if track ended (at or near duration, or sink is gone) - need to restart
                let has_track = current_samples.is_some() || current_file_path.is_some();
                let track_ended = (current_pos >= duration - 0.5 && duration > 0.0) ||
                                  (has_track && current_sink.is_none());

                if is_playing {
                    // Pause
                    if let Some(sink) = &current_sink {
                        sink.pause();
                        let paused_pos = position_timer.pause();
                        let mut state_guard = state.blocking_lock();
                        state_guard.is_playing = false;
                        state_guard.current_position = paused_pos;
                        println!("⏸️ Paused at {:.1}s", paused_pos);
                        drop(state_guard);
                        let _ = state_change_tx.send(());
                    }
                } else if track_ended {
                    // Track ended, restart from beginning
                    // Stop current sink if exists
                    if let Some(sink) = current_sink.take() {
                        sink.stop();
                    }

                    // Handle file-based restart using SymphoniaSource
                    if let Some(file_path) = &current_file_path {
                        let source = match SymphoniaSource::new(file_path) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("❌ Failed to create source for restart: {}", e);
                                continue;
                            }
                        };

                        if let Ok(sink) = Sink::try_new(&stream_handle) {
                            sink.set_volume(volume);
                            sink.set_speed(rate);
                            sink.append(source.convert_samples::<f32>());
                            sink.play();
                            current_sink = Some(sink);

                            position_timer.start(0.0, rate);
                            last_position_update = Instant::now();

                            let mut state_guard = state.blocking_lock();
                            state_guard.is_playing = true;
                            state_guard.current_position = 0.0;
                            drop(state_guard);
                            let _ = state_change_tx.send(());
                            println!("🔄 Restarted track from beginning");
                        }
                    }
                    // Handle memory-based restart
                    else if let Some(samples) = &current_samples {
                        let source = SamplesBuffer::new(CHANNELS, SAMPLE_RATE, samples.clone());
                        if let Ok(sink) = Sink::try_new(&stream_handle) {
                            sink.set_volume(volume);
                            sink.set_speed(rate);
                            sink.append(source.convert_samples::<f32>());
                            sink.play();
                            current_sink = Some(sink);

                            position_timer.start(0.0, rate);
                            last_position_update = Instant::now();

                            let mut state_guard = state.blocking_lock();
                            state_guard.is_playing = true;
                            state_guard.current_position = 0.0;
                            drop(state_guard);
                            let _ = state_change_tx.send(());
                            println!("🔄 Restarted track from beginning (memory-based)");
                        }
                    }
                } else {
                    // Normal resume
                    if let Some(sink) = &current_sink {
                        sink.play();
                        position_timer.start(current_pos, rate);
                        let mut state_guard = state.blocking_lock();
                        state_guard.is_playing = true;
                        state_guard.current_position = current_pos;
                        println!("▶️ Resumed from {:.1}s (rate: {:.2})", current_pos, rate);
                        drop(state_guard);
                        last_position_update = Instant::now();
                        let _ = state_change_tx.send(());
                    }
                }
            }
            AudioCommand::Pause => {
                if let Some(sink) = &current_sink {
                    sink.pause();
                    // Pause timer and get current position
                    let current_pos = position_timer.pause();
                    let mut state_guard = state.blocking_lock();
                    state_guard.is_playing = false;
                    state_guard.current_position = current_pos;
                    println!("⏸️ Explicit pause at {:.1}s", current_pos);
                    drop(state_guard);
                    let _ = state_change_tx.send(());
                }
            }
            AudioCommand::Stop => {
                if let Some(sink) = current_sink.take() {
                    sink.stop();
                }
                current_samples = None;
                current_file_path = None;
                position_timer.stop();
                let mut state_guard = state.blocking_lock();
                state_guard.is_playing = false;
                state_guard.current_position = 0.0;
                drop(state_guard);
                let _ = state_change_tx.send(());
                println!("⏹️ Stopped");
            }
            AudioCommand::SetVolume(volume) => {
                if let Some(sink) = &current_sink {
                    sink.set_volume(volume);
                }
            }
            AudioCommand::SetPlaybackRate(rate) => {
                if let Some(sink) = &current_sink {
                    sink.set_speed(rate);
                    // Update position timer with new rate
                    position_timer.set_rate(rate);
                }
            }
        }
    }
}
