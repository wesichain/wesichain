use async_trait::async_trait;
use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_core::{Runnable, Value, WesichainError};
use wesichain_llm::OllamaClient;

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo back the provided input."
    }

    fn schema(&self) -> Value {
        Value::from("string")
    }

    async fn call(&self, input: Value) -> Result<Value, WesichainError> {
        Ok(input)
    }
}

#[tokio::main]
async fn main() -> Result<(), WesichainError> {
    let llm = OllamaClient::new(
        "http://localhost:11434".to_string(),
        "llama3.1".to_string(),
    )
    .expect("client");
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let agent = ToolCallingAgent::new(llm, tools, "llama3.1".to_string()).max_steps(3);
    let output = agent
        .invoke("Say hi, then call the echo tool with 'ok'.".to_string())
        .await?;
    println!("{output}");

    Ok(())
}
