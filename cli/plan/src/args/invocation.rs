use dx_output::{ColorMode, OutputMode, Threshold, dx_text_visible};

use super::Command;
use super::build_mode::{BuildMode, resolve_build_mode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AffectedHandoff {
    pub tests: Vec<String>,
    pub coverage_tests: Vec<String>,
    pub base: Option<String>,
    pub generate: Vec<String>,
    pub format: Vec<String>,
    pub lint: Vec<String>,
    pub writable: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRequest {
    pub format: String,
    pub destination: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Check,
    Apply,
}

impl OperationMode {
    #[must_use]
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
    pub max_findings: Option<u32>,
    pub reports: Vec<ReportRequest>,
    pub quiet: bool,
    pub verbose: bool,
    pub color: ColorMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintScope {
    Affected,
    Targets(Vec<String>),
}

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
    pub origin: Option<UmbrellaOrigin>,
    pub unsafe_fixes: bool,
    pub online: bool,
    pub writable: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateRequest {
    pub mode: OperationMode,
    pub targets: Vec<String>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub writable: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmbrellaRequest {
    pub command: Command,
    pub mode: OperationMode,
    pub apply: bool,
    pub targets: Vec<String>,
    pub scope: HintScope,
    pub generate_targets: Vec<String>,
    pub format_targets: Vec<String>,
    pub lint_targets: Vec<String>,
    pub test: Option<Box<Invocation>>,
    pub coverage: Option<Box<Invocation>>,
    pub bazel_options: Vec<String>,
    pub bazel_startup_options: Vec<String>,
    pub workspace: Option<String>,
    pub common: CommonOptions,
    pub fail_on: Threshold,
    pub unsafe_fixes: bool,
    pub commits_base: Option<String>,
    pub writable: Option<Vec<String>>,
}

impl CommonOptions {
    #[must_use]
    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
    }
}

impl QualityRequest {
    #[must_use]
    pub fn operation(&self) -> OperationMode {
        self.mode
    }

    #[must_use]
    pub fn applies(&self) -> bool {
        self.mode == OperationMode::Apply
    }

    #[must_use]
    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }
}

impl GenerateRequest {
    #[must_use]
    pub fn operation(&self) -> OperationMode {
        self.mode
    }

    #[must_use]
    pub fn applies(&self) -> bool {
        self.mode == OperationMode::Apply
    }

    #[must_use]
    pub fn chatty(&self) -> bool {
        self.common.chatty()
    }

    #[must_use]
    pub fn bazel_head(&self) -> dx_process::BazelHead {
        let mut options = vec![BuildMode::default_for(Command::Generate).config_flag()];
        options.extend(self.bazel_options.iter().cloned());
        dx_process::BazelHead {
            startup: self.bazel_startup_options.clone(),
            options,
            settings: crate::plan::shared_settings("build"),
        }
    }
}

impl UmbrellaRequest {
    #[must_use]
    pub fn operation(&self) -> OperationMode {
        self.mode
    }

    #[must_use]
    pub fn applies(&self) -> bool {
        self.mode == OperationMode::Apply
    }

    #[must_use]
    pub fn selects_nothing(&self, phase: Command) -> bool {
        let targets = if phase == Command::Format {
            &self.format_targets
        } else if phase == Command::Lint {
            &self.lint_targets
        } else {
            return false;
        };
        self.scope == HintScope::Affected && targets.is_empty()
    }

    pub fn phase_quality_request(
        &self,
        phase: Command,
        reports: Vec<ReportRequest>,
    ) -> Result<QualityRequest, dx_error::DxError> {
        self.quality_request(phase, self.mode, reports)
    }

    #[must_use]
    pub fn phase_generate_request(&self, reports: Vec<ReportRequest>) -> GenerateRequest {
        self.generate_request(self.mode, reports)
    }

    pub fn verify_quality_request(
        &self,
        phase: Command,
    ) -> Result<QualityRequest, dx_error::DxError> {
        let mut request = self.quality_request(phase, OperationMode::Check, Vec::new())?;
        request.unsafe_fixes = false;
        Ok(request)
    }

    #[must_use]
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
            max_findings: self.common.max_findings,
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
    ) -> Result<QualityRequest, dx_error::DxError> {
        if !matches!(phase, Command::Lint | Command::Typecheck | Command::Format) {
            return Err(dx_error::DxError::usage(format!(
                "umbrella phase {} is not a quality request",
                phase.name()
            )));
        }
        Ok(QualityRequest {
            command: phase,
            mode,
            targets: if phase == Command::Format {
                self.format_targets.clone()
            } else if phase == Command::Lint {
                self.lint_targets.clone()
            } else {
                self.targets.clone()
            },
            bazel_options: self.bazel_options.clone(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_with(reports),
            fail_on: self.fail_on,
            origin: Some(self.origin()),
            unsafe_fixes: self.unsafe_fixes,
            online: false,
            writable: self.writable.clone(),
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
            writable: self.writable.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent on/off options, one per command line flag"
)]
pub struct Invocation {
    pub command: Command,
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
    pub build_slots: Option<dx_adopt::defaults::BuildSlots>,
    pub build_slots_wait: Option<std::time::Duration>,
    pub poll: bool,
    pub include_edits: bool,
    pub max_findings: Option<u32>,
    pub unsafe_fixes: bool,
    pub online: bool,
    pub min_coverage: Option<u32>,
    pub min_changed_line_coverage: Option<u32>,
    pub targets: Vec<String>,
    pub(super) bazel_options: Vec<String>,
    pub(super) app_args: Vec<String>,
    pub agents: Vec<crate::args::Agent>,
    pub ci: Option<crate::args::Ci>,
    pub no_generate: bool,
    pub accept_license: Vec<String>,
    pub to: Option<String>,
    pub here: bool,
    pub affected: bool,
    pub since: Option<String>,
    pub base: Option<String>,
    pub lint_closure: bool,
    pub affected_handoff: Option<AffectedHandoff>,
    pub serve: bool,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub open: bool,
    pub offline: bool,
    pub vendor: Option<String>,
    pub frozen: bool,
    pub outdated: bool,
    pub prune: bool,
    pub clean_bazel: bool,
    pub expunge: bool,
    pub add_missing: bool,
    pub pr: bool,
    pub automerge: Option<String>,
    pub min_age: Option<u32>,
    pub allow_major: bool,
    pub max_prs: Option<u16>,
    pub floor: bool,
    pub reason: Option<String>,
    pub accept: Vec<String>,
    pub expires: Option<String>,
    pub verify: Option<String>,
    pub repo: Option<String>,
    pub bundle: Option<String>,
    pub cases: bool,
    pub create: Option<String>,
    pub force: bool,
    pub disk: bool,
    pub bases: bool,
    pub capabilities: bool,
    pub files: bool,
    pub timings: bool,
    pub no_retry: bool,
    pub coverage: bool,
    pub budget: Option<String>,
    pub jobs: Option<u16>,
    pub bench_regression: Option<u32>,
    pub bench_compare: Option<String>,
    pub path_only: bool,
    pub agent: bool,
    pub piped: bool,
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
    #[must_use]
    pub fn edits_version(&self) -> bool {
        self.command == Command::Update
            && matches!(self.targets.as_slice(), [_, version] if is_version(version))
    }

    #[must_use]
    pub fn agent_quiet_flags(&self) -> Vec<String> {
        let agent_json = self.agent && self.output == OutputMode::Json;
        if (agent_json || self.piped) && !self.verbose {
            AGENT_QUIET_FLAGS
                .iter()
                .map(|flag| (*flag).to_owned())
                .collect()
        } else {
            Vec::new()
        }
    }

    #[must_use]
    pub fn quiet_bazel_options(&self) -> Vec<String> {
        let mut options = self.agent_quiet_flags();
        options.extend(self.bazel_options.iter().cloned());
        options
    }

    #[must_use]
    pub fn raw_bazel_options(&self) -> &[String] {
        &self.bazel_options
    }

    #[must_use]
    pub fn app_args(&self) -> &[String] {
        &self.app_args
    }

    #[must_use]
    pub fn started_mode(&self) -> &'static str {
        if self.command.supports_apply() && !self.applies() {
            "check"
        } else {
            "default"
        }
    }

    #[must_use]
    pub fn operation(&self) -> OperationMode {
        if self.apply || self.command == Command::Sync {
            OperationMode::Apply
        } else {
            OperationMode::Check
        }
    }

    #[must_use]
    pub fn applies(&self) -> bool {
        self.operation() == OperationMode::Apply
    }

    #[must_use]
    pub fn build_mode_flag(&self) -> Option<BuildMode> {
        if self.debug {
            Some(BuildMode::Debug)
        } else if self.release {
            Some(BuildMode::Release)
        } else {
            None
        }
    }

    #[must_use]
    pub fn build_mode(&self) -> BuildMode {
        resolve_build_mode(
            self.build_mode_flag(),
            None,
            BuildMode::default_for(self.command),
        )
    }

    #[must_use]
    pub fn bazel_head(&self) -> dx_process::BazelHead {
        dx_process::BazelHead {
            startup: self.bazel_startup_options.clone(),
            options: vec![self.build_mode().config_flag()],
            settings: crate::plan::shared_settings("build"),
        }
    }

    #[must_use]
    pub fn chatty(&self) -> bool {
        dx_text_visible(&self.output) && !self.quiet
    }

    #[must_use]
    pub fn common_options(&self) -> CommonOptions {
        CommonOptions {
            output: self.output,
            include_edits: self.include_edits,
            max_findings: self.max_findings,
            reports: self.reports.clone(),
            quiet: self.quiet,
            verbose: self.verbose,
            color: self.color,
        }
    }

    pub fn quality_request(&self) -> Result<QualityRequest, dx_error::DxError> {
        if !matches!(
            self.command,
            Command::Lint | Command::Typecheck | Command::Format
        ) {
            return Err(dx_error::DxError::usage(format!(
                "option \"--command\" is not supported by dx {}: want lint|typecheck|format",
                self.command.name()
            )));
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
            origin: None,
            unsafe_fixes: self.unsafe_fixes,
            online: self.online,
            writable: self.writable(),
        })
    }

    pub fn generate_request(&self) -> Result<GenerateRequest, dx_error::DxError> {
        if self.command != Command::Generate {
            return Err(dx_error::DxError::usage(format!(
                "option \"--command\" is not supported by dx {}: want generate",
                self.command.name()
            )));
        }
        Ok(GenerateRequest {
            mode: self.operation(),
            targets: self.targets.clone(),
            bazel_options: self.quiet_bazel_options(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            writable: self.writable(),
        })
    }

    #[must_use]
    pub fn is_affected_plan(&self) -> bool {
        self.affected_handoff.is_some()
    }

    fn test_phase(
        &self,
        command: Command,
        tests: &[String],
        reports: Vec<ReportRequest>,
        bazel_options: Vec<String>,
    ) -> Option<Box<Invocation>> {
        if tests.is_empty() {
            return None;
        }
        let mut phase = self.clone();
        phase.command = command;
        phase.apply = false;
        phase.reports = reports;
        phase.include_edits = false;
        phase.max_findings = None;
        phase.unsafe_fixes = false;
        phase.coverage = false;
        phase.targets = tests.to_vec();
        phase.bazel_options = bazel_options;
        phase.affected_handoff = None;
        Some(Box::new(phase))
    }

    pub fn umbrella_request(&self) -> Result<UmbrellaRequest, dx_error::DxError> {
        if self.command != Command::Check {
            return Err(dx_error::DxError::usage(format!(
                "option \"--command\" is not supported by dx {}: want check",
                self.command.name()
            )));
        }
        let affected = self.is_affected_plan();
        let (test, coverage) = match &self.affected_handoff {
            Some(handoff) if !self.applies() => (
                self.test_phase(
                    Command::Test,
                    &handoff.tests,
                    Vec::new(),
                    self.bazel_options.clone(),
                ),
                self.test_phase(
                    Command::Coverage,
                    &handoff.coverage_tests,
                    self.reports
                        .iter()
                        .filter(|report| matches!(report.format.as_str(), "lcov" | "sources"))
                        .cloned()
                        .collect(),
                    super::exclude_tags(&self.bazel_options, &super::COVERAGE_EXCLUDED_TAGS),
                ),
            ),
            _ => (None, None),
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
            generate_targets: match &self.affected_handoff {
                Some(handoff) => handoff.generate.clone(),
                None => self.targets.clone(),
            },
            format_targets: match &self.affected_handoff {
                Some(handoff) => handoff.format.clone(),
                None => self.targets.clone(),
            },
            lint_targets: match &self.affected_handoff {
                Some(handoff) => handoff.lint.clone(),
                None => self.targets.clone(),
            },
            test,
            coverage,
            commits_base: self
                .affected_handoff
                .as_ref()
                .and_then(|handoff| handoff.base.clone()),
            bazel_options: self.quiet_bazel_options(),
            bazel_startup_options: self.bazel_startup_options.clone(),
            workspace: self.workspace.clone(),
            common: self.common_options(),
            fail_on: self.fail_on,
            unsafe_fixes: self.unsafe_fixes,
            writable: self.writable(),
        })
    }

    fn writable(&self) -> Option<Vec<String>> {
        self.affected_handoff
            .as_ref()
            .and_then(|handoff| handoff.writable.clone())
    }
}

pub fn here_scope(
    workspace: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<String, dx_error::DxError> {
    use std::path::Component;
    let rel = cwd.strip_prefix(workspace).ok().ok_or_else(|| dx_error::DxError::usage({
        format!(
            "current directory {} is outside workspace {}: --here needs a directory under the workspace",
            cwd.display(),
            workspace.display()
        )
    }))?;
    if rel.as_os_str().is_empty() {
        return Ok("//...".to_owned());
    }
    let mut parts: Vec<String> = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str().ok_or_else(|| {
                    dx_error::DxError::usage({
                        format!(
                            "current directory {} is not valid UTF-8: --here needs a UTF-8 path",
                            cwd.display()
                        )
                    })
                })?;
                for piece in dx_path::posix(std::path::Path::new(&text)).split('/') {
                    if !piece.is_empty() && piece != "." {
                        parts.push(piece.to_owned());
                    }
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(dx_error::DxError::usage(format!(
                    "current directory {} is outside workspace {}: --here needs a directory under the workspace",
                    cwd.display(),
                    workspace.display()
                )));
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
) -> Result<Invocation, dx_error::DxError> {
    if !invocation.here {
        return Ok(invocation.clone());
    }
    let (owner, at) = here_owner(invocation);
    if !owner.supports_here() {
        return Err(dx_error::DxError::usage(format!(
            "option \"--here\" is not supported by dx {}",
            owner.name()
        )));
    }
    let scope = here_scope(workspace, cwd)?;
    let mut next = invocation.clone();
    if next.targets.len() > at {
        return Err(dx_error::DxError::usage(
            "option \"--here\" cannot be combined with explicit scopes".to_owned(),
        ));
    }
    next.targets.truncate(at);
    next.targets.push(scope);
    next.here = false;
    Ok(next)
}

#[must_use]
pub fn bare_scope_note(
    invocation: &Invocation,
    workspace: &std::path::Path,
    cwd: &std::path::Path,
) -> Option<String> {
    if invocation.here || !invocation.chatty() {
        return None;
    }
    let (owner, at) = here_owner(invocation);
    if !owner.supports_here() || invocation.targets.len() > at {
        return None;
    }
    let dir = here_scope(workspace, cwd)
        .ok()
        .filter(|dir| dir != "//...")?;
    Some(format!(
        "dx: {} //... (cwd is {dir}; pass --here for //{dir}/...)",
        owner.name()
    ))
}

fn here_owner(invocation: &Invocation) -> (Command, usize) {
    if invocation.command == Command::Watch
        && let Some(wrapped) = invocation
            .targets
            .first()
            .map(String::as_str)
            .and_then(Command::parse)
    {
        return (wrapped, 1);
    }
    (invocation.command, 0)
}

pub(crate) fn is_version(word: &str) -> bool {
    let digits = word.strip_prefix('v').unwrap_or(word);
    let sha = word.len() >= 7 && word.chars().all(|c| c.is_ascii_hexdigit());
    (sha || digits.starts_with(|c: char| c.is_ascii_digit())) && !word.contains([':', '/', '@'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_heads_carry_startup_and_the_default_build_mode() {
        let mut invocation = invocation_for(Command::Update, &[]);
        invocation.bazel_startup_options = vec!["--output_base=/ob".to_owned()];
        let head = invocation.bazel_head();
        assert_eq!(head.startup, vec!["--output_base=/ob".to_owned()]);
        assert_eq!(head.options, vec!["--config=dx_build_dev".to_owned()]);
        assert_eq!(head.settings, crate::plan::shared_settings("build"));
        assert_ne!(head.settings, [] as [String; 0]);
    }

    #[test]
    fn generate_heads_carry_the_callers_bazel_options_after_the_build_mode() {
        let mut invocation = invocation_for(Command::Generate, &[]);
        invocation.bazel_startup_options = vec!["--output_base=/ob".to_owned()];
        invocation.bazel_options = vec!["--config=ci".to_owned(), "--jobs=3".to_owned()];
        let head = invocation
            .generate_request()
            .expect("generate")
            .bazel_head();
        assert_eq!(head.startup, vec!["--output_base=/ob".to_owned()]);
        assert_eq!(
            head.options,
            vec![
                "--config=dx_build_dev".to_owned(),
                "--config=ci".to_owned(),
                "--jobs=3".to_owned(),
            ]
        );
        assert_eq!(head.settings, crate::plan::shared_settings("build"));
    }

    fn invocation_for(command: Command, targets: &[&str]) -> Invocation {
        use dx_output::{ColorMode, OutputMode, Threshold};
        Invocation {
            command,
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
            build_slots: None,
            build_slots_wait: None,
            poll: false,
            include_edits: false,
            max_findings: None,
            unsafe_fixes: false,
            online: false,
            min_coverage: None,
            min_changed_line_coverage: None,
            targets: targets.iter().map(ToString::to_string).collect(),
            bazel_options: Vec::new(),
            app_args: Vec::new(),
            agents: Vec::new(),
            ci: None,
            no_generate: false,
            accept_license: Vec::new(),
            to: None,
            here: true,
            affected: false,
            since: None,
            base: None,
            lint_closure: false,
            affected_handoff: None,
            serve: false,
            port: None,
            host: None,
            open: false,
            offline: false,
            vendor: None,
            frozen: false,
            outdated: false,
            prune: false,
            clean_bazel: false,
            expunge: false,
            add_missing: false,
            pr: false,
            automerge: None,
            min_age: None,
            allow_major: false,
            max_prs: None,
            floor: false,
            reason: None,
            accept: Vec::new(),
            expires: None,
            verify: None,
            repo: None,
            bundle: None,
            cases: false,
            create: None,
            force: false,
            disk: false,
            bases: false,
            capabilities: false,
            files: false,
            timings: false,
            no_retry: false,
            coverage: false,
            budget: None,
            jobs: None,
            bench_regression: None,
            bench_compare: None,
            path_only: false,
            agent: false,
            piped: false,
            bazel_startup_options: Vec::new(),
        }
    }

    #[test]
    fn operation_authorizes_only_explicit_apply() {
        let bare = invocation_for(Command::Check, &[]);
        assert!(!bare.apply);
        assert_eq!(bare.operation(), OperationMode::Check);
        assert!(!bare.applies());
        let mut apply = invocation_for(Command::Check, &[]);
        apply.apply = true;
        assert_eq!(apply.operation(), OperationMode::Apply);
        assert!(apply.applies());
        assert_eq!(OperationMode::Check.name(), "check");
        assert_eq!(OperationMode::Apply.name(), "apply");
    }

    #[test]
    fn affected_phases_write_only_the_handed_off_paths() {
        let mut check = invocation_for(Command::Check, &["//a:x"]);
        let writable = vec!["a/BUILD.bazel".to_owned(), "a/x.rs".to_owned()];
        check.affected_handoff = Some(AffectedHandoff {
            writable: Some(writable.clone()),
            ..AffectedHandoff::default()
        });
        let request = check.umbrella_request().expect("umbrella");
        for phase in [Command::Format, Command::Lint, Command::Typecheck] {
            assert_eq!(
                request
                    .phase_quality_request(phase, Vec::new())
                    .expect("phase")
                    .writable,
                Some(writable.clone())
            );
        }
        let plain = invocation_for(Command::Check, &["//a:x"]);
        assert_eq!(
            plain
                .umbrella_request()
                .expect("umbrella")
                .phase_quality_request(Command::Lint, Vec::new())
                .expect("phase")
                .writable,
            None
        );
    }

    #[test]
    fn coverage_handoff_splits_the_tests_into_two_phases() {
        let mut check = invocation_for(Command::Check, &["//a:x"]);
        check.bazel_options = vec!["--test_tag_filters=-audit".to_owned()];
        check.reports = vec![
            ReportRequest {
                format: "sarif".to_owned(),
                destination: "check.sarif".to_owned(),
            },
            ReportRequest {
                format: "lcov".to_owned(),
                destination: "lcov.dat".to_owned(),
            },
        ];
        check.affected_handoff = Some(AffectedHandoff {
            tests: vec!["//a:plain_test".to_owned()],
            coverage_tests: vec!["//a:x_test".to_owned()],
            ..AffectedHandoff::default()
        });
        let request = check.umbrella_request().expect("umbrella");
        let test = request.test.expect("test phase");
        assert_eq!(test.command, Command::Test);
        assert_eq!(test.targets, ["//a:plain_test"]);
        assert_eq!(test.reports, []);
        let coverage = request.coverage.expect("coverage phase");
        assert_eq!(coverage.command, Command::Coverage);
        assert_eq!(coverage.targets, ["//a:x_test"]);
        assert_eq!(coverage.reports, check.reports[1..]);
        assert_eq!(
            coverage.bazel_options,
            [format!(
                "--test_tag_filters=-audit,-{}",
                crate::args::COVERAGE_EXCLUDED_TAGS.join(",-")
            )]
        );
    }

    #[test]
    fn empty_test_lists_plan_no_test_phase() {
        let mut check = invocation_for(Command::Check, &["//a:x"]);
        check.affected_handoff = Some(AffectedHandoff::default());
        let request = check.umbrella_request().expect("umbrella");
        assert!(request.test.is_none());
        assert!(request.coverage.is_none());
    }

    #[test]
    fn umbrella_phases_carry_the_user_scope() {
        let mut affected = invocation_for(Command::Check, &["//a:x", "//a:y", "//b:user"]);
        affected.affected_handoff = Some(AffectedHandoff {
            generate: vec!["//a/...".to_owned()],
            format: vec!["//a:x".to_owned()],
            lint: vec!["//a:x".to_owned(), "//a:y".to_owned()],
            ..AffectedHandoff::default()
        });
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
        assert_eq!(phase.targets, vec!["//a:x".to_owned()]);
        assert_eq!(
            request
                .verify_quality_request(Command::Format)
                .expect("verify")
                .targets,
            vec!["//a:x".to_owned()]
        );
        assert_eq!(
            request
                .phase_quality_request(Command::Lint, Vec::new())
                .expect("phase")
                .targets,
            vec!["//a:x".to_owned(), "//a:y".to_owned()]
        );
        assert_eq!(
            request
                .verify_quality_request(Command::Lint)
                .expect("verify")
                .targets,
            vec!["//a:x".to_owned(), "//a:y".to_owned()]
        );
        assert_eq!(
            request
                .phase_quality_request(Command::Typecheck, Vec::new())
                .expect("phase")
                .targets,
            vec![
                "//a:x".to_owned(),
                "//a:y".to_owned(),
                "//b:user".to_owned()
            ]
        );
        assert_eq!(
            request.phase_generate_request(Vec::new()).targets,
            vec!["//a/...".to_owned()]
        );
        assert_eq!(
            request.verify_generate_request().targets,
            vec!["//a/...".to_owned()]
        );

        let mut fix = invocation_for(Command::Check, &["//a/..."]);
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
            Some(Command::Check)
        );

        let direct = invocation_for(Command::Lint, &["//a/..."]);
        assert_eq!(direct.quality_request().expect("quality").origin, None);
    }

    #[test]
    fn affected_phases_with_no_targets_select_nothing() {
        let mut affected = invocation_for(Command::Check, &["//svc:bin"]);
        affected.affected_handoff = Some(AffectedHandoff::default());
        let request = affected.umbrella_request().expect("umbrella");
        assert!(request.selects_nothing(Command::Format));
        assert!(request.selects_nothing(Command::Lint));
        assert!(!request.selects_nothing(Command::Typecheck));
        assert!(!request.selects_nothing(Command::Generate));
        let plain = invocation_for(Command::Check, &[]);
        let request = plain.umbrella_request().expect("umbrella");
        assert!(!request.selects_nothing(Command::Lint));
    }

    #[test]
    fn affected_fix_scopes_generate_apply_and_verify_to_packages() {
        let mut fix = invocation_for(Command::Check, &["//a:x", "//b:y"]);
        fix.apply = true;
        let want = vec!["//a/...".to_owned(), "//b/...".to_owned()];
        fix.affected_handoff = Some(AffectedHandoff {
            generate: want.clone(),
            ..AffectedHandoff::default()
        });
        let request = fix.umbrella_request().expect("umbrella");
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
        check.affected_handoff = Some(AffectedHandoff {
            generate: vec!["//...".to_owned()],
            ..AffectedHandoff::default()
        });
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
        assert!(
            refused
                .message()
                .starts_with("option \"--here\" is not supported by dx run")
        );

        let combined = apply_here(
            &invocation_for(Command::Watch, &["test", "//a:one"]),
            workspace,
            subdir,
        )
        .expect_err("explicit scope");
        assert_eq!(
            combined.message(),
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

        assert!(
            apply_here(
                &invocation_for(Command::Lint, &["//a:one"]),
                workspace,
                subdir
            )
            .is_err()
        );
        assert!(
            apply_here(
                &invocation_for(Command::Security, &["//a:one"]),
                workspace,
                subdir
            )
            .is_err()
        );
        assert!(apply_here(&invocation_for(Command::Clean, &[]), workspace, subdir).is_err());
        assert!(
            apply_here(
                &invocation_for(Command::Lint, &[]),
                workspace,
                std::path::Path::new("/other")
            )
            .is_err()
        );

        let scratch = dx_test_scratch::scratch("dx-here-bazel-run-").unwrap();
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
        assert_eq!(resolved.targets, [] as [String; 0]);
        assert!(!resolved.here);
    }

    #[test]
    fn bare_scope_note_only_below_the_root() {
        let workspace = std::path::Path::new("/ws");
        let sub = std::path::Path::new("/ws/services/api");
        let mut bare = invocation_for(Command::Test, &[]);
        bare.here = false;
        assert_eq!(
            bare_scope_note(&bare, workspace, sub).as_deref(),
            Some("dx: test //... (cwd is services/api; pass --here for //services/api/...)")
        );
        assert_eq!(bare_scope_note(&bare, workspace, workspace), None);
        let mut targeted = invocation_for(Command::Test, &["//a:one"]);
        targeted.here = false;
        assert_eq!(bare_scope_note(&targeted, workspace, sub), None);
        let mut quiet = bare.clone();
        quiet.quiet = true;
        assert_eq!(bare_scope_note(&quiet, workspace, sub), None);
        let mut json = bare.clone();
        json.output = OutputMode::Json;
        assert_eq!(bare_scope_note(&json, workspace, sub), None);
        let mut here = bare.clone();
        here.here = true;
        assert_eq!(bare_scope_note(&here, workspace, sub), None);
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
    fn watch_poll_is_watchs_own_flag_and_stays_off_the_wrapped_command() {
        let polled = parsed(&["watch", "test", "--poll", "//a:one"]);
        assert_eq!(polled.command, Command::Watch);
        assert!(polled.poll);
        assert_eq!(polled.targets, ["test", "//a:one"]);
        assert!(parsed(&["watch", "--poll", "test", "//a:one"]).poll);
        assert!(!parsed(&["watch", "test", "//a:one"]).poll);
        assert!(crate::args::parse(&["build", "--poll"]).is_err());
    }

    #[test]
    fn quality_request_accepts_only_the_quality_family() {
        for command in ["lint", "typecheck", "format"] {
            let request = parsed(&[command, "//a:one"])
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
            Threshold::Error,
            "severity rides the typed request"
        );
    }

    #[test]
    fn quality_request_has_no_unrelated_fields() {
        let request = parsed(&["lint", "//a:one"])
            .quality_request()
            .expect("request");
        let debug = format!("{request:?}");
        for unrelated in ["serve", "offline", "frozen", "min_coverage"] {
            assert!(
                !debug.contains(unrelated),
                "typed quality request cannot represent {unrelated}: {debug}"
            );
        }
    }

    #[test]
    fn generate_and_umbrella_requests_validate_their_commands() {
        let generate = parsed(&["generate"]).generate_request().expect("generate");
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
    fn piped_runs_are_quiet_unless_verbose() {
        let mut inv = parsed(&["build"]);
        assert_eq!(inv.quiet_bazel_options(), [] as [String; 0]);
        inv.piped = true;
        assert_eq!(inv.quiet_bazel_options(), quiet_flags());
        inv.verbose = true;
        assert_eq!(inv.quiet_bazel_options(), [] as [String; 0]);
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
        assert_eq!(
            agent(&["build", "--output=text"]).agent_quiet_flags().len(),
            0
        );
        assert_eq!(
            parsed(&["build", "--output=json"])
                .agent_quiet_flags()
                .len(),
            0
        );
    }

    #[test]
    fn piped_requests_carry_quiet_flags_until_verbose() {
        let mut inv = parsed(&["generate", "--", "--jobs=4"]);
        inv.piped = true;
        let mut want = quiet_flags();
        want.push("--jobs=4".to_owned());
        assert_eq!(
            inv.generate_request().expect("generate").bazel_options,
            want
        );
        inv.verbose = true;
        assert_eq!(
            inv.generate_request().expect("generate").bazel_options,
            vec!["--jobs=4".to_owned()]
        );
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
        let umbrella = parsed(&["check", "--apply", "--fail-on=error", "//a:one"])
            .umbrella_request()
            .expect("fix apply");
        assert_eq!(umbrella.mode, OperationMode::Apply);
        let phase = umbrella
            .phase_quality_request(Command::Lint, Vec::new())
            .expect("lint phase");
        assert_eq!(phase.command, Command::Lint);
        assert_eq!(phase.mode, OperationMode::Apply);
        assert_eq!(phase.targets, vec!["//a:one".to_owned()]);
        assert_eq!(phase.fail_on, Threshold::Error);
        assert!(
            umbrella
                .phase_quality_request(Command::Generate, Vec::new())
                .is_err()
        );
        let generate = umbrella.phase_generate_request(Vec::new());
        assert_eq!(generate.mode, OperationMode::Apply);
        assert_eq!(generate.targets, vec!["//a:one".to_owned()]);
    }

    #[test]
    fn umbrella_verification_is_always_read_only() {
        let umbrella = parsed(&["check", "--apply", "//a:one"])
            .umbrella_request()
            .expect("fix apply");
        let recheck = umbrella
            .verify_quality_request(Command::Lint)
            .expect("recheck");
        assert_eq!(recheck.mode, OperationMode::Check);
        assert!(!recheck.applies());
        assert_eq!(recheck.common.reports, [] as [ReportRequest; 0]);
        let generate = umbrella.verify_generate_request();
        assert_eq!(generate.mode, OperationMode::Check);
        assert!(!generate.applies());
    }
}
