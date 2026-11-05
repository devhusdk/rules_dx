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
    Bench,
    Run,
    Deploy,
    Check,
    Clean,
    Update,
    Env,
    Init,
    Upgrade,
    Status,
    Watch,
    Completion,
    Docs,
    Worktree,
    Explain,
    Fuzz,
    Mutate,
    Release,
    Unused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstSlot {
    None,
    UpdateSets,
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
pub enum WorkflowVerb {
    Build,
    Test,
    Coverage,
    Bench,
    Run,
}

impl WorkflowVerb {
    pub fn name(self) -> &'static str {
        match self {
            WorkflowVerb::Build => "build",
            WorkflowVerb::Test => "test",
            WorkflowVerb::Coverage => "coverage",
            WorkflowVerb::Bench => "bench",
            WorkflowVerb::Run => "run",
        }
    }

    /// The Bazel command this verb runs; `bench` is `bazel test` over the tagged tests.
    pub fn bazel_command(self) -> &'static str {
        match self {
            WorkflowVerb::Bench => "test",
            other => other.name(),
        }
    }

    pub fn collects_reports(self) -> bool {
        matches!(
            self,
            WorkflowVerb::Test | WorkflowVerb::Coverage | WorkflowVerb::Bench
        )
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
    pub default_release: bool,
    pub workflow_verb: Option<WorkflowVerb>,
    pub first_slot: FirstSlot,
    pub labels: LabelsPolicy,
}

