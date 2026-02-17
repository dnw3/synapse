use crate::character::merge_splits;
use crate::language::Language;
use crate::TextSplitter;

/// Recursively splits text using a hierarchy of separators.
///
/// Tries each separator in order, splitting with the first one that produces
/// chunks small enough. If a chunk is still too large, it recurses with
/// the next separator.
///
/// Default separators: `["\n\n", "\n", " ", ""]`
pub struct RecursiveCharacterTextSplitter {
    separators: Vec<String>,
    chunk_size: usize,
    chunk_overlap: usize,
}

impl RecursiveCharacterTextSplitter {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            separators: vec![
                "\n\n".to_string(),
                "\n".to_string(),
                " ".to_string(),
                String::new(),
            ],
            chunk_size,
            chunk_overlap: 0,
        }
    }

    pub fn with_separators(mut self, separators: Vec<String>) -> Self {
        self.separators = separators;
        self
    }

    /// Create a splitter with language-aware separators.
    pub fn from_language(language: Language, chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            separators: language.separators(),
            chunk_size,
            chunk_overlap,
        }
    }

    pub fn with_chunk_overlap(mut self, overlap: usize) -> Self {
        self.chunk_overlap = overlap;
        self
    }

    fn split_recursive(&self, text: &str, separator_idx: usize) -> Vec<String> {
        if text.len() <= self.chunk_size {
            return vec![text.to_string()];
        }

        if separator_idx >= self.separators.len() {
            // No more separators, force-split by chunk_size
            return text
                .chars()
                .collect::<Vec<char>>()
                .chunks(self.chunk_size)
                .map(|c| c.iter().collect::<String>())
                .collect();
        }

        let separator = &self.separators[separator_idx];

        if separator.is_empty() {
            // Character-level split
            return text
                .chars()
                .collect::<Vec<char>>()
                .chunks(self.chunk_size)
                .map(|c| c.iter().collect::<String>())
                .collect();
        }

        let splits: Vec<&str> = text.split(separator.as_str()).collect();
        let mut final_chunks = Vec::new();
        let mut good_splits: Vec<&str> = Vec::new();

        for split in &splits {
            if split.len() <= self.chunk_size {
                good_splits.push(split);
            } else {
                // Merge any accumulated good splits first
                if !good_splits.is_empty() {
                    let merged =
                        merge_splits(&good_splits, self.chunk_size, self.chunk_overlap, separator);
                    final_chunks.extend(merged);
                    good_splits.clear();
                }
                // Recurse with next separator
                let sub_chunks = self.split_recursive(split, separator_idx + 1);
                final_chunks.extend(sub_chunks);
            }
        }

        if !good_splits.is_empty() {
            let merged = merge_splits(&good_splits, self.chunk_size, self.chunk_overlap, separator);
            final_chunks.extend(merged);
        }

        final_chunks
    }
}

impl TextSplitter for RecursiveCharacterTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        self.split_recursive(text, 0)
    }
}
