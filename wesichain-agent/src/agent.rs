use futures::stream::{self, BoxStream, StreamExt};
use wesichain_core::callbacks::{
    ensure_object, RunConfig, RunContext, RunType, ToTraceInput, ToTraceOutput,
};
use wesichain_core::{Runnable, StreamEvent, WesichainError};
use wesichain_llm::{LlmRequest, LlmResponse, Message, Role};

use crate::ToolRegistry;

pub struct ToolCallingAgent<L> {
    llm: L,
    tools: ToolRegistry,
    model: String,
    max_steps: usize,
    run_config: Option<RunConfig>,
}

impl<L> ToolCallingAgent<L> {
    pub fn new(llm: L, tools: ToolRegistry, model: String) -> Self {
        Self {
            llm,
            tools,
            model,
            max_steps: 5,
            run_config: None,
        }
    }

    pub fn max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    pub fn with_run_config(mut self, run_config: RunConfig) -> Self {
        self.run_config = Some(run_config);
        self
    }
}

#[async_trait::async_trait]
impl<L> Runnable<String, String> for ToolCallingAgent<L>
where
    L: Runnable<LlmRequest, LlmResponse> + Send + Sync,
{
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        let input_text = input.clone();
        let mut messages = vec![Message {
            role: Role::User,
            content: input,
            tool_call_id: None,
        }];

        let callbacks = self.run_config.as_ref().and_then(|run_config| {
            run_config.callbacks.clone().and_then(|manager| {
                if manager.is_noop() {
                    return None;
                }
                let name = run_config
                    .name_override
                    .clone()
                    .unwrap_or_else(|| "agent_execution".to_string());
                let root = RunContext::root(
                    RunType::Agent,
                    name,
                    run_config.tags.clone(),
                    run_config.metadata.clone(),
                );
                Some((manager, root))
            })
        });

        if let Some((manager, root)) = &callbacks {
            let inputs = ensure_object(input_text.to_trace_input());
            manager.on_start(root, &inputs).await;
        }

        for _ in 0..self.max_steps {
            let tool_specs = self.tools.to_specs();
            let request = LlmRequest {
                model: self.model.clone(),
                messages: messages.clone(),
                tools: tool_specs,
            };
            let response = match &callbacks {
                Some((manager, root)) => {
                    let llm_ctx = root.child(RunType::Llm, "llm_invoke".to_string());
                    let inputs = ensure_object(request.to_trace_input());
                    manager.on_start(&llm_ctx, &inputs).await;
                    match self.llm.invoke(request).await {
                        Ok(response) => {
                            let outputs = ensure_object(response.to_trace_output());
                            let duration_ms = llm_ctx.start_instant.elapsed().as_millis();
                            manager.on_end(&llm_ctx, &outputs, duration_ms).await;
                            response
                        }
                        Err(err) => {
                            let error = ensure_object(err.to_string().to_trace_output());
                            let duration_ms = llm_ctx.start_instant.elapsed().as_millis();
                            manager.on_error(&llm_ctx, &error, duration_ms).await;
                            let root_duration = root.start_instant.elapsed().as_millis();
                            manager.on_error(root, &error, root_duration).await;
                            return Err(err);
                        }
                    }
                }
                None => self.llm.invoke(request).await?,
            };
            if response.tool_calls.is_empty() {
                if let Some((manager, root)) = &callbacks {
                    let outputs = ensure_object(response.content.to_trace_output());
                    let duration_ms = root.start_instant.elapsed().as_millis();
                    manager.on_end(root, &outputs, duration_ms).await;
                }
                return Ok(response.content);
            }

            messages.push(Message {
                role: Role::Assistant,
                content: response.content,
                tool_call_id: None,
            });

            for call in response.tool_calls {
                let args = call.args;
                let result = match &callbacks {
                    Some((manager, root)) => {
                        let tool_ctx = root.child(RunType::Tool, call.name.clone());
                        let inputs = ensure_object(args.to_trace_input());
                        manager.on_start(&tool_ctx, &inputs).await;
                        match self.tools.call(&call.name, args).await {
                            Ok(result) => {
                                let outputs = ensure_object(result.to_trace_output());
                                let duration_ms = tool_ctx.start_instant.elapsed().as_millis();
                                manager.on_end(&tool_ctx, &outputs, duration_ms).await;
                                result
                            }
                            Err(err) => {
                                let error = ensure_object(err.to_string().to_trace_output());
                                let duration_ms = tool_ctx.start_instant.elapsed().as_millis();
                                manager.on_error(&tool_ctx, &error, duration_ms).await;
                                let root_duration = root.start_instant.elapsed().as_millis();
                                manager.on_error(root, &error, root_duration).await;
                                return Err(err);
                            }
                        }
                    }
                    None => self.tools.call(&call.name, args).await?,
                };
                messages.push(Message {
                    role: Role::Tool,
                    content: result.to_string(),
                    tool_call_id: Some(call.id.clone()),
                });
            }
        }

        let err = WesichainError::Custom(format!(
            "max steps exceeded: {}",
            self.max_steps
        ));
        if let Some((manager, root)) = &callbacks {
            let error = ensure_object(err.to_string().to_trace_output());
            let duration_ms = root.start_instant.elapsed().as_millis();
            manager.on_error(root, &error, duration_ms).await;
        }
        Err(err)
    }

    fn stream(&self, _input: String) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        stream::once(
            async move { Err(WesichainError::Custom("stream not implemented".to_string())) },
        )
        .boxed()
    }
}
