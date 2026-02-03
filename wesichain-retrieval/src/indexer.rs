use wesichain_core::{Document, Embedding, VectorStore};

use crate::RetrievalError;

pub struct Indexer<E, S> {
    embedder: E,
    store: S,
}

impl<E, S> Indexer<E, S>
where
    E: Embedding,
    S: VectorStore,
{
    pub fn new(embedder: E, store: S) -> Self {
        Self { embedder, store }
    }

    pub async fn add_documents(&self, docs: Vec<Document>) -> Result<(), RetrievalError> {
        for doc in &docs {
            if doc.id.trim().is_empty() {
                return Err(RetrievalError::InvalidId(doc.id.clone()));
            }
        }

        let texts: Vec<String> = docs.iter().map(|doc| doc.content.clone()).collect();
        let embeddings = self.embedder.embed_batch(&texts).await?;
        let docs_with_embeddings = docs
            .into_iter()
            .zip(embeddings)
            .map(|(mut doc, embedding)| {
                doc.embedding = Some(embedding);
                doc
            })
            .collect();

        self.store.add(docs_with_embeddings).await?;
        Ok(())
    }
}
