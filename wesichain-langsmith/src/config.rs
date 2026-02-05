use std::time::Duration;

use regex::Regex;
use secrecy::SecretString;

/// Configuration for LangSmith observability.
#[derive(Clone, Debug)]
pub struct LangSmithConfig {
    /// LangSmith API key (stored as a secret).
    pub api_key: SecretString,
    /// Base URL for the LangSmith API.
    pub api_url: String,
    /// Project or session name used for runs.
    pub project_name: String,
    /// Interval between background flush attempts.
    pub flush_interval: Duration,
    /// Maximum number of events per batch.
    pub max_batch_size: usize,
    /// Maximum queued events before dropping oldest.
    pub queue_capacity: usize,
    /// Sampling rate in the range [0.0, 1.0].
    pub sampling_rate: f64,
    /// Optional redaction regex applied before truncation.
    pub redact_regex: Option<Regex>,
}

impl LangSmithConfig {
    /// Create a config with default batching and sampling settings.
    pub fn new(api_key: SecretString, project_name: impl Into<String>) -> Self {
        Self {
            api_key,
            api_url: "https://api.smith.langchain.com".to_string(),
            project_name: project_name.into(),
            flush_interval: Duration::from_secs(2),
            max_batch_size: 50,
            queue_capacity: 1000,
            sampling_rate: 1.0,
            redact_regex: None,
        }
    }
}
