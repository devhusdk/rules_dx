//! Refreshes `.dx/local/ide/` after `dx sync` and `dx upgrade`. Never changes the exit code.

use std::io::Write;
use std::path::{Path, PathBuf};

use dx_error::DxError;
use dx_output::{NoticeEvent, OutputMode, notice_event};
use dx_plan::resolve::QueryRunner;

use super::common::{emit_built, flush_out};
use super::notice_codes::CODE_IDE_REFRESH_FAILED;

struct BazelBuild<'a> {
    query_runner: &'a dyn QueryRunner,
    workspace: &'a Path,
    startup: &'a [String],
}

impl BazelBuild<'_> {
    fn run(&self, argv: &[String]) -> Result<String, DxError> {
        let result = self
            .query_runner
            .run_query(argv, self.workspace)
            .map_err(|error| DxError::infra(format!("could not run bazel: {error}")))?;
        if result.code != Some(0) {
            return Err(DxError::failure(dx_tool_resolve::exit_detail(
                result.code,
                &result.stderr,
            )));
        }
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    }

    fn argv(
        &self,
        verb: &str,
        options: &[String],
        labels: &[String],
    ) -> Result<Vec<String>, DxError> {
        dx_plan::plan::bazel_argv(verb, options, &[], &[], labels, self.startup)
            .map_err(|error| DxError::infra(format!("could not build the {verb} argv: {error}")))
    }

    #[allow(clippy::disallowed_methods)] // info takes no build settings
    fn info(&self, key: &str) -> Result<PathBuf, DxError> {
        let mut argv = dx_process::startup_argv(self.startup);
        argv.extend(["info".to_owned(), key.to_owned()]);
        let text = self.run(&argv)?;
        Ok(PathBuf::from(text.trim()))
    }
}

impl BazelBuild<'_> {
    fn build_files(&self, labels: &[String], options: &[String]) -> Result<Vec<PathBuf>, DxError> {
        self.run(&self.argv("build", options, labels)?)?;
        let mut cquery_options = options.to_vec();
        cquery_options.push("--output=files".to_owned());
        let files = self.run(&self.argv("cquery", &cquery_options, labels)?)?;
        let output_base = self.info("output_base")?;
        let execution_root = self.info("execution_root")?;
        Ok(files
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                if line.starts_with("external/") {
                    dx_tool_resolve::join_under(&output_base.to_string_lossy(), line)
                } else if line.starts_with("bazel-out/") {
                    dx_tool_resolve::join_under(&execution_root.to_string_lossy(), line)
                } else {
                    self.workspace.join(line)
                }
            })
            .collect())
    }
}

impl dx_ide::Runner for BazelBuild<'_> {
    fn build(&self, labels: &[String]) -> Result<Vec<PathBuf>, dx_error::DxError> {
        let labels: Vec<String> = labels
            .iter()
            .map(|label| dx_plan::plan::rules_dx_label(label))
            .collect();
        self.build_files(&labels, &[])
    }

    fn build_group(
        &self,
        labels: &[String],
        group: &str,
    ) -> Result<Vec<PathBuf>, dx_error::DxError> {
        self.build_files(labels, &[format!("--output_groups={group}")])
    }

    fn aquery(&self, expr: &str) -> Result<String, dx_error::DxError> {
        let options = ["--output=jsonproto".to_owned()];
        let argv = self.argv("aquery", &options, &[expr.to_owned()])?;
        self.run(&argv)
    }

    fn execution_root(&self) -> Result<PathBuf, dx_error::DxError> {
        self.info("execution_root")
    }
}

/// Refreshes `.dx/local/ide/` and returns the warning line when it fails.
pub(crate) fn refresh(
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    startup: &[String],
) -> Option<dx_error::DxError> {
    let bazel = BazelBuild {
        query_runner,
        workspace,
        startup,
    };
    match dx_ide::refresh(workspace, &bazel) {
        Ok(report) => report.warning().map(dx_error::DxError::failure),
        Err(error) => Some(dx_error::DxError::new(
            error.kind(),
            format!("ide refresh failed: {error}"),
        )),
    }
}

/// The JSON warning notice for a failed refresh.
pub(crate) fn warning_event(message: &str) -> Result<serde_json::Value, dx_output::OutputError> {
    notice_event(&NoticeEvent::warning(CODE_IDE_REFRESH_FAILED, message))
}

/// Refreshes and reports a failure: a notice in JSON mode, one stderr line otherwise.
pub(crate) fn refresh_and_warn(
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    startup: &[String],
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) {
    let Some(error) = refresh(workspace, query_runner, startup) else {
        return;
    };
    let message = error.message();
    if output == OutputMode::Json {
        if emit_built(out, warning_event(message)).is_ok() {
            let _ = flush_out(out);
        }
    } else {
        let _ = writeln!(err, "dx: {CODE_IDE_REFRESH_FAILED}: {message}");
    }
}
