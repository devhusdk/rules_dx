use std::collections::BTreeSet;
use std::ffi::OsString;

use clap::{Args, Parser, ValueEnum};

use super::command::COMMANDS;
use super::invocation::ReportRequest;

/// An agent tool whose files `dx init --agents` adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
    Gemini,
    Copilot,
}

impl Agent {
    /// Every agent.
    pub const ALL: [Self; 4] = [Self::Claude, Self::Codex, Self::Gemini, Self::Copilot];

    /// The word `--agents` takes for this agent.
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Copilot => "copilot",
        }
    }
}

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

/// A `set:package` pair.
fn set_package(raw: &str) -> Result<String, String> {
    match raw.split_once(':') {
        Some((set, package)) if !set.is_empty() && !package.is_empty() => Ok(raw.to_owned()),
        _ => Err("want set:package".to_owned()),
    }
}

/// A coverage percent between 0 and 100.
fn percent(raw: &str) -> Result<u32, String> {
    match raw.parse::<u32>() {
        Ok(value) if value <= 100 => Ok(value),
        _ => Err(format!("want 0-100, got {raw:?}")),
    }
}

/// The seconds in a fuzz budget such as `90`, `90s`, `5m` or `1h`.
pub fn fuzz_seconds(raw: &str) -> Option<u64> {
    let (digits, unit) = match raw.strip_suffix(['s', 'm', 'h']) {
        Some(digits) => (digits, raw.chars().last()),
        None => (raw, Some('s')),
    };
    let count: u64 = digits.parse().ok().filter(|count| *count > 0)?;
    let scale = match unit? {
        'm' => 60,
        'h' => 3600,
        _ => 1,
    };
    count.checked_mul(scale)
}

fn fuzz_duration(raw: &str) -> Result<String, String> {
    match fuzz_seconds(raw) {
        Some(_) => Ok(raw.to_owned()),
        None => Err(format!(
            "want a duration such as 90s, 5m or 1h, got {raw:?}"
        )),
    }
}

fn survived_percent(raw: &str) -> Result<u32, String> {
    match raw.strip_prefix("survived:") {
        Some(rest) => percent(rest),
        None => Err(format!("want survived:<pct>, got {raw:?}")),
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
    pub(crate) diff: bool,
    pub(crate) poll: bool,
    pub(crate) path_only: bool,
    pub(crate) shell: Option<String>,
    pub(crate) report: Vec<ReportRequest>,
    pub(crate) fail_on: Option<String>,
    pub(crate) baseline: Option<String>,
    pub(crate) build_slots: Option<String>,
    pub(crate) include_edits: bool,
    pub(crate) min_coverage: Option<u32>,
    pub(crate) min_changed_line_coverage: Option<u32>,
    pub(crate) apply: bool,
    pub(crate) online: bool,
    pub(crate) debug: bool,
    pub(crate) release: bool,
    pub(crate) prune_unobserved: bool,
    pub(crate) agents: Vec<Agent>,
    pub(crate) to: Option<String>,
    pub(crate) here: bool,
    pub(crate) affected: bool,
    pub(crate) serve: bool,
    pub(crate) port: Option<u16>,
    pub(crate) host: Option<String>,
    pub(crate) open: bool,
    pub(crate) offline: bool,
    pub(crate) frozen: bool,
    pub(crate) outdated: bool,
    pub(crate) prune: bool,
    pub(crate) pr: bool,
    pub(crate) group: Option<String>,
    pub(crate) labels: Option<String>,
    pub(crate) automerge: Option<String>,
    pub(crate) min_age: Option<u32>,
    pub(crate) ignore: Vec<String>,
    pub(crate) allow_major: bool,
    pub(crate) prereleases: bool,
    pub(crate) max_prs: Option<u16>,
    pub(crate) floor: bool,
    pub(crate) reason: Option<String>,
    pub(crate) cases: bool,
    pub(crate) create: Option<String>,
    pub(crate) force: bool,
    pub(crate) disk: bool,
    pub(crate) bases: bool,
    pub(crate) capabilities: bool,
    pub(crate) explain: bool,
    pub(crate) build_profile: bool,
    pub(crate) old_log: Option<String>,
    pub(crate) new_log: Option<String>,
    pub(crate) time: Option<String>,
    pub(crate) jobs: Option<u16>,
    pub(crate) fail_survived: Option<u32>,
    pub(crate) bazel_startup_options: Vec<String>,
    pub(crate) bazel_options: Vec<String>,
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
        #[arg(
            long,
            allow_negative_numbers = true,
            overrides_with = "color",
            value_parser = clap::builder::PossibleValuesParser::new(["auto", "always", "never"])
        )]
        color: Option<String>,
    }
}

