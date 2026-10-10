use std::ffi::OsString;

use clap::{Args, Parser};

use super::command::COMMANDS;
use super::invocation::ReportRequest;

/// A report request.
fn report_request(raw: &str) -> Result<ReportRequest, String> {
    match raw.split_once('=') {
        Some((format, destination)) if !format.is_empty() && !destination.is_empty() => {
            Ok(ReportRequest {
                format: format.to_owned(),
                destination: destination.to_owned(),
            })
        }
        _ => Err(format!("want <format>=<destination>, got {raw:?}")),
    }
}

/// A value that must not be empty.
fn non_empty(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        Err("want a value".to_owned())
    } else {
        Ok(raw.to_owned())
    }
}

/// A coverage percent between 0 and 100.
fn percent(raw: &str) -> Result<u32, String> {
    match raw.parse::<u32>() {
        Ok(value) if value <= 100 => Ok(value),
        _ => Err(format!("want 0-100, got {raw:?}")),
    }
}

/// The usage line clap renders for one command.
fn usage_of(index: usize) -> String {
    let usage = COMMANDS[index].usage;
    usage.strip_prefix("Usage: ").unwrap_or(usage).to_owned()
}

/// The prose clap prints under one command's options.
fn after_help_of(index: usize) -> String {
    super::help::after_long_help(COMMANDS[index].command)
}

/// Every flag the normalized invocation carries, whether or not one command takes it.
#[derive(Debug, Default)]
pub(crate) struct Flags {
    pub(crate) workspace: Option<OsString>,
    pub(crate) dry_run: Option<bool>,
    pub(crate) quiet: Option<bool>,
    pub(crate) verbose: Option<bool>,
    pub(crate) color: Option<String>,
    pub(crate) log_level: Option<String>,
    pub(crate) output: Option<String>,
    pub(crate) report: Vec<ReportRequest>,
    pub(crate) fail_on: Option<String>,
    pub(crate) min_coverage: Option<u32>,
    pub(crate) strict_evidence: bool,
    pub(crate) run_output: Option<String>,
    pub(crate) baseline: Option<String>,
    pub(crate) check: bool,
    pub(crate) apply: bool,
    pub(crate) debug: bool,
    pub(crate) release: bool,
    pub(crate) bazel_clean: bool,
    pub(crate) prune_unobserved: bool,
    pub(crate) pin: Option<String>,
    pub(crate) rollback: bool,
    pub(crate) configured: bool,
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) here: bool,
    pub(crate) serve: bool,
    pub(crate) port: Option<u16>,
    pub(crate) host: Option<String>,
    pub(crate) open: bool,
    pub(crate) offline: bool,
    pub(crate) frozen: bool,
    pub(crate) workspace_capabilities: bool,
    pub(crate) cases: bool,
    pub(crate) bazel_startup_options: Vec<String>,
}

/// One flag group: a clap argument group that writes into the normalized invocation flags.
macro_rules! flag_group {
    ($(#[$group_attr:meta])* $name:ident, { $(#[$attr:meta])* $field:ident : $ty:ty $(,)? }) => {
        $(#[$group_attr])*
        #[derive(Args)]
        pub(crate) struct $name {
            $(#[$group_attr])*
            $(#[$attr])*
            pub(crate) $field: $ty,
        }

        impl $name {
            /// Writes this flag into the invocation.
            pub(crate) fn put(self, flags: &mut Flags) {
                flags.$field = self.$field;
            }
        }
    };
}

/// One group of other groups, whose flags are all written through `put`.
macro_rules! composite_group {
    ($(#[$group_attr:meta])* $name:ident, { $( $(#[$attr:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $(#[$group_attr])*
        #[derive(Args)]
        pub(crate) struct $name {
            $( $(#[$attr])* #[command(flatten)] pub(crate) $field: $ty, )*
        }

        impl $name {
            /// Writes every flag this group carries into the invocation.
            pub(crate) fn put(self, flags: &mut Flags) {
                $( self.$field.put(flags); )*
            }
        }
    };
}

flag_group! {
    /// Use this workspace dir.
    WorkspaceFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "workspace")]
        workspace: Option<OsString>,
    }
}

flag_group! {
    /// Show the plan without running it. =false turns off an inherited default.
    DryRunFlag, {
        #[arg(
            long,
            action = clap::ArgAction::Set,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "true",
            value_parser = clap::value_parser!(bool),
            overrides_with = "dry_run"
        )]
        dry_run: Option<bool>,
    }
}

flag_group! {
    /// Hide summaries. =false turns off an inherited default.
    QuietFlag, {
        #[arg(
            long,
            action = clap::ArgAction::Set,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "true",
            value_parser = clap::value_parser!(bool),
            overrides_with = "quiet"
        )]
        quiet: Option<bool>,
    }
}

