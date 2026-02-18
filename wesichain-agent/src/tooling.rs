use std::collections::{BTreeMap, HashSet};

use schemars::{schema::RootSchema, JsonSchema};
use serde::de::DeserializeOwned;
pub use tokio_util::sync::CancellationToken;

pub type ToolError = wesichain_core::ToolError;

#[derive(Clone, Debug)]
pub struct ToolContext {
    pub correlation_id: String,
    pub step_id: u32,
    pub cancellation: CancellationToken,
}

#[allow(async_fn_in_trait)]
pub trait TypedTool {
    type Args: DeserializeOwned + JsonSchema;
    type Output: serde::Serialize + JsonSchema;

    const NAME: &'static str;

    async fn run(&self, args: Self::Args, ctx: ToolContext) -> Result<Self::Output, ToolError>;
}

#[derive(Clone, Debug)]
pub struct ToolSchema {
    pub args_schema: RootSchema,
    pub output_schema: RootSchema,
}

#[derive(Clone, Debug)]
pub struct ToolSet {
    entries: Vec<ToolMetadata>,
    schema_catalog: BTreeMap<String, ToolSchema>,
}

impl ToolSet {
    #[allow(clippy::new_ret_no_self, reason = "ToolSet::new intentionally starts a builder-first registration API")]
    pub fn new() -> ToolSetBuilder {
        ToolSetBuilder {
            entries: Vec::new(),
        }
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    pub fn schema_catalog(&self) -> &BTreeMap<String, ToolSchema> {
        &self.schema_catalog
    }
}

#[derive(Clone, Debug, Default)]
pub struct ToolSetBuilder {
    entries: Vec<ToolMetadata>,
}

impl ToolSetBuilder {
    pub fn register<T>(mut self) -> Self
    where
        T: TypedTool,
    {
        self.entries.push(ToolMetadata {
            name: T::NAME.to_string(),
            schema: ToolSchema {
                args_schema: schemars::schema_for!(T::Args),
                output_schema: schemars::schema_for!(T::Output),
            },
        });
        self
    }

    pub fn build(self) -> Result<ToolSet, ToolSetBuildError> {
        let mut seen = HashSet::new();
        let mut catalog = BTreeMap::new();

        for entry in &self.entries {
            if entry.name.trim().is_empty() {
                return Err(ToolSetBuildError::InvalidName {
                    name: entry.name.clone(),
                });
            }

            if !seen.insert(entry.name.clone()) {
                return Err(ToolSetBuildError::DuplicateName {
                    name: entry.name.clone(),
                });
            }

            catalog.insert(entry.name.clone(), entry.schema.clone());
        }

        Ok(ToolSet {
            entries: self.entries,
            schema_catalog: catalog,
        })
    }
}

#[derive(Clone, Debug)]
struct ToolMetadata {
    name: String,
    schema: ToolSchema,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolSetBuildError {
    InvalidName { name: String },
    DuplicateName { name: String },
}

impl std::fmt::Display for ToolSetBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolSetBuildError::InvalidName { name } => {
                write!(f, "tool name must not be empty or whitespace: {name:?}")
            }
            ToolSetBuildError::DuplicateName { name } => {
                write!(f, "duplicate tool name: {name}")
            }
        }
    }
}

impl std::error::Error for ToolSetBuildError {}