flag_group! {
    /// Set output format.
    OutputFlag, {
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
    /// Print the edits as a patch.
    DiffFlag, {
        #[arg(long, overrides_with = "diff")]
        diff: bool,
    }
}

flag_group! {
    /// Poll for changes instead of using the platform watcher.
    PollFlag, {
        #[arg(long, overrides_with = "poll")]
        poll: bool,
    }
}

flag_group! {
    /// Print shell lines for the managed bin instead of running env.
    EnvShellFlag, {
        #[arg(
            long,
            value_name = "SHELL",
            require_equals = true,
            overrides_with = "shell",
            value_parser = clap::builder::PossibleValuesParser::new(["direnv"])
        )]
        shell: Option<String>,
    }
}

flag_group! {
    /// Print only the new worktree path.
    PathOnlyFlag, {
        #[arg(long = "path-only", overrides_with = "path_only")]
        path_only: bool,
    }
}

flag_group! {
    /// Pass one Bazel startup option (repeatable).
    BazelStartupOptionFlag, {
        #[arg(long = "bazel-startup-option", value_name = "TOKEN", allow_hyphen_values = true)]
        bazel_startup_options: Vec<String>,
    }
}

flag_group! {
    /// Pass one Bazel option to the build and the run (repeatable).
    BazelOptionFlag, {
        #[arg(long = "bazel-option", value_name = "TOKEN", allow_hyphen_values = true)]
        bazel_options: Vec<String>,
    }
}

flag_group! {
    /// Set build slots: auto, a count, or off.
    BuildSlotsFlag, {
        #[arg(
            long = "build-slots",
            value_name = "auto|n|off",
            allow_negative_numbers = true,
            overrides_with = "build_slots"
        )]
        build_slots: Option<String>,
    }
}

composite_group! {
    /// The flags every command accepts.
    CommonArgs, {
        workspace: WorkspaceFlag,
        quiet: QuietFlag,
        verbose: VerboseFlag,
        color: ColorFlag,
        build_slots: BuildSlotsFlag,
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
        #[arg(
            long,
            allow_negative_numbers = true,
            overrides_with = "fail_on",
            value_parser = clap::builder::PossibleValuesParser::new(["info", "warning", "error"])
        )]
        fail_on: Option<String>,
    }
}

flag_group! {
    /// Fail on this severity, or on lock files without a dependency set.
    SecurityFailOnFlag, {
        #[arg(
            long,
            allow_negative_numbers = true,
            overrides_with = "fail_on",
            value_parser = clap::builder::PossibleValuesParser::new(["info", "warning", "error", "uncovered"])
        )]
        fail_on: Option<String>,
    }
}

flag_group! {
    /// Fail on this severity, or on tests that have no coverage collector.
    CoverageFailOnFlag, {
        #[arg(
            long,
            allow_negative_numbers = true,
            overrides_with = "fail_on",
            value_parser = clap::builder::PossibleValuesParser::new(["info", "warning", "error", "no-collector"])
        )]
        fail_on: Option<String>,
    }
}

