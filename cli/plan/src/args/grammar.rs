use std::collections::BTreeSet;
use std::ffi::OsString;

use clap::{Args, Parser, ValueEnum};

use super::command::COMMANDS;
use super::invocation::ReportRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
    Gemini,
    Copilot,
}

impl Agent {
    pub const ALL: [Self; 4] = [Self::Claude, Self::Codex, Self::Gemini, Self::Copilot];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Copilot => "copilot",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Ci {
    Github,
    Gitlab,
    None,
}

fn report_request(raw: &str) -> Result<ReportRequest, dx_error::DxError> {
    match raw.split_once('=') {
        Some((format, destination)) if !format.is_empty() && !destination.is_empty() => {
            Ok(ReportRequest {
                format: format.to_owned(),
                destination: destination.to_owned(),
            })
        }
        _ => Err(dx_error::DxError::usage(format!(
            "want <format>=<destination>, got {raw:?}"
        ))),
    }
}

fn non_empty(raw: &str) -> Result<String, dx_error::DxError> {
    if raw.is_empty() {
        Err(dx_error::DxError::usage("want a value".to_owned()))
    } else {
        Ok(raw.to_owned())
    }
}

fn percent(raw: &str) -> Result<u32, dx_error::DxError> {
    match raw.parse::<u32>() {
        Ok(value) if value <= 100 => Ok(value),
        _ => Err(dx_error::DxError::usage(format!("want 0-100, got {raw:?}"))),
    }
}

#[must_use]
pub fn fuzz_seconds(raw: &str) -> Option<u64> {
    let (digits, unit) = match raw.strip_suffix(['s', 'm', 'h']) {
        Some(digits) => (digits, raw.chars().last()),
        None => (raw, Some('s')),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let count: u64 = digits.parse().ok().filter(|count| *count > 0)?;
    let scale = match unit? {
        'm' => 60,
        'h' => 3600,
        _ => 1,
    };
    let max_seconds = u64::try_from(i32::MAX).ok()?;
    count
        .checked_mul(scale)
        .filter(|seconds| *seconds <= max_seconds)
}

fn fuzz_duration(raw: &str) -> Result<String, dx_error::DxError> {
    match fuzz_seconds(raw) {
        Some(_) => Ok(raw.to_owned()),
        None => Err(dx_error::DxError::usage(format!(
            "want a duration such as 90s, 5m or 1h, got {raw:?}"
        ))),
    }
}

fn usage_of(command: super::command::Command) -> String {
    let usage = command.meta().usage;
    usage.strip_prefix("Usage: ").unwrap_or(usage).to_owned()
}

fn after_help_of(command: super::command::Command) -> String {
    super::help::after_long_help(command)
}

#[derive(Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent on/off options, one per command line flag"
)]
pub(crate) struct Flags {
    pub(crate) workspace: Option<OsString>,
    pub(crate) quiet: bool,
    pub(crate) verbose: bool,
    pub(crate) color: Option<String>,
    pub(crate) output: Option<String>,
    pub(crate) diff: bool,
    pub(crate) poll: bool,
    pub(crate) path_only: bool,
    pub(crate) report: Vec<ReportRequest>,
    pub(crate) fail_on: Option<String>,
    pub(crate) build_slots: Option<String>,
    pub(crate) build_slots_wait: Option<String>,
    pub(crate) include_edits: bool,
    pub(crate) max_findings: Option<u32>,
    pub(crate) min_coverage: Option<u32>,
    pub(crate) min_changed_line_coverage: Option<u32>,
    pub(crate) apply: bool,
    pub(crate) unsafe_fixes: bool,
    pub(crate) online: bool,
    pub(crate) breaking_against: Option<String>,
    pub(crate) debug: bool,
    pub(crate) release: bool,
    pub(crate) agents: Vec<Agent>,
    pub(crate) ci: Option<Ci>,
    pub(crate) no_generate: bool,
    pub(crate) accept_license: Vec<String>,
    pub(crate) to: Option<String>,
    pub(crate) here: bool,
    pub(crate) affected: bool,
    pub(crate) since: Option<String>,
    pub(crate) base: Option<String>,
    pub(crate) lint_scope: Option<String>,
    pub(crate) serve: bool,
    pub(crate) port: Option<u16>,
    pub(crate) host: Option<String>,
    pub(crate) open: bool,
    pub(crate) offline: bool,
    pub(crate) vendor: Option<String>,
    pub(crate) frozen: bool,
    pub(crate) outdated: bool,
    pub(crate) prune: bool,
    pub(crate) clean_bazel: bool,
    pub(crate) expunge: bool,
    pub(crate) add_missing: bool,
    pub(crate) pr: bool,
    pub(crate) automerge: Option<String>,
    pub(crate) min_age: Option<u32>,
    pub(crate) allow_major: bool,
    pub(crate) max_prs: Option<u16>,
    pub(crate) floor: bool,
    pub(crate) reason: Option<String>,
    pub(crate) accept: Vec<String>,
    pub(crate) expires: Option<String>,
    pub(crate) verify: Option<String>,
    pub(crate) repo: Option<String>,
    pub(crate) bundle: Option<String>,
    pub(crate) cases: bool,
    pub(crate) create: Option<String>,
    pub(crate) force: bool,
    pub(crate) disk: bool,
    pub(crate) bases: bool,
    pub(crate) capabilities: bool,
    pub(crate) files: bool,
    pub(crate) timings: bool,
    pub(crate) no_retry: bool,
    pub(crate) coverage: bool,
    pub(crate) budget: Option<String>,
    pub(crate) jobs: Option<u16>,
    pub(crate) max_regression: Option<u32>,
    pub(crate) compare: Option<String>,
    pub(crate) bazel_startup_options: Vec<String>,
    pub(crate) bazel_options: Vec<String>,
}

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
            pub(crate) fn put(self, flags: &mut Flags) {
                flags.$field = self.$field;
            }
        }
    };
}

