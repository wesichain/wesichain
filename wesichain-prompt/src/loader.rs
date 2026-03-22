//! Load [`ChatPromptTemplate`] and [`PromptTemplate`] from YAML files.
//!
//! Enables iterating on prompts without recompiling — just edit the YAML.
//!
//! # YAML format
//! ```yaml
//! messages:
//!   - role: system
//!     content: "You are a helpful {{persona}} assistant."
//!   - role: human
//!     content: "{{input}}"
//! ```
//!
//! For a simple string template (no message list):
//! ```yaml
//! template: "Summarise the following in {{language}}: {{text}}"
//! ```

use std::path::Path;

use serde::Deserialize;
use wesichain_core::WesichainError;

use crate::{ChatPromptTemplate, MessagePromptTemplate, PromptTemplate};

// ── YAML schema ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PromptFile {
    Chat { messages: Vec<MessageSpec> },
    Simple { template: String },
}

#[derive(Debug, Deserialize)]
struct MessageSpec {
    role: String,
    content: String,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Load a [`ChatPromptTemplate`] from a YAML file.
///
/// The file must contain a `messages:` list where each entry has `role` and
/// `content` fields.  Supported roles: `system`, `human`, `ai`, `assistant`,
/// `placeholder`.
///
/// For `placeholder` messages, `content` is the variable name.
pub fn load_chat_prompt(path: &Path) -> Result<ChatPromptTemplate, WesichainError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        WesichainError::InvalidConfig(format!("Cannot read prompt file {:?}: {e}", path))
    })?;

    let file: PromptFile = serde_yaml::from_str(&text).map_err(|e| {
        WesichainError::InvalidConfig(format!("Invalid YAML in {:?}: {e}", path))
    })?;

    match file {
        PromptFile::Chat { messages } => {
            let templates = messages
                .into_iter()
                .map(|m| role_to_template(&m.role, &m.content))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ChatPromptTemplate::new(templates))
        }
        PromptFile::Simple { .. } => Err(WesichainError::InvalidConfig(
            "File contains a simple template, not a chat template. \
             Use load_prompt_template() instead."
                .to_string(),
        )),
    }
}

/// Load a [`PromptTemplate`] (single string template) from a YAML file.
///
/// The file must contain a `template:` key.
pub fn load_prompt_template(path: &Path) -> Result<PromptTemplate, WesichainError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        WesichainError::InvalidConfig(format!("Cannot read prompt file {:?}: {e}", path))
    })?;

    let file: PromptFile = serde_yaml::from_str(&text).map_err(|e| {
        WesichainError::InvalidConfig(format!("Invalid YAML in {:?}: {e}", path))
    })?;

    match file {
        PromptFile::Simple { template } => Ok(PromptTemplate::new(template)),
        PromptFile::Chat { .. } => Err(WesichainError::InvalidConfig(
            "File contains a chat template, not a simple template. \
             Use load_chat_prompt() instead."
                .to_string(),
        )),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn role_to_template(role: &str, content: &str) -> Result<MessagePromptTemplate, WesichainError> {
    match role {
        "system" => Ok(MessagePromptTemplate::system(content)),
        "human" | "user" => Ok(MessagePromptTemplate::human(content)),
        "ai" | "assistant" => Ok(MessagePromptTemplate::ai(content)),
        "placeholder" => Ok(MessagePromptTemplate::placeholder(content)),
        other => Err(WesichainError::InvalidConfig(format!(
            "Unknown message role '{other}' in prompt YAML. \
             Valid roles: system, human, user, ai, assistant, placeholder."
        ))),
    }
}
