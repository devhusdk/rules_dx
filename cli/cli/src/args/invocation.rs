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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonOptions {
    pub workspace: Option<String>,
    pub dry_run: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub log_level: Option<LogLevel>,
    pub color: ColorMode,
    pub output: OutputMode,
    pub bazel_startup_options: Vec<String>,
}

impl CommonOptions {
    pub fn from_invocation(invocation: &Invocation) -> Self {
        CommonOptions {
            workspace: invocation.workspace.clone(),
            dry_run: invocation.dry_run,
            quiet: invocation.quiet,
            verbose: invocation.verbose,
            log_level: invocation.log_level,
            color: invocation.color,
            output: invocation.output,
            bazel_startup_options: invocation.bazel_startup_options.clone(),
        }
    }

    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityRequest {
    pub command: Command,
    pub common: CommonOptions,
    pub operation: OperationMode,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub reports: Vec<ReportRequest>,
    pub fail_on: Threshold,
}

impl QualityRequest {
    pub fn from_invocation(invocation: &Invocation) -> Result<Self, String> {
        if !invocation.command.is_quality() {
            return Err(format!(
                "option \"--{}\" is not supported by dx {}",
                invocation.command.name(),
                "quality"
            ));
        }
        Self::reject_unrelated(invocation)?;
        Ok(QualityRequest {
            command: invocation.command,
            common: CommonOptions::from_invocation(invocation),
            operation: invocation.operation(),
            targets: invocation.targets.clone(),
            bazel_options: invocation.bazel_options.clone(),
            reports: invocation.reports.clone(),
            fail_on: invocation.fail_on,
        })
    }

    pub fn applies(&self) -> bool {
        self.operation == OperationMode::Apply
    }

    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }

    fn reject_unrelated(invocation: &Invocation) -> Result<(), String> {
        let command = invocation.command.name();
        if invocation.here {
            return Err(format!(
                "option \"--here\" is not supported by dx {command}"
            ));
        }
        if invocation.debug || invocation.release {
            return Err(format!(
                "option \"--debug|--release\" is not supported by dx {command}"
            ));
        }
        if invocation.strict_evidence {
            return Err(format!(
                "option \"--strict-evidence\" is not supported by dx {command}"
            ));
        }
        if invocation.run_output.is_some() {
            return Err(format!(
                "option \"--run-output\" is not supported by dx {command}"
            ));
        }
        if invocation.bazel_clean {
            return Err(format!(
                "option \"--bazel\" is not supported by dx {command}"
            ));
        }
        if invocation.prune_unobserved {
            return Err(format!(
                "option \"--prune-unobserved\" is not supported by dx {command}"
            ));
        }
        if invocation.pin.is_some() {
            return Err(format!("option \"--pin\" is not supported by dx {command}"));
        }
        if invocation.rollback {
            return Err(format!(
                "option \"--rollback\" is not supported by dx {command}"
            ));
        }
        if invocation.configured {
            return Err(format!(
                "option \"--configured\" is not supported by dx {command}"
            ));
        }
        if invocation.from.is_some() || invocation.to.is_some() {
            return Err(format!(
                "option \"--from|--to\" is not supported by dx {command}"
            ));
        }
        if invocation.serve
            || invocation.port.is_some()
            || invocation.host.is_some()
            || invocation.open
        {
            return Err(format!(
                "option \"--serve\" is not supported by dx {command}"
            ));
        }
        if invocation.offline {
            return Err(format!(
                "option \"--offline\" is not supported by dx {command}"
            ));
        }
        if invocation.frozen {
            return Err(format!(
                "option \"--frozen\" is not supported by dx {command}"
            ));
        }
        if invocation.workspace_capabilities {
            return Err(format!(
                "option \"--workspace-capabilities\" is not supported by dx {command}"
            ));
        }
        if invocation.cases {
            return Err(format!(
                "option \"--cases\" is not supported by dx {command}"
            ));
        }
        if invocation.min_coverage.is_some() {
            return Err(format!(
                "option \"--min-coverage\" is not supported by dx {command}"
            ));
        }
        Ok(())
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
    fn quality_request_preserves_mode_scopes_reports_severity_and_passthrough() {
        let lint = parsed(&[
            "lint",
            "--check",
            "--fail-on=error",
            "--report=sarif=out.sarif",
            "//a:one",
            "--",
            "--jobs=99",
        ]);
        let request = QualityRequest::from_invocation(&lint).expect("quality");
        assert_eq!(request.command, Command::Lint);
        assert_eq!(request.operation, OperationMode::Check);
        assert!(!request.applies());
        assert_eq!(request.targets, vec!["//a:one".to_owned()]);
        assert_eq!(request.bazel_options, vec!["--jobs=99".to_owned()]);
        assert_eq!(request.fail_on, dx_output::Threshold::Error);
        assert_eq!(request.reports.len(), 1);
        assert_eq!(request.reports[0].format, "sarif");
        assert_eq!(request.common.output, lint.output);
        assert!(request.chatty());

        let apply = parsed(&["lint", "--apply", "//a:one"]);
        let request = QualityRequest::from_invocation(&apply).expect("quality");
        assert_eq!(request.operation, OperationMode::Apply);
        assert!(request.applies());

        let plan = parsed(&["lint", "--dry-run", "//a:one"]);
        let request = QualityRequest::from_invocation(&plan).expect("quality");
        assert_eq!(request.operation, OperationMode::Plan);

        for command in ["typecheck", "format"] {
            let got = parsed(&[command, "//a:one"]);
            let request = QualityRequest::from_invocation(&got).expect("quality");
            assert_eq!(request.command.name(), command);
            assert_eq!(request.operation, OperationMode::Check);
        }
    }

    #[test]
    fn quality_request_rejects_non_quality_commands() {
        for words in [
            vec!["build", "//a:one"],
            vec!["check"],
            vec!["fix"],
            vec!["generate"],
            vec!["version"],
        ] {
            let invocation = parsed(&words);
            assert!(
                QualityRequest::from_invocation(&invocation).is_err(),
                "{words:?} is not a quality request"
            );
        }
        assert!(crate::args::parse(
            &["lint", "--pin=1.0.0"]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .is_err());
    }

    #[test]
    fn quality_request_rejects_unrelated_flags() {
        let mut base = parsed(&["lint", "//a:one"]);
        base.here = false;
        QualityRequest::from_invocation(&base).expect("clean quality");
        let mut polluted = base.clone();
        polluted.pin = Some("1.0.0".to_owned());
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.rollback = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.serve = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.port = Some(8080);
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.from = Some("1.0.0".to_owned());
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.offline = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.frozen = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.cases = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.min_coverage = Some(80);
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.debug = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.strict_evidence = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.run_output = Some("out".to_owned());
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.bazel_clean = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.configured = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base.clone();
        polluted.workspace_capabilities = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
        let mut polluted = base;
        polluted.here = true;
        assert!(QualityRequest::from_invocation(&polluted).is_err());
    }

    #[test]
    fn common_options_chatty_matches_invocation() {
        let lint = parsed(&["lint", "//a:one"]);
        assert_eq!(
            CommonOptions::from_invocation(&lint).chatty(),
            lint.chatty()
        );
        let mut quiet = lint.clone();
        quiet.quiet = true;
        assert!(!CommonOptions::from_invocation(&quiet).chatty());
    }
}
