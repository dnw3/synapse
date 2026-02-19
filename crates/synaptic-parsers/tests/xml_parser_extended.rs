use synaptic_core::RunnableConfig;
use synaptic_parsers::XmlOutputParser;
use synaptic_runnables::Runnable;

#[tokio::test]
async fn multiple_same_tag_children() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<list><item>a</item><item>b</item><item>c</item></list>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.children.len(), 3);
    for (i, expected) in ["a", "b", "c"].iter().enumerate() {
        assert_eq!(result.children[i].tag, "item");
        assert_eq!(result.children[i].text, Some(expected.to_string()));
    }
}

#[tokio::test]
async fn tag_names_with_underscores_and_numbers() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<item_1>content</item_1>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "item_1");
    assert_eq!(result.text, Some("content".to_string()));
}

#[tokio::test]
async fn tag_names_with_hyphens() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<my-tag>content</my-tag>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "my-tag");
}

#[tokio::test]
async fn multiple_attributes_on_same_element() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = r#"<div id="main" class="container" style="color:red">text</div>"#;
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.attributes.len(), 3);
    assert_eq!(result.attributes["id"], "main");
    assert_eq!(result.attributes["class"], "container");
    assert_eq!(result.attributes["style"], "color:red");
}

#[tokio::test]
async fn content_with_special_characters() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<msg>Hello &amp; goodbye</msg>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    // The parser might or might not decode entities — just verify no crash
    assert_eq!(result.tag, "msg");
    assert!(result.text.is_some());
}

#[tokio::test]
async fn very_deeply_nested() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<a><b><c><d><e>deep</e></d></c></b></a>";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "a");
    let e = &result.children[0].children[0].children[0].children[0];
    assert_eq!(e.tag, "e");
    assert_eq!(e.text, Some("deep".to_string()));
}

#[tokio::test]
async fn self_closing_with_space() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let input = "<br />";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "br");
    assert_eq!(result.text, None);
}

#[tokio::test]
async fn root_tag_filter_with_surrounding_text() {
    let parser = XmlOutputParser::with_root_tag("answer");
    let config = RunnableConfig::default();
    let input = "I'll give you the answer:\n<answer>42</answer>\nThat's my answer.";
    let result = parser.invoke(input.to_string(), &config).await.unwrap();
    assert_eq!(result.tag, "answer");
    assert_eq!(result.text, Some("42".to_string()));
}

#[tokio::test]
async fn error_on_mismatched_tags() {
    let parser = XmlOutputParser::new();
    let config = RunnableConfig::default();
    let err = parser
        .invoke("<open>text</close>".to_string(), &config)
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty());
}
