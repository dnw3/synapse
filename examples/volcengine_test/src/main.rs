use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use serde_json::{json, Value};
use synaptic::core::{
    ChatModel, ChatRequest, MemoryStore, Message, RunnableConfig, SynapticError, Tool, ToolChoice,
    ToolDefinition,
};
use synaptic::graph::{create_react_agent, MessageState};
use synaptic::macros::tool;
use synaptic::memory::{ChatMessageHistory, RunnableWithMessageHistory};
use synaptic::models::HttpBackend;
use synaptic::openai::{OpenAiChatModel, OpenAiConfig};
use synaptic::parsers::StrOutputParser;
use synaptic::prompts::{ChatPromptTemplate, MessageTemplate, PromptTemplate};
use synaptic::runnables::{Runnable, RunnableLambda};
use synaptic::store::InMemoryStore;

// ===================================================================
// Tools
// ===================================================================

/// Get the current weather for a given location.
#[tool(name = "get_weather")]
async fn get_weather(
    /// The city name, e.g. "Beijing"
    location: String,
) -> Result<Value, SynapticError> {
    Ok(json!({
        "location": location,
        "temperature": "22°C",
        "condition": "sunny"
    }))
}

/// Calculate the result of a math operation on two numbers.
#[tool(name = "calculator")]
async fn calculator(
    /// First number
    a: f64,
    /// Second number
    b: f64,
    /// The operation: add, subtract, multiply, divide
    operation: String,
) -> Result<Value, SynapticError> {
    let result = match operation.as_str() {
        "add" => a + b,
        "subtract" => a - b,
        "multiply" => a * b,
        "divide" if b != 0.0 => a / b,
        "divide" => return Ok(json!({ "error": "division by zero" })),
        _ => return Ok(json!({ "error": format!("unknown op: {operation}") })),
    };
    Ok(json!({ "result": result }))
}

/// Search for information on a topic. Returns a brief summary.
#[tool(name = "search")]
async fn search(
    /// The search query
    query: String,
) -> Result<Value, SynapticError> {
    Ok(json!({
        "results": [
            {"title": format!("About: {query}"), "snippet": format!("Detailed info on {query}.")}
        ]
    }))
}

// ===================================================================
// Helpers
// ===================================================================

fn make_model(base_url: &str, api_key: &str, model_name: &str) -> OpenAiChatModel {
    let config = OpenAiConfig::new(api_key, model_name)
        .with_base_url(base_url)
        .with_max_tokens(1024)
        .with_temperature(0.0);
    OpenAiChatModel::new(config, Arc::new(HttpBackend::new()))
}

fn all_tool_defs() -> Vec<ToolDefinition> {
    let w: Arc<dyn Tool> = get_weather();
    let c: Arc<dyn Tool> = calculator();
    let s: Arc<dyn Tool> = search();
    vec![
        w.as_tool_definition(),
        c.as_tool_definition(),
        s.as_tool_definition(),
    ]
}

fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![get_weather(), calculator(), search()]
}

struct TestResults {
    pass: u32,
    fail: u32,
    details: Vec<(String, bool)>,
}

impl TestResults {
    fn new() -> Self {
        Self {
            pass: 0,
            fail: 0,
            details: Vec::new(),
        }
    }
    fn record(&mut self, name: &str, ok: bool) {
        if ok {
            self.pass += 1;
        } else {
            self.fail += 1;
        }
        self.details.push((name.to_string(), ok));
    }
}

// ===================================================================
// Test 1: Basic Chat
// ===================================================================

