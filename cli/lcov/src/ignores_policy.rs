use crate::LcovError;

pub const MAX_REASON_LEN: usize = 120;

fn key_value(line: &str, key: &str) -> Option<String> {
    let offset = line.find(key)?;
    let value = line[offset + key.len()..].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn reason_value(line: &str) -> Option<String> {
    key_value(line, "reason:")
}

fn issue_value(line: &str) -> Option<String> {
    let value = key_value(line, "issue:")?;
    if value.chars().any(|c| c.is_ascii_digit()) {
        Some(value)
    } else {
        None
    }
}

fn has_policy(line: &str) -> bool {
    line.contains("policy:")
}

pub(crate) fn nearby_reason(
    path: &str,
    directive: &str,
    lineno: usize,
    lines: &[&str],
) -> Result<String, LcovError> {
    let current = lines[lineno - 1];
    let previous = if lineno >= 2 {
        Some(lines[lineno - 2])
    } else {
        None
    };
    let reason = if let Some(reason) = reason_value(current) {
        reason
    } else if let Some(prev) = previous {
        if let Some(reason) = reason_value(prev) {
            reason
        } else if has_policy(current) || has_policy(prev) {
            return Err(LcovError::BarePolicyWithoutReason {
                path: path.to_string(),
                lineno,
                directive: directive.to_string(),
            });
        } else {
            return Err(LcovError::MissingReason {
                path: path.to_string(),
                lineno,
                directive: directive.to_string(),
            });
        }
    } else if has_policy(current) {
        return Err(LcovError::BarePolicyWithoutReason {
            path: path.to_string(),
            lineno,
            directive: directive.to_string(),
        });
    } else {
        return Err(LcovError::MissingReason {
            path: path.to_string(),
            lineno,
            directive: directive.to_string(),
        });
    };
    let len = reason.chars().count();
    if len > MAX_REASON_LEN {
        return Err(LcovError::ReasonTooLong {
            path: path.to_string(),
            lineno,
            directive: directive.to_string(),
            len,
            max: MAX_REASON_LEN,
        });
    }
    let has_issue =
        issue_value(current).is_some() || previous.is_some_and(|prev| issue_value(prev).is_some());
    if !has_issue {
        return Err(LcovError::MissingIssue {
            path: path.to_string(),
            lineno,
            directive: directive.to_string(),
        });
    }
    Ok(reason)
}
