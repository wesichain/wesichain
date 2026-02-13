use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use tempfile::NamedTempFile;
use wesichain_core::{
    persistence::{load_json_parser, load_str_parser},
    save_runnable,
    serde::SerializableRunnable, JsonOutputParser, Runnable, RunnableExt, RunnableParallel,
    StrOutputParser, WesichainError,
};

#[tokio::test]
async fn test_serialization_str_parser() {
    let parser = StrOutputParser;
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    save_runnable::<wesichain_core::LlmResponse, String>(path, &parser).unwrap();

    // Test specific loader
    let loaded = load_str_parser(path).unwrap();
    let result = loaded
        .invoke(wesichain_core::LlmResponse {
            content: "hello".to_string(),
            tool_calls: vec![],
        })
        .await
        .unwrap();
    assert_eq!(result, "hello");

    // Test enum structure
    let content = fs::read_to_string(path).unwrap();
    let ser: SerializableRunnable = serde_json::from_str(&content).unwrap();
    match ser {
        SerializableRunnable::Parser { kind, .. } => assert_eq!(kind, "str"),
        _ => panic!("Wrong type"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct MyData {
    val: String,
}

#[tokio::test]
async fn test_serialization_json_parser() {
    let parser = JsonOutputParser::<MyData>::new();
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    save_runnable::<String, MyData>(path, &parser).unwrap();

    // Test specific loader
    let loaded = load_json_parser::<MyData>(path).unwrap();
    let result = loaded
        .invoke("{\"val\": \"test\"}".to_string())
        .await
        .unwrap();
    assert_eq!(
        result,
        MyData {
            val: "test".to_string()
        }
    );
}

// Mock Runnable for testing composition that is serializable
#[derive(Clone, Serialize, Deserialize)]
struct MockSerializableRunnable {
    suffix: String,
}

#[async_trait]
impl Runnable<String, String> for MockSerializableRunnable {
    async fn invoke(&self, input: String) -> Result<String, WesichainError> {
        Ok(format!("{}{}", input, self.suffix))
    }

    fn stream(
        &self,
        _input: String,
    ) -> futures::stream::BoxStream<'_, Result<wesichain_core::StreamEvent, WesichainError>> {
        futures::stream::empty().boxed()
    }

    // We must implement to_serializable for it to work in a chain!
    // But our Mock doesn't map to a standard SerializableRunnable variant perfectly unless we use "Passthrough" or custom?
    // Let's map it to "Tool" for now or just skip to_serializable checks for custom structs if not supported?
    // Actually, save_runnable calls to_serializable. If we return None, it fails.
    // So we must implement it.
    fn to_serializable(&self) -> Option<SerializableRunnable> {
        Some(SerializableRunnable::Tool {
            name: format!("mock{}", self.suffix),
            description: None,
            schema: None,
        })
    }
}

#[tokio::test]
async fn test_serialization_chain() {
    let step1 = MockSerializableRunnable {
        suffix: "_1".to_string(),
    };
    let step2 = MockSerializableRunnable {
        suffix: "_2".to_string(),
    };
    let chain = step1.then(step2);

    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    save_runnable(path, &chain).expect("Save failed");

    let content = fs::read_to_string(path).unwrap();
    let ser: SerializableRunnable = serde_json::from_str(&content).unwrap();

    // Check structure: Chain with 2 steps (Tool mock_1, Tool mock_2)
    match ser {
        SerializableRunnable::Chain { steps } => {
            assert_eq!(steps.len(), 2);
            match &steps[0] {
                SerializableRunnable::Tool { name, .. } => assert_eq!(name, "mock_1"),
                _ => panic!("Step 1 wrong"),
            }
            match &steps[1] {
                SerializableRunnable::Tool { name, .. } => assert_eq!(name, "mock_2"),
                _ => panic!("Step 2 wrong"),
            }
        }
        _ => panic!("Expected Chain"),
    }
}

#[tokio::test]
async fn test_serialization_parallel() {
    let step1 = Arc::new(MockSerializableRunnable {
        suffix: "_A".to_string(),
    });
    let step2 = Arc::new(MockSerializableRunnable {
        suffix: "_B".to_string(),
    });

    let mut map = BTreeMap::new();
    map.insert(
        "path_a".to_string(),
        step1 as Arc<dyn Runnable<String, String> + Send + Sync>,
    );
    map.insert(
        "path_b".to_string(),
        step2 as Arc<dyn Runnable<String, String> + Send + Sync>,
    );

    let parallel = RunnableParallel::new(map);
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    save_runnable(path, &parallel).unwrap();

    let content = fs::read_to_string(path).unwrap();
    let ser: SerializableRunnable = serde_json::from_str(&content).unwrap();

    match ser {
        SerializableRunnable::Parallel { steps } => {
            assert!(steps.contains_key("path_a"));
            assert!(steps.contains_key("path_b"));
        }
        _ => panic!("Expected Parallel"),
    }
}
