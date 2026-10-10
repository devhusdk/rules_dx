pub const WORKFLOW_DIR: &str = ".github/workflows";

pub const COMMIT_JSON_ENV: &str = "DX_BUMP_GITHUB_COMMIT_JSON";

pub fn workflow_file(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsesPin {
    pub action: String,
    pub reference: String,
    pub tag: String,
}

pub fn uses_line_rest(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("- uses:")
        .or_else(|| trimmed.strip_prefix("uses:"))
        .map(str::trim)
}

pub fn parse_uses_pin(rest: &str) -> Option<UsesPin> {
    let rest = rest.trim();
    let (expr, comment) = match rest.strip_prefix('"') {
        Some(after) => after.split_once('"')?,
        None => match rest.strip_prefix('\'') {
            Some(after) => after.split_once('\'')?,
            None => {
                let (expr, comment) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                (expr, comment)
            }
        },
    };
    let (action, reference) = expr.split_once('@')?;
    if action.is_empty() || action.chars().any(char::is_whitespace) {
        return None;
    }
    Some(UsesPin {
        action: action.to_owned(),
        reference: reference.to_owned(),
        tag: comment.trim().trim_start_matches('#').trim().to_owned(),
    })
}

pub fn uses_pin_targets_package(line: &str, package: &str) -> bool {
    match uses_line_rest(line).and_then(parse_uses_pin) {
        Some(pin) => pin.action == package,
        None => false,
    }
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
