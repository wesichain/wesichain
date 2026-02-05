//! Unit tests for SSE (Server-Sent Events) parsing

use wesichain_llm::openai_compatible::*;

#[test]
fn test_chat_completion_request_serialization() {
    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        tools: None,
        temperature: Some(0.7),
        max_tokens: Some(100),
        stream: false,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"model\":\"gpt-4\""));
    assert!(json.contains("\"temperature\":0.7"));
    assert!(json.contains("\"max_tokens\":100"));
    assert!(json.contains("\"stream\":false"));
}

#[test]
fn test_chat_completion_response_deserialization() {
    let json = r#"{
        "id": "chat-123",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello!"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    }"#;

    let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "chat-123");
    assert_eq!(response.model, "gpt-4");
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].message.content, Some("Hello!".to_string()));
    assert_eq!(response.usage.unwrap().total_tokens, 15);
}

#[test]
fn test_chat_completion_chunk_deserialization() {
    let json = r#"{
        "id": "chatcmpl-123",
        "object": "chat.completion.chunk",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "delta": {
                "content": "Hello"
            },
            "finish_reason": null
        }]
    }"#;

    let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
    assert_eq!(chunk.id, "chatcmpl-123");
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_error_response_deserialization() {
    let json = r#"{
        "error": {
            "message": "Invalid API key",
            "type": "authentication_error",
            "code": "invalid_api_key"
        }
    }"#;

    let error: OpenAiError = serde_json::from_str(json).unwrap();
    assert_eq!(error.error.message, "Invalid API key");
    assert_eq!(error.error.error_type, Some("authentication_error".to_string()));
    assert_eq!(error.error.code, Some("invalid_api_key".to_string()));
}
