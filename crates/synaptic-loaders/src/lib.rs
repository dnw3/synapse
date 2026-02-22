mod arxiv;
mod csv_loader;
mod directory_loader;
mod file_loader;
mod github;
mod json_loader;
mod markdown_loader;
mod notion;
mod text_loader;
mod web_loader;
mod youtube;

pub use arxiv::ArxivLoader;
pub use csv_loader::CsvLoader;
pub use directory_loader::DirectoryLoader;
pub use file_loader::FileLoader;
pub use github::GitHubLoader;
pub use json_loader::JsonLoader;
pub use markdown_loader::MarkdownLoader;
pub use notion::NotionLoader;
pub use text_loader::TextLoader;
pub use web_loader::WebBaseLoader;
pub use youtube::YoutubeLoader;

// Re-export Document and Loader from core for backward compatibility
pub use synaptic_core::{Document, Loader};
