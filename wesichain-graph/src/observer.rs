#[async_trait::async_trait]
pub trait Observer: Send + Sync + 'static {
    async fn on_node_start(&self, node_id: &str, input: &serde_json::Value);
    async fn on_node_end(&self, node_id: &str, output: &serde_json::Value, duration_ms: u128);
    async fn on_error(&self, node_id: &str, error: &crate::GraphError);
    async fn on_tool_call(&self, _node_id: &str, _tool_name: &str, _args: &serde_json::Value) {}
    async fn on_tool_result(&self, _node_id: &str, _tool_name: &str, _result: &serde_json::Value) {}
    async fn on_checkpoint_saved(&self, _node_id: &str) {}
}

pub struct ObserverCallbackAdapter(pub std::sync::Arc<dyn Observer>);

#[async_trait::async_trait]
impl wesichain_core::CallbackHandler for ObserverCallbackAdapter {
    async fn on_start(&self, ctx: &wesichain_core::RunContext, inputs: &serde_json::Value) {
        if matches!(
            ctx.run_type,
            wesichain_core::RunType::Runnable
                | wesichain_core::RunType::Chain
                | wesichain_core::RunType::Tool
                | wesichain_core::RunType::Retriever
        ) {
            let input_value = inputs.clone();
            self.0.on_node_start(&ctx.name, &input_value).await;
        }
    }

    async fn on_end(&self, ctx: &wesichain_core::RunContext, outputs: &serde_json::Value, duration_ms: u128) {
        if matches!(
            ctx.run_type,
            wesichain_core::RunType::Runnable
                | wesichain_core::RunType::Chain
                | wesichain_core::RunType::Tool
                | wesichain_core::RunType::Retriever
        ) {
            let output_value = outputs.clone();
            self.0.on_node_end(&ctx.name, &output_value, duration_ms).await;
        }
    }

    async fn on_error(&self, ctx: &wesichain_core::RunContext, error: &serde_json::Value, _duration_ms: u128) {
        if matches!(
            ctx.run_type,
            wesichain_core::RunType::Runnable
                | wesichain_core::RunType::Chain
                | wesichain_core::RunType::Tool
                | wesichain_core::RunType::Retriever
        ) {
            // Best effort error conversion
            let error_str = error.as_str().unwrap_or(&error.to_string()).to_string();
            let graph_error = crate::GraphError::System(error_str);
            self.0.on_error(&ctx.name, &graph_error).await;
        }
    }

    async fn on_event(&self, ctx: &wesichain_core::RunContext, event: &str, data: &serde_json::Value) {
        if event == "checkpoint_saved" {
             // For checkpoint saved, prefer node_id from data if present
             let node_id = data.get("node_id")
                 .and_then(|v| v.as_str())
                 .unwrap_or(&ctx.name);
             self.0.on_checkpoint_saved(node_id).await;
        }
    }
}
