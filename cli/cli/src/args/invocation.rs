use dx_output::{dx_text_visible, ColorMode, LogLevel, OutputMode, Threshold};

use super::profile::{resolve_profile, Profile};
use super::Command;

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
    Plan,
}

impl OperationMode {
    pub fn name(self) -> &'static str {
        match self {
            OperationMode::Check => "check",
            OperationMode::Apply => "apply",
            OperationMode::Plan => "plan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub command: Command,
    pub check: bool,
    pub strict_evidence: bool,
    pub run_output: Option<String>,
    pub apply: bool,
    pub debug: bool,
    pub release: bool,
    pub workspace: Option<String>,
    pub dry_run: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub log_level: Option<LogLevel>,
    pub color: ColorMode,
    pub output: OutputMode,
    pub reports: Vec<ReportRequest>,
    pub fail_on: Threshold,
    pub min_coverage: Option<u32>,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub bazel_clean: bool,
    pub prune_unobserved: bool,
    pub pin: Option<String>,
    pub rollback: bool,
    pub configured: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub here: bool,
    pub serve: bool,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub open: bool,
    /// The network policy: no rules_dx-controlled network acquisition when set.
    pub offline: bool,
    /// The resolution policy: no manifest or lock resolution changes when set.
    pub frozen: bool,
    pub workspace_capabilities: bool,
    pub cases: bool,
    pub bazel_startup_options: Vec<String>,
}

/// Shared output and workspace policy below command dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonOptions {
    pub quiet: bool,
    pub verbose: bool,
    pub log_level: Option<LogLevel>,
    pub color: ColorMode,
    pub output: OutputMode,
    pub reports: Vec<ReportRequest>,
    pub workspace: Option<String>,
    pub bazel_startup_options: Vec<String>,
}

impl CommonOptions {
    pub fn from_invocation(invocation: &Invocation) -> Self {
        Self {
            quiet: invocation.quiet,
            verbose: invocation.verbose,
            log_level: invocation.log_level,
            color: invocation.color,
            output: invocation.output,
            reports: invocation.reports.clone(),
            workspace: invocation.workspace.clone(),
            bazel_startup_options: invocation.bazel_startup_options.clone(),
        }
    }

    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
    }
}

/// Validated quality invocation for lint, typecheck, or format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityRequest {
    pub command: Command,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub fail_on: Threshold,
    pub dry_run: bool,
    pub apply: bool,
    pub check: bool,
    pub common: CommonOptions,
}

/// Validated generate invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateRequest {
    pub command: Command,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub dry_run: bool,
    pub apply: bool,
    pub check: bool,
    pub common: CommonOptions,
}

/// Validated umbrella invocation for check or fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmbrellaRequest {
    pub command: Command,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub fail_on: Threshold,
    pub dry_run: bool,
    pub apply: bool,
    pub check: bool,
    pub common: CommonOptions,
}

/// Typed composition request for the first migrated family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedRequest {
    Quality(QualityRequest),
    Generate(GenerateRequest),
    Umbrella(UmbrellaRequest),
}

fn unsupported_for(command: Command, flag: &str) -> String {
    format!("option {flag:?} is not supported by dx {}", command.name())
}

fn reject_unrelated_for_quality(invocation: &Invocation) -> Result<(), String> {
    let command = invocation.command;
    if invocation.strict_evidence {
        return Err(unsupported_for(command, "--strict-evidence"));
    }
    if invocation.run_output.is_some() {
        return Err(unsupported_for(command, "--run-output"));
    }
    if invocation.debug {
        return Err(unsupported_for(command, "--debug"));
    }
    if invocation.release {
        return Err(unsupported_for(command, "--release"));
    }
    if invocation.min_coverage.is_some() {
        return Err(unsupported_for(command, "--min-coverage"));
    }
    if invocation.bazel_clean {
        return Err(unsupported_for(command, "--bazel"));
    }
    if invocation.prune_unobserved {
        return Err(unsupported_for(command, "--prune-unobserved"));
    }
    if invocation.pin.is_some() {
        return Err(unsupported_for(command, "--pin"));
    }
    if invocation.rollback {
        return Err(unsupported_for(command, "--rollback"));
    }
    if invocation.configured {
        return Err(unsupported_for(command, "--configured"));
    }
    if invocation.from.is_some() {
        return Err(unsupported_for(command, "--from"));
    }
    if invocation.to.is_some() {
        return Err(unsupported_for(command, "--to"));
    }
    if invocation.here {
        return Err("option \"--here/--cwd\" needs cwd resolution".to_owned());
    }
    if invocation.serve {
        return Err(unsupported_for(command, "--serve"));
    }
    if invocation.port.is_some() {
        return Err(unsupported_for(command, "--port"));
    }
    if invocation.host.is_some() {
        return Err(unsupported_for(command, "--host"));
    }
    if invocation.open {
        return Err(unsupported_for(command, "--open"));
    }
    if invocation.offline {
        return Err(unsupported_for(command, "--offline"));
    }
    if invocation.frozen {
        return Err(unsupported_for(command, "--frozen"));
    }
    if invocation.workspace_capabilities {
        return Err(unsupported_for(command, "--workspace-capabilities"));
    }
    if invocation.cases {
        return Err(unsupported_for(command, "--cases"));
    }
    Ok(())
}

