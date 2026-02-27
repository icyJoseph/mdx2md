import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

import { compile } from "@mdx-js/mdx";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkStringify from "remark-stringify";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ROOT = path.resolve(__dirname, "..", "..");
const FIXTURES = path.join(
  ROOT,
  "worktree-benchmarking",
  "crates",
  "mdx2md-core",
  "tests",
  "fixtures",
);

async function loadFixture(name) {
  return fs.readFile(path.join(FIXTURES, name), "utf8");
}

async function time(label, iterations, fn) {
  // Warmup
  await fn();

  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    await fn();
  }
  const end = performance.now();
  const avg = (end - start) / iterations;
  console.log(`${label}: ${avg.toFixed(3)} ms (over ${iterations} iters)`);
}

async function benchMdxJs() {
  const kitchen = await loadFixture("kitchen_sink.mdx");

  // Large synthetic MDX similar in spirit to the Rust benchmark
  const paragraph =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore.\n\n";
  const block = `${paragraph.repeat(8)}
<Callout type="info">
  **Nested** content here.
</Callout>

`;
  const large = block.repeat(120);

  const mdxCompile = (source) => compile(source, { jsx: true });

  const remarkPipeline = unified().use(remarkParse).use(remarkStringify);

  console.log("== @mdx-js/mdx + remark ==");
  await time("mdx-js compile (kitchen_sink)", 50, () => mdxCompile(kitchen));
  await time("mdx-js compile (large ~100KB)", 20, () => mdxCompile(large));

  const commonmark = "# Title\n\nParagraph one.\n\nParagraph two with **bold** and *italic*.\n\n".repeat(
    500,
  );
  await time("remark parse+stringify (commonmark)", 50, () =>
    remarkPipeline.process(commonmark),
  );
}

benchMdxJs().catch((err) => {
  console.error(err);
  process.exit(1);
});

