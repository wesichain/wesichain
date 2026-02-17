# Wesichain Chroma Crates Release Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Publish `wesichain-chroma` to crates.io safely after PR merge, with reproducible verification and clear post-release validation.

**Architecture:** Keep release risk low by gating on CI, then running local dry-run packaging and publish commands from the crate directory. Follow with docs/announcement updates in a separate docs-only commit so rollback and hotfix operations remain simple.

**Tech Stack:** Cargo workspaces, crates.io publish flow, GitHub PR checks, docs.rs, GitHub Releases.

---

### Task 1: Gate on PR and CI before publishing

**Files:**
- Verify only: `https://github.com/wesichain/wesichain/pull/21`

**Step 1: Confirm PR is open and targeting `main`**

Run: `gh pr view 21 --json state,baseRefName,headRefName,url`
Expected: `state=OPEN`, `baseRefName=main`, `headRefName=feat/chroma-adapter`

**Step 2: Confirm required checks are green**

Run: `gh pr checks 21`
Expected: all checks pass, including `chroma-contract`

**Step 3: If checks fail, stop release and fix first**

Run: `gh pr checks 21 --watch`
Expected: do not proceed until all checks are successful

**Step 4: Merge PR with project-preferred strategy**

Run (squash example): `gh pr merge 21 --squash --delete-branch`
Expected: PR merged, remote feature branch deleted

**Step 5: Commit (if policy requires local merge metadata update)**

No code commit expected in this task.

### Task 2: Sync local main and verify release content

**Files:**
- Verify: `Cargo.toml`
- Verify: `wesichain-chroma/Cargo.toml`
- Verify: `wesichain-chroma/src/lib.rs`
- Verify: `wesichain-chroma/MIGRATION.md`

**Step 1: Update local `main`**

Run: `git checkout main && git pull --ff-only`
Expected: local `main` matches remote

**Step 2: Verify workspace includes `wesichain-chroma`**

Run: `cargo metadata --no-deps --format-version 1`
Expected: package list contains `wesichain-chroma`

**Step 3: Verify release tests pass on merged `main`**

Run: `cargo test -p wesichain-chroma -q`
Expected: all `wesichain-chroma` tests pass

**Step 4: Verify retrieval regression remains green**

Run: `cargo test -p wesichain-retrieval async_loader --features pdf -q`
Expected: async loader tests pass including PDF route test

**Step 5: Commit**

No code commit expected in this task.

### Task 3: Run packaging and dry-run publish checks

**Files:**
- Verify: `wesichain-chroma/Cargo.toml`
- Verify: `wesichain-chroma/`

**Step 1: Validate package contents**

Run: `cargo package -p wesichain-chroma --list`
Expected: includes `src/lib.rs`, tests, example, migration docs as intended

**Step 2: Run dry-run publish from workspace**

Run: `cargo publish -p wesichain-chroma --dry-run`
Expected: successful package build and publish simulation

**Step 3: Verify no blocked dependencies/version mismatches**

Run: `cargo tree -p wesichain-chroma`
Expected: all path dependencies resolve to published-compatible versions

**Step 4: Re-run lint gate before actual publish**

Run: `cargo clippy -p wesichain-chroma --tests --all-features -- -D warnings`
Expected: clean output

**Step 5: Commit**

No code commit expected in this task.

### Task 4: Publish to crates.io and verify availability

**Files:**
- Publish target: `wesichain-chroma/Cargo.toml`

**Step 1: Publish crate**

Run: `cargo publish -p wesichain-chroma`
Expected: crates.io accepts publish

**Step 2: Confirm crate appears on crates.io**

Run: `cargo search wesichain-chroma --limit 1`
Expected: version appears in index after propagation delay

**Step 3: Confirm docs.rs build starts/finishes**

Run: `gh api -X GET https://docs.rs/crate/wesichain-chroma`
Expected: docs page resolves (may take several minutes)

**Step 4: Record published version and timestamp**

Run: `cargo info wesichain-chroma`
Expected: published version metadata visible

**Step 5: Commit**

No code commit expected in this task.

### Task 5: Post-release docs and announcement follow-up

**Files:**
- Modify: `README.md` (crate matrix row for `wesichain-chroma`)
- Modify: `CHANGELOG.md` (release note entry)

**Step 1: Write failing docs check (optional if docs linter exists)**

Run: `cargo test --doc -p wesichain-chroma`
Expected: pass (or fail before docs adjustments if examples changed)

**Step 2: Add README crate table row for `wesichain-chroma`**

Update table with crates.io/docs.rs links.

**Step 3: Add changelog entry for Chroma adapter release**

Include migration significance and zero-core-change architecture note.

**Step 4: Run formatting/linting checks impacted by docs changes**

Run: `cargo fmt --all -- --check`
Expected: no formatting regressions

**Step 5: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs(chroma): add release notes and crate links"
```
