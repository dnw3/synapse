use synaptic_core::RunnableConfig;
use synaptic_parsers::XmlOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn parses_simple_element() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("<greeting>hello</greeting>".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result.tag, "greeting");
    assert_eq!(result.text, Some("hello".to_string()));
    assert!(result.children.is_empty());
    assert!(result.attributes.is_empty());
}

#[tokio::test]
async fn parses_nested_elements() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<root><child1>text1</child1><child2>text2</child2></root>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "root");
    assert_eq!(result.children.len(), 2);
    assert_eq!(result.children[0].tag, "child1");
    assert_eq!(result.children[0].text, Some("text1".to_string()));
    assert_eq!(result.children[1].tag, "child2");
    assert_eq!(result.children[1].text, Some("text2".to_string()));
}

#[tokio::test]
async fn parses_attributes() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = r#"<item id="42" type="book">The Title</item>"#;
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "item");
    assert_eq!(result.text, Some("The Title".to_string()));
    assert_eq!(result.attributes.get("id"), Some(&"42".to_string()));
    assert_eq!(result.attributes.get("type"), Some(&"book".to_string()));
}

#[tokio::test]
async fn parses_self_closing_tag() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let result = parser.invoke("<br/>".to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "br");
    assert_eq!(result.text, None);
    assert!(result.children.is_empty());
}

#[tokio::test]
async fn parses_self_closing_tag_with_attributes() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = r#"<img src="photo.jpg"/>"#;
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "img");
    assert_eq!(result.text, None);
    assert_eq!(result.attributes.get("src"), Some(&"photo.jpg".to_string()));
}

#[tokio::test]
async fn root_tag_filter() {
    let parser = XmlOutputParser::with_root_tag("data");
    let config = RunnableConfig::default();
    let input = "Here is the output:\n<data><item>value</item></data>\nDone.";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "data");
    assert_eq!(result.children.len(), 1);
    assert_eq!(result.children[0].tag, "item");
    assert_eq!(result.children[0].text, Some("value".to_string()));
}

#[tokio::test]
async fn root_tag_not_found_returns_error() {
    let parser = XmlOutputParser::with_root_tag("missing");
    let config = RunnableConfig::default();
    let err = parser
        .invoke("<other>content</other>".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("missing"));
}

#[tokio::test]
async fn error_on_invalid_xml() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let err = parser
        .invoke("not xml at all".to_string(), &config)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("expected '<'"));
}

#[tokio::test]
async fn deeply_nested_elements() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<a><b><c>deep</c></b></a>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "a");
    assert_eq!(result.children[0].tag, "b");
    assert_eq!(result.children[0].children[0].tag, "c");
    assert_eq!(
        result.children[0].children[0].text,
        Some("deep".to_string())
    );
}

#[tokio::test]
async fn handles_whitespace_around_input() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "  \n  <root>content</root>  \n  ";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "root");
    assert_eq!(result.text, Some("content".to_string()));
}

#[tokio::test]
async fn mixed_text_and_children() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<root>before <child>inner</child> after</root>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "root");
    assert_eq!(result.children.len(), 1);
    assert_eq!(result.children[0].tag, "child");
    // Text parts are collected from non-child segments
    assert!(result.text.is_some());
}

#[tokio::test]
async fn single_quoted_attributes() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<tag attr='value'>text</tag>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.attributes.get("attr"), Some(&"value".to_string()));
}

#[tokio::test]
async fn empty_element() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let result = parser
        .invoke("<empty></empty>".to_string(), &config)
        .await
        .unwrap();
    assert_eq!(result.tag, "empty");
    assert_eq!(result.text, None);
    assert!(result.children.is_empty());
}
