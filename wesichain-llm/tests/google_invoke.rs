#![cfg(feature = "google")]

use httpmock::prelude::*;
use serde_json::json;
use wesichain_core::{Runnable, WesichainError};
use wesichain_llm::{GoogleClient, LlmRequest, Message, Role, ToolSpec};

#[tokio::test]
async fn google_invoke_maps_text_response() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:generateContent")
            .query_param("key", "test-key")
            .json_body(json!({
                "contents": [
                    {
                        "role": "user",
                        "parts": [{"text": "hi"}]
                    }
                ]
            }));
        then.status(200).json_body(json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {"text": "hello"}
                        ]
                    },
                    "finishReason": "STOP"
                }
            ]
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let response = client.invoke(request).await.unwrap();
    assert_eq!(response.content, "hello");
    assert!(response.tool_calls.is_empty());
    mock.assert();
}

#[tokio::test]
async fn google_invoke_folds_system_messages_into_system_instruction() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:generateContent")
            .query_param("key", "test-key")
            .json_body(json!({
                "systemInstruction": {
                    "parts": [
                        {"text": "You are concise."},
                        {"text": "Prefer bullet points."}
                    ]
                },
                "contents": [
                    {
                        "role": "user",
                        "parts": [{"text": "hi"}]
                    },
                    {
                        "role": "model",
                        "parts": [{"text": "hello"}]
                    },
                    {
                        "role": "user",
                        "parts": [{"text": "summarize"}]
                    }
                ]
            }));
        then.status(200).json_body(json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {"text": "ok"}
                        ]
                    },
                    "finishReason": "STOP"
                }
            ]
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![
            Message {
                role: Role::System,
                content: "You are concise.".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            Message {
                role: Role::System,
                content: "Prefer bullet points.".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            Message {
                role: Role::User,
                content: "hi".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            Message {
                role: Role::Assistant,
                content: "hello".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
            Message {
                role: Role::User,
                content: "summarize".to_string(),
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools: vec![],
    };

    let response = client.invoke(request).await.unwrap();
    assert_eq!(response.content, "ok");
    mock.assert();
}

#[tokio::test]
async fn google_invoke_maps_tool_declarations_and_function_calls() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:generateContent")
            .query_param("key", "test-key")
            .json_body(json!({
                "contents": [
                    {
                        "role": "user",
                        "parts": [{"text": "what is 2+2?"}]
                    }
                ],
                "tools": [
                    {
                        "functionDeclarations": [
                            {
                                "name": "calculator",
                                "description": "Do arithmetic",
                                "parametersJsonSchema": {
                                    "type": "object",
                                    "properties": {
                                        "expression": {"type": "string"}
                                    },
                                    "required": ["expression"]
                                }
                            }
                        ]
                    }
                ],
                "toolConfig": {
                    "functionCallingConfig": {
                        "mode": "AUTO",
                        "allowedFunctionNames": ["calculator"]
                    }
                }
            }));
        then.status(200).json_body(json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "functionCall": {
                                    "name": "calculator",
                                    "args": {"expression": "2+2"}
                                }
                            }
                        ]
                    },
                    "finishReason": "STOP"
                }
            ]
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "what is 2+2?".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![ToolSpec {
            name: "calculator".to_string(),
            description: "Do arithmetic".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string"}
                },
                "required": ["expression"]
            }),
        }],
    };

    let response = client.invoke(request).await.unwrap();
    assert_eq!(response.content, "");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id, "google_call_1");
    assert_eq!(response.tool_calls[0].name, "calculator");
    assert_eq!(response.tool_calls[0].args, json!({"expression": "2+2"}));
    mock.assert();
}

#[tokio::test]
async fn google_invoke_returns_error_for_blocked_finish_reason_without_content() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:generateContent")
            .query_param("key", "test-key");
        then.status(200).json_body(json!({
            "candidates": [
                {
                    "content": {
                        "parts": []
                    },
                    "finishReason": "SAFETY"
                }
            ]
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "forbidden".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let err = client.invoke(request).await.unwrap_err();
    assert!(matches!(err, WesichainError::LlmProvider(message) if message.contains("SAFETY")));
}

#[tokio::test]
async fn google_invoke_returns_partial_text_when_blocked_with_content() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:generateContent")
            .query_param("key", "test-key");
        then.status(200).json_body(json!({
            "candidates": [
                {
                    "content": {
                        "parts": [{"text": "partial"}]
                    },
                    "finishReason": "SAFETY"
                }
            ]
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "forbidden".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let response = client.invoke(request).await.unwrap();
    assert_eq!(response.content, "partial");
    assert!(response.tool_calls.is_empty());
}

#[tokio::test]
async fn google_invoke_surfaces_rate_limit_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1beta/models/gemini-1.5-flash:generateContent")
            .query_param("key", "test-key");
        then.status(429).json_body(json!({
            "error": {
                "message": "quota exceeded"
            }
        }));
    });

    let client = GoogleClient::new("test-key", "gemini-1.5-flash").with_base_url(server.url(""));
    let request = LlmRequest {
        model: "".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
    };

    let err = client.invoke(request).await.unwrap_err();
    assert!(
        matches!(err, WesichainError::LlmProvider(message) if message.contains("quota exceeded"))
    );
}
