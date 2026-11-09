//! Converts the XML tree Doxygen writes for the public headers of a library into one validated DocIr shard.

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
use std::path::Path;

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

const FORMAT: u64 = 1;

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub xml: String,
    pub srcs: String,
    pub package: String,
    pub language: String,
    pub out: String,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: cc_extract --xml DIR --srcs FILE --package NAME --language cc|cuda --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--xml" => &mut options.xml,
            "--srcs" => &mut options.srcs,
            "--package" => &mut options.package,
            "--language" => &mut options.language,
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
        ("--xml", &options.xml),
        ("--srcs", &options.srcs),
        ("--package", &options.package),
        ("--language", &options.language),
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
    let mut documents = Vec::new();
    let entries = fs::read_dir(&options.xml)
        .map_err(|error| format!("cannot read {}: {error}", options.xml))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("cannot list {}: {error}", options.xml))?
            .path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if !name.ends_with(".xml") || name == "index.xml" || name == "Doxyfile.xml" {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        documents.push((name, text));
    }
    documents.sort();
    let manifest = fs::read_to_string(&options.srcs)
        .map_err(|error| format!("cannot read {}: {error}", options.srcs))?;
    let docs: Vec<&str> = documents.iter().map(|(_, text)| text.as_str()).collect();
    let shard = convert(&docs, &manifest, &options.package, &options.language)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(Path::new(&options.out), bytes)
        .map_err(|error| format!("cannot write {}: {error}", options.out))
}

pub use documentation_ir::escape_segment;

/// One XML element with its attributes and children.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Child>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Child {
    Node(Node),
    Text(String),
}

impl Node {
    fn attr(&self, key: &str) -> &str {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map_or("", |(_, v)| v.as_str())
    }

    fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|child| match child {
            Child::Node(node) => Some(node),
            Child::Text(_) => None,
        })
    }

    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.nodes().filter(move |node| node.name == name)
    }

    fn first(&self, name: &str) -> Option<&Node> {
        self.nodes().find(|node| node.name == name)
    }

    /// The plain text of this element, links and markup flattened.
    fn text(&self) -> String {
        let mut out = String::new();
        for child in &self.children {
            match child {
                Child::Text(text) => out.push_str(text),
                Child::Node(node) => out.push_str(&node.text()),
            }
        }
        out
    }
}

fn element(start: &quick_xml::events::BytesStart) -> Result<Node, String> {
    let mut node = Node {
        name: String::from_utf8_lossy(start.local_name().as_ref()).into_owned(),
        ..Node::default()
    };
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| format!("bad XML attribute: {error}"))?;
        let key = String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned();
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|error| format!("bad XML attribute value: {error}"))?
            .into_owned();
        node.attrs.push((key, value));
    }
    Ok(node)
}

