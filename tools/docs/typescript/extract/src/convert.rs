//! Maps TypeDoc JSON onto the versioned documentation IR.

use crate::types::{
    call_signature, comment_text, flag, list, modifiers, object_literal, parameters, render_type,
    text, type_parameters,
};
use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};
use serde_json::Value;
use std::collections::BTreeMap;

const LANGUAGE: &str = "typescript";
const KEY_CONFIG: &str = "typescript.config";

const SCHEMA_VERSION: &str = "2.0";

const KIND_MODULE: u64 = 2;
const KIND_NAMESPACE: u64 = 4;
const KIND_ENUM: u64 = 8;
const KIND_ENUM_MEMBER: u64 = 16;
const KIND_VARIABLE: u64 = 32;
const KIND_FUNCTION: u64 = 64;
const KIND_CLASS: u64 = 128;
const KIND_INTERFACE: u64 = 256;
const KIND_CONSTRUCTOR: u64 = 512;
const KIND_PROPERTY: u64 = 1024;
const KIND_METHOD: u64 = 2048;
const KIND_ACCESSOR: u64 = 262144;
const KIND_TYPE_ALIAS: u64 = 2097152;
const KIND_REFERENCE: u64 = 4194304;

/// The identity and location inputs of one extraction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub package: String,
    pub source_prefix: String,
    pub config: String,
}

/// Converts one TypeDoc JSON document into a validated IR shard.
pub fn convert(json: &str, settings: &Settings) -> Result<DocIr, String> {
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("TypeDoc JSON is malformed: {error}"))?;
    if text(&root, "variant") != "project" {
        return Err("TypeDoc JSON is not a project reflection".to_owned());
    }
    match root.get("schemaVersion").and_then(Value::as_str) {
        Some(SCHEMA_VERSION) => {}
        Some(other) => {
            return Err(format!(
                "TypeDoc JSON schemaVersion '{other}' is not supported, expected '{SCHEMA_VERSION}'"
            ))
        }
        None => return Err("TypeDoc JSON has no schemaVersion".to_owned()),
    }
    let mut names = BTreeMap::new();
    index_names(&root, "", &mut names);
    let walker = Walker {
        settings,
        names,
        symbols: Vec::new(),
        pending: Vec::new(),
        aliases: Vec::new(),
    };
    let mut walker = walker;
    walker.children(&root, "")?;
    walker.references()?;
    if walker.symbols.is_empty() {
        return Err("TypeDoc found no public symbols".to_owned());
    }
    let mut symbols = walker.symbols;
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: LANGUAGE.to_owned(),
        package: settings.package.clone(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

fn index_names(node: &Value, parent: &str, names: &mut BTreeMap<i64, String>) {
    let values: Vec<&str> = list(node, "children")
        .iter()
        .filter(|child| !is_type_kind(child))
        .map(|child| text(child, "name"))
        .collect();
    let plain: Vec<&str> = list(node, "children")
        .iter()
        .filter(|child| !is_type_kind(child) && !is_namespace(child))
        .map(|child| text(child, "name"))
        .collect();
    for child in list(node, "children") {
        let mut qualified = qualify(parent, text(child, "name"));
        if is_type_kind(child) && values.contains(&text(child, "name")) {
            qualified.push_str("#type");
        } else if is_namespace(child) && plain.contains(&text(child, "name")) {
            qualified.push_str("#namespace");
        }
        if let Some(id) = child.get("id").and_then(Value::as_i64) {
            names.insert(id, qualified.clone());
        }
        index_names(child, &qualified, names);
    }
}

fn is_type_kind(node: &Value) -> bool {
    matches!(
        node.get("kind").and_then(Value::as_u64),
        Some(KIND_INTERFACE | KIND_TYPE_ALIAS)
    )
}

fn is_namespace(node: &Value) -> bool {
    node.get("kind").and_then(Value::as_u64) == Some(KIND_NAMESPACE)
}

fn qualify(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}.{name}")
    }
}

struct Walker<'a> {
    settings: &'a Settings,
    names: BTreeMap<i64, String>,
    symbols: Vec<Symbol>,
    pending: Vec<(Value, String)>,
    aliases: Vec<(String, String)>,
}

struct Draft {
    qualified: String,
    kind: SymbolKind,
    signature: String,
    parent: String,
}

