use synaptic_embeddings::{Embeddings, FakeEmbeddings};

#[tokio::test]
async fn fake_default_dimensions() {
    let emb = FakeEmbeddings::default();
    let vec = emb.embed_query("test").await.unwrap();
    assert_eq!(vec.len(), 4, "default FakeEmbeddings should produce 4-dimensional vectors");
}

#[tokio::test]
async fn fake_custom_dimensions() {
    let emb = FakeEmbeddings::new(16);
    let vec = emb.embed_query("test").await.unwrap();
    assert_eq!(vec.len(), 16);
}

#[tokio::test]
async fn fake_deterministic_same_input() {
    let emb = FakeEmbeddings::default();
    let v1 = emb.embed_query("hello").await.unwrap();
    let v2 = emb.embed_query("hello").await.unwrap();
    assert_eq!(v1, v2, "same input text should always produce the same embedding");
}

#[tokio::test]
async fn fake_different_texts_produce_different_vectors() {
    let emb = FakeEmbeddings::default();
    let v1 = emb.embed_query("hello").await.unwrap();
    let v2 = emb.embed_query("world").await.unwrap();
    assert_ne!(v1, v2, "different texts should produce different embeddings");
}

#[tokio::test]
async fn fake_batch_embed_documents() {
    let emb = FakeEmbeddings::default();
    let vecs = emb.embed_documents(&["a", "b", "c"]).await.unwrap();
    assert_eq!(vecs.len(), 3, "batch should return one vector per input text");
    for v in &vecs {
        assert_eq!(v.len(), 4, "each vector should have default 4 dimensions");
    }
}

#[tokio::test]
async fn fake_embed_query_matches_embed_documents() {
    // embed_query("x") should produce the same vector as embed_documents(&["x"])[0]
    let emb = FakeEmbeddings::new(8);
    let query_vec = emb.embed_query("test input").await.unwrap();
    let batch_vecs = emb.embed_documents(&["test input"]).await.unwrap();
    assert_eq!(
        query_vec, batch_vecs[0],
        "embed_query and embed_documents should be consistent for the same text",
    );
}

#[tokio::test]
async fn fake_vectors_are_unit_normalized() {
    let emb = FakeEmbeddings::new(8);
    let vec = emb.embed_query("normalize me").await.unwrap();
    let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 0.001,
        "FakeEmbeddings should produce unit-normalized vectors, got magnitude {}",
        magnitude,
    );
}

#[tokio::test]
async fn fake_empty_batch_returns_empty() {
    let emb = FakeEmbeddings::default();
    let vecs = emb.embed_documents(&[]).await.unwrap();
    assert!(vecs.is_empty(), "empty input should produce empty output");
}
