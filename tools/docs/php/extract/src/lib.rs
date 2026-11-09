//! Converts the php_dump driver's JSON into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::fs;

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use serde_json::{Map, Value};

const LANGUAGE: &str = "php";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "php.config";
const KEY_INCOMPLETE: &str = "php.incomplete";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: php_extract --json FILE --package NAME --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--json" => &mut options.json,
            "--package" => &mut options.package,
            "--out" => &mut options.out,
            _ => return Err(format!("unknown argument '{flag}'")),
        };
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if !slot.is_empty() {
            return Err(format!("{flag} given more than once"));
        }
        if value.is_empty() {
            return Err(format!("{flag} must not be empty"));
        }
        *slot = value.clone();
        index += 2;
    }
    for (flag, value) in [
        ("--json", &options.json),
        ("--package", &options.package),
        ("--out", &options.out),
    ] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    Ok(options)
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
pub fn run(options: &Options) -> Result<(), String> {
    let json = fs::read_to_string(&options.json)
        .map_err(|error| format!("cannot read {}: {error}", options.json))?;
    let shard = convert(&json, &options.package)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

pub use documentation_ir::escape_segment;

fn field<'a>(object: &'a Map<String, Value>, key: &str, at: &str) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{at} has no '{key}'"))
}

fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
    field(object, key, at)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{at} '{key}' is not a string"))
}

fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<u64, String> {
    field(object, key, at)?
        .as_u64()
        .ok_or_else(|| format!("{at} '{key}' is not a number"))
}

fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    field(object, key, at)?
        .as_bool()
        .ok_or_else(|| format!("{at} '{key}' is not a boolean"))
}

fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, String> {
    field(object, key, at)?
        .as_array()
        .ok_or_else(|| format!("{at} '{key}' is not a list"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at} '{key}' holds a non-string"))
        })
        .collect()
}

/// One docblock tag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tag {
    tag: String,
    name: String,
    types: String,
    text: String,
}

/// One declared parameter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Argument {
    name: String,
    r#type: String,
    default: String,
    variadic: bool,
    by_ref: bool,
}

/// One element of the driver JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Element {
    kind: String,
    fqsen: String,
    name: String,
    parent: String,
    file: String,
    generated: bool,
    line: u64,
    summary: String,
    description: String,
    tags: Vec<Tag>,
    visibility: String,
    is_abstract: bool,
    is_final: bool,
    is_static: bool,
    is_readonly: bool,
    superclass: String,
    interfaces: Vec<String>,
    traits: Vec<String>,
    params: Vec<Argument>,
    return_type: String,
    value_type: String,
    value: String,
}

fn tags(object: &Map<String, Value>, at: &str) -> Result<Vec<Tag>, String> {
    let list = field(object, "tags", at)?
        .as_array()
        .ok_or_else(|| format!("{at} 'tags' is not a list"))?;
    list.iter()
        .enumerate()
        .map(|(index, tag)| {
            let at = format!("{at} tag {index}");
            let tag = tag
                .as_object()
                .ok_or_else(|| format!("{at} is not an object"))?;
            Ok(Tag {
                tag: string(tag, "tag", &at)?,
                name: string(tag, "name", &at)?,
                types: string(tag, "type", &at)?,
                text: string(tag, "text", &at)?,
            })
        })
        .collect()
}

fn arguments(object: &Map<String, Value>, at: &str) -> Result<Vec<Argument>, String> {
    let list = field(object, "params", at)?
        .as_array()
        .ok_or_else(|| format!("{at} 'params' is not a list"))?;
    list.iter()
        .enumerate()
        .map(|(index, argument)| {
            let at = format!("{at} param {index}");
            let argument = argument
                .as_object()
                .ok_or_else(|| format!("{at} is not an object"))?;
            Ok(Argument {
                name: string(argument, "name", &at)?,
                r#type: string(argument, "type", &at)?,
                default: string(argument, "default", &at)?,
                variadic: boolean(argument, "variadic", &at)?,
                by_ref: boolean(argument, "by_ref", &at)?,
            })
        })
        .collect()
}

