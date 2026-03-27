---
name: wesichain-prompt
description: |
  Prompt templating for Wesichain: variable substitution, multi-message chat
  templates, YAML loading, and prompt organization with PromptHub. Use when
  creating LLM prompts with dynamic content in Rust.
triggers:
  - "prompt"
  - "PromptTemplate"
  - "ChatPromptTemplate"
  - "MessagePromptTemplate"
  - "PromptHub"
  - "LocalPromptHub"
  - "{{"
  - "placeholder"
  - "wesichain-prompt"
---

## When to Use

Use wesichain-prompt when you need to:
- Create prompts with variable substitution using `{{name}}` syntax
- Build multi-turn chat prompts with System, User, and Assistant roles
- Load prompts from YAML files for hot-reload capability
- Organize prompts by name and version in a PromptHub
- Inject dynamic message lists with placeholders

## Quick Start

```rust
use wesichain_prompt::{PromptTemplate, ChatPromptTemplate, MessagePromptTemplate};
use std::collections::HashMap;
use wesichain_core::Value;

// Simple template with variable substitution
let template = PromptTemplate::new("Hello {{name}}!".to_string());
let mut vars = HashMap::new();
vars.insert("name".to_string(), Value::from("World"));
let result = template.render(&vars)?;
// Result: "Hello World!"
```

## Key Patterns

### Pattern 1: Basic Variable Substitution with PromptTemplate

```rust
use wesichain_prompt::PromptTemplate;
use std::collections::HashMap;
use wesichain_core::Value;

// Create template with {{variable}} syntax
let template = PromptTemplate::new(
    "Summarize the following in {{language}}:\n\n{{text}}".to_string()
);

// Prepare variables
let mut vars = HashMap::new();
vars.insert("language".to_string(), Value::from("English"));
vars.insert("text".to_string(), Value::from("Long document text here..."));

// Render to string
let prompt = template.render(&vars)?;
```

**Notes:**
- Variables use double braces: `{{name}}`
- Missing variables are left as-is (not replaced)
- Non-string values are converted to their JSON representation
- Keys can contain dots: `{{user.name}}`

### Pattern 2: Chat Prompts with Multiple Messages

```rust
use wesichain_prompt::{ChatPromptTemplate, MessagePromptTemplate};
use wesichain_core::{Message, Role};
use std::collections::HashMap;

// Build multi-message template
let chat_template = ChatPromptTemplate::new(vec![
    MessagePromptTemplate::system("You are a helpful assistant."),
    MessagePromptTemplate::human("What is {{topic}}?"),
    MessagePromptTemplate::ai("I can help with {{topic}}."),
]);

// Format with variables
let mut vars = HashMap::new();
vars.insert("topic".to_string(), serde_json::json!("Rust programming"));

let messages: Vec<Message> = chat_template.format_messages(&vars)?;
// messages[0]: role=System, content="You are a helpful assistant."
// messages[1]: role=User, content="What is Rust programming?"
// messages[2]: role=Assistant, content="I can help with Rust programming."
```

**Message Types:**
- `MessagePromptTemplate::system()` - System instructions
- `MessagePromptTemplate::human()` / `user()` - User messages
- `MessagePromptTemplate::ai()` / `assistant()` - Assistant messages
- `MessagePromptTemplate::placeholder()` - Inject message lists dynamically

### Pattern 3: Loading Prompts from YAML Files

**Requires:** `cargo add wesichain-prompt --features yaml`

**Chat prompt YAML format:**
```yaml
# prompts/qa.yaml
messages:
  - role: system
    content: "You are a {{persona}} assistant."
  - role: human
    content: "{{question}}"
```

**Simple template YAML format:**
```yaml
# prompts/summarize.yaml
template: "Summarize this in {{style}}: {{text}}"
```

**Loading in code:**
```rust
use wesichain_prompt::{load_chat_prompt, load_prompt_template};
use std::path::Path;

// Load chat prompt
let chat_template = load_chat_prompt(Path::new("prompts/qa.yaml"))?;

// Load simple template
let template = load_prompt_template(Path::new("prompts/summarize.yaml"))?;

// Use like any other template
let messages = chat_template.format_messages(&vars)?;
```

