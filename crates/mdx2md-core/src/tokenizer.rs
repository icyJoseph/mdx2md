#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Frontmatter(String),
    Import(String),
    Export(String),
    JsxOpenTag {
        tag: String,
        attributes: Vec<RawAttribute>,
        self_closing: bool,
    },
    JsxCloseTag {
        tag: String,
    },
    Expression(String),
    Markdown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawAttribute {
    pub name: String,
    pub value: Option<RawAttrValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawAttrValue {
    String(String),
    Expression(String),
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, TokenizeError> {
    let mut tokens = Vec::new();
    let mut chars: &str = input;
    let mut md_buf = String::new();

    // Handle frontmatter at the very start
    if chars.starts_with("---\n") || chars.starts_with("---\r\n") {
        let after_open = skip_past_newline(chars, 3);
        if let Some(end) = find_frontmatter_close(after_open) {
            let fm_content = &after_open[..end];
            let after_close = skip_past_newline(&after_open[end + 3..], 0);
            tokens.push(Token::Frontmatter(fm_content.trim_end().to_string()));
            chars = after_close;
        }
    }

    while !chars.is_empty() {
        // Check for import/export at line start
        if is_at_line_start(&md_buf) {
            if let Some((stmt, rest)) = try_parse_import_export(chars) {
                flush_md(&mut md_buf, &mut tokens);
                tokens.push(stmt);
                chars = rest;
                continue;
            }
        }

        // Check for JSX tag: `<ComponentName` or `</ComponentName`
        if chars.starts_with('<') {
            if let Some((tag_token, rest)) = try_parse_jsx_tag(chars) {
                flush_md(&mut md_buf, &mut tokens);
                tokens.push(tag_token);
                chars = rest;
                continue;
            }
        }

        // Check for expression block `{...}`
        if chars.starts_with('{') {
            if let Some((expr, rest)) = try_parse_expression(chars) {
                flush_md(&mut md_buf, &mut tokens);
                tokens.push(expr);
                chars = rest;
                continue;
            }
        }

        // Otherwise, consume one character as Markdown (safe for multi-byte UTF-8)
        let c = chars.chars().next().unwrap();
        md_buf.push(c);
        chars = &chars[c.len_utf8()..];
    }

    flush_md(&mut md_buf, &mut tokens);
    Ok(tokens)
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenizeError {
    pub message: String,
}

impl std::fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tokenize error: {}", self.message)
    }
}

impl std::error::Error for TokenizeError {}

fn flush_md(buf: &mut String, tokens: &mut Vec<Token>) {
    if !buf.is_empty() {
        tokens.push(Token::Markdown(std::mem::take(buf)));
    }
}

fn is_at_line_start(md_buf: &str) -> bool {
    md_buf.is_empty() || md_buf.ends_with('\n')
}

fn skip_past_newline(s: &str, offset: usize) -> &str {
    let s = &s[offset..];
    if let Some(pos) = s.find('\n') {
        &s[pos + 1..]
    } else {
        &s[s.len()..]
    }
}

fn find_frontmatter_close(s: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < s.len() {
        if let Some(idx) = s[pos..].find("---") {
            let abs = pos + idx;
            if abs == 0 || s.as_bytes()[abs - 1] == b'\n' {
                return Some(abs);
            }
            pos = abs + 3;
        } else {
            return None;
        }
    }
    None
}

fn try_parse_import_export(s: &str) -> Option<(Token, &str)> {
    let is_import = s.starts_with("import ");
    let is_export = s.starts_with("export ");

    if !is_import && !is_export {
        return None;
    }

    // Peek ahead to see if this looks like a JS import/export (not an HTML tag or MD)
    let keyword_len = if is_import { 7 } else { 7 };
    let rest_after_keyword = &s[keyword_len..];

    // `export default` is also an export
    // For imports: `import X from`, `import { X } from`, `import "x"`
    // For exports: `export const`, `export default`, `export function`, `export {`
    if is_import {
        let first_char = rest_after_keyword.chars().next()?;
        if !first_char.is_alphabetic() && first_char != '{' && first_char != '*' && first_char != '"' && first_char != '\'' {
            return None;
        }
    } else {
        let first_char = rest_after_keyword.chars().next()?;
        if !first_char.is_alphabetic() && first_char != '{' && first_char != '*' {
            return None;
        }
    }

    // Consume until end of statement. Handle multi-line imports/exports with braces.
    let mut depth = 0i32;
    let mut i = 0;
    let bytes = s.as_bytes();
    let mut in_string: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match in_string {
            Some(quote) => {
                if b == quote && (i == 0 || bytes[i - 1] != b'\\') {
                    in_string = None;
                }
            }
            None => match b {
                b'"' | b'\'' | b'`' => in_string = Some(b),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        // Check if next non-whitespace is a newline or semicolon
                        let after = &s[i + 1..];
                        let trimmed = after.trim_start_matches(|c: char| c == ' ' || c == '\t');
                        if trimmed.starts_with('\n') || trimmed.starts_with('\r') || trimmed.starts_with(';') || trimmed.is_empty() {
                            let end = if trimmed.starts_with(';') {
                                s.len() - trimmed.len() + 1
                            } else {
                                i + 1
                            };
                            let stmt_text = s[..end].trim_end().to_string();
                            let rest = consume_newline(&s[end..]);
                            let token = if is_import {
                                Token::Import(stmt_text)
                            } else {
                                Token::Export(stmt_text)
                            };
                            return Some((token, rest));
                        }
                    }
                }
                b'\n' if depth == 0 => {
                    let stmt_text = s[..i].trim_end().to_string();
                    let rest = &s[i + 1..];
                    let token = if is_import {
                        Token::Import(stmt_text)
                    } else {
                        Token::Export(stmt_text)
                    };
                    return Some((token, rest));
                }
                b';' if depth == 0 => {
                    let stmt_text = s[..=i].trim_end().to_string();
                    let rest = consume_newline(&s[i + 1..]);
                    let token = if is_import {
                        Token::Import(stmt_text)
                    } else {
                        Token::Export(stmt_text)
                    };
                    return Some((token, rest));
                }
                _ => {}
            },
        }
        i += 1;
    }

    // Reached EOF
    if depth == 0 {
        let stmt_text = s.trim_end().to_string();
        let token = if is_import {
            Token::Import(stmt_text)
        } else {
            Token::Export(stmt_text)
        };
        return Some((token, &s[s.len()..]));
    }

    None
}

