use synapse_core::RunnableConfig;
use synapse_parsers::MarkdownListOutputParser;
use synapse_runnables::Runnable;

#[tokio::test]
async fn parses_dash_items() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "- apple\n- banana\n- cherry";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["apple", "banana", "cherry"]);
}

#[tokio::test]
async fn parses_asterisk_items() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "* red\n* green\n* blue";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["red", "green", "blue"]);
}

#[tokio::test]
async fn parses_mixed_markers() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "- first\n* second\n- third";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["first", "second", "third"]);
}

#[tokio::test]
async fn skips_empty_lines() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "- one\n\n- two\n\n- three";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["one", "two", "three"]);
}

#[tokio::test]
async fn skips_non_list_lines() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "Here is a list:\n- item1\n- item2\nThat was it.";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["item1", "item2"]);
}

#[tokio::test]
async fn handles_leading_whitespace() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "  - indented1\n    * indented2";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["indented1", "indented2"]);
}

#[tokio::test]
async fn returns_empty_for_no_list_items() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "This is just a paragraph.\nNo list items here.";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn trims_item_whitespace() {
    let parser = MarkdownListOutputParser;
    let config = RunnableConfig::default();
    let input = "-   padded item   ";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["padded item"]);
}
