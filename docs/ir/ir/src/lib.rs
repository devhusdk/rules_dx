#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub use doc_ir_proto::dx::documentation::v1 as proto;
use proto::{DocIr, Symbol};
use std::collections::BTreeSet;

use dx_path::reject_reason;
use dx_proto_validate::{
    check_sorted_next, decode_with_validation, encode_with_validation, OrderViolation,
};
pub use dx_schema::SCHEMA_MAJOR;
pub use dx_schema::SCHEMA_MINOR;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Decode(String),
    MalformedText(String),
    UnsupportedMajor {
        found: u32,
    },
    EmptyLanguage,
    EmptyPackage,
    EmptySymbolId {
        index: usize,
    },
    DuplicateSymbolId {
        id: String,
    },
    UnsortedSymbols {
        id: String,
    },
    UnsafeSourcePath {
        id: String,
        path: String,
        reason: &'static str,
    },
    UnsortedExtensions {
        id: String,
    },
    DuplicateExtension {
        id: String,
        key: String,
    },
}

pub fn source_path_reason(file: &str) -> Option<&'static str> {
    reject_reason(file)
}

pub fn validate_shard(shard: &DocIr) -> Result<(), Error> {
    dx_schema::check_major(shard.schema_major)
        .map_err(|found| Error::UnsupportedMajor { found })?;
    if shard.language.is_empty() {
        return Err(Error::EmptyLanguage);
    }
    if shard.package.is_empty() {
        return Err(Error::EmptyPackage);
    }
    let mut previous_id: Option<&String> = None;
    for (index, symbol) in shard.symbols.iter().enumerate() {
        validate_symbol(symbol, index)?;
        match check_sorted_next(previous_id, &symbol.id) {
            Ok(()) => {}
            Err(OrderViolation::Duplicate) => {
                return Err(Error::DuplicateSymbolId {
                    id: symbol.id.clone(),
                });
            }
            Err(OrderViolation::Unsorted) => {
                return Err(Error::UnsortedSymbols {
                    id: symbol.id.clone(),
                });
            }
        }
        previous_id = Some(&symbol.id);
    }
    Ok(())
}

fn validate_symbol(symbol: &Symbol, index: usize) -> Result<(), Error> {
    if symbol.id.is_empty() {
        return Err(Error::EmptySymbolId { index });
    }
    if let Some(source) = symbol.source.as_ref() {
        if let Some(reason) = source_path_reason(&source.file) {
            return Err(Error::UnsafeSourcePath {
                id: symbol.id.clone(),
                path: source.file.clone(),
                reason,
            });
        }
    }
    let mut previous: Option<&String> = None;
    for extension in symbol.extensions.iter() {
        match check_sorted_next(previous, &extension.key) {
            Ok(()) => {}
            Err(OrderViolation::Duplicate) => {
                return Err(Error::DuplicateExtension {
                    id: symbol.id.clone(),
                    key: extension.key.clone(),
                });
            }
            Err(OrderViolation::Unsorted) => {
                return Err(Error::UnsortedExtensions {
                    id: symbol.id.clone(),
                });
            }
        }
        previous = Some(&extension.key);
    }
    Ok(())
}

pub fn encode_shard(shard: &DocIr) -> Result<Vec<u8>, Error> {
    encode_with_validation(shard, validate_shard)
}

pub fn decode_shard(bytes: &[u8]) -> Result<DocIr, Error> {
    decode_with_validation(bytes, validate_shard, Error::Decode)
}

const TEXT_HEADER_FIELDS: [&str; 4] = ["schema_major", "schema_minor", "language", "package"];
const TEXT_SYMBOL_FIELDS: [&str; 2] = ["id", "doc_markdown"];

