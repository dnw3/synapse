use crate::TextSplitter;

/// Splits text by estimated token count using a ~4 chars/token heuristic.
///
/// Splits at word boundaries to keep chunks readable. Consistent with
/// the token estimation used in `ConversationTokenBufferMemory`.
pub struct TokenTextSplitter {
    chunk_size: usize,
    chunk_overlap: usize,
}

impl TokenTextSplitter {
    /// Create a new token text splitter.
    ///
    /// `chunk_size` is in estimated tokens (not characters).
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap: 0,
        }
    }

    pub fn with_chunk_overlap(mut self, overlap: usize) -> Self {
        self.chunk_overlap = overlap;
        self
    }

    fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }
}

impl TextSplitter for TokenTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return vec![];
        }

        let mut chunks = Vec::new();
        let mut current_words: Vec<&str> = Vec::new();

        for word in &words {
            current_words.push(word);
            let current_text = current_words.join(" ");
            let tokens = Self::estimate_tokens(&current_text);

            if tokens > self.chunk_size && current_words.len() > 1 {
                // Remove last word, emit chunk
                current_words.pop();
                let chunk = current_words.join(" ");
                chunks.push(chunk);

                // Keep overlap words
                if self.chunk_overlap > 0 {
                    let overlap_text = current_words.join(" ");
                    let overlap_tokens = Self::estimate_tokens(&overlap_text);
                    while Self::estimate_tokens(&current_words.join(" ")) > self.chunk_overlap
                        && current_words.len() > 1
                    {
                        current_words.remove(0);
                    }
                    let _ = overlap_tokens; // just used for logic above
                } else {
                    current_words.clear();
                }

                current_words.push(word);
            }
        }

        if !current_words.is_empty() {
            chunks.push(current_words.join(" "));
        }

        chunks
    }
}