fn element(value: &Value, at: &str) -> Result<Element, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{at} is not an object"))?;
    let kind = string(object, "type", at)?;
    if !matches!(
        kind.as_str(),
        "class"
            | "interface"
            | "trait"
            | "enum"
            | "case"
            | "function"
            | "method"
            | "property"
            | "constant"
    ) {
        return Err(format!("{at} has unknown type '{kind}'"));
    }
    let fqsen = string(object, "fqsen", at)?;
    let name = string(object, "name", at)?;
    if fqsen.is_empty() || name.is_empty() {
        return Err(format!("{at} has an empty fqsen or name"));
    }
    Ok(Element {
        kind,
        fqsen,
        name,
        parent: string(object, "parent", at)?,
        file: string(object, "file", at)?,
        generated: boolean(object, "generated", at)?,
        line: number(object, "line", at)?,
        summary: string(object, "summary", at)?,
        description: string(object, "description", at)?,
        tags: tags(object, at)?,
        visibility: string(object, "visibility", at)?,
        is_abstract: boolean(object, "abstract", at)?,
        is_final: boolean(object, "final", at)?,
        is_static: boolean(object, "static", at)?,
        is_readonly: boolean(object, "readonly", at)?,
        superclass: string(object, "superclass", at)?,
        interfaces: strings(object, "interfaces", at)?,
        traits: strings(object, "traits", at)?,
        params: arguments(object, at)?,
        return_type: string(object, "return_type", at)?,
        value_type: string(object, "value_type", at)?,
        value: string(object, "value", at)?,
    })
}

fn is_container(element: &Element) -> bool {
    matches!(
        element.kind.as_str(),
        "class" | "interface" | "trait" | "enum"
    )
}