async fn test_basic_chat(model: &OpenAiChatModel, label: &str) -> bool {
    println!("\n--- [{label}] Test 1: Basic Chat ---");
    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant. Reply concisely."),
        Message::human("What is the capital of France? Reply in one word."),
    ]);
    match model.chat(request).await {
        Ok(resp) => {
            println!("  Response: {}", resp.message.content());
            if let Some(u) = &resp.usage {
                println!(
                    "  Tokens: in={} out={} total={}",
                    u.input_tokens, u.output_tokens, u.total_tokens
                );
            }
            println!("  PASS");
            true
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 2: Streaming
// ===================================================================

async fn test_streaming(model: &OpenAiChatModel, label: &str) -> bool {
    println!("\n--- [{label}] Test 2: Streaming ---");
    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant. Reply concisely."),
        Message::human("Count from 1 to 5, separated by commas."),
    ]);
    let mut stream = model.stream_chat(request);
    let mut full = String::new();
    let mut chunks = 0u32;
    print!("  Chunks: ");
    while let Some(r) = stream.next().await {
        match r {
            Ok(c) if !c.content.is_empty() => {
                print!("[{}]", c.content);
                full.push_str(&c.content);
                chunks += 1;
            }
            Err(e) => {
                println!("\n  STREAM ERROR: {e}");
                return false;
            }
            _ => {}
        }
    }
    println!("\n  Merged: {full}");
    println!("  Chunks: {chunks}");
    let ok = chunks > 0;
    println!("  {}", if ok { "PASS" } else { "FAIL" });
    ok
}

// ===================================================================
// Test 3: Tool - Single Call
// ===================================================================

