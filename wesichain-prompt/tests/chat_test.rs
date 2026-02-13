use serde_json::json;
use std::collections::HashMap;
use wesichain_core::{Role, Value};
use wesichain_prompt::{ChatPromptTemplate, MessagePromptTemplate};

#[tokio::test]
async fn chat_prompt_formats_messages() {
    let template = ChatPromptTemplate::new(vec![
        MessagePromptTemplate::system("You are a helpful assistant."),
        MessagePromptTemplate::human("Hello, my name is {{name}}."),
    ]);

    let mut vars = HashMap::new();
    vars.insert("name".to_string(), json!("Alice"));

    let messages = template.format_messages(&vars).unwrap();

    assert_eq!(messages.len(), 2);
    assert!(matches!(messages[0].role, Role::System));
    assert_eq!(messages[0].content, "You are a helpful assistant.");
    assert!(matches!(messages[1].role, Role::User));
    assert_eq!(messages[1].content, "Hello, my name is Alice.");
}

#[tokio::test]
async fn chat_prompt_handles_placeholders() {
    let template = ChatPromptTemplate::new(vec![
        MessagePromptTemplate::system("System"),
        MessagePromptTemplate::placeholder("history"),
        MessagePromptTemplate::human("User"),
    ]);

    let mut vars = HashMap::new();
    vars.insert(
        "history".to_string(),
        json!([
            { "role": "user", "content": "Hi" },
            { "role": "assistant", "content": "Hello" }
        ]),
    );

    let messages = template.format_messages(&vars).unwrap();

    assert_eq!(messages.len(), 4);
    assert!(matches!(messages[1].role, Role::User));
    assert_eq!(messages[1].content, "Hi");
    assert!(matches!(messages[2].role, Role::Assistant));
    assert_eq!(messages[2].content, "Hello");
}
