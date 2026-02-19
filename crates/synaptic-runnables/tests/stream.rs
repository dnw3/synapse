use futures::StreamExt;
use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_runnables::{
    BoxRunnable, Runnable, RunnableLambda, RunnableOutputStream, RunnableWithFallbacks,
};

/// A runnable that streams multiple items instead of just one.
struct MultiChunkRunnable;

#[async_trait::async_trait]
impl Runnable<String, String> for MultiChunkRunnable {
    async fn invoke(
        &self,
        input: String,
        _config: &RunnableConfig,
    ) -> Result<String, SynapticError> {
        Ok(input)
    }

    fn stream<'a>(
        &'a self,
        input: String,
        _config: &'a RunnableConfig,
    ) -> RunnableOutputStream<'a, String>
    where
        String: 'a,
    {
        Box::pin(async_stream::stream! {
            for ch in input.chars() {
                yield Ok(ch.to_string());
            }
        })
    }
}

#[tokio::test]
async fn stream_single_runnable() {
    let r = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let config = RunnableConfig::default();
    let items: Vec<_> = r
        .stream("hello".to_string(), &config)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(items, vec!["HELLO"]);
}

#[tokio::test]
async fn stream_multi_chunk_runnable() {
    let r = MultiChunkRunnable;
    let config = RunnableConfig::default();
    let items: Vec<_> = r
        .stream("abc".to_string(), &config)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(items, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn stream_sequence_delegates_to_last() {
    let upper = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let multi = MultiChunkRunnable;
    let chain = upper.boxed() | multi.boxed();
    let config = RunnableConfig::default();
    let items: Vec<_> = chain
        .stream("hi".to_string(), &config)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    // first step uppercases "hi" -> "HI", then MultiChunkRunnable streams char-by-char
    assert_eq!(items, vec!["H", "I"]);
}

#[tokio::test]
async fn stream_with_error() {
    let failing = RunnableLambda::new(|_s: String| async move {
        Err::<String, _>(SynapticError::Validation("bad".to_string()))
    });
    let config = RunnableConfig::default();
    let items: Vec<_> = failing
        .stream("hello".to_string(), &config)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(items.len(), 1);
    assert!(items[0].is_err());
}

#[tokio::test]
async fn stream_boxed_delegates() {
    let r = BoxRunnable::new(MultiChunkRunnable);
    let config = RunnableConfig::default();
    let items: Vec<_> = r
        .stream("xy".to_string(), &config)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(items, vec!["x", "y"]);
}

#[tokio::test]
async fn stream_fallback_uses_fallback_on_error() {
    let failing = RunnableLambda::new(|_s: String| async move {
        Err::<String, _>(SynapticError::Validation("primary failed".to_string()))
    });
    let ok = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let with_fallback = RunnableWithFallbacks::new(failing.boxed(), vec![ok.boxed()]);
    let config = RunnableConfig::default();
    let items: Vec<_> = with_fallback
        .stream("hello".to_string(), &config)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(items, vec!["HELLO"]);
}