pub static COMMANDS: [CommandMeta; 28] = [
    CommandMeta {
        command: Command::Security,
        name: "security",
        scope_policy: "default-//...",
        describe: "run security audit over resolved scopes (non-mutating; live Gitleaks plus advisory/vuln backends)",
        usage: "Usage: dx security [--offline] [--frozen] [--here] [--affected] [scope ...]",
        scopes: "Scopes: dependency-set/package/target selectors; With no scope, this command runs //...; pass --affected to run only what your changes reach. It audits secrets plus vulnerabilities. Pass --here for the current directory tree instead; --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::License,
        name: "license",
        scope_policy: "default-//...",
        describe: "run license audit over resolved scopes (non-mutating)",
        usage: "Usage: dx license [--offline] [--frozen] [--here] [--affected] [scope ...]",
        scopes: "Scopes: dependency-set/package/target selectors; With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead; --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Lint,
        name: "lint",
        scope_policy: "default-//...",
        describe: "run lint analysis over resolved scopes (check by default; --apply writes fixes)",
        usage: "Usage: dx lint|typecheck|format [--apply] [--online] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Typecheck,
        name: "typecheck",
        scope_policy: "default-//...",
        describe: "run typecheck analysis over resolved scopes (check by default; --apply writes fixes)",
        usage: "Usage: dx lint|typecheck|format [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Format,
        name: "format",
        scope_policy: "default-//...",
        describe: "check or rewrite formatting over resolved scopes (check by default; --apply writes fixes)",
        usage: "Usage: dx lint|typecheck|format [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Generate,
        name: "generate",
        scope_policy: "default-repo",
        describe: "update BUILD declarations from source with Gazelle (check by default; --apply writes)",
        usage: "Usage: dx generate [--apply] [--here] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target; main-repository labels only), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Build,
        name: "build",
        scope_policy: "default-//...",
        describe: "run Bazel build over resolved targets",
        usage: "Usage: dx build [--here] [--affected] [--debug|--release] [--explain] [--profile] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: Some(WorkflowVerb::Build),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Test,
        name: "test",
        scope_policy: "default-//...",
        describe: "run Bazel test over resolved targets",
        usage: "Usage: dx test [--here] [--affected] [--debug|--release] [--explain] [--profile] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: Some(WorkflowVerb::Test),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Coverage,
        name: "coverage",
        scope_policy: "default-//...",
        describe: "collect LCOV coverage with optional threshold",
        usage: "Usage: dx coverage [--here] [--affected] [--explain] [--profile] [--min-coverage <percent>] [--min-changed-line-coverage <percent>] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: Some(WorkflowVerb::Coverage),
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Bench,
        name: "bench",
        scope_policy: "default-//...",
        describe: "run dx-bench tagged tests and print their numbers",
        usage: "Usage: dx bench [--compare <path|ref>] [--fail-on=regression:<pct>] [--here] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; only tests tagged dx-bench run. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: Some(WorkflowVerb::Bench),
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
        default_release: false,
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
        default_release: true,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::OnlyEmpty,
    },
    CommandMeta {
        command: Command::Check,
        name: "check",
        scope_policy: "default-//...",
        describe: "run generate+format+lint+typecheck in order (check by default; --apply writes fixes, then verifies with a read-only check)",
        usage: "Usage: dx check [--apply] [--here] [--affected] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Clean,
        name: "clean",
        scope_policy: "reject",
        describe: "prune unselected managed state, never Bazel outputs; use `bazel clean` for those (check by default; --apply prunes; no scopes)",
        usage: "Usage: dx clean [--apply] [--prune-unobserved]",
        scopes: "Scopes: none (clean takes no scopes).",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Update,
        name: "update",
        scope_policy: "selector-default-all",
        describe: "update dependencies per set, or widen or add one package to a version (check by default; --apply writes)",
        usage: "Usage: dx update [--apply [--prune] [--add-missing]] [--outdated] [--pr [--group=package|set|all] [--labels=<a,b>] [--automerge=none|patch|minor|all] [--min-age=<days>] [--ignore=<set:package>] [--allow-major] [--prereleases] [--max-prs=<n>]] [--offline] [--frozen] [selector ...] | <set:package> <version> [--floor --reason=<text>]",
        scopes: "Scopes: dependency-set/package/target selectors (cargo|go|maven|npm|npm-adopt|npm-adopt-polyglot|npm-tools|nuget|powershell|ruby|uv|uv-adopt|uv-adopt-polyglot|uv-tools, set:package, labels/paths); bare run checks all sets. `<set:package> <version>` widens that one package, or adds it when missing (bazel|cargo|github-actions|go|maven|npm|nuget).",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::UpdateSets,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Env,
        name: "env",
        scope_policy: "default-repo|exact-label",
        describe: "collect managed tools and generated source (check by default; --apply writes managed state)",
        usage: "Usage: dx env [--apply] [--shell=direnv] [<label>] [-- bazel-options ...]",
        scopes: "Scopes: none for repository-wide canonical selection, or exactly one exact // or @ label; patterns, paths, relative labels, empty values, and multiple labels are usage failures (see docs/cli/commands/env.md).",
        is_managed: true,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::OnlyEmpty,
    },
    CommandMeta {
        command: Command::Init,
        name: "init",
        scope_policy: "optional-name",
        describe: "start or adopt a dx workspace in this or a new directory (check by default; --apply scaffolds and is safe to re-run)",
        usage: "Usage: dx init [--apply] [--agents=<list>] [--profile=standard|strict] [dir]",
        scopes: "Scopes: optional single directory, created by --apply when missing (defaults to the current directory); the module name is the directory name, or my_project for the current directory; Bazel labels/patterns are not scopes; extra positionals are usage failures.",
        is_managed: false,
        is_adoption: true,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Upgrade,
        name: "upgrade",
        scope_policy: "reject",
        describe: "move the pin to any version, newer or older, and refresh existing launchers and the AGENTS.md block (check by default; --apply moves)",
        usage: "Usage: dx upgrade [--apply] --to <version>",
        scopes: "Scopes: none (repository-wide: reads and writes .dx/pins, refreshes existing tools/dx launchers and an existing AGENTS.md block; --to must equal this binary's version; --to required, positional scopes rejected).",
        is_managed: false,
        is_adoption: true,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Status,
        name: "status",
        scope_policy: "reject",
        describe: "report workspace status; --capabilities adds command capabilities",
        usage: "Usage: dx status",
        scopes: "Scopes: none (status takes no scopes).",
        is_managed: false,
        is_adoption: true,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Watch,
        name: "watch",
        scope_policy: "require",
        describe: "watch for changes and rebuild (local only)",
        usage: "Usage: dx watch [--apply] [--debug|--release] [flags] <build|test|run|lint|typecheck|format|check> [scope ...] [-- bazel-options|app-args ...]",
        scopes: "Scopes: wrapped command plus its scopes, re-resolved each iteration (local only, refuses CI=true; only build|test|run|lint|typecheck|format|check are watchable). Args after `--` reach the wrapped command as its own command-options.",
        is_managed: false,
        is_adoption: true,
        default_release: false,
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
        scopes: "Scopes: exactly one shell (bash|zsh|fish|powershell); unknown shells fail with unknown shell.",
        is_managed: false,
        is_adoption: true,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::CompletionShells,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Docs,
        name: "docs",
        scope_policy: "default-repo",
        describe: "build and serve the mdBook documentation site, or an API index of the Rust libraries in scope (non-mutating)",
        usage: "Usage: dx docs [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. Bare scope selects every docs site, shard and API target in the repository (//...). Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
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
        default_release: false,
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
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Fuzz,
        name: "fuzz",
        scope_policy: "default-//...",
        describe: "run dx-fuzz targets for a time budget and store crashes under .dx/fuzz",
        usage: "Usage: dx fuzz [--here] [--time <duration>] [--jobs <n>] [scope ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //.... Only targets tagged dx-fuzz run.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Mutate,
        name: "mutate",
        scope_policy: "default-//...",
        describe: "run dx-mutate targets for a time budget and report surviving mutants",
        usage: "Usage: dx mutate [--here] [--time <duration>] [--fail-on=survived:<pct>] [scope ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //.... Only targets tagged dx-mutate run.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
    },
    CommandMeta {
        command: Command::Release,
        name: "release",
        scope_policy: "reject",
        describe: "derive the next version from conventional commits, list the manifests that carry it, and with --apply bump them, commit and tag",
        usage: "Usage: dx release [--check [--commits <range>] | --apply [--push]] [--to <x.y.z>]",
        scopes: "Scopes: none; positionals are usage failures. The last tag is the base; the version comes from conventional commits since it, or from --to.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Never,
    },
    CommandMeta {
        command: Command::Unused,
        name: "unused",
        scope_policy: "default-//...",
        describe: "report cross-file unused code over resolved scopes (non-mutating; warnings only unless --fail-on=warning)",
        usage: "Usage: dx unused [--here] [--affected] [--fail-on=none|warning] [scope ...] [-- bazel-options ...]",
        scopes: "Scopes: explicit Bazel labels/patterns (//..., //pkg:target), or workspace-relative files/dirs resolved via Bazel query. With no scope, this command runs //...; pass --affected to run only what your changes reach. Pass --here for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes. --affected selects targets changed since the default branch merge-base plus their reverse dependencies; it cannot be combined with scopes or --here.",
        is_managed: false,
        is_adoption: false,
        default_release: false,
        workflow_verb: None,
        first_slot: FirstSlot::None,
        labels: LabelsPolicy::Always,
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

    /// Whether the grammar lets `dx <command> --output` parse.
    pub fn supports_json(self) -> bool {
        super::grammar::accepts_long(self, "output")
    }

    /// Whether the grammar lets `dx <command> --diff` parse.
    pub fn supports_diff(self) -> bool {
        super::grammar::accepts_long(self, "diff")
    }

    /// Whether `--here` applies; watch passes it to the command it wraps.
    pub fn supports_here(self) -> bool {
        super::grammar::accepts_long(self, "here") && self != Command::Watch
    }

    /// Whether the grammar lets `dx <command> --apply` parse.
    pub fn supports_apply(self) -> bool {
        super::grammar::accepts_long(self, "apply")
    }

    /// Whether `--fail-on` applies; watch passes it to the command it wraps.
    pub fn supports_fail_on(self) -> bool {
        super::grammar::accepts_long(self, "fail-on")
            && !matches!(self, Command::Watch | Command::Mutate | Command::Bench)
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

    pub(crate) const APPLY_COMMANDS: [Command; 14] = [
        Lint, Typecheck, Format, Generate, Deploy, Check, Clean, Update, Env, Init, Upgrade, Watch,
        Worktree, Release,
    ];
    pub(crate) const PROFILE_OWNERS: [Command; 5] = [Build, Test, Run, Deploy, Watch];
    pub(crate) const AFFECTED_OWNERS: [Command; 10] = [
        Security, License, Lint, Typecheck, Format, Build, Test, Coverage, Check, Unused,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_sets_are_pinned() {
        use clap::ValueEnum;
        let all = Command::value_variants();
        let set = |pred: fn(Command) -> bool| -> Vec<Command> {
            all.iter().copied().filter(|c| pred(*c)).collect()
        };
        let json: Vec<Command> = all
            .iter()
            .copied()
            .filter(|c| {
                !matches!(
                    c,
                    Command::Deploy
                        | Command::Completion
                        | Command::Fuzz
                        | Command::Mutate
                        | Command::Release
                )
            })
            .collect();
        assert_eq!(set(Command::supports_json), json);
        let in_order = |names: &[Command]| -> Vec<Command> {
            all.iter().copied().filter(|c| names.contains(c)).collect()
        };
        assert_eq!(
            set(Command::supports_diff),
            in_order(&[
                Command::Lint,
                Command::Typecheck,
                Command::Format,
                Command::Generate,
                Command::Check,
                Command::Watch
            ])
        );
        assert_eq!(
            set(Command::supports_here),
            in_order(&[
                Command::Security,
                Command::License,
                Command::Lint,
                Command::Typecheck,
                Command::Format,
                Command::Generate,
                Command::Build,
                Command::Test,
                Command::Coverage,
                Command::Bench,
                Command::Check,
                Command::Docs,
                Command::Fuzz,
                Command::Mutate,
                Command::Unused
            ])
        );
        assert_eq!(
            set(Command::supports_fail_on),
            in_order(&[
                Command::Security,
                Command::License,
                Command::Lint,
                Command::Typecheck,
                Command::Format,
                Command::Coverage,
                Command::Check,
                Command::Update,
                Command::Unused
            ])
        );
    }

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
        assert_eq!(Command::Upgrade.name(), "upgrade");
        assert!(Command::Upgrade.is_adoption());
        assert!(Command::Upgrade.supports_json());
        assert!(!Command::Upgrade.supports_diff());
        assert_eq!(Command::Lint.name(), "lint");
        assert_eq!(Command::Typecheck.name(), "typecheck");
        assert_eq!(Command::Format.name(), "format");
        assert_eq!(Command::Generate.name(), "generate");
        assert_eq!(Command::Build.name(), "build");
        assert_eq!(Command::Test.name(), "test");
        assert_eq!(Command::Coverage.name(), "coverage");
        assert_eq!(Command::Bench.name(), "bench");
        assert_eq!(Command::Run.name(), "run");
        assert_eq!(Command::Env.name(), "env");
        assert!(Command::Env.is_managed());
        assert!(!Command::Env.is_adoption());
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
            Command::Bench,
            Command::Run,
            Command::Deploy,
            Command::Check,
            Command::Clean,
            Command::Update,
            Command::Env,
            Command::Init,
            Command::Upgrade,
            Command::Status,
            Command::Watch,
            Command::Completion,
            Command::Docs,
            Command::Worktree,
            Command::Explain,
            Command::Fuzz,
            Command::Mutate,
            Command::Release,
            Command::Unused,
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
            Command::Check.describe().contains("check by default"),
            "check help must name its check default"
        );
        for command in [
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Generate,
            Command::Check,
            Command::Env,
            Command::Clean,
            Command::Update,
            Command::Init,
            Command::Upgrade,
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
        ] {
            assert!(command.supports_diff(), "{command:?} must support --diff");
        }
        assert!(!Command::Update.supports_diff());
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
            "security|license|lint|typecheck|format|generate|build|test|coverage|bench|run|deploy|check|clean|update|env|init|upgrade|status|watch|completion|docs|worktree|explain|fuzz|mutate|release|unused",
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
                    Command::Status | Command::Upgrade | Command::Completion
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
        assert_eq!(Command::Update.scope_policy(), "selector-default-all");
    }

    #[test]
    fn watch_usage_shows_app_args_after_dashes() {
        let usage = Command::Watch.meta().usage;
        assert!(!usage.contains("--app-args"), "{usage}");
        assert!(usage.contains("-- bazel-options|app-args ..."), "{usage}");
    }

    #[test]
    fn value_sets_and_man_parity_are_pinned() {
        use clap::ValueEnum;
        let bad_output = super::super::ArgsError::BadOutput {
            value: "yaml".to_owned(),
        }
        .to_string();
        assert!(
            bad_output.contains("text|json"),
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
            .filter(|command| matches!(command, Command::Completion | Command::Watch))
            .map(|command| (command.name(), command.required_slot()))
            .collect();
        required.sort_unstable();
        assert_eq!(
            required,
            vec![
                ("completion", Some("<shell>")),
                (
                    "watch",
                    Some("<build|test|run|lint|typecheck|format|check>")
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
        assert_eq!(verbs, ["bench", "build", "coverage", "run", "test"]);
        assert!(WorkflowVerb::Test.collects_reports());
        assert!(WorkflowVerb::Coverage.collects_reports());
        assert!(!WorkflowVerb::Build.collects_reports());
        assert!(!WorkflowVerb::Run.collects_reports());
    }
}