**Supported roles in YAML:** `system`, `human`, `user`, `ai`, `assistant`, `placeholder`

### Pattern 4: Building Dynamic Prompts with Placeholders

```rust
use wesichain_prompt::{ChatPromptTemplate, MessagePromptTemplate};
use wesichain_core::Message;
use std::collections::HashMap;

// Template with placeholder for dynamic message injection
let template = ChatPromptTemplate::new(vec![
    MessagePromptTemplate::system("You are a helpful assistant."),
    MessagePromptTemplate::placeholder("history"),  // Inject conversation history
    MessagePromptTemplate::human("{{current_question}}"),
]);

// Prepare variables with message list
let mut vars = HashMap::new();
vars.insert("current_question".to_string(), serde_json::json!("What about Rust?"));
vars.insert(
    "history".to_string(),
    serde_json::json!([
        { "role": "user", "content": "Tell me about programming languages." },
        { "role": "assistant", "content": "Programming languages include Python, JavaScript, and Rust." }
    ])
);

let messages = template.format_messages(&vars)?;
// Results in 4 messages: system + 2 history + current question
```

**Placeholder behavior:**
- Expects a JSON array of Message objects
- Single Message object also accepted
- If missing or invalid, results in empty message list
- Useful for injecting conversation history dynamically

### Pattern 5: Using PromptHub for Organization

**Requires:** `cargo add wesichain-prompt --features yaml`

```rust
use wesichain_prompt::{LocalPromptHub, PromptHub};

// Create hub from directory of YAML files
let hub = LocalPromptHub::from_dir("./prompts")?;

// Load prompt by name (gets latest version)
let entry = hub.load("summarize", None)?;

// Or load specific version
let entry = hub.load("summarize", Some("2.0"))?;

// Access the template
match &entry.kind {
    PromptKind::Chat(chat_template) => {
        let messages = chat_template.format_messages(&vars)?;
    }
    PromptKind::Simple(template) => {
        let text = template.render(&vars)?;
    }
}
```

**File naming convention:**
- `name.yaml` - Registers as `name@latest`
- `name@version.yaml` - Registers as `name@version` (e.g., `summarize@2.0.yaml`)

```
prompts/
├── summarize.yaml          # summarize@latest
├── summarize@1.0.yaml      # summarize@1.0
├── summarize@2.0.yaml      # summarize@2.0
└── qa.yaml                 # qa@latest
```

## Golden Rules

1. **Variables use double braces** - Use `{{name}}` not `{name}` or `${name}`
2. **Use HashMap<String, Value> for variables** - Required for both PromptTemplate and ChatPromptTemplate
3. **Enable "yaml" feature for file loading** - Required for load_chat_prompt, load_prompt_template, and PromptHub
4. **ChatPromptTemplate implements Runnable** - Can be chained with .then() in pipelines
5. **Placeholder expects Message JSON** - Pass serde_json::Value array of Message objects
6. **Missing variables remain unchanged** - Not an error, just leaves the `{{var}}` as-is
7. **Use LocalPromptHub::from_dir() for organization** - Hot-reload capable, versioned prompts

## Common Mistakes

- **Wrong variable syntax** - Using `{name}` or `${name}` instead of `{{name}}`
- **Forgetting yaml feature** - File loading and PromptHub require `--features yaml`
- **Wrong variable type** - Must use `HashMap<String, Value>` not `HashMap<String, String>`
- **Wrong YAML structure** - Chat prompts need `messages:` list, simple prompts need `template:` key
- **Invalid role names** - Use `human`/`user` and `ai`/`assistant`, not arbitrary strings
- **Not checking PromptKind** - Hub returns PromptKind enum; use `as_chat()` or `as_simple()` to access
- **Wrong file naming** - Use `@` for versions: `prompt@1.0.yaml`, not `prompt_v1.0.yaml`

## Resources

- Crate: `wesichain-prompt`
- Feature flags: `yaml` (for file loading and PromptHub)
- Key types: `PromptTemplate`, `ChatPromptTemplate`, `MessagePromptTemplate`, `LocalPromptHub`, `PromptHub`
- Key functions: `load_chat_prompt()`, `load_prompt_template()`
- Message roles: `system`, `human`/`user`, `ai`/`assistant`, `placeholder`
