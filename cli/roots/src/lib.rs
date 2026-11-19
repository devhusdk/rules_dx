#![deny(clippy::indexing_slicing, clippy::string_slice)]

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
            if single.contains("...")
                || single.contains('*')
                || single.contains('?')
                || single.ends_with(":all")
                || single.ends_with(":all-targets")
            {
                Err(ExactScopeError::TargetPattern {
                    value: single.clone(),
                })
            } else if is_exact_label(single) {
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

// Bazel never reads a label with whitespace, control or invisible characters as the label the user typed.
fn is_exact_label(value: &str) -> bool {
    let body = value
        .strip_prefix("//")
        .or_else(|| value.strip_prefix("@@"))
        .or_else(|| value.strip_prefix('@'));
    match body {
        Some(body) => {
            !body.is_empty()
                && !body.starts_with('@')
                && !(value.starts_with("//") && body.starts_with('/'))
                && !value.chars().any(is_unreadable)
        }
        None => false,
    }
}

fn is_unreadable(character: char) -> bool {
    character.is_whitespace()
        || character.is_control()
        || matches!(
            character,
            '\u{ad}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{feff}'
        )
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
        assert_eq!(
            resolve_exact_target(&labels(&["@@repo//a:b"])),
            Ok(Some(label("@@repo//a:b")))
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

    #[test]
    fn exact_target_rejects_empty_label_body() {
        for value in ["//", "@", "@@"] {
            assert_eq!(
                resolve_exact_target(&labels(&[value])),
                Err(ExactScopeError::NotTargetLabel {
                    value: label(value)
                })
            );
        }
    }

    #[test]
    fn exact_target_rejects_whitespace_and_control_characters() {
        for value in [
            "//a b",
            "//a:b\n",
            "//a:b\r",
            "//a:b\t",
            " //a:b",
            "@repo//a:b ",
            "//a:b\u{0}",
            "//a:b\u{7f}",
            "//a:b\u{a0}",
        ] {
            assert_eq!(
                resolve_exact_target(&labels(&[value])),
                Err(ExactScopeError::NotTargetLabel {
                    value: label(value)
                })
            );
        }
    }

    #[test]
    fn exact_target_rejects_all_target_patterns() {
        for pattern in ["//a:all", "//a:all-targets", "@repo//a:all", "//:all"] {
            assert_eq!(
                resolve_exact_target(&labels(&[pattern])),
                Err(ExactScopeError::TargetPattern {
                    value: label(pattern)
                })
            );
        }
        assert_eq!(
            resolve_exact_target(&labels(&["//a:allowed"])),
            Ok(Some(label("//a:allowed")))
        );
    }

    #[test]
    fn exact_target_rejects_invisible_format_characters() {
        for value in [
            "//a:b\u{200b}",
            "//a\u{202e}:b",
            "\u{feff}//a:b",
            "//a:\u{ad}b",
            "//a:b\u{2060}",
        ] {
            assert_eq!(
                resolve_exact_target(&labels(&[value])),
                Err(ExactScopeError::NotTargetLabel {
                    value: label(value)
                })
            );
        }
    }

    #[test]
    fn exact_target_rejects_extra_prefix_characters() {
        for value in ["///a:b", "@@@repo//a:b"] {
            assert_eq!(
                resolve_exact_target(&labels(&[value])),
                Err(ExactScopeError::NotTargetLabel {
                    value: label(value)
                })
            );
        }
        assert_eq!(
            resolve_exact_target(&labels(&["@//a:b"])),
            Ok(Some(label("@//a:b")))
        );
    }
}
