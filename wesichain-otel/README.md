# wesichain-otel

OpenTelemetry tracing integration for Wesichain with correct span parenting and W3C traceparent propagation.

Part of the [wesichain](https://github.com/wesichain/wesichain) LLM agent framework.

## Installation

```toml
[dependencies]
wesichain-otel = "0.3"
```

## Quick Start

```rust
use wesichain_otel::{OtelHandler, init_tracer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize OTLP exporter (reads OTEL_EXPORTER_OTLP_ENDPOINT)
    let _tracer_provider = init_tracer("my-agent")?;

    let handler = OtelHandler::new("my-agent");
    // chain.with_callback(handler).invoke(input).await?

    Ok(())
}
```

## Features

- **W3C traceparent** — correct span context propagation across async boundaries
- **OTLP export** — sends traces to any OpenTelemetry Collector via gRPC
- **Span attributes** — LLM model, token counts, latency, and error status
- **Nested spans** — chain steps and tool calls appear as child spans
- **Tokio-aware** — span context preserved across `.await` points

## Configuration

| Env var | Default | Description |
|---------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP gRPC endpoint |
| `OTEL_SERVICE_NAME` | binary name | Service name in traces |

## License

Apache-2.0 OR MIT