flag_group! {
    /// Show more logs. =false turns off an inherited default.
    VerboseFlag, {
        #[arg(
            long,
            short = 'v',
            action = clap::ArgAction::Set,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "true",
            value_parser = clap::value_parser!(bool),
            overrides_with = "verbose",
            conflicts_with = "log_level"
        )]
        verbose: Option<bool>,
    }
}

flag_group! {
    /// Set color output.
    ColorFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "color")]
        color: Option<String>,
    }
}

flag_group! {
    /// Set log level.
    LogLevelFlag, {
        #[arg(
            long = "log-level",
            value_name = "LEVEL",
            allow_negative_numbers = true,
            overrides_with = "log_level",
            conflicts_with = "verbose"
        )]
        log_level: Option<String>,
    }
}

flag_group! {
    /// Set output format.
    OutputFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "output")]
        output: Option<String>,
    }
}

flag_group! {
    /// Pass one Bazel startup option (repeatable).
    BazelStartupOptionFlag, {
        #[arg(long = "bazel-startup-option", value_name = "TOKEN")]
        bazel_startup_options: Vec<String>,
    }
}

composite_group! {
    /// The flags every command accepts.
    CommonArgs, {
        workspace: WorkspaceFlag,
        dry_run: DryRunFlag,
        quiet: QuietFlag,
        verbose: VerboseFlag,
        color: ColorFlag,
        log_level: LogLevelFlag,
        output: OutputFlag,
        bazel_startup_option: BazelStartupOptionFlag,
    }
}

flag_group! {
    /// Write a report file.
    ReportFlag, {
        #[arg(long, allow_negative_numbers = true, value_parser = report_request)]
        report: Vec<ReportRequest>,
    }
}

flag_group! {
    /// Fail on this severity.
    FailOnFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "fail_on")]
        fail_on: Option<String>,
    }
}

flag_group! {
    /// Require at least this coverage percent (0-100).
    MinCoverageFlag, {
        #[arg(
            long,
            value_name = "PERCENT",
            allow_negative_numbers = true,
            overrides_with = "min_coverage",
            value_parser = percent
        )]
        min_coverage: Option<u32>,
    }
}

flag_group! {
    /// Fail when collected test evidence is incomplete.
    StrictEvidenceFlag, {
        #[arg(long = "strict-evidence", overrides_with = "strict_evidence")]
        strict_evidence: bool,
    }
}

flag_group! {
    /// Retain test logs under this directory.
    RunOutputFlag, {
        #[arg(long = "run-output", value_name = "DIR", allow_negative_numbers = true, overrides_with = "run_output", value_parser = non_empty)]
        run_output: Option<String>,
    }
}

flag_group! {
    /// Suppress baselined diagnostics from a versioned JSON file.
    BaselineFlag, {
        #[arg(long = "baseline", value_name = "PATH", allow_negative_numbers = true, overrides_with = "baseline", value_parser = non_empty)]
        baseline: Option<String>,
    }
}

flag_group! {
    /// Check without changing files.
    CheckFlag, {
        #[arg(long, overrides_with = "check")]
        check: bool,
    }
}

flag_group! {
    /// Authorize the managed mutation or effect.
    ApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Use debug build.
    DebugFlag, {
        #[arg(long, overrides_with = "debug", conflicts_with = "release")]
        debug: bool,
    }
}

flag_group! {
    /// Use release build.
    ReleaseFlag, {
        #[arg(long, overrides_with = "release", conflicts_with = "debug")]
        release: bool,
    }
}

