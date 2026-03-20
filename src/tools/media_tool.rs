//! Media analysis tools for the Deep Agent.
//!
//! - `AnalyzeImageTool`: encodes an image as a base64 data URL so the vision
//!   model can analyze it in the conversation.
//! - `TranscribeAudioTool`: transcribes audio files using the STT provider
//!   (requires the `voice` feature).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};

// ---------------------------------------------------------------------------
// AnalyzeImageTool
// ---------------------------------------------------------------------------

/// Reads an image file, base64-encodes it, and returns a data-URL plus the
/// user's analysis prompt so that the vision model can inspect the image.
pub struct AnalyzeImageTool {
    work_dir: PathBuf,
}

#[allow(clippy::new_ret_no_self)]
impl AnalyzeImageTool {
    pub fn new(work_dir: &Path) -> Arc<dyn Tool> {
        Arc::new(Self {
            work_dir: work_dir.to_path_buf(),
        })
    }
}

#[async_trait]
impl Tool for AnalyzeImageTool {
    fn name(&self) -> &'static str {
        "analyze_image"
    }

    fn description(&self) -> &'static str {
        "Analyze an image file and describe its contents. The image will be sent to the vision model for analysis."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the image file to analyze (relative to working directory or absolute)."
                },
                "prompt": {
                    "type": "string",
                    "description": "What to look for or describe in the image (default: general description)."
                }
            },
            "required": ["path"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'path' argument".into()))?;

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Describe this image in detail.");

        let full_path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            self.work_dir.join(path_str)
        };

        if !full_path.exists() {
            return Err(SynapticError::Tool(format!(
                "Image file not found: {}",
                path_str
            )));
        }

        tracing::info!(path = %path_str, "image analysis");

        // Check file size (max 10 MB)
        let metadata = std::fs::metadata(&full_path)
            .map_err(|e| SynapticError::Tool(format!("Cannot read file metadata: {}", e)))?;
        if metadata.len() > 10 * 1024 * 1024 {
            return Err(SynapticError::Tool(
                "Image file too large (max 10 MB)".into(),
            ));
        }

        // Read and base64-encode the image in a blocking task
        let path_clone = full_path.clone();
        let (b64, mime) = tokio::task::spawn_blocking(move || {
            let bytes = std::fs::read(&path_clone)
                .map_err(|e| SynapticError::Tool(format!("Cannot read file: {}", e)))?;
            let mime = detect_mime(&path_clone);
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok::<_, SynapticError>((encoded, mime))
        })
        .await
        .map_err(|e| SynapticError::Tool(format!("Image task failed: {}", e)))??;

        let data_url = format!("data:{};base64,{}", mime, b64);

        Ok(json!({
            "image_data_url": data_url,
            "prompt": prompt,
            "path": path_str,
        }))
    }
}

/// Detect MIME type from the file extension.
fn detect_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "heic" | "heif" => "image/heic",
        _ => "image/png",
    }
}

// ---------------------------------------------------------------------------
// TranscribeAudioTool
// ---------------------------------------------------------------------------

/// Transcribes an audio file using the STT provider.
///
/// When built without the `voice` feature the tool still exists but returns an
/// informational error asking the user to enable the feature.
#[allow(dead_code)]
pub struct TranscribeAudioTool {
    work_dir: PathBuf,
    #[cfg(feature = "voice")]
    stt: Arc<dyn synaptic_integrations::voice::SttProvider>,
}

impl TranscribeAudioTool {
    /// Create the tool backed by a real STT provider (voice feature).
    #[cfg(feature = "voice")]
    pub fn new(
        work_dir: &Path,
        stt: Arc<dyn synaptic_integrations::voice::SttProvider>,
    ) -> Arc<dyn Tool> {
        Arc::new(Self {
            work_dir: work_dir.to_path_buf(),
            stt,
        })
    }

    /// Stub constructor when the voice feature is disabled.
    #[allow(dead_code)]
    #[cfg(not(feature = "voice"))]
    pub fn new_stub(work_dir: &Path) -> Arc<dyn Tool> {
        Arc::new(Self {
            work_dir: work_dir.to_path_buf(),
        })
    }
}

#[async_trait]
impl Tool for TranscribeAudioTool {
    fn name(&self) -> &'static str {
        "transcribe_audio"
    }

    fn description(&self) -> &'static str {
        "Transcribe an audio file to text using speech-to-text."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the audio file (relative to working directory or absolute)."
                },
                "language": {
                    "type": "string",
                    "description": "Language hint (ISO 639-1, e.g. \"en\", \"zh\"). Optional."
                }
            },
            "required": ["path"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'path' argument".into()))?;

        let full_path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            self.work_dir.join(path_str)
        };

        if !full_path.exists() {
            return Err(SynapticError::Tool(format!(
                "Audio file not found: {}",
                path_str
            )));
        }

        #[cfg(not(feature = "voice"))]
        {
            let _ = full_path;
            return Err(SynapticError::Tool(
                "Audio transcription requires the 'voice' feature to be enabled. \
                 Rebuild with --features voice."
                    .into(),
            ));
        }

        #[cfg(feature = "voice")]
        {
            let language = args
                .get("language")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Detect audio format from extension
            let format = detect_audio_format(&full_path);

            // Read the audio bytes
            let path_clone = full_path.clone();
            let audio_bytes = tokio::task::spawn_blocking(move || {
                std::fs::read(&path_clone)
                    .map_err(|e| SynapticError::Tool(format!("Cannot read audio file: {}", e)))
            })
            .await
            .map_err(|e| SynapticError::Tool(format!("Audio task failed: {}", e)))??;

            let opts = synaptic_integrations::voice::SttOptions {
                language,
                format,
                prompt: None,
            };

            let result = self.stt.transcribe(&audio_bytes, &opts).await?;

            Ok(json!({
                "path": path_str,
                "text": result.text,
                "language": result.language,
                "duration_secs": result.duration_secs,
            }))
        }
    }
}

/// Detect audio format from file extension.
#[cfg(feature = "voice")]
fn detect_audio_format(path: &Path) -> synaptic_integrations::voice::AudioFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "wav" => synaptic_integrations::voice::AudioFormat::Wav,
        "mp3" => synaptic_integrations::voice::AudioFormat::Mp3,
        "ogg" | "oga" => synaptic_integrations::voice::AudioFormat::Ogg,
        "flac" => synaptic_integrations::voice::AudioFormat::Flac,
        "pcm" | "raw" => synaptic_integrations::voice::AudioFormat::Pcm,
        _ => synaptic_integrations::voice::AudioFormat::Wav,
    }
}
