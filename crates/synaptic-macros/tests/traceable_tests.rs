//! Integration tests for the `#[traceable]` macro.

use synaptic_macros::traceable;

// ---------------------------------------------------------------------------
// Basic async function with tracing
// ---------------------------------------------------------------------------

#[traceable]
async fn process_data(input: String, count: usize) -> String {
    format!("{}: {}", input, count)
}

#[tokio::test]
async fn test_traceable_async_basic() {
    let result = process_data("hello".into(), 42).await;
    assert_eq!(result, "hello: 42");
}

// ---------------------------------------------------------------------------
// Sync function with tracing
// ---------------------------------------------------------------------------

#[traceable]
fn add_numbers(a: i64, b: i64) -> i64 {
    a + b
}

#[test]
fn test_traceable_sync_basic() {
    let result = add_numbers(3, 4);
    assert_eq!(result, 7);
}

// ---------------------------------------------------------------------------
// Custom span name
// ---------------------------------------------------------------------------

#[traceable(name = "custom_span")]
async fn custom_named(value: String) -> String {
    value.to_uppercase()
}

#[tokio::test]
async fn test_traceable_custom_name() {
    let result = custom_named("hello".into()).await;
    assert_eq!(result, "HELLO");
}

// ---------------------------------------------------------------------------
// Skip parameters
// ---------------------------------------------------------------------------

#[traceable(skip = "secret")]
async fn with_secret(query: String, secret: String) -> String {
    format!("query={}, has_secret={}", query, !secret.is_empty())
}

#[tokio::test]
async fn test_traceable_skip_param() {
    let result = with_secret("test".into(), "my_secret".into()).await;
    assert_eq!(result, "query=test, has_secret=true");
}

// ---------------------------------------------------------------------------
// Both name and skip
// ---------------------------------------------------------------------------

#[traceable(name = "auth_check", skip = "token")]
async fn authenticate(user: String, token: String) -> bool {
    !user.is_empty() && !token.is_empty()
}

#[tokio::test]
async fn test_traceable_name_and_skip() {
    let result = authenticate("alice".into(), "abc123".into()).await;
    assert!(result);
}

// ---------------------------------------------------------------------------
// Skip multiple parameters
// ---------------------------------------------------------------------------

#[traceable(skip = "password,token")]
async fn login(username: String, _password: String, _token: String) -> String {
    format!("user={}", username)
}

#[tokio::test]
async fn test_traceable_skip_multiple() {
    let result = login("bob".into(), "pass".into(), "tok".into()).await;
    assert_eq!(result, "user=bob");
}

// ---------------------------------------------------------------------------
// Function with Result return type
// ---------------------------------------------------------------------------

#[traceable]
async fn fallible_op(input: String) -> Result<String, String> {
    if input.is_empty() {
        Err("empty input".into())
    } else {
        Ok(input.to_uppercase())
    }
}

#[tokio::test]
async fn test_traceable_result_ok() {
    let result = fallible_op("hello".into()).await;
    assert_eq!(result.unwrap(), "HELLO");
}

#[tokio::test]
async fn test_traceable_result_err() {
    let result = fallible_op("".into()).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// No-param function
// ---------------------------------------------------------------------------

#[traceable]
async fn no_params() -> String {
    "done".into()
}

#[tokio::test]
async fn test_traceable_no_params() {
    let result = no_params().await;
    assert_eq!(result, "done");
}

// ---------------------------------------------------------------------------
// Verify tracing actually emits spans (integration with subscriber)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_traceable_with_subscriber() {
    // Just verify the function works when tracing subscriber is active
    let _guard = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .try_init();

    let result = process_data("traced".into(), 99).await;
    assert_eq!(result, "traced: 99");
}
