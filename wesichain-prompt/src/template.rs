use std::collections::HashMap;

use regex::Regex;
use wesichain_core::{Value, WesichainError};

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    template: String,
}

impl PromptTemplate {
    pub fn new(template: String) -> Self {
        Self { template }
    }

    pub fn render(&self, vars: &HashMap<String, Value>) -> Result<String, WesichainError> {
        let pattern = Regex::new(r"\{\{\s*(\w+)\s*\}\}")
            .map_err(|e| WesichainError::InvalidConfig(e.to_string()))?;
        let rendered = pattern.replace_all(&self.template, |caps: &regex::Captures| {
            let key = &caps[1];
            match vars.get(key) {
                Some(value) => value
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| value.to_string()),
                None => "".to_string(),
            }
        });
        Ok(rendered.to_string())
    }
}