impl Walker<'_> {
    fn id(&self, qualified: &str) -> String {
        format!("{LANGUAGE}:{}:{qualified}", self.settings.package)
    }

    fn children(&mut self, node: &Value, parent: &str) -> Result<(), String> {
        for child in list(node, "children") {
            if flag(child, "isPrivate") || flag(child, "isProtected") {
                continue;
            }
            self.declaration(child, parent)?;
        }
        Ok(())
    }

    fn declaration(&mut self, node: &Value, parent: &str) -> Result<(), String> {
        let name = text(node, "name");
        let qualified = node
            .get("id")
            .and_then(Value::as_i64)
            .and_then(|id| self.names.get(&id).cloned())
            .unwrap_or_else(|| qualify(parent, name));
        let kind = node.get("kind").and_then(Value::as_u64).unwrap_or(0);
        let draft = |kind: SymbolKind, signature: String| Draft {
            qualified: qualified.clone(),
            kind,
            signature,
            parent: parent.to_owned(),
        };
        match kind {
            KIND_MODULE | KIND_NAMESPACE => {
                let word = if kind == KIND_MODULE {
                    "module"
                } else {
                    "namespace"
                };
                self.push(
                    node,
                    None,
                    draft(SymbolKind::Module, format!("{word} {name}")),
                )?;
                self.children(node, &qualified)
            }
            KIND_ENUM => {
                self.push(node, None, draft(SymbolKind::Enum, format!("enum {name}")))?;
                self.children(node, &qualified)
            }
            KIND_ENUM_MEMBER => {
                let value = match node.get("type") {
                    Some(ty) => format!(" = {}", render_type(ty)?),
                    None => String::new(),
                };
                self.push(
                    node,
                    None,
                    draft(SymbolKind::EnumVariant, format!("{name}{value}")),
                )
            }
            KIND_VARIABLE => {
                let word = if flag(node, "isConst") {
                    "const"
                } else {
                    "let"
                };
                let ty = type_suffix(node)?;
                self.push(
                    node,
                    None,
                    draft(SymbolKind::Constant, format!("{word} {name}{ty}")),
                )
            }
            KIND_CLASS => {
                let text = class_text(node)?;
                let mut symbol = self.build(node, None, draft(SymbolKind::Class, text))?;
                symbol.relations = Some(Relations {
                    member_of: member_of(self, parent),
                    implements: list(node, "implementedTypes")
                        .iter()
                        .map(render_type)
                        .collect::<Result<Vec<_>, _>>()?,
                    reexported_as: Vec::new(),
                });
                self.symbols.push(symbol);
                self.children(node, &qualified)
            }
            KIND_INTERFACE => {
                let text = interface_text(node)?;
                self.push(node, None, draft(SymbolKind::Interface, text))?;
                self.children(node, &qualified)
            }
            KIND_PROPERTY => {
                let optional = if flag(node, "isOptional") { "?" } else { "" };
                let ty = type_suffix(node)?;
                let text = format!("{}{name}{optional}{ty}", modifiers(node));
                self.push(node, None, draft(SymbolKind::Property, text))
            }
            KIND_TYPE_ALIAS => {
                let generics = type_parameters(node)?;
                let body = alias_body(node)?;
                let text = format!("type {name}{generics} = {body}");
                self.push(node, None, draft(SymbolKind::TypeAlias, text))?;
                self.children(node, &qualified)
            }
            KIND_FUNCTION | KIND_METHOD | KIND_CONSTRUCTOR => self.callable(node, parent, kind),
            KIND_ACCESSOR => {
                let text = accessor_text(node)?;
                let signature = node
                    .get("getSignature")
                    .or_else(|| node.get("setSignature"));
                self.push(node, signature, draft(SymbolKind::Property, text))
            }
            KIND_REFERENCE => {
                self.pending.push((node.clone(), parent.to_owned()));
                Ok(())
            }
            other => Err(format!(
                "unsupported TypeDoc reflection kind {other} for '{qualified}'"
            )),
        }
    }

    fn callable(&mut self, node: &Value, parent: &str, kind: u64) -> Result<(), String> {
        let signatures = list(node, "signatures");
        if signatures.is_empty() {
            return Err(format!(
                "TypeDoc reflection '{}' has no signatures",
                qualify(parent, text(node, "name"))
            ));
        }
        let name = text(node, "name");
        let qualified = qualify(parent, name);
        let symbol_kind = if kind == KIND_FUNCTION {
            SymbolKind::Function
        } else {
            SymbolKind::Method
        };
        for (index, signature) in signatures.iter().enumerate() {
            let line = match kind {
                KIND_FUNCTION => format!(
                    "{}function {}",
                    if flag(node, "isAbstract") {
                        "abstract "
                    } else {
                        ""
                    },
                    call_signature(signature)?
                ),
                KIND_CONSTRUCTOR => format!(
                    "constructor{}{}",
                    type_parameters(signature)?,
                    parameters(signature)?
                ),
                _ => format!("{}{}", modifiers(node), method_signature(node, signature)?),
            };
            let suffixed = if signatures.len() > 1 {
                format!("{qualified}#{}", index + 1)
            } else {
                qualified.clone()
            };
            let draft = Draft {
                qualified: suffixed,
                kind: symbol_kind,
                signature: line,
                parent: parent.to_owned(),
            };
            let mut symbol = self.build(node, Some(signature), draft)?;
            if kind != KIND_CONSTRUCTOR {
                if let Some(ty) = signature.get("type") {
                    let returns_doc = block(signature, "@returns").unwrap_or_default();
                    symbol.returns = Some(Returns {
                        r#type: render_type(ty)?,
                        doc: returns_doc,
                    });
                }
            }
            symbol.params = list(signature, "parameters")
                .iter()
                .map(|parameter| {
                    Ok(Param {
                        name: text(parameter, "name").to_owned(),
                        r#type: render_type(parameter.get("type").ok_or_else(|| {
                            format!("parameter '{}' has no type", text(parameter, "name"))
                        })?)?,
                        doc: summary(parameter),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            self.symbols.push(symbol);
        }
        Ok(())
    }

    fn references(&mut self) -> Result<(), String> {
        let pending = std::mem::take(&mut self.pending);
        for (node, parent) in pending.iter() {
            self.reference(node, parent)?;
        }
        self.link_aliases()?;
        Ok(())
    }

    /// Records one re-export as an alias id on its target. The site renders the alias pages.
    fn reference(&mut self, node: &Value, parent: &str) -> Result<(), String> {
        let name = text(node, "name");
        let target = node
            .get("target")
            .and_then(Value::as_i64)
            .and_then(|id| self.names.get(&id).cloned())
            .ok_or_else(|| {
                format!(
                    "re-export '{}' does not resolve to a documented symbol",
                    qualify(parent, name)
                )
            })?;
        let mut target_id = self.id(&target);
        if !self.symbols.iter().any(|s| s.id == target_id) {
            let first = format!("{target_id}#1");
            if self.symbols.iter().any(|s| s.id == first) {
                target_id = first;
            }
        }
        let alias = self.id(&qualify(parent, name));
        self.aliases.push((target_id, alias));
        Ok(())
    }

    /// Adds every alias to its target's `reexported_as`, sorted and unique.
    fn link_aliases(&mut self) -> Result<(), String> {
        for (target, alias) in std::mem::take(&mut self.aliases) {
            let Some(found) = self.symbols.iter_mut().find(|s| s.id == target) else {
                return Err(format!(
                    "re-export '{alias}' targets unknown symbol '{target}'"
                ));
            };
            let relations = found.relations.get_or_insert_with(Relations::default);
            relations.reexported_as.push(alias);
            relations.reexported_as.sort();
            relations.reexported_as.dedup();
        }
        Ok(())
    }

    fn push(
        &mut self,
        node: &Value,
        signature: Option<&Value>,
        draft: Draft,
    ) -> Result<(), String> {
        let symbol = self.build(node, signature, draft)?;
        self.symbols.push(symbol);
        Ok(())
    }

    fn build(
        &self,
        node: &Value,
        signature: Option<&Value>,
        draft: Draft,
    ) -> Result<Symbol, String> {
        let commented = signature.unwrap_or(node);
        let comment = commented.get("comment").or_else(|| node.get("comment"));
        let mut doc = comment
            .map(|comment| comment_text(list(comment, "summary")))
            .unwrap_or_default();
        let mut examples = Vec::new();
        if let Some(comment) = comment {
            for tag in list(comment, "blockTags") {
                let body = comment_text(list(tag, "content"));
                match text(tag, "tag") {
                    "@example" => examples.push(body),
                    "@deprecated" => doc.push_str(&format!("\n\n**Deprecated:** {body}")),
                    "@remarks" => doc.push_str(&format!("\n\n{body}")),
                    _ => {}
                }
            }
        }
        let doc = doc.trim().to_owned();
        let source = signature
            .and_then(|s| list(s, "sources").first())
            .or_else(|| list(node, "sources").first())
            .ok_or_else(|| format!("TypeDoc symbol '{}' has no sources entry", draft.qualified))
            .and_then(|source| self.source(source))
            .map(Some)?;
        let parent_ids = member_of(self, &draft.parent);
        Ok(Symbol {
            id: self.id(&draft.qualified),
            kind: draft.kind as i32,
            signature_text: draft.signature,
            doc_markdown: doc,
            params: Vec::new(),
            returns: None,
            examples,
            source,
            visibility: Visibility::Public as i32,
            relations: Some(Relations {
                member_of: parent_ids,
                implements: Vec::new(),
                reexported_as: Vec::new(),
            }),
            extensions: vec![Extension {
                key: KEY_CONFIG.to_owned(),
                value: self.settings.config.clone().into_bytes(),
            }],
            display_name: String::new(),
        })
    }

    fn source(&self, source: &Value) -> Result<SourceRef, String> {
        let file = text(source, "fileName");
        if file.is_empty() {
            return Err("TypeDoc source has no fileName".to_owned());
        }
        let line = source.get("line").and_then(Value::as_u64).unwrap_or(0);
        let prefix = &self.settings.source_prefix;
        let joined = if prefix.is_empty() {
            file.to_owned()
        } else {
            format!("{prefix}/{file}")
        };
        Ok(SourceRef {
            file: joined,
            line,
            generated: false,
        })
    }
}