pub fn decode_shard_text(text: &str) -> Result<DocIr, Error> {
    let mut shard = DocIr::default();
    let mut header = BTreeSet::new();
    let mut open: Option<(Symbol, BTreeSet<&str>)> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_suffix('{') {
            let name = name.trim();
            if name != "symbols" {
                return Err(text_error(index, format!("unknown block {name}")));
            }
            if open.is_some() {
                return Err(text_error(index, "symbols blocks do not nest"));
            }
            open = Some((Symbol::default(), BTreeSet::new()));
            continue;
        }
        if line == "}" {
            let (symbol, seen) = open
                .take()
                .ok_or_else(|| text_error(index, "} closes no block"))?;
            for field in TEXT_SYMBOL_FIELDS {
                if !seen.contains(field) {
                    return Err(text_error(index, format!("symbol names no {field}")));
                }
            }
            shard.symbols.push(symbol);
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| text_error(index, format!("{line} names no field")))?;
        let (name, value) = (name.trim(), value.trim());
        match open.as_mut() {
            Some((symbol, seen)) => {
                if !TEXT_SYMBOL_FIELDS.contains(&name) {
                    return Err(text_error(
                        index,
                        format!("symbol field not supported: {name}"),
                    ));
                }
                if !seen.insert(name) {
                    return Err(text_error(index, format!("symbol repeats {name}")));
                }
                if name == "id" {
                    symbol.id = text_string(index, name, value)?;
                } else {
                    symbol.doc_markdown = text_string(index, name, value)?;
                }
            }
            None => {
                if !TEXT_HEADER_FIELDS.contains(&name) {
                    return Err(text_error(
                        index,
                        format!("header field not supported: {name}"),
                    ));
                }
                if !header.insert(name) {
                    return Err(text_error(index, format!("header repeats {name}")));
                }
                match name {
                    "schema_major" => shard.schema_major = text_uint(index, name, value)?,
                    "schema_minor" => shard.schema_minor = text_uint(index, name, value)?,
                    "language" => shard.language = text_string(index, name, value)?,
                    _ => shard.package = text_string(index, name, value)?,
                }
            }
        }
    }
    if open.is_some() {
        return Err(Error::MalformedText(
            "symbols block is never closed".to_owned(),
        ));
    }
    for field in TEXT_HEADER_FIELDS {
        if !header.contains(field) {
            return Err(Error::MalformedText(format!("header names no {field}")));
        }
    }
    validate_shard(&shard)?;
    Ok(shard)
}

fn text_error(line: usize, message: impl std::fmt::Display) -> Error {
    Error::MalformedText(format!("line {}: {message}", line + 1))
}

fn text_uint(line: usize, name: &str, value: &str) -> Result<u32, Error> {
    value
        .parse::<u32>()
        .map_err(|_| text_error(line, format!("{name} is not a uint32: {value}")))
}

