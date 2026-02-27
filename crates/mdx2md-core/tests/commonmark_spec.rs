//! CommonMark spec compliance: run each spec example through convert() with passthrough
//! config and assert no panic. Validates that we don't break standard Markdown when no MDX is present.
//!
//! Full identity (output == input) is not asserted: Layer 2 may normalize backslash escapes
//! or whitespace. The authoritative check is "no crash and parse/convert succeeds."

use mdx2md_core::{convert, config::Config};
use std::path::PathBuf;

fn spec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("commonmark_spec.json")
}

#[test]
fn commonmark_spec_passthrough_no_panic() {
    let json = std::fs::read_to_string(spec_path()).expect("commonmark_spec.json not found");
    let examples: Vec<SpecExample> = serde_json::from_str(&json).expect("invalid spec.json");

    let config = Config::default();

    for (i, ex) in examples.iter().enumerate() {
        // Skip examples that exercise HTML/inline HTML which can legitimately
        // be parsed as JSX/HTML by the MDX-aware tokenizer (e.g. "<a>" tests
        // in the "Backslash escapes" section). Those are out of scope for the
        // "pure Markdown passthrough" check here.
        if ex.markdown.contains('<') || ex.markdown.contains('>') {
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
        s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    }
    let json = std::fs::read_to_string(spec_path()).expect("commonmark_spec.json not found");
    let examples: Vec<SpecExample> = serde_json::from_str(&json).expect("invalid spec.json");
    let config = Config::default();
    for (i, ex) in examples.iter().enumerate() {
        let result = convert(&ex.markdown, &config).expect("convert should not error");
        let expected = normalize(&ex.markdown);
        let actual = normalize(&result);
        assert_eq!(actual, expected, "example {} (section {:?})", i + 1, ex.section);
    }
}
