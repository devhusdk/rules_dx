use documentation_ir::escape_segment;
use documentation_ir::proto::{
    DocIr, Extension, Relations, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::ComponentExtractError;

/// The metadata format this converter reads.
pub const SCHEMA: &str = "dx-component-meta/1";

/// The caller-provided identity of one shard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub language: String,
    pub package: String,
    pub source_prefix: String,
    pub config: String,
}

/// Converts one metadata document into a shard, sorted by symbol id.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn convert(json: &str, settings: &Settings) -> Result<DocIr, ComponentExtractError> {
    let root: Value = serde_json::from_str(json).map_err(ComponentExtractError::Malformed)?;
    let schema = text(&root, "schema");
    if schema != SCHEMA {
        return Err(ComponentExtractError::UnsupportedSchema {
            found: schema.to_owned(),
        });
    }
    let framework = text(&root, "framework");
    if framework != settings.language {
        return Err(ComponentExtractError::WrongFramework {
            found: framework.to_owned(),
            expected: settings.language.clone(),
        });
    }
    let components = root
        .get("components")
        .and_then(Value::as_array)
        .ok_or(ComponentExtractError::ComponentsNotArray)?;
    if components.is_empty() {
        return Err(ComponentExtractError::NoComponents);
    }
    let mut names = BTreeSet::new();
    let mut symbols = Vec::new();
    for component in components {
        let name = text(component, "name");
        check_name("component", name)?;
        if name.contains('.') {
            return Err(ComponentExtractError::DottedName {
                name: name.to_owned(),
            });
        }
        if !names.insert(name.to_owned()) {
            return Err(ComponentExtractError::DuplicateComponent {
                name: name.to_owned(),
            });
        }
        component_symbols(component, name, settings, &mut symbols)?;
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(DocIr {
        language: settings.language.clone(),
        package: settings.package.clone(),
        symbols,
    })
}

fn text<'a>(node: &'a Value, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn check_name(what: &'static str, name: &str) -> Result<(), ComponentExtractError> {
    if name.is_empty() || name.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(ComponentExtractError::BadName {
            what,
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn array<'a>(node: &'a Value, key: &str) -> &'a [Value] {
    node.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn component_symbols(
    component: &Value,
    name: &str,
    settings: &Settings,
    out: &mut Vec<Symbol>,
) -> Result<(), ComponentExtractError> {
    let id = format!(
        "{}:{}:{}",
        settings.language,
        settings.package,
        escape_segment(name)
    );
    let display = match text(component, "display_name") {
        "" => name.to_owned(),
        shown => shown.to_owned(),
    };
    out.push(symbol(
        settings,
        Draft {
            id: id.clone(),
            kind: SymbolKind::Component,
            signature: format!("component {name}"),
            doc: doc(component),
            node: component,
            display: display.clone(),
            parent: None,
        },
    ));
    let groups: [(&str, &str); 4] = [
        ("props", "prop"),
        ("events", "event"),
        ("slots", "slot"),
        ("exposed", "exposed"),
    ];
    for (group, prefix) in groups {
        let mut seen = BTreeSet::new();
        for item in array(component, group) {
            let member = text(item, "name");
            check_name(prefix, member)?;
            if !seen.insert(member.to_owned()) {
                return Err(ComponentExtractError::DuplicateMember {
                    what: prefix,
                    member: member.to_owned(),
                    component: name.to_owned(),
                });
            }
            let ty = text(item, "type");
            let (kind, signature) = match prefix {
                "prop" => {
                    let optional = if item.get("required").and_then(Value::as_bool) == Some(true) {
                        ""
                    } else {
                        "?"
                    };
                    let default = match item.get("default").and_then(Value::as_str) {
                        Some(value) => format!(" = {value}"),
                        None => String::new(),
                    };
                    (
                        SymbolKind::Property,
                        format!("{member}{optional}: {ty}{default}"),
                    )
                }
                "event" => (
                    SymbolKind::Event,
                    match text(item, "signature") {
                        "" => format!("{member}: {ty}"),
                        signature => signature.to_owned(),
                    },
                ),
                "slot" => (SymbolKind::Slot, format!("{member}: {ty}")),
                _ => (
                    if item.get("callable").and_then(Value::as_bool) == Some(true) {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Field
                    },
                    format!("{member}: {ty}"),
                ),
            };
            out.push(symbol(
                settings,
                Draft {
                    id: format!("{id}.{prefix}_{}", escape_segment(member)),
                    kind,
                    signature,
                    doc: doc(item),
                    node: item,
                    display: format!("{display} {prefix}_{member}"),
                    parent: Some(id.clone()),
                },
            ));
        }
    }
    Ok(())
}

struct Draft<'a> {
    id: String,
    kind: SymbolKind,
    signature: String,
    doc: String,
    node: &'a Value,
    display: String,
    parent: Option<String>,
}

fn doc(node: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    let description = text(node, "description").trim();
    if !description.is_empty() {
        parts.push(description.to_owned());
    }
    for tag in array(node, "tags") {
        let name = text(tag, "name");
        if name.is_empty() {
            continue;
        }
        let body = text(tag, "text").trim();
        parts.push(if body.is_empty() {
            format!("**@{name}**")
        } else {
            format!("**@{name}** {body}")
        });
    }
    parts.join("\n\n")
}

fn symbol(settings: &Settings, draft: Draft<'_>) -> Symbol {
    let file = text(draft.node, "file");
    let source = if file.is_empty() {
        None
    } else {
        let line = draft.node.get("line").and_then(Value::as_u64).unwrap_or(0);
        let joined = if settings.source_prefix.is_empty() {
            file.to_owned()
        } else {
            format!("{}/{file}", settings.source_prefix)
        };
        Some(SourceRef {
            file: joined,
            line,
            generated: false,
        })
    };
    Symbol {
        id: draft.id,
        kind: draft.kind as i32,
        signature_text: draft.signature,
        doc_markdown: draft.doc,
        params: Vec::new(),
        returns: None,
        examples: Vec::new(),
        source,
        visibility: Visibility::Public as i32,
        relations: Some(Relations {
            member_of: draft.parent.into_iter().collect(),
            implements: Vec::new(),
            reexported_as: Vec::new(),
        }),
        extensions: vec![Extension {
            key: format!("{}.config", settings.language),
            value: settings.config.clone().into_bytes(),
        }],
        display_name: draft.display,
    }
}