flag_group! {
    /// Suppress findings already in this workspace-relative JSON baseline.
    BaselineFlag, {
        #[arg(
            long,
            value_name = "FILE",
            overrides_with = "baseline",
            value_parser = dx_adopt::defaults::check_quality_baseline
        )]
        baseline: Option<String>,
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
            value_name = "percent",
            allow_negative_numbers = true,
            overrides_with = "min_coverage",
            value_parser = percent
        )]
        min_coverage: Option<u32>,
    }
}

flag_group! {
    /// Require at least this coverage percent (0-100) on lines changed since the merge-base.
    MinChangedLineCoverageFlag, {
        #[arg(
            long,
            value_name = "percent",
            allow_negative_numbers = true,
            overrides_with = "min_changed_line_coverage",
            value_parser = percent
        )]
        min_changed_line_coverage: Option<u32>,
    }
}

flag_group! {
    /// Apply the changes instead of only showing them.
    ApplyFlag, {
        #[arg(long, overrides_with = "apply")]
        apply: bool,
    }
}

flag_group! {
    /// Check external links too (lychee, needs network).
    OnlineFlag, {
        #[arg(long, overrides_with = "online")]
        online: bool,
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
    /// Agent tools whose files init adds.
    AgentsFlag, {
        #[arg(long, value_enum, value_delimiter = ',', value_name = "LIST", require_equals = true)]
        agents: Vec<Agent>,
    }
}

flag_group! {
    /// Version to pin; must equal this binary.
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
    /// List packages whose newest stable version is outside the declared range.
    OutdatedFlag, {
        #[arg(long, overrides_with = "outdated")]
        outdated: bool,
    }
}

flag_group! {
    /// Remove unused dependencies from the manifests (needs --apply).
    PruneFlag, {
        #[arg(long, overrides_with = "prune")]
        prune: bool,
    }
}

flag_group! {
    /// Open or refresh one pull request per group of updates.
    PrFlag, {
        #[arg(long, overrides_with = "pr")]
        pr: bool,
    }
}

flag_group! {
    /// Group updates per package, set or all (needs --pr; default set).
    GroupFlag, {
        #[arg(
            long,
            value_name = "GROUP",
            require_equals = true,
            overrides_with = "group",
            value_parser = clap::builder::PossibleValuesParser::new(["package", "set", "all"])
        )]
        group: Option<String>,
    }
}

flag_group! {
    /// Comma-separated labels for new pull requests (needs --pr).
    LabelsFlag, {
        #[arg(long, value_name = "LABELS", require_equals = true, overrides_with = "labels", value_parser = non_empty)]
        labels: Option<String>,
    }
}

flag_group! {
    /// Auto-merge level: none, patch, minor or all (needs --pr; default patch).
    AutomergeFlag, {
        #[arg(
            long,
            value_name = "LEVEL",
            require_equals = true,
            overrides_with = "automerge",
            value_parser = clap::builder::PossibleValuesParser::new(["none", "patch", "minor", "all"])
        )]
        automerge: Option<String>,
    }
}

flag_group! {
    /// Skip versions published fewer than this many days ago (needs --pr; default 3).
    MinAgeFlag, {
        #[arg(long, value_name = "DAYS", require_equals = true, overrides_with = "min_age")]
        min_age: Option<u32>,
    }
}

flag_group! {
    /// Skip one package, as set:package; repeatable (needs --pr).
    IgnoreFlag, {
        #[arg(
            long,
            value_name = "SET:PACKAGE",
            require_equals = true,
            action = clap::ArgAction::Append,
            value_parser = set_package
        )]
        ignore: Vec<String>,
    }
}

flag_group! {
    /// Include major version updates (needs --pr).
    AllowMajorFlag, {
        #[arg(long, overrides_with = "allow_major")]
        allow_major: bool,
    }
}

flag_group! {
    /// Offer prerelease versions; they are never auto-merged (needs --pr).
    PrereleasesFlag, {
        #[arg(long, overrides_with = "prereleases")]
        prereleases: bool,
    }
}

