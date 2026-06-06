/// Splits text into overlapping chunks suitable for embedding.
/// Each chunk is at most `chunk_size` characters, with `overlap`
/// characters carried over from the previous chunk so context
/// isn't lost at boundaries.
pub struct Chunker {
    pub chunk_size: usize,
    pub overlap:    usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            overlap:    200,
        }
    }
}

impl Chunker {
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        Self { chunk_size, overlap }
    }

    pub fn chunk(&self, text: &str) -> Vec<String> {
        let text = normalize(text);
        if text.is_empty() {
            return vec![];
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();

        while start < total {
            let end = (start + self.chunk_size).min(total);

            // Try to break at a sentence boundary (. ! ?) or newline
            let end = if end < total {
                find_break(&chars, end)
            } else {
                end
            };

            let chunk: String = chars[start..end].iter().collect();
            let chunk = chunk.trim().to_string();

            if !chunk.is_empty() {
                chunks.push(chunk);
            }

            if end >= total {
                break;
            }

            // Step forward, keeping overlap
            start = end.saturating_sub(self.overlap);
        }

        chunks
    }
}

/// Walk back from `pos` to find a clean sentence/paragraph break.
fn find_break(chars: &[char], pos: usize) -> usize {
    let search_back = 150.min(pos);
    for i in (pos - search_back..pos).rev() {
        match chars[i] {
            '.' | '!' | '?' | '\n' => return i + 1,
            _ => {}
        }
    }
    pos
}

/// Collapse excessive whitespace and normalize line endings.
fn normalize(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_newline = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_newline {
                result.push('\n');
                prev_newline = true;
            }
        } else {
            result.push_str(trimmed);
            result.push('\n');
            prev_newline = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let chunker = Chunker::new(100, 20);
        let text = "Hello world. ".repeat(20);
        let chunks = chunker.chunk(&text);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 120); // slight buffer for boundary search
        }
    }

    #[test]
    fn test_empty_input() {
        let chunker = Chunker::default();
        assert!(chunker.chunk("").is_empty());
        assert!(chunker.chunk("   \n\n  ").is_empty());
    }

    #[test]
    fn test_short_text_single_chunk() {
        let chunker = Chunker::default();
        let chunks = chunker.chunk("Short text.");
        assert_eq!(chunks.len(), 1);
    }
}