use dx_fingerprint::finding::{Finding, line_context_digest, normalize_message};
use dx_output::DiagnosticEvent;

/// Builds the identity of a finding from its file bytes.
pub fn finding_identity(diagnostic: &DiagnosticEvent, bytes: Option<&[u8]>) -> Finding {
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

#[cfg(test)]
mod tests {
    use dx_fingerprint::finding::finding_fingerprint;
    use dx_output::Severity;

    use super::*;

    fn ranged(start: u64) -> DiagnosticEvent {
        DiagnosticEvent {
            rule: Some("t/final-newline".to_owned()),
            path: Some("a.txt".to_owned()),
            range: Some((start, start)),
            ..DiagnosticEvent::new(Severity::Warning, "t", "missing final newline")
        }
    }

    #[test]
    fn end_of_file_findings_get_a_stable_fingerprint() {
        let first = finding_fingerprint(&finding_identity(&ranged(1), Some(b"a")));
        assert!(first.is_some());
        assert_eq!(
            first,
            finding_fingerprint(&finding_identity(&ranged(1), Some(b"a")))
        );
        assert_eq!(
            first,
            finding_fingerprint(&finding_identity(&ranged(0), Some(b"a")))
        );
        let trailing = finding_identity(&ranged(2), Some(b"a\n"));
        assert_eq!(trailing.context, Some(dx_digest::blake3(b"")));
    }

    #[test]
    fn findings_past_end_of_file_get_no_fingerprint() {
        let past = finding_identity(&ranged(2), Some(b"a"));
        assert_eq!(past.context, None);
        assert_eq!(finding_fingerprint(&past), None);
    }
}