flag_group! {
    /// Also run bazel clean.
    BazelCleanFlag, {
        #[arg(long = "bazel", overrides_with = "bazel_clean")]
        bazel_clean: bool,
    }
}

flag_group! {
    /// Prune generations process observation cannot see.
    PruneUnobservedFlag, {
        #[arg(long = "prune-unobserved", overrides_with = "prune_unobserved")]
        prune_unobserved: bool,
    }
}

flag_group! {
    /// Re-pin to this version.
    PinFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "pin", value_parser = non_empty)]
        pin: Option<String>,
    }
}

flag_group! {
    /// Re-pin the last release.
    RollbackFlag, {
        #[arg(long, overrides_with = "rollback")]
        rollback: bool,
    }
}

flag_group! {
    /// Use cquery instead of query.
    ConfiguredFlag, {
        #[arg(long, overrides_with = "configured")]
        configured: bool,
    }
}

flag_group! {
    /// Migrate from this version.
    FromFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "from", value_parser = non_empty)]
        from: Option<String>,
    }
}

flag_group! {
    /// Migrate to this version.
    ToFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "to", value_parser = non_empty)]
        to: Option<String>,
    }
}

flag_group! {
    /// Use the current dir tree.
    HereFlag, {
        #[arg(long, visible_alias = "cwd", overrides_with = "here")]
        here: bool,
    }
}

flag_group! {
    /// Serve docs locally.
    ServeFlag, {
        #[arg(long, overrides_with = "serve")]
        serve: bool,
    }
}

flag_group! {
    /// Docs serve port.
    PortFlag, {
        #[arg(
            long,
            allow_negative_numbers = true,
            overrides_with = "port",
            value_parser = clap::value_parser!(u16).range(1..)
        )]
        port: Option<u16>,
    }
}

flag_group! {
    /// Docs serve host.
    HostFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "host", value_parser = non_empty)]
        host: Option<String>,
    }
}

flag_group! {
    /// Open docs in a browser.
    OpenFlag, {
        #[arg(long, overrides_with = "open")]
        open: bool,
    }
}

flag_group! {
    /// Run without network.
    OfflineFlag, {
        #[arg(long, overrides_with = "offline")]
        offline: bool,
    }
}

flag_group! {
    /// Keep manifest and lock resolution unchanged.
    FrozenFlag, {
        #[arg(long, overrides_with = "frozen")]
        frozen: bool,
    }
}

flag_group! {
    /// Include workspace facts from local records.
    WorkspaceCapabilitiesFlag, {
        #[arg(long = "workspace-capabilities", overrides_with = "workspace_capabilities")]
        workspace_capabilities: bool,
    }
}

flag_group! {
    /// Enumerate supported test cases via the test runtime.
    CasesFlag, {
        #[arg(long = "cases", overrides_with = "cases")]
        cases: bool,
    }
}

composite_group! {
    /// The profile flags the workflow commands share.
    ProfileArgs, {
        debug: DebugFlag,
        release: ReleaseFlag,
    }
}

composite_group! {
    /// The docs serve flags.
    ServeArgs, {
        serve: ServeFlag,
        port: PortFlag,
        host: HostFlag,
        open: OpenFlag,
    }
}

composite_group! {
    /// The migration gate flags.
    MigrationArgs, {
        from: FromFlag,
        to: ToFlag,
    }
}

/// Scopes and the Bazel passthrough.
#[derive(Args)]
pub(crate) struct BazelTail {
    /// Scopes to run on.
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(last = true, value_name = "BAZEL_OPTIONS")]
    pub(crate) passthrough: Vec<String>,
}

impl BazelTail {
    /// The scopes and the `--` payload.
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
    }
}

/// Scopes and the app passthrough.
#[derive(Args)]
pub(crate) struct AppTail {
    /// Targets to run.
    #[arg(value_name = "TARGET")]
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(last = true, value_name = "APP_ARGS")]
    pub(crate) passthrough: Vec<String>,
}

impl AppTail {
    /// The scopes and the app arguments.
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
    }
}

/// Scopes and the `--` payload this command refuses.
#[derive(Args)]
pub(crate) struct NoPassthroughTail {
    /// Scopes to run on.
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(hide = true, last = true)]
    pub(crate) passthrough: Vec<String>,
}