fn parse_xml(text: &str) -> Result<Node, String> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let push_text = |stack: &mut Vec<Node>, text: &str| {
        if let Some(top) = stack.last_mut() {
            match top.children.last_mut() {
                Some(Child::Text(last)) => last.push_str(text),
                _ => top.children.push(Child::Text(text.to_owned())),
            }
        }
    };
    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("Doxygen XML is malformed: {error}"))?;
        match event {
            Event::Start(start) => stack.push(element(&start)?),
            Event::Empty(start) => {
                let node = element(&start)?;
                match stack.last_mut() {
                    Some(top) => top.children.push(Child::Node(node)),
                    None => root = Some(node),
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or("Doxygen XML closes an unopened element")?;
                match stack.last_mut() {
                    Some(top) => top.children.push(Child::Node(node)),
                    None => root = Some(node),
                }
            }
            Event::Text(text) => {
                let text = text
                    .xml10_content()
                    .map_err(|error| format!("Doxygen XML text is not valid: {error}"))?;
                push_text(&mut stack, &text);
            }
            Event::CData(text) => {
                let text = text
                    .decode()
                    .map_err(|error| format!("Doxygen XML text is not valid: {error}"))?;
                push_text(&mut stack, &text);
            }
            Event::GeneralRef(reference) => {
                let resolved = match reference
                    .resolve_char_ref()
                    .map_err(|error| format!("Doxygen XML text is not valid: {error}"))?
                {
                    Some(ch) => ch.to_string(),
                    None => {
                        let name = reference
                            .decode()
                            .map_err(|error| format!("Doxygen XML text is not valid: {error}"))?;
                        quick_xml::escape::resolve_predefined_entity(&name)
                            .ok_or_else(|| format!("Doxygen XML uses unknown entity &{name};"))?
                            .to_owned()
                    }
                };
                push_text(&mut stack, &resolved);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| "Doxygen XML has no root element".to_owned())
}

/// Squeezes runs of whitespace into single spaces.
fn squeeze(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The pieces of one documentation block.
#[derive(Default)]
struct Described {
    markdown: String,
    params: BTreeMap<String, String>,
    returns: String,
}

fn inline(node: &Node, out: &mut String) {
    for child in &node.children {
        match child {
            Child::Text(text) => out.push_str(text),
            Child::Node(inner) => match inner.name.as_str() {
                "computeroutput" => {
                    let mut code = String::new();
                    inline(inner, &mut code);
                    out.push('`');
                    out.push_str(code.trim());
                    out.push('`');
                }
                "bold" => {
                    out.push_str("**");
                    inline(inner, out);
                    out.push_str("**");
                }
                "emphasis" => {
                    out.push('*');
                    inline(inner, out);
                    out.push('*');
                }
                "linebreak" => out.push('\n'),
                "ulink" => {
                    let mut label = String::new();
                    inline(inner, &mut label);
                    out.push_str(&format!("[{}]({})", label.trim(), inner.attr("url")));
                }
                "parameterlist" | "simplesect" | "itemizedlist" | "orderedlist"
                | "programlisting" => {}
                _ => inline(inner, out),
            },
        }
    }
}

fn listing(node: &Node) -> String {
    let mut lines = Vec::new();
    for line in node.all("codeline") {
        let mut text = String::new();
        for highlight in line.all("highlight") {
            for child in &highlight.children {
                match child {
                    Child::Text(t) => text.push_str(t),
                    Child::Node(n) if n.name == "sp" => text.push(' '),
                    Child::Node(n) => text.push_str(&n.text()),
                }
            }
        }
        lines.push(text.trim_end().to_owned());
    }
    format!("```cpp\n{}\n```", lines.join("\n"))
}

fn block(node: &Node, blocks: &mut Vec<String>, described: &mut Described) {
    let mut text = String::new();
    inline(node, &mut text);
    let text = text.trim();
    if !text.is_empty() {
        blocks.push(text.lines().map(str::trim).collect::<Vec<_>>().join("\n"));
    }
    for child in node.nodes() {
        match child.name.as_str() {
            "parameterlist" => {
                if child.attr("kind") != "param" && child.attr("kind") != "templateparam" {
                    continue;
                }
                for item in child.all("parameteritem") {
                    let doc = item
                        .first("parameterdescription")
                        .map(|d| paragraphs(d, &mut Described::default()))
                        .unwrap_or_default();
                    for names in item.all("parameternamelist") {
                        for name in names.all("parametername") {
                            for one in name.text().split(',') {
                                described
                                    .params
                                    .insert(one.trim().to_owned(), doc.clone());
                            }
                        }
                    }
                }
            }
            "simplesect" => {
                let body = paragraphs(child, &mut Described::default());
                match child.attr("kind") {
                    "return" => described.returns = body,
                    "see" | "note" | "warning" | "attention" | "remark" | "pre" | "post"
                    | "since" | "version" | "author" | "date" | "copyright" | "invariant"
                    | "par" | "rcs" | "user" | "todo" | "test" | "bug" | "deprecated" => {
                        if !body.is_empty() {
                            let label = match child.attr("kind") {
                                "see" => "See also",
                                "note" => "Note",
                                "warning" => "Warning",
                                "attention" => "Attention",
                                "deprecated" => "Deprecated",
                                other => other,
                            };
                            blocks.push(format!("**{label}:** {body}"));
                        }
                    }
                    _ => {}
                }
            }
            "itemizedlist" | "orderedlist" => {
                let ordered = child.name == "orderedlist";
                let items: Vec<String> = child
                    .all("listitem")
                    .enumerate()
                    .map(|(index, item)| {
                        let body = paragraphs(item, &mut Described::default());
                        let marker = if ordered {
                            format!("{}.", index + 1)
                        } else {
                            "-".to_owned()
                        };
                        format!("{marker} {}", body.replace("\n\n", "\n"))
                    })
                    .collect();
                blocks.push(items.join("\n"));
            }
            "programlisting" => blocks.push(listing(child)),
            "para" | "title" | "sect1" | "sect2" | "sect3" | "simplesectsep" => {
                block(child, blocks, described);
            }
            _ => {}
        }
    }
}

fn paragraphs(node: &Node, described: &mut Described) -> String {
    let mut blocks = Vec::new();
    for para in node.all("para") {
        block(para, &mut blocks, described);
    }
    blocks.join("\n\n")
}

fn describe(member: &Node) -> Described {
    let mut described = Described::default();
    let brief = member
        .first("briefdescription")
        .map(|d| paragraphs(d, &mut described))
        .unwrap_or_default();
    let detail = member
        .first("detaileddescription")
        .map(|d| paragraphs(d, &mut described))
        .unwrap_or_default();
    described.markdown = [brief, detail]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    described
}

/// Where each header Doxygen read lives in the workspace.
struct Sources {
    by_path: BTreeMap<String, (String, bool)>,
}

impl Sources {
    fn parse(manifest: &str) -> Result<Sources, String> {
        let mut by_path = BTreeMap::new();
        for line in manifest.lines().filter(|l| !l.is_empty()) {
            let mut parts = line.split('\t');
            let (Some(exec), Some(rel), Some(generated), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(format!("bad srcs manifest line '{line}'"));
            };
            by_path.insert(exec.to_owned(), (rel.to_owned(), generated == "1"));
        }
        Ok(Sources { by_path })
    }

    fn find(&self, file: &str) -> Option<&(String, bool)> {
        let file = file.trim_start_matches("./");
        self.by_path.get(file).or_else(|| {
            self.by_path
                .iter()
                .find(|(exec, _)| exec.ends_with(&format!("/{file}")))
                .map(|(_, value)| value)
        })
    }
}

/// Splits a qualified C++ name at top-level `::`, keeping template arguments whole.
fn segments(name: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let bytes: Vec<char> = name.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        let c = bytes[index];
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && c == ':' && bytes.get(index + 1) == Some(&':') {
            parts.push(std::mem::take(&mut current));
            index += 2;
            continue;
        }
        current.push(c);
        index += 1;
    }
    parts.push(current);
    parts.retain(|part| !part.is_empty());
    parts
}

fn qualified(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| documentation_ir::escape_id_segment(part))
        .collect::<Vec<_>>()
        .join(".")
}

