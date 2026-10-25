#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Command {
    Security,
    License,
    Lint,
    Typecheck,
    Format,
    Generate,
    Build,
    Test,
    Coverage,
    Run,
    Deploy,
    Check,
    Fix,
    Clean,
    Update,
    Bump,
    Migrate,
    Codegen,
    Env,
    Init,
    New,
    Upgrade,
    Status,
    Version,
    Watch,
    Completion,
    Docs,
    ListTests,
    Rollback,
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstSlot {
    None,
    UpdateSets,
    BumpSets,
    NewLanguages,
    WatchTasks,
    CompletionShells,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelsPolicy {
    Always,
    Never,
    OnlyEmpty,
    OnlyNonEmpty,
    FewerThanTwo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkewKind {
    Proceed,
    Warn,
    Refuse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowVerb {
    Build,
    Test,
    Coverage,
    Run,
}

impl WorkflowVerb {
    pub fn name(self) -> &'static str {
        match self {
            WorkflowVerb::Build => "build",
            WorkflowVerb::Test => "test",
            WorkflowVerb::Coverage => "coverage",
            WorkflowVerb::Run => "run",
        }
    }

    pub fn collects_reports(self) -> bool {
        matches!(self, WorkflowVerb::Test | WorkflowVerb::Coverage)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMeta {
    pub command: Command,
    pub name: &'static str,
    pub scope_policy: &'static str,
    pub describe: &'static str,
    pub usage: &'static str,
    pub flags: &'static str,
    pub scopes: &'static str,
    pub is_audit_update: bool,
    pub is_managed: bool,
    pub is_adoption: bool,
    pub supports_json: bool,
    pub supports_diff: bool,
    pub supports_here: bool,
    pub supports_offline: bool,
    pub supports_frozen: bool,
    pub supports_check: bool,
    pub supports_fail_on: bool,
    pub supports_min_coverage: bool,
    pub supports_profile: bool,
    pub is_mutating_by_default: bool,
    pub default_release: bool,
    pub skew: SkewKind,
    pub workflow_verb: Option<WorkflowVerb>,
    pub first_slot: FirstSlot,
    pub labels: LabelsPolicy,
}

pub static COMMANDS: [CommandMeta; 30] = [
    CommandMeta {
        command: Command::Security,
        name: "security",
        scope_policy: "default-//...",
        describe: "run security audit over resolved scopes (non-mutating; live Gitleaks plus advisory/vuln backends)",
        usage: "Usage: dx security [--offline] [--frozen] [--here] [--affected] [scope ...]",
        flags: "Per-command flags: --offline (no network; pinned cached snapshots or fail), --frozen (no manifest or lock resolution changes), --fail-on info|warning|error, --report sarif (security/license only; --check and `-- --bazel-options` do not apply; --output diff has no patch).",
        scopes: "Scopes: dependency-set/package/target selectors; bare run audits //... (secrets plus vulnerabilities). Pass --here for the current directory tree instead; --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: true,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: false,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: true,
        supports_frozen: true,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Warn,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::License,
        name: "license",
        scope_policy: "default-//...",
        describe: "run license audit over resolved scopes (non-mutating; live license-policy plus SPDX backend)",
        usage: "Usage: dx license [--offline] [--frozen] [--here] [--affected] [scope ...]",
        flags: "Per-command flags: --offline (no network; pinned cached snapshots or fail), --frozen (no manifest or lock resolution changes), --fail-on info|warning|error, --report sarif|spdx (security/license only; --check and `-- --bazel-options` do not apply; --output diff has no patch).",
        scopes: "Scopes: dependency-set/package/target selectors; bare run audits //... (license policy plus SPDX inventory). Pass --here for the current directory tree instead; --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: true,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: false,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: true,
        supports_frozen: true,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Warn,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Lint,
        name: "lint",
        scope_policy: "default-//...",
        describe: "run lint analysis over resolved scopes (check by default; --apply writes fixes)",
        usage: "Usage: dx lint|typecheck|format [--check] [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check/--fail-on/--report (quality only; --here for cwd scope; --output text|diff|json). --apply authorizes writing fixes. --include-edits adds full edits to JSON change records (--output json only).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_check: true,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Typecheck,
        name: "typecheck",
        scope_policy: "default-//...",
        describe: "run typecheck analysis over resolved scopes (check by default; --apply writes fixes)",
        usage: "Usage: dx lint|typecheck|format [--check] [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check/--fail-on/--report (quality only; --here for cwd scope; --output text|diff|json). --apply authorizes writing fixes. --include-edits adds full edits to JSON change records (--output json only).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_check: true,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Format,
        name: "format",
        scope_policy: "default-//...",
        describe: "check or rewrite formatting over resolved scopes (check by default; --apply writes fixes)",
        usage: "Usage: dx lint|typecheck|format [--check] [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check/--fail-on/--report (quality only; --here for cwd scope; --output text|diff|json). --apply authorizes writing fixes. --include-edits adds full edits to JSON change records (--output json only).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_check: true,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Generate,
        name: "generate",
        scope_policy: "default-repo",
        describe: "update BUILD declarations from source with Gazelle (check by default; --apply writes)",
        usage: "Usage: dx generate [--check] [--apply] [--here] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check (--fail-on and --report do not apply; --here for cwd scope; --output text|diff|json). --apply writes the updated BUILD files.",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Build,
        name: "build",
        scope_policy: "default-//...",
        describe: "run Bazel build over resolved targets",
        usage: "Usage: dx build [--here] [--affected] [--debug|--release] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --debug | --release (build/run/test/deploy only; mutually exclusive; bare invocation means dev, except deploy means release).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: true,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: Some(WorkflowVerb::Build),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Test,
        name: "test",
        scope_policy: "default-//...",
        describe: "run Bazel test over resolved targets",
        usage: "Usage: dx test [--here] [--affected] [--debug|--release] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --debug | --release (build/run/test/deploy only; mutually exclusive; bare invocation means dev, except deploy means release).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: true,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: Some(WorkflowVerb::Test),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Coverage,
        name: "coverage",
        scope_policy: "default-//...",
        describe: "collect LCOV coverage with optional threshold",
        usage: "Usage: dx coverage [--here] [--affected] [--min-coverage 0-100] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --min-coverage <0-100> (coverage only; collects without enforcing when absent).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: true,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: Some(WorkflowVerb::Coverage),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Run,
        name: "run",
        scope_policy: "require",
        describe: "build and run runnable targets sequentially (explicit labels/patterns; file/dir scopes need exactly one runnable)",
        usage: "Usage: dx run [--debug|--release] <label> ... [-- app-args ...]",
        flags: "Per-command flags: --debug | --release (build/run/test/deploy only; mutually exclusive; bare invocation means dev, except deploy means release).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Requires a scope (empty scope is a usage error); file/dir scopes need exactly one runnable.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: true,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: Some(WorkflowVerb::Run),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Deploy,
        name: "deploy",
        scope_policy: "require-label",
        describe: "build and run a single deployable target",
        usage: "Usage: dx deploy [--apply] [--debug|--release] <label> [-- app-args ...]",
        flags: "Per-command flags: --apply (authorizes the deployment). --debug | --release (build/run/test/deploy only; mutually exclusive; bare invocation means dev, except deploy means release).",
        scopes: "Scopes: exactly one main-workspace label (//pkg:target); patterns (//...), multiple labels, and file/path scopes are usage failures.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: true,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: true,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::OnlyEmpty,
    },
    CommandMeta {
        command: Command::Check,
        name: "check",
        scope_policy: "default-//...",
        describe: "run format+lint+typecheck+generate checks in order (non-mutating)",
        usage: "Usage: dx check|fix [--check] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check/--fail-on/--report pass through per phase (check only; non-mutating umbrella over format+lint+typecheck+generate; later phases still run after ordinary findings). --include-edits adds full edits to JSON change records (--output json only).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_check: true,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Warn,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Fix,
        name: "fix",
        scope_policy: "default-//...",
        describe: "apply format+lint+typecheck+generate fixes in order (check by default; --apply writes fixes, then verifies with a read-only check)",
        usage: "Usage: dx check|fix [--check] [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check/--fail-on/--report pass through per phase (fix only; check by default, --apply writes fixes then verifies with a read-only check). --apply authorizes the fix run to write sources. --include-edits adds full edits to JSON change records (--output json only).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_check: true,
        supports_fail_on: true,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Clean,
        name: "clean",
        scope_policy: "reject",
        describe: "prune unselected managed state, never Bazel outputs; use `bazel clean` for those (check by default; --apply prunes; no scopes)",
        usage: "Usage: dx clean [--check] [--apply] [--prune-unobserved]",
        flags: "Per-command flags: --check (report prune candidates without deleting; default). --apply (authorizes pruning managed state). Never touches Bazel outputs; --output text|json only, diff has no patch. --prune-unobserved (prune generations no process observation protects; without it, unavailable observation preserves unobserved state instead of assuming it idle; never overrides a held lease).",
        scopes: "Scopes: none (clean takes no scopes).",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Update,
        name: "update",
        scope_policy: "selector-default-all",
        describe: "update dependencies per set through qualified resolvers (check by default; --apply writes updates)",
        usage: "Usage: dx update [--check] [--apply] [--offline] [--frozen] [selector ...]",
        flags: "Per-command flags: --offline (no network; pinned cached snapshots or fail), --frozen (no manifest or lock resolution changes), --check (validates selected sets; writes nothing), --apply (authorizes the update) (update only; --fail-on/--report and `-- --bazel-options` do not apply; --output diff has no patch).",
        scopes: "Scopes: dependency-set/package/target selectors (cargo|go|maven|npm|npm-adopt|npm-adopt-polyglot|npm-tools|nuget|powershell|ruby|uv|uv-adopt|uv-adopt-polyglot|uv-tools, set:package, labels/paths); bare run checks all sets.",
        is_audit_update: true,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: true,
        supports_frozen: true,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::UpdateSets,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Bump,
        name: "bump",
        scope_policy: "require-selector+version",
        describe: "widen one declared requirement to a new version (check by default; --apply widens)",
        usage: "Usage: dx bump [--apply] [--offline] [--frozen] <set:package> <version>",
        flags: "Per-command flags: --offline (no network; pinned cached snapshots or fail), --frozen (no manifest or lock resolution changes), --apply (authorizes the bump; exactly one `set:package` plus version; --check/--fail-on/--report and `-- --bazel-options` do not apply; --output diff has no patch).",
        scopes: "Scopes: exactly one `set:package` plus one new version (bazel|cargo|github-actions|go|maven|npm|nuget); never batch.",
        is_audit_update: true,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: true,
        supports_frozen: true,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::BumpSets,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Migrate,
        name: "migrate",
        scope_policy: "default-//...",
        describe: "rewrite breaking changes across releases (upgrade-only; check by default; --apply migrates)",
        usage: "Usage: dx migrate [--apply] --from <version> --to <version> [scope ...]",
        flags: "Per-command flags: --from <version> --to <version> (migrate only; both Cargo semver, upgrade-only gate). --apply authorizes the migration.",
        scopes: "Scopes: explicit Bazel labels/patterns or workspace-relative files/dirs reusing generation scope resolution; external scopes rejected. No scope selects //....",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Codegen,
        name: "codegen",
        scope_policy: "default-repo|exact-label",
        describe: "materialize declared generated source (check by default; --apply writes)",
        usage: "Usage: dx codegen|env [--check] [--apply] [<label>] [-- bazel-options ...]",
        flags: "Per-command flags: --check (validate the intended selection without writing; default). --apply (authorizes writing managed state; repository-wide or one exact // or @ label; --fail-on/--report/--output diff and version/clean/inspect/migrate flags do not apply; --output text|json only).",
        scopes: "Scopes: none for repository-wide canonical selection, or exactly one exact // or @ label; patterns, paths, and multiple labels are usage failures (see docs/cli/commands/environment-codegen.md).",
        is_audit_update: false,
        is_managed: true,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::OnlyEmpty,
    },
    CommandMeta {
        command: Command::Env,
        name: "env",
        scope_policy: "default-repo|exact-label",
        describe: "collect the managed development environment (check by default; --apply writes managed state)",
        usage: "Usage: dx codegen|env [--check] [--apply] [<label>] [-- bazel-options ...]",
        flags: "Per-command flags: --check (validate the intended selection without writing; default). --apply (authorizes writing managed state; repository-wide or one exact // or @ label; --fail-on/--report/--output diff and version/clean/inspect/migrate flags do not apply; --output text|json only).",
        scopes: "Scopes: none for repository-wide canonical selection, or exactly one exact // or @ label; patterns, paths, and multiple labels are usage failures (see docs/cli/commands/environment-codegen.md).",
        is_audit_update: false,
        is_managed: true,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::OnlyEmpty,
    },
    CommandMeta {
        command: Command::Init,
        name: "init",
        scope_policy: "optional-name",
        describe: "adopt dx into an existing tree (absent-only; check by default; --apply scaffolds)",
        usage: "Usage: dx init [--apply] [module-name]",
        flags: "Per-command flags: --apply (authorizes scaffolding; optional [module-name]; --check/--fail-on/--report/--output json|diff and `-- --bazel-options` do not apply).",
        scopes: "Scopes: optional single module name (defaults to my_project when absent); Bazel labels/patterns are not scopes; extra positionals are usage failures.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::New,
        name: "new",
        scope_policy: "require",
        describe: "start a new project for one language (absent-only; check by default; --apply scaffolds)",
        usage: "Usage: dx new [--apply] <language> [name]",
        flags: "Per-command flags: --apply (authorizes scaffolding; <language> [name]; {languages}; rust-web scaffolds a shared Rust core with native and browser targets; absent-only, no --force).",
        scopes: "Scopes: <language> plus optional project name (defaults to my_project); unknown languages fail with the supported list; extra positionals are usage failures.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::NewLanguages,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Upgrade,
        name: "upgrade",
        scope_policy: "reject",
        describe: "one-shot pin+migrate+env+codegen composition with recovery pointer (check by default; --apply upgrades)",
        usage: "Usage: dx upgrade [--apply] --from <version> --to <version>",
        flags: "Per-command flags: --from <version> --to <version> (upgrade only; pin+migrate+env+codegen composition with recovery pointer). --apply authorizes the upgrade.",
        scopes: "Scopes: none (repository-wide pin+migrate+env+codegen composition; --from/--to required, positional scopes rejected).",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Status,
        name: "status",
        scope_policy: "reject",
        describe: "report workspace status; --verbose adds command capabilities",
        usage: "Usage: dx status",
        flags: "Per-command flags: none (no scopes; --output text|json only, diff has no patch; --check/--fail-on/--report/--configured and `-- --bazel-options` do not apply; JSON streams command_started, one status event per check (name, status, detail, hint), with --verbose one capability event per command, optional status_pin_mismatch or status_check_failed error, command_finished; no dx doctor, use dx status, see docs/cli/commands/status-version.md#failure-explainer).",
        scopes: "Scopes: none (status takes no scopes).",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Version,
        name: "version",
        scope_policy: "reject",
        describe: "report version and pin drift",
        usage: "Usage: dx version [--check]",
        flags: "Per-command flags: --check (drift check; version never writes, use dx upgrade or dx rollback to change the pin; --output text|json only, diff has no patch; JSON reuses the status envelope).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied; other commands follow per-command defaults (see docs/cli/commands/scope-defaults.md).",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Watch,
        name: "watch",
        scope_policy: "require",
        describe: "watch for changes and rebuild (local only)",
        usage: "Usage: dx watch [--apply] [--debug|--release] [flags] <build|test|run|lint|typecheck|format|check|fix> [scope ...] [-- bazel-options|--app-args ...]",
        flags: "Per-command flags: any flag of the wrapped command (--output, --check, --apply, --fail-on, --report, --debug | --release); a flag the wrapped command rejects is rejected here. watch only wraps build|test|run|lint|typecheck|format|check|fix; local only, refuses CI; watch run stops the running app before relaunching it; Ctrl-C stops it with exit 130.",
        scopes: "Scopes: wrapped command plus its scopes, re-resolved each iteration (local only, refuses CI=true; only build|test|run|lint|typecheck|format|check|fix are watchable). Args after `--` reach the wrapped command as its own command-options.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: true,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: true,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::WatchTasks,
        labels: LabelsPolicy::OnlyNonEmpty,
    },
    CommandMeta {
        command: Command::Completion,
        name: "completion",
        scope_policy: "require",
        describe: "emit shell completions from the CLI grammar (bash|zsh|fish|powershell)",
        usage: "Usage: dx completion <shell> bash|zsh|fish|powershell",
        flags: "Per-command flags: exactly one <shell> bash|zsh|fish|powershell; unknown shells fail with unknown-shell; --check, --output json|diff and `-- --bazel-options` do not apply.",
        scopes: "Scopes: exactly one shell (bash|zsh|fish|powershell); unknown shells fail with unknown-shell.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::CompletionShells,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Docs,
        name: "docs",
        scope_policy: "default-repo",
        describe: "build, check, and serve the mdBook documentation site (non-mutating; --check validates without rendering)",
        usage: "Usage: dx docs [--check] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --check/--serve/--port/--host/--open (docs only; `-- --bazel-options` pass through to the docs build; --check validates the book without rendering it, --serve previews the last build locally, --port/--host/--open require --serve; --output text|json only, diff has no patch).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Bare scope selects the repository docs site (//tools/docs/site:user_site, //tools/docs/site:user_site_check in --check). Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: true,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::ListTests,
        name: "list-tests",
        scope_policy: "default-//...",
        describe: "list configured test targets without building or running them (non-mutating; --cases builds and runs supported test binaries to list cases)",
        usage: "Usage: dx list-tests [--cases] [--configured] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        flags: "Per-command flags: --cases (build and run supported Rust test binaries with --list to enumerate cases; other frameworks report unsupported). --configured (use `bazel cquery` instead of `bazel query`; --output text|json only, diff has no patch; JSON reuses the status envelope with one status event per label or case).",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Warn,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Rollback,
        name: "rollback",
        scope_policy: "reject",
        describe: "restore the previous recorded pin (check by default; --apply restores)",
        usage: "Usage: dx rollback [--apply]",
        flags: "Per-command flags: --apply (restores the last recorded pin or removes the pin when none was recorded; refuses without a record or after manual pin edits; --output text|json only, diff has no patch; JSON reuses the status envelope).",
        scopes: "Scopes: none (rollback takes no scopes).",
        is_audit_update: false,
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Worktree,
        name: "worktree",
        scope_policy: "reject",
        describe: "lease warm checkout slots (add, release, list, path, prune; prune checks by default, --apply deletes)",
        usage: "Usage: dx worktree add <branch> [--create=<ref>] [--force] | release [<path>] [--force] | list [--disk] | path <branch> | prune [--apply] | hook create|remove|stop|guard",
        flags: "Per-command flags: --create=<ref> (add only; start a new branch from <ref>), --force (add and release; take or free a slot another holder holds, or discard uncommitted or unpushed work on release), --disk (list only; measure disk use per slot, slow), --apply (prune only; delete what prune lists), --output text|json|path (path prints only the path and works with add only; diff has no patch; JSON emits slot and summary events).",
        scopes: "Scopes: none (worktree takes a subcommand and its arguments, not scopes).",
        is_audit_update: false,
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_check: false,
        supports_fail_on: false,
        supports_min_coverage: false,
        supports_profile: false,
        supports_offline: false,
        supports_frozen: false,
        is_mutating_by_default: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
];

impl Command {
    pub fn meta(self) -> &'static CommandMeta {
        let entry = &COMMANDS[self as usize];
        debug_assert_eq!(entry.command, self);
        entry
    }

    pub fn pipe_list() -> String {
        use clap::ValueEnum;
        Self::value_variants()
            .iter()
            .map(|command| command.name())
            .collect::<Vec<_>>()
            .join("|")
    }

    pub fn scope_policy(self) -> &'static str {
        self.meta().scope_policy
    }

    pub fn name(self) -> &'static str {
        self.meta().name
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        use clap::ValueEnum;
        Self::from_str(text, false).ok()
    }

    pub fn workflow_verb(self) -> Option<WorkflowVerb> {
        self.meta().workflow_verb
    }

    pub fn is_audit_update(self) -> bool {
        self.meta().is_audit_update
    }

    pub fn is_managed(self) -> bool {
        self.meta().is_managed
    }

    pub fn is_adoption(self) -> bool {
        self.meta().is_adoption
    }

    pub fn supports_json(self) -> bool {
        self.meta().supports_json
    }

    pub fn supports_diff(self) -> bool {
        self.meta().supports_diff
    }

    pub fn supports_affected(self) -> bool {
        self.meta().supports_here && !matches!(self, Command::Generate | Command::Docs)
    }

    pub fn supports_here(self) -> bool {
        self.meta().supports_here
    }

    pub fn supports_offline(self) -> bool {
        self.meta().supports_offline
    }

    pub fn supports_frozen(self) -> bool {
        self.meta().supports_frozen
    }

    pub fn supports_check(self) -> bool {
        self.meta().supports_check
    }

    /// Whether `dx <command> --apply` parses: the command can change managed
    /// state or launch an effect, so only explicit `--apply` authorizes it.
    pub fn supports_apply(self) -> bool {
        matches!(
            self,
            Command::Lint
                | Command::Typecheck
                | Command::Format
                | Command::Generate
                | Command::Deploy
                | Command::Fix
                | Command::Clean
                | Command::Update
                | Command::Bump
                | Command::Migrate
                | Command::Codegen
                | Command::Env
                | Command::Init
                | Command::New
                | Command::Upgrade
                | Command::Watch
                | Command::Rollback
                | Command::Worktree
        )
    }

    pub fn supports_fail_on(self) -> bool {
        self.meta().supports_fail_on
    }

    pub fn supports_min_coverage(self) -> bool {
        self.meta().supports_min_coverage
    }

    pub fn supports_cases(self) -> bool {
        matches!(self, Command::ListTests)
    }

    pub fn supports_profile(self) -> bool {
        self.meta().supports_profile
    }

    pub fn is_mutating_by_default(self) -> bool {
        self.meta().is_mutating_by_default
    }

    pub fn describe(self) -> &'static str {
        self.meta().describe
    }

    pub fn usage(self) -> &'static str {
        self.meta().usage
    }

    /// The leading positionals the usage line spells with angle brackets, or none when it names none.
    pub fn required_slot(self) -> Option<&'static str> {
        let rest = self.usage().strip_prefix("Usage: dx ")?.split_once(' ')?.1;
        let mut slot: Option<(usize, usize)> = None;
        let mut offset = 0;
        for word in rest.split(' ') {
            if let Some(at) = word
                .strip_prefix("[<")
                .filter(|_| !word.contains(">]"))
                .and_then(|inner| inner.find('>'))
            {
                slot = Some((offset + 1, offset + 3 + at));
                break;
            }
            if word.starts_with('<') && !word.contains(']') {
                let start = slot.map_or(offset, |(start, _)| start);
                slot = Some((start, offset + word.len()));
            } else if !(word.starts_with('[') && slot.is_none()) {
                break;
            }
            offset += word.len() + 1;
        }
        let (start, end) = slot?;
        let slot = rest.get(start..end)?.trim();
        (!slot.is_empty()).then_some(slot)
    }

    pub fn flags(self) -> &'static str {
        if self == Command::New {
            static NEW_FLAGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            return NEW_FLAGS.get_or_init(|| {
                self.meta()
                    .flags
                    .replace("{languages}", &dx_adopt::SUPPORTED_NEW_LANGUAGES.join("|"))
            });
        }
        self.meta().flags
    }

    pub fn scopes_text(self) -> &'static str {
        self.meta().scopes
    }

    pub(crate) fn allows_labels(self, prior_len: usize) -> bool {
        match self.meta().labels {
            LabelsPolicy::Always => true,
            LabelsPolicy::Never => false,
            LabelsPolicy::OnlyEmpty => prior_len == 0,
            LabelsPolicy::OnlyNonEmpty => prior_len > 0,
            LabelsPolicy::FewerThanTwo => prior_len < 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_are_stable() {
        assert_eq!(Command::Security.name(), "security");
        assert_eq!(Command::License.name(), "license");
        assert_eq!(Command::Update.name(), "update");
        assert_eq!(Command::Bump.name(), "bump");
        assert_eq!(Command::Migrate.name(), "migrate");
        assert_eq!(Command::New.name(), "new");
        assert_eq!(Command::Upgrade.name(), "upgrade");
        assert!(Command::Bump.is_audit_update());
        assert!(Command::Update.is_audit_update());
        assert!(!Command::Migrate.is_audit_update());
        assert!(!Command::Migrate.is_adoption());
        assert!(!Command::Migrate.is_managed());
        assert!(!Command::Migrate.is_mutating_by_default());
        assert!(Command::Migrate.supports_json());
        assert!(!Command::Migrate.supports_diff());
        assert!(Command::New.is_adoption());
        assert!(Command::Upgrade.is_adoption());
        assert!(!Command::New.is_mutating_by_default());
        assert!(!Command::Upgrade.is_mutating_by_default());
        assert!(!Command::New.supports_json());
        assert!(Command::Upgrade.supports_json());
        assert!(!Command::New.supports_diff());
        assert!(!Command::Upgrade.supports_diff());
        assert_eq!(Command::Lint.name(), "lint");
        assert_eq!(Command::Typecheck.name(), "typecheck");
        assert_eq!(Command::Format.name(), "format");
        assert_eq!(Command::Generate.name(), "generate");
        assert_eq!(Command::Build.name(), "build");
        assert_eq!(Command::Test.name(), "test");
        assert_eq!(Command::Coverage.name(), "coverage");
        assert_eq!(Command::Run.name(), "run");
        assert_eq!(Command::Codegen.name(), "codegen");
        assert_eq!(Command::Env.name(), "env");
        assert!(Command::Codegen.is_managed());
        assert!(Command::Env.is_managed());
        assert!(!Command::Codegen.is_adoption());
        assert!(!Command::Clean.is_managed());
        assert_eq!(Command::Docs.name(), "docs");
        assert!(!Command::Docs.is_adoption());
        assert!(!Command::Docs.is_managed());
        assert!(!Command::Docs.is_audit_update());
        assert!(!Command::Docs.is_mutating_by_default());
        assert!(Command::Docs.supports_json());
        assert!(!Command::Docs.supports_diff());
        assert!(Command::Docs.supports_here());
    }

    #[test]
    fn command_names_are_clap_value_enum() {
        use clap::ValueEnum;
        let commands = [
            Command::Security,
            Command::License,
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Generate,
            Command::Build,
            Command::Test,
            Command::Coverage,
            Command::Run,
            Command::Deploy,
            Command::Check,
            Command::Fix,
            Command::Clean,
            Command::Update,
            Command::Bump,
            Command::Migrate,
            Command::Codegen,
            Command::Env,
            Command::Init,
            Command::New,
            Command::Upgrade,
            Command::Status,
            Command::Version,
            Command::Watch,
            Command::Completion,
            Command::Docs,
            Command::ListTests,
            Command::Rollback,
            Command::Worktree,
        ];
        assert_eq!(commands.len(), Command::value_variants().len());
        for command in commands {
            assert_eq!(Command::parse(command.name()), Some(command));
            assert_eq!(
                command.to_possible_value().expect("named").get_name(),
                command.name()
            );
        }
        assert_eq!(Command::parse("Lint"), None);
        assert_eq!(Command::parse("type-check"), None);
        assert_eq!(Command::parse("dx"), None);
    }

    #[test]
    fn registry_names_round_trip_and_reject_excluded_commands() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            assert_eq!(Command::parse(command.name()), Some(*command));
        }
        for excluded in ["doctor", "configure", "bogus"] {
            assert_eq!(
                Command::parse(excluded),
                None,
                "{excluded} must stay rejected as unknown"
            );
        }
    }

    #[test]
    fn mutating_by_default_matches_contract_and_help() {
        for command in [
            Command::Security,
            Command::License,
            Command::Build,
            Command::Test,
            Command::Coverage,
            Command::Run,
            Command::Deploy,
            Command::Check,
            Command::Fix,
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Generate,
            Command::Codegen,
            Command::Env,
            Command::Clean,
            Command::Update,
            Command::Bump,
            Command::Migrate,
            Command::New,
            Command::Upgrade,
            Command::Init,
            Command::Status,
            Command::Version,
            Command::Watch,
            Command::Completion,
            Command::Docs,
            Command::ListTests,
            Command::Rollback,
            Command::Worktree,
        ] {
            assert!(
                !command.is_mutating_by_default(),
                "{command:?} must stay non-mutating by default"
            );
        }
        assert!(
            Command::Check.describe().contains("non-mutating"),
            "check help must name its non-mutating mode"
        );
        for command in [
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Fix,
            Command::Generate,
            Command::Codegen,
            Command::Env,
            Command::Clean,
            Command::Update,
            Command::Bump,
            Command::Migrate,
            Command::Init,
            Command::New,
            Command::Upgrade,
            Command::Rollback,
        ] {
            assert!(
                command.describe().contains("check by default"),
                "{command:?} help must name its check default: {}",
                command.describe()
            );
            assert!(
                command.describe().contains("--apply"),
                "{command:?} help must name --apply: {}",
                command.describe()
            );
        }
        for command in [
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Generate,
            Command::Check,
            Command::Fix,
        ] {
            assert!(
                command.supports_diff(),
                "{command:?} must support --output=diff"
            );
        }
        assert!(!Command::Update.supports_diff());
        assert!(!Command::Bump.supports_diff());
        assert!(!Command::Migrate.supports_diff());
    }

    #[test]
    fn offline_is_audit_update_bump_only() {
        for command in [
            Command::Security,
            Command::License,
            Command::Update,
            Command::Bump,
        ] {
            assert!(
                command.supports_offline(),
                "{command:?} must support --offline"
            );
            assert!(
                command.supports_frozen(),
                "{command:?} must support --frozen"
            );
        }
        for command in [
            Command::Lint,
            Command::Build,
            Command::Clean,
            Command::Codegen,
            Command::Status,
            Command::Docs,
            Command::Migrate,
        ] {
            assert!(
                !command.supports_offline(),
                "{command:?} must reject --offline"
            );
            assert!(
                !command.supports_frozen(),
                "{command:?} must reject --frozen"
            );
        }
    }

    #[test]
    fn fallback_usage_registry_is_single_sourced() {
        use clap::ValueEnum;
        let list = Command::pipe_list();
        let missing_text = super::super::ArgsError::MissingCommand.to_string();
        let unknown_text = super::super::help::unknown_command_text("bogus");
        assert!(
            missing_text.contains(&list),
            "ArgsError::MissingCommand drifted from pipe_list: {missing_text}"
        );
        for command in Command::value_variants() {
            assert!(
                unknown_text.contains(command.name()),
                "clap must name {} when the command is unknown: {unknown_text}",
                command.name()
            );
        }
        assert_eq!(
            list,
            "security|license|lint|typecheck|format|generate|build|test|coverage|run|deploy|check|fix|clean|update|bump|migrate|codegen|env|init|new|upgrade|status|version|watch|completion|docs|list-tests|rollback|worktree",
            "pipe_list order must match declaration order"
        );
    }

    #[test]
    fn scope_defaults_partition_covers_all_commands() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let policy = command.scope_policy();
            assert!(
                [
                    "default-//...",
                    "default-repo",
                    "default-repo|exact-label",
                    "require",
                    "require-file+label",
                    "require-label",
                    "require-selector+version",
                    "selector-default-all",
                    "reject",
                    "optional-name",
                ]
                .contains(&policy),
                "{command:?} has unknown scope policy {policy}"
            );
            if command.supports_here() {
                assert!(
                    policy.starts_with("default-"),
                    "{command:?} supports --here so policy must be default-*: {policy}"
                );
            }
            if command.is_managed() {
                assert_eq!(
                    policy, "default-repo|exact-label",
                    "{command:?} managed policy must stay exact-label"
                );
            }
            if command.is_adoption()
                && matches!(
                    command,
                    Command::Status
                        | Command::Version
                        | Command::Upgrade
                        | Command::Completion
                        | Command::Rollback
                )
            {
                assert!(
                    policy == "reject" || policy == "require",
                    "{command:?} adoption scope must be reject/require: {policy}"
                );
            }
        }
        assert_eq!(Command::Run.scope_policy(), "require");
        assert_eq!(Command::Deploy.scope_policy(), "require-label");
        assert_eq!(Command::Clean.scope_policy(), "reject");
        assert_eq!(Command::Bump.scope_policy(), "require-selector+version");
        assert_eq!(Command::Update.scope_policy(), "selector-default-all");
        assert_eq!(Command::Migrate.scope_policy(), "default-//...");
    }

    #[test]
    fn value_sets_and_man_parity_are_pinned() {
        use clap::ValueEnum;
        let bad_output = super::super::ArgsError::BadOutput {
            value: "yaml".to_owned(),
        }
        .to_string();
        assert!(
            bad_output.contains("text|diff|json"),
            "output value set drifted: {bad_output}"
        );
        let bad_fail = super::super::ArgsError::BadFailOn {
            value: "never".to_owned(),
        }
        .to_string();
        assert!(
            bad_fail.contains("info|warning|error"),
            "fail-on value set drifted: {bad_fail}"
        );
        let bad_color = super::super::ArgsError::BadColor {
            value: "bright".to_owned(),
        }
        .to_string();
        assert!(
            bad_color.contains("auto|always|never"),
            "color value set drifted: {bad_color}"
        );
        assert_eq!(
            super::super::completion::COMPLETION_SHELLS,
            &["bash", "zsh", "fish", "powershell"],
            "shell value set changed; update docs plus fixtures"
        );
        let root = super::super::grammar::cli_command();
        let mut text = String::new();
        for command in Command::value_variants() {
            let sub = root
                .find_subcommand(command.name())
                .unwrap_or_else(|| panic!("dx has no {} subcommand", command.name()));
            let man = clap_mangen::Man::new(sub.clone());
            let mut buffer = Vec::new();
            man.render(&mut buffer).expect("man renders");
            text.push_str(&String::from_utf8(buffer).expect("man utf8"));
        }
        for command in Command::value_variants() {
            assert!(
                text.contains(command.name()),
                "man missing command {}",
                command.name()
            );
        }
        for stem in ["output", "fail", "here", "color", "host", "open"] {
            assert!(text.contains(stem), "man missing flag stem {stem}");
        }
    }

    /// The `dx_man` page ships one file; assert it sections every command.
    #[test]
    fn dx_man_page_sections_every_command() {
        use clap::ValueEnum;
        let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        let path = std::path::Path::new(&root)
            .join(&workspace)
            .join("cli/cli/man/dx.1");
        let page = std::fs::read_to_string(&path).expect("man_pages ships as test data");
        assert!(
            page.contains(".SH SUBCOMMANDS"),
            "man/dx.1 has no SUBCOMMANDS section:\n{page}"
        );
        let mut sections: Vec<String> = page
            .lines()
            .filter_map(|line| line.strip_prefix("dx\\-"))
            .filter_map(|rest| rest.strip_suffix("(1)"))
            .map(|name| name.replace("\\-", "-"))
            .collect();
        sections.sort_unstable();
        let mut expected: Vec<String> = Command::value_variants()
            .iter()
            .map(|command| command.name().to_owned())
            .collect();
        expected.sort_unstable();
        assert_eq!(
            sections, expected,
            "man/dx.1 must section every command exactly once"
        );
        for command in Command::value_variants() {
            let describe = command.describe().replace('-', "\\-");
            assert!(
                page.contains(&describe),
                "man/dx.1 never describes dx {}: {describe}",
                command.name()
            );
        }
    }

    #[test]
    fn required_slot_reads_whole_words_of_the_usage_tail() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let Some(slot) = command.required_slot() else {
                continue;
            };
            let tail = command
                .usage()
                .strip_prefix("Usage: dx ")
                .and_then(|rest| rest.split_once(' ').map(|(_, tail)| tail))
                .unwrap_or_default();
            let width = slot.split(' ').count();
            let runs = [Some(tail), tail.strip_prefix('[')]
                .into_iter()
                .flatten()
                .any(|candidate| {
                    let words: Vec<&str> = candidate.split(' ').collect();
                    words.len() >= width
                        && (0..=words.len() - width)
                            .any(|start| words[start..start + width].join(" ") == slot)
                });
            assert!(
                runs,
                "dx {}: {slot:?} is not a whole-word run of {tail:?}",
                command.name()
            );
        }
    }

    #[test]
    fn every_command_that_needs_a_positional_spells_it_in_its_usage() {
        use clap::ValueEnum;
        let mut required: Vec<(&str, Option<&str>)> = Command::value_variants()
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    Command::Bump | Command::Completion | Command::New | Command::Watch
                )
            })
            .map(|command| (command.name(), command.required_slot()))
            .collect();
        required.sort_unstable();
        assert_eq!(
            required,
            vec![
                ("bump", Some("<set:package> <version>")),
                ("completion", Some("<shell>")),
                ("new", Some("<language>")),
                (
                    "watch",
                    Some("<build|test|run|lint|typecheck|format|check|fix>")
                ),
            ]
        );
    }

    #[test]
    fn table_covers_every_command_exactly_once() {
        use clap::ValueEnum;
        assert_eq!(COMMANDS.len(), Command::value_variants().len());
        for (index, entry) in COMMANDS.iter().enumerate() {
            assert_eq!(
                entry.command,
                Command::value_variants()[index],
                "table order must match declaration order"
            );
            assert_eq!(entry.name, entry.command.meta().name);
            assert_eq!(entry.describe, entry.command.describe());
            assert_eq!(entry.usage, entry.command.usage());
            if entry.command == Command::New {
                assert!(entry.flags.contains("{languages}"));
            } else {
                assert_eq!(entry.flags, entry.command.flags());
            }
            assert_eq!(entry.scopes, entry.command.scopes_text());
            assert_eq!(entry.scope_policy, entry.command.scope_policy());
        }
        let mut seen: Vec<Command> = Vec::new();
        for entry in COMMANDS.iter() {
            assert!(!seen.contains(&entry.command), "duplicate table entry");
            seen.push(entry.command);
            assert!(!entry.name.is_empty());
            assert!(!entry.describe.is_empty());
            assert!(!entry.usage.is_empty());
            assert!(!entry.flags.is_empty());
            assert!(!entry.scopes.is_empty());
        }
    }

    #[test]
    fn every_workflow_verb_is_the_command_it_runs() {
        use clap::ValueEnum;
        let mut verbs: Vec<&'static str> = Vec::new();
        for command in Command::value_variants() {
            let Some(verb) = command.workflow_verb() else {
                continue;
            };
            assert_eq!(
                verb.name(),
                command.name(),
                "{command:?} runs bazel {} but is spelled {}",
                verb.name(),
                command.name()
            );
            verbs.push(verb.name());
        }
        verbs.sort_unstable();
        assert_eq!(verbs, ["build", "coverage", "run", "test"]);
        assert!(WorkflowVerb::Test.collects_reports());
        assert!(WorkflowVerb::Coverage.collects_reports());
        assert!(!WorkflowVerb::Build.collects_reports());
        assert!(!WorkflowVerb::Run.collects_reports());
    }
}
