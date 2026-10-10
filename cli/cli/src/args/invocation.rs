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
pub struct CommonOptions {
    pub output: OutputMode,
    pub reports: Vec<ReportRequest>,
    pub quiet: bool,
    pub verbose: bool,
    pub log_level: Option<LogLevel>,
    pub color: ColorMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityRequest {
    pub command: Command,
    pub mode: OperationMode,
    pub dry_run: bool,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub fail_on: Threshold,
    pub strict_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateRequest {
    pub mode: OperationMode,
    pub dry_run: bool,
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
    pub dry_run: bool,
    pub apply: bool,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub fail_on: Threshold,
    pub strict_evidence: bool,
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
        if !matches!(phase, Command::Lint | Command::Typecheck | Command::Format) {
            return Err(format!(
                "umbrella phase {} is not a quality request",
                phase.name()
            ));
        }
        Ok(QualityRequest {
            command: phase,
            mode: self.mode,
            dry_run: self.dry_run,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                reports,
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                log_level: self.common.log_level,
                color: self.common.color,
            },
            fail_on: self.fail_on,
            strict_evidence: self.strict_evidence,
        })
    }

    pub fn phase_generate_request(&self, reports: Vec<ReportRequest>) -> GenerateRequest {
        GenerateRequest {
            mode: self.mode,
            dry_run: self.dry_run,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                reports,
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                log_level: self.common.log_level,
                color: self.common.color,
            },
        }
    }

    pub fn verify_quality_request(&self, phase: Command) -> Result<QualityRequest, String> {
        if !matches!(phase, Command::Lint | Command::Typecheck | Command::Format) {
            return Err(format!(
                "umbrella phase {} is not a quality request",
                phase.name()
            ));
        }
        Ok(QualityRequest {
            command: phase,
            mode: OperationMode::Check,
            dry_run: false,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                reports: Vec::new(),
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                log_level: self.common.log_level,
                color: self.common.color,
            },
            fail_on: self.fail_on,
            strict_evidence: self.strict_evidence,
        })
    }

    pub fn verify_generate_request(&self) -> GenerateRequest {
        GenerateRequest {
            mode: OperationMode::Check,
            dry_run: false,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                reports: Vec::new(),
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                log_level: self.common.log_level,
                color: self.common.color,
            },
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

    pub fn common_options(&self) -> CommonOptions {
        CommonOptions {
            output: self.output,
            reports: self.reports.clone(),
            quiet: self.quiet,
            verbose: self.verbose,
            log_level: self.log_level,
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
            dry_run: self.dry_run,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            fail_on: self.fail_on,
            strict_evidence: self.strict_evidence,
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
            dry_run: self.dry_run,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
        })
    }

    pub fn umbrella_request(&self) -> Result<UmbrellaRequest, String> {
        if !matches!(self.command, Command::Check | Command::Fix) {
            return Err(format!(
                "option \"--command\" is not supported by dx {}: want check|fix",
                self.command.name()
            ));
        }
        Ok(UmbrellaRequest {
            command: self.command,
            mode: self.operation(),
            dry_run: self.dry_run,
            apply: self.apply,
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            fail_on: self.fail_on,
            strict_evidence: self.strict_evidence,
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
        assert!(!request.dry_run);
        assert_eq!(request.targets, vec!["//a:one".to_owned()]);
        assert_eq!(request.bazel_options, vec!["--jobs=4".to_owned()]);
        assert_eq!(
            request.fail_on,
            dx_output::Threshold::Error,
            "severity rides the typed request"
        );
        let planned = parsed(&["format", "--dry-run"])
            .quality_request()
            .expect("plan");
        assert_eq!(planned.mode, OperationMode::Plan);
        assert!(!planned.applies());
    }

    #[test]
    fn quality_request_has_no_unrelated_fields() {
        let request = parsed(&["lint", "//a:one"])
            .quality_request()
            .expect("request");
        let debug = format!("{request:?}");
        for unrelated in [
            "pin",
            "rollback",
            "serve",
            "offline",
            "frozen",
            "min_coverage",
            "run_output",
            "configured",
        ] {
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
        assert!(!recheck.dry_run);
        assert!(recheck.common.reports.is_empty());
        let generate = umbrella.verify_generate_request();
        assert_eq!(generate.mode, OperationMode::Check);
        assert!(!generate.applies());
    }
}
