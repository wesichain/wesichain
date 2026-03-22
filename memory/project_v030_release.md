---
name: v0.3.0 release preparation
description: v0.3.0 release — version bump, metadata, READMEs, CHANGELOG, git tag (not yet pushed)
type: project
---

All pre-publish steps for v0.3.0 are done as of 2026-03-22.

**Why:** All sprint 1–7 work needed packaging for crates.io after 9 new crates were added post-v0.2.1.

**What was done:**
- Version bumped 0.2.1 → 0.3.0 in workspace `Cargo.toml` and all 29 crate cross-references
- Added `keywords`, `categories`, `readme` fields to all 29 publishable crate `Cargo.toml` files
- Created `README.md` for 9 new crates: anthropic, tools, mcp, session, server, cli, langfuse, otel, rag
- Created `wesichain/README.md` for the facade crate
- Updated `CHANGELOG.md` with `[0.3.0] - 2026-03-22` section
- Created `v0.3.0` annotated git tag
- Updated `publish_all.sh` with correct 29-crate dependency order

**How to apply:** When user asks about publishing, the workspace is ready. Next step is:
```bash
git push origin main --tags
./publish_all.sh --execute
```
Publish order is defined in `publish_all.sh` (10 tiers, bottom-up).
