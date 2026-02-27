use crate::ast::*;
use crate::tokenizer::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(tokens: Vec<Token>) -> Result<MdxDocument, ParseError> {
    let mut parser = Parser::new(tokens);
    let nodes = parser.parse_nodes(None)?;
    Ok(MdxDocument { nodes })
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    /// Parse nodes until we hit a closing tag matching `until_close` or EOF.
    fn parse_nodes(&mut self, until_close: Option<&str>) -> Result<Vec<MdxNode>, ParseError> {
        let mut nodes = Vec::new();

        while let Some(token) = self.peek() {
            match token {
                Token::JsxCloseTag { tag } => {
                    let tag = tag.clone();
                    if let Some(expected) = until_close {
                        if tag == expected {
                            self.next(); // consume the close tag
                            return Ok(nodes);
                        }
                        return Err(ParseError {
                            message: format!(
                                "Unexpected closing tag </{tag}>, expected </{expected}>"
                            ),
                        });
                    }
                    return Err(ParseError {
                        message: format!("Unexpected closing tag </{tag}> with no matching open tag"),
                    });
                }
                _ => {
                    let node = self.parse_node()?;
                    nodes.push(node);
                }
            }
        }

        if let Some(expected) = until_close {
            return Err(ParseError {
                message: format!("Unclosed JSX element <{expected}>: reached end of input"),
            });
        }

        Ok(nodes)
    }

    fn parse_node(&mut self) -> Result<MdxNode, ParseError> {
        let token = self.next().ok_or_else(|| ParseError {
            message: "Unexpected end of input".to_string(),
        })?;

        match token {
            Token::Frontmatter(content) => Ok(MdxNode::Frontmatter(content)),
            Token::Import(content) => Ok(MdxNode::Import(content)),
            Token::Export(content) => Ok(MdxNode::Export(content)),
            Token::Markdown(content) => Ok(MdxNode::Markdown(content)),
            Token::Expression(content) => Ok(MdxNode::Expression(content)),
            Token::JsxOpenTag {
                tag,
                attributes,
                self_closing,
            } => {
                let attrs = attributes
                    .into_iter()
                    .map(|a| Attribute {
                        name: a.name,
                        value: a.value.map(|v| match v {
                            RawAttrValue::String(s) => AttrValue::String(s),
                            RawAttrValue::Expression(e) => AttrValue::Expression(e),
                        }),
                    })
                    .collect();

                if self_closing {
                    Ok(MdxNode::JsxElement {
                        tag,
                        attributes: attrs,
                        children: vec![],
                        self_closing: true,
                    })
                } else {
                    let children = self.parse_nodes(Some(&tag))?;
                    Ok(MdxNode::JsxElement {
                        tag,
                        attributes: attrs,
                        children,
                        self_closing: false,
                    })
                }
            }
            Token::JsxCloseTag { tag } => Err(ParseError {
                message: format!("Unexpected closing tag </{tag}>"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::tokenize;

    fn parse_str(input: &str) -> Result<MdxDocument, ParseError> {
        let tokens = tokenize(input).map_err(|e| ParseError {
            message: e.message,
        })?;
        parse(tokens)
    }

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
    }

    #[test]
    fn test_simple_markdown() {
        let doc = parse_str("# Hello\n\nWorld\n").unwrap();
        assert_eq!(doc.nodes.len(), 1);
        assert!(matches!(&doc.nodes[0], MdxNode::Markdown(_)));
    }

    #[test]
    fn test_frontmatter_and_markdown() {
        let doc = parse_str("---\ntitle: Test\n---\n\n# Hello\n").unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert!(matches!(&doc.nodes[0], MdxNode::Frontmatter(s) if s.contains("title: Test")));
        assert!(matches!(&doc.nodes[1], MdxNode::Markdown(_)));
    }

    #[test]
    fn test_self_closing_jsx() {
        let doc = parse_str(r#"<Badge label="new" />"#).unwrap();
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            MdxNode::JsxElement {
                tag,
                attributes,
                self_closing,
                children,
            } => {
                assert_eq!(tag, "Badge");
                assert!(self_closing);
                assert!(children.is_empty());
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].name, "label");
                assert_eq!(
                    attributes[0].value,
                    Some(AttrValue::String("new".to_string()))
                );
            }
            _ => panic!("Expected JsxElement"),
        }
    }

    #[test]
    fn test_jsx_with_children() {
        let doc = parse_str(r#"<Callout type="warning">Watch out!</Callout>"#).unwrap();
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            MdxNode::JsxElement {
                tag,
                children,
                self_closing,
                ..
            } => {
                assert_eq!(tag, "Callout");
                assert!(!self_closing);
                assert_eq!(children.len(), 1);
                assert!(matches!(&children[0], MdxNode::Markdown(s) if s == "Watch out!"));
            }
            _ => panic!("Expected JsxElement"),
        }
    }

    #[test]
    fn test_nested_jsx() {
        let input = r#"<Outer><Inner>text</Inner></Outer>"#;
        let doc = parse_str(input).unwrap();
        assert_eq!(doc.nodes.len(), 1);
        match &doc.nodes[0] {
            MdxNode::JsxElement { tag, children, .. } => {
                assert_eq!(tag, "Outer");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    MdxNode::JsxElement {
                        tag, children: inner_children, ..
                    } => {
                        assert_eq!(tag, "Inner");
                        assert_eq!(inner_children.len(), 1);
                        assert!(matches!(&inner_children[0], MdxNode::Markdown(s) if s == "text"));
                    }
                    _ => panic!("Expected inner JsxElement"),
                }
            }
            _ => panic!("Expected outer JsxElement"),
        }
    }

    #[test]
    fn test_jsx_with_markdown_and_expression_children() {
        let input = r#"<Wrapper>Hello {name} world</Wrapper>"#;
        let doc = parse_str(input).unwrap();
        match &doc.nodes[0] {
            MdxNode::JsxElement { children, .. } => {
                assert_eq!(children.len(), 3);
                assert!(matches!(&children[0], MdxNode::Markdown(s) if s == "Hello "));
                assert!(matches!(&children[1], MdxNode::Expression(s) if s == "name"));
                assert!(matches!(&children[2], MdxNode::Markdown(s) if s == " world"));
            }
            _ => panic!("Expected JsxElement"),
        }
    }

    #[test]
    fn test_mismatched_tags() {
        let result = parse_str("<Outer>text</Inner>");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unexpected closing tag </Inner>"));
    }

    #[test]
    fn test_unclosed_element() {
        let result = parse_str("<Outer>text");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unclosed JSX element <Outer>"));
    }

    #[test]
    fn test_unexpected_close_tag() {
        let result = parse_str("text</Outer>");
        assert!(result.is_err());
    }

    #[test]
    fn test_kitchen_sink_ast() {
        let input = std::fs::read_to_string(fixture_path("kitchen_sink.mdx")).unwrap();
        let doc = parse_str(&input).unwrap();

        // Should have: Frontmatter, Import, Import, Export, Markdown, JsxElement(Callout), Markdown, Markdown(expression area), Markdown(table), Export
        let mut found_frontmatter = false;
        let mut found_callout = false;
        let mut import_count = 0;
        let mut export_count = 0;

        for node in &doc.nodes {
            match node {
                MdxNode::Frontmatter(s) => {
                    assert!(s.contains("title: Kitchen Sink"));
                    found_frontmatter = true;
                }
                MdxNode::Import(_) => import_count += 1,
                MdxNode::Export(_) => export_count += 1,
                MdxNode::JsxElement { tag, children, .. } if tag == "Callout" => {
                    found_callout = true;
                    // Callout should contain nested CodeBlock
                    let has_codeblock = children.iter().any(|c| {
                        matches!(c, MdxNode::JsxElement { tag, .. } if tag == "CodeBlock")
                    });
                    assert!(has_codeblock, "Callout should contain nested CodeBlock");
                }
                _ => {}
            }
        }

        assert!(found_frontmatter, "Should have frontmatter");
        assert_eq!(import_count, 2, "Should have 2 imports");
        assert!(export_count >= 2, "Should have at least 2 exports");
        assert!(found_callout, "Should have Callout element");
    }
}
