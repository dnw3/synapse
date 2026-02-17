use std::collections::HashMap;

use synaptic_core::{Message, RunnableConfig};
use synaptic_prompts::{FewShotChatMessagePromptTemplate, FewShotExample, PromptTemplate};
use synaptic_runnables::Runnable;

#[test]
fn few_shot_produces_example_messages() {
    let template = FewShotChatMessagePromptTemplate::new(
        vec![
            FewShotExample {
                input: "What is 2+2?".to_string(),
                output: "4".to_string(),
            },
            FewShotExample {
                input: "What is 3+3?".to_string(),
                output: "6".to_string(),
            },
        ],
        PromptTemplate::new("{{ question }}"),
    );

    let values = HashMap::from([("question".to_string(), "What is 4+4?".to_string())]);
    let messages = template.format(&values).unwrap();

    assert_eq!(messages.len(), 5);
    assert_eq!(messages[0], Message::human("What is 2+2?"));
    assert_eq!(messages[1], Message::ai("4"));
    assert_eq!(messages[2], Message::human("What is 3+3?"));
    assert_eq!(messages[3], Message::ai("6"));
    assert_eq!(messages[4], Message::human("What is 4+4?"));
}

#[test]
fn few_shot_with_prefix() {
    let template = FewShotChatMessagePromptTemplate::new(
        vec![FewShotExample {
            input: "hi".to_string(),
            output: "hello".to_string(),
        }],
        PromptTemplate::new("{{ input }}"),
    )
    .with_prefix(PromptTemplate::new("You are a polite assistant."));

    let values = HashMap::from([("input".to_string(), "hey".to_string())]);
    let messages = template.format(&values).unwrap();

    assert_eq!(messages.len(), 4);
    assert!(messages[0].is_system());
    assert_eq!(messages[0].content(), "You are a polite assistant.");
    assert_eq!(messages[1], Message::human("hi"));
    assert_eq!(messages[2], Message::ai("hello"));
    assert_eq!(messages[3], Message::human("hey"));
}

#[tokio::test]
async fn few_shot_as_runnable() {
    let template = FewShotChatMessagePromptTemplate::new(
        vec![FewShotExample {
            input: "x".to_string(),
            output: "y".to_string(),
        }],
        PromptTemplate::new("{{ q }}"),
    );

    let config = RunnableConfig::default();
    let values = HashMap::from([("q".to_string(), "z".to_string())]);
    let messages = template.invoke(values, &config).await.unwrap();
    assert_eq!(messages.len(), 3);
}
