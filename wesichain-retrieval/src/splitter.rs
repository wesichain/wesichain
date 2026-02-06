use wesichain_core::Document;

const DEFAULT_SEPARATORS: [&str; 4] = ["\n\n", "\n", " ", ""];

pub struct TextSplitter;

impl TextSplitter {
    pub fn split(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        if chunk_size == 0 {
            return Vec::new();
        }

        split_with_overlap(text, chunk_size, overlap)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitterConfigError {
    ChunkSizeMustBeGreaterThanZero,
}

#[derive(Debug, Clone)]
pub struct RecursiveCharacterTextSplitter {
    chunk_size: usize,
    chunk_overlap: usize,
    separators: Vec<String>,
}

impl RecursiveCharacterTextSplitter {
    pub fn builder() -> RecursiveCharacterTextSplitterBuilder {
        RecursiveCharacterTextSplitterBuilder::default()
    }

    pub fn split_text(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        let recursive_chunks = self.split_recursive(text, 0);
        let merged_chunks = merge_chunks(recursive_chunks, self.chunk_size)
            .into_iter()
            .filter(|chunk| !chunk.is_empty())
            .collect::<Vec<_>>();

        if self.chunk_overlap > 0 {
            return split_with_overlap(
                &merged_chunks.concat(),
                self.chunk_size,
                self.chunk_overlap,
            );
        }

        merged_chunks
    }

    pub fn split_documents(&self, documents: &[Document]) -> Vec<Document> {
        let mut chunked = Vec::new();

        for document in documents {
            for (chunk_index, content) in self.split_text(&document.content).into_iter().enumerate()
            {
                let mut metadata = document.metadata.clone();
                metadata.insert("chunk_index".to_string(), serde_json::json!(chunk_index));

                chunked.push(Document {
                    id: format!("{}:{chunk_index}", document.id),
                    content,
                    metadata,
                    embedding: None,
                });
            }
        }

        chunked
    }

    fn split_recursive(&self, text: &str, separator_index: usize) -> Vec<String> {
        if text.chars().count() <= self.chunk_size {
            return vec![text.to_string()];
        }

        if separator_index >= self.separators.len() {
            return split_by_chars(text, self.chunk_size);
        }

        let separator = &self.separators[separator_index];
        if separator.is_empty() {
            return split_by_chars(text, self.chunk_size);
        }

        if !text.contains(separator) {
            return self.split_recursive(text, separator_index + 1);
        }

        text.split_inclusive(separator)
            .filter(|part| !part.is_empty())
            .flat_map(|part| self.split_recursive(part, separator_index + 1))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct RecursiveCharacterTextSplitterBuilder {
    chunk_size: usize,
    chunk_overlap: usize,
    separators: Vec<String>,
}

impl Default for RecursiveCharacterTextSplitterBuilder {
    fn default() -> Self {
        Self {
            chunk_size: 1_000,
            chunk_overlap: 200,
            separators: DEFAULT_SEPARATORS
                .iter()
                .map(|separator| separator.to_string())
                .collect(),
        }
    }
}

impl RecursiveCharacterTextSplitterBuilder {
    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub fn chunk_overlap(mut self, chunk_overlap: usize) -> Self {
        self.chunk_overlap = chunk_overlap;
        self
    }

    pub fn separators<I, S>(mut self, separators: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.separators = separators.into_iter().map(Into::into).collect();
        self
    }

    pub fn build(self) -> Result<RecursiveCharacterTextSplitter, SplitterConfigError> {
        if self.chunk_size == 0 {
            return Err(SplitterConfigError::ChunkSizeMustBeGreaterThanZero);
        }

        let max_overlap = self.chunk_size.saturating_sub(1);
        let chunk_overlap = self.chunk_overlap.min(max_overlap);
        let separators = if self.separators.is_empty() {
            DEFAULT_SEPARATORS
                .iter()
                .map(|separator| separator.to_string())
                .collect()
        } else {
            self.separators
        };

        Ok(RecursiveCharacterTextSplitter {
            chunk_size: self.chunk_size,
            chunk_overlap,
            separators,
        })
    }
}

fn split_by_chars(text: &str, chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for character in text.chars() {
        current.push(character);
        if current.chars().count() == chunk_size {
            chunks.push(current.clone());
            current.clear();
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn merge_chunks(chunks: Vec<String>, chunk_size: usize) -> Vec<String> {
    let mut merged = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }

        let chunk_len = chunk.chars().count();
        if current_len + chunk_len <= chunk_size {
            current.push_str(&chunk);
            current_len += chunk_len;
            continue;
        }

        if !current.is_empty() {
            merged.push(std::mem::take(&mut current));
            current_len = 0;
        }

        if chunk_len <= chunk_size {
            current = chunk;
            current_len = chunk_len;
            continue;
        }

        merged.extend(split_by_chars(&chunk, chunk_size));
    }

    if !current.is_empty() {
        merged.push(current);
    }

    merged
}

fn split_with_overlap(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let chars: Vec<char> = text.chars().collect();
    let max_overlap = chunk_size.saturating_sub(1);
    let clamped_overlap = overlap.min(max_overlap);
    let step = (chunk_size - clamped_overlap).max(1);

    while start < chars.len() {
        let end = usize::min(start + chunk_size, chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);

        if end == chars.len() {
            break;
        }

        start = start.saturating_add(step);
    }

    chunks
}
