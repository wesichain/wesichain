//! Langfuse connection configuration.

/// Configuration for the Langfuse integration.
#[derive(Debug, Clone)]
pub struct LangfuseConfig {
    /// Langfuse public key (starts with `pk-lf-`)
    pub public_key: String,
    /// Langfuse secret key (starts with `sk-lf-`)
    pub secret_key: String,
    /// Langfuse host URL (default: `https://cloud.langfuse.com`)
    pub host: String,
    /// Project / session name shown in the Langfuse UI
    pub project_name: String,
    /// Maximum number of events to batch before flushing
    pub batch_size: usize,
    /// How long (seconds) to wait before auto-flushing a partial batch
    pub flush_interval_secs: u64,
}

impl Default for LangfuseConfig {
    fn default() -> Self {
        Self {
            public_key: String::new(),
            secret_key: String::new(),
            host: "https://cloud.langfuse.com".into(),
            project_name: "default".into(),
            batch_size: 20,
            flush_interval_secs: 5,
        }
    }
}
