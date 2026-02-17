# Wesichain Documentation Website Design

**Date:** 2026-02-15
**Status:** Approved for Implementation
**Goal:** Marketing-first documentation site for wesichain v0.1.0

## Overview

Create a fast, modern documentation website for Wesichain (Rust LLM agent framework) using Astro with custom components. The site prioritizes developer adoption through a compelling landing page backed by comprehensive documentation.

## Design Philosophy

- **Technical/Minimal aesthetic** — Match Rust ecosystem standards, clean and trustworthy
- **Landing page first** — Convert curiosity into adoption quickly
- **Static-first performance** — Fast loads, great SEO, minimal JavaScript
- **Incremental growth** — Solid foundation for expanding content over time

## Site Structure

```
wesichain.dev/
│
├─ / (Landing Page)
│  ├─ Hero Section
│  ├─ Performance Comparison
│  ├─ Feature Grid
│  ├─ Quick Start Preview
│  └─ CTA (Get Started)
│
├─ /docs (Documentation Hub)
│  │
│  ├─ /getting-started
│  │  ├─ installation
│  │  ├─ quickstart-react
│  │  ├─ quickstart-rag
│  │  └─ quickstart-graph
│  │
│  ├─ /guides
│  │  ├─ architecture-overview
│  │  ├─ crates-explained (the 15 crates)
│  │  ├─ state-management
│  │  └─ tool-calling
│  │
│  ├─ /migration
│  │  └─ from-python (LangChain/LlamaIndex comparison)
│  │
│  ├─ /examples
│  │  ├─ react-agent
│  │  ├─ rag-streaming
│  │  └─ graph-checkpointing
│  │
│  └─ /reference
│     └─ crates (links to docs.rs with context)
│
└─ /benchmarks (performance methodology)
```

## Technical Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| **Framework** | Astro 4.x | Static-first, React islands, perfect for docs |
| **Styling** | Tailwind CSS | Technical/minimal vibe, fast iteration |
| **Search** | Pagefind | Static indexing, zero runtime cost, Rust-powered |
| **Syntax Highlighting** | Shiki | Best Rust support, themeable |
| **Deployment** | Vercel | Zero-config, preview deployments, fast CDN |
| **Content** | MDX + JSON | Flexibility for embedded components |

## Component Inventory

### Layout Components
- `SiteLayout` — Main wrapper with header/footer
- `DocsLayout` — Docs-specific with sidebar
- `TableOfContents` — Right sidebar navigation
- `Breadcrumbs` — Docs path navigation

### Interactive Islands (React)
- `CodeBlock` — Syntax highlighting + copy button
- `CodePlayground` — Embedded Rust Playground
- `PerformanceChart` — Animated comparison table
- `CrateSelector` — "Which crate do I need?" flowchart
- `SearchButton` — Pagefind search trigger (Cmd+K)

### Content Components
- `FeatureCard` — Landing page feature grid
- `BenchmarkTable` — Performance metrics display
- `CalloutBox` — Tips/warnings in docs
- `LinkCard` — Clickable navigation cards

## Content Architecture

```
src/
├─ content/
│  └─ docs/
│     ├─ getting-started/
│     │  ├─ installation.mdx
│     │  ├─ quickstart-react.mdx
│     │  └─ ...
│     └─ guides/
│        └─ architecture-overview.mdx
├─ data/
│  ├─ crates.json          # 15 crates metadata
│  ├─ benchmarks.json      # Performance data
│  └─ navigation.json      # Sidebar structure
└─ examples/
   └─ (fetched from GitHub repo during build)
```

## Hero Messaging

> "Build composable LLM agents in Rust — without Python's GIL battles. 10x faster, 70% less memory."

Primary CTA: "Get Started in 5 Minutes"
Secondary CTA: "View on GitHub"

## Build & Deployment

**Build Process:**
1. Fetch examples from Wesichain GitHub repo
2. Extract code snippets with frontmatter
3. Generate static pages with syntax highlighting
4. Run Pagefind to index all content
5. Deploy to Vercel CDN

**Vercel Configuration:**
```json
{
  "buildCommand": "npm run build",
  "outputDirectory": "dist",
  "framework": "astro"
}
```

## Success Criteria

- [ ] Landing page loads in < 2s on 3G
- [ ] Pagefind search returns results in < 100ms
- [ ] All code blocks have copy functionality
- [ ] Mobile-responsive design
- [ ] Dark theme by default (system preference aware)
- [ ] Lighthouse score 95+ across all categories

## Future Enhancements (Post-v1)

- Interactive code playground (WASM Rust compiler)
- API reference auto-generated from rustdoc
- Version switching for docs
- i18n support
- Community showcase page
