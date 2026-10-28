//! Converts the griffe driver's JSON into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};
use serde_json::Value;

const LANGUAGE: &str = "python";
const FORMAT: u64 = 1;
const KEY_CONFIG: &str = "python.config";
const KEY_INCOMPLETE: &str = "python.incomplete";
const ROOT_SEGMENT: &str = "__init__";

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub root: String,
    pub srcs: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: python_extract --json FILE --package NAME --root DIR --srcs FILE --out FILE"
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
            "--root" => &mut options.root,
            "--srcs" => &mut options.srcs,
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
        ("--root", &options.root),
        ("--srcs", &options.srcs),
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
    let manifest = fs::read_to_string(&options.srcs)
        .map_err(|error| format!("cannot read {}: {error}", options.srcs))?;
    let sources = parse_sources(&manifest)?;
    let root = options.root.trim_end_matches('/');
    let shard = convert_mapped(&json, &options.package, &|file| {
        let staged = format!("{root}/{file}");
        sources
            .get(&staged)
            .cloned()
            .ok_or_else(|| format!("source '{staged}' is not in the srcs manifest"))
    })?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

/// Parses a srcs manifest into staged path -> (repo-relative path, generated).
pub fn parse_sources(manifest: &str) -> Result<BTreeMap<String, (String, bool)>, String> {
    let mut sources = BTreeMap::new();
    for line in manifest.lines().filter(|line| !line.is_empty()) {
        let columns: Vec<&str> = line.split('\t').collect();
        let generated = match columns.as_slice() {
            [_, rel, flag] if !rel.is_empty() && !rel.starts_with('/') && !rel.contains("..") => {
                match *flag {
                    "0" => false,
                    "1" => true,
                    _ => return Err(format!("malformed srcs manifest line '{line}'")),
                }
            }
            _ => return Err(format!("malformed srcs manifest line '{line}'")),
        };
        sources.insert(columns[0].to_owned(), (columns[1].to_owned(), generated));
    }
    Ok(sources)
}

/// Escapes one id segment: every byte outside `[A-Za-z0-9_]` becomes `~` plus two hex digits.
pub fn escape_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("~{byte:02X}"));
        }
    }
    out
}

fn text<'a>(object: &'a Value, key: &str) -> &'a str {
    object.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn list<'a>(object: &'a Value, key: &str) -> &'a [Value] {
    object
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn members(object: &Value) -> BTreeMap<&str, &Value> {
    object
        .get("members")
        .and_then(Value::as_object)
        .map(|map| map.iter().map(|(k, v)| (k.as_str(), v)).collect())
        .unwrap_or_default()
}

fn kind_of(object: &Value) -> &str {
    text(object, "kind")
}

fn is_container(object: &Value) -> bool {
    matches!(kind_of(object), "module" | "class")
}

fn has_label(object: &Value, label: &str) -> bool {
    list(object, "labels")
        .iter()
        .any(|value| value.as_str() == Some(label))
}

/// Returns the `__all__` names of a module, or `None` when it defines none.
fn exports(object: &Value) -> Option<BTreeSet<&str>> {
    let entries = object.get("exports")?.as_array()?;
    Some(entries.iter().filter_map(Value::as_str).collect())
}

/// Applies the public rule to one member of a container.
fn is_public(container: &Value, name: &str, member: &Value) -> bool {
    let alias = kind_of(member) == "alias";
    if kind_of(member) == "module" && !alias {
        return !name.starts_with('_');
    }
    if kind_of(container) == "module" {
        if let Some(names) = exports(container) {
            return names.contains(name);
        }
    }
    if alias || name.starts_with('_') {
        return false;
    }
    // Attributes only assigned on instances without a docstring are noise.
    let instance_only = has_label(member, "instance-attribute")
        && !has_label(member, "class-attribute")
        && !has_label(member, "module-attribute")
        && member.get("docstring").is_none();
    !instance_only
}

fn is_enum(object: &Value) -> bool {
    list(object, "bases").iter().any(|base| {
        let base = base.as_str().unwrap_or_default();
        let last = base.rsplit('.').next().unwrap_or_default();
        matches!(last, "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag")
    })
}

