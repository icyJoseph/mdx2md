# mdx2md

Convert MDX (Markdown + JSX) into clean, portable Markdown.

Built from scratch in Rust. Available as a CLI, a Rust library, and a WASM package for JavaScript/TypeScript.

**[Try it in the playground](https://icyjoseph.github.io/mdx2md/)** | **[Sanitization demo](https://icyjoseph.github.io/mdx2md/sanitize.html)**

## Why

MDX is great for authoring, but the JSX, imports, and expressions make the files useless outside of MDX-aware tooling. If you want to feed your docs to an LLM, publish them on a platform that only speaks Markdown, or just archive them in a portable format, you need to strip the MDX layer away.

Existing tools pull in the entire unified/remark ecosystem. mdx2md takes a different approach: a purpose-built tokenizer and parser with minimal dependencies, no plugin resolution, and a single config file.

## Example

**Input (MDX):**

```mdx
---
title: Docs
---

import { Callout } from "./components";

# Getting started

<Callout type="warning">Watch out for **breaking changes**.</Callout>

| Feature | Status |
| ------- | ------ |
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
mdx2md-core = "0.2"
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

````typescript
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
````

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

[markdown]
strip_html_comments = false     # remove <!-- ... --> blocks

[markdown.tables]
format = "list"

[markdown.links]
make_absolute = true
base_url = "https://docs.example.com"
strip = false                   # remove all links, keep text
allowed_domains = []            # only keep links to these domains

[markdown.images]
make_absolute = true
base_url = "https://cdn.example.com"
strip = false                   # remove all images
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
- **Links**: `strip = true` removes all links, keeping only the link text
- **Links**: `allowed_domains = ["example.com"]` strips links whose domain is not in the list (relative URLs are always kept)
- **Images**: `make_absolute = true` prepends `base_url` to relative image sources
- **Images**: `strip = true` removes all images
- **HTML comments**: `strip_html_comments = true` removes `<!-- ... -->` blocks

Precedence for links: `strip` > `allowed_domains` > `make_absolute`.

## Use case: sanitize MDX for LLMs

When feeding MDX documents to an LLM (for summarization, RAG, or chatbot context), the JSX, expressions, and hidden content become attack surface. Hidden components can carry prompt injections, expressions can leak secrets, and links can point to phishing sites.

mdx2md can strip all of this in a single pass:

```toml
[options]
strip_imports = true
strip_exports = true
expression_handling = "strip"

[components._default]
template = ""

[markdown]
strip_html_comments = true

[markdown.links]
allowed_domains = ["docs.example.com"]

[markdown.images]
strip = true
```

This config strips imports, exports, expressions, unknown JSX components, HTML comments, tracking images, and any link not pointing to `docs.example.com`. See [docs/llm-sanitization.md](docs/llm-sanitization.md) for the full threat model, before/after examples, and recommended configurations.

**[Try the sanitization demo](https://icyjoseph.github.io/mdx2md/sanitize.html)**

## How it works

```
MDX source string
  → Tokenizer ─── splits into MDX-aware tokens (JSX, imports, expressions, markdown)
  → Parser ────── builds a nested AST from the token stream
  → Layer 1 ───── resolves JSX via templates/callbacks, strips imports/exports
  → Layer 2 ───── rewrites markdown elements (tables, links, images) in-place
  → Clean Markdown
```

The tokenizer and parser are built from scratch with no dependency on remark, unified, or any MDX/JSX parser. Layer 2 uses `pulldown-cmark` only to _locate_ elements by byte offset, then performs surgical string replacements to preserve formatting in untouched sections.

### Spec and expected behavior

- **MDX syntax** follows [mdxjs.com](https://mdxjs.com) and the [micromark MDX extensions](https://github.com/micromark/micromark-extension-mdx-jsx) (JSX, expressions, ESM). We accept the same inputs as valid MDX.
- **MDX → Markdown behavior** is defined by this project’s **fixtures and config**: there is no official “MDX in → Markdown out” spec. The fixtures in `crates/mdx2md-core/tests/fixtures/` (e.g. `kitchen_sink`, `adversarial`, `esm_only`, `jsx_only`) plus their `.toml` and expected `.md` files are the regression suite. Pure Markdown compatibility is checked by running the [CommonMark spec](https://spec.commonmark.org/0.31.2/) examples through `convert()` with a passthrough config (no table/link rewrites) and asserting no panic.

### Dependencies

| Crate                     | Purpose                                             |
| ------------------------- | --------------------------------------------------- |
| `pulldown-cmark`          | Layer 2: locates tables/links/images by byte offset |
| `serde` + `toml`          | Config deserialization                              |
| `clap`                    | CLI argument parsing                                |
| `wasm-bindgen` + `js-sys` | WASM/JS bridge                                      |

## Project structure

```
crates/
  mdx2md-core/    # Library: tokenizer, parser, config, transform, rewriter
  mdx2md-cli/     # Binary: CLI tool
  mdx2md-wasm/    # WASM bindings for JS/TS consumers
docs/
  llm-sanitization.md  # LLM sanitization guide and threat model
npm/
  package.json    # Canonical npm package metadata (@icyjoseph/mdx2md)
playground/
  index.html      # Live WASM playground (deployed to GitHub Pages)
  sanitize.html   # LLM sanitization demo
```

## License

MIT