flag_group! {
    /// Open at most this many dx-update pull requests (needs --pr; default 5).
    MaxPrsFlag, {
        #[arg(
            long,
            value_name = "N",
            require_equals = true,
            overrides_with = "max_prs",
            value_parser = clap::value_parser!(u16).range(1..)
        )]
        max_prs: Option<u16>,
    }
}

flag_group! {
    /// Edit the bazel_dep floor instead of the tested override (needs a version and --reason).
    FloorFlag, {
        #[arg(long, overrides_with = "floor")]
        floor: bool,
    }
}

flag_group! {
    /// Why this floor is the oldest supported version (needs --floor).
    ReasonFlag, {
        #[arg(long, value_name = "TEXT", overrides_with = "reason", value_parser = non_empty)]
        reason: Option<String>,
    }
}

composite_group! {
    /// The floor flags.
    FloorArgs, {
        floor: FloorFlag,
        reason: ReasonFlag,
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
    /// Add one capability record per command (status only).
    CapabilitiesFlag, {
        #[arg(long, overrides_with = "capabilities")]
        capabilities: bool,
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

flag_group! {
    /// Save the execution log for dx explain.
    ExplainFlag, {
        #[arg(long, overrides_with = "explain")]
        explain: bool,
    }
}

flag_group! {
    /// Print where build time went.
    BuildProfileFlag, {
        #[arg(long = "profile", overrides_with = "build_profile")]
        build_profile: bool,
    }
}

flag_group! {
    /// Older execution log (default: the second newest saved log).
    OldLogFlag, {
        #[arg(long = "old", value_name = "LOG", overrides_with = "old_log", value_parser = non_empty)]
        old_log: Option<String>,
    }
}

flag_group! {
    /// Newer execution log (default: the newest saved log).
    NewLogFlag, {
        #[arg(long = "new", value_name = "LOG", overrides_with = "new_log", value_parser = non_empty)]
        new_log: Option<String>,
    }
}

composite_group! {
    /// The two execution logs dx explain compares.
    LogPairArgs, {
        old_log: OldLogFlag,
        new_log: NewLogFlag,
    }
}

flag_group! {
    /// Time budget per target, such as 90s, 5m or 1h (default 60s).
    TimeFlag, {
        #[arg(long, value_name = "DURATION", overrides_with = "time", value_parser = fuzz_duration)]
        time: Option<String>,
    }
}

flag_group! {
    /// Jobs each fuzz runner may use.
    JobsFlag, {
        #[arg(
            long,
            value_name = "N",
            overrides_with = "jobs",
            value_parser = clap::value_parser!(u16).range(1..)
        )]
        jobs: Option<u16>,
    }
}

flag_group! {
    /// Fail when more than this percent of mutants survive.
    FailSurvivedFlag, {
        #[arg(
            long = "fail-on",
            value_name = "survived:PCT",
            overrides_with = "fail_survived",
            value_parser = survived_percent
        )]
        fail_survived: Option<u32>,
    }
}

composite_group! {
    /// The mutation budget and gate flags.
    MutateArgs, {
        time: TimeFlag,
        fail_survived: FailSurvivedFlag,
    }
}

