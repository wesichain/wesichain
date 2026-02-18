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

# Tier 1: Core & Foundation
publish_crate "wesichain-macros"
publish_crate "wesichain-core"
publish_crate "wesichain-compat"

# Tier 2: Middleware & Utilities
publish_crate "wesichain-prompt"
publish_crate "wesichain-embeddings"
publish_crate "wesichain-memory"
publish_crate "wesichain-llm"

# Tier 3: Functional Logic
publish_crate "wesichain-retrieval"
publish_crate "wesichain-qdrant"
publish_crate "wesichain-pinecone"
publish_crate "wesichain-weaviate"
publish_crate "wesichain-chroma"

# Tier 4: Graph & Integrations
publish_crate "wesichain-graph"
publish_crate "wesichain-checkpoint-sql"
publish_crate "wesichain-checkpoint-sqlite"
publish_crate "wesichain-checkpoint-postgres"
publish_crate "wesichain-checkpoint-redis"
publish_crate "wesichain-langsmith"
publish_crate "wesichain-rag"

# Tier 5: Top-Level
publish_crate "wesichain"

echo -e "${GREEN}All crates processed successfully!${NC}"
