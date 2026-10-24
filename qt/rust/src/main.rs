//! Writes the C++ side of CXX-Qt and CXX bridges with the pinned generators.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use cxx_qt_gen::{
    parse_qt_file, self_inlining::qualify_self_types, write_cpp, write_rust, CppFragment,
    CxxQtItem, GeneratedCppBlocks, GeneratedOpt, GeneratedRustBlocks, Parser,
};
use proc_macro2::TokenStream;
use quote::ToTokens;

const QML_TYPES: [(&str, &str); 8] = [
    ("::std::int8_t", "qint8"),
    ("::std::uint8_t", "quint8"),
    ("::std::int16_t", "short"),
    ("::std::uint16_t", "ushort"),
    ("::std::int32_t", "int"),
    ("::std::uint32_t", "uint"),
    ("::std::int64_t", "qlonglong"),
    ("::std::uint64_t", "qulonglong"),
];

const USAGE: &str = "usage: qt_rust_bridge header <cxx.h>\n       qt_rust_bridge cxx <bridge.rs> <out.cxx.h> <out.cxx.cpp>\n       qt_rust_bridge qt <bridge.rs> <include> <out.cxxqt.h> <out.cxxqt.cpp> <out.cxx.h> <out.cxx.cpp>";

struct Generated {
    qobject: Option<(String, String)>,
    header: Vec<u8>,
    source: Vec<u8>,
}

fn main() -> ExitCode {
    std::env::set_var("CLANG_FORMAT_BINARY", "");
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("qt_rust_bridge: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["header", out] => write(out, cxx_gen::HEADER.as_bytes()),
        ["cxx", src, header, source] => {
            let generated = generate(src, None)?;
            write(header, &generated.header)?;
            write(source, &generated.source)
        }
        ["qt", src, include, qobject_header, qobject_source, header, source] => {
            let generated = generate(src, Some(include))?;
            let Some((qt_header, qt_source)) = generated.qobject else {
                return Err(format!("{src}: no #[cxx_qt::bridge] module"));
            };
            write(qobject_header, qml_properties(&qt_header).as_bytes())?;
            write(qobject_source, qt_source.as_bytes())?;
            write(header, &generated.header)?;
            write(source, &generated.source)
        }
        _ => Err(USAGE.to_owned()),
    }
}

fn qml_properties(header: &str) -> String {
    let mut out = String::with_capacity(header.len());
    for line in header.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix("Q_PROPERTY(");
        let mapped = rest.and_then(|rest| {
            QML_TYPES.iter().find_map(|(cxx, qml)| {
                rest.strip_prefix(cxx)
                    .filter(|tail| tail.starts_with(' '))
                    .map(|tail| {
                        format!(
                            "{}Q_PROPERTY({qml}{tail}",
                            &line[..line.len() - trimmed.len()]
                        )
                    })
            })
        });
        out.push_str(mapped.as_deref().unwrap_or(line));
    }
    out
}

fn write(path: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("{path}: {error}"))
}

fn located(path: &str, error: &syn::Error) -> String {
    let start = error.span().start();
    format!("{path}:{}:{}: {error}", start.line, start.column + 1)
}

fn generate(path: &str, include: Option<&str>) -> Result<Generated, String> {
    if !Path::new(path).is_file() {
        return Err(format!("{path}: no such bridge file"));
    }
    let file = parse_qt_file(path).map_err(|error| located(path, &error))?;
    let mut tokens = TokenStream::new();
    for attr in &file.attrs {
        tokens.extend(attr.into_token_stream());
    }
    let mut qobject = None;
    let mut bridges = 0;
    for item in file.items {
        match item {
            CxxQtItem::Cxx(module) => {
                bridges += 1;
                tokens.extend(module.into_token_stream());
            }
            CxxQtItem::CxxQt(module) => {
                bridges += 1;
                let Some(include) = include else {
                    return Err(format!(
                        "{path}: #[cxx_qt::bridge] needs a qt bridge target"
                    ));
                };
                let mut parser = Parser::from(*module).map_err(|error| located(path, &error))?;
                qualify_self_types(&mut parser).map_err(|error| located(path, &error))?;
                let cpp = GeneratedCppBlocks::from(&parser, &GeneratedOpt::default())
                    .map_err(|error| located(path, &error))?;
                let rust =
                    GeneratedRustBlocks::from(&parser).map_err(|error| located(path, &error))?;
                qobject = Some(match write_cpp(&cpp, include) {
                    CppFragment::Pair { header, source } => (header, source),
                    CppFragment::Header(header) => (header, String::new()),
                    CppFragment::Source(source) => (String::new(), source),
                });
                tokens.extend(write_rust(&rust, Some(include)));
            }
            CxxQtItem::Item(item) => tokens.extend(item.into_token_stream()),
        }
    }
    if bridges != 1 {
        return Err(format!(
            "{path}: holds {bridges} bridge modules; want exactly one"
        ));
    }
    let code = cxx_gen::generate_header_and_cc(tokens, &cxx_gen::Opt::default())
        .map_err(|error| format!("{path}: {error}"))?;
    Ok(Generated {
        qobject,
        header: code.header,
        source: code.implementation,
    })
}

#[cfg(test)]
mod tests {
    use super::qml_properties;

    #[test]
    fn property_types_use_qml_names() {
        let header = "  Q_PROPERTY(::std::int32_t count READ getCount)\n\
                      Q_PROPERTY(::std::uint64_t size READ getSize)\n\
                      Q_PROPERTY(::std::int32_tx odd READ getOdd)\n\
                      Q_SIGNAL void rejected(::std::int32_t amount);\n";
        assert_eq!(
            qml_properties(header),
            "  Q_PROPERTY(int count READ getCount)\n\
             Q_PROPERTY(qulonglong size READ getSize)\n\
             Q_PROPERTY(::std::int32_tx odd READ getOdd)\n\
             Q_SIGNAL void rejected(::std::int32_t amount);\n"
        );
    }
}
