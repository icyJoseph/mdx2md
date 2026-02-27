# Benchmarking & Test Plan for mdx2md

Plan for what to benchmark, which specs/test suites to use, and how to confirm the implementation.

---

## 1. Goals

- **Correctness**: MDX → Markdown behavior matches a defined spec and doesn’t regress.
- **Coverage**: Pure Markdown passes through unchanged (or as configured); MDX constructs (JSX, ESM, expressions) are handled per config.
- **Performance** (optional): Measure parse/convert time for regression and comparison.

---

## 2. What to Benchmark Against

### 2.1 Pure Markdown (Layer 2 + identity)

**Source: [CommonMark Spec](https://spec.commonmark.org/0.31.2/)**

- **Spec**: [commonmark/commonmark-spec](https://github.com/commonmark/commonmark-spec) — `spec.txt` (human-readable) and [spec.json](https://spec.commonmark.org/0.31.2/spec.json) (machine-readable).
- **Format**: Each example has `markdown` (input) and `html` (reference output). We don’t render to HTML; we care that:
  - **Identity**: With a “passthrough” config (no table/list/link rewrites), running `convert(spec["markdown"], passthrough_config)` should yield the same string (or equivalent Markdown). That validates we don’t break CommonMark when no MDX is present.
- **Caveat**: CommonMark spec is markdown → HTML. For mdx2md we only need markdown → markdown. So the use is: “our parser/rewriter doesn’t corrupt standard Markdown.” Running all spec examples through `convert` and checking round-trip (or at least no crash + optional structural sanity) is the right benchmark for “Markdown compatibility.”

**Possible implementation:**

- Add a test (or small binary) that:
  - Downloads or vendors `spec.json`.
  - For each `markdown` string, runs `convert(md, passthrough_config)`.
  - Asserts either: output == input (strict), or “no panic + optional normalization” (softer).
- Start with “no panic” and “output equals input” for a passthrough config; later add GFM-only or link/table rewrite tests if needed.

### 2.2 MDX syntax (reference: micromark extensions)

**Source: [mdxjs.com](https://mdxjs.com) + micromark extensions (reference implementations)**

- **Spec status**: The [mdx-js/specification](https://github.com/mdx-js/specification) repo is archived. Authoritative syntax is described in:
  - [micromark-extension-mdx-jsx](https://github.com/micromark/micromark-extension-mdx-jsx#syntax) — JSX in MDX (BNF, tokens, errors).
  - [micromark-extension-mdx-expression](https://github.com/micromark/micromark-extension-mdx-expression) — `{expressions}`.
  - [micromark-extension-mdxjs-esm](https://github.com/micromark/micromark-extension-mdxjs-esm) — `import`/`export`.
- **No official MDX→Markdown spec**: There is no standard “MDX in → Markdown out” test suite. So we define our own expected behavior and optionally align with “what micromark parses as valid MDX.”

**Ways to use this:**

1. **Syntax coverage**: Build fixtures that exercise each construct from the micromark docs (JSX tags, attributes, expressions, ESM). We don’t need to match micromark’s AST; we need to **accept** the same inputs (no parse errors on valid MDX) and produce **our** defined Markdown (templates, strip, etc.).
2. **Error cases**: Optionally add tests for invalid MDX that we intentionally reject (e.g. unclosed tags) so we don’t silently emit wrong output.
3. **Extracting cases**: The micromark-extension-mdx-jsx repo has a single large `test/index.js`; we could manually copy a few representative examples into our fixtures rather than depending on that file.

### 2.3 Our own behavior (MDX → Markdown)

**Source: Project fixtures + config**

- **Current fixtures**: `kitchen_sink.mdx` (+ `.toml`, `.md`) and `adversarial.mdx` (+ `.toml`, `.md`) already define “expected” Markdown for a given config. This is the **authoritative** spec for “what mdx2md does.”
- **Expand with**:
  - One fixture per major feature (e.g. only JSX, only expressions, only ESM, only tables/links).
  - Edge cases: nested JSX, `{children}`, expression in attribute, empty components, HTML comments, etc.
  - Sanitization: `adversarial` already targets LLM/safety; we can add more cases (XSS, `javascript:`, allowed_domains, strip images/links).

**Benchmark definition**: “Implementation is correct” = all fixture tests pass (input MDX + config → expected .md). No external suite defines that; we own it.

---

## 3. Test Suites / Specs We Can Use

| Source | What it gives | How we use it |
|--------|----------------|----------------|
| **CommonMark spec.json** | ~600+ markdown examples | Run each through `convert` with passthrough config; assert no crash and (optionally) output == input. Validates “we don’t break standard Markdown.” |
| **GFM (GitHub)** | Tables, strikethrough, etc. | [cmark-gfm](https://github.com/github/cmark-gfm) has `test/spec.txt`. If we want GFM-specific behavior (e.g. tables), we can add a subset of GFM tests; currently we rewrite tables to lists, so “expected” is our choice. |
| **micromark-extension-mdx-jsx (syntax)** | BNF + docs for JSX | Use to design our own MDX fixtures (valid/invalid) so we stay aligned with “real” MDX. |
| **mdx-js/specification** | Archived; points to mdxjs.com + micromark | No fixtures to pull; only high-level reference. |
| **Existing mdx2md fixtures** | kitchen_sink, adversarial | Core regression suite; expand as needed. |

---

## 4. How to Confirm the Implementation

### 4.1 Unit / integration (already in place)

- **Tokenizer**: `tokenizer.rs` tests (frontmatter, import/export, JSX, expressions, markdown passthrough).
- **Parser**: `parser.rs` tests (AST shape, nested JSX, errors).
- **Transform**: `transform.rs` tests (strip imports/exports, frontmatter, component templates, expressions).
- **Rewriter**: `rewriter.rs` tests (tables, links, images, allowed_domains, strip, HTML comments).
- **Full pipeline**: `lib.rs` integration tests with `kitchen_sink` and `adversarial` fixtures.

**Improvement (from TODOS.md):** Use a shared `fixture_path()` based on `env!("CARGO_MANIFEST_DIR")` in all tests that read `tests/fixtures/...`, so paths work from any cwd.

### 4.2 CommonMark compliance (new)

- Add a test (or `tests/commonmark_spec.rs`) that:
  - Loads CommonMark `spec.json` (vendored or fetched in build).
  - Uses a strict passthrough config (no table/link rewrites, no strip).
  - For each example, runs `convert(markdown, config)` and checks:
    - No panic.
    - Optional: `output == markdown` (after normalizing line endings).
- If we later change table/link behavior, we can still run only the “pure markdown” subset or exclude examples that we intentionally rewrite.

### 4.3 MDX fixture expansion (new)

- Add more fixtures under `tests/fixtures/`:
  - `esm_only.mdx` — only import/export; expected .md has them stripped.
  - `jsx_only.mdx` — only components; expected .md uses templates.
  - `expressions_only.mdx` — only `{expr}`; strip vs preserve_raw vs placeholder.
  - `tables_links.mdx` — only Markdown tables/links; test list format and absolute URLs.
  - Optional: `invalid.mdx` — unclosed tag / invalid JSX; expect `convert` to return `Err`.
- Keep the pattern: `*.mdx` + `*.toml` + `*.md` (expected), and one integration test per fixture or a loop over a list of names.

### 4.4 Performance benchmarks (optional)

- **Tool**: [criterion](https://crates.io/crates/criterion) in a `[[bench]]` or a separate `benches/` crate.
- **Metrics**:
  - `convert()` on `kitchen_sink.mdx` (small).
  - `convert()` on a large synthetic MDX (e.g. 100 KB) to stress tokenizer/parser/rewriter.
  - Optionally: “identity” run over a large CommonMark-only file to measure Layer 2 overhead.
- **Baseline**: Record first run; use criterion to detect regressions (e.g. in CI).

---

## 5. Suggested Order of Work

1. **Fixture path cleanup** (from TODOS): Use `env!("CARGO_MANIFEST_DIR")` in all tests that reference `tests/fixtures/...`.
2. **CommonMark spec test**: Vendor or fetch `spec.json`, add one test that runs all markdown examples through `convert` with passthrough and asserts no panic + identity (or document any intentional differences).
3. **Expand MDX fixtures**: Add 3–5 small fixtures (esm_only, jsx_only, expressions_only, tables_links, maybe one invalid) and wire them into integration tests.
4. **Document expected behavior**: In README or `docs/`, state that “MDX syntax follows mdxjs.com / micromark; MDX→Markdown behavior is defined by our fixtures and config.”
5. **Benchmarks**: Add criterion and 2–3 benchmarks (small MDX, large MDX, maybe CommonMark identity) for future regression testing.

---

## 6. Summary

- **Spec we can use**: **CommonMark spec.json** — to ensure we don’t break standard Markdown (passthrough identity).
- **No off-the-shelf MDX→Markdown suite**: MDX has a syntax spec (micromark) but no “expected Markdown output” spec; **our fixtures are the spec** for mdx2md.
- **Confirm implementation by**: (1) existing unit + integration tests, (2) CommonMark passthrough test, (3) more MDX fixtures, (4) optional criterion benchmarks for performance.
