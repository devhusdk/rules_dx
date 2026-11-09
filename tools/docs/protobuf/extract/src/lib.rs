//! Converts a protoc descriptor set with source info into one validated DocIr shard.

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
use prost::Message;
use prost_types::field_descriptor_proto::{Label, Type};
use prost_types::{
    DescriptorProto, EnumDescriptorProto, FileDescriptorProto, FileDescriptorSet,
    ServiceDescriptorProto,
};

const LANGUAGE: &str = "protobuf";
const KEY_INCOMPLETE: &str = "protobuf.incomplete";

// Field numbers of the descriptor messages, used in source info paths.
const FILE_MESSAGE: i32 = 4;
const FILE_ENUM: i32 = 5;
const FILE_SERVICE: i32 = 6;
const MESSAGE_FIELD: i32 = 2;
const MESSAGE_NESTED: i32 = 3;
const MESSAGE_ENUM: i32 = 4;
const ENUM_VALUE: i32 = 2;
const SERVICE_METHOD: i32 = 2;

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub descriptor: String,
    pub package: String,
    pub out: String,
    /// Proto name (as in the descriptor) to workspace-relative path.
    pub srcs: BTreeMap<String, String>,
    /// Proto names whose source is build output.
    pub generated: BTreeSet<String>,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: protobuf_extract --descriptor FILE --package NAME --out FILE --src PROTO=PATH... [--generated PROTO]..."
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if value.is_empty() {
            return Err(format!("{flag} must not be empty"));
        }
        match flag {
            "--src" => {
                let Some((name, path)) = value.split_once('=') else {
                    return Err(format!("--src '{value}' is not PROTO=PATH"));
                };
                if name.is_empty() || path.is_empty() {
                    return Err(format!("--src '{value}' is not PROTO=PATH"));
                }
                if options
                    .srcs
                    .insert(name.to_owned(), path.to_owned())
                    .is_some()
                {
                    return Err(format!("--src given twice for '{name}'"));
                }
            }
            "--generated" => {
                options.generated.insert(value.clone());
            }
            "--descriptor" | "--package" | "--out" => {
                let slot = match flag {
                    "--descriptor" => &mut options.descriptor,
                    "--package" => &mut options.package,
                    _ => &mut options.out,
                };
                if !slot.is_empty() {
                    return Err(format!("{flag} given more than once"));
                }
                *slot = value.clone();
            }
            _ => return Err(format!("unknown argument '{flag}'")),
        }
        index += 2;
    }
    for (flag, value) in [
        ("--descriptor", &options.descriptor),
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
    let bytes = fs::read(&options.descriptor)
        .map_err(|error| format!("cannot read {}: {error}", options.descriptor))?;
    let shard = convert(&bytes, options)?;
    let bytes = documentation_ir::encode_shard(&shard)
        .map_err(|error| format!("cannot encode IR: {error:?}"))?;
    fs::write(&options.out, bytes).map_err(|error| format!("cannot write {}: {error}", options.out))
}

/// Strips the comment markers' shared leading space and trailing blank space.
fn clean_comment(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect();
    lines.join("\n").trim().to_owned()
}

/// The comments and line of every item in one file, keyed by source info path.
struct Locations {
    by_path: BTreeMap<Vec<i32>, (String, u64)>,
}

impl Locations {
    fn new(file: &FileDescriptorProto) -> Result<Self, String> {
        let Some(info) = file.source_code_info.as_ref() else {
            return Err(format!(
                "{} has no source info; run protoc with --include_source_info",
                file.name()
            ));
        };
        let mut by_path = BTreeMap::new();
        for location in &info.location {
            let leading = location.leading_comments();
            let comment = if leading.trim().is_empty() {
                location.trailing_comments()
            } else {
                leading
            };
            let line = location.span.first().map_or(0, |line| *line as u64 + 1);
            by_path
                .entry(location.path.clone())
                .or_insert((clean_comment(comment), line));
        }
        Ok(Self { by_path })
    }

    fn get(&self, path: &[i32]) -> (String, u64) {
        self.by_path.get(path).cloned().unwrap_or_default()
    }
}

