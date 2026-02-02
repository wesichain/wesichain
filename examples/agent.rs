use wesichain_agent::{Tool, ToolCallingAgent, ToolRegistry};
use wesichain_llm::OllamaClient;
use wesichain_core::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(EchoTool));

    let llm = OllamaClient::new("http://localhost:11434".to_string(), "llama3.1".to_string()).expect("client");
    let agent = ToolCallingAgent::new(llm, tools, "llama3.1".to_string());
    let result = agent.invoke("hello".to_string()).await?;
    println!("{result}");
    Ok(())
}

struct EchoTool;

#[async_trait::async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "echo" }
    fn schema(&self) -> Value { Value::from("schema") }
    async fn call(&self, input: Value) -> Result<Value, wesichain_core::WesichainError> { Ok(input) }
}
