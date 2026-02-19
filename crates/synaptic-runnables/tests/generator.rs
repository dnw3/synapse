use futures::StreamExt;
use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_runnables::{Runnable, RunnableGenerator};

fn default_config() -> RunnableConfig {
    RunnableConfig::default()
}

#[tokio::test]
async fn generator_invoke_collects_stream() {
    let gen = RunnableGenerator::new(|input: String| {
        async_stream::stream! {
            for ch in input.chars() {
                yield Ok(ch.to_string());
            }
        }
    });

    let result = gen.invoke("abc".into(), &default_config()).await.unwrap();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn generator_stream_yields_individually() {
    let gen = RunnableGenerator::new(|input: i32| {
        async_stream::stream! {
            for i in 0..input {
                yield Ok(i);
            }
        }
    });

    let config = default_config();
    let stream = gen.stream(3, &config);
    let items: Vec<_> = stream.collect().await;
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].as_ref().unwrap(), &vec![0]);
    assert_eq!(items[1].as_ref().unwrap(), &vec![1]);
    assert_eq!(items[2].as_ref().unwrap(), &vec![2]);
}

#[tokio::test]
async fn generator_empty_stream() {
    let gen = RunnableGenerator::new(|_input: String| {
        async_stream::stream! {
            // yield nothing — but need the type annotation
            if false {
                yield Ok::<String, SynapticError>("".into());
            }
        }
    });

    let result = gen
        .invoke("empty".into(), &default_config())
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn generator_error_in_stream() {
    let gen = RunnableGenerator::new(|_input: String| {
        async_stream::stream! {
            yield Ok("first".to_string());
            yield Err(SynapticError::Model("stream error".into()));
            yield Ok("never reached".to_string());
        }
    });

    let err = gen
        .invoke("test".into(), &default_config())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("stream error"));
}

#[tokio::test]
async fn generator_single_item() {
    let gen = RunnableGenerator::new(|input: i32| {
        async_stream::stream! {
            yield Ok(input * 10);
        }
    });

    let result = gen.invoke(5, &default_config()).await.unwrap();
    assert_eq!(result, vec![50]);
}

#[tokio::test]
async fn generator_stream_error_propagation() {
    let gen = RunnableGenerator::new(|_input: String| {
        async_stream::stream! {
            yield Err::<String, _>(SynapticError::Parsing("bad".into()));
        }
    });

    let config = default_config();
    let stream = gen.stream("x".into(), &config);
    let items: Vec<_> = stream.collect().await;
    assert_eq!(items.len(), 1);
    assert!(items[0].is_err());
}
