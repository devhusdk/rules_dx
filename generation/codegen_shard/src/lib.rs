#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub use codegen_proto::rules_dx::codegen as proto;
use proto::DxCodegenShard;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("cannot decode codegen shard: {0}")]
    Decode(String),
    #[error("empty producer: want a non-empty label")]
    EmptyProducer,
    #[error("invalid producer {value:?}: want a // or @ label")]
    BadProducer { value: String },
    #[error("empty language for {producer}: want a non-empty language")]
    EmptyLanguage { producer: String },
    #[error("empty entries for {producer}: want at least one entry")]
    EmptyEntries { producer: String },
    #[error("invalid path for {producer} {path:?}: {reason}")]
    BadPath {
        producer: String,
        path: String,
        reason: &'static str,
    },
    #[error("invalid exec path for {producer} {path:?}: {reason}")]
    BadExecPath {
        producer: String,
        path: String,
        reason: &'static str,
    },
    #[error("invalid replaces for {producer} {path:?}: {reason}")]
    BadReplaces {
        producer: String,
        path: String,
        reason: &'static str,
    },
    #[error("duplicate logical path for {producer} {path:?}")]
    DuplicateLogicalPath { producer: String, path: String },
    #[error("entry not read-only for {producer} {path:?}: projections are always read-only")]
    NotReadOnly { producer: String, path: String },
}

