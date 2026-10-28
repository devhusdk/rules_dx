//! Resolves one pinned tool: an absolute environment path wins, an unset
//! variable falls back to Bazel, a relative one is rejected, and `PATH` is
//! never searched. The runner reads the variable once.

use dx_plan::resolve::QueryRunner;
use std::path::{Path, PathBuf};

/// Runs the cquery and info calls tool resolution needs through dx's own Bazel launcher.
pub(crate) struct QueryBazel<'a> {
    pub(crate) query_runner: &'a dyn QueryRunner,
    pub(crate) workspace: &'a Path,
    pub(crate) startup: &'a [String],
}

impl dx_tool_resolve::Bazel for QueryBazel<'_> {
    fn cquery_files(
        &self,
        label: &str,
        args: &[String],
    ) -> Result<String, dx_tool_resolve::ResolveError> {
        let failed = |detail: String| dx_tool_resolve::ResolveError::CqueryFailed {
            label: label.to_owned(),
            detail,
        };
        let mut options = vec!["--output=files".to_owned()];
        options.extend(args.iter().cloned());
        let argv = dx_plan::plan::bazel_argv(
            "cquery",
            &options,
            &[],
            &[],
            &[label.to_owned()],
            self.startup,
        )
        .map_err(|error| failed(format!("could not build the cquery argv: {error}")))?;
        let result = self
            .query_runner
            .run_query(&argv, self.workspace)
            .map_err(|error| failed(format!("could not run bazel: {error}")))?;
        if result.code != Some(0) {
            return Err(failed(dx_tool_resolve::exit_detail(
                result.code,
                &result.stderr,
            )));
        }
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    }

    #[allow(clippy::disallowed_methods)] // info takes no build settings
    fn output_base(&self) -> Result<String, dx_tool_resolve::ResolveError> {
        let mut argv = dx_process::startup_argv(self.startup);
        argv.push("info".to_owned());
        argv.push("output_base".to_owned());
        let failed = |detail: String| dx_tool_resolve::ResolveError::OutputBaseFailed { detail };
        let result = self
            .query_runner
            .run_info(&argv, self.workspace)
            .map_err(|error| failed(format!("could not run bazel: {error}")))?;
        if result.code != Some(0) {
            return Err(failed(dx_tool_resolve::exit_detail(
                result.code,
                &result.stderr,
            )));
        }
        let base = String::from_utf8_lossy(&result.stdout).trim().to_owned();
        if base.is_empty() {
            return Err(failed("bazel info output_base printed nothing".to_owned()));
        }
        Ok(base)
    }
}

/// One pinned tool: the variable that overrides it and the Bazel label that pins it.
pub(crate) struct PinnedTool<'a> {
    pub(crate) var: &'a str,
    pub(crate) label: &'a str,
}

/// Why a pinned tool could not be resolved.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PinnedToolError {
    /// The variable is set but not to an absolute path.
    Invalid,
    /// The variable is unset and Bazel could not resolve the label.
    Unresolved { detail: String },
}

