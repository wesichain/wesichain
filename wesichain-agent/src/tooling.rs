use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use schemars::{schema::RootSchema, JsonSchema};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::error::ToolDispatchError;
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

#[derive(Clone, Debug, Deserialize)]
pub struct ToolCallEnvelope {
    pub name: String,
    pub args: Value,
    pub call_id: String,
}

#[derive(Clone)]
pub struct ToolSet {
    entries: Vec<ToolMetadata>,
    schema_catalog: BTreeMap<String, ToolSchema>,
    dispatchers: BTreeMap<String, Arc<dyn ErasedToolRunner>>,
}

impl std::fmt::Debug for ToolSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSet")
            .field("entries", &self.entries)
            .field("schema_catalog_len", &self.schema_catalog.len())
            .field("dispatchers_len", &self.dispatchers.len())
            .finish()
    }
}

impl ToolSet {
    #[allow(
        clippy::new_ret_no_self,
        reason = "ToolSet::new intentionally starts a builder-first registration API"
    )]
    pub fn new() -> ToolSetBuilder {
        ToolSetBuilder {
            entries: Vec::new(),
            dispatchers: Vec::new(),
        }
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect()
    }

    pub fn schema_catalog(&self) -> &BTreeMap<String, ToolSchema> {
        &self.schema_catalog
    }

    pub async fn dispatch(
        &self,
        envelope: ToolCallEnvelope,
        ctx: ToolContext,
    ) -> Result<Value, ToolDispatchError> {
        let Some(dispatcher) = self.dispatchers.get(&envelope.name) else {
            return Err(ToolDispatchError::UnknownTool {
                name: envelope.name,
                call_id: envelope.call_id,
            });
        };

        dispatcher
            .dispatch(&envelope.name, envelope.args, envelope.call_id, ctx)
            .await
    }
}

#[derive(Clone, Default)]
pub struct ToolSetBuilder {
    entries: Vec<ToolMetadata>,
    dispatchers: Vec<ToolDispatchMetadata>,
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

    pub fn register_with<T>(mut self, tool: T) -> Self
    where
        T: TypedTool + Send + Sync + 'static,
    {
        self.entries.push(ToolMetadata {
            name: T::NAME.to_string(),
            schema: ToolSchema {
                args_schema: schemars::schema_for!(T::Args),
                output_schema: schemars::schema_for!(T::Output),
            },
        });
        self.dispatchers.push(ToolDispatchMetadata {
            name: T::NAME.to_string(),
            runner: Arc::new(TypedToolRunner { tool }),
        });
        self
    }

    pub fn build(self) -> Result<ToolSet, ToolSetBuildError> {
        let mut seen = HashSet::new();
        let mut catalog = BTreeMap::new();
        let mut dispatchers = BTreeMap::new();

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

        for dispatch in self.dispatchers {
            dispatchers.insert(dispatch.name, dispatch.runner);
        }

        Ok(ToolSet {
            entries: self.entries,
            schema_catalog: catalog,
            dispatchers,
        })
    }
}

#[derive(Clone, Debug)]
struct ToolMetadata {
    name: String,
    schema: ToolSchema,
}

#[derive(Clone)]
struct ToolDispatchMetadata {
    name: String,
    runner: Arc<dyn ErasedToolRunner>,
}

#[async_trait::async_trait(?Send)]
trait ErasedToolRunner: Send + Sync {
    async fn dispatch(
        &self,
        name: &str,
        args: Value,
        call_id: String,
        ctx: ToolContext,
    ) -> Result<Value, ToolDispatchError>;
}

#[derive(Clone)]
struct TypedToolRunner<T> {
    tool: T,
}

#[async_trait::async_trait(?Send)]
impl<T> ErasedToolRunner for TypedToolRunner<T>
where
    T: TypedTool + Send + Sync,
{
    async fn dispatch(
        &self,
        name: &str,
        args: Value,
        call_id: String,
        ctx: ToolContext,
    ) -> Result<Value, ToolDispatchError> {
        let typed_args = serde_json::from_value::<T::Args>(args).map_err(|source| {
            ToolDispatchError::InvalidArgs {
                name: name.to_string(),
                call_id: call_id.clone(),
                source,
            }
        })?;

        let output = self.tool.run(typed_args, ctx).await.map_err(|source| {
            ToolDispatchError::Execution {
                name: name.to_string(),
                call_id: call_id.clone(),
                source,
            }
        })?;

        serde_json::to_value(output).map_err(|source| ToolDispatchError::Serialization {
            name: name.to_string(),
            call_id,
            source,
        })
    }
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
