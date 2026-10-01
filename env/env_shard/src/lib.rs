#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub use plan_proto::rules_dx::env as proto;
use proto::DxEnvShard;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("cannot decode env shard: {0}")]
    Decode(String),
    #[error("empty producer: want a non-empty label")]
    EmptyProducer,
    #[error("invalid producer {value:?}: want a // or @ label")]
    BadProducer { value: String },
    #[error("empty integration for {producer}: want a non-empty integration")]
    EmptyIntegration { producer: String },
    #[error("empty entries for {producer}: want at least one entry")]
    EmptyEntries { producer: String },
    #[error("invalid key for {producer} {key:?}: {reason}")]
    BadKey {
        producer: String,
        key: String,
        reason: &'static str,
    },
    #[error("invalid value for {producer} {key:?}: {reason}")]
    BadValue {
        producer: String,
        key: String,
        reason: &'static str,
    },
    #[error("invalid exec path for {producer} {path:?}: {reason}")]
    BadExecPath {
        producer: String,
        path: String,
        reason: &'static str,
    },
    #[error("duplicate key for {producer} {key:?}")]
    DuplicateKey { producer: String, key: String },
}

fn check_key(producer: &str, key: &str) -> Result<(), Error> {
    let reason = if key.is_empty() {
        Some("must be a non-empty single token")
    } else if key.contains('/') || key.contains('\\') {
        Some("must not contain '/' or '\\'")
    } else if key.contains('|') {
        Some("must not contain '|'")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(Error::BadKey {
            producer: producer.to_owned(),
            key: key.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

fn check_value(producer: &str, key: &str, value: &str) -> Result<(), Error> {
    let reason = if value.is_empty() {
        Some("must be a non-empty identity input")
    } else if value.contains('|') {
        Some("must not contain '|'")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(Error::BadValue {
            producer: producer.to_owned(),
            key: key.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

fn check_exec_path(producer: &str, path: &str) -> Result<(), Error> {
    if path.is_empty() {
        return Ok(());
    }
    if path.ends_with(".dxenv.pb") {
        return Err(Error::BadExecPath {
            producer: producer.to_owned(),
            path: path.to_owned(),
            reason: "must not use the reserved shard suffix",
        });
    }
    let reason = match dx_path::classify(path) {
        None => None,
        Some(dx_path::PathProblem::Empty) => None,
        Some(dx_path::PathProblem::Absolute) => Some("must not be absolute"),
        Some(dx_path::PathProblem::Backslash) => Some("must not contain '\\'"),
        Some(dx_path::PathProblem::EmptyComponent) => None,
        Some(dx_path::PathProblem::Dot) | Some(dx_path::PathProblem::DotDot) => {
            Some("must not contain '.' or '..' segments")
        }
    };
    match reason {
        Some(reason) => Err(Error::BadExecPath {
            producer: producer.to_owned(),
            path: path.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

pub fn validate(shard: &DxEnvShard) -> Result<(), Error> {
    if shard.producer.is_empty() {
        return Err(Error::EmptyProducer);
    }
    if !(shard.producer.starts_with("//") || shard.producer.starts_with('@')) {
        return Err(Error::BadProducer {
            value: shard.producer.clone(),
        });
    }
    if shard.integration.is_empty() {
        return Err(Error::EmptyIntegration {
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
        check_key(&shard.producer, &entry.key)?;
        check_value(&shard.producer, &entry.key, &entry.value)?;
        check_exec_path(&shard.producer, &entry.exec_path)?;
        dx_proto_validate::check_unique_insert(&mut seen, &entry.key, |existing| {
            Error::DuplicateKey {
                producer: shard.producer.clone(),
                key: (*existing).clone(),
            }
        })?;
    }
    Ok(())
}

pub fn encode_validated(shard: &DxEnvShard) -> Result<Vec<u8>, Error> {
    dx_proto_validate::encode_with_validation(shard, validate)
}

pub fn decode_validated(bytes: &[u8]) -> Result<DxEnvShard, Error> {
    dx_proto_validate::decode_with_validation(bytes, validate, Error::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use proto::DxEnvEntry;

    fn entry(key: &str, value: &str) -> DxEnvEntry {
        DxEnvEntry {
            key: key.into(),
            value: value.into(),
            exec_path: String::new(),
        }
    }

    fn entry_with_exec(key: &str, value: &str, exec_path: &str) -> DxEnvEntry {
        DxEnvEntry {
            key: key.into(),
            value: value.into(),
            exec_path: exec_path.into(),
        }
    }

    fn sample() -> DxEnvShard {
        DxEnvShard {
            producer: "//env:alpha".into(),
            integration: "rust".into(),
            entries: vec![entry("abi", "gnu"), entry("runtime", "stable-x86_64")],
        }
    }

    #[test]
    fn roundtrip_preserves_entries_and_exec_paths() {
        let shard = sample();
        let bytes = encode_validated(&shard).unwrap();
        assert_eq!(decode_validated(&bytes).unwrap(), shard);
        let mut with_exec = sample();
        with_exec.entries[0] = entry_with_exec("abi", "gnu", "toolchain/rustc");
        let bytes = encode_validated(&with_exec).unwrap();
        assert_eq!(decode_validated(&bytes).unwrap(), with_exec);
    }

    #[test]
    fn rejects_bad_producers() {
        let mut shard = sample();
        shard.producer.clear();
        assert_eq!(validate(&shard), Err(Error::EmptyProducer));
        shard.producer = "env:alpha".into();
        assert_eq!(
            validate(&shard),
            Err(Error::BadProducer {
                value: "env:alpha".into(),
            })
        );
    }

    #[test]
    fn rejects_empty_integration_and_empty_entries() {
        let mut shard = sample();
        shard.integration.clear();
        assert_eq!(
            validate(&shard),
            Err(Error::EmptyIntegration {
                producer: "//env:alpha".into(),
            })
        );
        let mut shard = sample();
        shard.entries.clear();
        assert_eq!(
            validate(&shard),
            Err(Error::EmptyEntries {
                producer: "//env:alpha".into(),
            })
        );
    }

    #[test]
    fn rejects_bad_keys_and_values() {
        for key in ["", "a/b", "a\\b", "a|b"] {
            let mut shard = sample();
            shard.entries[0].key = key.into();
            assert!(
                matches!(validate(&shard), Err(Error::BadKey { .. })),
                "key {key:?} must fail",
            );
        }
        for value in ["", "a|b"] {
            let mut shard = sample();
            shard.entries[0].value = value.into();
            assert!(
                matches!(validate(&shard), Err(Error::BadValue { .. })),
                "value {value:?} must fail",
            );
        }
        assert!(validate(&sample()).is_ok());
        for path in ["/out/rustc", "tool\\chain", "src/./rustc", "a.dxenv.pb"] {
            let mut shard = sample();
            shard.entries[0].exec_path = path.into();
            assert!(
                matches!(validate(&shard), Err(Error::BadExecPath { .. })),
                "exec path {path:?} must fail",
            );
        }
        let mut ok = sample();
        ok.entries[0].exec_path = "toolchain/rustc".into();
        assert!(validate(&ok).is_ok());
    }

    #[test]
    fn exec_path_messages_are_pinned_to_dx_path_ladder() {
        for (path, reason) in [
            ("/out/rustc", "must not be absolute"),
            ("C:/out/rustc", "must not be absolute"),
            ("tool\\chain", "must not contain '\\'"),
            ("src/./rustc", "must not contain '.' or '..' segments"),
            ("src/../rustc", "must not contain '.' or '..' segments"),
            ("a.dxenv.pb", "must not use the reserved shard suffix"),
        ] {
            let mut shard = sample();
            shard.entries[0].exec_path = path.into();
            assert_eq!(
                validate(&shard),
                Err(Error::BadExecPath {
                    producer: "//env:alpha".into(),
                    path: path.into(),
                    reason,
                }),
                "exec path {path:?}"
            );
        }
        assert!(validate(&sample()).is_ok());
        let mut allowed = sample();
        allowed.entries[0].exec_path = "a//b".into();
        assert!(
            validate(&allowed).is_ok(),
            "empty-component stays allowed for Starlark parity"
        );
        let mut ordered = sample();
        ordered.entries[0].exec_path = "/src/./rustc".into();
        assert_eq!(
            validate(&ordered),
            Err(Error::BadExecPath {
                producer: "//env:alpha".into(),
                path: "/src/./rustc".into(),
                reason: "must not be absolute",
            })
        );
    }

    #[test]
    fn rejects_duplicate_keys() {
        let mut shard = sample();
        shard.entries[1].key = shard.entries[0].key.clone();
        assert_eq!(
            validate(&shard),
            Err(Error::DuplicateKey {
                producer: "//env:alpha".into(),
                key: "abi".into(),
            })
        );
    }

    #[test]
    fn rejects_garbage_bytes() {
        assert!(matches!(
            decode_validated(&[0xff; 5]),
            Err(Error::Decode(_))
        ));
        let mut shard = sample();
        shard.integration.clear();
        let bytes = shard.encode_to_vec();
        assert_eq!(
            decode_validated(&bytes),
            Err(Error::EmptyIntegration {
                producer: "//env:alpha".into(),
            })
        );
    }

    #[test]
    fn display_names_the_error() {
        assert!(Error::EmptyProducer.to_string().contains("empty producer"));
    }
}
