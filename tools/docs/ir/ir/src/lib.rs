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

use dx_path::reject_reason;
use dx_proto_validate::{
    check_sorted_next, decode_with_validation, encode_with_validation, OrderViolation,
};
pub use dx_schema::SCHEMA_MAJOR;
pub use dx_schema::SCHEMA_MINOR;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Decode(String),
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
    UnsortedAliases {
        id: String,
    },
    DuplicateAlias {
        id: String,
        alias: String,
    },
    ForeignAlias {
        id: String,
        alias: String,
    },
    UnsafeDisplayName {
        id: String,
    },
    OrphanMember {
        id: String,
        parent: String,
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
    let prefix = format!("{}:{}:", shard.language, shard.package);
    let mut previous_id: Option<&String> = None;
    for (index, symbol) in shard.symbols.iter().enumerate() {
        validate_symbol(symbol, index, &prefix)?;
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
    validate_component_members(shard)
}

fn validate_component_members(shard: &DocIr) -> Result<(), Error> {
    let kind_of = |id: &str| {
        shard
            .symbols
            .iter()
            .find(|symbol| symbol.id == id)
            .map(|symbol| symbol.kind)
    };
    for symbol in &shard.symbols {
        let parents: &[String] = symbol
            .relations
            .as_ref()
            .map_or(&[], |relations| relations.member_of.as_slice());
        if symbol.kind == proto::SymbolKind::Component as i32 {
            if let Some(parent) = parents.first() {
                return Err(Error::OrphanMember {
                    id: symbol.id.clone(),
                    parent: parent.clone(),
                });
            }
        } else if symbol.kind == proto::SymbolKind::Event as i32
            || symbol.kind == proto::SymbolKind::Slot as i32
        {
            let component = proto::SymbolKind::Component as i32;
            match parents {
                [parent] if kind_of(parent) == Some(component) => {}
                _ => {
                    return Err(Error::OrphanMember {
                        id: symbol.id.clone(),
                        parent: parents.join(","),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_symbol(symbol: &Symbol, index: usize, prefix: &str) -> Result<(), Error> {
    if symbol.id.is_empty() {
        return Err(Error::EmptySymbolId { index });
    }
    if symbol.display_name.chars().any(char::is_control) {
        return Err(Error::UnsafeDisplayName {
            id: symbol.id.clone(),
        });
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
    validate_aliases(symbol, prefix)
}

fn validate_aliases(symbol: &Symbol, prefix: &str) -> Result<(), Error> {
    let Some(relations) = symbol.relations.as_ref() else {
        return Ok(());
    };
    let mut previous: Option<&String> = None;
    for alias in relations.reexported_as.iter() {
        if !alias.starts_with(prefix) || alias.len() == prefix.len() {
            return Err(Error::ForeignAlias {
                id: symbol.id.clone(),
                alias: alias.clone(),
            });
        }
        match check_sorted_next(previous, alias) {
            Ok(()) => {}
            Err(OrderViolation::Duplicate) => {
                return Err(Error::DuplicateAlias {
                    id: symbol.id.clone(),
                    alias: alias.clone(),
                });
            }
            Err(OrderViolation::Unsorted) => {
                return Err(Error::UnsortedAliases {
                    id: symbol.id.clone(),
                });
            }
        }
        previous = Some(alias);
    }
    Ok(())
}

pub fn encode_shard(shard: &DocIr) -> Result<Vec<u8>, Error> {
    encode_with_validation(shard, validate_shard)
}

pub fn decode_shard(bytes: &[u8]) -> Result<DocIr, Error> {
    decode_with_validation(bytes, validate_shard, Error::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_segment_keeps_word_bytes_and_hex_escapes_the_rest() {
        assert_eq!(escape_segment("Ab_9"), "Ab_9");
        assert_eq!(
            escape_segment("scale(double,double)"),
            "scale~28double~2Cdouble~29"
        );
        assert_eq!(escape_segment("é"), "~C3~A9");
    }
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
                    generated: false,
                }),
                visibility: Visibility::Public as i32,
                relations: Some(Relations {
                    member_of: vec!["python:mylib:AccountService".to_owned()],
                    implements: vec![],
                    reexported_as: vec!["python:mylib:create".to_owned()],
                }),
                extensions: vec![],
                display_name: String::new(),
            }],
        }
    }

    #[test]
    fn a_display_name_roundtrips_and_may_be_empty() {
        let mut shard = example_shard();
        shard.symbols[0].display_name = "AccountService#create?".to_owned();
        let bytes = encode_shard(&shard).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), shard);
        assert_eq!(encode_shard(&shard).unwrap(), bytes);
        assert_eq!(example_shard().symbols[0].display_name, "");
        assert!(encode_shard(&example_shard()).is_ok());
    }

    #[test]
    fn a_display_name_with_a_control_character_is_rejected() {
        for bad in ["Account\nService", "Account\u{7}", "\r", "tab\there"] {
            let mut shard = example_shard();
            shard.symbols[0].display_name = bad.to_owned();
            let expected = Error::UnsafeDisplayName {
                id: "python:mylib:AccountService.create".to_owned(),
            };
            assert_eq!(encode_shard(&shard), Err(expected.clone()), "{bad:?}");
            assert_eq!(
                decode_shard(&shard.encode_to_vec()),
                Err(expected),
                "{bad:?}"
            );
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
    fn a_variable_symbol_roundtrips() {
        let mut shard = example_shard();
        shard.symbols[0].kind = SymbolKind::Variable as i32;
        shard.symbols[0].relations = None;
        let bytes = encode_shard(&shard).unwrap();
        let decoded = decode_shard(&bytes).unwrap();
        assert_eq!(decoded.symbols[0].kind(), SymbolKind::Variable);
        assert_eq!(SymbolKind::Variable as i32, 13);
        assert_eq!(decoded, shard);
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
            ..SourceRef::default()
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
                ..SourceRef::default()
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
            ..SourceRef::default()
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
            ..SourceRef::default()
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

    fn component_shard() -> DocIr {
        let mut shard = example_shard();
        let make = |id: &str, kind: SymbolKind, parent: Option<&str>| Symbol {
            id: format!("vue:mylib:{id}"),
            kind: kind as i32,
            relations: parent.map(|parent| Relations {
                member_of: vec![format!("vue:mylib:{parent}")],
                ..Relations::default()
            }),
            ..Symbol::default()
        };
        shard.language = "vue".to_owned();
        shard.symbols = vec![
            make("Button", SymbolKind::Component, None),
            make("Button.event_click", SymbolKind::Event, Some("Button")),
            make("Button.exposed_focus", SymbolKind::Method, Some("Button")),
            make("Button.prop_label", SymbolKind::Property, Some("Button")),
            make("Button.slot_default", SymbolKind::Slot, Some("Button")),
        ];
        shard
    }

    #[test]
    fn a_component_with_all_member_kinds_validates() {
        let shard = component_shard();
        let bytes = encode_shard(&shard).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), shard);
    }

    #[test]
    fn events_and_slots_need_a_component_parent() {
        let mut orphan = component_shard();
        orphan.symbols[1].relations = None;
        assert_eq!(
            encode_shard(&orphan),
            Err(Error::OrphanMember {
                id: "vue:mylib:Button.event_click".to_owned(),
                parent: String::new(),
            })
        );
        let mut wrong = component_shard();
        wrong.symbols[0].kind = SymbolKind::Class as i32;
        assert!(matches!(
            encode_shard(&wrong),
            Err(Error::OrphanMember { .. })
        ));
    }

    #[test]
    fn a_component_cannot_be_a_member() {
        let mut shard = component_shard();
        shard.symbols[0].relations = Some(Relations {
            member_of: vec!["vue:mylib:Other".to_owned()],
            ..Relations::default()
        });
        assert_eq!(
            encode_shard(&shard),
            Err(Error::OrphanMember {
                id: "vue:mylib:Button".to_owned(),
                parent: "vue:mylib:Other".to_owned(),
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

    #[test]
    fn reexport_aliases_must_be_sorted_unique_and_in_the_shard() {
        let mut shard = example_shard();
        let relations = shard.symbols[0].relations.as_mut().unwrap();
        relations.reexported_as = vec!["python:mylib:b".to_owned(), "python:mylib:a".to_owned()];
        assert_eq!(
            encode_shard(&shard),
            Err(Error::UnsortedAliases {
                id: "python:mylib:AccountService.create".to_owned(),
            })
        );

        let relations = shard.symbols[0].relations.as_mut().unwrap();
        relations.reexported_as = vec!["python:mylib:a".to_owned(), "python:mylib:a".to_owned()];
        assert_eq!(
            encode_shard(&shard),
            Err(Error::DuplicateAlias {
                id: "python:mylib:AccountService.create".to_owned(),
                alias: "python:mylib:a".to_owned(),
            })
        );

        for foreign in ["python:other:a", "rust:mylib:a", "python:mylib:"] {
            let relations = shard.symbols[0].relations.as_mut().unwrap();
            relations.reexported_as = vec![foreign.to_owned()];
            assert_eq!(
                encode_shard(&shard),
                Err(Error::ForeignAlias {
                    id: "python:mylib:AccountService.create".to_owned(),
                    alias: foreign.to_owned(),
                }),
                "alias accepted: {foreign}"
            );
        }

        let relations = shard.symbols[0].relations.as_mut().unwrap();
        relations.reexported_as = vec!["python:mylib:a".to_owned(), "python:mylib:b".to_owned()];
        let bytes = encode_shard(&shard).unwrap();
        assert_eq!(decode_shard(&bytes).unwrap(), shard);
    }
}
