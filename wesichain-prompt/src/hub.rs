//! Central prompt registry — load and version prompts by name.
//!
//! # Overview
//!
//! A [`PromptHub`] is a named registry of prompts.  Each entry carries a name,
//! an optional version string, an optional description, and the actual
//! template (either a simple [`PromptTemplate`] or a [`ChatPromptTemplate`]).
//!
//! The [`LocalPromptHub`] implementation scans a directory for YAML files and
//! serves them.  Files may be named:
//!
//! - `<name>.yaml` — registers under `name` with version `"latest"`
//! - `<name>@<version>.yaml` — registers under `name` at the given version
//!
//! # Example
//! ```ignore
//! use wesichain_prompt::hub::LocalPromptHub;
//!
//! let hub = LocalPromptHub::from_dir("prompts/")?;
//! let entry = hub.load("summarise", None)?;   // latest
//! let v1    = hub.load("summarise", Some("1.0"))?;
//! ```

use std::path::Path;

use wesichain_core::WesichainError;

use crate::{ChatPromptTemplate, PromptTemplate};

// ── PromptEntry ───────────────────────────────────────────────────────────────

/// The kind of template stored in a [`PromptEntry`].
#[derive(Debug, Clone)]
pub enum PromptKind {
    /// A single-string template.
    Simple(PromptTemplate),
    /// A multi-turn chat template.
    Chat(ChatPromptTemplate),
}

/// A versioned prompt stored in a [`PromptHub`].
#[derive(Debug, Clone)]
pub struct PromptEntry {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub kind: PromptKind,
}

impl PromptEntry {
    /// Returns the inner `ChatPromptTemplate`, if this entry is a chat prompt.
    pub fn as_chat(&self) -> Option<&ChatPromptTemplate> {
        match &self.kind {
            PromptKind::Chat(t) => Some(t),
            PromptKind::Simple(_) => None,
        }
    }

    /// Returns the inner `PromptTemplate`, if this entry is a simple prompt.
    pub fn as_simple(&self) -> Option<&PromptTemplate> {
        match &self.kind {
            PromptKind::Simple(t) => Some(t),
            PromptKind::Chat(_) => None,
        }
    }
}

// ── PromptHub trait ───────────────────────────────────────────────────────────

/// Trait for a central prompt registry.
pub trait PromptHub: Send + Sync {
    /// Load a prompt by name and optional version.
    ///
    /// If `version` is `None`, returns the entry tagged `"latest"` or the only
    /// registered entry if there is exactly one version.
    fn load(&self, name: &str, version: Option<&str>) -> Result<PromptEntry, WesichainError>;

    /// List all registered entries (name + version pairs).
    fn list(&self) -> Vec<(String, String)>;
}

// ── LocalPromptHub ────────────────────────────────────────────────────────────

/// A [`PromptHub`] backed by a local directory of YAML files.
///
/// File naming conventions:
/// - `<name>.yaml` → version `"latest"`
/// - `<name>@<version>.yaml` → version `"<version>"`
pub struct LocalPromptHub {
    entries: Vec<PromptEntry>,
}

impl LocalPromptHub {
    /// Build a hub by scanning `dir` for `.yaml` files.
    ///
    /// Unrecognised or malformed files are silently skipped.
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self, WesichainError> {
        let dir = dir.as_ref();
        let mut entries = Vec::new();

        let read_dir = std::fs::read_dir(dir).map_err(|e| {
            WesichainError::InvalidConfig(format!("Cannot read prompt dir {:?}: {e}", dir))
        })?;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            if let Some(pe) = Self::load_file(&path) {
                entries.push(pe);
            }
        }

        // Sort so that "latest" versions appear first within each name group
        entries.sort_by(|a, b| a.name.cmp(&b.name).then(b.version.cmp(&a.version)));
        Ok(Self { entries })
    }

    /// Register a single entry directly (useful for testing).
    pub fn with_entry(mut self, entry: PromptEntry) -> Self {
        self.entries.push(entry);
        self
    }

    fn load_file(path: &Path) -> Option<PromptEntry> {
        let stem = path.file_stem()?.to_str()?;
        let (name, version) = if let Some(at_pos) = stem.find('@') {
            let n = &stem[..at_pos];
            let v = &stem[at_pos + 1..];
            (n.to_string(), v.to_string())
        } else {
            (stem.to_string(), "latest".to_string())
        };

        // Try loading as chat prompt first, then simple
        let kind = if let Ok(chat) = crate::loader::load_chat_prompt(path) {
            PromptKind::Chat(chat)
        } else if let Ok(simple) = crate::loader::load_prompt_template(path) {
            PromptKind::Simple(simple)
        } else {
            return None;
        };

        Some(PromptEntry { name, version, description: None, kind })
    }
}

impl PromptHub for LocalPromptHub {
    fn load(&self, name: &str, version: Option<&str>) -> Result<PromptEntry, WesichainError> {
        let ver = version.unwrap_or("latest");

        // First try exact version match
        if let Some(e) = self.entries.iter().find(|e| e.name == name && e.version == ver) {
            return Ok(e.clone());
        }

        // If no version specified and no "latest" tag exists, return the last entry for the name
        if version.is_none() {
            if let Some(e) = self.entries.iter().find(|e| e.name == name) {
                return Ok(e.clone());
            }
        }

        Err(WesichainError::InvalidConfig(format!(
            "Prompt '{name}@{ver}' not found in hub"
        )))
    }

    fn list(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .map(|e| (e.name.clone(), e.version.clone()))
            .collect()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatPromptTemplate, MessagePromptTemplate};

    fn chat_entry(name: &str, version: &str) -> PromptEntry {
        PromptEntry {
            name: name.to_string(),
            version: version.to_string(),
            description: None,
            kind: PromptKind::Chat(ChatPromptTemplate::new(vec![
                MessagePromptTemplate::system("You are helpful."),
            ])),
        }
    }

    fn make_hub() -> LocalPromptHub {
        LocalPromptHub { entries: vec![] }
            .with_entry(chat_entry("summarise", "latest"))
            .with_entry(chat_entry("summarise", "1.0"))
            .with_entry(chat_entry("translate", "latest"))
    }

    #[test]
    fn load_latest() {
        let hub = make_hub();
        let e = hub.load("summarise", None).unwrap();
        assert_eq!(e.name, "summarise");
    }

    #[test]
    fn load_specific_version() {
        let hub = make_hub();
        let e = hub.load("summarise", Some("1.0")).unwrap();
        assert_eq!(e.version, "1.0");
    }

    #[test]
    fn load_missing_errors() {
        let hub = make_hub();
        assert!(hub.load("nonexistent", None).is_err());
    }

    #[test]
    fn list_returns_all() {
        let hub = make_hub();
        let listed = hub.list();
        assert_eq!(listed.len(), 3);
    }

    #[test]
    fn prompt_entry_accessors() {
        let e = chat_entry("x", "latest");
        assert!(e.as_chat().is_some());
        assert!(e.as_simple().is_none());
    }
}
