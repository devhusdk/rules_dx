pub mod build_mode;
pub mod command;
pub mod completion;
pub mod grammar;
pub mod help;
pub mod invocation;
pub mod pages;
pub mod parser;
pub mod test_tags;
pub mod tokenizer;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgsError {
    #[error("{text}")]
    Help { text: String },
    #[error("{text}")]
    Usage { text: String },
    #[error(
        "missing command: want security|license|lint|typecheck|format|generate|build|test|coverage|bench|run|deploy|check|clean|update|sync|init|upgrade|status|watch|completion|docs|worktree|fuzz|config|fetch"
    )]
    MissingCommand,
    #[error("option {option:?} is not supported by dx {command}: {}", unsupported_hint(.command, .option))]
    UnsupportedOption {
        command: &'static str,
        option: String,
    },
    #[error("missing value for {option:?}")]
    MissingValue { option: String },
    #[error("unknown option {option:?}")]
    UnknownOption { option: String },
    #[error("unknown --output {value:?}: want text|json")]
    BadOutput { value: String },
    #[error("unknown --fail-on {value:?}: want info|warning|error")]
    BadFailOn { value: String },
    #[error("unknown --build-slots {value:?} from {origin}: want auto|<n>|off (n from 1 to 1024)")]
    BadBuildSlots { origin: &'static str, value: String },
    #[error(
        "unknown --build-slots-wait {value:?} from {origin}: want seconds, or a number ending in s, m or h"
    )]
    BadBuildSlotsWait { origin: &'static str, value: String },
    #[error("unknown --color {value:?}: want auto|always|never")]
    BadColor { value: String },
    #[error(
        "unknown --bazel-startup-option {value:?}: want --output_base=<path>|--output_user_root=<path>"
    )]
    BadStartupOption { value: String },
    #[error("unknown shell {shell:?}: want {}", COMPLETION_SHELLS.join("|"))]
    UnknownShell { shell: String },
    #[error("option {option:?} requires --output=json")]
    NeedsJson { option: &'static str },
    #[error("option \"--unsafe\" requires --apply or --diff")]
    UnsafeNeedsFixes,
    #[error("option \"--here\" cannot be combined with explicit scopes")]
    ConflictingHere,
    #[error("option \"--affected\" cannot be combined with explicit scopes")]
    ConflictingAffected,
    #[error("option \"--since\" requires \"--affected\"")]
    SinceNeedsAffected,
    #[error("option \"--base\" requires \"--affected\"")]
    BaseNeedsAffected,
    #[error("option \"--affected\" cannot be combined with \"--here\"")]
    AffectedWithHere,
    #[error("option \"--coverage\" requires \"--affected\"")]
    CoverageNeedsAffected,
    #[error("option \"--coverage\" cannot be combined with \"--apply\"")]
    CoverageWithApply,
    #[error("option \"--report={format}\" requires \"--coverage\"")]
    ReportNeedsCoverage { format: String },
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

#[must_use]
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

fn unsupported_hint(command: &str, option: &str) -> String {
    use clap::ValueEnum;
    if let Some(rest) = option.strip_prefix("--") {
        let long = rest.split('=').next().unwrap_or(rest);
        let names: Vec<&str> = Command::value_variants()
            .iter()
            .filter(|c| c.name() != command && grammar::accepts_long(**c, long))
            .map(|c| c.name())
            .collect();
        if !names.is_empty() {
            return format!("use it with dx {}", names.join("|"));
        }
    }
    format!("run dx {command} --help")
}

pub(crate) use build_mode::filter_build_mode_configs;
pub use build_mode::{BuildMode, DX_BUILD_MODE_ENV, resolve_build_mode};
pub use command::{Command, WorkflowVerb};
pub use completion::{
    COMPLETE_VAR, COMPLETION_SHELLS, registers_callback, render_completion, try_complete,
};
pub use grammar::{Agent, Ci, cli_command, man_page};
pub use invocation::{
    AGENT_QUIET_FLAGS, AffectedHandoff, CommonOptions, GenerateRequest, HintScope, Invocation,
    OperationMode, QualityRequest, ReportRequest, UmbrellaOrigin, UmbrellaRequest, apply_here,
    bare_scope_note, here_scope,
};
pub use pages::{
    HelpEnv, HelpFlag, HelpPositional, HelpSurface, ROOT_PAGE, exit_codes_in, help_environment,
    help_surface,
};
pub use parser::{
    BREAKING_AGAINST_OPTION, ci_ignores_env, early_output_flag, is_discovery_exempt,
    json_output_intent, parse, parse_with, parse_with_ci,
};
pub use test_tags::{COVERAGE_EXCLUDED_TAGS, TagFilter, exclude_tags, takes_coverage};

#[cfg(test)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "callers hand over the error from a temporary"
)]
pub(crate) fn assert_usage<D: std::fmt::Debug>(words: D, error: ArgsError, needles: &[&str]) {
    let ArgsError::Usage { text } = &error else {
        panic!("words: {words:?}: want Usage, got {error:?}");
    };
    for needle in needles {
        assert!(
            text.contains(needle),
            "words: {words:?}: {needle:?} missing from {text}"
        );
    }
}
