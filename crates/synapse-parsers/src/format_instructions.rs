/// Trait for parsers that can provide format instructions to include in prompts.
pub trait FormatInstructions {
    /// Return a string describing the expected output format.
    fn get_format_instructions(&self) -> String;
}
