use futures::stream::BoxStream;

use crate::callbacks::{
    ensure_object, CallbackManager, RunContext, RunType, ToTraceInput, ToTraceOutput,
};
use crate::{Runnable, StreamEvent, Value, WesichainError};

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
    Input: Send + Sync + ToTraceInput + Clone + 'static,
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
        if self.manager.is_noop() {
            return self.inner.stream(input);
        }

        let manager = self.manager.clone();
        let parent = self.parent.clone();
        let run_type = self.run_type.clone();
        let name = self.name.clone();
        let inner_stream = self.inner.stream(input.clone());

        Box::pin(async_stream::stream! {
            let ctx = parent.child(run_type, name);
            let inputs = ensure_object(input.to_trace_input());
            manager.on_start(&ctx, &inputs).await;

            let mut got_final_answer = false;

            for await event in inner_stream {
                match &event {
                    Ok(StreamEvent::ContentChunk(chunk)) => {
                        let chunk_value = Value::String(chunk.clone());
                        manager.on_stream_chunk(&ctx, &chunk_value).await;
                    }
                    Ok(StreamEvent::ToolCallDelta { id: _, delta }) => {
                        manager.on_stream_chunk(&ctx, delta).await;
                    }
                    Ok(StreamEvent::FinalAnswer(_)) => {
                        got_final_answer = true;
                        let outputs = ensure_object(Value::String("final_answer".to_string()));
                        let duration_ms = ctx.start_instant.elapsed().as_millis();
                        manager.on_end(&ctx, &outputs, duration_ms).await;
                    }
                    Ok(_) => {
                        // Other variants don't trigger specific callbacks
                    }
                    Err(err) => {
                        let error = ensure_object(err.to_string().to_trace_output());
                        let duration_ms = ctx.start_instant.elapsed().as_millis();
                        manager.on_error(&ctx, &error, duration_ms).await;
                    }
                }
                yield event;
            }

            // If stream ended without FinalAnswer, call on_end
            if !got_final_answer {
                let outputs = ensure_object(Value::Object(serde_json::Map::new()));
                let duration_ms = ctx.start_instant.elapsed().as_millis();
                manager.on_end(&ctx, &outputs, duration_ms).await;
            }
        })
    }
}
