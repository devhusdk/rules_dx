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
    pub(crate) quiet: Option<bool>,
    pub(crate) verbose: Option<bool>,
    pub(crate) color: Option<String>,
    pub(crate) output: Option<String>,
    pub(crate) report: Vec<ReportRequest>,
    pub(crate) fail_on: Option<String>,
    pub(crate) include_edits: bool,
    pub(crate) min_coverage: Option<u32>,
    pub(crate) check: bool,
    pub(crate) apply: bool,
    pub(crate) debug: bool,
    pub(crate) release: bool,
    pub(crate) prune_unobserved: bool,
    pub(crate) configured: bool,
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) here: bool,
    pub(crate) affected: bool,
    pub(crate) serve: bool,
    pub(crate) port: Option<u16>,
    pub(crate) host: Option<String>,
    pub(crate) open: bool,
    pub(crate) offline: bool,
    pub(crate) frozen: bool,
    pub(crate) cases: bool,
    pub(crate) create: Option<String>,
    pub(crate) force: bool,
    pub(crate) disk: bool,
    pub(crate) bases: bool,
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
    /// Hide summaries. =false turns off an inherited default.
    QuietFlag, {
        #[arg(
            long,
            short = 'q',
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
            overrides_with = "verbose"
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
    /// Set output format.
    ReadOutputFlag, {
        #[arg(
            long,
            short = 'o',
            overrides_with = "output",
            value_parser = clap::builder::PossibleValuesParser::new(["text", "json"])
        )]
        output: Option<String>,
    }
}

flag_group! {
    /// Set output format.
    DiffOutputFlag, {
        #[arg(
            long,
            short = 'o',
            overrides_with = "output",
            value_parser = clap::builder::PossibleValuesParser::new(["text", "diff", "json"])
        )]
        output: Option<String>,
    }
}

flag_group! {
    /// Pass one Bazel startup option (repeatable).
    BazelStartupOptionFlag, {
        #[arg(long = "bazel-startup-option", value_name = "TOKEN", allow_hyphen_values = true)]
        bazel_startup_options: Vec<String>,
    }
}

composite_group! {
    /// The flags every command accepts.
    CommonArgs, {
        workspace: WorkspaceFlag,
        quiet: QuietFlag,
        verbose: VerboseFlag,
        color: ColorFlag,
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
    /// Include full edits in JSON change records (needs --output json).
    IncludeEditsFlag, {
        #[arg(long = "include-edits", overrides_with = "include_edits")]
        include_edits: bool,
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
    /// Write fixes to source files.
    FixApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Write updated BUILD declarations.
    GenerateApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Write generated source files.
    CodegenApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Prune unselected managed state.
    CleanApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Write dependency updates.
    UpdateApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Widen the declared requirement.
    BumpApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Declare the new requirement.
    AddApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Rewrite breaking changes.
    MigrateApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Write the managed development environment state.
    EnvApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Scaffold the new files.
    ScaffoldApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Run the upgrade and record the recovery pointer.
    UpgradeApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Restore the previous recorded pin.
    RollbackApplyFlag, {
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
    /// Use release build (the deploy default).
    ReleaseFlag, {
        #[arg(long, overrides_with = "release", conflicts_with = "debug")]
        release: bool,
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
        #[arg(long, overrides_with = "here")]
        here: bool,
    }
}

flag_group! {
    /// Select targets affected by git changes.
    AffectedFlag, {
        #[arg(long, overrides_with = "affected")]
        affected: bool,
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
    /// Docs serve port (needs --serve).
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
    /// Docs serve host (needs --serve).
    HostFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "host", value_parser = non_empty)]
        host: Option<String>,
    }
}

flag_group! {
    /// Open docs in a browser (needs --serve).
    OpenFlag, {
        #[arg(long, overrides_with = "open")]
        open: bool,
    }
}

flag_group! {
    /// Run without network; uses pinned cached snapshots or fails.
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
    /// Enumerate supported test cases via the test runtime.
    CasesFlag, {
        #[arg(long = "cases", overrides_with = "cases")]
        cases: bool,
    }
}

flag_group! {
    /// Start a new branch from this ref (add only).
    CreateFlag, {
        #[arg(long, value_name = "REF", value_parser = non_empty, require_equals = true, overrides_with = "create")]
        create: Option<String>,
    }
}

flag_group! {
    /// Override another holder or discard unpushed work (add and release).
    ForceFlag, {
        #[arg(long, overrides_with = "force")]
        force: bool,
    }
}

flag_group! {
    /// Measure disk use per slot (list only, slow).
    DiskFlag, {
        #[arg(long, overrides_with = "disk")]
        disk: bool,
    }
}

flag_group! {
    /// List or delete every free slot's output base (prune only).
    BasesFlag, {
        #[arg(long, overrides_with = "bases")]
        bases: bool,
    }
}

flag_group! {
    /// Set output format (path works with add only).
    WorktreeOutputFlag, {
        #[arg(
            long,
            short = 'o',
            overrides_with = "output",
            value_parser = clap::builder::PossibleValuesParser::new(["text", "json", "path"])
        )]
        output: Option<String>,
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

/// Stray scopes and the `--` payload a no-scope command refuses.
///
/// Both stay hidden from help but still parse, so the dispatch layer reports its usual errors.
#[derive(Args)]
pub(crate) struct NoScopeTail {
    /// Stray scopes, collected only so they can be rejected.
    #[arg(hide = true)]
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(hide = true, last = true)]
    pub(crate) passthrough: Vec<String>,
}