composite_group! {
    /// The fuzz budget flags.
    FuzzArgs, {
        time: TimeFlag,
        jobs: JobsFlag,
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
        output: OutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: SecurityFailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx license` accepts.
    License => 1, NoPassthroughTail, {
        output: OutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx lint` accepts.
    Lint => 2, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        apply: ApplyFlag,
        online: OnlineFlag,
        fail_on: FailOnFlag,
        baseline: BaselineFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx typecheck` accepts.
    Typecheck => 3, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        baseline: BaselineFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx format` accepts.
    Format => 4, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        baseline: BaselineFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx generate` accepts.
    Generate => 5, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        apply: ApplyFlag,
        here: HereFlag,
    };
    /// The flags `dx build` accepts.
    Build => 6, BazelTail, {
        output: OutputFlag,
        own: ProfileArgs,
        explain: ExplainFlag,
        build_profile: BuildProfileFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx test` accepts.
    Test => 7, BazelTail, {
        output: OutputFlag,
        own: ProfileArgs,
        explain: ExplainFlag,
        build_profile: BuildProfileFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx coverage` accepts.
    Coverage => 8, BazelTail, {
        output: OutputFlag,
        fail_on: CoverageFailOnFlag,
        min_coverage: MinCoverageFlag,
        min_changed_line_coverage: MinChangedLineCoverageFlag,
        explain: ExplainFlag,
        build_profile: BuildProfileFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx bench` accepts.
    Bench => 9, BazelTail, {
        output: OutputFlag,
        here: HereFlag,
    };
    /// The flags `dx run` accepts.
    Run => 10, AppTail, {
        output: OutputFlag,
        own: ProfileArgs,
        bazel_option: BazelOptionFlag,
    };
    /// The flags `dx deploy` accepts.
    Deploy => 11, AppTail, { own: ProfileArgs, apply: ApplyFlag, };
    /// The flags `dx check` accepts.
    Check => 12, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        baseline: BaselineFlag,
        explain: ExplainFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    /// The flags `dx clean` accepts.
    Clean => 13, NoScopeTail, { output: OutputFlag, apply: ApplyFlag, prune_unobserved: PruneUnobservedFlag, };
    /// The flags `dx update` accepts.
    Update => 14, NoPassthroughTail, {
        output: OutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        apply: ApplyFlag,
        outdated: OutdatedFlag,
        prune: PruneFlag,
        pr: PrFlag,
        group: GroupFlag,
        labels: LabelsFlag,
        automerge: AutomergeFlag,
        min_age: MinAgeFlag,
        ignore: IgnoreFlag,
        allow_major: AllowMajorFlag,
        prereleases: PrereleasesFlag,
        max_prs: MaxPrsFlag,
        floor: FloorArgs,
    };
    /// The flags `dx env` accepts.
    Env => 15, BazelTail, { output: OutputFlag, apply: ApplyFlag, shell: EnvShellFlag, };
    /// The flags `dx init` accepts.
    Init => 16, NoPassthroughTail, {
        output: OutputFlag,
        apply: ApplyFlag,
        agents: AgentsFlag,
    };
    /// The flags `dx upgrade` accepts.
    Upgrade => 17, NoScopeTail, { output: OutputFlag, apply: ApplyFlag, own: ToFlag, };
    /// The flags `dx status` accepts.
    Status => 18, NoScopeTail, {
        output: OutputFlag,
        capabilities: CapabilitiesFlag,
    };
    /// The flags `dx watch` accepts.
    Watch => 19, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        apply: ApplyFlag,
        fail_on: FailOnFlag,
        baseline: BaselineFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        poll: PollFlag,
        own: ProfileArgs,
    };
    /// The flags `dx completion` accepts.
    Completion => 20, NoPassthroughTail, {};
    /// The flags `dx docs` accepts.
    Docs => 21, BazelTail, {
        output: OutputFlag,
        here: HereFlag,
        own: ServeArgs,
    };
    /// The flags `dx worktree` accepts.
    Worktree => 22, NoPassthroughTail, {
        output: OutputFlag,
        path_only: PathOnlyFlag,
        apply: ApplyFlag,
        create: CreateFlag,
        force: ForceFlag,
        disk: DiskFlag,
        bases: BasesFlag,
    };
    /// The flags `dx explain` accepts.
    Explain => 23, NoScopeTail, { output: OutputFlag, own: LogPairArgs, };
    /// The flags `dx fuzz` accepts.
    Fuzz => 24, NoPassthroughTail, { own: FuzzArgs, here: HereFlag, };
    /// The flags `dx mutate` accepts.
    Mutate => 25, NoPassthroughTail, { own: MutateArgs, here: HereFlag, };
}

#[derive(Parser)]
#[command(
    name = "dx",
    about = "Run Bazel workflows",
    long_about = "dx <command> [flags] [scope ...] [-- bazel-options ...]\n\nScopes: labels, patterns, files, or dirs. No scope means //... for commands that take scopes.\n\nExit codes: 0 success, 2 usage error, 1 failed check.",
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
    super::completion::with_scope_completers(
        Cli::command(),
        std::env::args_os().skip(1),
        &start,
        &cwd,
    )
}