fn member_of(walker: &Walker<'_>, parent: &str) -> Vec<String> {
    if parent.is_empty() {
        Vec::new()
    } else {
        vec![walker.id(parent)]
    }
}

fn summary(node: &Value) -> String {
    node.get("comment")
        .map(|comment| comment_text(list(comment, "summary")))
        .unwrap_or_default()
}

fn block(node: &Value, tag: &str) -> Option<String> {
    let comment = node.get("comment")?;
    list(comment, "blockTags")
        .iter()
        .find(|candidate| text(candidate, "tag") == tag)
        .map(|found| comment_text(list(found, "content")))
}

fn type_suffix(node: &Value) -> Result<String, String> {
    match node.get("type") {
        Some(ty) => Ok(format!(": {}", render_type(ty)?)),
        None => Ok(String::new()),
    }
}

fn heritage(node: &Value, key: &str, word: &str) -> Result<String, String> {
    let items = list(node, key)
        .iter()
        .map(render_type)
        .collect::<Result<Vec<_>, _>>()?;
    if items.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!(" {word} {}", items.join(", ")))
    }
}

fn body_signatures(node: &Value) -> Result<String, String> {
    let mut lines = Vec::new();
    for signature in list(node, "signatures") {
        lines.push(call_signature(signature)?);
    }
    for index in list(node, "indexSignatures") {
        let parameter = list(index, "parameters")
            .first()
            .ok_or_else(|| "TypeDoc index signature has no key".to_owned())?;
        let key = parameter
            .get("type")
            .ok_or_else(|| "TypeDoc index signature key has no type".to_owned())?;
        let value = index
            .get("type")
            .ok_or_else(|| "TypeDoc index signature has no type".to_owned())?;
        lines.push(format!(
            "[{}: {}]: {}",
            text(parameter, "name"),
            render_type(key)?,
            render_type(value)?
        ));
    }
    if lines.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(" {{\n  {};\n}}", lines.join(";\n  ")))
}

