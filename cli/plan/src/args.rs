pub mod command;
pub mod completion;
pub mod grammar;
pub mod help;
pub mod invocation;
pub mod parser;
pub mod profile;
pub mod tokenizer;

/// A failure raised while reading the command line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArgsError {
    /// clap rendered help or the version.
    #[error("{text}")]
    Help { text: String },
    /// clap rendered a usage failure, suggestion included.
    #[error("{text}")]
    Usage { text: String },
    /// No command was named.
    #[error(
        "missing command: want security|license|lint|typecheck|format|generate|build|test|coverage|run|deploy|check|fix|clean|update|bump|add|migrate|codegen|env|init|new|upgrade|status|version|watch|completion|docs|list-tests|rollback|worktree|explain"
    )]
    MissingCommand,
    /// The command does not take this flag.
    #[error("option {option:?} is not supported by dx {command}")]
    UnsupportedOption {
        command: &'static str,
        option: String,
    },
    /// A value flag arrived with no value.
    #[error("missing value for {option:?}")]
    MissingValue { option: String },
    /// A scope token is not a flag or a label.
    #[error("unknown option {option:?}")]
    UnknownOption { option: String },
    /// The resolved --output value is not one of the three modes.
    #[error("unknown --output {value:?}: want text|diff|json")]
    BadOutput { value: String },
    /// The resolved --fail-on value is not a severity.
    #[error("unknown --fail-on {value:?}: want info|warning|error")]
    BadFailOn { value: String },
    /// The resolved --build-slots value is not auto, a count, or off.
    #[error("unknown --build-slots {value:?} from {origin}: want auto|<n>|off (n from 1 to 1024)")]
    BadBuildSlots { origin: &'static str, value: String },
    /// The resolved --color value is not a mode.
    #[error("unknown --color {value:?}: want auto|always|never")]
    BadColor { value: String },
    /// A --bazel-startup-option token is not a qualified startup option.
    #[error(
        "unknown --bazel-startup-option {value:?}: want --output_base=<path>|--output_user_root=<path>"
    )]
    BadStartupOption { value: String },
    /// An environment default is not a value dx reads.
    #[error("{detail}")]
    BadDefault { detail: String },
    /// The completion shell is not one of the four.
    #[error("unknown-shell: {shell}")]
    UnknownShell { shell: String },
    /// Two operation modes were requested at once.
    #[error("options {first} and {second} are mutually exclusive")]
    ConflictingModes {
        first: &'static str,
        second: &'static str,
    },
    /// A JSON-only flag arrived without JSON output.
    #[error("option {option:?} requires --output=json")]
    NeedsJson { option: &'static str },
    /// The here tree was asked for alongside explicit scopes.
    #[error("option \"--here\" cannot be combined with explicit scopes")]
    ConflictingHere,
    /// Affected selection was asked for alongside explicit scopes.
    #[error("option \"--affected\" cannot be combined with explicit scopes")]
    ConflictingAffected,
    /// Affected selection was asked for alongside the here tree.
    #[error("option \"--affected\" cannot be combined with \"--here\"")]
    AffectedWithHere,
    /// No scope was given where one was required.
    #[error(
        "empty scope: pass no scope for repository-wide //... or a //, @, file, or directory scope"
    )]
    EmptyScope,
    /// The scope is package-relative.
    #[error(
        "unsupported scope {scope:?}: package-relative labels resolve against the current directory; spell the workspace label starting with //"
    )]
    RelativeLabel { scope: String },
    /// The scope is neither a label nor a path.
    #[error(
        "unsupported scope {scope:?}: want // or @ labels, or workspace-relative file and directory paths"
    )]
    InvalidScope { scope: String },
}

/// Build the scope error that matches the token.
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

pub use command::{Command, WorkflowVerb};
pub use completion::{
    registers_callback, render_completion, try_complete, COMPLETE_VAR, COMPLETION_SHELLS,
};
pub use grammar::{cli_command, man_page};
pub use invocation::{
    apply_here, here_scope, CommonOptions, GenerateRequest, HintScope, Invocation, OperationMode,
    QualityRequest, ReportRequest, UmbrellaOrigin, UmbrellaRequest, AGENT_QUIET_FLAGS,
};
pub use parser::{
    ci_ignores_env, early_output_flag, early_workspace_flag, freeze_workspace, is_discovery_exempt,
    is_help_request, json_output_intent, load_startup_defaults, parse, parse_with, parse_with_ci,
    select_startup_defaults_with_ci, startup_workspace, DefaultsFailure, StartupDefaults,
};
pub(crate) use profile::filter_profile_configs;
pub use profile::{resolve_profile, Profile, DX_PROFILE_ENV};

pub use dx_adopt::defaults::FileDefaults;
pub use dx_adopt::defaults::DX_WORKSPACE_ENV;

/// Assert that a parse failure is clap-rendered and names every needle.
#[cfg(test)]
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