fn consume_newline(s: &str) -> &str {
    if s.starts_with("\r\n") {
        &s[2..]
    } else if s.starts_with('\n') {
        &s[1..]
    } else {
        s
    }
}

/// Try to parse a JSX/HTML tag starting with `<`.
/// Matches: `<Tag ...>`, `<Tag ... />`, `</Tag>`, `<h1>`, `<br />`, etc.
/// Tag names must start with an ASCII letter (upper or lowercase).
/// Does NOT match `<!-- comments -->` (next char is `!`) or
/// `<http://url>` autolinks (`:` is not a valid attribute/close position).
fn try_parse_jsx_tag(s: &str) -> Option<(Token, &str)> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }

    let mut pos = 1; // skip `<`
    let is_closing = bytes[pos] == b'/';
    if is_closing {
        pos += 1;
    }

    if pos >= bytes.len() {
        return None;
    }

    let first = bytes[pos];
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let tag_start = pos;
    while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_' || bytes[pos] == b'.' || bytes[pos] == b'-') {
        pos += 1;
    }
    let tag_name = std::str::from_utf8(&bytes[tag_start..pos]).ok()?.to_string();

    if is_closing {
        // Expect `>`
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos < bytes.len() && bytes[pos] == b'>' {
            return Some((Token::JsxCloseTag { tag: tag_name }, &s[pos + 1..]));
        }
        return None;
    }

    // Parse attributes
    let mut attributes = Vec::new();
    loop {
        // Skip whitespace
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t' || bytes[pos] == b'\n' || bytes[pos] == b'\r') {
            pos += 1;
        }

        if pos >= bytes.len() {
            return None;
        }

        // Self-closing `/>` or closing `>`
        if bytes[pos] == b'/' && pos + 1 < bytes.len() && bytes[pos + 1] == b'>' {
            return Some((
                Token::JsxOpenTag {
                    tag: tag_name,
                    attributes,
                    self_closing: true,
                },
                &s[pos + 2..],
            ));
        }
        if bytes[pos] == b'>' {
            return Some((
                Token::JsxOpenTag {
                    tag: tag_name,
                    attributes,
                    self_closing: false,
                },
                &s[pos + 1..],
            ));
        }

        // Parse attribute name
        if !bytes[pos].is_ascii_alphabetic() && bytes[pos] != b'_' {
            return None;
        }
        let attr_start = pos;
        while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_' || bytes[pos] == b'-') {
            pos += 1;
        }
        let attr_name = std::str::from_utf8(&bytes[attr_start..pos]).ok()?.to_string();

        // Check for `=`
        if pos < bytes.len() && bytes[pos] == b'=' {
            pos += 1;
            if pos >= bytes.len() {
                return None;
            }

            if bytes[pos] == b'"' || bytes[pos] == b'\'' {
                let quote = bytes[pos];
                pos += 1;
                let val_start = pos;
                while pos < bytes.len() && bytes[pos] != quote {
                    if bytes[pos] == b'\\' {
                        pos += 1; // skip escaped char
                    }
                    pos += 1;
                }
                if pos >= bytes.len() {
                    return None;
                }
                let val = std::str::from_utf8(&bytes[val_start..pos]).ok()?.to_string();
                pos += 1; // skip closing quote
                attributes.push(RawAttribute {
                    name: attr_name,
                    value: Some(RawAttrValue::String(val)),
                });
            } else if bytes[pos] == b'{' {
                // Expression attribute value
                let (expr_content, rest) = parse_braced_expression(&s[pos..])?;
                pos = s.len() - rest.len();
                attributes.push(RawAttribute {
                    name: attr_name,
                    value: Some(RawAttrValue::Expression(expr_content)),
                });
            } else {
                return None;
            }
        } else {
            // Shorthand boolean attribute
            attributes.push(RawAttribute {
                name: attr_name,
                value: None,
            });
        }
    }
}

