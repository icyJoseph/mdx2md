//! Benchmarks for the MDX → Markdown conversion pipeline.
//! Run with: cargo bench -p mdx2md-core

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mdx2md_core::{convert, config::Config};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

fn kitchen_sink_input() -> (String, Config) {
    let input = std::fs::read_to_string(fixture_path("kitchen_sink.mdx")).unwrap();
    let toml_str = std::fs::read_to_string(fixture_path("kitchen_sink.toml")).unwrap();
    let config = Config::from_toml(&toml_str).unwrap();
    (input, config)
}

/// Small MDX: kitchen_sink fixture (~1.5 KB).
fn bench_convert_small_mdx(c: &mut Criterion) {
    let (input, config) = kitchen_sink_input();
    c.bench_function("convert_small_mdx", |b| {
        b.iter(|| {
            let _ = black_box(convert(black_box(&input), black_box(&config)).unwrap());
        })
    });
}

/// Large synthetic MDX (~100 KB): repeated paragraphs and JSX to stress tokenizer/parser/rewriter.
fn bench_convert_large_mdx(c: &mut Criterion) {
    let (small, config) = kitchen_sink_input();
    let paragraph = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore.\n\n";
    let block = format!(
        "{}\n<Callout type=\"info\">\n  **Nested** content here.\n</Callout>\n\n",
        paragraph.repeat(8)
    );
    let large: String = block.repeat(120); // ~100 KB
    c.bench_function("convert_large_mdx", |b| {
        b.iter(|| {
            let _ = black_box(convert(black_box(&large), black_box(&config)).unwrap());
        })
    });
    // Keep small in scope for type clarity
    let _ = small;
}

/// Passthrough (default config) on a long pure-Markdown string to measure Layer 2 overhead.
fn bench_convert_commonmark_identity(c: &mut Criterion) {
    let config = Config::default();
    let markdown: String = "# Title\n\nParagraph one.\n\nParagraph two with **bold** and *italic*.\n\n".repeat(500);
    c.bench_function("convert_commonmark_identity", |b| {
        b.iter(|| {
            let _ = black_box(convert(black_box(&markdown), black_box(&config)).unwrap());
        })
    });
}

criterion_group!(
    benches,
    bench_convert_small_mdx,
    bench_convert_large_mdx,
    bench_convert_commonmark_identity
);
criterion_main!(benches);
