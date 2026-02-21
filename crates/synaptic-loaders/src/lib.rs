mod csv_loader;
mod directory_loader;
mod file_loader;
mod json_loader;
mod markdown_loader;
mod text_loader;
mod web_loader;

pub use csv_loader::CsvLoader;
pub use directory_loader::DirectoryLoader;
pub use file_loader::FileLoader;
pub use json_loader::JsonLoader;
pub use markdown_loader::MarkdownLoader;
pub use text_loader::TextLoader;
pub use web_loader::WebBaseLoader;

// Re-export Document and Loader from core for backward compatibility
pub use synaptic_core::{Document, Loader};
