use std::marker::PhantomData;

use crate::{Runnable, StreamEvent, WesichainError};
use async_trait::async_trait;
use futures::stream::BoxStream;

use futures::StreamExt;

/// A trait for input types that can have arguments bound to them.
pub trait Bindable: Sized + Send + Sync + 'static {
    /// Bind arguments to the input.
    /// The `args` should be a `crate::Value` (typically a JSON object).
    fn bind(&mut self, args: crate::Value) -> Result<(), WesichainError>;
}

/// A Runnable that has arguments bound to it.
pub struct RunnableBinding<R, Input, Output> {
    pub(crate) bound: R,
    pub(crate) args: crate::Value,
    pub(crate) _marker: PhantomData<(Input, Output)>,
}

impl<R, Input, Output> RunnableBinding<R, Input, Output> {
    pub fn new(bound: R, args: crate::Value) -> Self {
        Self {
            bound,
            args,
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<R, Input, Output> Runnable<Input, Output> for RunnableBinding<R, Input, Output>
where
    R: Runnable<Input, Output> + Send + Sync,
    Input: Bindable + Clone + Send + 'static,
    Output: Send + Sync + 'static,
{
    async fn invoke(&self, mut input: Input) -> Result<Output, WesichainError> {
        input.bind(self.args.clone())?;
        self.bound.invoke(input).await
    }

    fn stream(&self, mut input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        // We can't return an error easily from stream establishment, so we log or panic if bind fails?
        // Better: stream should probably return a stream that starts with an error if bind fails.
        // For now, let's assume bind logic is simple enough or handle it inside the future stream if possible.
        // But `stream` returns a BoxStream immediately.
        // The `Input` is consumed.

        // Correct approach: we need to handle the bind error.
        // Since stream signature doesn't return Result implementation-wise, we might need adjustments.
        // However, for now let's try to bind and if it errors, return a stream of one error.

        if let Err(e) = input.bind(self.args.clone()) {
            return futures::stream::once(async move { Err(e) }).boxed();
        }

        self.bound.stream(input)
    }
}