/// Returns the id segments of one PHP name such as `Geo\Circle`, escaped.
fn qualified_name(name: &str) -> String {
    name.trim_matches('\\')
        .trim_end_matches("()")
        .split('\\')
        .map(documentation_ir::escape_id_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn is_hidden_itself(element: &Element) -> bool {
    element.visibility != "public"
        || element
            .tags
            .iter()
            .any(|tag| tag.tag == "internal" || tag.tag == "ignore")
}

/// Normalizes CRLF line endings and trims trailing whitespace.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

fn doc_markdown(element: &Element) -> String {
    let summary = normalize(&element.summary);
    let description = normalize(&element.description);
    match (summary.is_empty(), description.is_empty()) {
        (_, true) => summary,
        (true, false) => description,
        (false, false) => format!("{summary}\n\n{description}"),
    }
}

fn parameter_text(argument: &Argument) -> String {
    let mut text = String::new();
    if !argument.r#type.is_empty() {
        text.push_str(&argument.r#type);
        text.push(' ');
    }
    if argument.by_ref {
        text.push('&');
    }
    if argument.variadic {
        text.push_str("...");
    }
    text.push('$');
    text.push_str(&argument.name);
    if !argument.default.is_empty() {
        text.push_str(" = ");
        text.push_str(&argument.default);
    }
    text
}

fn modifiers(element: &Element, visibility: bool) -> String {
    let mut text = String::new();
    if element.is_final {
        text.push_str("final ");
    }
    if element.is_abstract {
        text.push_str("abstract ");
    }
    if visibility {
        text.push_str("public ");
    }
    if element.is_static {
        text.push_str("static ");
    }
    if element.is_readonly {
        text.push_str("readonly ");
    }
    text
}

fn list(prefix: &str, names: &[String]) -> String {
    if names.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = names.iter().map(|n| format!("\\{n}")).collect();
        format!(" {prefix} {}", names.join(", "))
    }
}

fn signature(element: &Element) -> String {
    let name = element.fqsen.trim_matches('\\').trim_end_matches("()");
    match element.kind.as_str() {
        "class" => {
            let mut text = format!("{}class {name}", modifiers(element, false));
            if !element.superclass.is_empty() {
                text.push_str(&format!(" extends \\{}", element.superclass));
            }
            text.push_str(&list("implements", &element.interfaces));
            text
        }
        "interface" => format!("interface {name}{}", list("extends", &element.interfaces)),
        "trait" => format!("trait {name}"),
        "enum" => {
            let backing = if element.value_type.is_empty() {
                String::new()
            } else {
                format!(": {}", element.value_type)
            };
            format!(
                "enum {name}{backing}{}",
                list("implements", &element.interfaces)
            )
        }
        "case" => {
            if element.value.is_empty() {
                format!("case {}", element.name)
            } else {
                format!("case {} = {}", element.name, element.value)
            }
        }
        "constant" => {
            let prefix = if element.parent.is_empty() {
                String::new()
            } else {
                modifiers(element, true)
            };
            format!("{prefix}const {} = {}", element.name, element.value)
        }
        "property" => {
            let mut text = modifiers(element, true);
            if !element.value_type.is_empty() {
                text.push_str(&element.value_type);
                text.push(' ');
            }
            text.push('$');
            text.push_str(&element.name);
            if !element.value.is_empty() {
                text.push_str(" = ");
                text.push_str(&element.value);
            }
            text
        }
        _ => {
            let prefix = if element.kind == "method" {
                modifiers(element, true)
            } else {
                String::new()
            };
            let params: Vec<String> = element.params.iter().map(parameter_text).collect();
            let mut text = format!("{prefix}function {}({})", element.name, params.join(", "));
            if !is_constructor(element) && !element.return_type.is_empty() {
                text.push_str(&format!(": {}", element.return_type));
            }
            text
        }
    }
}

fn is_constructor(element: &Element) -> bool {
    element.kind == "method" && element.name.eq_ignore_ascii_case("__construct")
}

fn kind_of(element: &Element) -> SymbolKind {
    match element.kind.as_str() {
        "class" | "trait" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "enum" => SymbolKind::Enum,
        "case" => SymbolKind::EnumVariant,
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "property" => SymbolKind::Property,
        _ => SymbolKind::Constant,
    }
}

fn params_and_incomplete(element: &Element, incomplete: &mut Vec<String>) -> Vec<Param> {
    let mut params = Vec::new();
    let mut undocumented = Vec::new();
    let mut untyped = Vec::new();
    for argument in &element.params {
        let tag = element
            .tags
            .iter()
            .find(|tag| tag.tag == "param" && tag.name == argument.name);
        if tag.is_none() {
            undocumented.push(argument.name.clone());
        }
        let declared = &argument.r#type;
        let documented = tag.map(|t| t.types.as_str()).unwrap_or_default();
        if declared.is_empty() && documented.is_empty() {
            untyped.push(argument.name.clone());
        }
        params.push(Param {
            name: argument.name.clone(),
            r#type: if declared.is_empty() {
                documented.to_owned()
            } else {
                declared.clone()
            },
            doc: tag.map(|t| normalize(&t.text)).unwrap_or_default(),
        });
    }
    if !undocumented.is_empty() {
        incomplete.push(format!("params={}", undocumented.join(",")));
    }
    if !untyped.is_empty() {
        incomplete.push(format!("param_types={}", untyped.join(",")));
    }
    params
}

