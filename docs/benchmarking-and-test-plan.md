# Benchmarking & Tests for mdx2md

What we benchmark against, which suites we use, how to rerun them, and current performance vs `@mdx-js/mdx` + remark.

---

## 1. Goals

- **Correctness**: MDX → Markdown behavior matches a defined spec and doesn’t regress.
- **Coverage**: Pure Markdown passes through unchanged (or as configured); MDX constructs (JSX, ESM, expressions) are handled per config.
- **Performance**: Measure parse/convert time for regression tracking and for rough comparison against a canonical JS MDX stack.

---

## 2. What we benchmark against

### 2.1 Pure Markdown (Layer 2 + identity)

**Source: [CommonMark Spec](https://spec.commonmark.org/0.31.2/)**

- **Spec**: [commonmark/commonmark-spec](https://github.com/commonmark/commonmark-spec) — `spec.txt` (human-readable) and [spec.json](https://spec.commonmark.org/0.31.2/spec.json) (machine-readable).
- **Format**: Each example has `markdown` (input) and `html` (reference output). We don’t render to HTML; we care that:
  - **Passthrough**: With a “passthrough” config (no table/list/link rewrites), running `convert(spec["markdown"], passthrough_config)` should at minimum succeed and ideally be an identity transform after normalization. That validates we don’t break CommonMark when no MDX is present.
- **Caveat**: CommonMark spec is markdown → HTML. For mdx2md we only need markdown → markdown. So the use is: “our parser/rewriter doesn’t corrupt standard Markdown.” Running all spec examples through `convert` and asserting “no crash” is the main check; identity is a best-effort extra (some backslash escape examples differ).

**Implementation (current):**

- Vendored file: `crates/mdx2md-core/tests/commonmark_spec.json`.
- Test: `crates/mdx2md-core/tests/commonmark_spec.rs`:
  - `commonmark_spec_passthrough_no_panic`: deserializes `spec.json` with `serde_json`, runs every `markdown` example through `convert(md, Config::default())`, and asserts `Ok(_)`.
  - `commonmark_spec_passthrough_identity`: `#[ignore]` test that also asserts identity after normalizing line endings; ignored because some examples (backslash escapes) intentionally differ.

**How to run:**

```bash
cd worktree-benchmarking
cargo test -p mdx2md-core commonmark_spec_passthrough_no_panic

# Optional, stricter identity check:
cargo test -p mdx2md-core --test commonmark_spec -- --ignored
```

### 2.2 MDX syntax (reference: micromark extensions)

**Source: [mdxjs.com](https://mdxjs.com) + micromark extensions (reference implementations)**

- **Spec status**: The [mdx-js/specification](https://github.com/mdx-js/specification) repo is archived. Authoritative syntax is described in:
  - [micromark-extension-mdx-jsx](https://github.com/micromark/micromark-extension-mdx-jsx#syntax) — JSX in MDX (BNF, tokens, errors).
  - [micromark-extension-mdx-expression](https://github.com/micromark/micromark-extension-mdx-expression) — `{expressions}`.
  - [micromark-extension-mdxjs-esm](https://github.com/micromark/micromark-extension-mdxjs-esm) — `import`/`export`.
- **No official MDX→Markdown spec**: There is no standard “MDX in → Markdown out” test suite. So we define our own expected behavior and optionally align with “what micromark parses as valid MDX.”

**How we use this:**

1. **Syntax coverage**: Fixtures exercise the constructs described in the micromark docs (JSX tags, attributes, expressions, ESM). We don’t need to match micromark’s AST; we need to **accept** the same inputs (no parse errors on valid MDX) and produce **our** defined Markdown (templates, strip, etc.).
2. **Error cases**: Negative tests assert that invalid MDX we intentionally reject (e.g. unclosed tags) returns `Err` so we don’t silently emit wrong output.
3. **Extracting cases**: Where useful, we mirror or adapt examples from micromark’s tests, but the canonical spec for MDX → Markdown here is our own fixtures.

### 2.3 Our own behavior (MDX → Markdown)

**Source: Project fixtures + config**

- **Core fixtures**: `kitchen_sink.mdx` (+ `.toml`, `.md`) and `adversarial.mdx` (+ `.toml`, `.md`) define “expected” Markdown for a given config. This is the **authoritative** spec for “what mdx2md does**.
- **Focused fixtures** (all live in `crates/mdx2md-core/tests/fixtures/` with `.mdx` + `.toml` + `.md`):
  - `esm_only` — only import/export; expected `.md` has them stripped.
  - `jsx_only` — only components; expected `.md` uses templates.
  - `expressions_only` — only `{expr}`; uses `expression_handling = "strip"` to drop expressions.
  - `tables_links` — only Markdown tables/links; tests list format and absolute URLs.
- **Sanitization**: `adversarial` targets LLM/safety; it exercises XSS-style links, `javascript:` URLs, `allowed_domains`, HTML comments, hidden prompt injections, and images.

**Definition of “correct”**: All fixture tests pass (input MDX + config → expected `.md`), and negative tests behave as expected (invalid MDX returns `Err`).

---

## 3. Test suites / specs in use

| Source | What it gives | How we use it |
|--------|----------------|----------------|
| **CommonMark spec.json** | ~600+ markdown examples | Vendored as `tests/commonmark_spec.json`; `tests/commonmark_spec.rs` runs each `markdown` example through `convert` with a passthrough config and asserts no panic. An optional ignored test additionally checks identity. |
| **GFM (GitHub)** | Tables, strikethrough, etc. | [cmark-gfm](https://github.com/github/cmark-gfm) has `test/spec.txt`. If we want GFM-specific behavior (e.g. tables), we can add a subset of GFM tests; currently we rewrite tables to lists, so “expected” is our choice. |
| **micromark-extension-mdx-jsx (syntax)** | BNF + docs for JSX | Used as reference for which JSX constructs our fixtures cover. |
| **mdx-js/specification** | Archived; points to mdxjs.com + micromark | High-level reference; no fixtures to pull. |
| **Existing mdx2md fixtures** | kitchen_sink, adversarial, esm_only, jsx_only, expressions_only, tables_links | Core regression suite; defines mdx2md’s MDX → Markdown behavior. |

---

## 4. How we confirm the implementation

### 4.1 Unit / integration (core crate)

- **Tokenizer**: `tokenizer.rs` tests (frontmatter, import/export, JSX, expressions, markdown passthrough).
- **Parser**: `parser.rs` tests (AST shape, nested JSX, errors).
- **Transform**: `transform.rs` tests (strip imports/exports, frontmatter, component templates, expressions).
- **Rewriter**: `rewriter.rs` tests (tables, links, images, allowed_domains, strip, HTML comments).
- **Full pipeline**: `lib.rs` integration tests:
  - `test_full_pipeline_kitchen_sink` and `test_full_pipeline_adversarial` (original fixtures).
  - `test_full_pipeline_all_fixtures` which drives `kitchen_sink`, `adversarial`, `esm_only`, `jsx_only`, `expressions_only`, and `tables_links` and compares against expected `.md`.
  - `test_invalid_mdx_returns_error` which asserts that clearly invalid MDX (unclosed JSX) returns `Err`.

All fixture-based tests use a shared `fixture_path(name)` helper based on `env!("CARGO_MANIFEST_DIR")` so paths work regardless of current working directory.

### 4.2 Performance benchmarks (Criterion)

**Tool**: [criterion](https://crates.io/crates/criterion) via `crates/mdx2md-core/benches/convert.rs` and the `[[bench]]` entry in `crates/mdx2md-core/Cargo.toml`.

**Benchmarks:**

- `convert_small_mdx` — `kitchen_sink.mdx` (~1–2 KB, realistic doc).
- `convert_large_mdx` — synthetic MDX (~100 KB) built from repeated paragraphs and JSX to stress tokenizer/parser/rewriter.
- `convert_commonmark_identity` — large CommonMark-only string (repeated headings/paragraphs) to measure Layer 2 overhead.

**How to run:**

```bash
cd worktree-benchmarking
cargo bench -p mdx2md-core
```

**Example results (on the author’s machine):**

- `convert_small_mdx`: ~**0.015 ms** per run (~15 µs).
- `convert_large_mdx`: ~**0.81 ms** per run.
- `convert_commonmark_identity`: ~**0.14 ms** per run.

These numbers serve as the baseline for future regression tracking.

### 4.3 Comparison with @mdx-js/mdx + remark (Node)

For a rough external baseline, there is a small Node benchmark under `benchmarks/mdx-js/bench.mjs` which:

- Uses `@mdx-js/mdx`’s `compile()` on:
  - `kitchen_sink.mdx`.
  - A synthetic ~100 KB MDX string similar to the Rust large benchmark.
- Uses a `remark-parse` + `remark-stringify` pipeline on a large CommonMark-only string.

**How to run:**

```bash
cd benchmarks/mdx-js
node bench.mjs
```

**Example results (same machine as the Criterion run):**

- `mdx-js compile (kitchen_sink)`: ~**2.09 ms**.
- `mdx-js compile (large ~100KB)`: ~**43.1 ms**.
- `remark parse+stringify (commonmark)`: ~**44.6 ms**.

**Very rough ratios vs `mdx2md-core`:**

- Small MDX: mdx2md is O(10²)× faster (~140× here).
- Large MDX: mdx2md is O(10¹–10²)× faster (~50× here).
- CommonMark-only text: mdx2md is O(10²–10³)× faster (~300× here).

This is not a strict apples-to-apples comparison (different outputs and ecosystems), but it shows that the Rust core is “fast enough” by a wide margin for typical doc workloads.

---

## 5. Summary

- **Specs we use**: **CommonMark spec.json** for Markdown compatibility, plus MDX syntax docs from mdxjs/micromark.
- **MDX → Markdown behavior**: There is no off-the-shelf MDX→Markdown spec; the fixtures in `crates/mdx2md-core/tests/fixtures/` are the spec for mdx2md.
- **Correctness**: Verified via unit tests, full-pipeline fixture tests, CommonMark passthrough, and negative tests for invalid MDX.
- **Performance**: Tracked via Criterion benchmarks in `mdx2md-core` with a rough external comparison to `@mdx-js/mdx` + remark.