impl QualityRequest {
    pub fn from_invocation(invocation: &Invocation) -> Result<Self, String> {
        match invocation.command {
            Command::Lint | Command::Typecheck | Command::Format => {}
            _ => {
                return Err(format!(
                    "option {:?} is not a quality command",
                    invocation.command.name()
                ));
            }
        }
        reject_unrelated_for_quality(invocation)?;
        Ok(Self {
            command: invocation.command,
            targets: invocation.targets.clone(),
            bazel_options: invocation.bazel_options.clone(),
            fail_on: invocation.fail_on,
            dry_run: invocation.dry_run,
            apply: invocation.apply,
            check: invocation.check,
            common: CommonOptions::from_invocation(invocation),
        })
    }

    pub fn operation(&self) -> OperationMode {
        if self.apply {
            OperationMode::Apply
        } else if self.dry_run {
            OperationMode::Plan
        } else {
            OperationMode::Check
        }
    }

    pub fn applies(&self) -> bool {
        self.operation() == OperationMode::Apply
    }

    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }
}

impl GenerateRequest {
    pub fn from_invocation(invocation: &Invocation) -> Result<Self, String> {
        if invocation.command != Command::Generate {
            return Err(format!(
                "option {:?} is not a generate command",
                invocation.command.name()
            ));
        }
        reject_unrelated_for_quality(invocation)?;
        if invocation.fail_on != Threshold::Warning {
            return Err(unsupported_for(invocation.command, "--fail-on"));
        }
        Ok(Self {
            command: invocation.command,
            targets: invocation.targets.clone(),
            bazel_options: invocation.bazel_options.clone(),
            dry_run: invocation.dry_run,
            apply: invocation.apply,
            check: invocation.check,
            common: CommonOptions::from_invocation(invocation),
        })
    }

    pub fn operation(&self) -> OperationMode {
        if self.apply {
            OperationMode::Apply
        } else if self.dry_run {
            OperationMode::Plan
        } else {
            OperationMode::Check
        }
    }

    pub fn applies(&self) -> bool {
        self.operation() == OperationMode::Apply
    }

    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }
}

impl UmbrellaRequest {
    pub fn from_invocation(invocation: &Invocation) -> Result<Self, String> {
        match invocation.command {
            Command::Check | Command::Fix => {}
            _ => {
                return Err(format!(
                    "option {:?} is not an umbrella command",
                    invocation.command.name()
                ));
            }
        }
        reject_unrelated_for_quality(invocation)?;
        Ok(Self {
            command: invocation.command,
            targets: invocation.targets.clone(),
            bazel_options: invocation.bazel_options.clone(),
            fail_on: invocation.fail_on,
            dry_run: invocation.dry_run,
            apply: invocation.apply,
            check: invocation.check,
            common: CommonOptions::from_invocation(invocation),
        })
    }

    pub fn fix_apply(&self) -> bool {
        self.command == Command::Fix && self.apply
    }

    pub fn phase_check(&self) -> bool {
        !self.fix_apply()
    }

    pub fn operation(&self) -> OperationMode {
        if self.apply {
            OperationMode::Apply
        } else if self.dry_run {
            OperationMode::Plan
        } else {
            OperationMode::Check
        }
    }

    pub fn applies(&self) -> bool {
        self.operation() == OperationMode::Apply
    }

    pub fn quality_for(&self, phase: Command, reports: Vec<ReportRequest>) -> QualityRequest {
        let mut common = self.common.clone();
        common.reports = reports;
        QualityRequest {
            command: phase,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            fail_on: self.fail_on,
            dry_run: self.dry_run,
            apply: self.apply,
            check: self.phase_check(),
            common,
        }
    }

    pub fn generate_for(&self, reports: Vec<ReportRequest>) -> GenerateRequest {
        let mut common = self.common.clone();
        common.reports = reports;
        GenerateRequest {
            command: Command::Generate,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            dry_run: self.dry_run,
            apply: self.apply,
            check: self.phase_check(),
            common,
        }
    }

