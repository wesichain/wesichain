use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{Runnable, StreamEvent, WesichainError};
use async_trait::async_trait;
use futures::future::join_all;
use futures::stream::{self, BoxStream, StreamExt};

pub struct RunnableParallel<Input, Output> {
    steps: BTreeMap<String, Arc<dyn Runnable<Input, Output> + Send + Sync>>,
}

impl<Input, Output> RunnableParallel<Input, Output> {
    pub fn new(steps: BTreeMap<String, Arc<dyn Runnable<Input, Output> + Send + Sync>>) -> Self {
        Self { steps }
    }
}

#[async_trait]
impl<Input, Output> Runnable<Input, BTreeMap<String, Output>> for RunnableParallel<Input, Output>
where
    Input: Clone + Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    async fn invoke(&self, input: Input) -> Result<BTreeMap<String, Output>, WesichainError> {
        let mut keys = Vec::new();
        let mut futures = Vec::new();

        for (key, step) in &self.steps {
            keys.push(key.clone());
            futures.push(step.invoke(input.clone()));
        }

        let results = join_all(futures).await;

        let mut output = BTreeMap::new();
        for (key, result) in keys.into_iter().zip(results) {
            output.insert(key, result?);
        }

        Ok(output)
    }

    fn stream<'a>(&'a self, input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        // Fan-out: each branch emits a Metadata event tagging its name, then its own stream.
        // All branch streams are merged with select_all for interleaved output.
        let streams: Vec<BoxStream<'a, Result<StreamEvent, WesichainError>>> = self
            .steps
            .iter()
            .map(|(key, step)| {
                let branch_name = key.clone();
                let metadata_event = Ok(StreamEvent::Metadata {
                    key: "parallel_step".to_string(),
                    value: serde_json::json!(branch_name),
                });
                stream::once(std::future::ready(metadata_event))
                    .chain(step.stream(input.clone()))
                    .boxed()
            })
            .collect();

        if streams.is_empty() {
            return stream::empty().boxed();
        }

        futures::stream::select_all(streams).boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        let mut steps = std::collections::HashMap::new();
        for (key, step) in &self.steps {
            steps.insert(key.clone(), step.to_serializable()?);
        }
        Some(crate::serde::SerializableRunnable::Parallel { steps })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ConstRunnable(String);

    #[async_trait]
    impl Runnable<String, String> for ConstRunnable {
        async fn invoke(&self, _input: String) -> Result<String, WesichainError> {
            Ok(self.0.clone())
        }

        fn stream<'a>(
            &'a self,
            _input: String,
        ) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
            stream::iter(vec![Ok(StreamEvent::ContentChunk(self.0.clone()))]).boxed()
        }
    }

    #[tokio::test]
    async fn test_parallel_invoke_two_branches() {
        let mut steps: BTreeMap<String, Arc<dyn Runnable<String, String> + Send + Sync>> =
            BTreeMap::new();
        steps.insert("a".to_string(), Arc::new(ConstRunnable("hello".to_string())));
        steps.insert("b".to_string(), Arc::new(ConstRunnable("world".to_string())));
        let parallel = RunnableParallel::new(steps);
        let result = parallel.invoke("input".to_string()).await.unwrap();
        assert_eq!(result.get("a").unwrap(), "hello");
        assert_eq!(result.get("b").unwrap(), "world");
    }

    #[tokio::test]
    async fn test_parallel_invoke_three_branches() {
        let mut steps: BTreeMap<String, Arc<dyn Runnable<String, String> + Send + Sync>> =
            BTreeMap::new();
        steps.insert("x".to_string(), Arc::new(ConstRunnable("1".to_string())));
        steps.insert("y".to_string(), Arc::new(ConstRunnable("2".to_string())));
        steps.insert("z".to_string(), Arc::new(ConstRunnable("3".to_string())));
        let parallel = RunnableParallel::new(steps);
        let result = parallel.invoke("input".to_string()).await.unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get("x").unwrap(), "1");
        assert_eq!(result.get("y").unwrap(), "2");
        assert_eq!(result.get("z").unwrap(), "3");
    }

    #[tokio::test]
    async fn test_parallel_stream_emits_from_all_branches() {
        let mut steps: BTreeMap<String, Arc<dyn Runnable<String, String> + Send + Sync>> =
            BTreeMap::new();
        steps.insert("a".to_string(), Arc::new(ConstRunnable("hello".to_string())));
        steps.insert("b".to_string(), Arc::new(ConstRunnable("world".to_string())));
        let parallel = RunnableParallel::new(steps);
        let events: Vec<_> = parallel.stream("input".to_string()).collect().await;

        // Each branch emits 1 Metadata + 1 ContentChunk → 4 total events
        assert_eq!(events.len(), 4);

        let metadata_count = events
            .iter()
            .filter(|e| matches!(e, Ok(StreamEvent::Metadata { key, .. }) if key == "parallel_step"))
            .count();
        assert_eq!(metadata_count, 2);

        let content_count = events
            .iter()
            .filter(|e| matches!(e, Ok(StreamEvent::ContentChunk(_))))
            .count();
        assert_eq!(content_count, 2);
    }

    #[tokio::test]
    async fn test_parallel_stream_empty() {
        let steps: BTreeMap<String, Arc<dyn Runnable<String, String> + Send + Sync>> =
            BTreeMap::new();
        let parallel = RunnableParallel::new(steps);
        let events: Vec<_> = parallel.stream("input".to_string()).collect().await;
        assert!(events.is_empty());
    }
}
