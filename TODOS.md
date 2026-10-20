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
  - Support converting MDX/HTML from a chunked stream (e.g. React SSR with
    Suspense) rather than requiring the full document up front.
  - Buffer at Suspense boundaries: React's out-of-order streaming emits
    fallback content first, then delivers resolved content later via
    `<template id="B:...">` chunks and `$RC`/`$RS` replacement scripts.
    The converter should hold partial output until a boundary is fully
    revealed before emitting its Markdown.
  - Handle out-of-order resolution: a deeply nested Suspense boundary can
    resolve before a shallower one. The buffer/reveal logic needs to track
    a tree of pending boundaries rather than assuming FIFO order.
  - Decide on the output API: `Stream<Item = MarkdownChunk>` that yields
    stable Markdown segments as boundaries resolve, vs accumulating
    everything and returning once complete. The streaming variant is more
    useful for real-time pipelines (e.g. feeding an LLM context window
    as content loads).

- **WASM bindings**
  - Revisit `JsComponentResolver::resolve`’s use of `func.call1(&JsValue::NULL, ...)`:
    if component callbacks ever need a non-null `this`, adjust to pass a more
    appropriate receiver or document the intended calling convention.

