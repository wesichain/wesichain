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

    fn stream<'a>(&'a self, _input: Input) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        // Simple implementation: just invoke and emit FinalAnswer?
        // Or specific parallel event?
        // For now, let's stream empty or just rely on invoke.
        // Ideally we stream events from all children interleaved.
        // Implementation of interleaved streaming is complex.
        // Let's return empty stream for now as a placeholder or TODO.
        stream::empty().boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        let mut steps = std::collections::HashMap::new();
        for (key, step) in &self.steps {
            steps.insert(key.clone(), step.to_serializable()?);
        }
        Some(crate::serde::SerializableRunnable::Parallel { steps })
    }
}
