use synaptic_core::SynapticError;

#[test]
fn new_error_variants_exist() {
    let errors = vec![
        SynapticError::Embedding("test".into()),
        SynapticError::VectorStore("test".into()),
        SynapticError::Retriever("test".into()),
        SynapticError::Loader("test".into()),
        SynapticError::Splitter("test".into()),
        SynapticError::Graph("test".into()),
        SynapticError::Cache("test".into()),
        SynapticError::Config("test".into()),
    ];
    for err in &errors {
        assert!(!err.to_string().is_empty());
    }
}
