pub struct TextSplitter;

impl TextSplitter {
    pub fn split(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        if chunk_size == 0 {
            return Vec::new();
        }

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
}
