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

#[async_trait::async_trait]
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
}

impl<Input: Send + 'static, Output: Send + 'static, T> RunnableExt<Input, Output> for T where
    T: Runnable<Input, Output> + Sized
{
}