fn type_name(field: &prost_types::FieldDescriptorProto) -> String {
    match field.r#type() {
        Type::Double => "double".to_owned(),
        Type::Float => "float".to_owned(),
        Type::Int64 => "int64".to_owned(),
        Type::Uint64 => "uint64".to_owned(),
        Type::Int32 => "int32".to_owned(),
        Type::Fixed64 => "fixed64".to_owned(),
        Type::Fixed32 => "fixed32".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::String => "string".to_owned(),
        Type::Bytes => "bytes".to_owned(),
        Type::Uint32 => "uint32".to_owned(),
        Type::Sfixed32 => "sfixed32".to_owned(),
        Type::Sfixed64 => "sfixed64".to_owned(),
        Type::Sint32 => "sint32".to_owned(),
        Type::Sint64 => "sint64".to_owned(),
        Type::Group | Type::Message | Type::Enum => {
            field.type_name().trim_start_matches('.').to_owned()
        }
    }
}

struct Builder<'a> {
    file: &'a FileDescriptorProto,
    locations: Locations,
    package: String,
    source: String,
    generated: bool,
    /// Full names of the generated `map<K, V>` entry messages and their types.
    maps: BTreeMap<String, (String, String)>,
    symbols: Vec<Symbol>,
}

impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        segments: &[String],
        kind: SymbolKind,
        signature: String,
        path: &[i32],
        parent: Option<&[String]>,
        params: Vec<Param>,
        returns: Option<Returns>,
    ) {
        let (doc, line) = self.locations.get(path);
        let id_of = |segments: &[String]| {
            format!(
                "{LANGUAGE}:{}:{}",
                self.package,
                segments
                    .iter()
                    .map(|s| documentation_ir::escape_id_segment(s))
                    .collect::<Vec<_>>()
                    .join(".")
            )
        };
        let mut extensions = Vec::new();
        if doc.is_empty() {
            extensions.push(Extension {
                key: KEY_INCOMPLETE.to_owned(),
                value: b"doc".to_vec(),
            });
        }
        self.symbols.push(Symbol {
            id: id_of(segments),
            kind: kind as i32,
            signature_text: signature,
            doc_markdown: doc,
            params,
            returns,
            source: Some(SourceRef {
                file: self.source.clone(),
                line,
                generated: self.generated,
            }),
            visibility: Visibility::Public as i32,
            relations: parent.map(|parent| Relations {
                member_of: vec![id_of(parent)],
                implements: Vec::new(),
                reexported_as: Vec::new(),
            }),
            extensions,
            display_name: segments.join("."),
            ..Symbol::default()
        });
    }

    fn collect_maps(&mut self, prefix: &str, messages: &[DescriptorProto]) {
        for message in messages {
            let full = format!("{prefix}.{}", message.name());
            if message.options.as_ref().is_some_and(|o| o.map_entry()) {
                let key = message.field.iter().find(|f| f.number() == 1);
                let value = message.field.iter().find(|f| f.number() == 2);
                if let (Some(key), Some(value)) = (key, value) {
                    self.maps.insert(
                        full.trim_start_matches('.').to_owned(),
                        (type_name(key), type_name(value)),
                    );
                }
            }
            self.collect_maps(&full, &message.nested_type);
        }
    }

    fn message(&mut self, scope: &[String], nested: bool, path: &[i32], message: &DescriptorProto) {
        let mut segments = scope.to_vec();
        segments.push(message.name().to_owned());
        self.push(
            &segments,
            SymbolKind::Class,
            format!("message {}", message.name()),
            path,
            nested.then_some(scope),
            Vec::new(),
            None,
        );
        for (index, field) in message.field.iter().enumerate() {
            let mut field_path = path.to_vec();
            field_path.extend([MESSAGE_FIELD, index as i32]);
            let mut field_segments = segments.clone();
            field_segments.push(field.name().to_owned());
            let ty = type_name(field);
            let signature = match self.maps.get(&ty) {
                Some((key, value)) => {
                    format!("map<{key}, {value}> {} = {};", field.name(), field.number())
                }
                None => {
                    let label = if field.label() == Label::Repeated {
                        "repeated "
                    } else if field.proto3_optional() {
                        "optional "
                    } else {
                        ""
                    };
                    format!("{label}{ty} {} = {};", field.name(), field.number())
                }
            };
            self.push(
                &field_segments,
                SymbolKind::Field,
                signature,
                &field_path,
                Some(&segments),
                Vec::new(),
                None,
            );
        }
        for (index, nested) in message.nested_type.iter().enumerate() {
            if nested.options.as_ref().is_some_and(|o| o.map_entry()) {
                continue;
            }
            let mut nested_path = path.to_vec();
            nested_path.extend([MESSAGE_NESTED, index as i32]);
            self.message(&segments, true, &nested_path, nested);
        }
        for (index, nested) in message.enum_type.iter().enumerate() {
            let mut nested_path = path.to_vec();
            nested_path.extend([MESSAGE_ENUM, index as i32]);
            self.enumeration(&segments, true, &nested_path, nested);
        }
    }

    fn enumeration(
        &mut self,
        scope: &[String],
        nested: bool,
        path: &[i32],
        item: &EnumDescriptorProto,
    ) {
        let mut segments = scope.to_vec();
        segments.push(item.name().to_owned());
        self.push(
            &segments,
            SymbolKind::Enum,
            format!("enum {}", item.name()),
            path,
            nested.then_some(scope),
            Vec::new(),
            None,
        );
        for (index, value) in item.value.iter().enumerate() {
            let mut value_path = path.to_vec();
            value_path.extend([ENUM_VALUE, index as i32]);
            let mut value_segments = segments.clone();
            value_segments.push(value.name().to_owned());
            self.push(
                &value_segments,
                SymbolKind::EnumVariant,
                format!("{} = {};", value.name(), value.number()),
                &value_path,
                Some(&segments),
                Vec::new(),
                None,
            );
        }
    }

    fn service(
        &mut self,
        scope: &[String],
        nested: bool,
        path: &[i32],
        item: &ServiceDescriptorProto,
    ) {
        let mut segments = scope.to_vec();
        segments.push(item.name().to_owned());
        self.push(
            &segments,
            SymbolKind::Interface,
            format!("service {}", item.name()),
            path,
            nested.then_some(scope),
            Vec::new(),
            None,
        );
        for (index, method) in item.method.iter().enumerate() {
            let mut method_path = path.to_vec();
            method_path.extend([SERVICE_METHOD, index as i32]);
            let mut method_segments = segments.clone();
            method_segments.push(method.name().to_owned());
            let input = method.input_type().trim_start_matches('.').to_owned();
            let output = method.output_type().trim_start_matches('.').to_owned();
            let in_stream = if method.client_streaming() {
                "stream "
            } else {
                ""
            };
            let out_stream = if method.server_streaming() {
                "stream "
            } else {
                ""
            };
            let signature = format!(
                "rpc {}({in_stream}{input}) returns ({out_stream}{output});",
                method.name()
            );
            self.push(
                &method_segments,
                SymbolKind::Method,
                signature,
                &method_path,
                Some(&segments),
                vec![Param {
                    name: "request".to_owned(),
                    r#type: format!("{in_stream}{input}"),
                    doc: String::new(),
                }],
                Some(Returns {
                    r#type: format!("{out_stream}{output}"),
                    doc: String::new(),
                }),
            );
        }
    }
}