/// Parse a `{...}` expression, tracking brace depth.
/// Empty braces `{}` are not treated as expressions (likely literal code).
fn try_parse_expression(s: &str) -> Option<(Token, &str)> {
    let (content, rest) = parse_braced_expression(s)?;
    if content.trim().is_empty() {
        return None;
    }
    Some((Token::Expression(content), rest))
}

fn parse_braced_expression(s: &str) -> Option<(String, &str)> {
    if !s.starts_with('{') {
        return None;
    }
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    let mut in_string: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match in_string {
            Some(quote) => {
                if b == quote && (i == 0 || bytes[i - 1] != b'\\') {
                    in_string = None;
                }
            }
            None => match b {
                b'"' | b'\'' | b'`' => in_string = Some(b),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        let content = &s[1..i];
                        let rest = &s[i + 1..];
                        return Some((content.to_string(), rest));
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontmatter() {
        let input = "---\ntitle: Hello\nauthor: Test\n---\n\n# Content\n";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens[0], Token::Frontmatter("title: Hello\nauthor: Test".to_string()));
        assert!(matches!(&tokens[1], Token::Markdown(s) if s.contains("# Content")));
    }

    #[test]
    fn test_import() {
        let input = "import { Callout } from './components';\n\n# Hello\n";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens[0], Token::Import("import { Callout } from './components';".to_string()));
        assert!(matches!(&tokens[1], Token::Markdown(_)));
    }

    #[test]
    fn test_export() {
        let input = "export const meta = { draft: true };\n\n# Hello\n";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens[0], Token::Export("export const meta = { draft: true };".to_string()));
    }

    #[test]
    fn test_export_default_multiline() {
        let input = "export default function Layout({ children }) {\n  return <main>{children}</main>;\n}\n";
        let tokens = tokenize(input).unwrap();
        assert!(matches!(&tokens[0], Token::Export(s) if s.starts_with("export default")));
    }

    #[test]
    fn test_jsx_self_closing() {
        let input = r#"<Badge label="new" />"#;
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens[0],
            Token::JsxOpenTag {
                tag: "Badge".to_string(),
                attributes: vec![RawAttribute {
                    name: "label".to_string(),
                    value: Some(RawAttrValue::String("new".to_string())),
                }],
                self_closing: true,
            }
        );
    }

    #[test]
    fn test_jsx_open_close() {
        let input = r#"<Callout type="warning">content</Callout>"#;
        let tokens = tokenize(input).unwrap();
        assert!(matches!(&tokens[0], Token::JsxOpenTag { tag, self_closing: false, .. } if tag == "Callout"));
        assert!(matches!(&tokens[1], Token::Markdown(s) if s == "content"));
        assert!(matches!(&tokens[2], Token::JsxCloseTag { tag } if tag == "Callout"));
    }

    #[test]
    fn test_jsx_boolean_attribute() {
        let input = "<Modal open />";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens[0],
            Token::JsxOpenTag {
                tag: "Modal".to_string(),
                attributes: vec![RawAttribute {
                    name: "open".to_string(),
                    value: None,
                }],
                self_closing: true,
            }
        );
    }

    #[test]
    fn test_expression() {
        let input = "The answer is {40 + 2}.";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0], Token::Markdown(s) if s == "The answer is "));
        assert_eq!(tokens[1], Token::Expression("40 + 2".to_string()));
        assert!(matches!(&tokens[2], Token::Markdown(s) if s == "."));
    }

    #[test]
    fn test_nested_braces_in_expression() {
        let input = "{obj.map(x => { return x; })}";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens[0], Token::Expression("obj.map(x => { return x; })".to_string()));
    }

    #[test]
    fn test_markdown_passthrough() {
        let input = "# Hello\n\nA paragraph with **bold** and *italic*.\n";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Markdown(s) if s == input));
    }

    #[test]
    fn test_lowercase_tags_are_jsx() {
        let input = "<div>hello</div>";
        let tokens = tokenize(input).unwrap();
        assert!(matches!(&tokens[0], Token::JsxOpenTag { tag, self_closing: false, .. } if tag == "div"));
        assert!(matches!(&tokens[1], Token::Markdown(s) if s == "hello"));
        assert!(matches!(&tokens[2], Token::JsxCloseTag { tag } if tag == "div"));
    }

    #[test]
    fn test_html_self_closing_tag() {
        let input = "<br />";
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens[0],
            Token::JsxOpenTag {
                tag: "br".to_string(),
                attributes: vec![],
                self_closing: true,
            }
        );
    }

    #[test]
    fn test_html_tag_with_attributes() {
        let input = r#"<a href="https://example.com">link</a>"#;
        let tokens = tokenize(input).unwrap();
        assert!(matches!(&tokens[0], Token::JsxOpenTag { tag, attributes, self_closing: false }
            if tag == "a" && attributes.len() == 1 && attributes[0].name == "href"));
        assert!(matches!(&tokens[1], Token::Markdown(s) if s == "link"));
        assert!(matches!(&tokens[2], Token::JsxCloseTag { tag } if tag == "a"));
    }

    #[test]
    fn test_html_comment_not_parsed_as_tag() {
        let input = "<!-- this is a comment -->";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Markdown(_)));
    }

    #[test]
    fn test_autolink_not_parsed_as_tag() {
        let input = "<http://example.com>";
        let tokens = tokenize(input).unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Markdown(_)));
    }

    #[test]
    fn test_expression_attr_value() {
        let input = r#"<Comp value={42} />"#;
        let tokens = tokenize(input).unwrap();
        assert_eq!(
            tokens[0],
            Token::JsxOpenTag {
                tag: "Comp".to_string(),
                attributes: vec![RawAttribute {
                    name: "value".to_string(),
                    value: Some(RawAttrValue::Expression("42".to_string())),
                }],
                self_closing: true,
            }
        );
    }

    #[test]
    fn test_kitchen_sink_token_types() {
        let input = r#"---
title: Kitchen Sink
author: Test
---

import { Callout } from './components';
import CodeBlock from './CodeBlock';
export const meta = { draft: true };

# Welcome

This is a paragraph with an [internal link](/docs/getting-started) and an
![image](/assets/logo.png "Logo").

<Callout type="warning">
  Watch out for **bold** and *italic* inside JSX.

  <CodeBlock language="rust">
    fn main() {}
  </CodeBlock>
</Callout>

Here is an inline component: <Badge label="new" />.

The answer is {40 + 2}.

## Data Table

| Feature     | Status |
|-------------|--------|
| Frontmatter | Done   |
| JSX         | Done   |
| Tables      | Done   |

export default function Layout({ children }) {
  return <main>{children}</main>;
}
"#;
        let tokens = tokenize(input).unwrap();

        let types: Vec<&str> = tokens
            .iter()
            .map(|t| match t {
                Token::Frontmatter(_) => "Frontmatter",
                Token::Import(_) => "Import",
                Token::Export(_) => "Export",
                Token::JsxOpenTag { .. } => "JsxOpen",
                Token::JsxCloseTag { .. } => "JsxClose",
                Token::Expression(_) => "Expression",
                Token::Markdown(_) => "Markdown",
            })
            .collect();

        // Verify key token types are present
        assert!(types.contains(&"Frontmatter"), "Should have frontmatter");
        assert!(types.iter().filter(|&&t| t == "Import").count() == 2, "Should have 2 imports");
        assert!(types.contains(&"Export"), "Should have export");
        assert!(types.contains(&"JsxOpen"), "Should have JSX open tags");
        assert!(types.contains(&"JsxClose"), "Should have JSX close tags");
        assert!(types.contains(&"Expression"), "Should have expression");
        assert!(types.contains(&"Markdown"), "Should have markdown");

        // Verify frontmatter content
        assert!(matches!(&tokens[0], Token::Frontmatter(s) if s.contains("title: Kitchen Sink")));
    }
}