fn class_text(node: &Value) -> Result<String, String> {
    let abstract_word = if flag(node, "isAbstract") {
        "abstract "
    } else {
        ""
    };
    Ok(format!(
        "{abstract_word}class {}{}{}{}{}",
        text(node, "name"),
        type_parameters(node)?,
        heritage(node, "extendedTypes", "extends")?,
        heritage(node, "implementedTypes", "implements")?,
        body_signatures(node)?
    ))
}

fn interface_text(node: &Value) -> Result<String, String> {
    Ok(format!(
        "interface {}{}{}{}",
        text(node, "name"),
        type_parameters(node)?,
        heritage(node, "extendedTypes", "extends")?,
        body_signatures(node)?
    ))
}

fn alias_body(node: &Value) -> Result<String, String> {
    if let Some(ty) = node.get("type") {
        return render_type(ty);
    }
    if let Some(signature) = list(node, "signatures").first() {
        let params = parameters(signature)?;
        let returns = match signature.get("type") {
            Some(ty) => render_type(ty)?,
            None => "void".to_owned(),
        };
        return Ok(format!("{params} => {returns}"));
    }
    object_literal(node)
}

fn method_signature(node: &Value, signature: &Value) -> Result<String, String> {
    let optional = if flag(node, "isOptional") { "?" } else { "" };
    let generics = type_parameters(signature)?;
    let params = parameters(signature)?;
    let returns = match signature.get("type") {
        Some(ty) => format!(": {}", render_type(ty)?),
        None => String::new(),
    };
    Ok(format!(
        "{}{optional}{generics}{params}{returns}",
        text(node, "name")
    ))
}

fn accessor_text(node: &Value) -> Result<String, String> {
    let mut lines = Vec::new();
    if let Some(getter) = node.get("getSignature") {
        let returns = match getter.get("type") {
            Some(ty) => format!(": {}", render_type(ty)?),
            None => String::new(),
        };
        lines.push(format!("get {}(){returns}", text(node, "name")));
    }
    if let Some(setter) = node.get("setSignature") {
        lines.push(format!("set {}{}", text(node, "name"), parameters(setter)?));
    }
    if lines.is_empty() {
        return Err(format!(
            "accessor '{}' has no signatures",
            text(node, "name")
        ));
    }
    Ok(format!("{}{}", modifiers(node), lines.join("\n")))
}
