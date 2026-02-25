use crate::ast::*;
use crate::config::*;
use std::collections::HashMap;

/// External resolver for JSX components. Called with (tag, props_map, children_str)
/// and returns Some(rendered_string) to handle the component, or None to fall back
/// to config-based templates.
pub trait ComponentResolver {
    fn resolve(&self, tag: &str, props: &HashMap<String, String>, children: &str) -> Option<String>;
}

/// No-op resolver that always falls back to config.
struct NoResolver;
impl ComponentResolver for NoResolver {
    fn resolve(&self, _tag: &str, _props: &HashMap<String, String>, _children: &str) -> Option<String> {
        None
    }
}

/// Layer 1: Transform an MDX AST into raw Markdown by resolving JSX components,
/// stripping imports/exports, and handling expressions according to config.
pub fn transform(doc: &MdxDocument, config: &Config) -> String {
    transform_with_resolver(doc, config, &NoResolver)
}

/// Layer 1 with an external component resolver (used by WASM for JS callbacks).
pub fn transform_with_resolver(doc: &MdxDocument, config: &Config, resolver: &dyn ComponentResolver) -> String {
    let mut output = String::new();

    for node in &doc.nodes {
        transform_node(node, config, resolver, &mut output);
    }

    clean_blank_lines(&output)
}

fn transform_node(node: &MdxNode, config: &Config, resolver: &dyn ComponentResolver, out: &mut String) {
    match node {
        MdxNode::Frontmatter(content) => {
            if config.options.preserve_frontmatter {
                out.push_str("---\n");
                out.push_str(content);
                out.push_str("\n---\n");
            }
        }
        MdxNode::Import(_) => {
            if !config.options.strip_imports {
                if let MdxNode::Import(s) = node {
                    out.push_str(s);
                    out.push('\n');
                }
            }
        }
        MdxNode::Export(_) => {
            if !config.options.strip_exports {
                if let MdxNode::Export(s) = node {
                    out.push_str(s);
                    out.push('\n');
                }
            }
        }
        MdxNode::Markdown(content) => {
            out.push_str(content);
        }
        MdxNode::Expression(content) => match config.options.expression_handling {
            ExpressionHandling::Strip => {}
            ExpressionHandling::PreserveRaw => {
                out.push('{');
                out.push_str(content);
                out.push('}');
            }
            ExpressionHandling::Placeholder => {
                out.push_str("[expression]");
            }
        },
        MdxNode::JsxElement {
            tag,
            attributes,
            children,
            ..
        } => {
            let children_str = transform_children(children, config, resolver);

            // Build props map for resolver
            let props: HashMap<String, String> = attributes
                .iter()
                .map(|a| {
                    let val = match &a.value {
                        Some(AttrValue::String(s)) => s.clone(),
                        Some(AttrValue::Expression(e)) => e.clone(),
                        None => "true".to_string(),
                    };
                    (a.name.clone(), val)
                })
                .collect();

            // Try external resolver first, then config templates
            if let Some(rendered) = resolver.resolve(tag, &props, &children_str) {
                out.push_str(&rendered);
            } else {
                let component_config = config
                    .components
                    .get(tag)
                    .or_else(|| config.components.get("_default"));

                match component_config {
                    Some(ct) => {
                        let rendered = apply_template(&ct.template, attributes, &children_str);
                        out.push_str(&rendered);
                    }
                    None => {
                        out.push_str(&children_str);
                    }
                }
            }
        }
    }
}

fn transform_children(children: &[MdxNode], config: &Config, resolver: &dyn ComponentResolver) -> String {
    let mut parts: Vec<String> = Vec::new();
    for child in children {
        let mut buf = String::new();
        transform_node(child, config, resolver, &mut buf);
        parts.push(buf);
    }

    // Trim trailing whitespace from markdown segments that precede other nodes.
    // This prevents indentation leaking from MDX source formatting.
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i < parts.len() - 1 {
            // Trim trailing whitespace (spaces/tabs) from lines, but keep newlines
            out.push_str(&trim_trailing_line_spaces(part));
        } else {
            out.push_str(part);
        }
    }

    out.trim().to_string()
}

/// Trim trailing spaces/tabs from each line (but preserve newlines).
fn trim_trailing_line_spaces(s: &str) -> String {
    s.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        + if s.ends_with('\n') { "\n" } else { "" }
}

/// Replace `{attr_name}` placeholders in a template with attribute values,
/// and `{children}` with the rendered children string.
///
/// When `{children}` expands to multiple lines and the template line has a
/// prefix before `{children}` (e.g. `> `), that prefix is applied to all
/// continuation lines of the expanded children.
fn apply_template(template: &str, attributes: &[Attribute], children: &str) -> String {
    let mut result = template.to_string();

    // Handle literal \n in templates (from TOML strings)
    result = result.replace("\\n", "\n");

    // Replace attribute placeholders first
    for attr in attributes {
        let placeholder = format!("{{{}}}", attr.name);
        let value = match &attr.value {
            Some(AttrValue::String(s)) => s.clone(),
            Some(AttrValue::Expression(e)) => e.clone(),
            None => "true".to_string(),
        };
        result = result.replace(&placeholder, &value);
    }

    // Replace {children} with line-prefix awareness
    if let Some(placeholder_pos) = result.find("{children}") {
        let before = &result[..placeholder_pos];
        let after = &result[placeholder_pos + "{children}".len()..];

        // Determine the block prefix: leading `> ` or whitespace from the line containing {children}
        let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_content = &before[line_start..];
        let prefix = extract_block_prefix(line_content);
        let is_block_prefix = !prefix.is_empty();

        if is_block_prefix && children.contains('\n') {
            let child_lines: Vec<&str> = children.lines().collect();
            let mut expanded = String::new();
            for (i, line) in child_lines.iter().enumerate() {
                if i == 0 {
                    expanded.push_str(line);
                } else {
                    expanded.push('\n');
                    if line.is_empty() {
                        // Blank line inside blockquote: just the prefix marker
                        expanded.push_str(prefix.trim_end());
                    } else {
                        expanded.push_str(&prefix);
                        expanded.push_str(line);
                    }
                }
            }
            // Handle trailing newline in children
            if children.ends_with('\n') {
                expanded.push('\n');
            }
            result = format!("{before}{expanded}{after}");
        } else {
            result = format!("{before}{children}{after}");
        }
    }

    result
}

