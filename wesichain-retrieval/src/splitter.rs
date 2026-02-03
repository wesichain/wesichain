#[derive(Debug, Clone, Copy)]
pub struct TextSplitter {
    chunk_size: usize,
}

impl TextSplitter {
    pub fn new(chunk_size: usize) -> Self {
        assert!(chunk_size > 0, "chunk_size must be greater than zero");

        Self { chunk_size }
    }

    pub fn split(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();

        for word in text.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }

            if current.len() + 1 + word.len() <= self.chunk_size {
                current.push(' ');
                current.push_str(word);
            } else {
                chunks.push(current);
                current = word.to_string();
            }
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }
}
