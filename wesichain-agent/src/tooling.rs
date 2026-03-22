use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use schemars::schema::RootSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::error::ToolDispatchError;
pub use wesichain_core::{CancellationToken, Tool, ToolContext, ToolSpec, TypedTool};

pub type ToolError = wesichain_core::ToolError;

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

    /// Build a [`Vec<ToolSpec>`] suitable for inclusion in an [`LlmRequest`].
    ///
    /// The `parameters` field is the JSON Schema for the tool's arguments.
    /// The `description` field is derived from the schema's metadata when
    /// available; otherwise the tool name is used as a fallback.
    ///
    /// [`LlmRequest`]: wesichain_core::LlmRequest
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.entries
            .iter()
            .map(|e| {
                let description = e
                    .schema
                    .args_schema
                    .schema
                    .metadata
                    .as_ref()
                    .and_then(|m| m.description.clone())
                    .unwrap_or_else(|| e.name.clone());

                let parameters =
                    serde_json::to_value(&e.schema.args_schema).unwrap_or(Value::Object(
                        serde_json::Map::new(),
                    ));

                ToolSpec { name: e.name.clone(), description, parameters }
            })
            .collect()
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

    /// Dispatch multiple tool calls concurrently via `tokio::spawn`.
    ///
    /// Results are returned in the same order as `envelopes`.
    pub async fn dispatch_many(
        &self,
        envelopes: Vec<ToolCallEnvelope>,
        ctx: ToolContext,
    ) -> Vec<(String, Result<Value, ToolDispatchError>)> {
        let mut handles = Vec::with_capacity(envelopes.len());

        for envelope in envelopes {
            let call_id = envelope.call_id.clone();
            match self.dispatchers.get(&envelope.name) {
                None => {
                    let err = Err(ToolDispatchError::UnknownTool {
                        name: envelope.name.clone(),
                        call_id: envelope.call_id.clone(),
                    });
                    handles.push((call_id, tokio::spawn(async move { err })));
                }
                Some(dispatcher) => {
                    let dispatcher = dispatcher.clone();
                    let ctx = ctx.clone();
                    let name = envelope.name.clone();
                    let args = envelope.args.clone();
                    let cid = envelope.call_id.clone();
                    handles.push((
                        call_id,
                        tokio::spawn(async move {
                            dispatcher.dispatch(&name, args, cid, ctx).await
                        }),
                    ));
                }
            }
        }

        let mut results = Vec::with_capacity(handles.len());
        for (call_id, handle) in handles {
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(ToolDispatchError::Execution {
                    name: String::new(),
                    call_id: call_id.clone(),
                    source: crate::ToolError::ExecutionFailed(format!("task panicked: {join_err}")),
                }),
            };
            results.push((call_id, result));
        }
        results
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

    /// Register a `Tool` implementation (dynamic dispatch) by instance.
    ///
    /// Use this for tools that implement `wesichain_core::Tool` directly rather
    /// than `TypedTool` — e.g. `AgentAsTool` and MCP bridge tools.
    pub fn register_dynamic(mut self, tool: impl Tool + 'static) -> Self {
        let name = tool.name().to_string();
        let arc: Arc<dyn Tool> = Arc::new(tool);

        // Build a minimal schema entry using the tool's schema() Value.
        // We store the schema as args_schema and leave output_schema empty.
        let args_root: RootSchema = serde_json::from_value(arc.schema())
            .unwrap_or_else(|_| schemars::schema_for!(serde_json::Value));
        let output_root: RootSchema = schemars::schema_for!(serde_json::Value);

        self.entries.push(ToolMetadata {
            name: name.clone(),
            schema: ToolSchema {
                args_schema: args_root,
                output_schema: output_root,
            },
        });
        self.dispatchers.push(ToolDispatchMetadata {
            name,
            runner: Arc::new(DynamicToolRunner { tool: arc }),
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

#[async_trait::async_trait]
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

#[async_trait::async_trait]
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

/// Wraps a `Tool` (dynamic dispatch) as an `ErasedToolRunner`.
struct DynamicToolRunner {
    tool: Arc<dyn Tool>,
}

#[async_trait::async_trait]
impl ErasedToolRunner for DynamicToolRunner {
    async fn dispatch(
        &self,
        name: &str,
        args: Value,
        call_id: String,
        _ctx: ToolContext,
    ) -> Result<Value, ToolDispatchError> {
        self.tool.invoke(args).await.map_err(|source| ToolDispatchError::Execution {
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
