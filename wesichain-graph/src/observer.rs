pub trait Observer: Send + Sync {
    fn on_node_enter(&self, _node: &str) {}
    fn on_node_exit(&self, _node: &str) {}
    fn on_error(&self, _node: &str, _error: &str) {}
    fn on_checkpoint_saved(&self, _node: &str) {}
}