macro_rules! composite_group {
    ($(#[$group_attr:meta])* $name:ident, { $( $(#[$attr:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $(#[$group_attr])*
        #[derive(Args)]
        pub(crate) struct $name {
            $( $(#[$attr])* #[command(flatten)] pub(crate) $field: $ty, )*
        }

        impl $name {
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
    /// Hide summaries.
    QuietFlag, {
        #[arg(long, short = 'q', action = clap::ArgAction::SetTrue, overrides_with = "quiet")]
        quiet: bool,
    }
}

flag_group! {
    /// Show more logs.
    VerboseFlag, {
        #[arg(long, short = 'v', action = clap::ArgAction::SetTrue, overrides_with = "verbose")]
        verbose: bool,
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

flag_group! {
    /// Give up waiting for a build slot after this long; 0 fails at once.
    BuildSlotsWaitFlag, {
        #[arg(
            long = "build-slots-wait",
            value_name = "duration",
            overrides_with = "build_slots_wait"
        )]
        build_slots_wait: Option<String>,
    }
}

flag_group! {
    /// Fail when a result is slower than the baseline by more than this percent.
    MaxRegressionFlag, {
        #[arg(long = "max-regression", value_name = "PCT", overrides_with = "max_regression")]
        max_regression: Option<u32>,
    }
}

flag_group! {
    /// Compare with a dx-bench/v1 file, or with the dx-bench artifact of the latest run on a ref.
    CompareFlag, {
        #[arg(
            long,
            value_name = "PATH|REF",
            overrides_with = "compare"
        )]
        compare: Option<String>,
    }
}

composite_group! {
    CommonArgs, {
        workspace: WorkspaceFlag,
        quiet: QuietFlag,
        verbose: VerboseFlag,
        color: ColorFlag,
        build_slots: BuildSlotsFlag,
        build_slots_wait: BuildSlotsWaitFlag,
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
    /// Include full edits in JSON change records (needs --output json).
    IncludeEditsFlag, {
        #[arg(long = "include-edits", overrides_with = "include_edits")]
        include_edits: bool,
    }
}

flag_group! {
    /// Print at most this many findings per tool in JSON output (default 50, 0 for no limit).
    MaxFindingsFlag, {
        #[arg(long = "max-findings", value_name = "n", overrides_with = "max_findings")]
        max_findings: Option<u32>,
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
    /// With --apply or --diff, include fixes the tool marks unsafe.
    UnsafeFlag, {
        #[arg(long = "unsafe", overrides_with = "unsafe_fixes")]
        unsafe_fixes: bool,
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
    /// Also run buf breaking against the merge base with this ref (default origin/HEAD).
    BreakingAgainstFlag, {
        #[arg(
            long,
            value_name = "REF",
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "origin/HEAD",
            overrides_with = "breaking_against"
        )]
        breaking_against: Option<String>,
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
    /// Agent tools whose files init adds.
    AgentsFlag, {
        #[arg(long, value_enum, value_delimiter = ',', value_name = "LIST")]
        agents: Vec<Agent>,
    }
}

flag_group! {
    /// CI provider whose workflow init writes.
    CiFlag, {
        #[arg(long, value_enum, value_name = "PROVIDER", overrides_with = "ci")]
        ci: Option<Ci>,
    }
}

flag_group! {
    /// License ids `dx init` accepts in `MODULE.bazel`.
    AcceptLicenseFlag, {
        #[arg(long = "accept-license", value_name = "ID", value_parser = non_empty)]
        accept_license: Vec<String>,
    }
}

flag_group! {
    /// Skips the generate that `dx init --apply` ends with.
    NoGenerateFlag, {
        #[arg(long = "no-generate")]
        no_generate: bool,
    }
}

flag_group! {
    /// Version to pin; another version hands off to that version's dx.
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
    /// With --affected, compare against this ref instead of the default branch merge-base.
    SinceFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "since", value_name = "REF", value_parser = non_empty)]
        since: Option<String>,
    }
}

flag_group! {
    /// With --affected, take the merge-base with this branch instead of the default branch.
    BaseFlag, {
        #[arg(long, allow_negative_numbers = true, overrides_with = "base", conflicts_with = "since", value_name = "REF", value_parser = non_empty)]
        base: Option<String>,
    }
}

flag_group! {
    /// With --affected, lint the changed targets and their direct users (direct) or every reverse dependency (closure).
    LintScopeFlag, {
        #[arg(
            long,
            value_name = "SCOPE",
            overrides_with = "lint_scope",
            value_parser = clap::builder::PossibleValuesParser::new(["direct", "closure"])
        )]
        lint_scope: Option<String>,
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
    /// Never download; use only what dx fetch already fetched.
    BazelOfflineFlag, {
        #[arg(long, overrides_with = "offline")]
        offline: bool,
    }
}

flag_group! {
    /// Run bazel vendor into DIR (default vendor_src) instead of bazel fetch.
    VendorFlag, {
        #[arg(
            long,
            value_name = "DIR",
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "vendor_src",
            overrides_with = "vendor",
            value_parser = non_empty
        )]
        vendor: Option<String>,
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
    /// Also run `bazel clean`.
    CleanBazelFlag, {
        #[arg(long = "bazel", overrides_with = "clean_bazel")]
        clean_bazel: bool,
    }
}

