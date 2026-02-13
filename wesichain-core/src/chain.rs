use async_trait::async_trait;
use std::marker::PhantomData;

use futures::stream::{self, BoxStream};
use futures::{StreamExt, TryStreamExt};

use crate::{Retrying, Runnable, StreamEvent, WesichainError};

pub struct Chain<Head, Tail, Mid> {
    head: Head,
    tail: Tail,
    _marker: PhantomData<Mid>,
}

impl<Head, Tail, Mid> Chain<Head, Tail, Mid> {
    pub fn new(head: Head, tail: Tail) -> Self {
        Self {
            head,
            tail,
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<Input, Mid, Output, Head, Tail> Runnable<Input, Output> for Chain<Head, Tail, Mid>
where
    Input: Send + 'static,
    Mid: Send + Sync + 'static,
    Output: Send + 'static,
    Head: Runnable<Input, Mid> + Send + Sync,
    Tail: Runnable<Mid, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        let mid = self.head.invoke(input).await?;
        self.tail.invoke(mid).await
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // v0: streaming reflects the tail runnable only; the head is executed via invoke.
        let head = &self.head;
        let tail = &self.tail;
        let stream = stream::once(async move { head.invoke(input).await })
            .map_ok(move |mid| tail.stream(mid))
            .try_flatten();
        stream.boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        let head_ser = self.head.to_serializable()?;
        let tail_ser = self.tail.to_serializable()?;

        // Attempt to flatten if head is also a Chain
        let mut steps = Vec::new();
        match head_ser {
            crate::serde::SerializableRunnable::Chain { steps: mut s } => steps.append(&mut s),
            _ => steps.push(head_ser),
        }

        // Same for tail?? No, tail is just the next step.
        // Actually, Chain<A, Chain<B, C>> is A -> B -> C.
        // So if tail is a chain, we append its steps.
        match tail_ser {
            crate::serde::SerializableRunnable::Chain { steps: mut s } => steps.append(&mut s),
            _ => steps.push(tail_ser),
        }

        Some(crate::serde::SerializableRunnable::Chain { steps })
    }
}

pub trait RunnableExt<Input: Send + 'static, Output: Send + 'static>:
    Runnable<Input, Output> + Sized
{
    fn then<NextOutput, Next>(self, next: Next) -> Chain<Self, Next, Output>
    where
        Next: Runnable<Output, NextOutput> + Send + Sync,
        NextOutput: Send + 'static,
    {
        Chain::new(self, next)
    }

    fn with_retries(self, max_attempts: usize) -> Retrying<Self>
    where
        Self: Send + Sync,
        Input: Clone,
    {
        Retrying::new(self, max_attempts)
    }

    fn bind(self, args: crate::Value) -> crate::RunnableBinding<Self, Input, Output>
    where
        Self: Send + Sync,
        Input: crate::Bindable + Clone + Send + 'static,
        Output: Send + Sync + 'static,
    {
        crate::RunnableBinding::new(self, args)
    }

    fn with_fallbacks(
        self,
        fallbacks: Vec<std::sync::Arc<dyn Runnable<Input, Output> + Send + Sync>>,
    ) -> crate::RunnableWithFallbacks<Input, Output>
    where
        Self: Send + Sync + 'static,
        Input: Clone + Send + 'static,
    {
        crate::RunnableWithFallbacks::new(std::sync::Arc::new(self), fallbacks)
    }
}

impl<Input: Send + 'static, Output: Send + 'static, T> RunnableExt<Input, Output> for T where
    T: Runnable<Input, Output> + Sized
{
}

use crate::Value;
use std::sync::Arc;

/// A runtime-constructed chain that operates on `Value`.
/// This is used for deserialization where types are not known at compile time.
pub struct RuntimeChain {
    steps: Vec<Arc<dyn Runnable<Value, Value>>>,
}

impl RuntimeChain {
    pub fn new(steps: Vec<Arc<dyn Runnable<Value, Value>>>) -> Self {
        Self { steps }
    }
}

#[async_trait]
impl Runnable<Value, Value> for RuntimeChain {
    async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
        let mut current = input;
        for step in &self.steps {
            current = step.invoke(current).await?;
        }
        Ok(current)
    }

    fn stream<'a>(&'a self, input: Value) -> BoxStream<'a, Result<StreamEvent, WesichainError>> {
        if self.steps.is_empty() {
            return stream::empty().boxed();
        }

        let steps = &self.steps;

        let s = async move {
            let mut current = input;
            let last_idx = steps.len() - 1;
            for (i, step) in steps.iter().enumerate() {
                if i == last_idx {
                    break;
                }
                current = step.invoke(current).await?;
            }
            Ok::<Value, WesichainError>(current)
        };

        stream::once(s)
            .map_ok(move |val| steps.last().unwrap().stream(val))
            .try_flatten()
            .boxed()
    }

    fn to_serializable(&self) -> Option<crate::serde::SerializableRunnable> {
        let mut steps = Vec::new();
        for step in &self.steps {
            steps.push(step.to_serializable()?);
        }
        Some(crate::serde::SerializableRunnable::Chain { steps })
    }
}
