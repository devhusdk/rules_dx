fn suggestion_hint(suggestion: &Option<String>) -> String {
    match suggestion {
        Some(name) => format!(". did you mean {name:?}?"),
        None => String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgsError {
    #[error("{text}")]
    Help { text: String },
    #[error(
        "missing command: want security|license|lint|typecheck|format|generate|build|test|coverage|run|deploy|check|fix|clean|update|bump|migrate|codegen|env|setup|init|new|upgrade|hooks|status|version|watch|owners|deps|why|completion|docs|bazel"
    )]
    MissingCommand,
    #[error(
        "unknown command {command:?}: want security|license|lint|typecheck|format|generate|build|test|coverage|run|deploy|check|fix|clean|update|bump|migrate|codegen|env|setup|init|new|upgrade|hooks|status|version|watch|owners|deps|why|completion|docs|bazel{suggestion_hint}",
        suggestion_hint = suggestion_hint(suggestion)
    )]
    UnknownCommand {
        command: String,
        suggestion: Option<String>,
    },
    #[error("unknown option {option:?}{suggestion_hint}", suggestion_hint = suggestion_hint(suggestion))]
    UnknownOption {
        option: String,
        suggestion: Option<String>,
    },
    #[error("invalid arguments: {message}")]
    InvalidArguments { message: String },
    #[error("option {option:?} is not supported by dx {command}")]
    UnsupportedOption {
        command: &'static str,
        option: String,
    },
    #[error("missing value for {option:?}")]
    MissingValue { option: String },
    #[error("unknown --output {value:?}: want text|diff|json")]
    BadOutput { value: String },
    #[error("unknown --fail-on {value:?}: want info|warning|error")]
    BadFailOn { value: String },
    #[error("unknown --log-level {value:?}: want error|warn|info|debug|trace")]
    BadLogLevel { value: String },
    #[error("unknown --color {value:?}: want auto|always|never")]
    BadColor { value: String },
    #[error("invalid --min-coverage {value:?}: want an integer 0-100")]
    BadMinCoverage { value: String },
    #[error("malformed --report {value:?}: want <format>=<destination>")]
    BadReport { value: String },
    #[error("unknown-shell: {shell}")]
    UnknownShell { shell: String },
    #[error("options --debug and --release are mutually exclusive")]
    ConflictingProfiles,
    #[error("options --verbose and --log-level are mutually exclusive")]
    ConflictingVerboseLogLevel,
    #[error("option \"--here/--cwd\" cannot be combined with explicit scopes")]
    ConflictingHere,
    #[error(
        "empty scope: pass no scope for repository-wide //... or a //, @, file, or directory scope"
    )]
    EmptyScope,
    #[error(
        "unsupported scope {scope:?}: package-relative labels resolve against the current directory; spell the workspace label starting with //"
    )]
    RelativeLabel { scope: String },
    #[error(
        "unsupported scope {scope:?}: want // or @ labels, or workspace-relative file and directory paths"
    )]
    InvalidScope { scope: String },
}

pub fn scope_error(scope: &str) -> ArgsError {
    if scope.is_empty() {
        ArgsError::EmptyScope
    } else if scope.starts_with(':') {
        ArgsError::RelativeLabel {
            scope: scope.to_owned(),
        }
    } else {
        ArgsError::InvalidScope {
            scope: scope.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_reports_variant() {
        assert!(format!("{}", ArgsError::MissingCommand).contains("missing command"));
        assert!(format!(
            "{}",
            ArgsError::InvalidScope {
                scope: "//...".to_owned(),
            }
        )
        .contains("//..."));
        assert!(format!("{}", ArgsError::EmptyScope).contains("empty scope"));
        assert!(format!(
            "{}",
            ArgsError::RelativeLabel {
                scope: ":corpus".to_owned(),
            }
        )
        .contains(":corpus"));
        assert!(format!(
            "{}",
            ArgsError::UnknownCommand {
                command: "bogus".to_owned(),
                suggestion: None,
            }
        )
        .contains("bogus"));
        assert!(format!(
            "{}",
            ArgsError::UnknownOption {
                option: "--bogus".to_owned(),
                suggestion: None,
            }
        )
        .contains("--bogus"));
        let invalid = format!(
            "{}",
            ArgsError::InvalidArguments {
                message: "the argument '--quiet' cannot be used multiple times".to_owned(),
            }
        );
        assert!(
            invalid.starts_with("invalid arguments: the argument"),
            "{invalid}"
        );
        assert!(!invalid.contains("unknown option"), "{invalid}");
        assert!(format!(
            "{}",
            ArgsError::UnsupportedOption {
                command: "build",
                option: "--check".to_owned(),
            }
        )
        .contains("--check"));
        assert!(format!(
            "{}",
            ArgsError::MissingValue {
                option: "--output".to_owned(),
            }
        )
        .contains("--output"));
        assert!(format!(
            "{}",
            ArgsError::BadOutput {
                value: "yaml".to_owned(),
            }
        )
        .contains("yaml"));
        assert!(format!(
            "{}",
            ArgsError::BadFailOn {
                value: "never".to_owned(),
            }
        )
        .contains("never"));
        assert!(format!(
            "{}",
            ArgsError::BadReport {
                value: "sarif".to_owned(),
            }
        )
        .contains("sarif"));
        assert!(format!(
            "{}",
            ArgsError::BadLogLevel {
                value: "verbose".to_owned(),
            }
        )
        .contains("verbose"));
        assert!(format!("{}", ArgsError::ConflictingVerboseLogLevel).contains("--log-level"));
        assert!(format!(
            "{}",
            ArgsError::BadColor {
                value: "bright".to_owned(),
            }
        )
        .contains("bright"));
    }
}
