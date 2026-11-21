//! Converts a protoc descriptor set with source info into one validated `DocIr` shard.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::tool::{FlagError, PathIoError, WriteShardError, path_io};
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

/// Why the `protobuf_extract` arguments were rejected.
#[derive(Debug)]
pub enum ArgsError {
    Flag(FlagError),
    NotPair { value: String },
    RepeatedSrc { name: String },
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(error) => error.fmt(f),
            Self::NotPair { value } => write!(f, "--src '{value}' is not PROTO=PATH"),
            Self::RepeatedSrc { name } => write!(f, "--src given twice for '{name}'"),
        }
    }
}

impl std::error::Error for ArgsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Flag(error) => Some(error),
            Self::NotPair { .. } | Self::RepeatedSrc { .. } => None,
        }
    }
}

impl From<FlagError> for ArgsError {
    fn from(error: FlagError) -> Self {
        Self::Flag(error)
    }
}

/// Why one descriptor set could not become a shard.
#[derive(Debug)]
pub enum ProtobufExtractError {
    Read(PathIoError),
    InvalidPackage { package: String },
    Malformed(prost::DecodeError),
    NoSourceInfo { file: String },
    NotASource { file: String },
    NoPublicApi { package: String },
    InvalidIr(documentation_ir::Error),
    Write(WriteShardError),
}

impl fmt::Display for ProtobufExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(f),
            Self::InvalidPackage { package } => write!(f, "invalid package '{package}'"),
            Self::Malformed(error) => write!(f, "descriptor set is malformed: {error}"),
            Self::NoSourceInfo { file } => write!(
                f,
                "{file} has no source info; run protoc with --include_source_info"
            ),
            Self::NotASource { file } => write!(
                f,
                "descriptor set holds '{file}', which is not one of the library's sources"
            ),
            Self::NoPublicApi { package } => write!(
                f,
                "descriptor set declares no Protobuf API of package '{package}'"
            ),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProtobufExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Malformed(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::InvalidPackage { .. }
            | Self::NoSourceInfo { .. }
            | Self::NotASource { .. }
            | Self::NoPublicApi { .. }
            | Self::InvalidIr(_) => None,
        }
    }
}

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

/// Runs `protobuf_extract` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("protobuf_extract", usage(), parse, run, args)
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> &'static str {
    "usage: protobuf_extract --descriptor FILE --package NAME --out FILE --src PROTO=PATH... [--generated PROTO]..."
}

/// Parses argv into inputs, or returns the rejection text.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse(args: &[String]) -> Result<Options, ArgsError> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &["--src", "--generated", "--descriptor", "--package", "--out"],
    )?;
    let mut options = Options {
        descriptor: flags.one("--descriptor")?,
        package: flags.one("--package")?,
        out: flags.one("--out")?,
        ..Options::default()
    };
    for value in flags.many("--src") {
        let Some((name, path)) = value
            .split_once('=')
            .filter(|(name, path)| !name.is_empty() && !path.is_empty())
        else {
            return Err(ArgsError::NotPair { value });
        };
        if options
            .srcs
            .insert(name.to_owned(), path.to_owned())
            .is_some()
        {
            return Err(ArgsError::RepeatedSrc {
                name: name.to_owned(),
            });
        }
    }
    options.generated.extend(flags.many("--generated"));
    Ok(options)
}

/// Reads, converts, validates and writes one shard, returning the failure text on error.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn run(options: &Options) -> Result<(), ProtobufExtractError> {
    let bytes =
        path_io::read(Path::new(&options.descriptor)).map_err(ProtobufExtractError::Read)?;
    let shard = convert(&bytes, options)?;
    documentation_ir::tool::write_shard(&options.out, &shard).map_err(ProtobufExtractError::Write)
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
    fn new(file: &FileDescriptorProto) -> Result<Self, ProtobufExtractError> {
        let Some(info) = file.source_code_info.as_ref() else {
            return Err(ProtobufExtractError::NoSourceInfo {
                file: file.name().to_owned(),
            });
        };
        let mut by_path = BTreeMap::new();
        for location in &info.location {
            let leading = location.leading_comments();
            let comment = if leading.trim().is_empty() {
                location.trailing_comments()
            } else {
                leading
            };
            let line = location
                .span
                .first()
                .map_or(0, |line| u64::try_from(*line).unwrap_or(0) + 1);
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
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a distinct input of the call"
    )]
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
            if message
                .options
                .as_ref()
                .is_some_and(prost_types::MessageOptions::map_entry)
            {
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
            field_path.extend([MESSAGE_FIELD, proto_index(index)]);
            let mut field_segments = segments.clone();
            field_segments.push(field.name().to_owned());
            let ty = type_name(field);
            let signature = if let Some((key, value)) = self.maps.get(&ty) {
                format!("map<{key}, {value}> {} = {};", field.name(), field.number())
            } else {
                let label = if field.label() == Label::Repeated {
                    "repeated "
                } else if field.proto3_optional() {
                    "optional "
                } else {
                    ""
                };
                format!("{label}{ty} {} = {};", field.name(), field.number())
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
            if nested
                .options
                .as_ref()
                .is_some_and(prost_types::MessageOptions::map_entry)
            {
                continue;
            }
            let mut nested_path = path.to_vec();
            nested_path.extend([MESSAGE_NESTED, proto_index(index)]);
            self.message(&segments, true, &nested_path, nested);
        }
        for (index, nested) in message.enum_type.iter().enumerate() {
            let mut nested_path = path.to_vec();
            nested_path.extend([MESSAGE_ENUM, proto_index(index)]);
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
            value_path.extend([ENUM_VALUE, proto_index(index)]);
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
            method_path.extend([SERVICE_METHOD, proto_index(index)]);
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
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn convert(descriptor: &[u8], options: &Options) -> Result<DocIr, ProtobufExtractError> {
    let package = options.package.as_str();
    if package.is_empty() || package.contains(':') || package.chars().any(char::is_whitespace) {
        return Err(ProtobufExtractError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    let set = FileDescriptorSet::decode(descriptor).map_err(ProtobufExtractError::Malformed)?;
    let mut symbols = Vec::new();
    for file in &set.file {
        let Some(source) = options.srcs.get(file.name()) else {
            return Err(ProtobufExtractError::NotASource {
                file: file.name().to_owned(),
            });
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
            builder.message(&scope, false, &[FILE_MESSAGE, proto_index(index)], message);
        }
        for (index, item) in file.enum_type.iter().enumerate() {
            builder.enumeration(&scope, false, &[FILE_ENUM, proto_index(index)], item);
        }
        for (index, item) in file.service.iter().enumerate() {
            builder.service(&scope, false, &[FILE_SERVICE, proto_index(index)], item);
        }
        symbols.append(&mut builder.symbols);
    }
    if symbols.is_empty() {
        return Err(ProtobufExtractError::NoPublicApi {
            package: package.to_owned(),
        });
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        language: LANGUAGE.to_owned(),
        package: package.to_owned(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(ProtobufExtractError::InvalidIr)?;
    Ok(shard)
}

fn proto_index(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod lib_tests;
