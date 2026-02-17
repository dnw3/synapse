use synaptic_core::SynapseError;

#[test]
fn new_error_variants_exist() {
    let errors = vec![
        SynapseError::Embedding("test".into()),
        SynapseError::VectorStore("test".into()),
        SynapseError::Retriever("test".into()),
        SynapseError::Loader("test".into()),
        SynapseError::Splitter("test".into()),
        SynapseError::Graph("test".into()),
        SynapseError::Cache("test".into()),
        SynapseError::Config("test".into()),
    ];
    for err in &errors {
        assert!(!err.to_string().is_empty());
    }
}
