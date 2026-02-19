use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_runnables::{Runnable, RunnableLambda, RunnableRetry, RetryPolicy};

fn default_config() -> RunnableConfig {
    RunnableConfig::default()
}

#[tokio::test]
async fn retry_succeeds_immediately() {
    let inner = RunnableLambda::new(|x: i32| async move { Ok(x * 2) });
    let retry = RunnableRetry::new(inner.boxed(), RetryPolicy::default());
    let result = retry.invoke(5, &default_config()).await.unwrap();
    assert_eq!(result, 10);
}

#[tokio::test]
async fn retry_after_failures() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let inner = RunnableLambda::new(move |x: i32| {
        let c = counter_clone.clone();
        async move {
            let attempt = c.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                Err(SynapticError::Model("transient failure".into()))
            } else {
                Ok(x * 2)
            }
        }
    });

    let policy = RetryPolicy::default()
        .with_max_attempts(3)
        .with_base_delay(Duration::from_millis(1));
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let result = retry.invoke(5, &default_config()).await.unwrap();
    assert_eq!(result, 10);
    assert_eq!(counter.load(Ordering::SeqCst), 3); // 2 failures + 1 success
}

#[tokio::test]
async fn retry_exhausts_and_returns_last_error() {
    let inner = RunnableLambda::new(|_x: i32| async move {
        Err::<i32, _>(SynapticError::Model("always fails".into()))
    });

    let policy = RetryPolicy::default()
        .with_max_attempts(3)
        .with_base_delay(Duration::from_millis(1));
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let err = retry.invoke(1, &default_config()).await.unwrap_err();
    assert!(err.to_string().contains("always fails"));
}

#[tokio::test]
async fn retry_respects_retry_on_predicate() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let inner = RunnableLambda::new(move |_x: i32| {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err::<i32, _>(SynapticError::RateLimit("too fast".into()))
        }
    });

    let policy = RetryPolicy::default()
        .with_max_attempts(3)
        .with_base_delay(Duration::from_millis(1))
        .with_retry_on(|e| matches!(e, SynapticError::RateLimit(_)));
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let err = retry.invoke(1, &default_config()).await.unwrap_err();
    assert!(err.to_string().contains("too fast"));
    assert_eq!(counter.load(Ordering::SeqCst), 3); // retried 3 times because it matches
}

#[tokio::test]
async fn retry_not_retried_when_predicate_false() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let inner = RunnableLambda::new(move |_x: i32| {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err::<i32, _>(SynapticError::Validation("bad input".into()))
        }
    });

    // Only retry on RateLimit errors, not Validation
    let policy = RetryPolicy::default()
        .with_max_attempts(3)
        .with_base_delay(Duration::from_millis(1))
        .with_retry_on(|e| matches!(e, SynapticError::RateLimit(_)));
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let err = retry.invoke(1, &default_config()).await.unwrap_err();
    assert!(err.to_string().contains("bad input"));
    assert_eq!(counter.load(Ordering::SeqCst), 1); // Only 1 attempt, no retry
}

#[tokio::test]
async fn retry_input_cloned_for_each_attempt() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let inner = RunnableLambda::new(move |s: String| {
        let c = counter_clone.clone();
        async move {
            let attempt = c.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                Err(SynapticError::Model("fail first".into()))
            } else {
                Ok(format!("got: {s}"))
            }
        }
    });

    let policy = RetryPolicy::default()
        .with_max_attempts(2)
        .with_base_delay(Duration::from_millis(1));
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let result = retry
        .invoke("hello".to_string(), &default_config())
        .await
        .unwrap();
    assert_eq!(result, "got: hello"); // input was cloned correctly
}

#[tokio::test]
async fn retry_one_attempt_no_retry() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let inner = RunnableLambda::new(move |_x: i32| {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err::<i32, _>(SynapticError::Model("fail".into()))
        }
    });

    let policy = RetryPolicy::default()
        .with_max_attempts(1)
        .with_base_delay(Duration::from_millis(1));
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let err = retry.invoke(1, &default_config()).await.unwrap_err();
    assert!(err.to_string().contains("fail"));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retry_zero_attempts() {
    let inner = RunnableLambda::new(|_x: i32| async move { Ok(42) });

    let policy = RetryPolicy::default().with_max_attempts(0);
    let retry = RunnableRetry::new(inner.boxed(), policy);

    let err = retry.invoke(1, &default_config()).await.unwrap_err();
    assert!(err.to_string().contains("max_attempts must be >= 1"));
}
