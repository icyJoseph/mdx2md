pub mod ast;
pub mod config;
pub mod parser;
pub mod rewriter;
pub mod tokenizer;
pub mod transform;

use config::Config;
pub use transform::ComponentResolver;

/// Full MDX-to-Markdown conversion pipeline (Layer 1 + Layer 2).
pub fn convert(mdx: &str, config: &Config) -> Result<String, ConvertError> {
    let tokens = tokenizer::tokenize(mdx).map_err(|e| ConvertError(e.message))?;
    let doc = parser::parse(tokens).map_err(|e| ConvertError(e.message))?;
    let raw_md = transform::transform(&doc, config);
    let final_md = rewriter::rewrite_markdown(&raw_md, config);
    Ok(final_md)
}

/// Full pipeline with an external component resolver (for WASM JS callbacks).
pub fn convert_with_resolver(
    mdx: &str,
    config: &Config,
    resolver: &dyn ComponentResolver,
) -> Result<String, ConvertError> {
    let tokens = tokenizer::tokenize(mdx).map_err(|e| ConvertError(e.message))?;
    let doc = parser::parse(tokens).map_err(|e| ConvertError(e.message))?;
    let raw_md = transform::transform_with_resolver(&doc, config, resolver);
    let final_md = rewriter::rewrite_markdown(&raw_md, config);
    Ok(final_md)
}

#[derive(Debug)]
pub struct ConvertError(pub String);

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ConvertError {}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
    }

    #[test]
    fn test_full_pipeline_kitchen_sink() {
        let input = std::fs::read_to_string(fixture_path("kitchen_sink.mdx")).unwrap();
        let toml_str = std::fs::read_to_string(fixture_path("kitchen_sink.toml")).unwrap();
        let expected = std::fs::read_to_string(fixture_path("kitchen_sink.md")).unwrap();
        let config = Config::from_toml(&toml_str).unwrap();

        let result = convert(&input, &config).unwrap();

        // Normalize for comparison: trim trailing whitespace on each line and normalize line endings
        let result_lines = normalize(&result);
        let expected_lines = normalize(&expected);

        if result_lines != expected_lines {
            eprintln!("=== EXPECTED ===");
            eprintln!("{expected}");
            eprintln!("=== GOT ===");
            eprintln!("{result}");
            eprintln!("=== DIFF ===");
            for (i, (r, e)) in result_lines.iter().zip(expected_lines.iter()).enumerate() {
                if r != e {
                    eprintln!("Line {}: expected {:?}, got {:?}", i + 1, e, r);
                }
            }
            if result_lines.len() != expected_lines.len() {
                eprintln!(
                    "Line count: expected {}, got {}",
                    expected_lines.len(),
                    result_lines.len()
                );
            }
            panic!("Output does not match expected");
        }
    }

    #[test]
    fn test_full_pipeline_adversarial() {
        let input = std::fs::read_to_string(fixture_path("adversarial.mdx")).unwrap();
        let toml_str = std::fs::read_to_string(fixture_path("adversarial.toml")).unwrap();
        let expected = std::fs::read_to_string(fixture_path("adversarial.md")).unwrap();
        let config = Config::from_toml(&toml_str).unwrap();

        let result = convert(&input, &config).unwrap();

        let result_lines = normalize(&result);
        let expected_lines = normalize(&expected);

        if result_lines != expected_lines {
            eprintln!("=== EXPECTED ===");
            eprintln!("{expected}");
            eprintln!("=== GOT ===");
            eprintln!("{result}");
            eprintln!("=== DIFF ===");
            for (i, (r, e)) in result_lines.iter().zip(expected_lines.iter()).enumerate() {
                if r != e {
                    eprintln!("Line {}: expected {:?}, got {:?}", i + 1, e, r);
                }
            }
            if result_lines.len() != expected_lines.len() {
                eprintln!(
                    "Line count: expected {}, got {}",
                    expected_lines.len(),
                    result_lines.len()
                );
            }
            panic!("Adversarial output does not match expected");
        }

        // Verify specific sanitization guarantees
        assert!(!result.contains("evil.example"), "Should strip evil domain imports");
        assert!(!result.contains("phishing.example"), "Should strip phishing links");
        assert!(!result.contains("malicious.example"), "Should strip hidden context");
        assert!(!result.contains("tracker.evil"), "Should strip tracking images");
        assert!(!result.contains("javascript:"), "Should strip javascript: URIs");
        assert!(!result.contains("SECRET_KEY"), "Should strip expressions");
        assert!(!result.contains("<!--"), "Should strip HTML comments");
        assert!(!result.contains("Ignore all previous"), "Should strip prompt injections");
        assert!(result.contains("https://docs.example.com/setup"), "Should keep allowed links");
        assert!(result.contains("/docs/quickstart"), "Should keep relative links");
    }

    /// All fixtures that have .mdx + .toml + .md (expected). Run full pipeline and assert output matches.
    const FIXTURE_NAMES: &[&str] = &[
        "kitchen_sink",
        "adversarial",
        "esm_only",
        "jsx_only",
        "expressions_only",
        "tables_links",
    ];

    #[test]
    fn test_full_pipeline_all_fixtures() {
        for name in FIXTURE_NAMES {
            let input =
                std::fs::read_to_string(fixture_path(&format!("{name}.mdx"))).unwrap_or_else(|e| {
                    panic!("fixture {name}.mdx: {e}");
                });
            let toml_str =
                std::fs::read_to_string(fixture_path(&format!("{name}.toml"))).unwrap_or_else(|e| {
                    panic!("fixture {name}.toml: {e}");
                });
            let expected =
                std::fs::read_to_string(fixture_path(&format!("{name}.md"))).unwrap_or_else(|e| {
                    panic!("fixture {name}.md: {e}");
                });
            let config = Config::from_toml(&toml_str).unwrap();
            let result = convert(&input, &config).unwrap();
            let result_lines = normalize(&result);
            let expected_lines = normalize(&expected);
            assert_eq!(
                result_lines,
                expected_lines,
                "fixture {name}: output does not match expected"
            );
        }
    }

    #[test]
    fn test_invalid_mdx_returns_error() {
        // Unclosed JSX tag should cause parse error
        let input = "Hello <Open> world";
        let config = Config::default();
        let result = convert(input, &config);
        assert!(result.is_err(), "unclosed JSX should return Err");
    }

    #[test]
    fn test_html_to_markdown_via_components() {
        let input = "<h1>Hello</h1>\n<p>This is <strong>bold</strong> and <em>italic</em>.</p>";
        let toml_config = r##"
[components.h1]
template = "# {children}\n"

[components.p]
template = "{children}\n"

[components.strong]
template = "**{children}**"

[components.em]
template = "*{children}*"

[components._default]
template = "{children}"
"##;
        let config = Config::from_toml(toml_config).unwrap();
        let result = convert(input, &config).unwrap();
        assert!(result.contains("# Hello"), "Expected '# Hello', got: {}", result);
        assert!(result.contains("**bold**"), "Expected '**bold**', got: {}", result);
        assert!(result.contains("*italic*"), "Expected '*italic*', got: {}", result);
    }

    fn normalize(s: &str) -> Vec<String> {
        s.lines().map(|l| l.trim_end().to_string()).collect()
    }
}
