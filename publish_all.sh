#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Default to dry-run unless --execute is passed
DRY_RUN=true
if [ "$1" == "--execute" ]; then
    DRY_RUN=false
    echo -e "${RED}WARNING: Executing REAL publish to crates.io${NC}"
    echo "Press Ctrl+C to cancel or Enter to continue in 5 seconds..."
    sleep 5
else
    echo -e "${YELLOW}Running in DRY-RUN mode. Use --execute to publish.${NC}"
fi

# Function to publish a crate
publish_crate() {
    local crate=$1
    echo -e "${GREEN}Processing ${crate}...${NC}"

    if [ "$DRY_RUN" = true ]; then
        cargo publish -p "$crate" --dry-run
    else
        echo "Publishing $crate..."
        cargo publish -p "$crate"
        echo "Waiting 30s for crates.io index propagation..."
        sleep 30
    fi
    echo -e "${GREEN}Success: ${crate}${NC}\n"
}

# Tier 1: Core & Foundation (no wesichain deps)
publish_crate "wesichain-macros"
publish_crate "wesichain-core"

# Tier 2: Providers & Prompt (depend on core only)
publish_crate "wesichain-prompt"
publish_crate "wesichain-embeddings"
publish_crate "wesichain-llm"
publish_crate "wesichain-anthropic"       # new in v0.3

# Tier 3: Checkpoints (depend on core)
publish_crate "wesichain-checkpoint-sql"
publish_crate "wesichain-checkpoint-sqlite"
publish_crate "wesichain-checkpoint-postgres"
publish_crate "wesichain-checkpoint-redis"

# Tier 4: Agent & Memory
publish_crate "wesichain-agent"
publish_crate "wesichain-memory"
publish_crate "wesichain-retrieval"
publish_crate "wesichain-session"         # new in v0.3

# Tier 5: MCP & Tools (depend on agent)
publish_crate "wesichain-mcp"             # new in v0.3
publish_crate "wesichain-tools"           # new in v0.3

# Tier 6: Graph & RAG
publish_crate "wesichain-graph"
publish_crate "wesichain-rag"             # new in v0.3

# Tier 7: Observability
publish_crate "wesichain-otel"            # new in v0.3
publish_crate "wesichain-langsmith"
publish_crate "wesichain-langfuse"        # new in v0.3

# Tier 8: Server & CLI
publish_crate "wesichain-server"          # new in v0.3
publish_crate "wesichain-cli"             # new in v0.3

# Tier 9: Compat & Vector stores
publish_crate "wesichain-compat"
publish_crate "wesichain-pinecone"
publish_crate "wesichain-qdrant"
publish_crate "wesichain-weaviate"
publish_crate "wesichain-chroma"

# Tier 10: Facade (last — depends on everything)
publish_crate "wesichain"

echo -e "${GREEN}All crates processed successfully!${NC}"