    pub fn verify_quality_for(&self, phase: Command) -> QualityRequest {
        QualityRequest {
            command: phase,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            fail_on: self.fail_on,
            dry_run: false,
            apply: false,
            check: true,
            common: CommonOptions {
                reports: Vec::new(),
                ..self.common.clone()
            },
        }
    }

    pub fn verify_generate_for(&self) -> GenerateRequest {
        GenerateRequest {
            command: Command::Generate,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            dry_run: false,
            apply: false,
            check: true,
            common: CommonOptions {
                reports: Vec::new(),
                ..self.common.clone()
            },
        }
    }
}

impl TypedRequest {
    pub fn from_invocation(invocation: &Invocation) -> Result<Self, String> {
        match invocation.command {
            Command::Lint | Command::Typecheck | Command::Format => Ok(TypedRequest::Quality(
                QualityRequest::from_invocation(invocation)?,
            )),
            Command::Generate => Ok(TypedRequest::Generate(GenerateRequest::from_invocation(
                invocation,
            )?)),
            Command::Check | Command::Fix => Ok(TypedRequest::Umbrella(
                UmbrellaRequest::from_invocation(invocation)?,
            )),
            _ => Err(format!(
                "option {:?} is not in the first typed family",
                invocation.command.name()
            )),
        }
    }
}