/// Renders the section 1 man page for the dx command.
pub fn man_page() -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    clap_mangen::Man::new(cli_command())
        .section("1")
        .render(&mut buffer)?;
    Ok(buffer)
}

/// Every long and short option in the clap command tree that takes a value, e.g. `--output` and `-o`.
pub(crate) fn value_options() -> &'static BTreeSet<String> {
    static OPTIONS: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut command = <Cli as clap::CommandFactory>::command();
        command.build();
        let mut options = BTreeSet::new();
        collect_value_options(&command, &mut options);
        options
    })
}

/// Adds the value-taking options of one command and its subcommands.
fn collect_value_options(command: &clap::Command, options: &mut BTreeSet<String>) {
    for arg in command.get_arguments().filter(|arg| takes_next_word(arg)) {
        if let Some(short) = arg.get_short() {
            options.insert(format!("-{short}"));
        }
        if let Some(long) = arg.get_long() {
            options.insert(format!("--{long}"));
        }
    }
    for sub in command.get_subcommands() {
        collect_value_options(sub, options);
    }
}

/// Whether an argument takes its value from the next word, not only after `=`.
fn takes_next_word(arg: &clap::Arg) -> bool {
    !arg.is_require_equals_set()
        && arg.get_action().takes_values()
        && arg
            .get_num_args()
            .is_some_and(|range| range.min_values() >= 1)
}

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

/// One option word: its name, inline value, and whether it took the next word.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OptionWord {
    pub(crate) name: String,
    pub(crate) value: Option<String>,
    pub(crate) used_next: bool,
}

/// Reads `word` as an option the way clap does; `next` is the word after it.
pub(crate) fn option_at(word: &str, next: Option<&str>) -> Option<OptionWord> {
    let raw = clap_lex::RawArgs::new([word]);
    let arg = raw.next(&mut raw.cursor())?;
    let (name, value) = if let Some((long, inline)) = arg.to_long() {
        let long = long.ok()?;
        let inline = inline.map(|text| text.to_string_lossy().into_owned());
        (format!("--{long}"), inline)
    } else {
        let mut cluster = arg.to_short()?;
        let mut found = None;
        while let Some(letter) = cluster.next_flag() {
            let letter = letter.ok()?;
            if !short_flags().contains(&letter) {
                return None;
            }
            let name = format!("-{letter}");
            if value_options().contains(&name) {
                let rest = cluster.next_value_os().map(|rest| rest.to_string_lossy());
                let rest = rest.map(|rest| rest.strip_prefix('=').unwrap_or(&rest).to_owned());
                found = Some((name, rest));
                break;
            }
        }
        found?
    };
    let used_next = value.is_none()
        && value_options().contains(&name)
        && next.is_some_and(|next| next_is_value(&name, next));
    let value = match (value, used_next) {
        (None, true) => next.map(str::to_owned),
        (value, _) => value,
    };
    Some(OptionWord {
        name,
        value,
        used_next,
    })
}

/// Whether `next` can be the separate value of the option `name`.
fn next_is_value(name: &str, next: &str) -> bool {
    if next.is_empty() {
        return false;
    }
    if !next.starts_with('-') {
        return true;
    }
    let (hyphen, negative) = hyphen_value_options();
    hyphen.contains(name)
        || (negative.contains(name) && {
            let raw = clap_lex::RawArgs::new([next]);
            raw.next(&mut raw.cursor())
                .is_some_and(|arg| arg.is_negative_number())
        })
}

