use std::collections::HashMap;

use synaptic_prompts::{PromptError, PromptTemplate};

#[test]
fn renders_all_variables() {
    let template = PromptTemplate::new("Hello, {{name}}. Today is {{day}}.");
    let values = HashMap::from([
        ("name".to_string(), "Synaptic".to_string()),
        ("day".to_string(), "Monday".to_string()),
    ]);

    let rendered = template.render(&values).expect("should render");

    assert_eq!(rendered, "Hello, Synaptic. Today is Monday.");
}

#[test]
fn returns_missing_variable_error() {
    let template = PromptTemplate::new("Hello, {{name}}.");
    let values = HashMap::new();

    let err = template.render(&values).expect_err("should fail");

    match err {
        PromptError::MissingVariable(name) => assert_eq!(name, "name"),
    }
}
