use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wesichain_agent::{CancellationToken, ToolContext, ToolSet, TypedTool};

#[derive(Debug, Deserialize, JsonSchema)]
struct EchoArgs {
    text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct EchoOutput {
    echoed: String,
}

struct EchoTool;

impl TypedTool for EchoTool {
    type Args = EchoArgs;
    type Output = EchoOutput;

    const NAME: &'static str = "echo";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, wesichain_agent::ToolError> {
        Ok(EchoOutput { echoed: args.text })
    }
}

struct EchoToolDuplicate;

impl TypedTool for EchoToolDuplicate {
    type Args = EchoArgs;
    type Output = EchoOutput;

    const NAME: &'static str = "echo";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, wesichain_agent::ToolError> {
        Ok(EchoOutput { echoed: args.text })
    }
}

struct InvalidNameTool;

impl TypedTool for InvalidNameTool {
    type Args = EchoArgs;
    type Output = EchoOutput;

    const NAME: &'static str = "   ";

    async fn run(&self, args: Self::Args, _ctx: ToolContext) -> Result<Self::Output, wesichain_agent::ToolError> {
        Ok(EchoOutput { echoed: args.text })
    }
}

#[test]
fn build_rejects_duplicate_tool_names() {
    let err = ToolSet::new()
        .register::<EchoTool>()
        .register::<EchoToolDuplicate>()
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("duplicate"));
    assert!(err.to_string().contains("echo"));
}

#[test]
fn typed_registration_path_compiles() {
    let toolset = ToolSet::new().register::<EchoTool>().build().unwrap();
    assert_eq!(toolset.names(), ["echo"]);
}

#[test]
fn schema_catalog_contains_typed_args_and_output() {
    let toolset = ToolSet::new().register::<EchoTool>().build().unwrap();
    let catalog = toolset.schema_catalog();

    let schema = catalog.get("echo").expect("schema entry for echo");
    assert!(schema.args_schema.schema.object.is_some());
    assert!(schema.output_schema.schema.object.is_some());
}

#[test]
fn build_rejects_empty_or_whitespace_tool_names() {
    let err = ToolSet::new().register::<InvalidNameTool>().build().unwrap_err();

    assert!(err.to_string().contains("tool name"));
    assert!(err.to_string().contains("empty"));
}

#[test]
fn tool_context_contains_required_fields() {
    let ctx = ToolContext {
        correlation_id: "corr-1".to_string(),
        step_id: 7,
        cancellation: CancellationToken::new(),
    };

    assert_eq!(ctx.correlation_id, "corr-1");
    assert_eq!(ctx.step_id, 7);
    assert!(!ctx.cancellation.is_cancelled());
}
