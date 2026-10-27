pub const WORKFLOW_DIR: &str = ".github/workflows";

pub const COMMIT_JSON_ENV: &str = "DX_BUMP_GITHUB_COMMIT_JSON";

pub fn workflow_file(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}

use saphyr_parser::{Event, Parser, ScalarStyle};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsesKind {
    Job,
    Step,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsesPin {
    pub kind: UsesKind,
    pub value: String,
    pub style: ScalarStyle,
    pub span: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowPins {
    pub pins: Vec<UsesPin>,
    pub loose: Vec<String>,
    pub indirect: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WorkflowYamlError {
    #[error("workflow is not valid YAML: {0}")]
    Parse(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Root,
    Jobs,
    Job,
    Steps,
    Step,
    Other,
}

enum Shape {
    Map { key: Option<String> },
    Seq,
}

struct Frame {
    role: Role,
    shape: Shape,
}

fn child_role(parent: Option<&Frame>) -> (Role, bool, Option<String>) {
    let Some(frame) = parent else {
        return (Role::Root, false, None);
    };
    match &frame.shape {
        Shape::Seq => (
            if frame.role == Role::Steps {
                Role::Step
            } else {
                Role::Other
            },
            false,
            None,
        ),
        Shape::Map { key: None } => (Role::Other, true, None),
        Shape::Map { key: Some(key) } => {
            let name = Some(key.as_str());
            let role = match (frame.role, name) {
                (Role::Root, Some("jobs")) => Role::Jobs,
                (Role::Jobs, _) => Role::Job,
                (Role::Job, Some("steps")) => Role::Steps,
                _ => Role::Other,
            };
            (role, false, Some(key.clone()))
        }
    }
}

fn advance(stack: &mut [Frame]) {
    if let Some(Frame {
        shape: Shape::Map { key },
        ..
    }) = stack.last_mut()
    {
        *key = match key {
            None => Some(String::new()),
            Some(_) => None,
        };
    }
}

pub fn parse_workflow_pins(content: &str) -> Result<WorkflowPins, WorkflowYamlError> {
    let mut offsets: Vec<usize> = content.char_indices().map(|(at, _)| at).collect();
    offsets.push(content.len());
    let byte = |index: usize| offsets.get(index).copied().unwrap_or(content.len());
    let mut stack: Vec<Frame> = Vec::new();
    let mut found = WorkflowPins {
        pins: Vec::new(),
        loose: Vec::new(),
        indirect: false,
    };
    for item in Parser::new_from_str(content) {
        let (event, span) = item.map_err(|error| WorkflowYamlError::Parse(error.to_string()))?;
        match event {
            Event::DocumentStart(_) => stack.clear(),
            Event::Alias(_) => {
                found.indirect = true;
                advance(&mut stack);
            }
            Event::Scalar(value, style, _, _) => {
                let (_, is_key, key) = child_role(stack.last());
                if is_key {
                    if value == "<<" {
                        found.indirect = true;
                    }
                    if let Some(Frame {
                        shape: Shape::Map { key },
                        ..
                    }) = stack.last_mut()
                    {
                        *key = Some(value.to_string());
                    }
                    continue;
                }
                if key.as_deref() == Some("uses") {
                    let parent = stack.last().map(|frame| frame.role);
                    found.loose.push(value.to_string());
                    let kind = match parent {
                        Some(Role::Job) => Some(UsesKind::Job),
                        Some(Role::Step) => Some(UsesKind::Step),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        found.pins.push(UsesPin {
                            kind,
                            value: value.to_string(),
                            style,
                            span: scalar_span(
                                content,
                                byte(span.start.index()),
                                byte(span.end.index()),
                                style,
                            ),
                        });
                    }
                }
                advance(&mut stack);
            }
            Event::MappingStart(..) | Event::SequenceStart(..) => {
                let (role, is_key, _) = child_role(stack.last());
                let is_map = matches!(event, Event::MappingStart(..));
                let role = match (role, is_map) {
                    (Role::Root | Role::Job | Role::Step | Role::Jobs, true) => role,
                    (Role::Steps, false) => role,
                    _ => Role::Other,
                };
                if is_key {
                    advance(&mut stack);
                }
                stack.push(Frame {
                    role,
                    shape: if is_map {
                        Shape::Map { key: None }
                    } else {
                        Shape::Seq
                    },
                });
            }
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
                advance(&mut stack);
            }
            _ => {}
        }
    }
    Ok(found)
}

// Framing only: trims quote delimiters off the span; the YAML event parser reads the pins.
fn scalar_span(content: &str, start: usize, end: usize, style: ScalarStyle) -> Range<usize> {
    let quote = match style {
        ScalarStyle::SingleQuoted => '\'',
        ScalarStyle::DoubleQuoted => '"',
        _ => return start..end,
    };
    let Some(rest) = content.get(start..) else {
        return start..end;
    };
    let mut chars = rest.char_indices().skip(1).peekable();
    while let Some((at, ch)) = chars.next() {
        if quote == '"' && ch == '\\' {
            chars.next();
        } else if ch == quote {
            if quote == '\'' && matches!(chars.peek(), Some((_, '\''))) {
                chars.next();
            } else {
                return start..start + at + 1;
            }
        }
    }
    start..end
}

pub fn split_pin(value: &str) -> Option<(&str, &str)> {
    let (action, reference) = value.split_once('@')?;
    if action.is_empty() || action.chars().any(char::is_whitespace) {
        return None;
    }
    Some((action, reference))
}

pub fn pin_targets_package(kind: UsesKind, value: &str, package: &str) -> bool {
    let Some((action, _)) = split_pin(value) else {
        return false;
    };
    let mut parts = action.splitn(3, '/');
    let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else {
        return false;
    };
    if !format!("{owner}/{repo}").eq_ignore_ascii_case(package) {
        return false;
    }
    match (kind, parts.next()) {
        (_, None) => kind == UsesKind::Step,
        (UsesKind::Step, Some(path)) => {
            !path.is_empty() && !path.split('/').any(|part| part.is_empty() || part == "..")
        }
        (UsesKind::Job, Some(path)) => {
            path.starts_with(".github/workflows/")
                && (path.ends_with(".yml") || path.ends_with(".yaml"))
        }
    }
}

pub fn replace_pin_reference(content: &str, pin: &UsesPin, sha: &str) -> Option<String> {
    let (_, reference) = split_pin(&pin.value)?;
    if reference.is_empty() || reference.chars().any(char::is_whitespace) {
        return None;
    }
    let raw = content.get(pin.span.clone())?;
    let (open, close) = match pin.style {
        ScalarStyle::Plain => (0, 0),
        ScalarStyle::SingleQuoted => (1, 1),
        ScalarStyle::DoubleQuoted => (1, 1),
        _ => return None,
    };
    let inner_end = raw.len().checked_sub(close)?;
    let inner = raw.get(open..inner_end)?;
    let head = inner.strip_suffix(reference)?;
    if !head.ends_with('@') {
        return None;
    }
    let tail_at = pin.span.start + open + head.len();
    let mut out = String::with_capacity(content.len() + sha.len());
    out.push_str(&content[..tail_at]);
    out.push_str(sha);
    out.push_str(&content[pin.span.start + inner_end..]);
    Some(out)
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GhaResolveError {
    #[error("empty GitHub commit response (nothing widened; never invent a SHA)")]
    Empty,
    #[error(
        "invalid GitHub commit response: not a JSON object (nothing widened; never invent a SHA)"
    )]
    InvalidJson,
    #[error(
        "GitHub reported {message:?} for the requested ref (nothing widened; never invent a SHA)"
    )]
    Upstream { message: String },
    #[error("GitHub commit response carries no commit SHA (nothing widened; never invent a SHA)")]
    MissingSha,
    #[error("GitHub commit response carries an invalid SHA {sha:?} (nothing widened; never invent a SHA)")]
    InvalidSha { sha: String },
}

pub fn resolve_sha_from_commit_response(text: &str) -> Result<String, GhaResolveError> {
    if text.trim().is_empty() {
        return Err(GhaResolveError::Empty);
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| GhaResolveError::InvalidJson)?;
    let object = value.as_object().ok_or(GhaResolveError::InvalidJson)?;
    match object.get("sha") {
        Some(serde_json::Value::String(sha)) => {
            if dx_digest::is_commit_sha(sha) {
                Ok(sha.clone())
            } else {
                Err(GhaResolveError::InvalidSha { sha: sha.clone() })
            }
        }
        _ => match object.get("message") {
            Some(serde_json::Value::String(message)) => Err(GhaResolveError::Upstream {
                message: message.clone(),
            }),
            _ => Err(GhaResolveError::MissingSha),
        },
    }
}
