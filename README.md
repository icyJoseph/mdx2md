# mdx2md

A Rust-based MDX-to-Markdown converter with a two-layer transform pipeline, available as a library, CLI, and WASM/JS package.

## What it does

Converts MDX files (Markdown + JSX) into clean Markdown by:

1. **Layer 1 (MDX → raw Markdown):** Resolves JSX components via user-defined templates, strips imports/exports, handles expressions.
2. **Layer 2 (Markdown → Markdown):** Applies structural rewrites — tables to bullet lists, relative links/images to absolute URLs.

Both layers are independently configurable and composable.

## Usage

### CLI

```sh
# Single file to stdout
mdx2md input.mdx --config mdx2md.toml

# Single file to output file
mdx2md input.mdx -o output.md --config mdx2md.toml

# Directory of .mdx files
mdx2md docs/ -o out/ --config mdx2md.toml

# Stdin/stdout
cat input.mdx | mdx2md --config mdx2md.toml
```

### Rust library

```rust
use mdx2md_core::config::Config;

let config = Config::from_toml(&std::fs::read_to_string("mdx2md.toml")?)?;
let markdown = mdx2md_core::convert(&mdx_source, &config)?;
```

### WASM / JavaScript

```typescript
import { convert } from "mdx2md";

const md = convert(mdxSource, {
  components: {
    // Template string (simple case)
    Image: "![{alt}]({src})",

    // Callback (full control)
    Callout: ({ type, children }) => `> **${type}**: ${children}`,

    // Catch-all for unknown components
    _default: ({ children }) => children ?? "",
  },
  markdown: {
    tables: "list",
    links: { makeAbsolute: true, baseUrl: "https://docs.example.com" },
  },
});
```

## Configuration (TOML)

```toml
[options]
strip_imports = true
strip_exports = true
expression_handling = "strip"   # "strip" | "preserve_raw" | "placeholder"
preserve_frontmatter = true

[components.Callout]
template = "> **{type}**: {children}"

[components.CodeBlock]
template = "```{language}\n{children}\n```"

[components.Badge]
template = "{label}"

[components._default]
template = "{children}"

[markdown.tables]
format = "list"

[markdown.links]
make_absolute = true
base_url = "https://docs.example.com"

[markdown.images]
make_absolute = true
base_url = "https://cdn.example.com"
```

## Architecture

```
MDX Source
  → MDX Tokenizer (ours)
  → MDX Parser (ours)
  → Layer 1: JSX Transform (ours) — resolves components via templates/callbacks
  → Layer 2: MD Rewriter (ours + pulldown-cmark for element location)
  → Clean Markdown
```

### Dependencies

The dependency footprint is intentionally lean:

| Crate | What we use it for |
|---|---|
| **`pulldown-cmark`** | Layer 2 only. Locates table boundaries via its offset iterator so we know which byte ranges to replace with bullet lists. Not used for MDX parsing. |
| **`serde` + `toml`** | TOML config file deserialization. |
| **`clap`** | CLI argument parsing. |
| **`wasm-bindgen` + `js-sys`** | WASM/JS bridge. `wasm-bindgen` generates JS glue, `js-sys` provides `Reflect` and `Function` for working with plain JS objects and callbacks. |

Everything else — MDX tokenizer, MDX parser, JSX transform engine, link/image URL rewriter — is built from scratch with no external parser dependencies.

## Project structure

```
crates/
  mdx2md-core/    # Library: tokenizer, parser, config, transform, rewriter
  mdx2md-cli/     # Binary: CLI tool
  mdx2md-wasm/    # WASM bindings for JS/TS consumers
```