/// One definition reached through the public API.
#[derive(Default)]
struct Reach<'a> {
    def: Option<&'a Value>,
    /// Public dotted path to the public path of the container it was found in.
    paths: BTreeMap<String, String>,
}

struct Walker<'a> {
    defs: BTreeMap<String, &'a Value>,
    reached: BTreeMap<String, Reach<'a>>,
    walked: BTreeSet<String>,
    pending: BTreeMap<String, BTreeMap<String, String>>,
}

impl<'a> Walker<'a> {
    fn record(&mut self, def: &'a Value, path: String, parent: &str) {
        let key = text(def, "path").to_owned();
        let reach = self.reached.entry(key).or_default();
        reach.def = Some(def);
        reach.paths.insert(path, parent.to_owned());
    }

    /// Walks the public members of one container reached at one public path.
    fn walk(&mut self, container: &'a Value, container_path: &str) {
        for (name, member) in members(container) {
            if !is_public(container, name, member) {
                continue;
            }
            let path = format!("{container_path}.{name}");
            if kind_of(member) == "alias" {
                let Some(target) = self.defs.get(text(member, "target_path")).copied() else {
                    continue;
                };
                if is_container(target) {
                    self.pending
                        .entry(text(target, "path").to_owned())
                        .or_default()
                        .insert(path.clone(), container_path.to_owned());
                }
                self.record(target, path, container_path);
                continue;
            }
            self.record(member, path.clone(), container_path);
            if is_container(member) && self.walked.insert(text(member, "path").to_owned()) {
                self.walk(member, &path);
            }
        }
    }
}

fn index<'a>(object: &'a Value, defs: &mut BTreeMap<String, &'a Value>) {
    if kind_of(object) == "alias" {
        return;
    }
    defs.insert(text(object, "path").to_owned(), object);
    for member in members(object).into_values() {
        index(member, defs);
    }
}

/// Renders one parameter list into signature text, the self parameter dropped.
fn render_parameters(parameters: &[Value], in_class: bool) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut keyword_marker_needed = true;
    let mut after_positional_only = false;
    let mut shown: Vec<&Value> = parameters.iter().collect();
    if in_class && matches!(shown.first().map(|p| text(p, "name")), Some("self" | "cls")) {
        shown.remove(0);
    }
    for (index, parameter) in shown.iter().enumerate() {
        let kind = text(parameter, "kind");
        if after_positional_only && kind != "positional-only" {
            out.push("/".to_owned());
            after_positional_only = false;
        }
        if keyword_marker_needed && kind == "keyword-only" {
            out.push("*".to_owned());
            keyword_marker_needed = false;
        }
        let prefix = match kind {
            "variadic positional" => "*",
            "variadic keyword" => "**",
            _ => "",
        };
        if kind == "variadic positional" {
            keyword_marker_needed = false;
        } else if kind == "positional or keyword" || kind == "positional-only" {
            keyword_marker_needed = true;
        }
        if kind == "positional-only" {
            after_positional_only = true;
        }
        let mut rendered = format!("{prefix}{}", text(parameter, "name"));
        let annotation = text(parameter, "annotation");
        let default = text(parameter, "default");
        if !annotation.is_empty() {
            rendered.push_str(&format!(": {annotation}"));
        }
        if !default.is_empty() && !prefix.starts_with('*') {
            let spacing = if annotation.is_empty() { "=" } else { " = " };
            rendered.push_str(&format!("{spacing}{default}"));
        }
        out.push(rendered);
        if index + 1 == shown.len() && after_positional_only {
            out.push("/".to_owned());
        }
    }
    out.join(", ")
}

/// Normalizes CRLF line endings and trims trailing whitespace.
fn normalize(value: &str) -> String {
    value.replace("\r\n", "\n").trim_end().to_owned()
}

fn sections<'a>(object: &'a Value, kind: &str) -> Vec<&'a Value> {
    let parsed = object
        .get("docstring")
        .map(|doc| list(doc, "parsed"))
        .unwrap_or_default();
    parsed
        .iter()
        .filter(|section| text(section, "kind") == kind)
        .collect()
}

