# Wesichain Roadmap

This document tracks major features in active design or planned for future releases.

## In Active Design

### OpenTelemetry Support
- **Status**: Design phase
- **Scope**: OTel SDK integration with span export, W3C traceparent propagation for distributed tracing
- **Blockers**: Trait unification (see `docs/plans/2025-02-17-observability-tracing-design.md`) must stabilize first
- **Target**: Post-v0.3, no committed timeline

## Recently Completed

- v0.2.0: Graph execution with persistence, checkpointing (SQLite, Postgres, Redis), parallel node execution

## Under Consideration

### Langfuse Integration
- **Status**: Evaluating demand
- **Notes**: Open-source alternative to LangSmith. Lower priority than OTel; will revisit after trait unification
