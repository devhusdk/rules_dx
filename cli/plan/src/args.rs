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
        "missing command: want security|license|lint|typecheck|format|generate|build|test|coverage|bench|run|deploy|check|clean|update|env|init|upgrade|status|watch|completion|docs|worktree|explain|fuzz|release"
    )]
    MissingCommand,
    /// The command does not take this flag.
    #[error("option {option:?} is not supported by dx {command}: {}", unsupported_hint(.command, .option))]
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
    /// The resolved --output value is not one of the two modes.
    #[error("unknown --output {value:?}: want text|json")]
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
    #[error("unknown shell {shell:?}: want {}", COMPLETION_SHELLS.join("|"))]
    UnknownShell { shell: String },
    /// A JSON-only flag arrived without JSON output.
    #[error("option {option:?} requires --output=json")]
    NeedsJson { option: &'static str },
    /// Unsafe fixes were asked for without applying or showing fixes.
    #[error("option \"--unsafe\" requires --apply or --diff")]
    UnsafeNeedsFixes,
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

pub use command::{Command, WorkflowVerb};
pub use completion::{
    COMPLETE_VAR, COMPLETION_SHELLS, registers_callback, render_completion, try_complete,
};
pub use grammar::{Agent, ProfileArg, cli_command, man_page};
pub use invocation::{
    AGENT_QUIET_FLAGS, AffectedHandoff, CommonOptions, GenerateRequest, HintScope, Invocation,
    OperationMode, QualityRequest, ReportRequest, UmbrellaOrigin, UmbrellaRequest, apply_here,
    bare_scope_note, here_scope,
};
pub use parser::{
    BREAKING_AGAINST_OPTION, ci_ignores_env, early_output_flag, is_discovery_exempt,
    json_output_intent, parse, parse_with, parse_with_ci, startup_workspace,
};
pub(crate) use profile::filter_profile_configs;
pub use profile::{DX_PROFILE_ENV, Profile, resolve_profile};

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
