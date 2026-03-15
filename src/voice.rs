//! Voice mode — speak to the agent, hear responses.
//!
//! When the `voice` feature is enabled and an API key is available,
//! uses OpenAI TTS/STT for audio synthesis and transcription.
//! Falls back to text-based input/output otherwise.
//!
//! # Audio I/O
//!
//! With the `voice` feature (which includes `cpal`), audio is captured
//! from the system's default input device and played back through the
//! default output device. If no audio device is available, the loop
//! falls back to text-only mode with a warning.
//!
//! # Wake Word
//!
//! [`WakeWordDetector`] performs simple case-insensitive substring matching
//! on the STT transcript. When the wake word is found, everything after it
//! is treated as the command to send to the chat model.

use std::sync::Arc;

use colored::Colorize;
use synaptic::core::{ChatModel, ChatRequest, Message};

use crate::config::VoiceConfig;

// ---------------------------------------------------------------------------
// Wake word detection
// ---------------------------------------------------------------------------

/// Detects a configurable wake word in transcribed text.
///
/// Uses simple case-insensitive substring matching on the STT output.
/// When the wake word is found, [`WakeWordDetector::extract_command`] returns
/// the portion of the transcript that follows it, trimmed of whitespace.
pub struct WakeWordDetector {
    /// The wake word to listen for, stored in lowercase for comparison.
    wake_word_lower: String,
}

#[allow(dead_code)]
impl WakeWordDetector {
    /// Create a detector for the given keyword (comparison is case-insensitive).
    pub fn new(wake_word: &str) -> Self {
        Self {
            wake_word_lower: wake_word.to_lowercase(),
        }
    }

    /// Create a detector using the default wake word ("synapse").
    pub fn default_keyword() -> Self {
        Self::new("synapse")
    }

    /// Check whether the transcript contains the wake word.
    pub fn is_triggered(&self, transcript: &str) -> bool {
        transcript.to_lowercase().contains(&self.wake_word_lower)
    }

    /// Extract the command text that follows the wake word in the transcript.
    ///
    /// Returns `None` if the wake word is not found.
    /// Returns `Some("")` if nothing follows the wake word.
    pub fn extract_command<'a>(&self, transcript: &'a str) -> Option<&'a str> {
        let lower = transcript.to_lowercase();
        let pos = lower.find(&self.wake_word_lower)?;
        let after = &transcript[pos + self.wake_word_lower.len()..];
        // Strip a leading comma/space that often appears after the wake word.
        Some(after.trim_start_matches(|c: char| c == ',' || c.is_whitespace()))
    }
}

// ---------------------------------------------------------------------------
// Audio I/O — gated on the `voice` feature (which pulls in `cpal`)
// ---------------------------------------------------------------------------

/// Record audio from the default input device until silence is detected.
///
/// Returns raw PCM i16 little-endian bytes at the device's native sample rate.
///
/// # Parameters
/// - `silence_threshold`: RMS amplitude [0.0, 1.0] below which a frame is
///   considered silent (default 0.02).
/// - `silence_duration_ms`: milliseconds of consecutive silence that end the
///   recording (default 1500).
///
/// On any initialisation error (no device, unsupported format, etc.) this
/// returns `Err` and the caller should fall back to text input.
#[cfg(feature = "voice")]
pub fn record_audio(
    silence_threshold: f32,
    silence_duration_ms: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{Arc as StdArc, Mutex};
    use std::time::{Duration, Instant};

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("no default audio input device")?;

    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    // Number of samples per frame for silence detection (30 ms frame).
    let frame_samples = (sample_rate as usize * channels * 30) / 1000;

    let pcm_buf: StdArc<Mutex<Vec<i16>>> = StdArc::new(Mutex::new(Vec::new()));
    let pcm_buf_clone = StdArc::clone(&pcm_buf);

    let err_fn = |e| eprintln!("  {} audio stream error: {}", "voice:".yellow(), e);

    // Build a stream that converts whatever sample format the device uses to i16.
    let stream = match config.sample_format() {
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let mut buf = pcm_buf_clone.lock().unwrap();
                buf.extend_from_slice(data);
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| {
                let mut buf = pcm_buf_clone.lock().unwrap();
                for &s in data {
                    buf.push(s.wrapping_sub(0x8000) as i16);
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut buf = pcm_buf_clone.lock().unwrap();
                for &s in data {
                    let clamped = s.clamp(-1.0, 1.0);
                    buf.push((clamped * i16::MAX as f32) as i16);
                }
            },
            err_fn,
            None,
        )?,
        fmt => {
            return Err(format!("unsupported sample format: {:?}", fmt).into());
        }
    };

    stream.play()?;

    let silence_duration = Duration::from_millis(silence_duration_ms);
    let mut last_speech = Instant::now();
    let mut consumed = 0usize; // samples already analysed

    loop {
        std::thread::sleep(Duration::from_millis(30));

        let current_len = {
            let buf = pcm_buf.lock().unwrap();
            buf.len()
        };

        // Analyse newly arrived frames.
        while consumed + frame_samples <= current_len {
            let rms = {
                let buf = pcm_buf.lock().unwrap();
                let frame = &buf[consumed..consumed + frame_samples];
                let sum: f64 = frame.iter().map(|&s| (s as f64).powi(2)).sum();
                ((sum / frame_samples as f64).sqrt() / 32768.0) as f32
            };
            if rms >= silence_threshold {
                last_speech = Instant::now();
            }
            consumed += frame_samples;
        }

        if last_speech.elapsed() >= silence_duration && consumed > 0 {
            break;
        }
    }

    drop(stream);

    let samples = pcm_buf.lock().unwrap().clone();
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    Ok(bytes)
}

