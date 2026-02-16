use synapse_embeddings::{Embeddings, FakeEmbeddings};

#[tokio::test]
async fn fake_embeddings_returns_correct_dimensions() {
    let embeddings = FakeEmbeddings::new(8);
    let result = embeddings.embed_query("hello").await.unwrap();
    assert_eq!(result.len(), 8);
}

#[tokio::test]
async fn fake_embeddings_deterministic() {
    let embeddings = FakeEmbeddings::default();
    let v1 = embeddings.embed_query("test").await.unwrap();
    let v2 = embeddings.embed_query("test").await.unwrap();
    assert_eq!(v1, v2);
}

#[tokio::test]
async fn fake_embeddings_batch() {
    let embeddings = FakeEmbeddings::new(4);
    let results = embeddings
        .embed_documents(&["hello", "world"])
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].len(), 4);
    assert_eq!(results[1].len(), 4);
}

#[tokio::test]
async fn fake_embeddings_normalized() {
    let embeddings = FakeEmbeddings::new(4);
    let vec = embeddings.embed_query("hello world").await.unwrap();
    let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 0.001,
        "vector should be unit length"
    );
}
