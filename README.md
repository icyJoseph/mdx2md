# mdx2md

Convert MDX (Markdown + JSX) into clean, portable Markdown.

Built from scratch in Rust. Available as a CLI, a Rust library, and a WASM package for JavaScript/TypeScript.

**[Try it in the playground](https://icyjoseph.github.io/mdx2md/)**

## Why

MDX is great for authoring, but the JSX, imports, and expressions make the files useless outside of MDX-aware tooling. If you want to feed your docs to an LLM, publish them on a platform that only speaks Markdown, or just archive them in a portable format, you need to strip the MDX layer away.

Existing tools pull in the entire unified/remark ecosystem. mdx2md takes a different approach: a purpose-built tokenizer and parser with minimal dependencies, no plugin resolution, and a single config file.

## Example

**Input (MDX):**

```mdx
---
title: Docs
---

import { Callout } from './components';

# Getting started

<Callout type="warning">
  Watch out for **breaking changes**.
</Callout>

| Feature | Status |
|---------|--------|
| Auth    | Done   |
| API     | Beta   |
```

**Output (Markdown):**

```markdown
---
title: Docs
---

# Getting started

> **warning**: Watch out for **breaking changes**.

- **Feature**: Auth, **Status**: Done
- **Feature**: API, **Status**: Beta
```

Imports and exports are stripped. JSX components are replaced using configurable templates. Tables are converted to lists. Links and images can be made absolute. All controlled by a single TOML config (CLI/Rust) or a plain JS object (WASM).

## Install

### CLI

```sh
cargo install mdx2md-cli
```

### npm (WASM)

```sh
npm install @icyjoseph/mdx2md
```

### Rust library

```toml
[dependencies]
mdx2md-core = "0.1"
```

## Usage

### CLI

```sh
mdx2md input.mdx --config mdx2md.toml

mdx2md input.mdx -o output.md --config mdx2md.toml

mdx2md docs/ -o out/ --config mdx2md.toml

cat input.mdx | mdx2md --config mdx2md.toml
```

### JavaScript / TypeScript (WASM)

```typescript
import init, { convert } from "@icyjoseph/mdx2md";

await init();

const md = convert(mdxSource, {
  stripImports: true,
  stripExports: true,
  preserveFrontmatter: true,
  expressionHandling: "strip",
  components: {
    Callout: "> **{type}**: {children}",
    CodeBlock: "```{language}\n{children}\n```",
    _default: "{children}",
  },
  markdown: {
    tables: "list",
    links: { makeAbsolute: true, baseUrl: "https://docs.example.com" },
    images: { makeAbsolute: true, baseUrl: "https://cdn.example.com" },
  },
});
```

Component values can be template strings (simple) or callbacks (full control):

```typescript
const md = convert(mdxSource, {
  components: {
    Callout: (props, children) => `> **${props.type}**: ${children}`,
    _default: (_props, children) => children ?? "",
  },
});
```

### Rust library

```rust
use mdx2md_core::config::Config;

let config = Config::from_toml(&std::fs::read_to_string("mdx2md.toml")?)?;
let markdown = mdx2md_core::convert(&mdx_source, &config)?;
```

## Configuration (TOML)

Used by the CLI and the Rust library. The WASM/JS API accepts the same options as a plain object.

````toml
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
````

### Component templates

Templates use `{attribute_name}` placeholders that are replaced with the component's props. `{children}` is replaced with the component's rendered children. `_default` is the catch-all for any component without a specific template.

### Expression handling

- `"strip"`: remove `{expressions}` entirely (default)
- `"preserve_raw"`: keep the raw expression text without braces
- `"placeholder"`: replace with `[expression]`

### Markdown rewrites

- **Tables**: `format = "list"` converts tables to bullet lists with bolded headers
- **Links**: `make_absolute = true` prepends `base_url` to relative hrefs
- **Images**: same as links, for image `src` attributes

## How it works

```
MDX source string
  → Tokenizer ─── splits into MDX-aware tokens (JSX, imports, expressions, markdown)
  → Parser ────── builds a nested AST from the token stream
  → Layer 1 ───── resolves JSX via templates/callbacks, strips imports/exports
  → Layer 2 ───── rewrites markdown elements (tables, links, images) in-place
  → Clean Markdown
```

The tokenizer and parser are built from scratch with no dependency on remark, unified, or any MDX/JSX parser. Layer 2 uses `pulldown-cmark` only to *locate* elements by byte offset, then performs surgical string replacements to preserve formatting in untouched sections.

### Dependencies

| Crate | Purpose |
|---|---|
| `pulldown-cmark` | Layer 2: locates tables/links/images by byte offset |
| `serde` + `toml` | Config deserialization |
| `clap` | CLI argument parsing |
| `wasm-bindgen` + `js-sys` | WASM/JS bridge |

## Publishing to npm

```sh
./publish.sh
```

## Project structure

```
crates/
  mdx2md-core/    # Library: tokenizer, parser, config, transform, rewriter
  mdx2md-cli/     # Binary: CLI tool
  mdx2md-wasm/    # WASM bindings for JS/TS consumers
npm/
  package.json    # Canonical npm package metadata (@icyjoseph/mdx2md)
playground/
  index.html      # Live WASM playground (deployed to GitHub Pages)
```

## License

MIT