/// Resolves `tool` from its variable, else through Bazel. Under `offline` the
/// resolution cquery disables repository downloads but still uses cached repos.
pub(crate) fn resolve_pinned(
    runner: &dyn dx_process::Runner,
    bazel: &dyn dx_tool_resolve::Bazel,
    tool: &PinnedTool<'_>,
    offline: bool,
) -> Result<PathBuf, PinnedToolError> {
    match runner.pinned_tool(tool.var) {
        dx_process::PinnedVar::Path(path) => return Ok(path),
        dx_process::PinnedVar::NotAbsolute => return Err(PinnedToolError::Invalid),
        dx_process::PinnedVar::Unset => {}
    }
    let args: Vec<String> = if offline {
        vec!["--repository_disable_download".to_owned()]
    } else {
        Vec::new()
    };
    dx_tool_resolve::resolve(bazel, tool.label, &args).map_err(|error| {
        let text = error.to_string();
        let prefix = format!("{}: ", dx_tool_resolve::RESOLVE_CODE);
        let detail = text.strip_prefix(&prefix).unwrap_or(&text).to_owned();
        PinnedToolError::Unresolved { detail }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io;

    struct EnvRunner(dx_process::PinnedVar);

    impl dx_process::Runner for EnvRunner {
        fn run(
            &self,
            _argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            Err(io::Error::other("unexpected run"))
        }

        fn pinned_tool(&self, _var: &str) -> dx_process::PinnedVar {
            self.0.clone()
        }
    }

    #[derive(Default)]
    struct FakeBazel {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        fail: bool,
        base: String,
    }

    /// An output base holding the file the fake cquery names.
    fn base() -> (tempfile::TempDir, String) {
        let dir = super::super::test_support::temp_dir("pinned-tool");
        let file = dir.path().join("external/tool/bin");
        std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
        std::fs::write(&file, b"").expect("file");
        let text = dir.path().to_string_lossy().into_owned();
        (dir, text)
    }

    impl dx_tool_resolve::Bazel for FakeBazel {
        fn cquery_files(
            &self,
            label: &str,
            args: &[String],
        ) -> Result<String, dx_tool_resolve::ResolveError> {
            self.calls
                .borrow_mut()
                .push((label.to_owned(), args.to_vec()));
            if self.fail {
                return Err(dx_tool_resolve::ResolveError::CqueryFailed {
                    label: label.to_owned(),
                    detail: "exit 1: boom".to_owned(),
                });
            }
            Ok("external/tool/bin\n".to_owned())
        }

        fn output_base(&self) -> Result<String, dx_tool_resolve::ResolveError> {
            Ok(self.base.clone())
        }
    }

    const TOOL: PinnedTool<'static> = PinnedTool {
        var: "DX_TEST_UNSET_PINNED_TOOL_BIN",
        label: "@dx_tools//:tool",
    };

    #[test]
    fn the_environment_path_wins_and_bazel_is_never_called() {
        let bazel = FakeBazel::default();
        let path = resolve_pinned(
            &EnvRunner(dx_process::PinnedVar::Path(PathBuf::from("/pinned/tool"))),
            &bazel,
            &TOOL,
            false,
        );
        assert_eq!(path, Ok(PathBuf::from("/pinned/tool")));
        assert!(bazel.calls.borrow().is_empty());
    }

    #[test]
    fn a_relative_value_is_invalid_and_bazel_is_never_called() {
        let bazel = FakeBazel::default();
        let result = resolve_pinned(
            &EnvRunner(dx_process::PinnedVar::NotAbsolute),
            &bazel,
            &TOOL,
            false,
        );
        assert_eq!(result, Err(PinnedToolError::Invalid));
        assert!(bazel.calls.borrow().is_empty());
    }

    #[test]
    fn unset_falls_back_with_no_extra_args_online() {
        let (_dir, base) = base();
        let bazel = FakeBazel {
            base: base.clone(),
            ..FakeBazel::default()
        };
        let path = resolve_pinned(
            &EnvRunner(dx_process::PinnedVar::Unset),
            &bazel,
            &TOOL,
            false,
        )
        .expect("resolved");
        assert!(path.starts_with(&base), "{path:?}");
        assert_eq!(
            *bazel.calls.borrow(),
            vec![("@dx_tools//:tool".to_owned(), Vec::<String>::new())]
        );
    }

    #[test]
    fn offline_disables_downloads_and_never_uses_nofetch() {
        let (_dir, base) = base();
        let bazel = FakeBazel {
            base,
            ..FakeBazel::default()
        };
        resolve_pinned(
            &EnvRunner(dx_process::PinnedVar::Unset),
            &bazel,
            &TOOL,
            true,
        )
        .expect("resolved");
        let calls = bazel.calls.borrow();
        assert_eq!(calls[0].1, vec!["--repository_disable_download".to_owned()]);
        assert!(!calls[0].1.iter().any(|arg| arg == "--nofetch"));
    }

    #[test]
    fn a_resolve_failure_keeps_the_label_and_detail() {
        let bazel = FakeBazel {
            fail: true,
            ..FakeBazel::default()
        };
        let Err(PinnedToolError::Unresolved { detail }) = resolve_pinned(
            &EnvRunner(dx_process::PinnedVar::Unset),
            &bazel,
            &TOOL,
            false,
        ) else {
            panic!("expected Unresolved");
        };
        assert!(detail.contains("@dx_tools//:tool"), "{detail}");
        assert!(detail.contains("exit 1: boom"), "{detail}");
        assert!(!detail.starts_with("tool_resolve_failed"), "{detail}");
    }
}
