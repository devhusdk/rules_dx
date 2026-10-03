use crate::lifecycle::base;
use crate::validation::{check_edits, check_path, parse_digest, Edit, OutputError};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Modify,
    Create,
}

impl ChangeKind {
    fn name(self) -> &'static str {
        match self {
            ChangeKind::Modify => "modify",
            ChangeKind::Create => "create",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeEvent {
    pub path: String,
    pub kind: ChangeKind,
    pub source_digest: Option<String>,
    pub edits: Vec<Edit>,
}

pub fn change_event(change: &ChangeEvent) -> Result<Value, OutputError> {
    check_change(change)?;
    Ok(change_value(change))
}

fn check_change(change: &ChangeEvent) -> Result<(), OutputError> {
    check_path(&change.path)?;
    check_edits(&change.edits)?;
    match change.kind {
        ChangeKind::Modify => {
            let digest = change
                .source_digest
                .as_deref()
                .ok_or(OutputError::BadDigest {
                    field: "source_digest",
                    value: String::new(),
                })?;
            parse_digest("source_digest", digest)?;
        }
        ChangeKind::Create => {
            if change.source_digest.is_some() {
                return Err(OutputError::BadDigest {
                    field: "source_digest",
                    value: change.source_digest.clone().unwrap_or_default(),
                });
            }
            if change.edits.len() != 1 || change.edits[0].start != 0 || change.edits[0].end != 0 {
                return Err(OutputError::BadEdit {
                    index: 0,
                    reason: "create holds exactly one 0..0 insertion",
                });
            }
        }
    }
    Ok(())
}

/// Builds the event for a change whose producer already validated it.
pub fn change_value(change: &ChangeEvent) -> Value {
    let mut map = base("change");
    map.insert("path".to_owned(), Value::String(change.path.clone()));
    map.insert(
        "kind".to_owned(),
        Value::String(change.kind.name().to_owned()),
    );
    if let Some(digest) = &change.source_digest {
        map.insert("source_digest".to_owned(), Value::String(digest.clone()));
    }
    map.insert(
        "edits".to_owned(),
        Value::Array(
            change
                .edits
                .iter()
                .map(|e| {
                    json!({
                        "start_byte": e.start,
                        "end_byte": e.end,
                        "replacement": e.replacement,
                    })
                })
                .collect(),
        ),
    );
    Value::Object(map)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOutcome {
    Applied,
    NotApplied,
}

pub fn mutation_event(
    path: &str,
    kind: ChangeKind,
    outcome: MutationOutcome,
    reason: Option<&str>,
) -> Result<Value, OutputError> {
    check_mutation(path, outcome, reason)?;
    Ok(mutation_value(path, kind, outcome, reason))
}

fn check_mutation(
    path: &str,
    outcome: MutationOutcome,
    reason: Option<&str>,
) -> Result<(), OutputError> {
    check_path(path)?;
    match (outcome, reason) {
        (MutationOutcome::NotApplied, None) | (MutationOutcome::NotApplied, Some("")) => {
            return Err(OutputError::MissingReason {
                path: path.to_owned(),
            });
        }
        (MutationOutcome::Applied, Some(_)) => {
            return Err(OutputError::UnexpectedReason {
                path: path.to_owned(),
            });
        }
        (MutationOutcome::Applied, None) | (MutationOutcome::NotApplied, Some(_)) => {}
    }
    Ok(())
}

/// Builds the event for a mutation whose producer already validated it.
pub fn mutation_value(
    path: &str,
    kind: ChangeKind,
    outcome: MutationOutcome,
    reason: Option<&str>,
) -> Value {
    let mut map = base("mutation");
    map.insert("path".to_owned(), Value::String(path.to_owned()));
    map.insert("kind".to_owned(), Value::String(kind.name().to_owned()));
    map.insert(
        "outcome".to_owned(),
        Value::String(
            match outcome {
                MutationOutcome::Applied => "applied",
                MutationOutcome::NotApplied => "not_applied",
            }
            .to_owned(),
        ),
    );
    if let Some(reason) = reason {
        map.insert("reason".to_owned(), Value::String(reason.to_owned()));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn change_shapes_validated() {
        let change = ChangeEvent {
            path: "src/app.py".to_owned(),
            kind: ChangeKind::Modify,
            source_digest: Some(DIGEST.to_owned()),
            edits: vec![Edit {
                start: 18,
                end: 24,
                replacement: String::new(),
            }],
        };
        let event = change_event(&change).expect("change");
        assert_eq!(event["edits"][0]["start_byte"], Value::from(18));
        let mut no_digest = change.clone();
        no_digest.source_digest = None;
        assert!(change_event(&no_digest).is_err());
        let create = ChangeEvent {
            path: "new/package/BUILD.bazel".to_owned(),
            kind: ChangeKind::Create,
            source_digest: None,
            edits: vec![Edit {
                start: 0,
                end: 0,
                replacement: "py_library(\n)\n".to_owned(),
            }],
        };
        change_event(&create).expect("create");
        let mut create_digest = create.clone();
        create_digest.source_digest = Some(DIGEST.to_owned());
        assert!(change_event(&create_digest).is_err());
    }

    #[test]
    fn mutation_reasons_required() {
        mutation_event("a.py", ChangeKind::Modify, MutationOutcome::Applied, None)
            .expect("applied");
        assert_eq!(
            mutation_event(
                "a.py",
                ChangeKind::Modify,
                MutationOutcome::NotApplied,
                None
            )
            .expect_err("reason required"),
            OutputError::MissingReason {
                path: "a.py".to_owned()
            }
        );
        assert!(mutation_event(
            "a.py",
            ChangeKind::Modify,
            MutationOutcome::Applied,
            Some("stale_source"),
        )
        .is_err());
    }

    #[test]
    fn change_events_carry_every_field() {
        let modify = ChangeEvent {
            path: "src/app.py".to_owned(),
            kind: ChangeKind::Modify,
            source_digest: Some(DIGEST.to_owned()),
            edits: vec![Edit {
                start: 0,
                end: 3,
                replacement: "b".to_owned(),
            }],
        };
        let event = change_value(&modify);
        assert_eq!(event, change_event(&modify).expect("modify"));
        assert_eq!(event["path"], Value::from("src/app.py"));
        assert_eq!(event["kind"], Value::from("modify"));
        assert_eq!(event["source_digest"], Value::from(DIGEST));
        assert_eq!(event["edits"][0]["replacement"], Value::from("b"));

        let create = ChangeEvent {
            path: "new/package/BUILD.bazel".to_owned(),
            kind: ChangeKind::Create,
            source_digest: None,
            edits: vec![Edit {
                start: 0,
                end: 0,
                replacement: "py_library(\n)\n".to_owned(),
            }],
        };
        let event = change_value(&create);
        assert_eq!(event, change_event(&create).expect("create"));
        assert_eq!(event["kind"], Value::from("create"));
        assert_eq!(
            event["edits"][0]["replacement"],
            Value::from("py_library(\n)\n")
        );
        assert!(event.get("source_digest").is_none());
    }

    #[test]
    fn mutation_events_carry_every_field() {
        let applied = mutation_value("a.py", ChangeKind::Create, MutationOutcome::Applied, None);
        assert_eq!(
            applied,
            mutation_event("a.py", ChangeKind::Create, MutationOutcome::Applied, None)
                .expect("applied")
        );
        assert_eq!(applied["path"], Value::from("a.py"));
        assert_eq!(applied["kind"], Value::from("create"));
        assert_eq!(applied["outcome"], Value::from("applied"));
        assert!(applied.get("reason").is_none());
        let reasoned = mutation_value(
            "a.py",
            ChangeKind::Modify,
            MutationOutcome::NotApplied,
            Some("io_error"),
        );
        assert_eq!(reasoned["reason"], Value::from("io_error"));
    }

    #[test]
    fn create_shape_violations_fail() {
        let two_edits = ChangeEvent {
            path: "new.txt".to_owned(),
            kind: ChangeKind::Create,
            source_digest: None,
            edits: vec![
                Edit {
                    start: 0,
                    end: 0,
                    replacement: "a".to_owned(),
                },
                Edit {
                    start: 0,
                    end: 0,
                    replacement: "b".to_owned(),
                },
            ],
        };
        assert!(change_event(&two_edits).is_err());
        let shifted = ChangeEvent {
            path: "new.txt".to_owned(),
            kind: ChangeKind::Create,
            source_digest: None,
            edits: vec![Edit {
                start: 3,
                end: 3,
                replacement: "a".to_owned(),
            }],
        };
        assert!(change_event(&shifted).is_err());
    }

    #[test]
    fn mutation_empty_reason_and_reason_field() {
        assert_eq!(
            mutation_event(
                "a.py",
                ChangeKind::Modify,
                MutationOutcome::NotApplied,
                Some("")
            )
            .expect_err("empty reason"),
            OutputError::MissingReason {
                path: "a.py".to_owned()
            }
        );
        let event = mutation_event(
            "a.py",
            ChangeKind::Modify,
            MutationOutcome::NotApplied,
            Some("disagreeing_agents"),
        )
        .expect("reasoned");
        assert_eq!(event["outcome"], Value::String("not_applied".to_owned()));
        assert_eq!(
            event["reason"],
            Value::String("disagreeing_agents".to_owned())
        );
    }
}
