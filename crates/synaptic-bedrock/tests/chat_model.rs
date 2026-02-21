use synaptic_bedrock::{BedrockChatModel, BedrockConfig};
use synaptic_core::{ChatModel, ChatRequest, Message};

#[test]
fn config_builder_defaults() {
    let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0");
    assert_eq!(
        config.model_id,
        "anthropic.claude-3-5-sonnet-20241022-v2:0"
    );
    assert!(config.region.is_none());
    assert!(config.max_tokens.is_none());
    assert!(config.temperature.is_none());
    assert!(config.top_p.is_none());
    assert!(config.stop.is_none());
}

#[test]
fn config_builder_all_fields() {
    let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_region("us-west-2")
        .with_max_tokens(1000)
        .with_temperature(0.7)
        .with_top_p(0.9)
        .with_stop(vec!["END".to_string()]);

    assert_eq!(
        config.model_id,
        "anthropic.claude-3-5-sonnet-20241022-v2:0"
    );
    assert_eq!(config.region, Some("us-west-2".to_string()));
    assert_eq!(config.max_tokens, Some(1000));
    assert_eq!(config.temperature, Some(0.7));
    assert_eq!(config.top_p, Some(0.9));
    assert_eq!(config.stop, Some(vec!["END".to_string()]));
}

#[test]
fn config_builder_chaining() {
    let config = BedrockConfig::new("amazon.titan-text-express-v1")
        .with_region("eu-west-1")
        .with_max_tokens(500);

    assert_eq!(config.model_id, "amazon.titan-text-express-v1");
    assert_eq!(config.region, Some("eu-west-1".to_string()));
    assert_eq!(config.max_tokens, Some(500));
}

#[tokio::test]
#[ignore] // Requires AWS credentials
async fn integration_basic_chat() {
    let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_max_tokens(100);
    let model = BedrockChatModel::new(config).await;

    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant."),
        Message::human("Say hello in exactly one word."),
    ]);

    let response = model.chat(request).await.unwrap();
    assert!(response.message.is_ai());
    assert!(!response.message.content().is_empty());
}

#[tokio::test]
#[ignore] // Requires AWS credentials
async fn integration_streaming() {
    use futures::StreamExt;

    let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_max_tokens(50);
    let model = BedrockChatModel::new(config).await;

    let request = ChatRequest::new(vec![Message::human("Count from 1 to 5.")]);

    let mut stream = model.stream_chat(request);
    let mut chunks = Vec::new();

    while let Some(result) = stream.next().await {
        let chunk = result.unwrap();
        chunks.push(chunk);
    }

    assert!(!chunks.is_empty());
}

#[tokio::test]
#[ignore] // Requires AWS credentials
async fn integration_tool_calling() {
    use synaptic_core::{ToolDefinition, ToolChoice};

    let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_max_tokens(200);
    let model = BedrockChatModel::new(config).await;

    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["city"]
        }),
        extras: None,
    };

    let request = ChatRequest::new(vec![
        Message::human("What is the weather in San Francisco?"),
    ])
    .with_tools(vec![tool])
    .with_tool_choice(ToolChoice::Auto);

    let response = model.chat(request).await.unwrap();
    assert!(response.message.is_ai());
    // The model should call the tool.
    assert!(!response.message.tool_calls().is_empty());
    assert_eq!(response.message.tool_calls()[0].name, "get_weather");
}
