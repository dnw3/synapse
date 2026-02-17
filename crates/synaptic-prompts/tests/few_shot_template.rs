use std::collections::HashMap;

use synaptic_core::RunnableConfig;
use synaptic_prompts::{FewShotExample, FewShotPromptTemplate, PromptTemplate};
use synaptic_runnables::Runnable;

#[test]
fn renders_with_examples() {
    let template = FewShotPromptTemplate::new(
        vec![
            FewShotExample {
                input: "2+2".to_string(),
                output: "4".to_string(),
            },
            FewShotExample {
                input: "3+3".to_string(),
                output: "6".to_string(),
            },
        ],
        PromptTemplate::new("Input: {{ input }}\nOutput: {{ output }}"),
        PromptTemplate::new("Input: {{ question }}\nOutput:"),
    );

    let values = HashMap::from([("question".to_string(), "4+4".to_string())]);
    let rendered = template.render(&values).unwrap();

    let expected = "Input: 2+2\nOutput: 4\n\nInput: 3+3\nOutput: 6\n\nInput: 4+4\nOutput:";
    assert_eq!(rendered, expected);
}

#[test]
fn renders_with_prefix() {
    let template = FewShotPromptTemplate::new(
        vec![FewShotExample {
            input: "hi".to_string(),
            output: "hello".to_string(),
        }],
        PromptTemplate::new("Input: {{ input }}\nOutput: {{ output }}"),
        PromptTemplate::new("Input: {{ question }}\nOutput:"),
    )
    .with_prefix("You are a math tutor. Here are some examples:");

    let values = HashMap::from([("question".to_string(), "hey".to_string())]);
    let rendered = template.render(&values).unwrap();

    let expected =
        "You are a math tutor. Here are some examples:\n\nInput: hi\nOutput: hello\n\nInput: hey\nOutput:";
    assert_eq!(rendered, expected);
}

#[test]
fn renders_with_custom_separator() {
    let template = FewShotPromptTemplate::new(
        vec![
            FewShotExample {
                input: "a".to_string(),
                output: "b".to_string(),
            },
            FewShotExample {
                input: "c".to_string(),
                output: "d".to_string(),
            },
        ],
        PromptTemplate::new("{{ input }} -> {{ output }}"),
        PromptTemplate::new("{{ input }} ->"),
    )
    .with_separator("\n---\n");

    let values = HashMap::from([("input".to_string(), "e".to_string())]);
    let rendered = template.render(&values).unwrap();

    let expected = "a -> b\n---\nc -> d\n---\ne ->";
    assert_eq!(rendered, expected);
}

#[test]
fn renders_no_examples() {
    let template = FewShotPromptTemplate::new(
        vec![],
        PromptTemplate::new("{{ input }} -> {{ output }}"),
        PromptTemplate::new("Question: {{ q }}"),
    );

    let values = HashMap::from([("q".to_string(), "hello".to_string())]);
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "Question: hello");
}

#[test]
fn renders_with_prefix_and_no_examples() {
    let template = FewShotPromptTemplate::new(
        vec![],
        PromptTemplate::new("{{ input }} -> {{ output }}"),
        PromptTemplate::new("Question: {{ q }}"),
    )
    .with_prefix("Prefix");

    let values = HashMap::from([("q".to_string(), "hello".to_string())]);
    let rendered = template.render(&values).unwrap();

    assert_eq!(rendered, "Prefix\n\nQuestion: hello");
}

#[test]
fn missing_suffix_variable_errors() {
    let template = FewShotPromptTemplate::new(
        vec![],
        PromptTemplate::new("{{ input }} -> {{ output }}"),
        PromptTemplate::new("{{ missing }}"),
    );

    let values = HashMap::new();
    let err = template.render(&values).unwrap_err();
    assert!(err.to_string().contains("missing"));
}

#[tokio::test]
async fn few_shot_template_as_runnable() {
    let template = FewShotPromptTemplate::new(
        vec![FewShotExample {
            input: "x".to_string(),
            output: "y".to_string(),
        }],
        PromptTemplate::new("{{ input }}={{ output }}"),
        PromptTemplate::new("{{ q }}="),
    );

    let config = RunnableConfig::default();
    let values = HashMap::from([("q".to_string(), "z".to_string())]);
    let result = template.invoke(values, &config).await.unwrap();

    assert_eq!(result, "x=y\n\nz=");
}