impl NoScopeTail {
    /// The stray scopes and the `--` payload.
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
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
        output: ReadOutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx license` accepts.
    License => 1, NoPassthroughTail, {
        output: ReadOutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx lint` accepts.
    Lint => 2, BazelTail, {
        output: DiffOutputFlag,
        include_edits: IncludeEditsFlag,
        check: CheckFlag,
        apply: FixApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx typecheck` accepts.
    Typecheck => 3, BazelTail, {
        output: DiffOutputFlag,
        include_edits: IncludeEditsFlag,
        check: CheckFlag,
        apply: FixApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx format` accepts.
    Format => 4, BazelTail, {
        output: DiffOutputFlag,
        include_edits: IncludeEditsFlag,
        check: CheckFlag,
        apply: FixApplyFlag,
        fail_on: FailOnFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx generate` accepts.
    Generate => 5, BazelTail, {
        output: DiffOutputFlag,
        check: CheckFlag,
        apply: GenerateApplyFlag,
        here: HereFlag,
    };
    /// The flags `dx build` accepts.
    Build => 6, BazelTail, {
        output: ReadOutputFlag,
        own: ProfileArgs,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx test` accepts.
    Test => 7, BazelTail, {
        output: ReadOutputFlag,
        own: ProfileArgs,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx coverage` accepts.
    Coverage => 8, BazelTail, {
        output: ReadOutputFlag,
        min_coverage: MinCoverageFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx run` accepts.
    Run => 9, AppTail, { output: ReadOutputFlag, own: ProfileArgs, };
    /// The flags `dx deploy` accepts.
    Deploy => 10, AppTail, { own: ProfileArgs, apply: ApplyFlag, };
    /// The flags `dx check` accepts.
    Check => 11, BazelTail, {
        output: DiffOutputFlag,
        include_edits: IncludeEditsFlag,
        check: CheckFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx fix` accepts.
    Fix => 12, BazelTail, {
        output: DiffOutputFlag,
        include_edits: IncludeEditsFlag,
        check: CheckFlag,
        apply: FixApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx clean` accepts.
    Clean => 13, NoScopeTail, { output: ReadOutputFlag, check: CheckFlag, apply: CleanApplyFlag, prune_unobserved: PruneUnobservedFlag, };
    /// The flags `dx update` accepts.
    Update => 14, NoPassthroughTail, {
        output: ReadOutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        check: CheckFlag,
        apply: UpdateApplyFlag,
    };
    /// The flags `dx bump` accepts.
    Bump => 15, NoPassthroughTail, { output: ReadOutputFlag, apply: BumpApplyFlag, own: OfflineFlag, frozen: FrozenFlag, };
    /// The flags `dx add` accepts.
    Add => 16, NoPassthroughTail, { output: ReadOutputFlag, apply: AddApplyFlag, own: OfflineFlag, frozen: FrozenFlag, };
    /// The flags `dx migrate` accepts.
    Migrate => 17, NoPassthroughTail, { output: ReadOutputFlag, apply: MigrateApplyFlag, own: MigrationArgs, };
    /// The flags `dx codegen` accepts.
    Codegen => 18, BazelTail, { output: ReadOutputFlag, check: CheckFlag, apply: CodegenApplyFlag, };
    /// The flags `dx env` accepts.
    Env => 19, BazelTail, { output: ReadOutputFlag, check: CheckFlag, apply: EnvApplyFlag, };
    /// The flags `dx init` accepts.
    Init => 20, NoPassthroughTail, { apply: ScaffoldApplyFlag, };
    /// The flags `dx new` accepts.
    New => 21, NoPassthroughTail, { apply: ScaffoldApplyFlag, };
    /// The flags `dx upgrade` accepts.
    Upgrade => 22, NoScopeTail, { output: ReadOutputFlag, apply: UpgradeApplyFlag, own: MigrationArgs, };
    /// The flags `dx status` accepts.
    Status => 23, NoScopeTail, {
        output: ReadOutputFlag,
    };
    /// The flags `dx version` accepts.
    Version => 24, NoScopeTail, {
        output: ReadOutputFlag,
        check: CheckFlag,
    };
    /// The flags `dx watch` accepts.
    Watch => 25, BazelTail, {
        output: DiffOutputFlag,
        include_edits: IncludeEditsFlag,
        check: CheckFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        own: ProfileArgs,
    };
    /// The flags `dx completion` accepts.
    Completion => 26, NoPassthroughTail, {};
    /// The flags `dx docs` accepts.
    Docs => 27, BazelTail, {
        output: ReadOutputFlag,
        check: CheckFlag,
        here: HereFlag,
        own: ServeArgs,
    };
    /// The flags `dx list-tests` accepts.
    ListTests => 28, BazelTail, {
        output: ReadOutputFlag,
        own: CasesFlag,
        configured: ConfiguredFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx rollback` accepts.
    Rollback => 29, NoScopeTail, { output: ReadOutputFlag, apply: RollbackApplyFlag, };
    /// The flags `dx worktree` accepts.
    Worktree => 30, NoPassthroughTail, {
        output: WorktreeOutputFlag,
        apply: ApplyFlag,
        create: CreateFlag,
        force: ForceFlag,
        disk: DiskFlag,
        bases: BasesFlag,
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
    let cwd = dx_process::current_directory().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let start = dx_process::workspace_start(&cwd);
    super::completion::with_scope_completers(Cli::command(), std::env::args_os().skip(1), &start)
}

/// Renders the section 1 man page for the dx command.
pub fn man_page() -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    clap_mangen::Man::new(cli_command())
        .section("1")
        .render(&mut buffer)?;
    Ok(buffer)
}

pub(crate) const VALUE_OPTIONS: &[&str] = &[
    "--workspace",
    "--output",
    "-o",
    "--report",
    "--fail-on",
    "--min-coverage",
    "--from",
    "--to",
    "--port",
    "--host",
    "--color",
    "--bazel-startup-option",
];

/// Every short letter in the clap command tree.
pub(crate) fn short_flags() -> &'static [char] {
    static SHORTS: std::sync::OnceLock<Vec<char>> = std::sync::OnceLock::new();
    SHORTS.get_or_init(|| {
        let mut command = <Cli as clap::CommandFactory>::command();
        command.build();
        let mut letters = Vec::new();
        collect_shorts(&command, &mut letters);
        letters.sort_unstable();
        letters.dedup();
        letters
    })
}

/// Adds the short letters of one command and its subcommands.
fn collect_shorts(command: &clap::Command, letters: &mut Vec<char>) {
    letters.extend(command.get_arguments().filter_map(clap::Arg::get_short));
    for sub in command.get_subcommands() {
        collect_shorts(sub, letters);
    }
}

/// Whether a short letter is listed in VALUE_OPTIONS.
fn short_takes_value(letter: char) -> bool {
    letter != '-'
        && VALUE_OPTIONS.iter().any(|option| {
            let mut chars = option.chars();
            chars.next() == Some('-') && chars.next() == Some(letter) && chars.next().is_none()
        })
}

/// Splits a short cluster into its value letter, value, and whether it used the next word.
pub(crate) fn short_cluster(
    word: &str,
    next: Option<&str>,
) -> Option<(char, Option<String>, bool)> {
    let letters = word
        .strip_prefix('-')
        .filter(|rest| !rest.starts_with('-'))?;
    let mut walk = letters.char_indices().peekable();
    while let Some((at, letter)) = walk.next() {
        if short_takes_value(letter) {
            let rest = &letters[at + letter.len_utf8()..];
            if let Some(value) = rest.strip_prefix('=') {
                return Some((letter, Some(value.to_owned()), false));
            }
            if !rest.is_empty() {
                return Some((letter, Some(rest.to_owned()), false));
            }
            return Some(match next {
                Some(next) if !next.is_empty() && !next.starts_with('-') => {
                    (letter, Some(next.to_owned()), true)
                }
                _ => (letter, None, false),
            });
        }
        if !short_flags().contains(&letter) || walk.peek().is_some_and(|(_, next)| *next == '=') {
            return None;
        }
    }
    None
}

/// Whether `word` is a value option whose value is the separate word `next`.
pub(crate) fn consumes_next(word: &str, next: Option<&str>) -> bool {
    if word.contains('=') {
        return false;
    }
    if word.starts_with("--") {
        return VALUE_OPTIONS.contains(&word);
    }
    short_cluster(word, next).is_some_and(|(_, _, used_next)| used_next)
}

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

/// Whether `dx <command>` parses `--<long>`, hidden arguments included.
pub(crate) fn accepts_long(command: super::command::Command, long: &str) -> bool {
    subcommand_args(command)
        .get_arguments()
        .any(|arg| arg.get_long() == Some(long))
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
    fn every_grammar_index_names_its_own_command() {
        use clap::{CommandFactory, ValueEnum};
        let cli = Cli::command();
        for (index, command) in super::super::command::Command::value_variants()
            .iter()
            .enumerate()
        {
            assert_eq!(COMMANDS[index].command, *command, "index {index}");
            assert_eq!(
                subcommand_args(*command).get_name(),
                command.name(),
                "{command:?} grammar index"
            );
            let sub = cli
                .find_subcommand(command.name())
                .unwrap_or_else(|| panic!("{command:?} has a subcommand"));
            assert_eq!(
                sub.get_about().map(ToString::to_string).as_deref(),
                Some(command.describe()),
                "{command:?} about"
            );
        }
    }

    #[test]
    fn the_grammar_rejects_conflicting_value_types() {
        for words in [
            vec!["build", "--debug", "--release"],
            vec!["build", "--release", "--debug"],
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
    fn output_modes_follow_the_command_family() {
        let accepts = |words: &[&str]| kind_of(words) == clap::error::ErrorKind::UnknownArgument;
        for words in [
            vec!["build", "-o", "json"],
            vec!["lint", "--output=diff"],
            vec!["lint", "-ojson"],
            vec!["security", "--output", "text"],
        ] {
            assert!(accepts(&words), "words: {words:?}");
        }
        for (words, kind) in [
            (
                vec!["build", "--output=diff"],
                clap::error::ErrorKind::InvalidValue,
            ),
            (
                vec!["security", "-o", "diff"],
                clap::error::ErrorKind::InvalidValue,
            ),
            (
                vec!["new", "rust", "--output=json"],
                clap::error::ErrorKind::UnknownArgument,
            ),
            (
                vec!["completion", "bash", "-o", "json"],
                clap::error::ErrorKind::UnknownArgument,
            ),
        ] {
            let got = Cli::try_parse_from(
                std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
            );
            assert_eq!(
                got.err().map(|error| error.kind()),
                Some(kind),
                "words: {words:?}"
            );
        }
    }

    #[test]
    fn short_clusters_parse_the_output_value() {
        for words in [
            vec!["lint", "-qojson"],
            vec!["lint", "-vo", "json"],
            vec!["lint", "-qvojson"],
            vec!["lint", "-qo=json"],
        ] {
            let cli = Cli::try_parse_from(
                std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
            )
            .unwrap_or_else(|error| panic!("words: {words:?} must parse: {error}"));
            let (_, flags, _, _) = cli.verb.expect("verb").into_parts();
            assert_eq!(flags.output.as_deref(), Some("json"), "words: {words:?}");
        }
    }

    #[test]
    fn value_options_list_every_value_taking_short_flag() {
        let mut command = <Cli as clap::CommandFactory>::command();
        command.build();
        let mut stack = vec![&command];
        let mut seen = 0;
        while let Some(command) = stack.pop() {
            for arg in command.get_arguments() {
                let Some(letter) = arg.get_short() else {
                    continue;
                };
                seen += 1;
                assert!(
                    short_flags().contains(&letter),
                    "-{letter} is in short_flags"
                );
                let takes_value = arg.get_action().takes_values()
                    && arg
                        .get_num_args()
                        .is_some_and(|range| range.min_values() >= 1);
                assert_eq!(
                    VALUE_OPTIONS.contains(&format!("-{letter}").as_str()),
                    takes_value,
                    "-{letter} in {} must match VALUE_OPTIONS",
                    command.get_name()
                );
            }
            stack.extend(command.get_subcommands());
        }
        assert!(seen > 0);
    }

    #[test]
    fn short_cluster_mirrors_clap() {
        let some = |value: &str, used: bool| Some(('o', Some(value.to_owned()), used));
        assert_eq!(short_cluster("-qojson", None), some("json", false));
        assert_eq!(short_cluster("-vo", Some("json")), some("json", true));
        assert_eq!(short_cluster("-qvojson", None), some("json", false));
        assert_eq!(short_cluster("-qo=json", None), some("json", false));
        assert_eq!(short_cluster("-o=", None), some("", false));
        assert_eq!(short_cluster("-o", Some("text")), some("text", true));
        assert_eq!(short_cluster("-qo", None), Some(('o', None, false)));
        assert_eq!(
            short_cluster("-qo", Some("--quiet")),
            Some(('o', None, false))
        );
        assert_eq!(short_cluster("-qo", Some("")), Some(('o', None, false)));
        assert!(consumes_next("--output", Some("json")));
        assert!(!consumes_next("--output=json", Some("json")));
        assert!(consumes_next("-o", Some("json")));
        assert!(consumes_next("-qo", Some("json")));
        assert!(!consumes_next("-qo", Some("--quiet")));
        assert!(!consumes_next("-q", Some("json")));
        assert!(!consumes_next("--check", Some("json")));
        assert_eq!(short_cluster("-q=false", None), None);
        assert_eq!(short_cluster("-json", None), None);
        assert_eq!(short_cluster("-qv", Some("json")), None);
        assert_eq!(short_cluster("--output=json", None), None);
        assert_eq!(short_cluster("-", Some("json")), None);
        assert_eq!(short_cluster("lint", None), None);
    }

    #[test]
    fn the_grammar_rejects_the_prefix_flag_order() {
        for words in [
            vec!["--output=json", "lint"],
            vec!["--workspace", "/repo", "lint"],
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
