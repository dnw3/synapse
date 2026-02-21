mod loader;

pub use loader::PdfLoader;

// Re-export Document and Loader from core for convenience
pub use synaptic_core::{Document, Loader};