fn check_path(producer: &str, path: &str) -> Result<(), Error> {
    let reason = match dx_path::classify(path) {
        None => None,
        Some(dx_path::PathProblem::Empty) => Some("must be a non-empty workspace-relative path"),
        Some(dx_path::PathProblem::Absolute) => Some("must not be absolute"),
        Some(dx_path::PathProblem::Backslash) => Some("must not contain '\\'"),
        Some(dx_path::PathProblem::EmptyComponent) => None,
        Some(dx_path::PathProblem::Dot) | Some(dx_path::PathProblem::DotDot) => {
            Some("must not contain '.' or '..' segments")
        }
    };
    match reason {
        Some(reason) => Err(Error::BadPath {
            producer: producer.to_owned(),
            path: path.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

fn check_exec_path(producer: &str, path: &str) -> Result<(), Error> {
    if path.is_empty() {
        return Ok(());
    }
    if path.ends_with(".dxcodegen.pb") {
        return Err(Error::BadExecPath {
            producer: producer.to_owned(),
            path: path.to_owned(),
            reason: "must not use the reserved shard suffix",
        });
    }
    check_path(producer, path).map_err(|error| match error {
        Error::BadPath {
            producer,
            path,
            reason,
        } => Error::BadExecPath {
            producer,
            path,
            reason,
        },
        other => other,
    })
}

fn check_replaces(
    producer: &str,
    logical_path: &str,
    exec_path: &str,
    replaces: &str,
) -> Result<(), Error> {
    if replaces.is_empty() {
        return Ok(());
    }
    if let Err(error) = check_path(producer, replaces) {
        let (path, reason) = match error {
            Error::BadPath { path, reason, .. } => (path, reason),
            other => return Err(other),
        };
        return Err(Error::BadReplaces {
            producer: producer.to_owned(),
            path,
            reason,
        });
    }
    if replaces != logical_path {
        return Err(Error::BadReplaces {
            producer: producer.to_owned(),
            path: replaces.to_owned(),
            reason: "must equal logical path",
        });
    }
    if exec_path.is_empty() {
        return Err(Error::BadReplaces {
            producer: producer.to_owned(),
            path: replaces.to_owned(),
            reason: "needs a non-empty exec path binding the replacing artifact",
        });
    }
    Ok(())
}

pub fn validate(shard: &DxCodegenShard) -> Result<(), Error> {
    if shard.producer.is_empty() {
        return Err(Error::EmptyProducer);
    }
    if !(shard.producer.starts_with("//") || shard.producer.starts_with('@')) {
        return Err(Error::BadProducer {
            value: shard.producer.clone(),
        });
    }
    if shard.language.is_empty() {
        return Err(Error::EmptyLanguage {
            producer: shard.producer.clone(),
        });
    }
    if shard.entries.is_empty() {
        return Err(Error::EmptyEntries {
            producer: shard.producer.clone(),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in &shard.entries {
        check_path(&shard.producer, &entry.logical_path)?;
        check_path(&shard.producer, &entry.import_root)?;
        check_exec_path(&shard.producer, &entry.exec_path)?;
        check_replaces(
            &shard.producer,
            &entry.logical_path,
            &entry.exec_path,
            &entry.replaces,
        )?;
        if !entry.read_only {
            return Err(Error::NotReadOnly {
                producer: shard.producer.clone(),
                path: entry.logical_path.clone(),
            });
        }
        dx_proto_validate::check_unique_insert(&mut seen, &entry.logical_path, |existing| {
            Error::DuplicateLogicalPath {
                producer: shard.producer.clone(),
                path: (*existing).clone(),
            }
        })?;
    }
    Ok(())
}

pub fn encode_validated(shard: &DxCodegenShard) -> Result<Vec<u8>, Error> {
    dx_proto_validate::encode_with_validation(shard, validate)
}

pub fn decode_validated(bytes: &[u8]) -> Result<DxCodegenShard, Error> {
    dx_proto_validate::decode_with_validation(bytes, validate, Error::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use proto::DxCodegenEntry;

    fn entry(logical_path: &str, import_root: &str, namespace: &str) -> DxCodegenEntry {
        DxCodegenEntry {
            logical_path: logical_path.into(),
            import_root: import_root.into(),
            namespace: namespace.into(),
            read_only: true,
            exec_path: String::new(),
            replaces: String::new(),
        }
    }

    fn entry_with_exec(
        logical_path: &str,
        import_root: &str,
        namespace: &str,
        exec_path: &str,
    ) -> DxCodegenEntry {
        DxCodegenEntry {
            logical_path: logical_path.into(),
            import_root: import_root.into(),
            namespace: namespace.into(),
            read_only: true,
            exec_path: exec_path.into(),
            replaces: String::new(),
        }
    }

    fn entry_with_replaces(
        logical_path: &str,
        import_root: &str,
        namespace: &str,
        exec_path: &str,
        replaces: &str,
    ) -> DxCodegenEntry {
        DxCodegenEntry {
            logical_path: logical_path.into(),
            import_root: import_root.into(),
            namespace: namespace.into(),
            read_only: true,
            exec_path: exec_path.into(),
            replaces: replaces.into(),
        }
    }

    fn sample() -> DxCodegenShard {
        DxCodegenShard {
            producer: "//gen:alpha".into(),
            language: "rust".into(),
            entries: vec![
                entry("src/beta.rs", "src", "beta"),
                entry("src/alpha.rs", "src", "alpha"),
            ],
        }
    }

    #[test]
    fn roundtrip_preserves_field_order_and_read_only() {
        let shard = sample();
        let bytes = encode_validated(&shard).unwrap();
        assert_eq!(decode_validated(&bytes).unwrap(), shard);
        let mut with_exec = sample();
        with_exec.entries[0] = entry_with_exec("src/beta.rs", "src", "beta", "result.lib.rs");
        let bytes = encode_validated(&with_exec).unwrap();
        assert_eq!(decode_validated(&bytes).unwrap(), with_exec);
    }

    #[test]
    fn rejects_bad_producers() {
        let mut shard = sample();
        shard.producer.clear();
        assert_eq!(validate(&shard), Err(Error::EmptyProducer));
        shard.producer = "gen:alpha".into();
        assert_eq!(
            validate(&shard),
            Err(Error::BadProducer {
                value: "gen:alpha".into(),
            })
        );
    }

    #[test]
    fn rejects_empty_language_and_empty_entries() {
        let mut shard = sample();
        shard.language.clear();
        assert_eq!(
            validate(&shard),
            Err(Error::EmptyLanguage {
                producer: "//gen:alpha".into(),
            })
        );
        let mut shard = sample();
        shard.entries.clear();
        assert_eq!(
            validate(&shard),
            Err(Error::EmptyEntries {
                producer: "//gen:alpha".into(),
            })
        );
    }

    #[test]
    fn rejects_bad_paths() {
        for path in [
            "",
            "/src/a.rs",
            "C:/src/a.rs",
            "src\\a.rs",
            "src/./a.rs",
            "src/../a.rs",
        ] {
            let mut shard = sample();
            shard.entries[0].logical_path = path.into();
            assert!(
                matches!(validate(&shard), Err(Error::BadPath { .. })),
                "logical path {path:?} must fail",
            );
            let mut shard = sample();
            shard.entries[0].import_root = path.into();
            assert!(
                matches!(validate(&shard), Err(Error::BadPath { .. })),
                "import root {path:?} must fail",
            );
        }
        assert!(validate(&sample()).is_ok());
        for path in ["/out/a.rs", "src\\a.rs", "src/./a.rs", "a.dxcodegen.pb"] {
            let mut shard = sample();
            shard.entries[0].exec_path = path.into();
            assert!(
                matches!(validate(&shard), Err(Error::BadExecPath { .. })),
                "exec path {path:?} must fail",
            );
        }
        let mut ok = sample();
        ok.entries[0].exec_path = "result_proto.lib.rs".into();
        assert!(validate(&ok).is_ok());
    }

    #[test]
    fn path_messages_are_pinned_to_dx_path_ladder() {
        for (path, reason) in [
            ("", "must be a non-empty workspace-relative path"),
            ("/src/a.rs", "must not be absolute"),
            ("C:/src/a.rs", "must not be absolute"),
            ("src\\a.rs", "must not contain '\\'"),
            ("src/./a.rs", "must not contain '.' or '..' segments"),
            ("src/../a.rs", "must not contain '.' or '..' segments"),
        ] {
            let mut shard = sample();
            shard.entries[0].logical_path = path.into();
            assert_eq!(
                validate(&shard),
                Err(Error::BadPath {
                    producer: "//gen:alpha".into(),
                    path: path.into(),
                    reason,
                }),
                "logical path {path:?}"
            );
        }
        let mut allowed = sample();
        allowed.entries[0].logical_path = "a//b".into();
        allowed.entries[0].import_root = "src".into();
        assert!(
            validate(&allowed).is_ok(),
            "empty-component stays allowed for Starlark parity"
        );
        let mut ordered = sample();
        ordered.entries[0].logical_path = "/src/./a.rs".into();
        assert_eq!(
            validate(&ordered),
            Err(Error::BadPath {
                producer: "//gen:alpha".into(),
                path: "/src/./a.rs".into(),
                reason: "must not be absolute",
            })
        );
        let mut ordered = sample();
        ordered.entries[0].logical_path = "src\\/./a.rs".into();
        assert_eq!(
            validate(&ordered),
            Err(Error::BadPath {
                producer: "//gen:alpha".into(),
                path: "src\\/./a.rs".into(),
                reason: "must not contain '\\'",
            })
        );
    }

    #[test]
    fn rejects_duplicates_and_writable_entries() {
        let mut shard = sample();
        shard.entries[1].logical_path = shard.entries[0].logical_path.clone();
        assert_eq!(
            validate(&shard),
            Err(Error::DuplicateLogicalPath {
                producer: "//gen:alpha".into(),
                path: "src/beta.rs".into(),
            })
        );
        let mut shard = sample();
        shard.entries[0].read_only = false;
        assert_eq!(
            validate(&shard),
            Err(Error::NotReadOnly {
                producer: "//gen:alpha".into(),
                path: "src/beta.rs".into(),
            })
        );
    }

    #[test]
    fn accepts_explicit_replacement_contract() {
        let mut shard = sample();
        shard.entries[0] =
            entry_with_replaces("src/beta.rs", "src", "beta", "result.lib.rs", "src/beta.rs");
        assert!(validate(&shard).is_ok());
        let bytes = encode_validated(&shard).expect("encode contracted shard");
        assert_eq!(decode_validated(&bytes).expect("decode"), shard);
    }

    #[test]
    fn rejects_bad_replacement_contracts() {
        let mut shard = sample();
        shard.entries[0] = entry_with_replaces("src/beta.rs", "src", "beta", "", "src/beta.rs");
        assert!(
            matches!(validate(&shard), Err(Error::BadReplaces { .. })),
            "replaces without exec must fail",
        );
        let mut shard = sample();
        shard.entries[0] = entry_with_replaces(
            "src/beta.rs",
            "src",
            "beta",
            "result.lib.rs",
            "src/other.rs",
        );
        assert!(
            matches!(validate(&shard), Err(Error::BadReplaces { .. })),
            "cross-path replaces must fail",
        );
        for path in ["/src/a.rs", "src\\a.rs", "src/./a.rs"] {
            let mut shard = sample();
            shard.entries[0] =
                entry_with_replaces("src/beta.rs", "src", "beta", "result.lib.rs", path);
            assert!(
                matches!(validate(&shard), Err(Error::BadReplaces { .. })),
                "replaces {path:?} must fail",
            );
        }
        assert!(validate(&sample()).is_ok());
    }

    #[test]
    fn rejects_garbage_bytes() {
        assert!(matches!(
            decode_validated(&[0xff; 5]),
            Err(Error::Decode(_))
        ));
        let mut shard = sample();
        shard.language.clear();
        let bytes = shard.encode_to_vec();
        assert_eq!(
            decode_validated(&bytes),
            Err(Error::EmptyLanguage {
                producer: "//gen:alpha".into(),
            })
        );
    }

    #[test]
    fn display_names_the_error() {
        assert!(Error::EmptyProducer.to_string().contains("empty producer"));
    }
}