fn text_string(line: usize, name: &str, raw: &str) -> Result<String, Error> {
    let body = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .ok_or_else(|| text_error(line, format!("{name} is not a quoted string: {raw}")))?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some(other) => {
                    return Err(text_error(line, format!("{name} escapes \\{other}")));
                }
                None => {
                    return Err(text_error(
                        line,
                        format!("{name} ends in a dangling escape"),
                    ));
                }
            },
            '"' => return Err(text_error(line, format!("{name} leaves a quote unescaped"))),
            _ => out.push(ch),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use proto::{Extension, Param, Relations, Returns, SourceRef, SymbolKind, Visibility};

    fn example_shard() -> DocIr {
        DocIr {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            language: "python".to_owned(),
            package: "mylib".to_owned(),
            symbols: vec![Symbol {
                id: "python:mylib:AccountService.create".to_owned(),
                kind: SymbolKind::Method as i32,
                signature_text: "def create(self, input: AccountInput) -> Account".to_owned(),
                doc_markdown: "Creates a new account.".to_owned(),
                params: vec![Param {
                    name: "input".to_owned(),
                    r#type: "AccountInput".to_owned(),
                    doc: "Validated input.".to_owned(),
                }],
                returns: Some(Returns {
                    r#type: "Account".to_owned(),
                    doc: "The created account.".to_owned(),
                }),
                examples: vec!["```python\nsvc.create(data)\n```".to_owned()],
                source: Some(SourceRef {
                    file: "src/account.py".to_owned(),
                    line: 42,
                }),
                visibility: Visibility::Public as i32,
                relations: Some(Relations {
                    member_of: vec!["python:mylib:AccountService".to_owned()],
                    implements: vec![],
                }),
                extensions: vec![],
            }],
        }
    }

    #[test]
    fn documented_example_roundtrips_byte_identical() {
        let shard = example_shard();
        let bytes = encode_shard(&shard).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), shard);
        assert_eq!(encode_shard(&shard).unwrap(), bytes);
    }

    #[test]
    fn encode_and_decode_reject_the_same_invalid_shards() {
        let mut bad_major = example_shard();
        bad_major.schema_major = 0;
        assert_eq!(
            encode_shard(&bad_major),
            Err(Error::UnsupportedMajor { found: 0 })
        );

        let mut bad_lang = example_shard();
        bad_lang.language.clear();
        assert_eq!(encode_shard(&bad_lang), Err(Error::EmptyLanguage));

        let mut bad_pkg = example_shard();
        bad_pkg.package.clear();
        assert_eq!(encode_shard(&bad_pkg), Err(Error::EmptyPackage));

        let mut bad_id = example_shard();
        bad_id.symbols[0].id.clear();
        assert_eq!(
            encode_shard(&bad_id),
            Err(Error::EmptySymbolId { index: 0 })
        );

        let mut dup = example_shard();
        dup.symbols.push(dup.symbols[0].clone());
        assert_eq!(
            encode_shard(&dup),
            Err(Error::DuplicateSymbolId {
                id: "python:mylib:AccountService.create".to_owned(),
            })
        );

        let mut bad_path = example_shard();
        bad_path.symbols[0].source = Some(SourceRef {
            file: "/home/user/src/account.py".to_owned(),
            line: 42,
        });
        assert_eq!(
            encode_shard(&bad_path),
            Err(Error::UnsafeSourcePath {
                id: "python:mylib:AccountService.create".to_owned(),
                path: "/home/user/src/account.py".to_owned(),
                reason: "path must be workspace-relative, not absolute",
            })
        );

        let raw = bad_id.encode_to_vec();
        assert_eq!(decode_shard(&raw), Err(Error::EmptySymbolId { index: 0 }));
        assert!(matches!(decode_shard(&[0xff; 5]), Err(Error::Decode(_))));
    }

    #[test]
    fn every_unsafe_source_path_is_rejected_with_its_ladder_reason() {
        for (file, reason) in [
            ("", "path must be non-empty"),
            ("/abs/a.py", "path must be workspace-relative, not absolute"),
            ("..\\a.py", "path must use forward slashes"),
            ("a//b.py", "path must have no empty component"),
            ("a/./b.py", "path must have no '.' component"),
            ("../a.py", "path must have no '..' component"),
            ("a/../../b.py", "path must have no '..' component"),
        ] {
            assert_eq!(source_path_reason(file), Some(reason), "rung: {file:?}");
            let mut shard = example_shard();
            shard.symbols[0].source = Some(SourceRef {
                file: file.to_owned(),
                line: 42,
            });
            assert_eq!(
                encode_shard(&shard),
                Err(Error::UnsafeSourcePath {
                    id: "python:mylib:AccountService.create".to_owned(),
                    path: file.to_owned(),
                    reason,
                }),
                "source path accepted: {file:?}"
            );
        }
        assert_eq!(source_path_reason("src/account.py"), None);
    }

    #[test]
    fn decoding_rejects_a_symbol_whose_source_path_escapes_the_workspace() {
        let mut escaping = example_shard();
        escaping.symbols[0].source = Some(SourceRef {
            file: "../../../etc/passwd".to_owned(),
            line: 1,
        });
        let bytes = escaping.encode_to_vec();
        assert_eq!(
            decode_shard(&bytes),
            Err(Error::UnsafeSourcePath {
                id: "python:mylib:AccountService.create".to_owned(),
                path: "../../../etc/passwd".to_owned(),
                reason: "path must have no '..' component",
            })
        );
    }

    #[test]
    fn extension_keys_must_be_strictly_increasing() {
        let mut shard = example_shard();
        shard.symbols[0].extensions = vec![
            Extension {
                key: "b".to_owned(),
                value: b"2".to_vec(),
            },
            Extension {
                key: "a".to_owned(),
                value: b"1".to_vec(),
            },
        ];
        assert_eq!(
            encode_shard(&shard),
            Err(Error::UnsortedExtensions {
                id: "python:mylib:AccountService.create".to_owned(),
            })
        );

        shard.symbols[0].extensions = vec![
            Extension {
                key: "a".to_owned(),
                value: b"1".to_vec(),
            },
            Extension {
                key: "a".to_owned(),
                value: b"2".to_vec(),
            },
        ];
        assert_eq!(
            encode_shard(&shard),
            Err(Error::DuplicateExtension {
                id: "python:mylib:AccountService.create".to_owned(),
                key: "a".to_owned(),
            })
        );

        shard.symbols[0].extensions = vec![Extension {
            key: "a".to_owned(),
            value: b"1".to_vec(),
        }];
        let bytes = encode_shard(&shard).unwrap();
        assert_eq!(encode_shard(&shard).unwrap(), bytes);
    }

    #[test]
    fn symbols_must_be_strictly_increasing() {
        let mut second = example_shard().symbols[0].clone();
        second.id = "python:mylib:AccountService.delete".to_owned();
        second.source = Some(SourceRef {
            file: "src/account.py".to_owned(),
            line: 84,
        });

        let mut reversed = example_shard();
        reversed.symbols = vec![second.clone(), reversed.symbols.remove(0)];
        assert_eq!(
            encode_shard(&reversed),
            Err(Error::UnsortedSymbols {
                id: "python:mylib:AccountService.create".to_owned(),
            })
        );

        let mut ordered = example_shard();
        ordered.symbols.push(second);
        let bytes = encode_shard(&ordered).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), ordered);
        assert_eq!(encode_shard(&ordered).unwrap(), bytes);

        let raw = reversed.encode_to_vec();
        assert_eq!(
            decode_shard(&raw),
            Err(Error::UnsortedSymbols {
                id: "python:mylib:AccountService.create".to_owned(),
            })
        );
    }

    #[test]
    fn newer_minors_decode_when_understood() {
        let mut shard = example_shard();
        shard.schema_minor = 3;
        let bytes = encode_shard(&shard).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), shard);
    }

    fn text_header() -> String {
        format!(
            "schema_major: {SCHEMA_MAJOR}\nschema_minor: {SCHEMA_MINOR}\nlanguage: \"python\"\npackage: \"mylib\"\n"
        )
    }

    fn text_symbol(id: &str, doc: &str) -> String {
        format!("symbols {{\n  id: \"{id}\"\n  doc_markdown: \"{doc}\"\n}}\n")
    }

    #[test]
    fn text_shards_decode_into_the_same_message_the_binary_codec_encodes() {
        let text = text_header()
            + &text_symbol(
                "python:mylib:AccountService.create",
                "Creates a new account.",
            )
            + &text_symbol("python:mylib:AccountService.get", "Fetches by ID.");
        let decoded = decode_shard_text(&text).unwrap();
        assert_eq!(decoded.schema_major, SCHEMA_MAJOR);
        assert_eq!(decoded.schema_minor, SCHEMA_MINOR);
        assert_eq!(decoded.language, "python");
        assert_eq!(decoded.package, "mylib");
        assert_eq!(
            decoded.symbols,
            vec![
                Symbol {
                    id: "python:mylib:AccountService.create".to_owned(),
                    doc_markdown: "Creates a new account.".to_owned(),
                    ..Symbol::default()
                },
                Symbol {
                    id: "python:mylib:AccountService.get".to_owned(),
                    doc_markdown: "Fetches by ID.".to_owned(),
                    ..Symbol::default()
                },
            ]
        );
        let bytes = encode_shard(&decoded).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), decoded);
        assert_eq!(encode_shard(&decoded).unwrap(), bytes);
    }

    #[test]
    fn text_escapes_decode_back_to_the_original_prose() {
        let text = text_header()
            + &text_symbol("python:mylib:AccountService.escape", "")
            + &text_symbol(
                "python:mylib:AccountService.quote",
                r#"Reads C:\\temp and a \"quoted\" name."#,
            );
        let decoded = decode_shard_text(&text).unwrap();
        assert_eq!(decoded.symbols[0].doc_markdown, "");
        assert_eq!(
            decoded.symbols[1].doc_markdown,
            r#"Reads C:\temp and a "quoted" name."#
        );
    }

    #[test]
    fn text_decoding_runs_the_same_validation_as_the_binary_codec() {
        assert_eq!(
            decode_shard_text(&text_header().replace('1', "0")),
            Err(Error::UnsupportedMajor { found: 0 })
        );
        assert_eq!(
            decode_shard_text(
                &(text_header().replace("\"mylib\"", "\"\"") + &text_symbol("a", ""))
            ),
            Err(Error::EmptyPackage)
        );
        assert_eq!(
            decode_shard_text(
                &(text_header()
                    + &text_symbol("python:mylib:b", "")
                    + &text_symbol("python:mylib:a", ""))
            ),
            Err(Error::UnsortedSymbols {
                id: "python:mylib:a".to_owned()
            })
        );
        assert_eq!(
            decode_shard_text(&(text_header() + &text_symbol("", ""))),
            Err(Error::EmptySymbolId { index: 0 })
        );
    }

    #[test]
    fn text_decoding_rejects_malformed_and_unsupported_text() {
        let head = text_header();
        for (text, expected) in [
            (
                format!("{head}kind: FUNCTION\n"),
                "line 5: header field not supported: kind",
            ),
            (
                format!("{head}language: \"python\"\n"),
                "line 5: header repeats language",
            ),
            (
                "schema_major: 1x\n".to_owned(),
                "line 1: schema_major is not a uint32: 1x",
            ),
            (
                "language: python\n".to_owned(),
                "line 1: language is not a quoted string: python",
            ),
            (
                "language: \"python\n".to_owned(),
                "line 1: language is not a quoted string: \"python",
            ),
            (
                "language: \"py\"thon\"\n".to_owned(),
                "line 1: language leaves a quote unescaped",
            ),
            (
                "language: \"a\\nb\"\n".to_owned(),
                "line 1: language escapes \\n",
            ),
            (
                "language: \"a\\\"\n".to_owned(),
                "line 1: language ends in a dangling escape",
            ),
            (
                "schema_major: 1\n".to_owned(),
                "header names no schema_minor",
            ),
            ("symbol {\n}\n".to_owned(), "line 1: unknown block symbol"),
            (
                format!("{head}symbols {{\nsymbols {{\n}}\n}}\n"),
                "line 6: symbols blocks do not nest",
            ),
            (
                format!("{head}symbols {{\n"),
                "symbols block is never closed",
            ),
            ("}\n".to_owned(), "line 1: } closes no block"),
            (
                format!("{head}symbols {{\n  id: \"a\"\n}}\n"),
                "line 7: symbol names no doc_markdown",
            ),
            (
                format!("{head}symbols {{\n  id: \"a\"\n  id: \"b\"\n}}\n"),
                "line 7: symbol repeats id",
            ),
            (
                format!("{head}symbols {{\n  kind: FUNCTION\n}}\n"),
                "line 6: symbol field not supported: kind",
            ),
            (format!("{head}symbols\n"), "line 5: symbols names no field"),
            (
                format!("{head}symbols {{\n  doc_markdown: \"first\nsecond\"\n}}\n"),
                "line 6: doc_markdown is not a quoted string: \"first",
            ),
        ] {
            assert_eq!(
                decode_shard_text(&text),
                Err(Error::MalformedText(expected.to_owned())),
                "text must not decode: {text}"
            );
        }
    }
}
