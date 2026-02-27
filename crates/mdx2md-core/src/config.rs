use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub components: HashMap<String, ComponentTransform>,
    #[serde(default)]
    pub markdown: MarkdownRewrites,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Options {
    #[serde(default = "default_true")]
    pub strip_imports: bool,
    #[serde(default = "default_true")]
    pub strip_exports: bool,
    #[serde(default = "default_strip")]
    pub expression_handling: ExpressionHandling,
    #[serde(default = "default_true")]
    pub preserve_frontmatter: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            strip_imports: true,
            strip_exports: true,
            expression_handling: ExpressionHandling::Strip,
            preserve_frontmatter: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionHandling {
    Strip,
    PreserveRaw,
    Placeholder,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentTransform {
    pub template: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarkdownRewrites {
    #[serde(default)]
    pub tables: Option<TableRewrite>,
    #[serde(default)]
    pub links: Option<LinkRewrite>,
    #[serde(default)]
    pub images: Option<ImageRewrite>,
    #[serde(default = "default_true")]
    pub strip_html_comments: bool,
    #[serde(default = "default_true")]
    pub strip_doctype: bool,
}

impl Default for MarkdownRewrites {
    fn default() -> Self {
        Self {
            tables: None,
            links: None,
            images: None,
            strip_html_comments: false,
            strip_doctype: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableRewrite {
    #[serde(default = "default_preserve")]
    pub format: TableFormat,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TableFormat {
    Preserve,
    List,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkRewrite {
    #[serde(default)]
    pub make_absolute: bool,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub strip: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageRewrite {
    #[serde(default)]
    pub make_absolute: bool,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub strip: bool,
}

fn default_true() -> bool {
    true
}

fn default_strip() -> ExpressionHandling {
    ExpressionHandling::Strip
}

fn default_preserve() -> TableFormat {
    TableFormat::Preserve
}

impl Config {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kitchen_sink_config() {
        let toml_str = std::fs::read_to_string("tests/fixtures/kitchen_sink.toml").unwrap();
        let config = Config::from_toml(&toml_str).unwrap();

        assert!(config.options.strip_imports);
        assert!(config.options.strip_exports);
        assert_eq!(config.options.expression_handling, ExpressionHandling::Strip);
        assert!(config.options.preserve_frontmatter);

        assert!(config.components.contains_key("Callout"));
        assert!(config.components.contains_key("CodeBlock"));
        assert!(config.components.contains_key("Badge"));
        assert!(config.components.contains_key("_default"));

        let tables = config.markdown.tables.unwrap();
        assert_eq!(tables.format, TableFormat::List);

        let links = config.markdown.links.unwrap();
        assert!(links.make_absolute);
        assert_eq!(links.base_url, "https://docs.example.com");

        let images = config.markdown.images.unwrap();
        assert!(images.make_absolute);
        assert_eq!(images.base_url, "https://cdn.example.com");
    }
}
