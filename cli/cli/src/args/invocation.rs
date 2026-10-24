use dx_output::{dx_text_visible, ColorMode, OutputMode, Threshold};

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
    pub generate_targets: Vec<String>,
    pub test: Option<Box<Invocation>>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub fail_on: Threshold,
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
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                include_edits: self.common.include_edits,
                reports,
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                color: self.common.color,
            },
            fail_on: self.fail_on,
        })
    }

    pub fn phase_generate_request(&self, reports: Vec<ReportRequest>) -> GenerateRequest {
        GenerateRequest {
            mode: self.mode,
            targets: self.generate_targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                include_edits: self.common.include_edits,
                reports,
                quiet: self.common.quiet,
                verbose: self.common.verbose,
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
            targets: self.targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                include_edits: self.common.include_edits,
                reports: Vec::new(),
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                color: self.common.color,
            },
            fail_on: self.fail_on,
        })
    }

    pub fn verify_generate_request(&self) -> GenerateRequest {
        GenerateRequest {
            mode: OperationMode::Check,
            targets: self.generate_targets.clone(),
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: CommonOptions {
                output: self.common.output,
                include_edits: self.common.include_edits,
                reports: Vec::new(),
                quiet: self.common.quiet,
                verbose: self.common.verbose,
                color: self.common.color,
            },
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
    pub include_edits: bool,
    pub min_coverage: Option<u32>,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub prune_unobserved: bool,
    pub configured: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub here: bool,
    pub affected: bool,
    pub test_targets: Option<Vec<String>>,
    pub serve: bool,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub open: bool,
    /// The network policy: no rules_dx-controlled network acquisition when set.
    pub offline: bool,
    /// The resolution policy: no manifest or lock resolution changes when set.
    pub frozen: bool,
    pub cases: bool,
    pub create: Option<String>,
    pub force: bool,
    pub disk: bool,
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
    /// Bazel options for the run: agent JSON sessions get no progress output.
    pub fn quiet_bazel_options(&self) -> Vec<String> {
        let mut options = Vec::new();
        if self.agent && self.output == OutputMode::Json {
            options.extend(AGENT_QUIET_FLAGS.iter().map(|flag| (*flag).to_owned()));
        }
        options.extend(self.bazel_options.iter().cloned());
        options
    }

    pub fn mode(self) -> &'static str {
        if self.check {
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
        let affected = self.test_targets.is_some();
        let test = match (&self.test_targets, self.command) {
            (Some(tests), Command::Check) if !tests.is_empty() => {
                let mut test = self.clone();
                test.command = Command::Test;
                test.check = false;
                test.apply = false;
                test.reports = Vec::new();
                test.include_edits = false;
                test.targets = tests.clone();
                test.test_targets = None;
                Some(Box::new(test))
            }
            _ => None,
        };
        Ok(UmbrellaRequest {
            command: self.command,
            mode: self.operation(),
            apply: self.apply,
            targets: self.targets.clone(),
            generate_targets: if affected {
                Vec::new()
            } else {
                self.targets.clone()
            },
            test,
            bazel_options: self.quiet_bazel_options(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            fail_on: self.fail_on,
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
        return Err("option \"--here\" cannot be combined with explicit scopes".to_owned());
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
            include_edits: false,
            min_coverage: None,
            targets: targets.iter().map(ToString::to_string).collect(),
            bazel_options: Vec::new(),
            prune_unobserved: false,
            configured: false,
            from: None,
            to: None,
            here: true,
            affected: false,
            test_targets: None,
            serve: false,
            port: None,
            host: None,
            open: false,
            offline: false,
            frozen: false,
            cases: false,
            create: None,
            force: false,
            disk: false,
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
