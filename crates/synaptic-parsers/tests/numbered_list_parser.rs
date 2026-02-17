use synaptic_core::RunnableConfig;
use synaptic_parsers::NumberedListOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn parses_numbered_items() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    let input = "1. first\n2. second\n3. third";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["first", "second", "third"]);
}

#[tokio::test]
async fn preserves_order_not_number() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    let input = "3. third\n1. first\n2. second";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["third", "first", "second"]);
}

#[tokio::test]
async fn skips_non_numbered_lines() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    let input = "Here is a list:\n1. alpha\n2. beta\nThat's all.";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["alpha", "beta"]);
}

#[tokio::test]
async fn skips_empty_lines() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    let input = "1. one\n\n2. two\n\n3. three";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["one", "two", "three"]);
}

#[tokio::test]
async fn handles_multi_digit_numbers() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    let input = "10. ten\n100. hundred";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["ten", "hundred"]);
}

#[tokio::test]
async fn returns_empty_for_no_numbered_items() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    let input = "Just some text.\nNo numbered items.";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn ignores_lines_without_space_after_dot() {
    let parser = NumberedListOutputParser;
    let config = RunnableConfig::default();
    // "1.item" without space should not match
    let input = "1.nospace\n2. with space";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result, vec!["with space"]);
}