impl NoPassthroughTail {
    /// The scopes and the `--` payload.
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
    }
}

/// The raw Bazel payload this command forwards.
#[derive(Args)]
pub(crate) struct NoScopeTail {
    /// Args after --.
    #[arg(last = true, value_name = "BAZEL_OPTIONS")]
    pub(crate) passthrough: Vec<String>,
}

impl NoScopeTail {
    /// No scopes, and the raw Bazel payload.
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (Vec::new(), self.passthrough)
    }
}

macro_rules! command_args {
    ($(
        $(#[$group_attr:meta])*
        $variant:ident => $index:expr, $tail:ident, { $( $(#[$attr:meta])* $field:ident : $ty:ty ),* $(,)? };
    )*) => {
        $(
            $(#[$group_attr])*
            #[derive(Args)]
            pub(crate) struct $variant {
                #[command(flatten)]
                pub(crate) common: CommonArgs,
                $(
                    $(#[$attr])*
                    #[command(flatten)]
                    pub(crate) $field: $ty,
                )*
                #[command(flatten)]
                pub(crate) tail: $tail,
            }

            impl $variant {
                /// The flags, scopes and `--` payload one command carries.
                pub(crate) fn split(self) -> (Flags, Vec<OsString>, Vec<String>) {
                    let Self { common, tail, $($field,)* } = self;
                    let (targets, passthrough) = tail.split();
                    let mut flags = Flags::default();
                    common.put(&mut flags);
                    $( $field.put(&mut flags); )*
                    (flags, targets, passthrough)
                }
            }
        )*

        #[derive(clap::Subcommand)]
        pub(crate) enum Verb {
            $(
                #[command(
                    name = COMMANDS[$index].name,
                    about = COMMANDS[$index].describe,
                    override_usage = usage_of($index),
                    after_help = after_help_of($index),
                )]
                $variant($variant),
            )*
        }

        impl Verb {
            /// The registry command and the arguments one verb carries.
            pub(crate) fn into_parts(self) -> (super::command::Command, Flags, Vec<OsString>, Vec<String>) {
                match self {
                    $(
                        Verb::$variant(args) => {
                            let (flags, targets, passthrough) = args.split();
                            (super::command::Command::$variant, flags, targets, passthrough)
                        }
                    )*
                }
            }
        }

        /// The arguments one command's subcommand carries, without help prose.
        pub(crate) fn subcommand_args(command: super::command::Command) -> clap::Command {
            match command {
                $(
                    super::command::Command::$variant => <$variant>::augment_args(
                        clap::Command::new(COMMANDS[$index].name),
                    ),
                )*
            }
        }
    }
}

command_args! {
    /// The flags `dx security` accepts.
    Security => 0, NoPassthroughTail, {
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
    };
    /// The flags `dx license` accepts.
    License => 1, NoPassthroughTail, {
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
    };
    /// The flags `dx lint` accepts.
    Lint => 2, BazelTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        baseline: BaselineFlag,
    };
    /// The flags `dx typecheck` accepts.
    Typecheck => 3, BazelTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        baseline: BaselineFlag,
    };
    /// The flags `dx format` accepts.
    Format => 4, BazelTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        here: HereFlag,
        baseline: BaselineFlag,
    };
    /// The flags `dx generate` accepts.
    Generate => 5, BazelTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        here: HereFlag,
    };
    /// The flags `dx build` accepts.
    Build => 6, BazelTail, {
        own: ProfileArgs,
        here: HereFlag,
    };
    /// The flags `dx test` accepts.
    Test => 7, BazelTail, {
        own: ProfileArgs,
        report: ReportFlag,
        strict_evidence: StrictEvidenceFlag,
        run_output: RunOutputFlag,
        here: HereFlag,
    };
    /// The flags `dx coverage` accepts.
    Coverage => 8, BazelTail, {
        min_coverage: MinCoverageFlag,
        strict_evidence: StrictEvidenceFlag,
        report: ReportFlag,
        run_output: RunOutputFlag,
        here: HereFlag,
    };
    /// The flags `dx run` accepts.
    Run => 9, AppTail, { own: ProfileArgs, apply: ApplyFlag, };
    /// The flags `dx deploy` accepts.
    Deploy => 10, AppTail, { own: ProfileArgs, apply: ApplyFlag, };
    /// The flags `dx check` accepts.
    Check => 11, BazelTail, {
        check: CheckFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        baseline: BaselineFlag,
    };
    /// The flags `dx fix` accepts.
    Fix => 12, BazelTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        baseline: BaselineFlag,
    };
    /// The flags `dx clean` accepts.
    Clean => 13, NoPassthroughTail, { check: CheckFlag, apply: ApplyFlag, own: BazelCleanFlag, prune_unobserved: PruneUnobservedFlag, };
    /// The flags `dx update` accepts.
    Update => 14, NoPassthroughTail, {
        offline: OfflineFlag,
        frozen: FrozenFlag,
        check: CheckFlag,
        apply: ApplyFlag,
    };
    /// The flags `dx bump` accepts.
    Bump => 15, NoPassthroughTail, { apply: ApplyFlag, own: OfflineFlag, frozen: FrozenFlag, };
    /// The flags `dx migrate` accepts.
    Migrate => 16, NoPassthroughTail, { apply: ApplyFlag, own: MigrationArgs, };
    /// The flags `dx codegen` accepts.
    Codegen => 17, BazelTail, { check: CheckFlag, apply: ApplyFlag, };
    /// The flags `dx env` accepts.
    Env => 18, BazelTail, { check: CheckFlag, apply: ApplyFlag, };
    /// The flags `dx setup` accepts.
    Setup => 19, BazelTail, { check: CheckFlag, apply: ApplyFlag, };
    /// The flags `dx init` accepts.
    Init => 20, NoPassthroughTail, { apply: ApplyFlag, };
    /// The flags `dx new` accepts.
    New => 21, NoPassthroughTail, { apply: ApplyFlag, };
    /// The flags `dx upgrade` accepts.
    Upgrade => 22, NoPassthroughTail, { apply: ApplyFlag, own: MigrationArgs, };
    /// The flags `dx hooks` accepts.
    Hooks => 23, NoPassthroughTail, { apply: ApplyFlag, };
    /// The flags `dx status` accepts.
    Status => 24, NoPassthroughTail, {};
    /// The flags `dx version` accepts.
    Version => 25, NoPassthroughTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        pin: PinFlag,
        rollback: RollbackFlag,
    };
    /// The flags `dx watch` accepts.
    Watch => 26, NoPassthroughTail, { apply: ApplyFlag, };
    /// The flags `dx owners` accepts.
    Owners => 27, NoPassthroughTail, { own: ConfiguredFlag, };
    /// The flags `dx deps` accepts.
    Deps => 28, NoPassthroughTail, { own: ConfiguredFlag, };
    /// The flags `dx why` accepts.
    Why => 29, NoPassthroughTail, { own: ConfiguredFlag, };
    /// The flags `dx completion` accepts.
    Completion => 30, NoPassthroughTail, { own: CheckFlag, };
    /// The flags `dx docs` accepts.
    Docs => 31, BazelTail, {
        check: CheckFlag,
        apply: ApplyFlag,
        here: HereFlag,
        own: ServeArgs,
    };
    /// The flags `dx bazel` accepts.
    Bazel => 32, NoScopeTail, {};
    /// The flags `dx capabilities` accepts.
    Capabilities => 33, NoPassthroughTail, { own: WorkspaceCapabilitiesFlag, };
    /// The flags `dx verify` accepts.
    Verify => 34, BazelTail, {};
    /// The flags `dx rerun` accepts.
    Rerun => 35, BazelTail, {};
    /// The flags `dx tests` accepts.
    Tests => 36, BazelTail, {
        own: CasesFlag,
        configured: ConfiguredFlag,
        here: HereFlag,
    };
}

