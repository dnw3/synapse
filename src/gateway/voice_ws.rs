use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TTS configuration
// ---------------------------------------------------------------------------

/// Configuration for Text-to-Speech synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// TTS provider name (e.g. `"openai"`).
    pub provider: String,
    /// Model identifier (e.g. `"tts-1"` or `"tts-1-hd"`).
    pub model: String,
    /// Voice name (e.g. `"alloy"`, `"echo"`, `"fable"`, `"onyx"`, `"nova"`, `"shimmer"`).
    pub voice: String,
    /// API key for the provider. Falls back to the `OPENAI_API_KEY` env var when `None`.
    pub api_key: Option<String>,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "tts-1".to_string(),
            voice: "alloy".to_string(),
            api_key: None,
        }
    }
}

// ---------------------------------------------------------------------------

/// Client → Server voice messages
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum VoiceClientMessage {
    #[serde(rename = "voice_start")]
    VoiceStart { format: String }, // "pcm_16k", "opus", etc.
    #[serde(rename = "voice_chunk")]
    VoiceChunk { data: String }, // base64 encoded audio
    #[serde(rename = "voice_end")]
    VoiceEnd,
}

/// Server → Client voice messages
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum VoiceServerMessage {
    #[serde(rename = "voice_transcript")]
    VoiceTranscript { text: String, partial: bool },
    #[serde(rename = "voice_response_start")]
    VoiceResponseStart,
    #[serde(rename = "voice_response_chunk")]
    VoiceResponseChunk { data: String }, // base64 encoded audio
    #[serde(rename = "voice_response_end")]
    VoiceResponseEnd,
}

/// Voice session state
#[allow(dead_code)]
pub struct VoiceSession {
    format: String,
    is_active: bool,
    buffer: Vec<u8>,
}

impl VoiceSession {
    pub fn new(format: String) -> Self {
        Self {
            format,
            is_active: true,
            buffer: Vec::new(),
        }
    }

    pub fn append_chunk(&mut self, base64_data: &str) -> Result<(), Box<dyn std::error::Error>> {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD.decode(base64_data)?;
        self.buffer.extend_from_slice(&decoded);
        Ok(())
    }

    pub fn take_buffer(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buffer)
    }

    pub fn end(&mut self) {
        self.is_active = false;
    }

    /// Synthesize `text` to Opus-encoded audio bytes via the OpenAI TTS API.
    ///
    /// `api_key` is the OpenAI API key; the caller is responsible for resolving
    /// it from config / environment before calling this method.
    pub async fn synthesize(
        &self,
        text: &str,
        api_key: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        self.synthesize_with_config(text, api_key, &TtsConfig::default())
            .await
    }

    /// Synthesize `text` to audio bytes using the supplied [`TtsConfig`].
    ///
    /// The `response_format` sent to the API is always `"opus"` so the result
    /// can be streamed directly over the voice WebSocket.
    pub async fn synthesize_with_config(
        &self,
        text: &str,
        api_key: &str,
        config: &TtsConfig,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": config.model,
                "input": text,
                "voice": config.voice,
                "response_format": "opus",
            }))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(resp.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_start_deserializes() {
        let json = r#"{"type": "voice_start", "format": "pcm_16k"}"#;
        let msg: VoiceClientMessage = serde_json::from_str(json).unwrap();
        matches!(msg, VoiceClientMessage::VoiceStart { format } if format == "pcm_16k");
    }

    #[test]
    fn voice_chunk_accumulates() {
        let mut session = VoiceSession::new("pcm_16k".into());
        session.append_chunk("AQID").unwrap(); // base64 of [1,2,3]
        assert_eq!(session.buffer.len(), 3);
    }

    #[test]
    fn voice_response_serializes() {
        let msg = VoiceServerMessage::VoiceTranscript {
            text: "hello".into(),
            partial: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("voice_transcript"));
        assert!(json.contains("\"partial\":true"));
    }

    // ------------------------------------------------------------------
    // TtsConfig tests
    // ------------------------------------------------------------------

    #[test]
    fn tts_config_defaults() {
        let cfg = TtsConfig::default();
        assert_eq!(cfg.provider, "openai");
        assert_eq!(cfg.model, "tts-1");
        assert_eq!(cfg.voice, "alloy");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn tts_config_custom() {
        let cfg = TtsConfig {
            provider: "openai".to_string(),
            model: "tts-1-hd".to_string(),
            voice: "nova".to_string(),
            api_key: Some("sk-test-key".to_string()),
        };
        assert_eq!(cfg.model, "tts-1-hd");
        assert_eq!(cfg.voice, "nova");
        assert_eq!(cfg.api_key.as_deref(), Some("sk-test-key"));
    }

    #[test]
    fn tts_config_serializes_round_trip() {
        let cfg = TtsConfig {
            provider: "openai".to_string(),
            model: "tts-1".to_string(),
            voice: "echo".to_string(),
            api_key: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: TtsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.provider, cfg.provider);
        assert_eq!(restored.model, cfg.model);
        assert_eq!(restored.voice, cfg.voice);
        assert!(restored.api_key.is_none());
    }

    // ------------------------------------------------------------------
    // VoiceSession construction tests
    // ------------------------------------------------------------------

    #[test]
    fn voice_session_new_is_active() {
        let session = VoiceSession::new("opus".into());
        assert!(session.is_active);
        assert!(session.buffer.is_empty());
        assert_eq!(session.format, "opus");
    }

    #[test]
    fn voice_session_end_marks_inactive() {
        let mut session = VoiceSession::new("pcm_16k".into());
        assert!(session.is_active);
        session.end();
        assert!(!session.is_active);
    }

    #[test]
    fn voice_session_take_buffer_clears() {
        let mut session = VoiceSession::new("pcm_16k".into());
        session.append_chunk("AQID").unwrap(); // [1,2,3]
        let buf = session.take_buffer();
        assert_eq!(buf, vec![1u8, 2, 3]);
        assert!(session.buffer.is_empty());
    }
}