/// Converts one descriptor set into a validated IR shard of one package.
pub fn convert(descriptor: &[u8], options: &Options) -> Result<DocIr, String> {
    let package = options.package.as_str();
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(format!("invalid package '{package}'"));
    }
    let set = FileDescriptorSet::decode(descriptor)
        .map_err(|error| format!("descriptor set is malformed: {error}"))?;
    let mut symbols = Vec::new();
    for file in &set.file {
        let Some(source) = options.srcs.get(file.name()) else {
            return Err(format!(
                "descriptor set holds '{}', which is not one of the library's sources",
                file.name()
            ));
        };
        let mut builder = Builder {
            file,
            locations: Locations::new(file)?,
            package: package.to_owned(),
            source: source.clone(),
            generated: options.generated.contains(file.name()),
            maps: BTreeMap::new(),
            symbols: Vec::new(),
        };
        let proto_package = builder.file.package().to_owned();
        let scope: Vec<String> = proto_package
            .split('.')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let prefix = format!(".{proto_package}");
        let prefix = if proto_package.is_empty() {
            String::new()
        } else {
            prefix
        };
        builder.collect_maps(&prefix, &file.message_type);
        for (index, message) in file.message_type.iter().enumerate() {
            builder.message(&scope, false, &[FILE_MESSAGE, index as i32], message);
        }
        for (index, item) in file.enum_type.iter().enumerate() {
            builder.enumeration(&scope, false, &[FILE_ENUM, index as i32], item);
        }
        for (index, item) in file.service.iter().enumerate() {
            builder.service(&scope, false, &[FILE_SERVICE, index as i32], item);
        }
        symbols.append(&mut builder.symbols);
    }
    if symbols.is_empty() {
        return Err(format!(
            "descriptor set declares no Protobuf API of package '{package}'"
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
