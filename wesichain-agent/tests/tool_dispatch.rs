use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wesichain_agent::{
    CancellationToken, ToolCallEnvelope, ToolContext, ToolDispatchError, ToolSet, TypedTool,
};

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

    async fn run(
        &self,
        args: Self::Args,
        _ctx: ToolContext,
    ) -> Result<Self::Output, wesichain_agent::ToolError> {
        Ok(EchoOutput { echoed: args.text })
    }
}

fn ctx() -> ToolContext {
    ToolContext {
        correlation_id: "corr-1".to_string(),
        step_id: 1,
        cancellation: CancellationToken::new(),
    }
}

#[tokio::test]
async fn unknown_tool_maps_to_unknown_tool_error() {
    let toolset = ToolSet::new().register_with(EchoTool).build().unwrap();
    let envelope = ToolCallEnvelope {
        name: "missing".to_string(),
        args: json!({"text":"hi"}),
        call_id: "call-1".to_string(),
    };

    let err = toolset.dispatch(envelope, ctx()).await.unwrap_err();
    assert!(matches!(
        err,
        ToolDispatchError::UnknownTool {
            name,
            call_id
        } if name == "missing" && call_id == "call-1"
    ));
}

#[tokio::test]
async fn invalid_args_map_to_invalid_args_error() {
    let toolset = ToolSet::new().register_with(EchoTool).build().unwrap();
    let envelope = ToolCallEnvelope {
        name: "echo".to_string(),
        args: json!({"text": 42}),
        call_id: "call-2".to_string(),
    };

    let err = toolset.dispatch(envelope, ctx()).await.unwrap_err();
    assert!(matches!(
        err,
        ToolDispatchError::InvalidArgs {
            name,
            call_id,
            ..
        } if name == "echo" && call_id == "call-2"
    ));
}

#[tokio::test]
async fn dispatch_runs_typed_tool_and_serializes_output() {
    let toolset = ToolSet::new().register_with(EchoTool).build().unwrap();
    let envelope = ToolCallEnvelope {
        name: "echo".to_string(),
        args: json!({"text":"hello"}),
        call_id: "call-3".to_string(),
    };

    let value = toolset.dispatch(envelope, ctx()).await.unwrap();
    assert_eq!(value, json!({"echoed":"hello"}));
}
