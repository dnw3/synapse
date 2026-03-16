use serde::{Deserialize, Serialize};

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
}
