use synaptic_parsers::{
    EnumOutputParser, FormatInstructions, JsonOutputParser, ListOutputParser, ListSeparator,
    StrOutputParser, StructuredOutputParser,
};

#[test]
fn str_parser_returns_empty() {
    let parser = StrOutputParser;
    assert_eq!(parser.get_format_instructions(), "");
}

#[test]
fn json_parser_instructions() {
    let parser = JsonOutputParser;
    assert_eq!(
        parser.get_format_instructions(),
        "Your response should be a valid JSON object."
    );
}

#[test]
fn list_parser_newline_instructions() {
    let parser = ListOutputParser::newline();
    assert_eq!(
        parser.get_format_instructions(),
        "Your response should be a list of items separated by newlines."
    );
}

#[test]
fn list_parser_comma_instructions() {
    let parser = ListOutputParser::comma();
    assert_eq!(
        parser.get_format_instructions(),
        "Your response should be a list of items separated by commas."
    );
}

#[test]
fn list_parser_custom_separator_instructions() {
    let parser = ListOutputParser::new(ListSeparator::Custom(" | ".to_string()));
    assert_eq!(
        parser.get_format_instructions(),
        "Your response should be a list of items separated by  | ."
    );
}

#[test]
fn enum_parser_instructions() {
    let parser = EnumOutputParser::new(vec![
        "yes".to_string(),
        "no".to_string(),
        "maybe".to_string(),
    ]);
    assert_eq!(
        parser.get_format_instructions(),
        "Your response should be one of the following values: yes, no, maybe"
    );
}

#[test]
fn structured_parser_instructions() {
    let parser = StructuredOutputParser::<serde_json::Value>::new();
    assert_eq!(
        parser.get_format_instructions(),
        "Your response should be a valid JSON object matching the expected schema."
    );
}
