use dx_fingerprint::baseline::{line_context_digest, normalize_message, Finding};
use dx_output::DiagnosticEvent;

/// Builds the baseline identity of a finding from its file bytes.
pub(crate) fn finding_identity(diagnostic: &DiagnosticEvent, bytes: Option<&[u8]>) -> Finding {
    let target = match (&diagnostic.path, diagnostic.origins.first()) {
        (None, Some(origin)) => origin.target.clone(),
        _ => String::new(),
    };
    let unattributed = diagnostic.path.is_none() && target.is_empty();
    let context = match (&diagnostic.path, diagnostic.range) {
        _ if unattributed => None,
        (None, _) => Some(dx_digest::blake3(b"")),
        (Some(_), None) => bytes.map(dx_digest::blake3),
        (Some(_), Some((start, _))) => bytes.and_then(|bytes| line_context_digest(bytes, start)),
    };
    Finding {
        tool: diagnostic.tool.clone(),
        rule: diagnostic.rule.clone().unwrap_or_default(),
        path: diagnostic.path.clone().unwrap_or_default(),
        target,
        message: normalize_message(&diagnostic.message),
        context,
    }
}
