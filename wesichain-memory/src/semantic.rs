//! Semantic (vector-backed) long-term memory and entity memory.
//!
//! # Components
//!
//! - [`VectorMemoryStore`] — stores conversation turns as vector embeddings,
//!   retrieves the most semantically relevant memories at query time.
//! - [`EntityMemory`] — key/value store for named entities (e.g. user name,
//!   project name) persisted across conversation turns.
//! - [`MemoryRouter`] — fan-outs load/save to multiple `Memory` layers.
//!
//! # Example
//! ```ignore
//! let mem = VectorMemoryStore::new(my_embedder, my_vector_store, 3);
//! mem.save_context("t1", &inputs, &outputs).await?;
//! let vars = mem.load_memory_variables("t1").await?;
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;
use wesichain_core::{Document, Embedding, VectorStore, WesichainError};

use crate::Memory;

// ── VectorMemoryStore ─────────────────────────────────────────────────────────

/// A long-term memory that stores conversation turns as vector embeddings and
/// retrieves the most relevant memories for each new query.
pub struct VectorMemoryStore<E, V> {
    embedder: Arc<E>,
    store: Arc<V>,
    top_k: usize,
    memory_key: String,
}

impl<E, V> VectorMemoryStore<E, V>
where
    E: Embedding + Send + Sync + 'static,
    V: VectorStore + Send + Sync + 'static,
{
    /// Create a new `VectorMemoryStore`.
    ///
    /// - `embedder`: converts text to vectors
    /// - `store`: the underlying vector store
    /// - `top_k`: number of memories to retrieve per query
    pub fn new(embedder: Arc<E>, store: Arc<V>, top_k: usize) -> Self {
        Self {
            embedder,
            store,
            top_k,
            memory_key: "history".to_string(),
        }
    }

    /// Override the key used in the returned memory variables map.
    pub fn with_memory_key(mut self, key: impl Into<String>) -> Self {
        self.memory_key = key.into();
        self
    }
}

#[async_trait]
impl<E, V> Memory for VectorMemoryStore<E, V>
where
    E: Embedding + Send + Sync + 'static,
    V: VectorStore + Send + Sync + 'static,
{
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError> {
        // Use thread_id itself as the query text — callers should pass the
        // latest user message as thread_id when they want contextual recall.
        let query_emb = self.embedder.embed(thread_id).await.map_err(|e| {
            WesichainError::LlmProvider(format!("embedding failed: {e}"))
        })?;

        let results = self
            .store
            .search(&query_emb, self.top_k, None)
            .await
            .map_err(|e| WesichainError::LlmProvider(format!("vector search failed: {e}")))?;

        let memories: Vec<Value> = results
            .into_iter()
            .map(|r| Value::String(r.document.content))
            .collect();

        let mut map = HashMap::new();
        map.insert(self.memory_key.clone(), Value::Array(memories));
        Ok(map)
    }

    async fn save_context(
        &self,
        thread_id: &str,
        inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError> {
        // Build a human-readable turn text and embed it
        let input_text = inputs
            .get("input")
            .or_else(|| inputs.values().next())
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default();
        let output_text = outputs
            .get("output")
            .or_else(|| outputs.values().next())
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default();

        let turn_text = format!("Human: {input_text}\nAI: {output_text}");

        let embedding = self.embedder.embed(&turn_text).await.map_err(|e| {
            WesichainError::LlmProvider(format!("embedding failed: {e}"))
        })?;

        let mut metadata = HashMap::new();
        metadata.insert("thread_id".to_string(), Value::String(thread_id.to_string()));

        let doc = Document {
            id: uuid::Uuid::new_v4().to_string(),
            content: turn_text,
            metadata,
            embedding: Some(embedding),
        };

        self.store
            .add(vec![doc])
            .await
            .map_err(|e| WesichainError::LlmProvider(format!("vector store write failed: {e}")))?;

        Ok(())
    }

    async fn clear(&self, _thread_id: &str) -> Result<(), WesichainError> {
        // Vector stores typically don't support per-thread bulk delete via the
        // base trait — callers can downcast if needed.
        Ok(())
    }
}

// ── EntityMemory ──────────────────────────────────────────────────────────────

/// In-memory key/value store for named entities.
///
/// On `save_context` all values from the `outputs` map are stored as entities.
/// On `load_memory_variables` the full entity map for the thread is returned
/// under the `"entities"` key.
#[derive(Default, Clone)]
pub struct EntityMemory {
    inner: Arc<Mutex<HashMap<String, HashMap<String, Value>>>>,
}

impl EntityMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Directly upsert an entity value for a thread.
    pub fn upsert(&self, thread_id: &str, key: impl Into<String>, value: Value) {
        let mut guard = self.inner.lock().unwrap();
        guard
            .entry(thread_id.to_string())
            .or_default()
            .insert(key.into(), value);
    }

    /// Return all entities for a thread as a flat map.
    pub fn entities(&self, thread_id: &str) -> HashMap<String, Value> {
        let guard = self.inner.lock().unwrap();
        guard.get(thread_id).cloned().unwrap_or_default()
    }
}

#[async_trait]
impl Memory for EntityMemory {
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError> {
        let entities = self.entities(thread_id);
        let mut map = HashMap::new();
        map.insert(
            "entities".to_string(),
            serde_json::to_value(entities)
                .map_err(|e| WesichainError::Custom(e.to_string()))?,
        );
        Ok(map)
    }

