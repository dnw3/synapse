use crate::TextSplitter;

/// Splits text by a single separator string.
///
/// After splitting, chunks are merged to stay under `chunk_size` with
/// `chunk_overlap` characters of overlap between consecutive chunks.
pub struct CharacterTextSplitter {
    separator: String,
    chunk_size: usize,
    chunk_overlap: usize,
}

impl CharacterTextSplitter {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            separator: "\n\n".to_string(),
            chunk_size,
            chunk_overlap: 0,
        }
    }

    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    pub fn with_chunk_overlap(mut self, overlap: usize) -> Self {
        self.chunk_overlap = overlap;
        self
    }
}

impl TextSplitter for CharacterTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        let splits: Vec<&str> = text.split(&self.separator).collect();
        merge_splits(
            &splits,
            self.chunk_size,
            self.chunk_overlap,
            &self.separator,
        )
    }
}

/// Merge small splits into chunks that are at most `chunk_size` long,
/// with `overlap` characters of context from the previous chunk.
pub(crate) fn merge_splits(
    splits: &[&str],
    chunk_size: usize,
    overlap: usize,
    separator: &str,
) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_parts: Vec<&str> = Vec::new();
    let mut current_len = 0;

    for &split in splits {
        let split_len = split.len();
        let sep_len = if current_parts.is_empty() {
            0
        } else {
            separator.len()
        };

        if current_len + sep_len + split_len > chunk_size && !current_parts.is_empty() {
            let chunk = current_parts.join(separator);
            chunks.push(chunk);

            // Keep parts for overlap
            if overlap == 0 {
                current_parts.clear();
                current_len = 0;
            } else {
                while current_len > overlap && current_parts.len() > 1 {
                    let removed = current_parts.remove(0);
                    current_len -= removed.len() + separator.len();
                }
            }
        }

        current_parts.push(split);
        current_len += if current_parts.len() == 1 {
            split_len
        } else {
            separator.len() + split_len
        };
    }

    if !current_parts.is_empty() {
        chunks.push(current_parts.join(separator));
    }

    chunks
}
