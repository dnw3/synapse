use synaptic_core::{RunnableConfig, SynapticError};
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};

fn default_config() -> RunnableConfig {
    RunnableConfig::default()
}

#[tokio::test]
async fn each_maps_over_vec() {
    let inner = RunnableLambda::new(|x: i32| async move { Ok(x * 2) });
    let each = RunnableEach::new(inner.boxed());

    let result = each
        .invoke(vec![1, 2, 3, 4, 5], &default_config())
        .await
        .unwrap();
    assert_eq!(result, vec![2, 4, 6, 8, 10]);
}

#[tokio::test]
async fn each_empty_input() {
    let inner = RunnableLambda::new(|x: i32| async move { Ok(x) });
    let each = RunnableEach::new(inner.boxed());

    let result = each
        .invoke(vec![], &default_config())
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn each_propagates_first_error() {
    let inner = RunnableLambda::new(|x: i32| async move {
        if x == 3 {
            Err(SynapticError::Validation("bad value 3".into()))
        } else {
            Ok(x * 2)
        }
    });
    let each = RunnableEach::new(inner.boxed());

    let err = each
        .invoke(vec![1, 2, 3, 4], &default_config())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bad value 3"));
}

#[tokio::test]
async fn each_with_string_transform() {
    let inner = RunnableLambda::new(|s: String| async move { Ok(s.to_uppercase()) });
    let each = RunnableEach::new(inner.boxed());

    let result = each
        .invoke(
            vec!["hello".into(), "world".into()],
            &default_config(),
        )
        .await
        .unwrap();
    assert_eq!(result, vec!["HELLO", "WORLD"]);
}

#[tokio::test]
async fn each_single_element() {
    let inner = RunnableLambda::new(|x: i32| async move { Ok(x + 1) });
    let each = RunnableEach::new(inner.boxed());

    let result = each.invoke(vec![42], &default_config()).await.unwrap();
    assert_eq!(result, vec![43]);
}

#[tokio::test]
async fn each_preserves_order() {
    let inner = RunnableLambda::new(|x: i32| async move { Ok(format!("item_{x}")) });
    let each = RunnableEach::new(inner.boxed());

    let result = each
        .invoke(vec![3, 1, 4, 1, 5], &default_config())
        .await
        .unwrap();
    assert_eq!(
        result,
        vec!["item_3", "item_1", "item_4", "item_1", "item_5"]
    );
}
