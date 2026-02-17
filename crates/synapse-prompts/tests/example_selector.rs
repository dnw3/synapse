use std::sync::Arc;

use synapse_embeddings::FakeEmbeddings;
use synapse_prompts::{ExampleSelector, FewShotExample, SemanticSimilarityExampleSelector};

#[tokio::test]
async fn select_examples_returns_top_k() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let selector = SemanticSimilarityExampleSelector::new(embeddings, 2);

    selector
        .add_example(FewShotExample {
            input: "What is the capital of France?".to_string(),
            output: "Paris".to_string(),
        })
        .await
        .unwrap();

    selector
        .add_example(FewShotExample {
            input: "What is the capital of Germany?".to_string(),
            output: "Berlin".to_string(),
        })
        .await
        .unwrap();

    selector
        .add_example(FewShotExample {
            input: "How to bake a cake?".to_string(),
            output: "Mix flour, eggs, and sugar.".to_string(),
        })
        .await
        .unwrap();

    let results = selector
        .select_examples("What is the capital of Spain?")
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn select_examples_empty_pool() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let selector = SemanticSimilarityExampleSelector::new(embeddings, 3);

    let results = selector.select_examples("anything").await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn select_examples_fewer_than_k() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let selector = SemanticSimilarityExampleSelector::new(embeddings, 5);

    selector
        .add_example(FewShotExample {
            input: "hello".to_string(),
            output: "hi".to_string(),
        })
        .await
        .unwrap();

    let results = selector.select_examples("hey").await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn add_and_select_preserves_example_content() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let selector = SemanticSimilarityExampleSelector::new(embeddings, 1);

    selector
        .add_example(FewShotExample {
            input: "test input".to_string(),
            output: "test output".to_string(),
        })
        .await
        .unwrap();

    let results = selector.select_examples("test input").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].input, "test input");
    assert_eq!(results[0].output, "test output");
}

#[tokio::test]
async fn most_similar_example_is_returned_first() {
    let embeddings = Arc::new(FakeEmbeddings::new(8));
    let selector = SemanticSimilarityExampleSelector::new(embeddings, 1);

    // Add two very different examples
    selector
        .add_example(FewShotExample {
            input: "cat dog pet animal".to_string(),
            output: "animals".to_string(),
        })
        .await
        .unwrap();

    selector
        .add_example(FewShotExample {
            input: "123 456 789 numbers".to_string(),
            output: "numbers".to_string(),
        })
        .await
        .unwrap();

    // Query something closer to the first example
    let results = selector
        .select_examples("cat dog pet animal")
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    // The identical input should be the most similar
    assert_eq!(results[0].input, "cat dog pet animal");
}