impl Invocation {
    pub fn mode(self) -> &'static str {
        if self.check {
            "check"
        } else {
            "default"
        }
    }

    /// The operation the flags authorize: explicit `--apply` opts into
    /// mutation, `--dry-run` plans without running, everything else checks.
    pub fn operation(&self) -> OperationMode {
        if self.apply {
            OperationMode::Apply
        } else if self.dry_run {
            OperationMode::Plan
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

    /// Reports whether chatty dx text prints: text output, no `--quiet`.
    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
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
    if !invocation.command.supports_here() {
        return Err(format!(
            "option \"--here\" is not supported by dx {}",
            invocation.command.name()
        ));
    }
    let scope = here_scope(workspace, cwd)?;
    let mut next = invocation.clone();
    if !next.targets.is_empty() {
        return Err("option \"--here/--cwd\" cannot be combined with explicit scopes".to_owned());
    }
    next.targets = vec![scope];
    next.here = false;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation_for(command: Command, targets: &[&str]) -> Invocation {
        use dx_output::{ColorMode, OutputMode, Threshold};
        Invocation {
            command,
            check: false,
            strict_evidence: false,
            run_output: None,
            apply: false,
            debug: false,
            release: false,
            workspace: None,
            dry_run: false,
            quiet: false,
            verbose: false,
            log_level: None,
            color: ColorMode::Auto,
            output: OutputMode::Text { quiet: false },
            reports: Vec::new(),
            fail_on: Threshold::Warning,
            min_coverage: None,
            targets: targets.iter().map(ToString::to_string).collect(),
            bazel_options: Vec::new(),
            bazel_clean: false,
            prune_unobserved: false,
            pin: None,
            rollback: false,
            configured: false,
            from: None,
            to: None,
            here: true,
            serve: false,
            port: None,
            host: None,
            open: false,
            offline: false,
            frozen: false,
            workspace_capabilities: false,
            cases: false,
            bazel_startup_options: Vec::new(),
        }
    }

    #[test]
    fn operation_authorizes_only_explicit_apply() {
        let bare = invocation_for(Command::Fix, &[]);
        assert!(!bare.apply);
        assert!(!bare.check);
        assert!(!bare.dry_run);
        assert_eq!(bare.operation(), OperationMode::Check);
        assert!(!bare.applies());
        let mut check = invocation_for(Command::Fix, &[]);
        check.check = true;
        assert_eq!(check.operation(), OperationMode::Check);
        assert!(!check.applies());
        let mut plan = invocation_for(Command::Fix, &[]);
        plan.dry_run = true;
        assert_eq!(plan.operation(), OperationMode::Plan);
        assert!(!plan.applies());
        let mut apply = invocation_for(Command::Fix, &[]);
        apply.apply = true;
        assert_eq!(apply.operation(), OperationMode::Apply);
        assert!(apply.applies());
        assert_eq!(OperationMode::Check.name(), "check");
        assert_eq!(OperationMode::Apply.name(), "apply");
        assert_eq!(OperationMode::Plan.name(), "plan");
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
        let aliased = crate::args::parse(
            &["lint", "--cwd"]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .expect("cwd alias parses");
        assert!(aliased.here, "--cwd must set here");
    }

    fn parsed(words: &[&str]) -> Invocation {
        crate::args::parse(&crate::test_support::strings(words)).expect("parse")
    }

    #[test]
    fn quality_request_accepts_quality_commands() {
        for command in ["lint", "typecheck", "format"] {
            let invocation = parsed(&[command, "//a:one", "--fail-on=error"]);
            let request = QualityRequest::from_invocation(&invocation).expect("quality validates");
            assert_eq!(request.command.name(), command);
            assert_eq!(request.targets, vec!["//a:one".to_owned()]);
            assert_eq!(request.fail_on, dx_output::Threshold::Error);
            assert!(!request.applies());
            assert!(request.chatty());
        }
        let lint = parsed(&["lint"]);
        let request = QualityRequest::from_invocation(&lint).expect("bare lint validates");
        assert!(request.targets.is_empty());
        assert_eq!(request.operation(), OperationMode::Check);
    }

    #[test]
    fn quality_request_rejects_unrelated_flags() {
        let base = parsed(&["lint"]);
        for mutate in [
            "--pin",
            "--rollback",
            "--from",
            "--to",
            "--offline",
            "--frozen",
            "--debug",
            "--strict-evidence",
            "--min-coverage",
            "--cases",
        ] as [&str; 10]
        {
            let mut tainted = base.clone();
            match mutate {
                "--pin" => tainted.pin = Some("1.2.3".to_owned()),
                "--rollback" => tainted.rollback = true,
                "--from" => tainted.from = Some("1.2.3".to_owned()),
                "--to" => tainted.to = Some("2.0.0".to_owned()),
                "--offline" => tainted.offline = true,
                "--frozen" => tainted.frozen = true,
                "--debug" => tainted.debug = true,
                "--strict-evidence" => tainted.strict_evidence = true,
                "--min-coverage" => tainted.min_coverage = Some(90),
                "--cases" => tainted.cases = true,
                _ => unreachable!(),
            }
            let error = QualityRequest::from_invocation(&tainted).expect_err("unrelated must fail");
            assert!(error.contains(mutate), "{mutate} must name itself: {error}");
            assert!(error.contains("lint"), "{error}");
        }
        let mut wrong = base;
        wrong.command = Command::Build;
        assert!(QualityRequest::from_invocation(&wrong).is_err());
        let mut generate = parsed(&["generate"]);
        generate.fail_on = dx_output::Threshold::Error;
        assert!(GenerateRequest::from_invocation(&generate).is_err());
    }

    #[test]
    fn umbrella_request_derives_phases_without_unrelated() {
        let invocation = parsed(&["check", "//a:one", "--fail-on=error"]);
        let request = UmbrellaRequest::from_invocation(&invocation).expect("umbrella validates");
        assert_eq!(request.command, Command::Check);
        assert!(request.phase_check());
        assert!(!request.fix_apply());
        let phase = request.quality_for(Command::Lint, Vec::new());
        assert_eq!(phase.command, Command::Lint);
        assert_eq!(phase.targets, vec!["//a:one".to_owned()]);
        assert_eq!(phase.fail_on, dx_output::Threshold::Error);
        assert!(phase.check);
        assert!(!phase.apply);
        let generated = request.generate_for(Vec::new());
        assert_eq!(generated.command, Command::Generate);
        assert_eq!(generated.targets, vec!["//a:one".to_owned()]);
        let fix = parsed(&["fix", "--apply", "//a:one"]);
        let fix_request = UmbrellaRequest::from_invocation(&fix).expect("fix validates");
        assert!(fix_request.fix_apply());
        assert!(!fix_request.phase_check());
        let fix_phase = fix_request.quality_for(Command::Format, Vec::new());
        assert!(!fix_phase.check);
        assert!(fix_phase.apply);
        let verify = request.verify_quality_for(Command::Lint);
        assert!(verify.check);
        assert!(!verify.apply);
        assert!(!verify.dry_run);
        assert!(verify.common.reports.is_empty());
        let verify_generate = request.verify_generate_for();
        assert_eq!(verify_generate.command, Command::Generate);
        assert!(verify_generate.check);
    }

    #[test]
    fn typed_request_dispatches_first_family_only() {
        for words in [&["lint"][..], &["generate"][..], &["check"][..]] {
            let invocation = parsed(words);
            assert!(TypedRequest::from_invocation(&invocation).is_ok());
        }
        let build = parsed(&["build", "//..."]);
        let error = TypedRequest::from_invocation(&build).expect_err("build is later");
        assert!(error.contains("first typed family"), "{error}");
    }
}