flag_group! {
    /// Also run `bazel clean --expunge`.
    ExpungeFlag, {
        #[arg(long, overrides_with = "expunge")]
        expunge: bool,
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
    /// Declare imported but undeclared dependencies at their latest version (needs --apply).
    AddMissingFlag, {
        #[arg(long = "add-missing", overrides_with = "add_missing")]
        add_missing: bool,
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
    /// Auto-merge level: none, patch, minor or all (needs --pr; default patch).
    AutomergeFlag, {
        #[arg(
            long,
            value_name = "LEVEL",
            overrides_with = "automerge",
            value_parser = clap::builder::PossibleValuesParser::new(["none", "patch", "minor", "all"])
        )]
        automerge: Option<String>,
    }
}

flag_group! {
    /// Skip versions published fewer than this many days ago (needs --pr; default 3; max 36500).
    MinAgeFlag, {
        #[arg(
            long,
            value_name = "DAYS",
            overrides_with = "min_age",
            value_parser = clap::value_parser!(u32).range(..=36_500)
        )]
        min_age: Option<u32>,
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
    /// Open at most this many dx-update pull requests (needs --pr; default 5).
    MaxPrsFlag, {
        #[arg(
            long,
            value_name = "N",
            overrides_with = "max_prs",
            value_parser = clap::value_parser!(u16).range(1..)
        )]
        max_prs: Option<u16>,
    }
}