/// The value options that accept hyphen values, and those that accept negative numbers.
fn hyphen_value_options() -> &'static (BTreeSet<String>, BTreeSet<String>) {
    static OPTIONS: std::sync::OnceLock<(BTreeSet<String>, BTreeSet<String>)> =
        std::sync::OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut command = <Cli as clap::CommandFactory>::command();
        command.build();
        let mut sets = (BTreeSet::new(), BTreeSet::new());
        collect_hyphen_options(&command, &mut sets);
        sets
    })
}

/// Adds the hyphen-accepting value options of one command and its subcommands.
fn collect_hyphen_options(
    command: &clap::Command,
    sets: &mut (BTreeSet<String>, BTreeSet<String>),
) {
    for arg in command.get_arguments().filter(|arg| takes_next_word(arg)) {
        let mut names = Vec::new();
        names.extend(arg.get_short().map(|short| format!("-{short}")));
        names.extend(arg.get_long().map(|long| format!("--{long}")));
        if arg.is_allow_hyphen_values_set() {
            sets.0.extend(names.iter().cloned());
        }
        if arg.is_allow_negative_numbers_set() {
            sets.1.extend(names);
        }
    }
    for sub in command.get_subcommands() {
        collect_hyphen_options(sub, sets);
    }
}

/// The long names the grammar gives the flags that belong to some commands only.
pub fn advertised_flags(command: super::command::Command) -> Vec<String> {
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
pub fn accepts_long(command: super::command::Command, long: &str) -> bool {
    subcommand_args(command)
        .get_arguments()
        .any(|arg| arg.get_long() == Some(long))
}

/// Whether the grammar lets `dx <command> -- <bazel-options>` through.
pub fn advertises_passthrough(command: super::command::Command) -> bool {
    subcommand_args(command)
        .get_arguments()
        .any(|arg| arg.is_last_set() && !arg.is_hide_set())
}

/// Whether the `--` payload of `dx <command>` goes to the app rather than to Bazel.
pub(crate) fn takes_app_args(command: super::command::Command) -> bool {
    subcommand_args(command).get_arguments().any(|arg| {
        arg.is_last_set()
            && !arg.is_hide_set()
            && arg
                .get_value_names()
                .is_some_and(|names| names.iter().any(|name| name.as_str() == "APP_ARGS"))
    })
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
    fn takes_app_args_holds_for_run_and_deploy_only() {
        use super::super::command::Command;
        use clap::ValueEnum;
        for command in Command::value_variants() {
            assert_eq!(
                takes_app_args(*command),
                matches!(command, Command::Run | Command::Deploy),
                "{command:?}"
            );
        }
    }

    #[test]
    fn bazel_option_parses_only_under_run() {
        let parses = |words: &[&str]| {
            Cli::try_parse_from(
                std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
            )
            .is_ok()
        };
        assert!(parses(&["run", "--bazel-option=--jobs=4", "//a"]));
        assert!(parses(&["run", "--bazel-option", "--config=ci", "//a"]));
        for command in ["build", "test", "deploy", "watch", "lint"] {
            assert!(!parses(&[command, "--bazel-option=--jobs=4"]), "{command}");
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
            vec!["coverage", "--min-changed-line-coverage=eighty"],
            vec!["coverage", "--min-changed-line-coverage=101"],
            vec!["coverage", "--min-changed-line-coverage=-1"],
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
            vec!["coverage", "--min-changed-line-coverage=80"],
            vec!["coverage", "--min-changed-line-coverage=0"],
            vec!["coverage", "--min-changed-line-coverage=100"],
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
            vec!["lint", "--diff"],
            vec!["lint", "-ojson"],
            vec!["security", "--output", "text"],
        ] {
            assert!(accepts(&words), "words: {words:?}");
        }
        for (words, kind) in [
            (
                vec!["build", "--diff"],
                clap::error::ErrorKind::UnknownArgument,
            ),
            (
                vec!["security", "--diff"],
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
    fn value_options_list_every_value_taking_flag() {
        let mut command = <Cli as clap::CommandFactory>::command();
        command.build();
        let mut stack = vec![&command];
        let mut seen = 0;
        while let Some(command) = stack.pop() {
            for arg in command.get_arguments() {
                let expected = takes_next_word(arg);
                if let Some(letter) = arg.get_short() {
                    seen += 1;
                    assert!(
                        short_flags().contains(&letter),
                        "-{letter} is in short_flags"
                    );
                    assert_eq!(
                        value_options().contains(format!("-{letter}").as_str()),
                        expected,
                        "-{letter} in {} must match value_options",
                        command.get_name()
                    );
                }
                if let Some(long) = arg.get_long() {
                    assert_eq!(
                        value_options().contains(format!("--{long}").as_str()),
                        expected,
                        "--{long} in {} must match value_options",
                        command.get_name()
                    );
                }
            }
            stack.extend(command.get_subcommands());
        }
        assert!(seen > 0);
        for option in ["--output", "-o", "--workspace", "--bazel-option"] {
            assert!(value_options().contains(option), "{option} takes a value");
        }
        for flag in ["-q", "-v", "--verbose"] {
            assert!(!value_options().contains(flag), "{flag} takes no value");
        }
    }

    #[test]
    fn short_cluster_mirrors_clap() {
        let word = |name: &str, value: Option<&str>, used_next: bool| {
            Some(OptionWord {
                name: name.to_owned(),
                value: value.map(str::to_owned),
                used_next,
            })
        };
        assert_eq!(option_at("-qojson", None), word("-o", Some("json"), false));
        assert_eq!(
            option_at("-vo", Some("json")),
            word("-o", Some("json"), true)
        );
        assert_eq!(option_at("-qvojson", None), word("-o", Some("json"), false));
        assert_eq!(option_at("-qo=json", None), word("-o", Some("json"), false));
        assert_eq!(option_at("-o=", None), word("-o", Some(""), false));
        assert_eq!(
            option_at("-o", Some("text")),
            word("-o", Some("text"), true)
        );
        assert_eq!(option_at("-qo", None), word("-o", None, false));
        assert_eq!(option_at("-qo", Some("--quiet")), word("-o", None, false));
        assert_eq!(option_at("-qo", Some("")), word("-o", None, false));
        assert_eq!(
            option_at("--output", Some("json")),
            word("--output", Some("json"), true)
        );
        assert_eq!(
            option_at("--output=json", Some("json")),
            word("--output", Some("json"), false)
        );
        assert_eq!(
            option_at("--output", Some("--quiet")),
            word("--output", None, false)
        );
        assert_eq!(
            option_at("--bazel-option", Some("--config=ci")),
            word("--bazel-option", Some("--config=ci"), true)
        );
        assert_eq!(
            option_at("--apply", Some("json")),
            word("--apply", None, false)
        );
        assert_eq!(option_at("-q", Some("json")), None);
        assert_eq!(option_at("-q=false", None), None);
        assert_eq!(option_at("-json", None), None);
        assert_eq!(option_at("-qv", Some("json")), None);
        assert_eq!(option_at("-", Some("json")), None);
        assert_eq!(option_at("--", Some("json")), None);
        assert_eq!(option_at("lint", None), None);
    }

    #[test]
    fn early_output_flag_matches_clap() {
        for words in [
            "-qojson",
            "-vo json",
            "-qo=json",
            "--output json",
            "--output=json",
            "-o",
            "--bazel-option --config=ci",
            "-q=false",
        ] {
            let verb = if words.contains("bazel-option") {
                "run"
            } else {
                "lint"
            };
            let mut args = vec!["dx", verb];
            args.extend(words.split(' '));
            let early = crate::args::parser::early_output_flag(&args[1..]);
            let expected = match Cli::try_parse_from(&args) {
                Ok(cli) => cli.verb.expect("verb").into_parts().1.output,
                Err(_) => None,
            };
            assert_eq!(early, expected, "words: {words}");
        }
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
