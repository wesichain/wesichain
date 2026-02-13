use crate::runnable::Runnable;
use crate::serde::SerializableRunnable;
use crate::WesichainError;
use crate::{
    JsonOutputParser, StrOutputParser,
};
use futures::StreamExt;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Save a Runnable to a JSON file.
pub fn save_runnable<Input, Output>(
    path: impl AsRef<Path>,
    runnable: &dyn Runnable<Input, Output>,
) -> Result<(), WesichainError>
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    let serializable = runnable
        .to_serializable()
        .ok_or_else(|| WesichainError::Custom("Runnable is not serializable".to_string()))?;
    let serialized = serde_json::to_string_pretty(&serializable).map_err(WesichainError::Serde)?;
    fs::write(path, serialized)
        .map_err(|e| WesichainError::Custom(format!("Failed to write file: {}", e)))?;
    Ok(())
}

use crate::registry::RunnableRegistry;

use crate::{IntoValue, TryFromValue};
use std::marker::PhantomData;

struct RuntimeChainAdapter<Input, Output> {
    inner: crate::chain::RuntimeChain,
    _marker: PhantomData<(Input, Output)>,
}

#[async_trait::async_trait]
impl<Input, Output> Runnable<Input, Output> for RuntimeChainAdapter<Input, Output>
where
    Input: IntoValue + Send + Sync + 'static,
    Output: TryFromValue + Send + Sync + 'static,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        let val = input.into_value();
        let result = self.inner.invoke(val).await?;
        Output::try_from_value(result)
    }

    fn stream<'a>(
        &'a self,
        input: Input,
    ) -> futures::stream::BoxStream<'a, Result<crate::StreamEvent, WesichainError>> {
        let val = input.into_value();
        self.inner.stream(val)
    }

    fn to_serializable(&self) -> Option<SerializableRunnable> {
        self.inner.to_serializable()
    }
}

/// Load a Runnable from a JSON file.
pub fn load_runnable<Input, Output>(
    path: impl AsRef<Path>,
    registry: Option<&RunnableRegistry>,
) -> Result<Box<dyn Runnable<Input, Output>>, WesichainError>
where
    Input: IntoValue + TryFromValue + Send + Sync + 'static,
    Output: IntoValue + TryFromValue + Send + Sync + 'static,
{
    let content = fs::read_to_string(path)
        .map_err(|e| WesichainError::Custom(format!("Failed to read file: {}", e)))?;
    let serializable: SerializableRunnable =
        serde_json::from_str(&content).map_err(WesichainError::Serde)?;

    let arc = reconstruct(serializable, registry)?;
    // Wrap Arc in Box to match return type.
    Ok(Box::new(arc))
}