#[derive(Parser)]
#[command(
    name = "dx",
    about = "Run Bazel workflows",
    long_about = "dx <command> [flags] [scope ...] [-- bazel-options ...]\n\nScopes: labels, patterns, files, or dirs. No scope means //... for most commands.\n\nExit codes: 0 success, 2 usage error, 1 failed check.",
    version,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    /// Command to run.
    #[command(subcommand)]
    pub(crate) verb: Option<Verb>,
}

pub fn cli_command() -> clap::Command {
    use clap::CommandFactory;
    let start = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    super::completion::with_scope_completers(Cli::command(), std::env::args_os().skip(1), &start)
}

pub(crate) const VALUE_OPTIONS: &[&str] = &[
    "--workspace",
    "--output",
    "--report",
    "--fail-on",
    "--min-coverage",
    "--pin",
    "--from",
    "--to",
    "--port",
    "--host",
    "--color",
    "--log-level",
    "--run-output",
    "--bazel-startup-option",
];

/// The long names the grammar gives the flags that belong to some commands only.
pub(crate) fn advertised_flags(command: super::command::Command) -> Vec<String> {
    let sub = subcommand_args(command);
    let mut flags = Vec::new();
    for arg in sub.get_arguments() {
        if arg.is_hide_set() {
            continue;
        }
        for name in arg.get_long_and_visible_aliases().into_iter().flatten() {
            flags.push(name.to_owned());
        }
    }
    flags.sort();
    flags.dedup();
    flags
}

