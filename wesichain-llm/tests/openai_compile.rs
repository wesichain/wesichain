#[cfg(feature = "openai")]
#[test]
fn openai_tool_calling_compiles() {
    use wesichain_llm::OpenAiClient;

    let _ = OpenAiClient::new("gpt-4o-mini".to_string());
}
