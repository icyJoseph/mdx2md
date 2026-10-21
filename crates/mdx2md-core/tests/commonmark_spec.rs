//! CommonMark spec compliance: run each spec example through convert() with passthrough
//! config and assert no panic. Validates that we don't break standard Markdown when no MDX is present.
//!
//! Full identity (output == input) is not asserted: Layer 2 may normalize backslash escapes
//! or whitespace. The authoritative check is "no crash and parse/convert succeeds."

use mdx2md_core::{config::Config, convert};
use std::path::PathBuf;

/// Matches content that looks like an HTML/JSX tag or autolink (e.g. `<div`, `</p`, `<!`, `<http`)
/// but not bare `>` used for blockquotes.
fn contains_html_like(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'<' {
            if let Some(&next) = bytes.get(i + 1) {
                if next.is_ascii_alphabetic() || next == b'/' || next == b'!' || next == b'?' {
                    return true;
                }
            }
        }
    }
    false
}

fn spec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("commonmark_spec.json")
}

#[test]
fn commonmark_spec_passthrough_no_panic() {
    let json = std::fs::read_to_string(spec_path()).expect("commonmark_spec.json not found");
    let examples: Vec<SpecExample> = serde_json::from_str(&json).expect("invalid spec.json");

    let config = Config::default();

    for (i, ex) in examples.iter().enumerate() {
        // Skip examples containing HTML-like tags (e.g. `<div>`, `</p>`, `<!DOCTYPE`)
        // which the MDX-aware tokenizer legitimately parses as JSX/HTML.
        // Bare `>` (blockquotes) and non-tag `<` (comparisons) are fine to test.
        if contains_html_like(&ex.markdown) {
            continue;
        }

        let result = convert(&ex.markdown, &config);
        assert!(
            result.is_ok(),
            "example {} (section {:?}) should not error: {:?}",
            i + 1,
            ex.section,
            result.err()
        );
    }
}

#[derive(serde::Deserialize)]
struct SpecExample {
    markdown: String,
    #[allow(dead_code)]
    html: String,
    #[allow(dead_code)]
    example: u32,
    #[allow(dead_code)]
    start_line: u32,
    #[allow(dead_code)]
    end_line: u32,
    section: String,
}

#[test]
#[ignore] // Run with --ignored when checking identity; some examples differ (e.g. backslash escapes)
fn commonmark_spec_passthrough_identity() {
    fn normalize(s: &str) -> String {
        s.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }
    let json = std::fs::read_to_string(spec_path()).expect("commonmark_spec.json not found");
    let examples: Vec<SpecExample> = serde_json::from_str(&json).expect("invalid spec.json");
    let config = Config::default();
    for (i, ex) in examples.iter().enumerate() {
        let result = convert(&ex.markdown, &config).expect("convert should not error");
        let expected = normalize(&ex.markdown);
        let actual = normalize(&result);
        assert_eq!(
            actual,
            expected,
            "example {} (section {:?})",
            i + 1,
            ex.section
        );
    }
}