fn returns_and_incomplete(element: &Element, incomplete: &mut Vec<String>) -> Option<Returns> {
    if !matches!(element.kind.as_str(), "function" | "method") || is_constructor(element) {
        return None;
    }
    let tag = element.tags.iter().find(|tag| tag.tag == "return");
    let documented = tag.map(|t| t.types.as_str()).unwrap_or_default();
    if element.return_type.is_empty() && documented.is_empty() {
        incomplete.push("return_type".to_owned());
    }
    let kind = if element.return_type.is_empty() {
        documented.to_owned()
    } else {
        element.return_type.clone()
    };
    Some(Returns {
        r#type: kind,
        doc: tag.map(|t| normalize(&t.text)).unwrap_or_default(),
    })
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

/// Converts one driver JSON document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("php_dump JSON is malformed: {error}"))?;
    let root = root.as_object().ok_or("php_dump JSON is not an object")?;
    let at = "php_dump JSON";
    match number(root, "format", at)? {
        FORMAT => {}
        other => {
            return Err(format!(
                "php_dump JSON format {other} is not supported, expected {FORMAT}"
            ));
        }
    }
    let config = format!(
        "php={};phpdocumentor={};format={FORMAT}",
        string(root, "php", at)?,
        string(root, "phpdocumentor", at)?
    );
    let elements: Vec<Element> = field(root, "objects", at)?
        .as_array()
        .ok_or("php_dump JSON 'objects' is not a list")?
        .iter()
        .enumerate()
        .map(|(index, value)| element(value, &format!("php_dump JSON object {index}")))
        .collect::<Result<_, _>>()?;

    let hidden_containers: BTreeSet<&str> = elements
        .iter()
        .filter(|element| is_container(element) && is_hidden_itself(element))
        .map(|element| element.fqsen.as_str())
        .collect();
    let hidden = |element: &Element| {
        is_hidden_itself(element) || hidden_containers.contains(element.parent.as_str())
    };
    let prefix = format!("{LANGUAGE}:{package}:");
    let id_of = |element: &Element| -> String {
        if element.parent.is_empty() {
            format!("{prefix}{}", qualified_name(&element.fqsen))
        } else {
            let name = if element.kind == "property" {
                format!("${}", element.name)
            } else {
                element.name.clone()
            };
            format!(
                "{prefix}{}.{}",
                qualified_name(&element.parent),
                documentation_ir::escape_id_segment(&name)
            )
        }
    };
    let known: BTreeSet<String> = elements
        .iter()
        .filter(|element| is_container(element) && !hidden(element))
        .map(id_of)
        .collect();

    let mut symbols = Vec::new();
    for element in elements.iter().filter(|element| !hidden(element)) {
        let mut incomplete = Vec::new();
        let doc = doc_markdown(element);
        if doc.is_empty() {
            incomplete.push("doc".to_owned());
        }
        let callable = matches!(element.kind.as_str(), "function" | "method");
        let params = if callable {
            params_and_incomplete(element, &mut incomplete)
        } else {
            Vec::new()
        };
        let returns = returns_and_incomplete(element, &mut incomplete);
        let member_of: Vec<String> = if element.parent.is_empty() {
            Vec::new()
        } else {
            vec![format!("{prefix}{}", qualified_name(&element.parent))]
        };
        let mut implements: Vec<String> = element
            .interfaces
            .iter()
            .chain(element.traits.iter())
            .map(|name| format!("{prefix}{}", qualified_name(name)))
            .filter(|id| known.contains(id))
            .collect();
        implements.sort();
        implements.dedup();
        let relations = if member_of.is_empty() && implements.is_empty() {
            None
        } else {
            Some(Relations {
                member_of,
                implements,
                reexported_as: Vec::new(),
            })
        };
        let mut extensions = vec![extension(KEY_CONFIG, &config)];
        if !incomplete.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
        }
        symbols.push(Symbol {
            id: id_of(element),
            kind: kind_of(element) as i32,
            signature_text: signature(element),
            doc_markdown: doc,
            params,
            returns,
            source: Some(SourceRef {
                file: element.file.clone(),
                line: element.line,
                generated: element.generated,
            }),
            visibility: Visibility::Public as i32,
            relations,
            extensions,
            display_name: if element.parent.is_empty() {
                element.fqsen.trim_matches('\\').trim_end_matches("()").to_owned()
            } else {
                let sep = if element.kind == "property" { "::$" } else { "::" };
                format!("{}{sep}{}", element.parent.trim_matches('\\'), element.name)
            },
            ..Symbol::default()
        });
    }
    if symbols.is_empty() {
        return Err(format!(
            "php_dump JSON lists no public PHP API of package '{package}'"
        ));
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
