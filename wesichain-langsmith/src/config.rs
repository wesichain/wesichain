use std::time::Duration;

use regex::Regex;
use secrecy::SecretString;

#[derive(Clone, Debug)]
pub struct LangSmithConfig {
    pub api_key: SecretString,
    pub api_url: String,
    pub project_name: String,
    pub flush_interval: Duration,
    pub max_batch_size: usize,
    pub queue_capacity: usize,
    pub sampling_rate: f64,
    pub redact_regex: Option<Regex>,
}

impl LangSmithConfig {
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