fn extension(key: &str, value: &str) -> Extension {
    Extension {
        key: key.to_owned(),
        value: value.as_bytes().to_vec(),
    }
}

/// One symbol before ids are made unique.
struct Draft {
    path: Vec<String>,
    order: (u8, String, u64),
    symbol: Symbol,
}

fn type_text(node: Option<&Node>) -> String {
    node.map(|n| squeeze(&n.text())).unwrap_or_default()
}

fn template_prefix(node: &Node) -> String {
    let Some(list) = node.first("templateparamlist") else {
        return String::new();
    };
    let params: Vec<String> = list
        .all("param")
        .map(|p| {
            let kind = type_text(p.first("type"));
            match p.first("declname") {
                Some(name) => format!("{kind} {}", squeeze(&name.text())),
                None => kind,
            }
        })
        .collect();
    format!("template <{}> ", params.join(", "))
}

struct Context<'a> {
    sources: &'a Sources,
    language: &'a str,
    config: String,
}

fn location<'a>(node: &'a Node, context: &Context) -> Option<(String, u64, bool)> {
    let at = node.first("location")?;
    let file = if at.attr("declfile").is_empty() {
        at.attr("file")
    } else {
        at.attr("declfile")
    };
    let line = if at.attr("declline").is_empty() {
        at.attr("line")
    } else {
        at.attr("declline")
    };
    let (rel, generated) = context.sources.find(file)?;
    Some((rel.clone(), line.parse().unwrap_or(0), *generated))
}

fn incomplete_extensions(
    context: &Context,
    undocumented: Vec<String>,
) -> Vec<Extension> {
    let mut extensions = vec![extension(
        &format!("{}.config", context.language),
        &context.config,
    )];
    if !undocumented.is_empty() {
        extensions.push(extension(
            &format!("{}.incomplete", context.language),
            &undocumented.join(";"),
        ));
    }
    extensions
}

