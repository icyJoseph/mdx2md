#[derive(Debug, Clone, PartialEq)]
pub enum MdxNode {
    Frontmatter(String),
    Import(String),
    Export(String),
    /// Opaque Markdown text, passed through until Layer 2
    Markdown(String),
    /// JS expression: `{some_js_expr}`
    Expression(String),
    JsxElement {
        tag: String,
        attributes: Vec<Attribute>,
        children: Vec<MdxNode>,
        self_closing: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: Option<AttrValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    String(String),
    Expression(String),
}

/// A flat document is a sequence of top-level nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct MdxDocument {
    pub nodes: Vec<MdxNode>,
}