flag_group! {
    /// Edit the `bazel_dep` floor instead of the tested override (needs a version and --reason).
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

flag_group! {
    /// Append an exception for a finding (needs --reason and --expires).
    AcceptFlag, {
        #[arg(long, value_name = "FINDING", action = clap::ArgAction::Append, value_parser = non_empty)]
        accept: Vec<String>,
    }
}

flag_group! {
    /// Why the accepted finding is allowed (needs --accept).
    AcceptReasonFlag, {
        #[arg(long, value_name = "TEXT", overrides_with = "reason", value_parser = non_empty)]
        reason: Option<String>,
    }
}

flag_group! {
    /// The date the accepted finding must be reviewed again, YYYY-MM-DD (needs --accept).
    ExpiresFlag, {
        #[arg(long, value_name = "YYYY-MM-DD", overrides_with = "expires", value_parser = non_empty)]
        expires: Option<String>,
    }
}

composite_group! {
    AcceptArgs, {
        accept: AcceptFlag,
        reason: AcceptReasonFlag,
        expires: ExpiresFlag,
    }
}

flag_group! {
    /// Verify a downloaded asset against its Sigstore bundle and attestation (needs --repo).
    VerifyFlag, {
        #[arg(long, value_name = "FILE", overrides_with = "verify", value_parser = non_empty)]
        verify: Option<String>,
    }
}

flag_group! {
    /// The GitHub repository that must have signed the asset, OWNER/NAME (needs --verify).
    RepoFlag, {
        #[arg(long, value_name = "OWNER/NAME", overrides_with = "repo", value_parser = non_empty)]
        repo: Option<String>,
    }
}

flag_group! {
    /// The Sigstore bundle, default <FILE>.sigstore.json (needs --verify).
    BundleFlag, {
        #[arg(long, value_name = "FILE", overrides_with = "bundle", value_parser = non_empty)]
        bundle: Option<String>,
    }
}

composite_group! {
    VerifyArgs, {
        verify: VerifyFlag,
        repo: RepoFlag,
        bundle: BundleFlag,
    }
}

composite_group! {
    FloorArgs, {
        floor: FloorFlag,
        reason: ReasonFlag,
    }
}

flag_group! {
    /// Start a new branch from this ref (add only).
    CreateFlag, {
        #[arg(long, value_name = "REF", value_parser = non_empty, overrides_with = "create")]
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
    /// Measure disk use per slot and mark slots past `DX_WORKTREE_MAX_SLOTS` prunable (list only, slow).
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
    /// Add one record per file dx init writes (status only).
    FilesFlag, {
        #[arg(long, overrides_with = "files")]
        files: bool,
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
    BuildModeArgs, {
        debug: DebugFlag,
        release: ReleaseFlag,
    }
}

composite_group! {
    ServeArgs, {
        serve: ServeFlag,
        port: PortFlag,
        host: HostFlag,
        open: OpenFlag,
    }
}

flag_group! {
    /// Do not rerun Bazel after a remote cache error.
    NoRetryFlag, {
        #[arg(long = "no-retry", overrides_with = "no_retry")]
        no_retry: bool,
    }
}

flag_group! {
    /// With --affected, run the tests that take coverage under coverage.
    CoverageFlag, {
        #[arg(long, overrides_with = "coverage")]
        coverage: bool,
    }
}

flag_group! {
    /// Print where build time went.
    TimingsFlag, {
        #[arg(long = "timings", overrides_with = "timings")]
        timings: bool,
    }
}

flag_group! {
    /// Time budget per target, such as 90s, 5m or 1h (default 60s).
    BudgetFlag, {
        #[arg(long, value_name = "DURATION", overrides_with = "budget", value_parser = fuzz_duration)]
        budget: Option<String>,
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

composite_group! {
    FuzzArgs, {
        budget: BudgetFlag,
        jobs: JobsFlag,
    }
}

#[derive(Args)]
pub(crate) struct BazelTail {
    /// Scopes to run on.
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(last = true, value_name = "BAZEL_OPTIONS")]
    pub(crate) passthrough: Vec<String>,
}

impl BazelTail {
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
    }
}

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
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
    }
}

#[derive(Args)]
pub(crate) struct NoPassthroughTail {
    /// Scopes to run on.
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(hide = true, last = true)]
    pub(crate) passthrough: Vec<String>,
}

