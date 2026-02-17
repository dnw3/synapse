use std::collections::HashMap;

use synapse_prompts::{PromptError, PromptTemplate};

#[test]
fn partial_variables_fill_in_template() {
    let template =
        PromptTemplate::new("Hello, {{ name }}. Today is {{ day }}.").with_partial("day", "Monday");

    let values = HashMap::from([("name".to_string(), "Alice".to_string())]);
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "Hello, Alice. Today is Monday.");
}

#[test]
fn provided_values_override_partial_variables() {
    let template = PromptTemplate::new("Color: {{ color }}").with_partial("color", "red");

    let values = HashMap::from([("color".to_string(), "blue".to_string())]);
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "Color: blue");
}

#[test]
fn partial_variables_alone_can_render() {
    let template = PromptTemplate::new("Hello, {{ name }}!").with_partial("name", "World");

    let values = HashMap::new();
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "Hello, World!");
}

#[test]
fn missing_variable_still_errors_with_partials() {
    let template =
        PromptTemplate::new("{{ greeting }}, {{ name }}!").with_partial("greeting", "Hi");

    let values = HashMap::new();
    let err = template.render(&values).expect_err("should fail");

    match err {
        PromptError::MissingVariable(name) => assert_eq!(name, "name"),
    }
}

#[test]
fn multiple_partial_variables() {
    let template = PromptTemplate::new("{{ a }} {{ b }} {{ c }}")
        .with_partial("a", "1")
        .with_partial("b", "2");

    let values = HashMap::from([("c".to_string(), "3".to_string())]);
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "1 2 3");
}

#[test]
fn no_partial_variables_works_as_before() {
    let template = PromptTemplate::new("Hello, {{ name }}.");
    let values = HashMap::from([("name".to_string(), "Synapse".to_string())]);
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "Hello, Synapse.");
}