    async fn save_context(
        &self,
        thread_id: &str,
        _inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError> {
        let mut guard = self.inner.lock().unwrap();
        let thread_entities = guard.entry(thread_id.to_string()).or_default();
        for (k, v) in outputs {
            thread_entities.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    async fn clear(&self, thread_id: &str) -> Result<(), WesichainError> {
        let mut guard = self.inner.lock().unwrap();
        guard.remove(thread_id);
        Ok(())
    }
}

// ── MemoryRouter ──────────────────────────────────────────────────────────────

/// Routes memory operations to multiple [`Memory`] layers.
///
/// `load_memory_variables` merges results from all layers (later layers win on
/// key collisions).  `save_context` and `clear` are called on all layers.
pub struct MemoryRouter {
    layers: Vec<Arc<dyn Memory>>,
}

impl MemoryRouter {
    pub fn new(layers: Vec<Arc<dyn Memory>>) -> Self {
        Self { layers }
    }

    pub fn push(mut self, layer: Arc<dyn Memory>) -> Self {
        self.layers.push(layer);
        self
    }
}

#[async_trait]
impl Memory for MemoryRouter {
    async fn load_memory_variables(
        &self,
        thread_id: &str,
    ) -> Result<HashMap<String, Value>, WesichainError> {
        let mut merged = HashMap::new();
        for layer in &self.layers {
            let vars = layer.load_memory_variables(thread_id).await?;
            merged.extend(vars);
        }
        Ok(merged)
    }

    async fn save_context(
        &self,
        thread_id: &str,
        inputs: &HashMap<String, Value>,
        outputs: &HashMap<String, Value>,
    ) -> Result<(), WesichainError> {
        for layer in &self.layers {
            layer.save_context(thread_id, inputs, outputs).await?;
        }
        Ok(())
    }

    async fn clear(&self, thread_id: &str) -> Result<(), WesichainError> {
        for layer in &self.layers {
            layer.clear(thread_id).await?;
        }
        Ok(())
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wesichain_core::EmbeddingError;

    // Minimal stub embedder: returns a fixed 3-dim vector
    struct StubEmbedder;

    #[async_trait]
    impl Embedding for StubEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(vec![0.1, 0.2, 0.3])
        }
        async fn embed_batch(
            &self,
            texts: &[String],
        ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }
        fn dimension(&self) -> usize {
            3
        }
    }

    // Minimal stub vector store: stores docs in memory, returns all on search
    #[derive(Default)]
    struct StubVectorStore {
        docs: Mutex<Vec<Document>>,
    }

    #[async_trait]
    impl VectorStore for StubVectorStore {
        async fn add(&self, new_docs: Vec<Document>) -> Result<(), wesichain_core::StoreError> {
            self.docs.lock().unwrap().extend(new_docs);
            Ok(())
        }
        async fn search(
            &self,
            _query: &[f32],
            top_k: usize,
            _filter: Option<&wesichain_core::MetadataFilter>,
        ) -> Result<Vec<wesichain_core::SearchResult>, wesichain_core::StoreError> {
            let docs = self.docs.lock().unwrap();
            Ok(docs
                .iter()
                .take(top_k)
                .map(|d| wesichain_core::SearchResult { document: d.clone(), score: 1.0 })
                .collect())
        }
        async fn delete(&self, _ids: &[String]) -> Result<(), wesichain_core::StoreError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn vector_memory_round_trip() {
        let store = VectorMemoryStore::new(
            Arc::new(StubEmbedder),
            Arc::new(StubVectorStore::default()),
            5,
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), Value::String("Hello".to_string()));
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), Value::String("Hi!".to_string()));

        store.save_context("t1", &inputs, &outputs).await.unwrap();

        let vars = store.load_memory_variables("latest message").await.unwrap();
        let history = vars.get("history").unwrap().as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0].as_str().unwrap().contains("Human: Hello"));
    }

    #[tokio::test]
    async fn entity_memory_stores_and_loads() {
        let mem = EntityMemory::new();
        let mut inputs = HashMap::new();
        inputs.insert("user".to_string(), Value::String("Alice".to_string()));
        let mut outputs = HashMap::new();
        outputs.insert("project".to_string(), Value::String("wesichain".to_string()));

        mem.save_context("t1", &inputs, &outputs).await.unwrap();

        let vars = mem.load_memory_variables("t1").await.unwrap();
        let entities = vars.get("entities").unwrap().as_object().unwrap();
        assert_eq!(entities.get("project").unwrap().as_str().unwrap(), "wesichain");
    }

    #[tokio::test]
    async fn entity_memory_clear() {
        let mem = EntityMemory::new();
        mem.upsert("t1", "foo", Value::String("bar".to_string()));
        mem.clear("t1").await.unwrap();
        assert!(mem.entities("t1").is_empty());
    }

    #[tokio::test]
    async fn memory_router_merges_layers() {
        let entity_mem = Arc::new(EntityMemory::new());
        entity_mem.upsert("t1", "entity_key", Value::String("v1".to_string()));

        let router = MemoryRouter::new(vec![entity_mem]);
        let vars = router.load_memory_variables("t1").await.unwrap();
        assert!(vars.contains_key("entities"));
    }
}