/// Play raw PCM i16 little-endian audio through the default output device.
///
/// On any initialisation or playback error, a warning is printed and the
/// function returns `Ok(())` — TTS audio is best-effort.
#[cfg(feature = "voice")]
pub fn play_audio(pcm_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{Arc as StdArc, Mutex};

    if pcm_bytes.is_empty() {
        return Ok(());
    }

    // Parse bytes into i16 samples.
    let samples: Vec<i16> = pcm_bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no default audio output device")?;

    let config = device.default_output_config()?;
    let channels = config.channels() as usize;

    let cursor = StdArc::new(Mutex::new(0usize));
    let cursor_clone = StdArc::clone(&cursor);
    let samples = StdArc::new(samples);
    let samples_clone = StdArc::clone(&samples);
    let done_flag = StdArc::new(Mutex::new(false));
    let done_flag_clone = StdArc::clone(&done_flag);

    let err_fn = |e| eprintln!("  {} playback error: {}", "voice:".yellow(), e);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _| {
                let mut cur = cursor_clone.lock().unwrap();
                let src = &*samples_clone;
                for (i, frame) in output.chunks_mut(channels).enumerate() {
                    let sample_idx = *cur + i;
                    let sample = if sample_idx < src.len() {
                        src[sample_idx] as f32 / i16::MAX as f32
                    } else {
                        0.0
                    };
                    for ch in frame.iter_mut() {
                        *ch = sample;
                    }
                }
                let frames = output.len() / channels;
                *cur += frames;
                if *cur >= samples_clone.len() {
                    *done_flag_clone.lock().unwrap() = true;
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [i16], _| {
                let mut cur = cursor_clone.lock().unwrap();
                let src = &*samples_clone;
                for (i, frame) in output.chunks_mut(channels).enumerate() {
                    let sample = if *cur + i < src.len() {
                        src[*cur + i]
                    } else {
                        0
                    };
                    for ch in frame.iter_mut() {
                        *ch = sample;
                    }
                }
                let frames = output.len() / channels;
                *cur += frames;
                if *cur >= samples_clone.len() {
                    *done_flag_clone.lock().unwrap() = true;
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [u16], _| {
                let mut cur = cursor_clone.lock().unwrap();
                let src = &*samples_clone;
                for (i, frame) in output.chunks_mut(channels).enumerate() {
                    let sample = if *cur + i < src.len() {
                        (src[*cur + i] as i32 + 0x8000) as u16
                    } else {
                        0x8000
                    };
                    for ch in frame.iter_mut() {
                        *ch = sample;
                    }
                }
                let frames = output.len() / channels;
                *cur += frames;
                if *cur >= samples_clone.len() {
                    *done_flag_clone.lock().unwrap() = true;
                }
            },
            err_fn,
            None,
        )?,
        fmt => {
            return Err(format!("unsupported output sample format: {:?}", fmt).into());
        }
    };

    stream.play()?;

    // Poll until playback finishes.
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if *done_flag.lock().unwrap() {
            break;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Voice loop
// ---------------------------------------------------------------------------

/// Core voice processing loop.
///
/// Continuously:
/// 1. Records audio from the microphone (or reads a line of text in fallback
///    mode).
/// 2. Sends the audio to the STT provider for transcription.
/// 3. Checks the transcript for the wake word.
/// 4. If the wake word is found, sends the command to the chat model.
/// 5. Plays back the TTS-synthesised response (or prints it in fallback mode).
///
/// The loop exits when the user types "quit"/"exit" (text mode) or when
/// `Ctrl-C` is received.
pub async fn run_voice_loop(
    model: Arc<dyn ChatModel>,
    voice_config: Option<&VoiceConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wake_word = voice_config
        .and_then(|vc| vc.wake_word.as_deref())
        .unwrap_or("synapse");
    let _silence_threshold = voice_config
        .and_then(|vc| vc.silence_threshold)
        .unwrap_or(0.02);
    let _silence_duration_ms = voice_config
        .and_then(|vc| vc.silence_duration_ms)
        .unwrap_or(1500);

    let detector = WakeWordDetector::new(wake_word);

    // TTS/STT setup (voice feature only)
    #[cfg(feature = "voice")]
    let voice_provider: Option<Arc<synaptic_voice::openai::OpenAiVoice>> = {
        let api_key_env = voice_config
            .and_then(|vc| vc.api_key_env.as_deref())
            .unwrap_or("OPENAI_API_KEY");
        match synaptic_voice::openai::OpenAiVoice::new(api_key_env) {
            Ok(provider) => {
                eprintln!(
                    "  {} OpenAI TTS/STT enabled (wake word: \"{}\")",
                    "voice:".green().bold(),
                    wake_word.cyan()
                );
                Some(Arc::new(provider))
            }
            Err(e) => {
                eprintln!(
                    "  {} STT/TTS unavailable ({}), using text-only mode",
                    "voice:".yellow().bold(),
                    e
                );
                None
            }
        }
    };
    #[cfg(not(feature = "voice"))]
    let _voice_provider: Option<()> = {
        let _ = voice_config;
        None
    };

    // Detect whether cpal audio I/O is available.
    #[cfg(feature = "voice")]
    let audio_available: bool = {
        use cpal::traits::HostTrait as _;
        let host = cpal::default_host();
        let has_in = host.default_input_device().is_some();
        let has_out = host.default_output_device().is_some();
        if !has_in || !has_out {
            eprintln!(
                "  {} no audio device found (in={}, out={}), using text input",
                "voice:".yellow(),
                has_in,
                has_out
            );
        }
        has_in && has_out
    };
    #[cfg(not(feature = "voice"))]
    let audio_available: bool = false;

    let voice_name = voice_config
        .and_then(|vc| vc.voice.as_deref())
        .unwrap_or("alloy");

    eprintln!("Type 'quit' to exit.\n");

    let mut messages = vec![Message::system(
        "You are Synapse in voice mode. Keep responses short and conversational.",
    )];

    let stdin = std::io::stdin();
    let mut text_input = String::new();

    loop {
        // --- Step 1: Obtain user input (audio or text) ----------------------
        let transcript = if audio_available {
            #[cfg(feature = "voice")]
            {
                eprint!("  {} listening... ", "voice:".dimmed());
                match record_audio(_silence_threshold, _silence_duration_ms) {
                    Ok(pcm_bytes) => {
                        eprintln!("({} bytes recorded)", pcm_bytes.len());
                        // Step 2: Transcribe
                        if let Some(ref provider) = voice_provider {
                            use synaptic_voice::SttProvider as _;
                            let opts = synaptic_voice::SttOptions {
                                format: synaptic_voice::AudioFormat::Pcm,
                                ..Default::default()
                            };
                            match provider.transcribe(&pcm_bytes, &opts).await {
                                Ok(t) => {
                                    eprintln!("  {} \"{}\"", "stt:".dimmed(), t.text.trim());
                                    t.text
                                }
                                Err(e) => {
                                    eprintln!("  {} STT error: {}", "warning:".yellow(), e);
                                    continue;
                                }
                            }
                        } else {
                            // No STT provider — fall through to text input.
                            eprint!("[you] ");
                            text_input.clear();
                            stdin.read_line(&mut text_input)?;
                            text_input.trim().to_string()
                        }
                    }
                    Err(e) => {
                        eprintln!("  {} record_audio failed: {}", "warning:".yellow(), e);
                        eprint!("[you] ");
                        text_input.clear();
                        stdin.read_line(&mut text_input)?;
                        text_input.trim().to_string()
                    }
                }
            }
            #[cfg(not(feature = "voice"))]
            {
                eprint!("[you] ");
                text_input.clear();
                stdin.read_line(&mut text_input)?;
                text_input.trim().to_string()
            }
        } else {
            eprint!("[you] ");
            text_input.clear();
            stdin.read_line(&mut text_input)?;
            text_input.trim().to_string()
        };

        if transcript.is_empty() {
            continue;
        }
        if transcript == "quit" || transcript == "exit" {
            break;
        }

        // --- Step 3: Wake word detection ------------------------------------
        let command = if audio_available {
            // In audio mode, require the wake word.
            match detector.extract_command(&transcript) {
                Some(cmd) if !cmd.is_empty() => cmd.to_string(),
                Some(_) => {
                    // Wake word heard but no command followed — keep listening.
                    eprintln!(
                        "  {} wake word detected, waiting for command...",
                        "voice:".dimmed()
                    );
                    continue;
                }
                None => {
                    // No wake word — ignore the audio.
                    eprintln!(
                        "  {} (no wake word \"{}\" detected, ignoring)",
                        "voice:".dimmed(),
                        wake_word
                    );
                    continue;
                }
            }
        } else {
            // Text mode: send the whole input directly, no wake word required.
            transcript.clone()
        };

        // --- Step 4: Chat model call ----------------------------------------
        messages.push(Message::human(&command));
        let request = ChatRequest::new(messages.clone());

        match model.chat(request).await {
            Ok(response) => {
                let reply = response.message.content();
                println!("[synapse] {}", reply);
                messages.push(Message::ai(reply));

                // --- Step 5: TTS + playback -----------------------------------
                #[cfg(feature = "voice")]
                if let Some(ref provider) = voice_provider {
                    use synaptic_voice::TtsProvider as _;
                    let tts_opts = synaptic_voice::TtsOptions {
                        voice: voice_name.to_string(),
                        format: synaptic_voice::AudioFormat::Pcm,
                        ..Default::default()
                    };
                    match provider.synthesize(reply, &tts_opts).await {
                        Ok(audio_bytes) => {
                            eprintln!(
                                "  {} {} bytes synthesised",
                                "tts:".dimmed(),
                                audio_bytes.len()
                            );
                            if audio_available {
                                if let Err(e) = play_audio(&audio_bytes) {
                                    eprintln!("  {} playback failed: {}", "warning:".yellow(), e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("  {} TTS failed: {}", "warning:".yellow(), e);
                        }
                    }
                }
                let _ = voice_name; // suppress unused warning in non-voice build
            }
            Err(e) => {
                eprintln!("[error] {}", e);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point (backward-compatible)
// ---------------------------------------------------------------------------

/// Run voice mode (interactive loop).
///
/// With the `voice` feature enabled and a valid API key, responses are
/// synthesised via OpenAI TTS and audio is captured from the microphone.
/// Without the feature or API key, falls back to text-only mode.
///
/// This function delegates to [`run_voice_loop`] and exists for backward
/// compatibility with callers that used the original `run_voice_mode` API.
pub async fn run_voice_mode(
    model: Arc<dyn ChatModel>,
    voice_config: Option<&VoiceConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    let voice_name = voice_config
        .and_then(|vc| vc.voice.as_deref())
        .unwrap_or("alloy");

    // Announce the mode.
    #[cfg(feature = "voice")]
    eprintln!(
        "{} Voice mode starting (TTS voice: {})...",
        "voice:".green().bold(),
        voice_name.cyan()
    );
    #[cfg(not(feature = "voice"))]
    {
        let _ = voice_name;
        eprintln!("Voice mode starting (text-based fallback)...");
    }

    run_voice_loop(model, voice_config).await?;
    eprintln!("Voice mode ended.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_word_detector_triggered() {
        let det = WakeWordDetector::new("synapse");
        assert!(det.is_triggered("Hey Synapse, what time is it?"));
        assert!(det.is_triggered("SYNAPSE help me"));
        assert!(!det.is_triggered("hello there"));
    }

    #[test]
    fn wake_word_extract_command() {
        let det = WakeWordDetector::new("synapse");
        assert_eq!(
            det.extract_command("synapse, what is the weather?"),
            Some("what is the weather?")
        );
        assert_eq!(
            det.extract_command("Hey synapse   play music"),
            Some("play music")
        );
        assert_eq!(det.extract_command("synapse"), Some(""));
        assert_eq!(det.extract_command("nothing here"), None);
    }

    #[test]
    fn wake_word_case_insensitive() {
        let det = WakeWordDetector::new("synapse");
        assert_eq!(
            det.extract_command("SYNAPSE tell me a joke"),
            Some("tell me a joke")
        );
    }

    #[test]
    fn custom_wake_word() {
        let det = WakeWordDetector::new("jarvis");
        assert!(det.is_triggered("Jarvis, open the pod bay doors"));
        assert!(!det.is_triggered("synapse, open the pod bay doors"));
    }
}
