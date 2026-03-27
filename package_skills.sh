#!/bin/bash
# Package Wesichain skills for distribution
# Usage: ./package_skills.sh [output_file]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_DIR="$SCRIPT_DIR/.claude/skills"
OUTPUT_FILE="${1:-wesichain.skills}"

echo "📦 Packaging Wesichain skills..."
echo "   Source: $SKILLS_DIR"
echo "   Output: $OUTPUT_FILE"

# Create temporary directory
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# Copy all skill directories
cp -r "$SKILLS_DIR"/* "$TMP_DIR/"

# Create metadata file
cat > "$TMP_DIR/metadata.json" << 'EOF'
{
  "name": "wesichain",
  "version": "0.3.0",
  "description": "Wesichain LLM framework skills for Claude Code",
  "skills": [
    "wesichain-core",
    "wesichain-graph",
    "wesichain-react",
    "wesichain-rag",
    "wesichain-llm",
    "wesichain-memory",
    "wesichain-checkpoint",
    "wesichain-embeddings",
    "wesichain-tools",
    "wesichain-prompt",
    "wesichain-langsmith"
  ],
  "repository": "https://github.com/wesichain/wesichain",
  "documentation": "https://wesichain.vercel.app"
}
EOF

# Create tarball
tar -czf "$OUTPUT_FILE" -C "$TMP_DIR" .

echo "✅ Packaged $(ls -1 "$SKILLS_DIR" | wc -l) skills into $OUTPUT_FILE"
echo ""
echo "To install in Claude Code:"
echo "  1. Drag $OUTPUT_FILE into Claude Code chat"
echo "  2. Or place in ~/.config/claude/skills/"
echo ""
echo "Skills included:"
ls -1 "$SKILLS_DIR"