pub fn reconstruct<Input, Output>(
    ser: SerializableRunnable,
    registry: Option<&RunnableRegistry>,
) -> Result<Arc<dyn Runnable<Input, Output> + Send + Sync>, WesichainError>
where
    Input: IntoValue + TryFromValue + Send + Sync + 'static,
    Output: IntoValue + TryFromValue + Send + Sync + 'static,
{
    match ser {
        SerializableRunnable::Parser { kind, .. } => {
            if kind == "str" {
                // Adapter for StrOutputParser
                struct StrParserAdapter {
                    inner: crate::StrOutputParser,
                }
                #[async_trait::async_trait]
                impl Runnable<Value, Value> for StrParserAdapter {
                    async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
                        // Check string directly first to avoid clone if possible, but as_str borrows.
                        if let Some(s) = input.as_str() {
                             let out = self.inner.invoke(s.to_string()).await?;
                             return Ok(Value::String(out));
                        }
                        
                        // Try LlmResponse
                        if let Ok(resp) = serde_json::from_value::<crate::LlmResponse>(input.clone()) {
                             let out = self.inner.invoke(resp).await?;
                             return Ok(Value::String(out));
                        } 
                        
                        // Try String (consumes input)
                        if let Ok(s) = serde_json::from_value::<String>(input) {
                             let out = self.inner.invoke(s).await?;
                             return Ok(Value::String(out));
                        } 
                        
                        Err(WesichainError::Custom("Invalid input for StrOutputParser".into()))
                    }

                    fn stream<'a>(
                        &'a self,
                        input: Value,
                    ) -> futures::stream::BoxStream<'a, Result<crate::StreamEvent, WesichainError>>
                    {
                        if let Some(s) = input.as_str() {
                             return self.inner.stream(s.to_string());
                        }

                        if let Ok(resp) = serde_json::from_value::<crate::LlmResponse>(input.clone()) {
                             return self.inner.stream(resp);
                        } 

                        if let Ok(s) = serde_json::from_value::<String>(input) {
                             return self.inner.stream(s);
                        } 

                        futures::stream::once(async { Err(WesichainError::Custom("Invalid input for StrOutputParser".into())) }).boxed()
                    }
                    
                    fn to_serializable(&self) -> Option<SerializableRunnable> {
                        Some(SerializableRunnable::Parser { kind: "str".to_string(), target_type: None })
                    }
                }
                
                Ok(Arc::new(RuntimeChainAdapter {
                    inner: crate::chain::RuntimeChain::new(vec![Arc::new(StrParserAdapter { inner: crate::StrOutputParser })]),
                    _marker: PhantomData,
                }))
            } else {
                Err(WesichainError::Custom("Unknown parser".to_string()))
            }
        }
        SerializableRunnable::Chain { steps } => {
            if steps.is_empty() {
                return Err(WesichainError::Custom("Empty chain".to_string()));
            }

            let mut runtime_steps: Vec<Arc<dyn Runnable<Value, Value>>> = Vec::new();
            for step in steps {
                let runnable = reconstruct(step, registry)?;
                runtime_steps.push(runnable as Arc<dyn Runnable<Value, Value>>);
            }

            let chain = crate::chain::RuntimeChain::new(runtime_steps);
            let adapter = RuntimeChainAdapter {
                inner: chain,
                _marker: PhantomData,
            };
            Ok(Arc::new(adapter))
        }
        SerializableRunnable::Llm { model, params } => {
            if let Some(reg) = registry {
                let runnable = reg.lookup_llm(&model, params)?;

                struct LlmAdapter {
                    inner: Arc<dyn Runnable<crate::LlmRequest, crate::LlmResponse>>,
                }

                #[async_trait::async_trait]
                impl Runnable<Value, Value> for LlmAdapter {
                    async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
                        let req: crate::LlmRequest = serde_json::from_value(input)?;
                        let res = self.inner.invoke(req).await?;
                        Ok(serde_json::to_value(res)?)
                    }

                    fn stream<'a>(
                        &'a self,
                        input: Value,
                    ) -> futures::stream::BoxStream<'a, Result<crate::StreamEvent, WesichainError>>
                    {
                        match serde_json::from_value::<crate::LlmRequest>(input) {
                            Ok(req) => self.inner.stream(req),
                            Err(_) => futures::stream::empty().boxed(),
                        }
                    }

                    fn to_serializable(&self) -> Option<SerializableRunnable> {
                        self.inner.to_serializable()
                    }
                }

                let adapter = LlmAdapter { inner: runnable };

                let step_scheduler = Arc::new(adapter);
                let chain = crate::chain::RuntimeChain::new(vec![step_scheduler]);
                let final_adapter = RuntimeChainAdapter {
                    inner: chain,
                    _marker: PhantomData,
                };
                Ok(Arc::new(final_adapter))
            } else {
                Err(WesichainError::Custom(
                    "Registry required for LLM reconstruction".to_string(),
                ))
            }
        }
        SerializableRunnable::Passthrough => {
            struct RuntimePassthrough;
            #[async_trait::async_trait]
            impl Runnable<Value, Value> for RuntimePassthrough {
                async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
                    Ok(input)
                }
                fn stream<'a>(
                    &'a self,
                    input: Value,
                ) -> futures::stream::BoxStream<'a, Result<crate::StreamEvent, WesichainError>>
                {
                    let s = serde_json::to_string(&input).unwrap_or_default();
                    futures::stream::once(async move { Ok(crate::StreamEvent::FinalAnswer(s)) })
                        .boxed()
                }
                fn to_serializable(&self) -> Option<SerializableRunnable> {
                    Some(SerializableRunnable::Passthrough)
                }
            }
            Ok(Arc::new(RuntimeChainAdapter {
                inner: crate::chain::RuntimeChain::new(vec![Arc::new(RuntimePassthrough)]),
                _marker: PhantomData,
            }))
        }
        SerializableRunnable::Parallel { steps } => {
            let mut runtime_steps = BTreeMap::new();
            for (key, val) in steps {
                let runnable: Arc<dyn Runnable<Value, Value> + Send + Sync> = reconstruct(val, registry)?;
                runtime_steps.insert(key, runnable);
            }
            let parallel = crate::RunnableParallel::new(runtime_steps);
             
             struct ParallelWrapper { inner: crate::RunnableParallel<Value, Value> }
             
             #[async_trait::async_trait]
             impl Runnable<Value, Value> for ParallelWrapper {
                 async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
                     let map = self.inner.invoke(input).await?;
                     let json_map: serde_json::Map<String, Value> = map.into_iter().collect();
                     Ok(Value::Object(json_map))
                 }
                 fn stream<'a>(&'a self, input: Value) -> futures::stream::BoxStream<'a, Result<crate::StreamEvent, WesichainError>> {
                     self.inner.stream(input)
                 }
                 fn to_serializable(&self) -> Option<SerializableRunnable> {
                     self.inner.to_serializable()
                 }
             }

             Ok(Arc::new(RuntimeChainAdapter {
                 inner: crate::chain::RuntimeChain::new(vec![Arc::new(ParallelWrapper { inner: parallel })]),
                 _marker: PhantomData,
            }))
        }
        SerializableRunnable::Fallbacks { primary, fallbacks } => {
            let primary_runnable: Arc<dyn Runnable<Value, Value> + Send + Sync> =
                reconstruct(*primary, registry)?;
            let mut fallback_runnables = Vec::new();
            for fb in fallbacks {
                let runnable: Arc<dyn Runnable<Value, Value> + Send + Sync> = reconstruct(fb, registry)?;
                fallback_runnables.push(runnable);
            }

            let with_fallbacks =
                crate::RunnableWithFallbacks::new(primary_runnable, fallback_runnables);
            Ok(Arc::new(RuntimeChainAdapter {
                inner: crate::chain::RuntimeChain::new(vec![Arc::new(with_fallbacks)]),
                _marker: PhantomData,
            }))
        }
        SerializableRunnable::Prompt {
            template,
            input_variables,
        } => {
            if let Some(reg) = registry {
                let runnable = reg.lookup_prompt(&template, input_variables)?;
                Ok(Arc::new(RuntimeChainAdapter {
                    inner: crate::chain::RuntimeChain::new(vec![runnable]),
                    _marker: PhantomData,
                }))
            } else {
                Err(WesichainError::Custom(
                    "Registry required for Prompt reconstruction".to_string(),
                ))
            }
        }
        SerializableRunnable::Tool {
            name,
            schema: _,
            description: _,
        } => {
            if let Some(reg) = registry {
                let tool = reg.lookup_tool(&name, serde_json::json!({}))?;
                struct ToolWrapper(Arc<dyn crate::Tool>);
                #[async_trait::async_trait]
                impl Runnable<Value, Value> for ToolWrapper {
                    async fn invoke(&self, input: Value) -> Result<Value, WesichainError> {
                        self.0
                            .invoke(input)
                            .await
                            .map_err(|e| WesichainError::Custom(e.to_string()))
                    }
                    fn stream<'a>(
                        &'a self,
                        input: Value,
                    ) -> futures::stream::BoxStream<'a, Result<crate::StreamEvent, WesichainError>>
                    {
                        futures::stream::once(async move {
                            let res = self.invoke(input).await?;
                            let s = serde_json::to_string(&res).unwrap_or_default();
                            Ok(crate::StreamEvent::FinalAnswer(s))
                        })
                        .boxed()
                    }
                    fn to_serializable(&self) -> Option<SerializableRunnable> {
                        Some(SerializableRunnable::Tool {
                            name: self.0.name().to_string(),
                            description: Some(self.0.description().to_string()),
                            schema: Some(self.0.schema()),
                        })
                    }
                }

                let wrapper = ToolWrapper(tool);
                let step = Arc::new(wrapper);
                let chain = crate::chain::RuntimeChain::new(vec![step]);
                let adapter = RuntimeChainAdapter {
                    inner: chain,
                    _marker: PhantomData,
                };
                Ok(Arc::new(adapter))
            } else {
                Err(WesichainError::Custom(
                    "Registry required for Tool reconstruction".to_string(),
                ))
            }
        }
    }
}