/// Extract the repeatable block prefix from a line (e.g. `> ` from `> **warning**: text`).
/// This captures leading `>`, spaces, and tabs that form the block structure.
fn extract_block_prefix(line: &str) -> String {
    let mut prefix = String::new();
    for ch in line.chars() {
        match ch {
            '>' | ' ' | '\t' => prefix.push(ch),
            _ => break,
        }
    }
    prefix
}

/// Collapse runs of 3+ blank lines into 2 (one blank line between blocks).
fn clean_blank_lines(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut consecutive_newlines = 0u32;

    for ch in input.chars() {
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push(ch);
            }
        } else if ch == '\r' {
            // skip \r, we normalize to \n
        } else {
            consecutive_newlines = 0;
            result.push(ch);
        }
    }

    // Trim trailing whitespace
    let trimmed = result.trim_end();
    let mut final_result = trimmed.to_string();
    final_result.push('\n');
    final_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::tokenizer::tokenize;

    fn run_transform(input: &str, config: &Config) -> String {
        let tokens = tokenize(input).unwrap();
        let doc = parse(tokens).unwrap();
        transform(&doc, config)
    }

    #[test]
    fn test_strip_imports_exports() {
        let input = "import X from 'x';\nexport const y = 1;\n\n# Hello\n";
        let config = Config::default();
        let result = run_transform(input, &config);
        assert!(!result.contains("import"));
        assert!(!result.contains("export"));
        assert!(result.contains("# Hello"));
    }

    #[test]
    fn test_preserve_frontmatter() {
        let input = "---\ntitle: Test\n---\n\n# Hello\n";
        let config = Config::default();
        let result = run_transform(input, &config);
        assert!(result.contains("---\ntitle: Test\n---"));
        assert!(result.contains("# Hello"));
    }

    #[test]
    fn test_strip_frontmatter() {
        let input = "---\ntitle: Test\n---\n\n# Hello\n";
        let config = Config {
            options: Options {
                preserve_frontmatter: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = run_transform(input, &config);
        assert!(!result.contains("title: Test"));
        assert!(result.contains("# Hello"));
    }

    #[test]
    fn test_component_template() {
        let input = r#"<Callout type="warning">Watch out!</Callout>"#;
        let mut components = std::collections::HashMap::new();
        components.insert(
            "Callout".to_string(),
            ComponentTransform {
                template: "> **{type}**: {children}".to_string(),
            },
        );
        let config = Config {
            components,
            ..Default::default()
        };
        let result = run_transform(input, &config);
        assert_eq!(result.trim(), "> **warning**: Watch out!");
    }

    #[test]
    fn test_self_closing_component() {
        let input = r#"<Badge label="new" />"#;
        let mut components = std::collections::HashMap::new();
        components.insert(
            "Badge".to_string(),
            ComponentTransform {
                template: "{label}".to_string(),
            },
        );
        let config = Config {
            components,
            ..Default::default()
        };
        let result = run_transform(input, &config);
        assert_eq!(result.trim(), "new");
    }

    #[test]
    fn test_default_component() {
        let input = r#"<Unknown>fallback content</Unknown>"#;
        let mut components = std::collections::HashMap::new();
        components.insert(
            "_default".to_string(),
            ComponentTransform {
                template: "{children}".to_string(),
            },
        );
        let config = Config {
            components,
            ..Default::default()
        };
        let result = run_transform(input, &config);
        assert_eq!(result.trim(), "fallback content");
    }

    #[test]
    fn test_expression_strip() {
        let input = "The answer is {40 + 2}.";
        let config = Config::default();
        let result = run_transform(input, &config);
        assert_eq!(result.trim(), "The answer is .");
    }

    #[test]
    fn test_expression_preserve() {
        let input = "The answer is {40 + 2}.";
        let config = Config {
            options: Options {
                expression_handling: ExpressionHandling::PreserveRaw,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = run_transform(input, &config);
        assert_eq!(result.trim(), "The answer is {40 + 2}.");
    }

    #[test]
    fn test_kitchen_sink_layer1() {
        let input = std::fs::read_to_string("tests/fixtures/kitchen_sink.mdx").unwrap();
        let toml_str = std::fs::read_to_string("tests/fixtures/kitchen_sink.toml").unwrap();
        let config = Config::from_toml(&toml_str).unwrap();

        let result = run_transform(&input, &config);

        // Layer 1 checks: imports/exports stripped, JSX resolved
        assert!(!result.contains("import "), "imports should be stripped");
        assert!(!result.contains("export "), "exports should be stripped");
        assert!(result.contains("---\ntitle: Kitchen Sink"), "frontmatter preserved");
        assert!(result.contains("> **warning**"), "Callout transformed");
        assert!(result.contains("fn main() {}"), "CodeBlock content preserved");
        // Table should still be raw at this point (Layer 2 handles it)
        assert!(result.contains("| Feature"), "Table still raw after Layer 1");
        // Links should still be relative (Layer 2 handles it)
        assert!(result.contains("/docs/getting-started"), "Links still relative after Layer 1");
    }
}
