#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactScopeError {
    MultipleTargets { count: usize },
    TargetPattern { value: String },
    NotTargetLabel { value: String },
}

impl std::fmt::Display for ExactScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExactScopeError::MultipleTargets { count } => {
                write!(f, "expected at most one target, found {count}")
            }
            ExactScopeError::TargetPattern { value } => {
                write!(f, "invalid target {value:?}: patterns never select scope")
            }
            ExactScopeError::NotTargetLabel { value } => {
                write!(f, "invalid target {value:?}: want an exact // or @ label")
            }
        }
    }
}

impl std::error::Error for ExactScopeError {}

pub fn resolve_exact_target(targets: &[String]) -> Result<Option<String>, ExactScopeError> {
    match targets {
        [] => Ok(None),
        [single] => {
            if single.contains("...") || single.contains('*') || single.contains('?') {
                Err(ExactScopeError::TargetPattern {
                    value: single.clone(),
                })
            } else if single.starts_with("//") || single.starts_with('@') {
                Ok(Some(single.clone()))
            } else {
                Err(ExactScopeError::NotTargetLabel {
                    value: single.clone(),
                })
            }
        }
        _ => Err(ExactScopeError::MultipleTargets {
            count: targets.len(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(value: &str) -> String {
        value.to_owned()
    }

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| label(value)).collect()
    }

    #[test]
    fn exact_target_accepts_none_or_one_label() {
        assert_eq!(resolve_exact_target(&[]), Ok(None));
        assert_eq!(
            resolve_exact_target(&labels(&["//a:b"])),
            Ok(Some(label("//a:b")))
        );
        assert_eq!(
            resolve_exact_target(&labels(&["@repo//a:b"])),
            Ok(Some(label("@repo//a:b")))
        );
    }

    #[test]
    fn exact_target_rejects_patterns_and_non_labels_and_lists() {
        for pattern in ["//a/...", "//a:*", "//a:b?"] {
            let error = resolve_exact_target(&labels(&[pattern])).unwrap_err();
            assert_eq!(
                error,
                ExactScopeError::TargetPattern {
                    value: label(pattern)
                }
            );
            assert!(error.to_string().contains("patterns never select scope"));
        }
        let error = resolve_exact_target(&labels(&["a:b"])).unwrap_err();
        assert_eq!(
            error,
            ExactScopeError::NotTargetLabel {
                value: label("a:b")
            }
        );
        assert!(error.to_string().contains("want an exact // or @ label"));
        let error = resolve_exact_target(&labels(&["//a", "//b"])).unwrap_err();
        assert_eq!(error, ExactScopeError::MultipleTargets { count: 2 });
        assert_eq!(error.to_string(), "expected at most one target, found 2");
    }
}
