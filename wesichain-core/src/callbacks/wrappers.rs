use futures::stream::BoxStream;

use crate::callbacks::{
    ensure_object, CallbackManager, RunContext, RunType, ToTraceInput, ToTraceOutput,
};
use crate::{Runnable, StreamEvent, WesichainError};

#[allow(dead_code)]
pub struct TracedRunnable<R> {
    inner: R,
    manager: CallbackManager,
    parent: RunContext,
    run_type: RunType,
    name: String,
}

impl<R> TracedRunnable<R> {
    #[allow(dead_code)]
    pub fn new(
        inner: R,
        manager: CallbackManager,
        parent: RunContext,
        run_type: RunType,
        name: String,
    ) -> Self {
        Self {
            inner,
            manager,
            parent,
            run_type,
            name,
        }
    }
}

#[async_trait::async_trait]
impl<Input, Output, R> Runnable<Input, Output> for TracedRunnable<R>
where
    Input: Send + Sync + ToTraceInput + 'static,
    Output: Send + Sync + ToTraceOutput + 'static,
    R: Runnable<Input, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        if self.manager.is_noop() {
            return self.inner.invoke(input).await;
        }

        let ctx = self.parent.child(self.run_type.clone(), self.name.clone());
        let inputs = ensure_object(input.to_trace_input());
        self.manager.on_start(&ctx, &inputs).await;

        let result = self.inner.invoke(input).await;
        let duration_ms = ctx.start_instant.elapsed().as_millis();

        match &result {
            Ok(output) => {
                let outputs = ensure_object(output.to_trace_output());
                self.manager.on_end(&ctx, &outputs, duration_ms).await;
            }
            Err(err) => {
                let error = ensure_object(err.to_string().to_trace_output());
                self.manager.on_error(&ctx, &error, duration_ms).await;
            }
        }

        result
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        self.inner.stream(input)
    }
}
