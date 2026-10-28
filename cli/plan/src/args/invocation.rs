use std::path::PathBuf;

use dx_output::{dx_text_visible, ColorMode, OutputMode, Threshold};

use super::profile::{resolve_profile, Profile};
use super::Command;

/// Typed marker that an invocation was handed off from an affected plan,
/// carrying the planned test targets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AffectedHandoff {
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRequest {
    pub format: String,
    pub destination: String,
}

/// How the invocation may touch the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Check,
    Apply,
}

impl OperationMode {
    pub fn name(self) -> &'static str {
        match self {
            OperationMode::Check => "check",
            OperationMode::Apply => "apply",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonOptions {
    pub output: OutputMode,
    pub include_edits: bool,
    pub reports: Vec<ReportRequest>,
    pub quiet: bool,
    pub verbose: bool,
    pub color: ColorMode,
}

/// The scope a dx check or dx fix run was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintScope {
    Affected,
    Targets(Vec<String>),
}

/// The umbrella command and scope a phase runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmbrellaOrigin {
    pub command: Command,
    pub scope: HintScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityRequest {
    pub command: Command,
    pub mode: OperationMode,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub fail_on: Threshold,
    pub baseline: Option<String>,
    pub origin: Option<UmbrellaOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateRequest {
    pub mode: OperationMode,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmbrellaRequest {
    pub command: Command,
    pub mode: OperationMode,
    pub apply: bool,
    pub targets: Vec<String>,
    pub scope: HintScope,
    pub generate_targets: Vec<String>,
    pub test: Option<Box<Invocation>>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub fail_on: Threshold,
    pub baseline: Option<String>,
}

impl CommonOptions {
    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
    }
}

impl QualityRequest {
    pub fn operation(&self) -> OperationMode {
        self.mode
    }

    pub fn applies(&self) -> bool {
        self.mode == OperationMode::Apply
    }

    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }
}

impl GenerateRequest {
    pub fn operation(&self) -> OperationMode {
        self.mode
    }

    pub fn applies(&self) -> bool {
        self.mode == OperationMode::Apply
    }

    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }
}

impl UmbrellaRequest {
    pub fn operation(&self) -> OperationMode {
        self.mode
    }

    pub fn applies(&self) -> bool {
        self.mode == OperationMode::Apply
    }

    pub fn phase_quality_request(
        &self,
        phase: Command,
        reports: Vec<ReportRequest>,
    ) -> Result<QualityRequest, String> {
        self.quality_request(phase, self.mode, reports)
    }

    pub fn phase_generate_request(&self, reports: Vec<ReportRequest>) -> GenerateRequest {
        self.generate_request(self.mode, reports)
    }

    pub fn verify_quality_request(&self, phase: Command) -> Result<QualityRequest, String> {
        self.quality_request(phase, OperationMode::Check, Vec::new())
    }

    pub fn verify_generate_request(&self) -> GenerateRequest {
        self.generate_request(OperationMode::Check, Vec::new())
    }

    fn origin(&self) -> UmbrellaOrigin {
        UmbrellaOrigin {
            command: self.command,
            scope: self.scope.clone(),
        }
    }

    fn common_with(&self, reports: Vec<ReportRequest>) -> CommonOptions {
        CommonOptions {
            output: self.common.output,
            include_edits: self.common.include_edits,
            reports,
            quiet: self.common.quiet,
            verbose: self.common.verbose,
            color: self.common.color,
        }
    }

    fn quality_request(
        &self,
        phase: Command,
        mode: OperationMode,
        reports: Vec<ReportRequest>,
    ) -> Result<QualityRequest, String> {
        if !matches!(phase, Command::Lint | Command::Typecheck | Command::Format) {
            return Err(format!(
                "umbrella phase {} is not a quality request",
                phase.name()
            ));
        }
        Ok(QualityRequest {
            command: phase,
            mode,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_with(reports),
            fail_on: self.fail_on,
            baseline: self.baseline.clone(),
            origin: Some(self.origin()),
        })
    }

