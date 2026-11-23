use crate::pinned_tool::{PinnedTool, PinnedToolError, QueryBazel, resolve_pinned};

pub fn configured_uv_closure(
    runner: &dyn dx_process::Runner,
    query_runner: &dyn dx_plan::resolve::QueryRunner,
    workspace: &std::path::Path,
    startup: &[String],
    offline: bool,
) -> dx_update::backend::UvClosure {
    let bazel = QueryBazel {
        query_runner,
        workspace,
        startup,
    };
    UvTools::new(runner, &bazel, offline).closure()
}

type Resolved = (Option<String>, Option<String>);

pub struct UvTools<'a> {
    runner: &'a dyn dx_process::Runner,
    bazel: &'a dyn dx_tool_resolve::Bazel,
    offline: bool,
    uv: Option<Resolved>,
    python: Option<Resolved>,
}

impl<'a> UvTools<'a> {
    pub fn new(
        runner: &'a dyn dx_process::Runner,
        bazel: &'a dyn dx_tool_resolve::Bazel,
        offline: bool,
    ) -> Self {
        UvTools {
            runner,
            bazel,
            offline,
            uv: None,
            python: None,
        }
    }

    fn resolve(&self, var: &str, label: &str) -> Resolved {
        let tool = PinnedTool { var, label };
        match resolve_pinned(self.runner, self.bazel, &tool, self.offline) {
            Ok(path) => (Some(path.to_string_lossy().into_owned()), None),
            Err(PinnedToolError::Invalid) => (None, None),
            Err(PinnedToolError::Unresolved { detail }) => (None, Some(detail)),
        }
    }

    pub fn closure(&mut self) -> dx_update::backend::UvClosure {
        use dx_update::backend as be;
        let runner = self.runner;
        let (uv, uv_detail) = if let Some(done) = self.uv.take() {
            done
        } else {
            let (path, detail) = self.resolve(be::UV_BIN_VAR, be::UV_LABEL);
            let path = path.filter(|path| {
                runner
                    .tool_version(std::path::Path::new(path))
                    .is_some_and(|line| {
                        let name_and_version: Vec<&str> = line.split_whitespace().take(2).collect();
                        name_and_version.join(" ") == be::UV_VERSION_LINE
                    })
            });
            (path, detail)
        };
        self.uv = Some((uv.clone(), uv_detail.clone()));
        let (python, python_detail) = if uv.is_some() {
            let done = match self.python.take() {
                Some(done) => done,
                None => match be::uv_python_label() {
                    Some(label) => self.resolve(be::UV_PYTHON_VAR, label),
                    None => (
                        None,
                        Some("python/lang.json names no python_interpreter command".to_owned()),
                    ),
                },
            };
            self.python = Some(done.clone());
            done
        } else {
            (None, None)
        };
        be::UvClosure {
            uv,
            python,
            uv_detail,
            python_detail,
            ..Default::default()
        }
    }
}
