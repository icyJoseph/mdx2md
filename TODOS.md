## Code review TODOS

- **Fixtures & tests**
  - Switch `config.rs`, `transform.rs`, and `parser.rs` tests that use `"tests/fixtures/..."`
    to a shared `fixture_path(...)` helper based on `env!("CARGO_MANIFEST_DIR")`, like the
    one now used in `lib.rs` integration tests.

- **Rewriter & URL handling**
  - Extend `parse_link_destination` to also support single-quoted link titles
    (e.g. `[x](url 'title')`) to match the comment and common Markdown behavior.
  - Consider improving `extract_host` to correctly handle IPv6 literals such as
    `https://[::1]/path` when evaluating allowed domains.

- **Component templates**
  - Clarify or adjust the `{children}` placeholder behavior in `apply_template`:
    currently attribute replacement runs first, so an attribute named `children`
    will shadow the real children content. Either document this as a reserved
    name or change the replacement order so `{children}` always means node
    children.

- **CLI behavior**
  - Decide whether `mdx2md` should accept non-`.mdx` single-file inputs
    (e.g. `foo.md`); today they are passed through without extension checks.
    If not desired, add validation or a warning.
  - Improve batch directory conversion error handling: instead of exiting on
    the first failing file, consider collecting per-file errors and printing a
    summary like `N/M files failed` before exiting with non-zero status.

- **Streaming / partial input support**
  - ~~React SSR with Suspense: `StreamConverter` handles `<!--$?-->` boundaries,
    `<template id="B:X">`, `$RC` swap scripts, and out-of-order resolution.~~
    ✅ Done — `streaming.rs` + `--stream` CLI flag.
  - **LLM / raw text streaming**: tokens arrive mid-syntax (e.g. `**bol` then
    `d**`, or `<div cl` then `ass="x">`). Need a `TextStreamConverter` that:
    - Buffers incoming tokens until a "safe emit point" is reached.
    - Safe points: end of a complete paragraph (`\n\n`), closed inline syntax
      (`**...**`, `*...*`, `` `...` ``), closed HTML/JSX tag, closed link
      `[text](url)`, end of a fenced code block.
    - Tracks open syntax depth: unclosed `**`, `*`, `` ` ``, `[`, `<Tag>`.
      Only emits buffered content when all openers are closed or a paragraph
      boundary is reached.
    - Handles the ambiguity of `*` (could be list item OR italic opener) by
      looking at context (start of line vs inline).
    - Timeout / force-flush: if no safe point is reached after N bytes or
      T milliseconds, emit what we have (best-effort) so the consumer isn't
      starved. The next chunk can re-parse if needed.
    - Composable with `StreamConverter`: LLM stream → `TextStreamConverter`
      (stabilizes syntax) → `StreamConverter` (resolves Suspense) → Markdown.
    - CLI: `--stream` should auto-detect React markers vs raw text, or add a
      `--stream-mode=react|text|auto` flag.

- **Markdown ↔ HTML round-trip / progressive serving**
  - The MDX → Markdown → HTML cycle is semantically idempotent: mdx2md strips
    runtime concerns (JSX, expressions, React) but preserves content. This means
    the tool can sit at the interchange point between all three formats.
  - **`text/markdown` responses** ([RFC 7763](https://datatracker.ietf.org/doc/html/rfc7763)):
    serve Markdown natively from any SSR app. A middleware/proxy could
    content-negotiate via `Accept: text/markdown` and pipe the HTML stream
    through `StreamConverter` → emit `text/markdown; variant=CommonMark`.
  - **Progressive Markdown rendering**: stream Markdown chunks via SSE
    (`text/event-stream`) or chunked transfer encoding. Each chunk is valid
    Markdown that the client can render immediately. Pairs with
    `TextStreamConverter` to ensure chunks break at safe syntax points.
  - **Clean HTML proxy**: `curl site.com | mdx2md --stream | md-to-html` for
    zero-JS static HTML — useful for RSS feeds, email digests, archival,
    search indexing, and accessibility.
  - **LLM context pipe**: stream a live site into an LLM context window as
    clean Markdown, progressively, without waiting for full page load.

- **WASM bindings**
  - Revisit `JsComponentResolver::resolve`’s use of `func.call1(&JsValue::NULL, ...)`:
    if component callbacks ever need a non-null `this`, adjust to pass a more
    appropriate receiver or document the intended calling convention.