    fn generate_request(
        &self,
        mode: OperationMode,
        reports: Vec<ReportRequest>,
    ) -> GenerateRequest {
        GenerateRequest {
            mode,
            targets: self.generate_targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_with(reports),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub command: Command,
    pub check: bool,
    pub apply: bool,
    pub debug: bool,
    pub release: bool,
    pub workspace: Option<String>,
    pub quiet: bool,
    pub verbose: bool,
    pub color: ColorMode,
    pub output: OutputMode,
    pub reports: Vec<ReportRequest>,
    pub fail_on: Threshold,
    /// How many build slots this process sizes its gate with.
    pub build_slots: dx_adopt::defaults::BuildSlots,
    pub baseline: Option<String>,
    pub include_edits: bool,
    pub min_coverage: Option<u32>,
    /// The `--min-changed-line-coverage` percent gate on lines changed since the merge-base.
    pub min_changed_line_coverage: Option<u32>,
    pub targets: Vec<String>,
    pub(super) bazel_options: Vec<String>,
    pub(super) app_args: Vec<String>,
    pub prune_unobserved: bool,
    pub configured: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub here: bool,
    pub affected: bool,
    pub affected_handoff: Option<AffectedHandoff>,
    pub serve: bool,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub open: bool,
    /// The network policy: no rules_dx-controlled network acquisition when set.
    pub offline: bool,
    /// The resolution policy: no manifest or lock resolution changes when set.
    pub frozen: bool,
    /// `dx bump --floor`: edit the bazel_dep floor instead of the tested override.
    pub floor: bool,
    /// `dx bump --reason`: why the floor is the oldest supported version.
    pub reason: Option<String>,
    pub cases: bool,
    pub create: Option<String>,
    pub force: bool,
    pub disk: bool,
    pub bases: bool,
    /// The optional `dx init` parts `--with` or `--full` selected.
    pub init_parts: dx_adopt::InitParts,
    /// Extra Rust target triples `--rust-target-triple` adds to the init snippet.
    pub init_rust_target_triples: Vec<String>,
    /// `build`/`test --explain`: save the execution log for `dx explain`.
    pub explain: bool,
    /// `dx explain --old`.
    pub old_log: Option<String>,
    /// `dx explain --new`.
    pub new_log: Option<String>,
    /// `dx install --tag`.
    pub install_tag: Option<String>,
    /// `dx install --archive`, as typed (the CLI resolves it from the invocation cwd).
    pub install_archive: Option<PathBuf>,
    /// `dx install --dir`, absolute (resolved from the invocation cwd).
    pub install_dir: Option<PathBuf>,
    pub path_only: bool,
    pub agent: bool,
    pub bazel_startup_options: Vec<String>,
}

pub const AGENT_QUIET_FLAGS: [&str; 6] = [
    "--noshow_progress",
    "--noshow_loading_progress",
    "--noshow_timestamps",
    "--curses=no",
    "--ui_event_filters=-info,-debug",
    "--test_summary=none",
];

impl Invocation {
    /// Bazel UI flags that silence progress output in agent JSON sessions.
    pub fn agent_quiet_flags(&self) -> Vec<String> {
        if self.agent && self.output == OutputMode::Json {
            AGENT_QUIET_FLAGS
                .iter()
                .map(|flag| (*flag).to_owned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Bazel options for the run: agent JSON sessions get no progress output.
    pub fn quiet_bazel_options(&self) -> Vec<String> {
        let mut options = self.agent_quiet_flags();
        options.extend(self.bazel_options.iter().cloned());
        options
    }

    /// Bazel options as typed, without agent quiet flags; not for build argv.
    pub fn raw_bazel_options(&self) -> &[String] {
        &self.bazel_options
    }

    /// The `--` payload `dx run` and `dx deploy` hand to the app.
    pub fn app_args(&self) -> &[String] {
        &self.app_args
    }

    pub fn mode(self) -> &'static str {
        if self.check {
            "check"
        } else {
            "default"
        }
    }

    /// The `command_started` mode: check unless `--apply` authorizes writing.
    pub fn started_mode(&self) -> &'static str {
        let check_only = self.command.supports_apply() && !self.applies();
        if self.command == Command::Check || self.check || check_only {
            "check"
        } else {
            "default"
        }
    }

    /// The operation the flags authorize: explicit `--apply` opts into mutation, else check.
    pub fn operation(&self) -> OperationMode {
        if self.apply {
            OperationMode::Apply
        } else {
            OperationMode::Check
        }
    }

    /// Whether the flags authorize writing sources: explicit `--apply` only.
    pub fn applies(&self) -> bool {
        self.operation() == OperationMode::Apply
    }

    pub fn profile_flag(&self) -> Option<Profile> {
        if self.debug {
            Some(Profile::Debug)
        } else if self.release {
            Some(Profile::Release)
        } else {
            None
        }
    }

    pub fn profile(&self) -> Profile {
        resolve_profile(
            self.profile_flag(),
            None,
            Profile::default_for(self.command),
        )
    }

    /// The startup flags and profile config every update Bazel call starts with.
    pub fn bazel_head(&self) -> dx_process::BazelHead {
        dx_process::BazelHead {
            startup: self.bazel_startup_options.clone(),
            options: vec![self.profile().config_flag()],
            settings: crate::plan::head_settings(crate::plan::rules_dx_repo()),
        }
    }

    /// Reports whether chatty dx text prints: text output, no `--quiet`.
    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
    }

    pub fn common_options(&self) -> CommonOptions {
        CommonOptions {
            output: self.output,
            include_edits: self.include_edits,
            reports: self.reports.clone(),
            quiet: self.quiet,
            verbose: self.verbose,
            color: self.color,
        }
    }

    pub fn quality_request(&self) -> Result<QualityRequest, String> {
        if !matches!(
            self.command,
            Command::Lint | Command::Typecheck | Command::Format
        ) {
            return Err(format!(
                "option \"--command\" is not supported by dx {}: want lint|typecheck|format",
                self.command.name()
            ));
        }
        Ok(QualityRequest {
            command: self.command,
            mode: self.operation(),
            targets: self.targets.clone(),
            bazel_options: self.quiet_bazel_options(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            fail_on: self.fail_on,
            baseline: self.baseline.clone(),
            origin: None,
        })
    }

    pub fn generate_request(&self) -> Result<GenerateRequest, String> {
        if self.command != Command::Generate {
            return Err(format!(
                "option \"--command\" is not supported by dx {}: want generate",
                self.command.name()
            ));
        }
        Ok(GenerateRequest {
            mode: self.operation(),
            targets: self.targets.clone(),
            bazel_options: self.quiet_bazel_options(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
        })
    }

    /// Reports whether the targets came from an affected plan.
    pub fn is_affected_plan(&self) -> bool {
        self.affected_handoff.is_some()
    }

    pub fn umbrella_request(&self) -> Result<UmbrellaRequest, String> {
        if !matches!(self.command, Command::Check | Command::Fix) {
            return Err(format!(
                "option \"--command\" is not supported by dx {}: want check|fix",
                self.command.name()
            ));
        }
        let affected = self.is_affected_plan();
        let test = match (&self.affected_handoff, self.command) {
            (Some(AffectedHandoff { tests }), Command::Check) if !tests.is_empty() => {
                let mut test = self.clone();
                test.command = Command::Test;
                test.check = false;
                test.apply = false;
                test.reports = Vec::new();
                test.include_edits = false;
                test.targets = tests.clone();
                test.affected_handoff = None;
                Some(Box::new(test))
            }
            _ => None,
        };
        Ok(UmbrellaRequest {
            command: self.command,
            mode: self.operation(),
            apply: self.apply,
            targets: self.targets.clone(),
            scope: if affected {
                HintScope::Affected
            } else {
                HintScope::Targets(self.targets.clone())
            },
            generate_targets: if affected {
                crate::plan::generate_package_scope(&self.targets)
            } else {
                self.targets.clone()
            },
            test,
            bazel_options: self.quiet_bazel_options(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            fail_on: self.fail_on,
            baseline: self.baseline.clone(),
        })
    }
}

pub fn here_scope(workspace: &std::path::Path, cwd: &std::path::Path) -> Result<String, String> {
    use std::path::Component;
    let rel = cwd.strip_prefix(workspace).map_err(|_| {
        format!(
            "current directory {} is outside workspace {}: --here needs a directory under the workspace",
            cwd.display(),
            workspace.display()
        )
    })?;
    if rel.as_os_str().is_empty() {
        return Ok("//...".to_owned());
    }
    let mut parts: Vec<String> = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str().ok_or_else(|| {
                    format!(
                        "current directory {} is not valid UTF-8: --here needs a UTF-8 path",
                        cwd.display()
                    )
                })?;
                for piece in dx_path::posix(std::path::Path::new(&text)).split('/') {
                    if !piece.is_empty() && piece != "." {
                        parts.push(piece.to_owned());
                    }
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(format!(
                    "current directory {} is outside workspace {}: --here needs a directory under the workspace",
                    cwd.display(),
                    workspace.display()
                ));
            }
        }
    }
    if parts.is_empty() {
        return Ok("//...".to_owned());
    }
    Ok(parts.join("/"))
}

pub fn apply_here(
    invocation: &Invocation,
    workspace: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<Invocation, String> {
    if !invocation.here {
        return Ok(invocation.clone());
    }
    let (owner, at) = here_owner(invocation);
    if !owner.supports_here() {
        return Err(format!(
            "option \"--here\" is not supported by dx {}",
            owner.name()
        ));
    }
    let scope = here_scope(workspace, cwd)?;
    let mut next = invocation.clone();
    if next.targets.len() > at {
        return Err("option \"--here\" cannot be combined with explicit scopes".to_owned());
    }
    next.targets.truncate(at);
    next.targets.push(scope);
    next.here = false;
    Ok(next)
}

/// The command that owns `--here` and the index where its scopes start.
fn here_owner(invocation: &Invocation) -> (Command, usize) {
    if invocation.command == Command::Watch {
        if let Some(wrapped) = invocation
            .targets
            .first()
            .map(String::as_str)
            .and_then(Command::parse)
        {
            return (wrapped, 1);
        }
    }
    (invocation.command, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_bump_and_add_heads_carry_startup_and_the_default_profile() {
        for command in [Command::Update, Command::Bump, Command::Add] {
            let mut invocation = invocation_for(command, &[]);
            invocation.bazel_startup_options = vec!["--output_base=/ob".to_owned()];
            let head = invocation.bazel_head();
            assert_eq!(head.startup, vec!["--output_base=/ob".to_owned()]);
            assert_eq!(head.options, vec!["--config=dx_dev".to_owned()]);
            assert_eq!(head.settings, crate::plan::shared_settings("build"));
            assert!(!head.settings.is_empty());
        }
    }

    fn invocation_for(command: Command, targets: &[&str]) -> Invocation {
        use dx_output::{ColorMode, OutputMode, Threshold};
        Invocation {
            command,
            check: false,
            apply: false,
            debug: false,
            release: false,
            workspace: None,
            quiet: false,
            verbose: false,
            color: ColorMode::Auto,
            output: OutputMode::Text { quiet: false },
            reports: Vec::new(),
            fail_on: Threshold::Warning,
            build_slots: dx_adopt::defaults::BuildSlots::Auto,
            baseline: None,
            include_edits: false,
            min_coverage: None,
            min_changed_line_coverage: None,
            targets: targets.iter().map(ToString::to_string).collect(),
            bazel_options: Vec::new(),
            app_args: Vec::new(),
            prune_unobserved: false,
            configured: false,
            from: None,
            to: None,
            here: true,
            affected: false,
            affected_handoff: None,
            serve: false,
            port: None,
            host: None,
            open: false,
            offline: false,
            frozen: false,
            floor: false,
            reason: None,
            cases: false,
            create: None,
            force: false,
            disk: false,
            bases: false,
            init_parts: dx_adopt::InitParts::NONE,
            init_rust_target_triples: Vec::new(),
            explain: false,
            old_log: None,
            new_log: None,
            install_tag: None,
            install_archive: None,
            install_dir: None,
            path_only: false,
            agent: false,
            bazel_startup_options: Vec::new(),
        }
    }

    #[test]
    fn operation_authorizes_only_explicit_apply() {
        let bare = invocation_for(Command::Fix, &[]);
        assert!(!bare.apply);
        assert!(!bare.check);
        assert_eq!(bare.operation(), OperationMode::Check);
        assert!(!bare.applies());
        let mut check = invocation_for(Command::Fix, &[]);
        check.check = true;
        assert_eq!(check.operation(), OperationMode::Check);
        assert!(!check.applies());
        let mut apply = invocation_for(Command::Fix, &[]);
        apply.apply = true;
        assert_eq!(apply.operation(), OperationMode::Apply);
        assert!(apply.applies());
        assert_eq!(OperationMode::Check.name(), "check");
        assert_eq!(OperationMode::Apply.name(), "apply");
    }

    #[test]
    fn umbrella_phases_carry_the_user_scope() {
        let mut affected = invocation_for(Command::Check, &["//a:x", "//a:y"]);
        affected.affected_handoff = Some(AffectedHandoff::default());
        let request = affected.umbrella_request().expect("umbrella");
        assert_eq!(request.scope, HintScope::Affected);
        let phase = request
            .phase_quality_request(Command::Format, Vec::new())
            .expect("phase");
        assert_eq!(
            phase.origin,
            Some(UmbrellaOrigin {
                command: Command::Check,
                scope: HintScope::Affected,
            })
        );
        assert_eq!(phase.targets, vec!["//a:x".to_owned(), "//a:y".to_owned()]);
        assert_eq!(
            request.phase_generate_request(Vec::new()).targets,
            vec!["//a/...".to_owned()]
        );
        assert_eq!(
            request.verify_generate_request().targets,
            vec!["//a/...".to_owned()]
        );

        let mut fix = invocation_for(Command::Fix, &["//a/..."]);
        fix.apply = true;
        let request = fix.umbrella_request().expect("umbrella");
        assert_eq!(
            request.scope,
            HintScope::Targets(vec!["//a/...".to_owned()])
        );
        let verify = request
            .verify_quality_request(Command::Lint)
            .expect("verify");
        assert_eq!(
            verify.origin.map(|origin| origin.command),
            Some(Command::Fix)
        );

        let direct = invocation_for(Command::Lint, &["//a/..."]);
        assert_eq!(direct.quality_request().expect("quality").origin, None);
    }

    #[test]
    fn affected_fix_scopes_generate_apply_and_verify_to_packages() {
        let mut fix = invocation_for(Command::Fix, &["//a:x", "//b:y"]);
        fix.apply = true;
        fix.affected_handoff = Some(AffectedHandoff::default());
        let request = fix.umbrella_request().expect("umbrella");
        let want = vec!["//a/...".to_owned(), "//b/...".to_owned()];
        let phase = request.phase_generate_request(Vec::new());
        assert_eq!(phase.mode, OperationMode::Apply);
        assert_eq!(phase.targets, want);
        let verify = request.verify_generate_request();
        assert_eq!(verify.mode, OperationMode::Check);
        assert_eq!(verify.targets, want);
    }

    #[test]
    fn affected_repo_scope_keeps_generate_repo_wide() {
        let mut check = invocation_for(Command::Check, &["//..."]);
        check.affected_handoff = Some(AffectedHandoff::default());
        let request = check.umbrella_request().expect("umbrella");
        assert_eq!(
            request.phase_generate_request(Vec::new()).targets,
            vec!["//...".to_owned()]
        );
        assert_eq!(
            request.verify_generate_request().targets,
            vec!["//...".to_owned()]
        );
    }

    #[test]
    fn chatty_needs_plain_text_output_and_no_quiet() {
        assert!(invocation_for(Command::Lint, &[]).chatty());
        for output in [
            OutputMode::Text { quiet: true },
            OutputMode::Diff,
            OutputMode::Json,
        ] {
            let mut got = invocation_for(Command::Lint, &[]);
            got.output = output;
            assert!(!got.chatty(), "{output:?} prints no dx text");
        }
        let mut quiet = invocation_for(Command::Lint, &[]);
        quiet.quiet = true;
        assert!(!quiet.chatty(), "--quiet prints no dx text");
    }

    #[test]
    fn here_scope_maps_cwd_to_directory_spelling() {
        let workspace = std::path::Path::new("/ws");
        assert_eq!(
            here_scope(workspace, std::path::Path::new("/ws")),
            Ok("//...".to_owned())
        );
        assert_eq!(
            here_scope(workspace, std::path::Path::new("/ws/cli/cli")),
            Ok("cli/cli".to_owned())
        );
        assert!(here_scope(workspace, std::path::Path::new("/other")).is_err());
    }

    #[test]
    fn apply_here_under_watch_scopes_the_wrapped_command() {
        let workspace = std::path::Path::new("/ws");
        let subdir = std::path::Path::new("/ws/sub");

        let resolved = apply_here(
            &invocation_for(Command::Watch, &["test"]),
            workspace,
            subdir,
        )
        .expect("resolves");
        assert!(!resolved.here);
        assert_eq!(resolved.command, Command::Watch);
        assert_eq!(resolved.targets, vec!["test".to_owned(), "sub".to_owned()]);

        let refused = apply_here(&invocation_for(Command::Watch, &["run"]), workspace, subdir)
            .expect_err("run takes no --here");
        assert_eq!(refused, "option \"--here\" is not supported by dx run");

        let combined = apply_here(
            &invocation_for(Command::Watch, &["test", "//a:one"]),
            workspace,
            subdir,
        )
        .expect_err("explicit scope");
        assert_eq!(
            combined,
            "option \"--here\" cannot be combined with explicit scopes"
        );
    }

    #[test]
    fn apply_here_consumes_flag_into_explicit_targets() {
        let workspace = std::path::Path::new("/ws");
        let subdir = std::path::Path::new("/ws/cli/cli");
        let root = std::path::Path::new("/ws");

        let resolved =
            apply_here(&invocation_for(Command::Lint, &[]), workspace, subdir).expect("resolves");
        assert!(!resolved.here);
        assert_eq!(resolved.targets, vec!["cli/cli".to_owned()]);

        let resolved =
            apply_here(&invocation_for(Command::Lint, &[]), workspace, root).expect("resolves");
        assert_eq!(resolved.targets, vec!["//...".to_owned()]);

        let resolved = apply_here(&invocation_for(Command::Security, &[]), workspace, subdir)
            .expect("resolves");
        assert_eq!(resolved.targets, vec!["cli/cli".to_owned()]);
        let resolved =
            apply_here(&invocation_for(Command::License, &[]), workspace, root).expect("resolves");
        assert_eq!(resolved.targets, vec!["//...".to_owned()]);

        assert!(apply_here(
            &invocation_for(Command::Lint, &["//a:one"]),
            workspace,
            subdir
        )
        .is_err());
        assert!(apply_here(
            &invocation_for(Command::Security, &["//a:one"]),
            workspace,
            subdir
        )
        .is_err());
        assert!(apply_here(&invocation_for(Command::Clean, &[]), workspace, subdir).is_err());
        assert!(apply_here(
            &invocation_for(Command::Lint, &[]),
            workspace,
            std::path::Path::new("/other")
        )
        .is_err());

        let scratch = dx_test_scratch::scratch("dx-here-bazel-run-");
        let ws = scratch.path().join("ws");
        std::fs::create_dir_all(ws.join("cli/cli")).expect("dirs");
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let resolved = apply_here(
            &invocation_for(Command::Lint, &[]),
            &dx_process::bazel_run_dir(Some(ws.as_os_str()), &runfiles),
            &dx_process::bazel_run_dir(Some(ws.join("cli/cli").as_os_str()), &runfiles),
        )
        .expect("resolves");
        assert_eq!(resolved.targets, vec!["cli/cli".to_owned()]);
        scratch.close().expect("cleanup");

        let mut plain = invocation_for(Command::Lint, &[]);
        plain.here = false;
        let resolved = apply_here(&plain, workspace, subdir).expect("passthrough");
        assert!(resolved.targets.is_empty());
        assert!(!resolved.here);
    }

    #[test]
    fn here_scope_canonicalizes_and_bare_stays_repo_wide() {
        let workspace = std::path::Path::new("/ws");
        assert_eq!(
            here_scope(workspace, std::path::Path::new("/ws")),
            Ok("//...".to_owned())
        );
        assert_eq!(
            here_scope(workspace, std::path::Path::new("/ws/cli/cli")),
            Ok("cli/cli".to_owned())
        );
        assert_eq!(
            here_scope(workspace, std::path::Path::new("/ws/cli/cli/")),
            Ok("cli/cli".to_owned())
        );
        assert_eq!(
            here_scope(workspace, std::path::Path::new("/ws/./cli/cli")),
            Ok("cli/cli".to_owned())
        );
        let mut bare = invocation_for(Command::Lint, &[]);
        bare.here = false;
        let resolved =
            apply_here(&bare, workspace, std::path::Path::new("/ws/cli/cli")).expect("passthrough");
        assert!(
            resolved.targets.is_empty(),
            "bare subdir must stay empty (=//...)"
        );
        assert!(
            crate::args::parse(&["lint".to_owned(), "--cwd".to_owned()]).is_err(),
            "--cwd is not an option"
        );
    }

    fn parsed(words: &[&str]) -> Invocation {
        crate::args::parse(&words.iter().map(ToString::to_string).collect::<Vec<_>>())
            .expect("parse")
    }

    #[test]
    fn quality_request_accepts_only_the_quality_family() {
        for command in ["lint", "typecheck", "format"] {
            let request = parsed(&[command, "--check", "//a:one"])
                .quality_request()
                .expect("quality family");
            assert_eq!(request.command.name(), command);
            assert_eq!(request.mode, OperationMode::Check);
            assert_eq!(request.targets, vec!["//a:one".to_owned()]);
        }
        assert!(parsed(&["check"]).quality_request().is_err());
        assert!(parsed(&["generate"]).quality_request().is_err());
        assert!(parsed(&["build", "//a:one"]).quality_request().is_err());
    }

    #[test]
    fn quality_request_carries_mode_scopes_and_severity() {
        let request = parsed(&[
            "lint",
            "--apply",
            "--fail-on=error",
            "//a:one",
            "--",
            "--jobs=4",
        ])
        .quality_request()
        .expect("apply request");
        assert_eq!(request.mode, OperationMode::Apply);
        assert!(request.applies());
        assert_eq!(request.targets, vec!["//a:one".to_owned()]);
        assert_eq!(request.bazel_options, vec!["--jobs=4".to_owned()]);
        assert_eq!(
            request.fail_on,
            dx_output::Threshold::Error,
            "severity rides the typed request"
        );
    }

    #[test]
    fn baseline_selection_rides_every_quality_request() {
        let defaults = crate::args::FileDefaults {
            baseline: Some("b.json".to_owned()),
            ..Default::default()
        };
        let inv = crate::args::parse_with(&["lint".to_owned()], &|_| None, &defaults)
            .expect("parse lint");
        assert_eq!(
            inv.quality_request().expect("quality").baseline.as_deref(),
            Some("b.json")
        );
        let fix =
            crate::args::parse_with(&["fix".to_owned()], &|_| None, &defaults).expect("parse fix");
        let umbrella = fix.umbrella_request().expect("umbrella");
        let phase = umbrella
            .phase_quality_request(Command::Lint, Vec::new())
            .expect("phase");
        assert_eq!(phase.baseline.as_deref(), Some("b.json"));
        let verify = umbrella
            .verify_quality_request(Command::Lint)
            .expect("verify");
        assert_eq!(verify.baseline.as_deref(), Some("b.json"));
    }

    #[test]
    fn quality_request_has_no_unrelated_fields() {
        let request = parsed(&["lint", "//a:one"])
            .quality_request()
            .expect("request");
        let debug = format!("{request:?}");
        for unrelated in ["serve", "offline", "frozen", "min_coverage", "configured"] {
            assert!(
                !debug.contains(unrelated),
                "typed quality request cannot represent {unrelated}: {debug}"
            );
        }
    }

    #[test]
    fn generate_and_umbrella_requests_validate_their_commands() {
        let generate = parsed(&["generate", "--check"])
            .generate_request()
            .expect("generate");
        assert_eq!(generate.mode, OperationMode::Check);
        assert!(parsed(&["lint"]).generate_request().is_err());
        let umbrella = parsed(&["check", "//..."])
            .umbrella_request()
            .expect("umbrella");
        assert_eq!(umbrella.command, Command::Check);
        assert_eq!(umbrella.targets, vec!["//...".to_owned()]);
        assert!(parsed(&["lint"]).umbrella_request().is_err());
        assert!(parsed(&["generate"]).umbrella_request().is_err());
    }

    fn agent(words: &[&str]) -> Invocation {
        let mut invocation = parsed(words);
        invocation.agent = true;
        invocation
    }

    fn quiet_flags() -> Vec<String> {
        AGENT_QUIET_FLAGS.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn raw_bazel_options_never_carry_agent_quiet_flags() {
        let session = agent(&["build", "--output=json", "--", "--jobs=4"]);
        assert_eq!(session.raw_bazel_options(), ["--jobs=4".to_owned()]);
        let mut want = quiet_flags();
        want.push("--jobs=4".to_owned());
        assert_eq!(session.quiet_bazel_options(), want);
        let plain = parsed(&["build", "--", "--jobs=4"]);
        assert_eq!(plain.raw_bazel_options(), plain.quiet_bazel_options());
    }

    #[test]
    fn agent_quiet_flags_need_an_agent_json_session() {
        assert_eq!(
            agent(&["build", "--output=json"]).agent_quiet_flags(),
            quiet_flags()
        );
        assert!(agent(&["build", "--output=text"])
            .agent_quiet_flags()
            .is_empty());
        assert!(parsed(&["build", "--output=json"])
            .agent_quiet_flags()
            .is_empty());
    }

    #[test]
    fn agent_quiet_generate_request_puts_quiet_flags_before_the_callers_options() {
        let generate = agent(&["generate", "--output=json", "--", "--jobs=4"])
            .generate_request()
            .expect("generate");
        let mut want = quiet_flags();
        want.push("--jobs=4".to_owned());
        assert_eq!(generate.bazel_options, want);
        let plain = parsed(&["generate", "--output=json", "--", "--jobs=4"])
            .generate_request()
            .expect("generate");
        assert_eq!(plain.bazel_options, vec!["--jobs=4".to_owned()]);
    }

    #[test]
    fn agent_quiet_umbrella_generate_phase_gets_the_flags_once() {
        let umbrella = agent(&["check", "--output=json", "//a:one"])
            .umbrella_request()
            .expect("umbrella");
        assert_eq!(
            umbrella.phase_generate_request(Vec::new()).bazel_options,
            quiet_flags()
        );
        assert_eq!(
            umbrella.verify_generate_request().bazel_options,
            quiet_flags()
        );
    }

    #[test]
    fn umbrella_phases_derive_typed_requests_without_resets() {
        let umbrella = parsed(&["fix", "--apply", "--fail-on=error", "//a:one"])
            .umbrella_request()
            .expect("fix apply");
        assert_eq!(umbrella.mode, OperationMode::Apply);
        let phase = umbrella
            .phase_quality_request(Command::Lint, Vec::new())
            .expect("lint phase");
        assert_eq!(phase.command, Command::Lint);
        assert_eq!(phase.mode, OperationMode::Apply);
        assert_eq!(phase.targets, vec!["//a:one".to_owned()]);
        assert_eq!(phase.fail_on, dx_output::Threshold::Error);
        assert!(umbrella
            .phase_quality_request(Command::Generate, Vec::new())
            .is_err());
        let generate = umbrella.phase_generate_request(Vec::new());
        assert_eq!(generate.mode, OperationMode::Apply);
        assert_eq!(generate.targets, vec!["//a:one".to_owned()]);
    }

    #[test]
    fn umbrella_verification_is_always_read_only() {
        let umbrella = parsed(&["fix", "--apply", "//a:one"])
            .umbrella_request()
            .expect("fix apply");
        let recheck = umbrella
            .verify_quality_request(Command::Lint)
            .expect("recheck");
        assert_eq!(recheck.mode, OperationMode::Check);
        assert!(!recheck.applies());
        assert!(recheck.common.reports.is_empty());
        let generate = umbrella.verify_generate_request();
        assert_eq!(generate.mode, OperationMode::Check);
        assert!(!generate.applies());
    }
}