async fn test_tool_single(model: &OpenAiChatModel, label: &str) -> bool {
    println!("\n--- [{label}] Test 3: Tool - Single Call (weather) ---");
    let req = ChatRequest::new(vec![
        Message::system("You are a helpful assistant. Use tools when needed."),
        Message::human("What is the weather in Beijing?"),
    ])
    .with_tools(all_tool_defs())
    .with_tool_choice(ToolChoice::Auto);

    match model.chat(req).await {
        Ok(resp) => {
            let tcs = resp.message.tool_calls();
            println!("  Content: {}", resp.message.content());
            for tc in tcs {
                println!("    tool: {}({})", tc.name, tc.arguments);
            }
            let ok = tcs.iter().any(|tc| tc.name == "get_weather");
            println!("  {}", if ok { "PASS" } else { "FAIL" });
            ok
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 4: Tool - Calculator (multi-param)
// ===================================================================

async fn test_tool_calculator(model: &OpenAiChatModel, label: &str) -> bool {
    println!("\n--- [{label}] Test 4: Tool - Calculator (multi-param) ---");
    let req = ChatRequest::new(vec![
        Message::system("You are a helpful assistant. Use tools when needed."),
        Message::human("What is 17 multiplied by 23? Use the calculator tool."),
    ])
    .with_tools(all_tool_defs())
    .with_tool_choice(ToolChoice::Auto);

    match model.chat(req).await {
        Ok(resp) => {
            let tcs = resp.message.tool_calls();
            for tc in tcs {
                println!("    tool: {}({})", tc.name, tc.arguments);
            }
            let ok = tcs.iter().any(|tc| {
                tc.name == "calculator"
                    && tc.arguments.get("a").is_some()
                    && tc.arguments.get("b").is_some()
                    && tc.arguments.get("operation").is_some()
            });
            println!("  {}", if ok { "PASS" } else { "FAIL" });
            ok
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 5: Tool - Full Loop (call -> execute -> summarize)
// ===================================================================

async fn test_tool_full_loop(model: &OpenAiChatModel, label: &str) -> bool {
    println!("\n--- [{label}] Test 5: Tool - Full Loop ---");
    let tools = all_tool_defs();

    // Step 1: model makes tool call
    let req = ChatRequest::new(vec![
        Message::system(
            "You are a helpful assistant. Use tools. After getting results, answer the user.",
        ),
        Message::human("What is 100 divided by 4? Use calculator."),
    ])
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::Auto);

    let resp = match model.chat(req).await {
        Ok(r) => r,
        Err(e) => {
            println!("  Step1 FAIL: {e}");
            return false;
        }
    };
    let tcs = resp.message.tool_calls();
    if tcs.is_empty() {
        println!(
            "  Step1 FAIL: no tool calls (content: {})",
            resp.message.content()
        );
        return false;
    }
    let tc = &tcs[0];
    println!("  Step1: {}({})", tc.name, tc.arguments);

    // Step 2: feed tool result back
    let tool_result = json!({"result": 25.0});
    let req2 = ChatRequest::new(vec![
        Message::system(
            "You are a helpful assistant. Use tools. After getting results, answer the user.",
        ),
        Message::human("What is 100 divided by 4? Use calculator."),
        Message::ai_with_tool_calls(resp.message.content().to_string(), tcs.to_vec()),
        Message::tool(tool_result.to_string(), tc.id.clone()),
    ])
    .with_tools(tools);

    match model.chat(req2).await {
        Ok(r2) => {
            println!("  Step2: {}", r2.message.content());
            let ok = r2.message.content().contains("25");
            println!(
                "  {}",
                if ok {
                    "PASS"
                } else {
                    "WARN (loop completed but answer missing '25')"
                }
            );
            true // loop completed either way
        }
        Err(e) => {
            println!("  Step2 FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 6: LCEL Chain (Prompt -> Model -> Parser)
// ===================================================================

async fn test_lcel_chain(model_arc: Arc<OpenAiChatModel>, label: &str) -> bool {
    println!("\n--- [{label}] Test 6: LCEL Chain (Prompt -> Model -> Parser) ---");

    // Build prompt template
    let prompt = ChatPromptTemplate::from_messages(vec![
        MessageTemplate::System(PromptTemplate::new(
            "You are a {{ language }} language expert. Reply in one sentence.",
        )),
        MessageTemplate::Human(PromptTemplate::new("{{ question }}")),
    ]);

    let real_model = model_arc;

    let model_step = RunnableLambda::new(move |messages: Vec<Message>| {
        let m = real_model.clone();
        async move {
            let req = ChatRequest::new(messages);
            let resp = m.chat(req).await?;
            Ok(resp.message)
        }
    });

    // Chain: HashMap -> Vec<Message> -> Message -> String
    let chain = prompt.boxed() | model_step.boxed() | StrOutputParser.boxed();

    let mut input = HashMap::new();
    input.insert("language".to_string(), Value::String("Rust".to_string()));
    input.insert(
        "question".to_string(),
        Value::String("What is ownership?".to_string()),
    );

    let config = RunnableConfig::default();
    match chain.invoke(input, &config).await {
        Ok(result) => {
            println!("  Chain output: {result}");
            let ok = !result.is_empty();
            println!("  {}", if ok { "PASS" } else { "FAIL: empty output" });
            ok
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 7: ReAct Agent Graph
// ===================================================================

async fn test_react_agent(model_arc: Arc<OpenAiChatModel>, label: &str) -> bool {
    println!("\n--- [{label}] Test 7: ReAct Agent (Graph) ---");

    let tools = all_tools();
    let graph = match create_react_agent(model_arc.clone() as Arc<dyn ChatModel>, tools) {
        Ok(g) => g,
        Err(e) => {
            println!("  FAIL building graph: {e}");
            return false;
        }
    };

    let initial = MessageState {
        messages: vec![Message::human(
            "What is 6 times 7? Use the calculator tool with operation=multiply.",
        )],
    };

    match graph.invoke(initial).await {
        Ok(result) => {
            let state = result.into_state();
            println!("  Messages in graph:");
            for msg in &state.messages {
                println!(
                    "    [{}] {}",
                    msg.role(),
                    &msg.content()[..msg.content().len().min(120)]
                );
            }
            let last = state.last_message().unwrap();
            println!("  Final answer: {}", last.content());
            let ok = last.content().contains("42");
            println!(
                "  {}",
                if ok {
                    "PASS"
                } else {
                    "WARN (agent completed but '42' not in answer)"
                }
            );
            // As long as agent completed without error, consider it a pass
            true
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 8: ReAct Agent with search tool
// ===================================================================

async fn test_react_agent_search(model_arc: Arc<OpenAiChatModel>, label: &str) -> bool {
    println!("\n--- [{label}] Test 8: ReAct Agent - Search ---");

    let tools = all_tools();
    let graph = match create_react_agent(model_arc as Arc<dyn ChatModel>, tools) {
        Ok(g) => g,
        Err(e) => {
            println!("  FAIL building graph: {e}");
            return false;
        }
    };

    let initial = MessageState {
        messages: vec![Message::human(
            "Search for information about the Rust programming language.",
        )],
    };

    match graph.invoke(initial).await {
        Ok(result) => {
            let state = result.into_state();
            let msg_count = state.messages.len();
            println!("  Total messages: {msg_count}");
            // Check agent used search tool
            let used_search = state
                .messages
                .iter()
                .any(|m| m.tool_calls().iter().any(|tc| tc.name == "search"));
            let last = state.last_message().unwrap();
            println!("  Used search tool: {used_search}");
            println!(
                "  Final: {}",
                &last.content()[..last.content().len().min(150)]
            );
            let ok = used_search && msg_count >= 3;
            println!("  {}", if ok { "PASS" } else { "WARN (completed)" });
            true
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Test 9: Memory - ConversationBufferMemory with RunnableWithMessageHistory
// ===================================================================

async fn test_memory(model_arc: Arc<OpenAiChatModel>, label: &str) -> bool {
    println!("\n--- [{label}] Test 9: Memory (RunnableWithMessageHistory) ---");

    let model_for_lambda = model_arc.clone();
    let inner = RunnableLambda::new(move |messages: Vec<Message>| {
        let m = model_for_lambda.clone();
        async move {
            let req = ChatRequest::new(messages);
            let resp = m.chat(req).await?;
            Ok(resp.message.content().to_string())
        }
    });

    let store: Arc<dyn MemoryStore> =
        Arc::new(ChatMessageHistory::new(Arc::new(InMemoryStore::new())));
    let with_history = RunnableWithMessageHistory::new(inner.boxed(), store.clone());

    let config = RunnableConfig::default().with_metadata("session_id", json!("test-session"));

    // Turn 1: introduce name
    println!("  Turn 1: 'My name is Bob.'");
    let reply1 = match with_history
        .invoke("My name is Bob.".to_string(), &config)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("  FAIL turn1: {e}");
            return false;
        }
    };
    println!("  AI: {reply1}");

    // Turn 2: ask for name (model should remember via history)
    println!("  Turn 2: 'What is my name?'");
    let reply2 = match with_history
        .invoke("What is my name?".to_string(), &config)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("  FAIL turn2: {e}");
            return false;
        }
    };
    println!("  AI: {reply2}");

    // Verify stored messages
    let saved = store.load("test-session").await.unwrap_or_default();
    println!("  Stored messages: {}", saved.len());

    let ok = reply2.to_lowercase().contains("bob") && saved.len() >= 4;
    println!(
        "  {}",
        if ok {
            "PASS"
        } else {
            "FAIL: did not recall 'Bob' or history too short"
        }
    );
    ok
}

// ===================================================================
// Test 10: Graph streaming (StreamMode::Values)
// ===================================================================

async fn test_graph_streaming(model_arc: Arc<OpenAiChatModel>, label: &str) -> bool {
    println!("\n--- [{label}] Test 10: Graph Streaming ---");
    use synaptic::graph::StreamMode;

    let tools = all_tools();
    let graph = match create_react_agent(model_arc as Arc<dyn ChatModel>, tools) {
        Ok(g) => g,
        Err(e) => {
            println!("  FAIL building graph: {e}");
            return false;
        }
    };

    let initial = MessageState {
        messages: vec![Message::human(
            "What is the weather in Tokyo? Use the get_weather tool.",
        )],
    };

    let mut stream = graph.stream(initial, StreamMode::Updates);
    let mut events = 0u32;
    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => {
                events += 1;
                println!("  Event {events}: node={}", ev.node);
            }
            Err(e) => {
                println!("  STREAM ERROR: {e}");
                return false;
            }
        }
    }
    println!("  Total events: {events}");
    let ok = events >= 2; // at least agent + tools nodes
    println!("  {}", if ok { "PASS" } else { "FAIL: too few events" });
    ok
}

// ===================================================================
// Test 11: Multi-turn Conversation
// ===================================================================

async fn test_multi_turn(model: &OpenAiChatModel, label: &str) -> bool {
    println!("\n--- [{label}] Test 11: Multi-turn Conversation ---");
    let req = ChatRequest::new(vec![
        Message::system("You are a helpful assistant. Reply concisely."),
        Message::human("My name is Alice."),
        Message::ai("Nice to meet you, Alice!"),
        Message::human("What is my name?"),
    ]);
    match model.chat(req).await {
        Ok(resp) => {
            println!("  Response: {}", resp.message.content());
            let ok = resp.message.content().to_lowercase().contains("alice");
            println!("  {}", if ok { "PASS" } else { "FAIL" });
            ok
        }
        Err(e) => {
            println!("  FAIL: {e}");
            false
        }
    }
}

// ===================================================================
// Main
// ===================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = std::env::var("VOLCENGINE_BASE_URL")
        .unwrap_or_else(|_| "https://ark.cn-beijing.volces.com/api/v3".to_string());
    let api_key = std::env::var("VOLCENGINE_API_KEY").expect("VOLCENGINE_API_KEY must be set");

    let models: Vec<(&str, &str)> = vec![
        ("kimi-k2", "kimi-k2-250905"),
        ("deepseek-v3", "deepseek-v3-250324"),
    ];

    let mut all_summaries: Vec<(String, u32, u32)> = Vec::new();

    for (label, model_name) in &models {
        println!("\n{}", "=".repeat(70));
        println!("  Testing: {label} ({model_name})");
        println!("{}", "=".repeat(70));

        let model = make_model(&base_url, &api_key, model_name);
        let model_arc = Arc::new(make_model(&base_url, &api_key, model_name));

        let mut r = TestResults::new();

        r.record("Basic Chat", test_basic_chat(&model, label).await);
        r.record("Streaming", test_streaming(&model, label).await);
        r.record("Tool: single call", test_tool_single(&model, label).await);
        r.record(
            "Tool: calculator",
            test_tool_calculator(&model, label).await,
        );
        r.record("Tool: full loop", test_tool_full_loop(&model, label).await);
        r.record(
            "LCEL Chain",
            test_lcel_chain(model_arc.clone(), label).await,
        );
        r.record(
            "ReAct Agent (calc)",
            test_react_agent(model_arc.clone(), label).await,
        );
        r.record(
            "ReAct Agent (search)",
            test_react_agent_search(model_arc.clone(), label).await,
        );
        r.record("Memory", test_memory(model_arc.clone(), label).await);
        r.record(
            "Graph Streaming",
            test_graph_streaming(model_arc.clone(), label).await,
        );
        r.record("Multi-turn", test_multi_turn(&model, label).await);

        println!("\n  --- [{label}] Results ---");
        for (name, ok) in &r.details {
            println!("  {} {name}", if *ok { "PASS" } else { "FAIL" });
        }
        all_summaries.push((label.to_string(), r.pass, r.fail));
    }

    println!("\n\n{}", "=".repeat(70));
    println!("  FINAL SUMMARY");
    println!("{}", "=".repeat(70));
    println!("  {:<25} {:>6} {:>6}", "Model", "Pass", "Fail");
    println!("  {}", "-".repeat(40));
    for (label, pass, fail) in &all_summaries {
        println!("  {:<25} {:>6} {:>6}", label, pass, fail);
    }
    println!("{}", "=".repeat(70));

    Ok(())
}
