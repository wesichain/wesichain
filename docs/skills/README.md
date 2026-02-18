# Wesichain Skills Documentation

This directory contains AI-friendly documentation for building LLM applications with Wesichain.

## For AI Agents

If you are an AI agent working with this codebase, read these files in order:

1. **AGENTS.md** - Universal rules (MUST read first)
2. **core-concepts.md** - Foundation traits and patterns
3. **rag-pipelines.md** - Retrieval-augmented generation
4. **react-agents.md** - ReAct agents with tools

## How to Use These Files

- **Quick reference**: Jump to "Quick Reference" section for syntax and API overview
- **Code patterns**: Copy from "Code Patterns" sections for implementation templates
- **Vibe coding**: Use prompts from "Vibe Coding Prompts" sections to guide implementation
- **Debug errors**: Check "Common Errors" sections for solutions to known issues

## When to Use Each Skill

| Use Case | Start With |
|----------|------------|
| Basic chain/pipeline | core-concepts.md |
| Document Q&A, search | rag-pipelines.md |
| Tool-using agents | react-agents.md |
| Complex workflows | stateful-graphs.md (coming in phase 2) |
| Persistence | checkpointing.md (coming in phase 2) |

## Phase 2 (Coming Soon)

The following skills documentation is planned for phase 2:

- **stateful-graphs.md** - LangGraph-style nodes and edges
- **checkpointing.md** - Persistence and resumability
- **memory.md** - Conversation and vector memory
- **llm-integrations.md** - Provider-specific patterns
- **custom-tools.md** - Implementing the Tool trait
- **streaming.md** - StreamEvent patterns
- **best-practices.md** - Rust idioms and gotchas
- **migration-guide.md** - Python LangChain → Rust
