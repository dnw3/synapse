use synaptic_lark::events::verify::verify_lark_signature;

#[test]
fn valid_signature_passes() {
    // Lark signature = HMAC-SHA256(key = timestamp+nonce+verification_token, msg = body)
    let timestamp = "1609459200";
    let nonce = "test-nonce";
    let token = "my-verification-token";
    let body = r#"{"type":"url_verification","challenge":"test"}"#;

    let sig = compute_expected(timestamp, nonce, token, body);
    assert!(verify_lark_signature(timestamp, nonce, token, body.as_bytes(), &sig).is_ok());
}

#[test]
fn tampered_body_fails() {
    let sig = compute_expected("ts", "n", "tok", "body");
    let result = verify_lark_signature("ts", "n", "tok", b"tampered", &sig);
    assert!(result.is_err());
}

fn compute_expected(ts: &str, nonce: &str, token: &str, body: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    // Lark's formula: HMAC-SHA256 key=ts+nonce+token, msg=body
    let key = format!("{ts}{nonce}{token}");
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

use std::sync::{Arc, Mutex};
use synaptic_lark::{events::listener::LarkEventListener, LarkConfig};

#[tokio::test]
async fn dispatch_url_verification() {
    let listener = LarkEventListener::new(LarkConfig::new("a", "b"));
    let body = br#"{"type":"url_verification","challenge":"abc123"}"#;
    let result = listener.dispatch(body, None, None, None).await.unwrap();
    assert_eq!(result["challenge"], "abc123");
}

#[tokio::test]
async fn dispatch_calls_registered_handler() {
    let called = Arc::new(Mutex::new(false));
    let called_clone = called.clone();

    let listener = LarkEventListener::new(LarkConfig::new("a", "b")).on(
        "im.message.receive_v1",
        move |_event| {
            let c = called_clone.clone();
            async move {
                *c.lock().unwrap() = true;
                Ok(())
            }
        },
    );

    let body = br#"{
        "schema": "2.0",
        "header": { "event_type": "im.message.receive_v1" },
        "event": {}
    }"#;
    listener.dispatch(body, None, None, None).await.unwrap();
    assert!(*called.lock().unwrap());
}

#[tokio::test]
async fn dispatch_unknown_event_type_is_noop() {
    let listener = LarkEventListener::new(LarkConfig::new("a", "b"));
    let body = br#"{"schema":"2.0","header":{"event_type":"unknown.event"},"event":{}}"#;
    let result = listener.dispatch(body, None, None, None).await;
    assert!(result.is_ok()); // no handler, but no error either
}
