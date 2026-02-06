use std::error::Error;
use std::path::PathBuf;
use wesichain_retrieval::{load_and_split_recursive, RecursiveCharacterTextSplitter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let splitter = RecursiveCharacterTextSplitter::builder()
        .chunk_size(1000)
        .chunk_overlap(200)
        .build()
        .map_err(|e| format!("Failed to create splitter: {:?}", e))?;

    let paths = vec![PathBuf::from("tests/fixtures/sample.txt")];
    let chunks = load_and_split_recursive(paths, &splitter).await?;

    println!("Loaded and split into {} chunks:", chunks.len());
    for (i, chunk) in chunks.iter().take(3).enumerate() {
        let preview: String = chunk.content.chars().take(50).collect();
        println!("  Chunk {}: {}...", i + 1, preview);
        println!("    Metadata: {:?}", chunk.metadata);
    }

    Ok(())
}
