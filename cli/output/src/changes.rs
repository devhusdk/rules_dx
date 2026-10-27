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

/// What happened to one proposed file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeEffect {
    Proposed,
    Applied,
    Refused,
}

impl ChangeEffect {
    fn name(self) -> &'static str {
        match self {
            ChangeEffect::Proposed => "proposed",
            ChangeEffect::Applied => "applied",
            ChangeEffect::Refused => "refused",
        }
    }
}

fn check_effect(path: &str, effect: ChangeEffect, reason: Option<&str>) -> Result<(), OutputError> {
    match (effect, reason) {
        (ChangeEffect::Refused, None) | (ChangeEffect::Refused, Some("")) => {
            Err(OutputError::MissingReason {
                path: path.to_owned(),
            })
        }
        (ChangeEffect::Proposed | ChangeEffect::Applied, Some(_)) => {
            Err(OutputError::UnexpectedReason {
                path: path.to_owned(),
            })
        }
        _ => Ok(()),
    }
}

/// Validates one change record before building it.
pub fn change_event(
    change: &ChangeEvent,
    effect: ChangeEffect,
    reason: Option<&str>,
    include_edits: bool,
) -> Result<Value, OutputError> {
    check_change(change)?;
    check_effect(&change.path, effect, reason)?;
    Ok(change_value(change, effect, reason, include_edits))
}

/// Builds the record for a change whose producer already validated it.
pub fn change_value(
    change: &ChangeEvent,
    effect: ChangeEffect,
    reason: Option<&str>,
    include_edits: bool,
) -> Value {
    let mut map = record(
        &change.path,
        change.kind,
        change.edits.len() as u64,
        effect,
        reason,
    );
    if include_edits {
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
    }
    Value::Object(map)
}

/// Builds a refused change that has no single edit set.
pub fn refusal_value(path: &str, kind: ChangeKind, reason: &str) -> Value {
    Value::Object(record(path, kind, 0, ChangeEffect::Refused, Some(reason)))
}

fn record(
    path: &str,
    kind: ChangeKind,
    edit_count: u64,
    effect: ChangeEffect,
    reason: Option<&str>,
) -> serde_json::Map<String, Value> {
    let mut map = base("change");
    map.insert("path".to_owned(), Value::String(path.to_owned()));
    map.insert("kind".to_owned(), Value::String(kind.name().to_owned()));
    map.insert("edit_count".to_owned(), Value::from(edit_count));
    map.insert("effect".to_owned(), Value::String(effect.name().to_owned()));
    if let Some(reason) = reason {
        map.insert("reason".to_owned(), Value::String(reason.to_owned()));
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn modify() -> ChangeEvent {
        ChangeEvent {
            path: "src/app.py".to_owned(),
            kind: ChangeKind::Modify,
            source_digest: Some(DIGEST.to_owned()),
            edits: vec![
                Edit {
                    start: 0,
                    end: 3,
                    replacement: "b".to_owned(),
                },
                Edit {
                    start: 18,
                    end: 24,
                    replacement: String::new(),
                },
            ],
        }
    }

    fn create() -> ChangeEvent {
        ChangeEvent {
            path: "new/package/BUILD.bazel".to_owned(),
            kind: ChangeKind::Create,
            source_digest: None,
            edits: vec![Edit {
                start: 0,
                end: 0,
                replacement: "py_library(\n)\n".to_owned(),
            }],
        }
    }

    #[test]
    fn change_shapes_validated() {
        let change = modify();
        change_event(&change, ChangeEffect::Proposed, None, false).expect("change");
        let mut no_digest = change.clone();
        no_digest.source_digest = None;
        assert!(change_event(&no_digest, ChangeEffect::Proposed, None, false).is_err());
        change_event(&create(), ChangeEffect::Applied, None, false).expect("create");
        let mut create_digest = create();
        create_digest.source_digest = Some(DIGEST.to_owned());
        assert!(change_event(&create_digest, ChangeEffect::Proposed, None, false).is_err());
    }

    #[test]
    fn compact_records_hold_no_replacement_bodies() {
        let event = change_event(&modify(), ChangeEffect::Proposed, None, false).expect("modify");
        let keys: Vec<&str> = event
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["edit_count", "effect", "event", "kind", "path", "schema"]
        );
        assert_eq!(event["path"], Value::from("src/app.py"));
        assert_eq!(event["kind"], Value::from("modify"));
        assert_eq!(event["edit_count"], Value::from(2));
        assert_eq!(event["effect"], Value::from("proposed"));
        let created = change_value(&create(), ChangeEffect::Applied, None, false);
        assert_eq!(created["kind"], Value::from("create"));
        assert_eq!(created["effect"], Value::from("applied"));
        assert!(created.get("edits").is_none());
    }

    #[test]
    fn include_edits_adds_the_full_payload() {
        let event = change_event(&modify(), ChangeEffect::Applied, None, true).expect("modify");
        assert_eq!(event["source_digest"], Value::from(DIGEST));
        assert_eq!(event["edits"][0]["start_byte"], Value::from(0));
        assert_eq!(event["edits"][0]["replacement"], Value::from("b"));
        assert_eq!(event["edits"][1]["end_byte"], Value::from(24));
        assert_eq!(event["edit_count"], Value::from(2));
        let created = change_value(&create(), ChangeEffect::Proposed, None, true);
        assert!(created.get("source_digest").is_none());
        assert_eq!(
            created["edits"][0]["replacement"],
            Value::from("py_library(\n)\n")
        );
    }

    #[test]
    fn only_refusals_carry_a_reason() {
        let event = change_event(
            &modify(),
            ChangeEffect::Refused,
            Some("stale_source"),
            false,
        )
        .expect("refused");
        assert_eq!(event["effect"], Value::from("refused"));
        assert_eq!(event["reason"], Value::from("stale_source"));
        for reason in [None, Some("")] {
            assert_eq!(
                change_event(&modify(), ChangeEffect::Refused, reason, false)
                    .expect_err("reason required"),
                OutputError::MissingReason {
                    path: "src/app.py".to_owned()
                }
            );
        }
        for effect in [ChangeEffect::Proposed, ChangeEffect::Applied] {
            assert_eq!(
                change_event(&modify(), effect, Some("stale_source"), false)
                    .expect_err("no reason"),
                OutputError::UnexpectedReason {
                    path: "src/app.py".to_owned()
                }
            );
        }
    }

    #[test]
    fn refusals_without_edits() {
        let event = refusal_value("a.py", ChangeKind::Modify, "conflicting_candidates");
        assert_eq!(event["edit_count"], Value::from(0));
        assert_eq!(event["effect"], Value::from("refused"));
        assert_eq!(event["reason"], Value::from("conflicting_candidates"));
    }

    #[test]
    fn create_shape_violations_fail() {
        let mut two_edits = create();
        two_edits.edits.push(Edit {
            start: 0,
            end: 0,
            replacement: "b".to_owned(),
        });
        assert!(change_event(&two_edits, ChangeEffect::Proposed, None, false).is_err());
        let mut shifted = create();
        shifted.edits[0].start = 3;
        shifted.edits[0].end = 3;
        assert!(change_event(&shifted, ChangeEffect::Proposed, None, false).is_err());
    }
}
