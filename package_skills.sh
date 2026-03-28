#!/bin/bash
# Package Wesichain skills for distribution
# Generates adapters for: Claude Code, Cursor, Windsurf, GitHub Copilot, Continue.dev, Aider, OpenCode
#
# Usage: ./package_skills.sh [output_file]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_DIR="$SCRIPT_DIR/.claude/skills"
OUTPUT_FILE="${1:-wesichain.skills}"
VERSION="0.4.0"

echo "Packaging Wesichain skills..."
echo "   Source: $SKILLS_DIR"
echo "   Output: $OUTPUT_FILE"
echo ""

# Create temporary directory
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# Create adapter subdirectories (_adapters/ prefix is ignored by Claude Code's skill loader)
mkdir -p \
  "$TMP_DIR/_adapters/cursor" \
  "$TMP_DIR/_adapters/windsurf" \
  "$TMP_DIR/_adapters/copilot" \
  "$TMP_DIR/_adapters/continue" \
  "$TMP_DIR/_adapters/aider" \
  "$TMP_DIR/_adapters/opencode"

# ── Frontmatter helpers ───────────────────────────────────────────────────────

get_name() {
  awk '/^---$/{if(++c==2)exit} c==1 && /^name: /{sub(/^name: */,""); print; exit}' "$1"
}

# Returns description as a single space-separated line (collapses block scalars)
get_description() {
  awk '
    BEGIN{c=0; in_block=0; in_single=0}
    /^---$/{c++; if(c==2)exit; next}
    c==1 && /^description: \|$/{in_block=1; next}
    c==1 && /^description: /{sub(/^description: /,""); printf "%s ", $0; in_single=1; next}
    in_block && /^  /{sub(/^  /,""); printf "%s ", $0; next}
    in_block{exit}
    in_single{exit}
  ' "$1" | sed 's/ *$//'
}

# Returns everything after the closing ---
get_body() {
  awk '/^---$/{if(++c==2){found=1; next}} found{print}' "$1"
}

# ── Initialize single-file outputs ───────────────────────────────────────────

cat > "$TMP_DIR/_adapters/copilot/copilot-instructions.md" << 'HEADER'
# Wesichain Framework Reference

This file provides Wesichain framework knowledge for GitHub Copilot.
Wesichain is a Rust-native LLM agent framework — follow these patterns when writing Wesichain code.
HEADER

cat > "$TMP_DIR/_adapters/aider/CONVENTIONS.md" << 'HEADER'
# Wesichain Conventions

Wesichain is a Rust-native LLM framework. Follow these patterns and golden rules when writing Wesichain code.
HEADER

# ── Process each skill ───────────────────────────────────────────────────────

SKILL_COUNT=0

for skill_dir in "$SKILLS_DIR"/*/; do
  skill_file="$skill_dir/SKILL.md"
  [[ -f "$skill_file" ]] || continue

  name="$(get_name "$skill_file")"
  desc="$(get_description "$skill_file")"
  body="$(get_body "$skill_file")"

  # Claude Code — copy folder verbatim at root level (unchanged format)
  cp -r "$skill_dir" "$TMP_DIR/$name"

  # OpenCode — copy folder verbatim to .opencode/skills/ (native format)
  mkdir -p "$TMP_DIR/_adapters/opencode/.opencode/skills/${name}"
  cp -r "$skill_dir" "$TMP_DIR/_adapters/opencode/.opencode/skills/${name}"

  # Cursor — .mdc with YAML frontmatter (description collapsed to one line)
  {
    printf -- '---\n'
    printf 'description: %s\n' "$desc"
    printf 'alwaysApply: false\n'
    printf -- '---\n\n'
    printf '%s\n' "$body"
  } > "$TMP_DIR/_adapters/cursor/${name}.mdc"

  # Windsurf — plain markdown: h1 heading + description paragraph + body
  {
    printf '# %s\n\n' "$name"
    printf '%s\n\n' "$desc"
    printf '%s\n' "$body"
  } > "$TMP_DIR/_adapters/windsurf/${name}.md"

  # Continue.dev — frontmatter with name + globs for *.rs files, then body
  {
    printf -- '---\n'
    printf 'name: %s\n' "$name"
    printf 'globs: "*.rs"\n'
    printf -- '---\n\n'
    printf '%s\n' "$body"
  } > "$TMP_DIR/_adapters/continue/${name}.md"

  # GitHub Copilot — append each skill to the single concatenated file
  {
    printf '\n---\n\n'
    printf '## %s\n\n' "$name"
    printf '%s\n\n' "$desc"
    printf '%s\n' "$body"
  } >> "$TMP_DIR/_adapters/copilot/copilot-instructions.md"

  # Aider — append each skill to the single concatenated CONVENTIONS.md
  {
    printf '\n---\n\n'
    printf '## %s\n\n' "$name"
    printf '%s\n\n' "$desc"
    printf '%s\n' "$body"
  } >> "$TMP_DIR/_adapters/aider/CONVENTIONS.md"

  SKILL_COUNT=$((SKILL_COUNT + 1))
  echo "  + $name"
done

# ── Metadata ─────────────────────────────────────────────────────────────────

cat > "$TMP_DIR/metadata.json" << EOF
{
  "name": "wesichain",
  "version": "${VERSION}",
  "description": "Wesichain LLM framework skills for AI coding tools",
  "tools": ["claude-code", "cursor", "windsurf", "copilot", "continue", "aider", "opencode"],
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
  "documentation": "https://wesichain.pages.dev"
}
EOF

# ── Pack ──────────────────────────────────────────────────────────────────────

tar -czf "$OUTPUT_FILE" -C "$TMP_DIR" .

echo ""
echo "Packaged ${SKILL_COUNT} skills (7 tool formats) -> $OUTPUT_FILE"
echo ""
echo "  Claude Code    ${SKILL_COUNT} skill folders (root level)"
echo "  Cursor         ${SKILL_COUNT} .mdc files  (_adapters/cursor/)"
echo "  Windsurf       ${SKILL_COUNT} .md files   (_adapters/windsurf/)"
echo "  GitHub Copilot 1 combined file  (_adapters/copilot/copilot-instructions.md)"
echo "  Continue.dev   ${SKILL_COUNT} .md files   (_adapters/continue/)"
echo "  Aider          1 combined file  (_adapters/aider/CONVENTIONS.md)"
echo "  OpenCode       ${SKILL_COUNT} skill folders  (_adapters/opencode/.opencode/skills/)"
echo ""
echo "Install:"
echo "  curl -fsSL https://wesichain.pages.dev/skills.sh | bash"
