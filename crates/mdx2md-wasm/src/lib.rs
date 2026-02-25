use js_sys::{Function, Object, Reflect};
use mdx2md_core::config::*;
use mdx2md_core::ComponentResolver;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn convert(mdx: &str, options: JsValue) -> Result<String, JsError> {
    let (config, js_resolvers) = if options.is_undefined() || options.is_null() {
        (Config::default(), HashMap::new())
    } else {
        parse_options(&options).map_err(|e| JsError::new(&e))?
    };

    if js_resolvers.is_empty() {
        mdx2md_core::convert(mdx, &config).map_err(|e| JsError::new(&e.0))
    } else {
        let resolver = JsComponentResolver {
            callbacks: js_resolvers,
        };
        mdx2md_core::convert_with_resolver(mdx, &config, &resolver)
            .map_err(|e| JsError::new(&e.0))
    }
}

struct JsComponentResolver {
    callbacks: HashMap<String, Function>,
}

impl ComponentResolver for JsComponentResolver {
    fn resolve(
        &self,
        tag: &str,
        props: &HashMap<String, String>,
        children: &str,
    ) -> Option<String> {
        let func = self.callbacks.get(tag).or_else(|| self.callbacks.get("_default"))?;

        let js_props = Object::new();
        for (key, value) in props {
            Reflect::set(&js_props, &JsValue::from_str(key), &JsValue::from_str(value)).ok();
        }
        Reflect::set(
            &js_props,
            &JsValue::from_str("children"),
            &JsValue::from_str(children),
        )
        .ok();

        let result = func.call1(&JsValue::NULL, &js_props).ok()?;
        result.as_string()
    }
}

/// Parse the JS options object into a Config + map of JS function callbacks.
fn parse_options(options: &JsValue) -> Result<(Config, HashMap<String, Function>), String> {
    let mut config = Config::default();
    let mut js_resolvers: HashMap<String, Function> = HashMap::new();

    // Parse top-level options
    if let Some(v) = get_bool(options, "stripImports") {
        config.options.strip_imports = v;
    }
    if let Some(v) = get_bool(options, "stripExports") {
        config.options.strip_exports = v;
    }
    if let Some(v) = get_bool(options, "preserveFrontmatter") {
        config.options.preserve_frontmatter = v;
    }
    if let Some(v) = get_string(options, "expressionHandling") {
        config.options.expression_handling = match v.as_str() {
            "strip" => ExpressionHandling::Strip,
            "preserve" => ExpressionHandling::PreserveRaw,
            "placeholder" => ExpressionHandling::Placeholder,
            _ => ExpressionHandling::Strip,
        };
    }

    // Parse components
    if let Ok(components_val) = Reflect::get(options, &JsValue::from_str("components")) {
        if !components_val.is_undefined() && !components_val.is_null() {
            let components_obj: Object = components_val.unchecked_into();
            let keys = Object::keys(&components_obj);
            for i in 0..keys.length() {
                let key = keys.get(i);
                let key_str = key.as_string().unwrap_or_default();
                let val = Reflect::get(&components_obj, &key).unwrap_or(JsValue::UNDEFINED);

                if let Some(template) = val.as_string() {
                    config.components.insert(
                        key_str,
                        ComponentTransform { template },
                    );
                } else if val.is_function() {
                    let func: Function = val.unchecked_into();
                    js_resolvers.insert(key_str, func);
                }
            }
        }
    }

    // Parse markdown rewrites
    if let Ok(md_val) = Reflect::get(options, &JsValue::from_str("markdown")) {
        if !md_val.is_undefined() && !md_val.is_null() {
            // Tables
            if let Some(tables_str) = get_string(&md_val, "tables") {
                config.markdown.tables = Some(TableRewrite {
                    format: match tables_str.as_str() {
                        "list" => TableFormat::List,
                        _ => TableFormat::Preserve,
                    },
                });
            }

            // Links
            if let Ok(links_val) = Reflect::get(&md_val, &JsValue::from_str("links")) {
                if !links_val.is_undefined() && !links_val.is_null() {
                    config.markdown.links = Some(LinkRewrite {
                        make_absolute: get_bool(&links_val, "makeAbsolute").unwrap_or(false),
                        base_url: get_string(&links_val, "baseUrl").unwrap_or_default(),
                    });
                }
            }

            // Images
            if let Ok(images_val) = Reflect::get(&md_val, &JsValue::from_str("images")) {
                if !images_val.is_undefined() && !images_val.is_null() {
                    config.markdown.images = Some(ImageRewrite {
                        make_absolute: get_bool(&images_val, "makeAbsolute").unwrap_or(false),
                        base_url: get_string(&images_val, "baseUrl").unwrap_or_default(),
                    });
                }
            }
        }
    }

    Ok((config, js_resolvers))
}

fn get_string(obj: &JsValue, key: &str) -> Option<String> {
    Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

fn get_bool(obj: &JsValue, key: &str) -> Option<bool> {
    Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_bool())
}
