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
    Add,
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
    Explain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstSlot {
    None,
    UpdateSets,
    BumpSets,
    AddSets,
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
    pub scopes: &'static str,
    pub is_managed: bool,
    pub is_adoption: bool,
    pub supports_json: bool,
    pub supports_diff: bool,
    pub supports_here: bool,
    pub supports_fail_on: bool,
    pub default_release: bool,
    pub skew: SkewKind,
    pub workflow_verb: Option<WorkflowVerb>,
    pub first_slot: FirstSlot,
    pub labels: LabelsPolicy,
}

pub static COMMANDS: [CommandMeta; 32] = [
    CommandMeta {
        command: Command::Security,
        name: "security",
        scope_policy: "default-//...",
        describe: "run security audit over resolved scopes (non-mutating; live Gitleaks plus advisory/vuln backends)",
        usage: "Usage: dx security [--offline] [--frozen] [--here] [--affected] [scope ...]",
        scopes: "Scopes: dependency-set/package/target selectors; bare run audits //... (secrets plus vulnerabilities). Pass --here for the current directory tree instead; --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: true,
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
        scopes: "Scopes: dependency-set/package/target selectors; bare run audits //... (license policy plus SPDX inventory). Pass --here for the current directory tree instead; --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: true,
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
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_fail_on: true,
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
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_fail_on: true,
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
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_fail_on: true,
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
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target; main-repository labels only), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_fail_on: false,
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
        usage: "Usage: dx build [--here] [--affected] [--debug|--release] [--explain] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: false,
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
        usage: "Usage: dx test [--here] [--affected] [--debug|--release] [--explain] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: false,
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
        usage: "Usage: dx coverage [--here] [--affected] [--min-coverage 0-100] [--min-changed-line-coverage 0-100] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: false,
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
        usage: "Usage: dx run [--debug|--release] [--bazel-option <token>]... <label> ... [-- app-args ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Requires a scope (empty scope is a usage error); file/dir scopes need exactly one runnable.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: exactly one main-workspace label (//pkg:target); patterns (//...), multiple labels, and file/path scopes are usage failures.",
        is_managed: false,
        is_adoption: false,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        describe: "run generate+format+lint+typecheck checks in order (non-mutating)",
        usage: "Usage: dx check|fix [--check] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_fail_on: true,
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
        describe: "apply generate+format+lint+typecheck fixes in order (check by default; --apply writes fixes, then verifies with a read-only check)",
        usage: "Usage: dx check|fix [--check] [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: true,
        supports_here: true,
        supports_fail_on: true,
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
        scopes: "Scopes: none (clean takes no scopes).",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: dependency-set/package/target selectors (cargo|go|maven|npm|npm-adopt|npm-adopt-polyglot|npm-tools|nuget|powershell|ruby|uv|uv-adopt|uv-adopt-polyglot|uv-tools, set:package, labels/paths); bare run checks all sets.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        usage: "Usage: dx bump [--apply] [--offline] [--frozen] [--floor] [--reason=<text>] <set:package> <version>",
        scopes: "Scopes: exactly one `set:package` plus one new version (bazel|cargo|github-actions|go|maven|npm|nuget, or a configured bazel set as `<set>:<module>`); never batch.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::BumpSets,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Add,
        name: "add",
        scope_policy: "require-selector+version",
        describe: "declare one new requirement at an exact version (check by default; --apply adds)",
        usage: "Usage: dx add [--apply] [--offline] [--frozen] <set:package> <version>",
        scopes: "Scopes: exactly one `set:package` that is not yet declared plus its exact version (bazel|cargo|go|maven|npm|nuget); never batch. Already declared packages change with `dx bump`.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
        default_release: false,
        skew: SkewKind::Refuse,
        workflow_verb: None,
        first_slot: FirstSlot::AddSets,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Migrate,
        name: "migrate",
        scope_policy: "default-//...",
        describe: "rewrite breaking changes across releases (upgrade-only; check by default; --apply migrates)",
        usage: "Usage: dx migrate [--apply] --from <version> --to <version> [scope ...]",
        scopes: "Scopes: explicit Bazel labels/patterns or workspace-relative files/dirs. No scope selects //.... Scopes are accepted but not read until a migration ships.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: none for repository-wide canonical selection, or exactly one exact // or @ label; patterns, paths, relative labels, empty values, and multiple labels are usage failures (see docs/cli/commands/environment-codegen.md).",
        is_managed: true,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: none for repository-wide canonical selection, or exactly one exact // or @ label; patterns, paths, relative labels, empty values, and multiple labels are usage failures (see docs/cli/commands/environment-codegen.md).",
        is_managed: true,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        usage: "Usage: dx init [--apply] [--with=<parts>|--full] [module-name]",
        scopes: "Scopes: optional single module name (defaults to my_project when absent); Bazel labels/patterns are not scopes; extra positionals are usage failures.",
        is_managed: false,
        is_adoption: true,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: <language> ({languages}) plus optional project name (defaults to my_project); rust-web scaffolds a shared Rust core with native and browser targets; rust-android scaffolds a Rust core packaged as a debug APK with adb launchers; absent-only, no --force; unknown languages fail with the supported list; extra positionals are usage failures.",
        is_managed: false,
        is_adoption: true,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        describe: "pin the module version and refresh existing launchers and the AGENTS.md block (runs on skew; check by default; --apply upgrades)",
        usage: "Usage: dx upgrade [--apply] --from <version> --to <version>",
        scopes: "Scopes: none (repository-wide: writes .dx/version and the pin history, refreshes existing tools/dx launchers and an existing AGENTS.md block; --to must equal this binary's version; --from/--to required, positional scopes rejected).",
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Status,
        name: "status",
        scope_policy: "reject",
        describe: "report workspace status; --verbose adds command capabilities; without .dx/version it exits 1 with status_pin_mismatch",
        usage: "Usage: dx status",
        scopes: "Scopes: none (status takes no scopes).",
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: none (version takes no scopes).",
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: wrapped command plus its scopes, re-resolved each iteration (local only, refuses CI=true; only build|test|run|lint|typecheck|format|check|fix are watchable). Args after `--` reach the wrapped command as its own command-options.",
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: true,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: exactly one shell (bash|zsh|fish|powershell); unknown shells fail with unknown-shell.",
        is_managed: false,
        is_adoption: true,
        supports_json: false,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Bare scope selects the repository docs site (//tools/docs/site:user_site, //tools/docs/site:user_site_check in --check). Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: false,
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
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default. --affected lists only the affected tests, not their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: true,
        supports_fail_on: false,
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
        scopes: "Scopes: none (rollback takes no scopes).",
        is_managed: false,
        is_adoption: true,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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
        usage: "Usage: dx worktree add <branch> [--create=<ref>] [--force] | release [<path>] [--force] | list [--disk] | path <branch> | prune [--apply] [--bases] | hook create|remove|stop|guard",
        scopes: "Scopes: none (worktree takes a subcommand and its arguments, not scopes).",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
        default_release: false,
        skew: SkewKind::Proceed,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Explain,
        name: "explain",
        scope_policy: "reject",
        describe: "explain why actions missed the cache by diffing two execution logs from build/test --explain (non-mutating)",
        usage: "Usage: dx explain [--old=<log>] [--new=<log>]",
        scopes: "Scopes: none (explain compares saved execution logs, not scopes).",
        is_managed: false,
        is_adoption: false,
        supports_json: true,
        supports_diff: false,
        supports_here: false,
        supports_fail_on: false,
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

    pub fn parse(text: &str) -> Option<Self> {
        use clap::ValueEnum;
        Self::from_str(text, false).ok()
    }

    pub fn workflow_verb(self) -> Option<WorkflowVerb> {
        self.meta().workflow_verb
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

    pub fn supports_here(self) -> bool {
        self.meta().supports_here
    }

    /// Whether the grammar lets `dx <command> --apply` parse.
    pub fn supports_apply(self) -> bool {
        super::grammar::accepts_long(self, "apply")
    }

    pub fn supports_fail_on(self) -> bool {
        self.meta().supports_fail_on
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

    pub fn scopes_text(self) -> &'static str {
        if self == Command::New {
            static NEW_SCOPES: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            return NEW_SCOPES.get_or_init(|| {
                self.meta()
                    .scopes
                    .replace("{languages}", &dx_adopt::SUPPORTED_NEW_LANGUAGES.join("|"))
            });
        }
        self.meta().scopes
    }

    pub(crate) fn allows_labels(self, prior_len: usize) -> bool {
        match self.meta().labels {
            LabelsPolicy::Always => true,
            LabelsPolicy::Never => false,
            LabelsPolicy::OnlyEmpty => prior_len == 0,
            LabelsPolicy::OnlyNonEmpty => prior_len > 0,
        }
    }
}

/// Explicit owner lists the tests pin parser behaviour against.
#[cfg(test)]
pub(crate) mod owners {
    use super::Command::{self, *};

    pub(crate) const APPLY_COMMANDS: [Command; 19] = [
        Lint, Typecheck, Format, Generate, Deploy, Fix, Clean, Update, Bump, Add, Migrate, Codegen,
        Env, Init, New, Upgrade, Watch, Rollback, Worktree,
    ];
    pub(crate) const PROFILE_OWNERS: [Command; 5] = [Build, Test, Run, Deploy, Watch];
    pub(crate) const CHECK_OWNERS: [Command; 12] = [
        Lint, Typecheck, Format, Generate, Check, Fix, Clean, Update, Codegen, Env, Version, Docs,
    ];
    pub(crate) const AFFECTED_OWNERS: [Command; 11] = [
        Security, License, Lint, Typecheck, Format, Build, Test, Coverage, Check, Fix, ListTests,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_commands_are_exactly_the_grammar_apply_owners() {
        use super::owners::APPLY_COMMANDS;
        use clap::ValueEnum;
        for command in Command::value_variants() {
            assert_eq!(
                APPLY_COMMANDS.contains(command),
                command.supports_apply(),
                "{command:?}"
            );
        }
    }

    #[test]
    fn command_names_are_stable() {
        assert_eq!(Command::Security.name(), "security");
        assert_eq!(Command::License.name(), "license");
        assert_eq!(Command::Update.name(), "update");
        assert_eq!(Command::Bump.name(), "bump");
        assert_eq!(Command::Add.name(), "add");
        assert_eq!(Command::Migrate.name(), "migrate");
        assert_eq!(Command::New.name(), "new");
        assert_eq!(Command::Upgrade.name(), "upgrade");
        assert!(!Command::Migrate.is_adoption());
        assert!(!Command::Migrate.is_managed());
        assert!(Command::Migrate.supports_json());
        assert!(!Command::Migrate.supports_diff());
        assert!(Command::New.is_adoption());
        assert!(Command::Upgrade.is_adoption());
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
            Command::Add,
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
            Command::Explain,
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
    fn help_names_check_default_and_apply() {
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
            Command::Add,
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
        assert!(!Command::Add.supports_diff());
        assert!(!Command::Migrate.supports_diff());
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
            "security|license|lint|typecheck|format|generate|build|test|coverage|run|deploy|check|fix|clean|update|bump|add|migrate|codegen|env|init|new|upgrade|status|version|watch|completion|docs|list-tests|rollback|worktree|explain",
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
        assert_eq!(Command::Add.scope_policy(), "require-selector+version");
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
        let bytes = super::super::grammar::man_page().expect("man page renders");
        let page = String::from_utf8(bytes).expect("man page is utf-8");
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
                    Command::Bump
                        | Command::Add
                        | Command::Completion
                        | Command::New
                        | Command::Watch
                )
            })
            .map(|command| (command.name(), command.required_slot()))
            .collect();
        required.sort_unstable();
        assert_eq!(
            required,
            vec![
                ("add", Some("<set:package> <version>")),
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
                assert!(entry.scopes.contains("{languages}"));
            } else {
                assert_eq!(entry.scopes, entry.command.scopes_text());
            }
            assert_eq!(entry.scope_policy, entry.command.scope_policy());
        }
        let mut seen: Vec<Command> = Vec::new();
        for entry in COMMANDS.iter() {
            assert!(!seen.contains(&entry.command), "duplicate table entry");
            seen.push(entry.command);
            assert!(!entry.name.is_empty());
            assert!(!entry.describe.is_empty());
            assert!(!entry.usage.is_empty());
            assert!(!entry.scopes.is_empty());
        }
    }

    #[test]
    fn reject_scope_policy_help_says_no_scopes() {
        for entry in COMMANDS
            .iter()
            .filter(|entry| entry.scope_policy == "reject")
        {
            assert!(
                entry.scopes.starts_with("Scopes: none"),
                "{} rejects scopes but its help advertises them: {}",
                entry.name,
                entry.scopes
            );
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

    #[test]
    fn no_command_or_alias_takes_a_reserved_name() {
        use clap::CommandFactory;
        let mut cli = crate::args::grammar::Cli::command();
        cli.build();
        for (reserved, _) in crate::args::tokenizer::RESERVED_COMMANDS {
            assert!(Command::parse(reserved).is_none(), "{reserved}");
            assert!(COMMANDS.iter().all(|entry| entry.name != *reserved));
            for sub in cli.get_subcommands() {
                assert_ne!(sub.get_name(), *reserved);
                assert!(sub.get_all_aliases().all(|alias| alias != *reserved));
            }
        }
    }
}
