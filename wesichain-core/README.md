# Wesichain Core

Core abstractions for building LLM applications with migration-friendly primitives.

## Features

### Advanced Runnable Patterns

#### Parallel Execution (`RunnableParallel`)
Run multiple branches concurrently and collect results in a map.

```rust
use std::collections::BTreeMap;
use std::sync::Arc;
use wesichain_core::{RunnableParallel, Runnable};

let parallel = RunnableParallel::new(BTreeMap::from([
    ("branch1".to_string(), Arc::new(runnable1)),
    ("branch2".to_string(), Arc::new(runnable2)),
]));
let result = parallel.invoke(input).await?;
```

#### Fallbacks (`with_fallbacks`)
Add resilience by specifying fallback runnables to try upon failure.

```rust
use wesichain_core::RunnableExt;

let robust_chain = unreliable_chain.with_fallbacks(vec![
    Arc::new(backup_chain)
]);
```

#### Structured Output (`with_structured_output`)
Easily extract structured data (JSON) from LLM responses using schema binding.

```rust
use wesichain_core::ToolCallingLlmExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Deserialize, Serialize)]
struct Person {
    name: String,
    age: u8,
}

let extractor = llm.with_structured_output::<Person>();
let person: Person = extractor.invoke(request).await?;
```

### Serialization (`SerializableRunnable`)
Represent chain structures as JSON for saving and loading.

```rust
use wesichain_core::SerializableRunnable;

let json = chain.to_json()?;
let loaded = SerializableRunnable::from_json(&json)?;
```
