//! Bootstrap tokens for device pairing.
//!
//! A bootstrap token is a short-lived credential (10 minutes) included in QR codes
//! to allow a new device to initiate a pairing request over WebSocket.

use std::collections::HashMap;
use std::path::PathBuf;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::gateway::presence::now_ms;

/// Bootstrap token time-to-live: 10 minutes.
const BOOTSTRAP_TTL_MS: u64 = 10 * 60 * 1000;

/// Size of the random token in bytes (32 bytes → 43 base64url chars).
const TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapTokenRecord {
    pub token: String,
    pub issued_at_ms: u64,
    pub device_id: Option<String>,
    pub public_key: Option<String>,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
    pub last_used_at_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct BootstrapData {
    tokens: HashMap<String, BootstrapTokenRecord>,
}

pub struct BootstrapStore {
    data: BootstrapData,
    path: PathBuf,
}

impl BootstrapStore {
    pub fn new() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".synapse")
            .join("pairing");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bootstrap.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { data, path }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    fn prune_expired(&mut self) {
        let now = now_ms();
        self.data
            .tokens
            .retain(|_, r| now.saturating_sub(r.issued_at_ms) < BOOTSTRAP_TTL_MS);
    }

    /// Issue a new bootstrap token. Returns the token string.
    pub fn issue(&mut self) -> String {
        self.prune_expired();
        let mut bytes = [0u8; TOKEN_BYTES];
        rand::rng().fill(&mut bytes);
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let record = BootstrapTokenRecord {
            token: token.clone(),
            issued_at_ms: now_ms(),
            device_id: None,
            public_key: None,
            roles: Vec::new(),
            scopes: Vec::new(),
            last_used_at_ms: None,
        };
        self.data.tokens.insert(token.clone(), record);
        self.save();
        token
    }

    /// Verify a bootstrap token. On first use, binds to the device identity.
    /// Returns true if valid.
    pub fn verify(
        &mut self,
        token: &str,
        device_id: &str,
        public_key: &str,
        role: &str,
        scopes: &[String],
    ) -> bool {
        self.prune_expired();
        let Some(record) = self.data.tokens.get_mut(token) else {
            return false;
        };

        // Check if already bound to a different device
        if let Some(ref bound_id) = record.device_id {
            if bound_id != device_id {
                return false;
            }
        }

        // Bind to device on first use
        if record.device_id.is_none() {
            record.device_id = Some(device_id.to_string());
            record.public_key = Some(public_key.to_string());
        }

        // Accumulate roles and scopes
        if !record.roles.contains(&role.to_string()) {
            record.roles.push(role.to_string());
        }
        for scope in scopes {
            if !record.scopes.contains(scope) {
                record.scopes.push(scope.clone());
            }
        }

        record.last_used_at_ms = Some(now_ms());
        self.save();
        true
    }

    /// Consume (remove) a used bootstrap token after pairing is approved.
    #[allow(dead_code)]
    pub fn consume(&mut self, token: &str) -> Option<BootstrapTokenRecord> {
        let record = self.data.tokens.remove(token);
        if record.is_some() {
            self.save();
        }
        record
    }

    /// List all active (non-expired) bootstrap tokens.
    pub fn list(&mut self) -> Vec<BootstrapTokenRecord> {
        self.prune_expired();
        self.data.tokens.values().cloned().collect()
    }
}

impl Default for BootstrapStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Setup Code
// ---------------------------------------------------------------------------

/// Encode a setup code payload as URL-safe base64.
/// Payload: `{"url": "<gateway_url>", "bootstrapToken": "<token>"}`
pub fn encode_setup_code(gateway_url: &str, bootstrap_token: &str) -> String {
    let payload = serde_json::json!({
        "url": gateway_url,
        "bootstrapToken": bootstrap_token,
    });
    let json = payload.to_string();
    URL_SAFE_NO_PAD.encode(json.as_bytes())
}

/// Decode a setup code from URL-safe base64.
pub fn decode_setup_code(code: &str) -> Option<(String, String)> {
    let bytes = URL_SAFE_NO_PAD.decode(code).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let url = json.get("url")?.as_str()?.to_string();
    let token = json.get("bootstrapToken")?.as_str()?.to_string();
    Some((url, token))
}

// ---------------------------------------------------------------------------
// QR Code Generation
// ---------------------------------------------------------------------------

/// Generate a QR code as unicode text (for terminal display).
#[allow(dead_code)]
pub fn generate_qr_text(content: &str) -> Option<String> {
    let code = qrcode::QrCode::new(content.as_bytes()).ok()?;
    let text = code
        .render::<char>()
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .build();
    Some(text)
}

/// Generate a QR code as SVG string (for web display).
pub fn generate_qr_svg(content: &str) -> Option<String> {
    let code = qrcode::QrCode::new(content.as_bytes()).ok()?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .quiet_zone(true)
        .min_dimensions(200, 200)
        .build();
    Some(svg)
}

// ---------------------------------------------------------------------------
// Pairing Token Generation (for device auth tokens)
// ---------------------------------------------------------------------------

/// Generate a random 32-byte pairing token as base64url.
pub fn generate_pairing_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::rng().fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Constant-time token comparison.
#[allow(dead_code)]
pub fn verify_token(provided: &str, expected: &str) -> bool {
    if provided.len() != expected.len() {
        return false;
    }
    let a = provided.as_bytes();
    let b = expected.as_bytes();
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