/// Whether the grammar lets `dx <command> -- <bazel-options>` through.
pub(crate) fn advertises_passthrough(command: super::command::Command) -> bool {
    subcommand_args(command)
        .get_arguments()
        .any(|arg| arg.is_last_set() && !arg.is_hide_set())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind_of(words: &[&str]) -> clap::error::ErrorKind {
        Cli::try_parse_from(
            std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
        )
        .err()
        .map_or(clap::error::ErrorKind::UnknownArgument, |error| {
            error.kind()
        })
    }

    #[test]
    fn the_grammar_rejects_conflicting_value_types() {
        for words in [
            vec!["build", "--debug", "--release"],
            vec!["build", "--release", "--debug"],
            vec!["lint", "--verbose", "--log-level=debug"],
            vec!["lint", "-v", "--log-level=info"],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::ArgumentConflict,
                "words: {words:?}"
            );
        }
        for words in [
            vec!["coverage", "--min-coverage=eighty"],
            vec!["coverage", "--min-coverage=101"],
            vec!["lint", "--report=sarif"],
            vec!["docs", "--serve", "--port=0"],
            vec!["docs", "--serve", "--port=notanumber"],
            vec!["docs", "--serve", "--host="],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::ValueValidation,
                "words: {words:?}"
            );
        }
        for words in [
            vec!["build", "--debug"],
            vec!["build", "--release"],
            vec!["lint", "--verbose"],
            vec!["coverage", "--min-coverage=80"],
            vec!["coverage", "--min-coverage=0"],
            vec!["coverage", "--min-coverage=100"],
            vec!["lint", "--report=sarif=out.sarif"],
            vec!["docs", "--serve", "--port=1"],
            vec!["docs", "--serve", "--port=65535"],
            vec!["docs", "--serve", "--host=example.test"],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::UnknownArgument,
                "words: {words:?}"
            );
            Cli::try_parse_from(
                std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
            )
            .unwrap_or_else(|error| panic!("words: {words:?} must parse: {error}"));
        }
    }

    #[test]
    fn the_grammar_rejects_the_prefix_flag_order() {
        for words in [
            vec!["--output=json", "lint"],
            vec!["--workspace", "/repo", "lint"],
            vec!["--dry-run", "build"],
            vec!["--quiet", "typecheck"],
            vec!["--verbose", "lint"],
            vec!["-v", "lint"],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::UnknownArgument,
                "words: {words:?}"
            );
        }
    }

    #[test]
    fn every_command_names_its_own_registry_entry() {
        let root = cli_command();
        for entry in COMMANDS.iter() {
            let built = root
                .get_subcommands()
                .find(|sub| sub.get_name() == entry.name)
                .unwrap_or_else(|| panic!("dx {} is not a subcommand", entry.name));
            assert_eq!(
                built.get_about().map(ToString::to_string).as_deref(),
                Some(entry.describe),
                "dx {} about must come from its registry row",
                entry.name
            );
        }
    }
}