fn description(object: &Value) -> String {
    let paragraphs: Vec<String> = sections(object, "text")
        .iter()
        .map(|section| normalize(text(section, "value")))
        .filter(|paragraph| !paragraph.is_empty())
        .collect();
    paragraphs.join("\n\n")
}

fn examples(object: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for section in sections(object, "examples") {
        for entry in list(section, "value") {
            let code = entry
                .as_array()
                .and_then(|pair| pair.get(1))
                .and_then(Value::as_str)
                .map(normalize)
                .unwrap_or_default();
            if !code.is_empty() {
                out.push(code);
            }
        }
    }
    out
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

fn join_id(package: &str, path: &str) -> String {
    let prefix = format!("{package}.");
    let rest = path.strip_prefix(&prefix).unwrap_or("");
    let qualified = if rest.is_empty() {
        ROOT_SEGMENT.to_owned()
    } else {
        rest.split('.')
            .map(escape_segment)
            .collect::<Vec<_>>()
            .join(".")
    };
    format!("{LANGUAGE}:{package}:{qualified}")
}

/// The IR shape of one definition before ids are resolved.
struct Draft {
    kind: SymbolKind,
    signature: String,
    params: Vec<Param>,
    returns: Option<Returns>,
    incomplete: Vec<String>,
}

fn function_draft(def: &Value, docs: &Value, in_class: bool, name: &str) -> Draft {
    let parameters = list(def, "parameters");
    let documented = sections(docs, "parameters");
    let documented: Vec<&Value> = documented
        .iter()
        .flat_map(|section| list(section, "value"))
        .collect();
    let mut undocumented = Vec::new();
    let mut untyped = Vec::new();
    let mut params = Vec::new();
    let skip = usize::from(
        in_class
            && matches!(
                parameters.first().map(|p| text(p, "name")),
                Some("self" | "cls")
            ),
    );
    for parameter in parameters.iter().skip(skip) {
        let raw = text(parameter, "name");
        let prefix = match text(parameter, "kind") {
            "variadic positional" => "*",
            "variadic keyword" => "**",
            _ => "",
        };
        let note = documented.iter().find(|entry| text(entry, "name") == raw);
        let doc = note.map(|entry| normalize(text(entry, "description")));
        let signature_type = text(parameter, "annotation");
        let documented_type = note.map_or("", |entry| text(entry, "annotation"));
        let annotation = if signature_type.is_empty() {
            documented_type
        } else {
            signature_type
        };
        if doc.as_deref().unwrap_or_default().is_empty() {
            undocumented.push(raw.to_owned());
        }
        if annotation.is_empty() {
            untyped.push(raw.to_owned());
        }
        params.push(Param {
            name: format!("{prefix}{raw}"),
            r#type: annotation.to_owned(),
            doc: doc.unwrap_or_default(),
        });
    }
    let mut incomplete = Vec::new();
    if !undocumented.is_empty() {
        incomplete.push(format!("params={}", undocumented.join(",")));
    }
    if !untyped.is_empty() {
        incomplete.push(format!("param_types={}", untyped.join(",")));
    }
    let annotation = def
        .get("returns")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let notes: Vec<String> = sections(def, "returns")
        .iter()
        .flat_map(|section| list(section, "value"))
        .map(|entry| normalize(text(entry, "description")))
        .filter(|note| !note.is_empty())
        .collect();
    let section_type = sections(def, "returns")
        .iter()
        .flat_map(|section| list(section, "value"))
        .map(|entry| text(entry, "annotation"))
        .find(|kind| !kind.is_empty())
        .unwrap_or_default();
    if annotation.is_empty() && section_type.is_empty() {
        incomplete.push("return_type".to_owned());
    }
    let returns = if annotation.is_empty() && notes.is_empty() && section_type.is_empty() {
        None
    } else {
        let kind = if annotation.is_empty() {
            section_type
        } else {
            annotation
        };
        Some(Returns {
            r#type: kind.to_owned(),
            doc: notes.join("\n"),
        })
    };
    let asynchronous = if has_label(def, "async") {
        "async "
    } else {
        ""
    };
    let rendered = render_parameters(parameters, in_class);
    let tail = if annotation.is_empty() {
        String::new()
    } else {
        format!(" -> {annotation}")
    };
    Draft {
        kind: if in_class {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        },
        signature: format!("{asynchronous}def {name}({rendered}){tail}"),
        params,
        returns,
        incomplete,
    }
}

fn value_suffix(def: &Value) -> String {
    let value = text(def, "value");
    if value.is_empty() {
        String::new()
    } else {
        format!(" = {}", value.lines().next().unwrap_or_default())
    }
}

fn draft(def: &Value, parent: Option<&Value>) -> Draft {
    let name = text(def, "name");
    let in_class = parent.is_some_and(|p| kind_of(p) == "class");
    let plain = |kind, signature: String| Draft {
        kind,
        signature,
        params: Vec::new(),
        returns: None,
        incomplete: Vec::new(),
    };
    let annotation = text(def, "annotation");
    let annotated = if annotation.is_empty() {
        name.to_owned()
    } else {
        format!("{name}: {annotation}")
    };
    match kind_of(def) {
        "module" => plain(SymbolKind::Module, format!("module {}", text(def, "path"))),
        "class" => {
            let bases: Vec<&str> = list(def, "bases")
                .iter()
                .filter_map(Value::as_str)
                .collect();
            let tail = if bases.is_empty() {
                String::new()
            } else {
                format!("({})", bases.join(", "))
            };
            let mut out = plain(
                if is_enum(def) {
                    SymbolKind::Enum
                } else {
                    SymbolKind::Class
                },
                format!("class {name}{tail}"),
            );
            if let Some(init) = members(def).get("__init__") {
                let function = function_draft(init, def, true, name);
                out.params = function.params;
                out.incomplete = function
                    .incomplete
                    .into_iter()
                    .filter(|reason| reason != "return_type")
                    .collect();
            }
            out
        }
        "function" => function_draft(def, def, in_class, name),
        "type alias" => plain(
            SymbolKind::TypeAlias,
            format!("type {name}{}", value_suffix(def)),
        ),
        _ if has_label(def, "property") => {
            let mut out = plain(SymbolKind::Property, annotated);
            if annotation.is_empty() {
                out.incomplete.push("return_type".to_owned());
            }
            out
        }
        _ if in_class && parent.is_some_and(is_enum) => plain(
            SymbolKind::EnumVariant,
            format!("{name}{}", value_suffix(def)),
        ),
        _ if in_class => plain(
            SymbolKind::Field,
            format!("{annotated}{}", value_suffix(def)),
        ),
        _ => plain(
            SymbolKind::Constant,
            format!("{annotated}{}", value_suffix(def)),
        ),
    }
}

/// Converts one driver JSON document into a validated IR shard of one package.
pub fn convert(json: &str, package: &str) -> Result<DocIr, String> {
    convert_mapped(json, package, &|file| Ok((file.to_owned(), false)))
}

/// Source mapper: griffe's package-relative file to (repo-relative file, generated).
pub type SourceMap<'a> = &'a dyn Fn(&str) -> Result<(String, bool), String>;

/// Like [`convert`], with every source path mapped through `map`.
pub fn convert_mapped(json: &str, package: &str, map: SourceMap) -> Result<DocIr, String> {
    if package.is_empty()
        || package.contains([':', '.'])
        || package.chars().any(char::is_whitespace)
    {
        return Err(format!("invalid package '{package}'"));
    }
    let root: Value =
        serde_json::from_str(json).map_err(|error| format!("griffe JSON is malformed: {error}"))?;
    if root.get("format").and_then(Value::as_u64) != Some(FORMAT) {
        return Err(format!(
            "griffe JSON format {:?} is not supported, expected {FORMAT}",
            root.get("format")
        ));
    }
    let header = |key: &str| -> Result<&str, String> {
        root.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("griffe JSON has no '{key}'"))
    };
    if header("package")? != package {
        return Err(format!(
            "griffe JSON describes package '{}', expected '{package}'",
            header("package")?
        ));
    }
    let config = format!(
        "docstyle={};format={FORMAT};griffe={};python={}",
        header("docstyle")?,
        header("griffe")?,
        header("python")?
    );
    let module = root
        .get("module")
        .filter(|module| kind_of(module) == "module")
        .ok_or("griffe JSON has no module")?;
    if text(module, "path") != package {
        return Err(format!(
            "griffe JSON module '{}' is not package '{package}'",
            text(module, "path")
        ));
    }

    let mut defs = BTreeMap::new();
    index(module, &mut defs);
    let mut walker = Walker {
        defs,
        reached: BTreeMap::new(),
        walked: BTreeSet::new(),
        pending: BTreeMap::new(),
    };
    walker.record(module, package.to_owned(), "");
    walker.walked.insert(package.to_owned());
    walker.walk(module, package);
    // Containers only reachable through a re-export are walked at their first sorted path.
    loop {
        let next = walker
            .pending
            .iter()
            .find(|(def, _)| !walker.walked.contains(*def))
            .map(|(def, paths)| (def.clone(), paths.keys().next().cloned()));
        let Some((def, Some(path))) = next else {
            break;
        };
        walker.walked.insert(def.clone());
        if let Some(container) = walker.defs.get(&def).copied() {
            walker.walk(container, &path);
        }
    }

    let canonical = |def: &str, reach: &Reach| -> Option<String> {
        if reach.paths.contains_key(def) {
            Some(def.to_owned())
        } else {
            reach.paths.keys().next().cloned()
        }
    };
    let by_path: BTreeMap<&str, &Value> = walker
        .reached
        .values()
        .filter_map(|reach| reach.def.map(|def| (reach, def)))
        .flat_map(|(reach, def)| reach.paths.keys().map(move |path| (path.as_str(), def)))
        .collect();
    let mut symbols = Vec::new();
    for (def_path, reach) in &walker.reached {
        let (Some(def), Some(path)) = (reach.def, canonical(def_path, reach)) else {
            continue;
        };
        let parent_path = reach.paths.get(&path).map_or("", String::as_str);
        let parent_def = by_path.get(parent_path).copied();
        let draft = draft(def, parent_def);
        let mut incomplete = Vec::new();
        let doc = description(def);
        if doc.is_empty() {
            incomplete.push("doc".to_owned());
        }
        incomplete.extend(draft.incomplete);
        let member_of: Vec<String> = if parent_path.is_empty() {
            Vec::new()
        } else {
            vec![join_id(package, parent_path)]
        };
        let reexported_as: Vec<String> = reach
            .paths
            .keys()
            .filter(|other| **other != path)
            .map(|other| join_id(package, other))
            .collect();
        let relations = if member_of.is_empty() && reexported_as.is_empty() {
            None
        } else {
            Some(Relations {
                member_of,
                implements: Vec::new(),
                reexported_as,
            })
        };
        let mut extensions = vec![extension(KEY_CONFIG, &config)];
        if !incomplete.is_empty() {
            extensions.push(extension(KEY_INCOMPLETE, &incomplete.join(";")));
        }
        let file = text(def, "relative_package_filepath");
        let mapped = if file.is_empty() {
            None
        } else {
            Some(map(file)?)
        };
        symbols.push(Symbol {
            id: join_id(package, &path),
            kind: draft.kind as i32,
            signature_text: draft.signature,
            doc_markdown: doc,
            params: draft.params,
            returns: draft.returns,
            examples: examples(def),
            source: mapped.map(|(file, generated)| SourceRef {
                file,
                line: def.get("lineno").and_then(Value::as_u64).unwrap_or(0),
                generated,
            }),
            visibility: Visibility::Public as i32,
            relations,
            extensions,
            display_name: path,
        });
    }
    // The root module symbol alone is not an API.
    if symbols.len() < 2 {
        return Err(format!(
            "griffe JSON lists no public Python API of package '{package}'"
        ));
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
