use hmac::{Hmac, Mac};
use sha2::Sha256;
use synaptic_core::SynapticError;

/// Verify Lark's event push signature.
///
/// Lark signs: HMAC-SHA256(key = timestamp+nonce+verification_token, msg = body)
pub fn verify_lark_signature(
    timestamp: &str,
    nonce: &str,
    verification_token: &str,
    body: &[u8],
    expected_hex: &str,
) -> Result<(), SynapticError> {
    let key = format!("{timestamp}{nonce}{verification_token}");
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
        .map_err(|e| SynapticError::Config(format!("HMAC init: {e}")))?;
    mac.update(body);
    let result = mac.finalize();
    let computed = hex_encode(result.into_bytes().as_slice());
    if computed != expected_hex {
        Err(SynapticError::Config(
            "Lark event signature mismatch".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