// Helper to load specific known types for testing
pub fn load_str_parser(path: impl AsRef<Path>) -> Result<StrOutputParser, WesichainError> {
    let content = fs::read_to_string(path)
        .map_err(|e| WesichainError::Custom(format!("Failed to read file: {}", e)))?;
    let ser: SerializableRunnable =
        serde_json::from_str(&content).map_err(WesichainError::Serde)?;
    if let SerializableRunnable::Parser { kind, .. } = ser {
        if kind == "str" {
            Ok(StrOutputParser)
        } else {
            Err(WesichainError::Custom("Not a str parser".to_string()))
        }
    } else {
        Err(WesichainError::Custom("Not a parser".to_string()))
    }
}

pub fn load_json_parser<T>(path: impl AsRef<Path>) -> Result<JsonOutputParser<T>, WesichainError> {
    let content = fs::read_to_string(path)
        .map_err(|e| WesichainError::Custom(format!("Failed to read file: {}", e)))?;
    let ser: SerializableRunnable =
        serde_json::from_str(&content).map_err(WesichainError::Serde)?;
    if let SerializableRunnable::Parser { kind, .. } = ser {
        if kind == "json" {
            Ok(JsonOutputParser::new())
        } else {
            Err(WesichainError::Custom("Not a json parser".to_string()))
        }
    } else {
        Err(WesichainError::Custom("Not a parser".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output_parsers::StrOutputParser;
    use crate::{LlmResponse, ToolCall};
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_load_str_parser() {
        let parser = StrOutputParser;
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        save_runnable::<LlmResponse, String>(path, &parser).unwrap();

        let loaded = load_str_parser(path).unwrap();

        // Verify loaded works (mock invoke not easy here without async runtime in test, but deserialization success is key)
        // We can inspect the file content to verify tag
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("\"type\": \"parser\""));
        assert!(content.contains("\"kind\": \"str\""));
    }
}
