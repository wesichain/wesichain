#[async_trait::async_trait]
pub trait Observer: Send + Sync + 'static {
    async fn on_node_start(&self, node_id: &str, input: &serde_json::Value);
    async fn on_node_end(&self, node_id: &str, output: &serde_json::Value, duration_ms: u128);
    async fn on_error(&self, node_id: &str, error: &crate::GraphError);
    async fn on_tool_call(&self, _node_id: &str, _tool_name: &str, _args: &serde_json::Value) {}
    async fn on_tool_result(&self, _node_id: &str, _tool_name: &str, _result: &serde_json::Value) {}
}
