#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use quality_result::proto::{Convergence, Diagnostic, QualityResult, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Threshold {
    Info,
    Warning,
    Error,
}

impl Threshold {
    pub fn rank(self) -> i32 {
        match self {
            Threshold::Info => Severity::Info as i32,
            Threshold::Warning => Severity::Warning as i32,
            Threshold::Error => Severity::Error as i32,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Threshold::Info => "info",
            Threshold::Warning => "warning",
            Threshold::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvaluatorError {
    #[error("unknown fail_on {value:?}, want info|warning|error")]
    UnknownThreshold { value: String },
}

pub fn parse_threshold(text: &str) -> Result<Threshold, EvaluatorError> {
    match text {
        "info" => Ok(Threshold::Info),
        "warning" => Ok(Threshold::Warning),
        "error" => Ok(Threshold::Error),
        _ => Err(EvaluatorError::UnknownThreshold {
            value: text.to_owned(),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub passed: bool,
    pub reasons: Vec<String>,
}

fn describe(set: &str, diagnostic: &Diagnostic) -> String {
    format!(
        "{set} diagnostic {:?}:{}:{}:{:?}:{:?}:{}",
        diagnostic.severity,
        diagnostic.tool_id,
        diagnostic.path,
        diagnostic.start_byte,
        diagnostic.end_byte,
        diagnostic.message
    )
}

pub fn evaluate(result: &QualityResult, threshold: Threshold) -> Evaluation {
    let mut reasons = Vec::new();
    if result.convergence != Convergence::Stable as i32 {
        reasons.push(format!(
            "convergence is {}, want STABLE",
            result.convergence
        ));
    }
    if !result.replacements.is_empty() {
        reasons.push(format!(
            "proposes {} replacement file(s), independently of severity",
            result.replacements.len()
        ));
    }
    let rank = threshold.rank();
    for (set, diagnostics) in [
        ("initial", &result.initial_diagnostics),
        ("terminal", &result.terminal_diagnostics),
    ] {
        for diagnostic in diagnostics {
            if diagnostic.severity >= rank {
                reasons.push(describe(set, diagnostic));
            }
        }
    }
    let passed = reasons.is_empty();
    Evaluation { passed, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quality_result::proto::FileEdits;

    fn finding(severity: i32) -> Diagnostic {
        Diagnostic {
            severity,
            message: "synthetic finding".to_owned(),
            tool_id: "lint-a".to_owned(),
            path: "quality/testdata/dirty.py".to_owned(),
            start_byte: Some(0),
            end_byte: Some(3),
            ..Default::default()
        }
    }

    fn clean() -> QualityResult {
        QualityResult {
            convergence: Convergence::Stable as i32,
            ..Default::default()
        }
    }

    #[test]
    fn rank_orders_severity_scale() {
        assert!(Threshold::Info.rank() < Threshold::Warning.rank());
        assert!(Threshold::Warning.rank() < Threshold::Error.rank());
    }

    #[test]
    fn names_match_fail_on_values() {
        assert_eq!(Threshold::Info.name(), "info");
        assert_eq!(Threshold::Warning.name(), "warning");
        assert_eq!(Threshold::Error.name(), "error");
    }

    #[test]
    fn parses_all_thresholds() {
        assert_eq!(parse_threshold("info"), Ok(Threshold::Info));
        assert_eq!(parse_threshold("warning"), Ok(Threshold::Warning));
        assert_eq!(parse_threshold("error"), Ok(Threshold::Error));
    }

    #[test]
    fn rejects_unknown_and_never() {
        assert!(parse_threshold("never").is_err());
        assert!(parse_threshold("WARNING").is_err());
        assert!(parse_threshold("").is_err());
    }

    #[test]
    fn clean_result_passes_every_threshold() {
        let result = clean();
        for threshold in [Threshold::Info, Threshold::Warning, Threshold::Error] {
            let evaluation = evaluate(&result, threshold);
            assert!(evaluation.passed);
            assert!(evaluation.reasons.is_empty());
        }
    }

    #[test]
    fn info_passes_below_threshold_and_fails_at_info() {
        let mut result = clean();
        result
            .initial_diagnostics
            .push(finding(Severity::Info as i32));
        assert!(evaluate(&result, Threshold::Warning).passed);
        assert!(evaluate(&result, Threshold::Error).passed);
        let evaluation = evaluate(&result, Threshold::Info);
        assert!(!evaluation.passed);
        assert_eq!(evaluation.reasons.len(), 1);
    }

    #[test]
    fn warning_fails_warning_and_info_but_passes_error() {
        let mut result = clean();
        result
            .initial_diagnostics
            .push(finding(Severity::Warning as i32));
        assert!(!evaluate(&result, Threshold::Warning).passed);
        assert!(!evaluate(&result, Threshold::Info).passed);
        assert!(evaluate(&result, Threshold::Error).passed);
    }

    #[test]
    fn error_fails_every_threshold() {
        let mut result = clean();
        result
            .initial_diagnostics
            .push(finding(Severity::Error as i32));
        for threshold in [Threshold::Info, Threshold::Warning, Threshold::Error] {
            assert!(!evaluate(&result, threshold).passed);
        }
    }

    #[test]
    fn terminal_only_finding_still_fails() {
        let mut result = clean();
        result
            .terminal_diagnostics
            .push(finding(Severity::Error as i32));
        let evaluation = evaluate(&result, Threshold::Error);
        assert!(!evaluation.passed);
        assert_eq!(evaluation.reasons.len(), 1);
    }

    #[test]
    fn fixable_finding_still_fails_without_apply() {
        let mut fixable = finding(Severity::Warning as i32);
        fixable.fixable = true;
        let mut result = clean();
        result.initial_diagnostics.push(fixable);
        assert!(!evaluate(&result, Threshold::Warning).passed);
    }

    #[test]
    fn replacements_fail_at_every_threshold_without_diagnostics() {
        let mut result = clean();
        result.replacements.push(FileEdits::default());
        for threshold in [Threshold::Info, Threshold::Warning, Threshold::Error] {
            let evaluation = evaluate(&result, threshold);
            assert!(!evaluation.passed);
            assert_eq!(evaluation.reasons.len(), 1);
        }
    }

    #[test]
    fn formatter_replacement_without_diagnostics_fails_like_check_mode() {
        use quality_result::proto::Edit;
        let original = b"x  \n";
        let terminal = b"x\n";
        let mut result = clean();
        assert!(result.initial_diagnostics.is_empty());
        assert!(result.terminal_diagnostics.is_empty());
        result.replacements.push(FileEdits {
            path: "src/main.py".to_owned(),
            original_digest: vec![0u8; 32],
            edits: vec![Edit {
                start_byte: 0,
                end_byte: original.len() as u64,
                replacement: terminal.to_vec(),
            }],
        });
        for threshold in [Threshold::Info, Threshold::Warning, Threshold::Error] {
            let evaluation = evaluate(&result, threshold);
            assert!(!evaluation.passed);
            assert_eq!(evaluation.reasons.len(), 1);
            assert!(evaluation.reasons[0].contains("replacement"));
        }
    }

    #[test]
    fn non_stable_convergence_fails_with_no_replacements() {
        for convergence in [Convergence::Oscillation, Convergence::IterationLimit] {
            let result = QualityResult {
                convergence: convergence as i32,
                ..Default::default()
            };
            let evaluation = evaluate(&result, Threshold::Error);
            assert!(!evaluation.passed);
            assert_eq!(evaluation.reasons.len(), 1);
        }
    }
}