impl NoPassthroughTail {
    pub(crate) fn split(self) -> (Vec<OsString>, Vec<String>) {
        (self.targets, self.passthrough)
    }
}

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
                    override_usage = usage_of(super::command::Command::$variant),
                    after_help = after_help_of(super::command::Command::$variant),
                )]
                $variant($variant),
            )*
        }

        impl Verb {
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
    Security => 0, NoPassthroughTail, {
        output: OutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        accept: AcceptArgs,
        verify: VerifyArgs,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    License => 1, NoPassthroughTail, {
        output: OutputFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        fail_on: FailOnFlag,
        accept: AcceptArgs,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    Lint => 2, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        max_findings: MaxFindingsFlag,
        apply: ApplyFlag,
        unsafe_fixes: UnsafeFlag,
        online: OnlineFlag,
        breaking_against: BreakingAgainstFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
        lint_scope: LintScopeFlag,
    };
    Typecheck => 3, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        max_findings: MaxFindingsFlag,
        apply: ApplyFlag,
        unsafe_fixes: UnsafeFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    Format => 4, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        max_findings: MaxFindingsFlag,
        apply: ApplyFlag,
        unsafe_fixes: UnsafeFlag,
        fail_on: FailOnFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    Generate => 5, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        apply: ApplyFlag,
        here: HereFlag,
        affected: AffectedFlag,
    };
    Build => 6, BazelTail, {
        output: OutputFlag,
        offline: BazelOfflineFlag,
        own: BuildModeArgs,
        timings: TimingsFlag,
        no_retry: NoRetryFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    Test => 7, BazelTail, {
        output: OutputFlag,
        offline: BazelOfflineFlag,
        own: BuildModeArgs,
        timings: TimingsFlag,
        no_retry: NoRetryFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    #[expect(clippy::struct_field_names, reason = "the fields are the clap argument ids")]
    Coverage => 8, BazelTail, {
        output: OutputFlag,
        fail_on: FailOnFlag,
        min_coverage: MinCoverageFlag,
        min_changed_line_coverage: MinChangedLineCoverageFlag,
        timings: TimingsFlag,
        no_retry: NoRetryFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    Bench => 9, BazelTail, {
        output: OutputFlag,
        max_regression: MaxRegressionFlag,
        compare: CompareFlag,
        here: HereFlag,
    };
    Run => 10, AppTail, {
        output: OutputFlag,
        own: BuildModeArgs,
        timings: TimingsFlag,
        bazel_option: BazelOptionFlag,
    };
    Deploy => 11, AppTail, {
        own: BuildModeArgs,
        apply: ApplyFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
    Check => 12, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        max_findings: MaxFindingsFlag,
        apply: ApplyFlag,
        unsafe_fixes: UnsafeFlag,
        fail_on: FailOnFlag,
        no_retry: NoRetryFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
        coverage: CoverageFlag,
        lint_scope: LintScopeFlag,
    };
    #[expect(clippy::struct_field_names, reason = "the fields are the clap argument ids")]
    Clean => 13, NoScopeTail, {
        output: OutputFlag,
        clean_bazel: CleanBazelFlag,
        expunge: ExpungeFlag,
    };
    Update => 14, NoPassthroughTail, {
        output: OutputFlag,
        fail_on: FailOnFlag,
        offline: OfflineFlag,
        frozen: FrozenFlag,
        apply: ApplyFlag,
        outdated: OutdatedFlag,
        prune: PruneFlag,
        add_missing: AddMissingFlag,
        pr: PrFlag,
        automerge: AutomergeFlag,
        min_age: MinAgeFlag,
        allow_major: AllowMajorFlag,
        max_prs: MaxPrsFlag,
        floor: FloorArgs,
    };
    Sync => 15, BazelTail, { output: OutputFlag, };
    Init => 16, NoPassthroughTail, {
        output: OutputFlag,
        apply: ApplyFlag,
        agents: AgentsFlag,
        ci: CiFlag,
        no_generate: NoGenerateFlag,
        accept_license: AcceptLicenseFlag,
    };
    Upgrade => 17, NoScopeTail, { output: OutputFlag, apply: ApplyFlag, own: ToFlag, };
    Status => 18, NoScopeTail, {
        output: OutputFlag,
        capabilities: CapabilitiesFlag,
        files: FilesFlag,
    };
    Watch => 19, BazelTail, {
        output: OutputFlag,
        diff: DiffFlag,
        include_edits: IncludeEditsFlag,
        max_findings: MaxFindingsFlag,
        apply: ApplyFlag,
        unsafe_fixes: UnsafeFlag,
        fail_on: FailOnFlag,
        report: ReportFlag,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
        poll: PollFlag,
        own: BuildModeArgs,
    };
    Completion => 20, NoPassthroughTail, {};
    Docs => 21, BazelTail, {
        output: OutputFlag,
        here: HereFlag,
        own: ServeArgs,
    };
    Worktree => 22, NoPassthroughTail, {
        output: OutputFlag,
        path_only: PathOnlyFlag,
        apply: ApplyFlag,
        create: CreateFlag,
        force: ForceFlag,
        disk: DiskFlag,
        bases: BasesFlag,
    };
    Fuzz => 23, NoPassthroughTail, { output: OutputFlag, own: FuzzArgs, here: HereFlag, };
    Config => 24, NoPassthroughTail, { output: OutputFlag, };
    Fetch => 25, BazelTail, {
        output: OutputFlag,
        vendor: VendorFlag,
        own: BuildModeArgs,
        here: HereFlag,
        affected: AffectedFlag,
        since: SinceFlag,
        base: BaseFlag,
    };
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

#[must_use]
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

pub fn man_page() -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    clap_mangen::Man::new(cli_command())
        .section("1")
        .render(&mut buffer)?;
    Ok(buffer)
}

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

fn takes_next_word(arg: &clap::Arg) -> bool {
    !arg.is_require_equals_set()
        && arg.get_action().takes_values()
        && arg
            .get_num_args()
            .is_some_and(|range| range.min_values() >= 1)
}

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

fn collect_shorts(command: &clap::Command, letters: &mut Vec<char>) {
    letters.extend(command.get_arguments().filter_map(clap::Arg::get_short));
    for sub in command.get_subcommands() {
        collect_shorts(sub, letters);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OptionWord {
    pub(crate) name: String,
    pub(crate) value: Option<String>,
    pub(crate) used_next: bool,
}

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

#[must_use]
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

#[must_use]
pub fn accepts_long(command: super::command::Command, long: &str) -> bool {
    subcommand_args(command)
        .get_arguments()
        .any(|arg| arg.get_long() == Some(long))
}

#[must_use]
pub fn advertises_passthrough(command: super::command::Command) -> bool {
    subcommand_args(command)
        .get_arguments()
        .any(|arg| arg.is_last_set() && !arg.is_hide_set())
}

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
    fn fuzz_seconds_rejects_budgets_the_engines_cannot_use() {
        assert_eq!(fuzz_seconds("90"), Some(90));
        assert_eq!(fuzz_seconds("5m"), Some(300));
        assert_eq!(fuzz_seconds("1h"), Some(3600));
        assert_eq!(fuzz_seconds("2147483647"), Some(2_147_483_647));
        assert_eq!(fuzz_seconds("2147483648"), None);
        assert_eq!(fuzz_seconds("3000000000"), None);
        assert_eq!(fuzz_seconds("10000000000"), None);
        assert_eq!(fuzz_seconds("35791395m"), None);
        assert_eq!(fuzz_seconds("+5m"), None);
        assert_eq!(fuzz_seconds("+90"), None);
        assert_eq!(fuzz_seconds("-5m"), None);
        assert_eq!(fuzz_seconds("m"), None);
        assert_eq!(fuzz_seconds("0s"), None);
    }

    #[test]
    fn every_command_names_its_own_registry_entry() {
        let root = cli_command();
        for entry in &COMMANDS {
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
