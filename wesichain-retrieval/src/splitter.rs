pub struct TextSplitter;

impl TextSplitter {
    pub fn split(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        if chunk_size == 0 {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut start = 0usize;
        let bytes = text.as_bytes();

        while start < bytes.len() {
            let end = usize::min(start + chunk_size, bytes.len());
            let chunk = String::from_utf8_lossy(&bytes[start..end]).to_string();
            chunks.push(chunk);

            if end == bytes.len() {
                break;
            }

            let step = chunk_size.saturating_sub(overlap);
            start = start.saturating_add(step.max(1));
        }

        chunks
    }
}