fn member_draft(
    member: &Node,
    compound_path: &[String],
    compound_id: &str,
    in_class: bool,
    context: &Context,
    prefix: &str,
) -> Option<Draft> {
    if member.attr("prot") != "public" {
        return None;
    }
    let name = squeeze(&member.first("name")?.text());
    if name.is_empty() || name.starts_with('@') {
        return None;
    }
    let (file, line, generated) = location(member, context)?;
    let kind = member.attr("kind");
    let described = describe(member);
    let mut undocumented = Vec::new();
    if described.markdown.is_empty() {
        undocumented.push("doc".to_owned());
    }
    let ty = type_text(member.first("type"));
    let mut params = Vec::new();
    let mut returns = None;
    let (symbol_kind, signature) = match kind {
        "define" => {
            let args: Vec<String> = member
                .all("param")
                .map(|p| squeeze(&p.first("defname").map(Node::text).unwrap_or_default()))
                .collect();
            let head = if member.all("param").next().is_some() {
                format!("{name}({})", args.join(", "))
            } else {
                name.clone()
            };
            for arg in &args {
                params.push(Param {
                    name: arg.clone(),
                    r#type: String::new(),
                    doc: described.params.get(arg).cloned().unwrap_or_default(),
                });
            }
            let value = squeeze(&type_text(member.first("initializer")));
            let signature = if value.is_empty() {
                format!("#define {head}")
            } else {
                format!("#define {head} {value}")
            };
            (SymbolKind::Macro, signature)
        }
        "function" => {
            let mut head = template_prefix(member);
            if member.attr("static") == "yes" {
                head.push_str("static ");
            }
            if member.attr("virt") != "non-virtual" {
                head.push_str("virtual ");
            }
            if member.attr("explicit") == "yes" {
                head.push_str("explicit ");
            }
            if !ty.is_empty() {
                head.push_str(&ty);
                head.push(' ');
            }
            head.push_str(&name);
            head.push_str(&squeeze(&type_text(member.first("argsstring"))));
            let mut missing = Vec::new();
            for param in member.all("param") {
                let pname = param
                    .first("declname")
                    .or_else(|| param.first("defname"))
                    .map(|n| squeeze(&n.text()))
                    .unwrap_or_default();
                if pname.is_empty() {
                    continue;
                }
                let doc = described.params.get(&pname).cloned().unwrap_or_default();
                if doc.is_empty() {
                    missing.push(pname.clone());
                }
                params.push(Param {
                    name: pname,
                    r#type: type_text(param.first("type")),
                    doc,
                });
            }
            if !missing.is_empty() {
                undocumented.push(format!("params={}", missing.join(",")));
            }
            let returns_value = !ty.is_empty() && ty != "void" && ty != "auto";
            if returns_value {
                if described.returns.is_empty() {
                    undocumented.push("return".to_owned());
                }
                returns = Some(Returns {
                    r#type: ty.clone(),
                    doc: described.returns.clone(),
                });
            }
            let kind = if in_class {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            (kind, head)
        }
        "variable" => {
            let constant = ty.contains("constexpr")
                || (ty.starts_with("const ") && !ty.contains('*') && member.attr("static") != "no")
                || (!in_class && ty.starts_with("const ") && !ty.contains('*'));
            let mut head = template_prefix(member);
            if member.attr("static") == "yes" {
                head.push_str("static ");
            }
            head.push_str(&format!("{ty} {name}{}", type_text(member.first("argsstring"))));
            let init = type_text(member.first("initializer"));
            if !init.is_empty() {
                head.push(' ');
                head.push_str(&init);
            }
            let kind = if constant {
                SymbolKind::Constant
            } else if in_class {
                SymbolKind::Field
            } else {
                SymbolKind::Variable
            };
            (kind, head)
        }
        "typedef" => {
            let definition = squeeze(&type_text(member.first("definition")));
            let args = type_text(member.first("argsstring"));
            let template = template_prefix(member);
            (
                SymbolKind::TypeAlias,
                format!("{template}{definition}{args}"),
            )
        }
        "enum" => {
            let keyword = if member.attr("strong") == "yes" {
                "enum class"
            } else {
                "enum"
            };
            let head = if ty.is_empty() {
                format!("{keyword} {name}")
            } else {
                format!("{keyword} {name} : {ty}")
            };
            (SymbolKind::Enum, head)
        }
        _ => return None,
    };
    let mut path = compound_path.to_vec();
    path.push(name.clone());
    let mut extensions = incomplete_extensions(context, undocumented);
    extensions.sort_by(|a, b| a.key.cmp(&b.key));
    let owner_id = (!compound_path.is_empty() && !compound_id.is_empty())
        .then(|| format!("{prefix}{}", qualified(compound_path)));
    let symbol = Symbol {
        id: String::new(),
        kind: symbol_kind as i32,
        signature_text: signature,
        doc_markdown: described.markdown,
        params,
        returns,
        source: Some(SourceRef {
            file: file.clone(),
            line,
            generated,
        }),
        visibility: Visibility::Public as i32,
        relations: owner_id.map(|owner| Relations {
            member_of: vec![owner],
            implements: Vec::new(),
            reexported_as: Vec::new(),
        }),
        extensions,
        display_name: path.join("::"),
        ..Symbol::default()
    };
    Some(Draft {
        order: (1, file, line),
        path,
        symbol,
    })
}

fn enum_values(member: &Node, enum_path: &[String], context: &Context, prefix: &str) -> Vec<Draft> {
    let Some((file, line, generated)) = location(member, context) else {
        return Vec::new();
    };
    let enum_id = format!("{prefix}{}", qualified(enum_path));
    // Values of a plain enum are visible in the enclosing scope; Doxygen nests them under the enum for docs.
    member
        .all("enumvalue")
        .filter(|value| value.attr("prot") == "public" || value.attr("prot").is_empty())
        .filter_map(|value| {
            let name = squeeze(&value.first("name")?.text());
            let described = describe(value);
            let mut undocumented = Vec::new();
            if described.markdown.is_empty() {
                undocumented.push("doc".to_owned());
            }
            let init = type_text(value.first("initializer"));
            let signature = if init.is_empty() {
                name.clone()
            } else {
                format!("{name} {init}")
            };
            let mut path = enum_path.to_vec();
            path.push(name);
            Some(Draft {
                order: (2, file.clone(), line),
                symbol: Symbol {
                    id: String::new(),
                    kind: SymbolKind::EnumVariant as i32,
                    signature_text: signature,
                    doc_markdown: described.markdown,
                    source: Some(SourceRef {
                        file: file.clone(),
                        line,
                        generated,
                    }),
                    visibility: Visibility::Public as i32,
                    relations: Some(Relations {
                        member_of: vec![enum_id.clone()],
                        implements: Vec::new(),
                        reexported_as: Vec::new(),
                    }),
                    extensions: incomplete_extensions(context, undocumented),
                    display_name: path.join("::"),
                    ..Symbol::default()
                },
                path,
            })
        })
        .collect()
}

/// Converts the Doxygen XML documents of one package into a validated IR shard.
pub fn convert(
    documents: &[&str],
    manifest: &str,
    package: &str,
    language: &str,
) -> Result<DocIr, String> {
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    if language != "cc" && language != "cuda" {
        return Err(format!("unknown language '{language}', expected cc or cuda"));
    }
    let sources = Sources::parse(manifest)?;
    let prefix = format!("{language}:{package}:");
    let mut version = String::new();
    let mut compounds: Vec<Node> = Vec::new();
    for text in documents {
        let root = parse_xml(text)?;
        if root.name != "doxygen" {
            return Err(format!("Doxygen XML root is <{}>, want <doxygen>", root.name));
        }
        if version.is_empty() {
            version = root.attr("version").to_owned();
        }
        compounds.extend(root.all("compounddef").cloned());
    }
    let context = Context {
        sources: &sources,
        language,
        config: format!("doxygen={version};format={FORMAT}"),
    };

    let mut drafts: Vec<Draft> = Vec::new();
    let mut seen_members: BTreeSet<String> = BTreeSet::new();
    let compound_names: BTreeMap<String, Vec<String>> = compounds
        .iter()
        .filter_map(|c| {
            let name = c.first("compoundname")?.text();
            Some((c.attr("id").to_owned(), segments(&squeeze(&name))))
        })
        .collect();
    for compound in &compounds {
        let kind = compound.attr("kind");
        let path = compound_names
            .get(compound.attr("id"))
            .cloned()
            .unwrap_or_default();
        let scoped = matches!(kind, "class" | "struct" | "union" | "namespace");
        if scoped
            && (path.is_empty()
                || path.iter().any(|p| p.starts_with('@'))
                || (kind != "namespace" && compound.attr("prot") != "public"))
        {
            continue;
        }
        let in_class = matches!(kind, "class" | "struct" | "union");
        if matches!(kind, "class" | "struct" | "union") {
            let Some((file, line, generated)) = location(compound, &context) else {
                continue;
            };
            let described = describe(compound);
            let mut undocumented = Vec::new();
            if described.markdown.is_empty() {
                undocumented.push("doc".to_owned());
            }
            let mut implements: Vec<String> = compound
                .all("basecompoundref")
                .filter(|base| base.attr("prot") == "public")
                .filter_map(|base| compound_names.get(base.attr("refid")))
                .map(|base| format!("{prefix}{}", qualified(base)))
                .collect();
            implements.sort();
            implements.dedup();
            let parent: Vec<String> = path[..path.len() - 1].to_vec();
            let member_of = if parent.is_empty() {
                Vec::new()
            } else {
                vec![format!("{prefix}{}", qualified(&parent))]
            };
            let relations = (!member_of.is_empty() || !implements.is_empty()).then_some(Relations {
                member_of,
                implements,
                reexported_as: Vec::new(),
            });
            let head = format!(
                "{}{} {}",
                template_prefix(compound),
                kind,
                path.join("::")
            );
            drafts.push(Draft {
                order: (0, file.clone(), line),
                symbol: Symbol {
                    id: String::new(),
                    kind: SymbolKind::Class as i32,
                    signature_text: head,
                    doc_markdown: described.markdown,
                    source: Some(SourceRef {
                        file,
                        line,
                        generated,
                    }),
                    visibility: Visibility::Public as i32,
                    relations,
                    extensions: incomplete_extensions(&context, undocumented),
                    display_name: path.join("::"),
                    ..Symbol::default()
                },
                path: path.clone(),
            });
        }
        let member_path: &[String] = if scoped { &path } else { &[] };
        for section in compound.all("sectiondef") {
            for member in section.all("memberdef") {
                if !seen_members.insert(member.attr("id").to_owned()) {
                    continue;
                }
                let compound_id = if scoped { compound.attr("id") } else { "" };
                if let Some(draft) =
                    member_draft(member, member_path, compound_id, in_class, &context, &prefix)
                {
                    if member.attr("kind") == "enum" {
                        drafts.extend(enum_values(member, &draft.path, &context, &prefix));
                    }
                    drafts.push(draft);
                }
            }
        }
    }

    // Namespaces appear when something public lives in them.
    let mut namespaces: BTreeMap<Vec<String>, &Node> = BTreeMap::new();
    for compound in compounds.iter().filter(|c| c.attr("kind") == "namespace") {
        if let Some(path) = compound_names.get(compound.attr("id")) {
            namespaces.insert(path.clone(), compound);
        }
    }
    let mut used: BTreeSet<Vec<String>> = BTreeSet::new();
    for draft in &drafts {
        let mut path = draft.path[..draft.path.len().saturating_sub(1)].to_vec();
        while !path.is_empty() {
            if namespaces.contains_key(&path) {
                used.insert(path.clone());
            }
            path.pop();
        }
    }
    for path in used {
        let Some(compound) = namespaces.get(&path) else {
            continue;
        };
        let described = describe(compound);
        let mut undocumented = Vec::new();
        if described.markdown.is_empty() {
            undocumented.push("doc".to_owned());
        }
        let parent: Vec<String> = path[..path.len() - 1].to_vec();
        let relations = (!parent.is_empty()).then(|| Relations {
            member_of: vec![format!("{prefix}{}", qualified(&parent))],
            implements: Vec::new(),
            reexported_as: Vec::new(),
        });
        let source = location(compound, &context).map(|(file, line, generated)| SourceRef {
            file,
            line,
            generated,
        });
        drafts.push(Draft {
            order: (
                0,
                source.as_ref().map(|s| s.file.clone()).unwrap_or_default(),
                source.as_ref().map_or(0, |s| s.line),
            ),
            symbol: Symbol {
                id: String::new(),
                kind: SymbolKind::Module as i32,
                signature_text: format!("namespace {}", path.join("::")),
                doc_markdown: described.markdown,
                source,
                visibility: Visibility::Public as i32,
                relations,
                extensions: incomplete_extensions(&context, undocumented),
                display_name: path.join("::"),
                ..Symbol::default()
            },
            path,
        });
    }

    if drafts.is_empty() {
        return Err(format!(
            "Doxygen XML lists no public {language} API of package '{package}'"
        ));
    }
    drafts.sort_by(|a, b| (&a.path, &a.order).cmp(&(&b.path, &b.order)));
    // Overloads and a struct that shares its typedef name get "@N" suffixes in source order.
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let mut symbols = Vec::new();
    for mut draft in drafts {
        let base = format!("{prefix}{}", qualified(&draft.path));
        let n = count.entry(base.clone()).or_insert(0);
        *n += 1;
        draft.symbol.id = if *n == 1 { base } else { format!("{base}@{n}") };
        symbols.push(draft.symbol);
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: language.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

#[cfg(test)]
mod lib_tests;
