//! Anthropic Claude provider for Wesichain.
//!
//! # Quick start
//!
//! ```no_run
//! use wesichain_anthropic::AnthropicClient;
//! use wesichain_core::{LlmRequest, Message, Role, Runnable};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = AnthropicClient::new(
//!         std::env::var("ANTHROPIC_API_KEY").unwrap(),
//!         "claude-3-5-sonnet-20241022",
//!     );
//!
//!     let request = LlmRequest {
//!         model: String::new(),
//!         messages: vec![Message {
//!             role: Role::User,
//!             content: "Hello, Claude!".into(),
//!             tool_call_id: None,
//!             tool_calls: vec![],
//!         }],
//!         tools: vec![],
//!         temperature: None,
//!         max_tokens: None,
//!         stop_sequences: vec![],  // note: field name matches LlmRequest
//!     };
//!
//!     let response = client.invoke(request).await?;
//!     println!("{}", response.content);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod stream;
pub mod types;

pub use client::AnthropicClient;
pub use types::{
    AnthropicContent, AnthropicMessage, AnthropicPart, AnthropicRequest, AnthropicResponse,
    AnthropicTool, AnthropicUsage, ResponseContentBlock,
};
