//! OpenAI RAG example.
//!
//! Requires: OPENAI_API_KEY environment variable.
//!
//! Run with: cargo run --bin openai_rag

use futures::StreamExt;
use wesichain_core::{Message, Role, StreamEvent};
use wesichain_llm::OpenAiCompatibleClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY environment variable must be set");

    let llm = OpenAiCompatibleClient::builder()
        .base_url("https://api.openai.com")?
        .api_key(api_key)
        .default_model("gpt-4o")
        .build()?;

    // Simulated document corpus
    let documents = [
        "Wesichain is a Rust-native LLM agent framework with resumable ReAct workflows.",
        "The framework provides a Runnable trait for composable async pipelines.",
        "Wesichain supports OpenAI, Anthropic, and Google Gemini providers.",
    ];

    let query = "What providers does Wesichain support?";

    // Simple keyword-based retrieval (in production, use vector embeddings)
    let relevant_docs: Vec<&str> = documents
        .iter()
        .filter(|doc| doc.to_lowercase().contains("provider"))
        .copied()
        .collect();

    let context = relevant_docs.join("\n");
    let prompt = format!(
        "Context:\n{}\n\nQuestion: {}\n\nAnswer based on the context above:",
        context, query
    );

    println!("Query: {}", query);
    println!("Streaming RAG answer...\n");

    let request = wesichain_core::LlmRequest {
        model: String::new(),
        messages: vec![
            Message {
                role: Role::System,
                content: "You are a helpful assistant. Answer questions based on the provided context.".into(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            Message {
                role: Role::User,
                content: prompt.into(),
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools: vec![],
        temperature: Some(0.3),
        max_tokens: Some(512),
        stop_sequences: vec![],
    };

    use wesichain_core::Runnable;
    let mut stream = llm.stream(request);
    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentChunk(text) => print!("{}", text),
            StreamEvent::FinalAnswer(_) => println!("\n\nStream complete."),
            _ => {}
        }
    }

    Ok(())
}
