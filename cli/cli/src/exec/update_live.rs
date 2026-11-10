use dx_process::{ChildStatus, Runner};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::rc::Rc;

pub(super) struct ScriptRunner {
    pub(super) calls: Rc<RefCell<Vec<Vec<String>>>>,
    pub(super) codes: RefCell<HashMap<String, Option<i32>>>,
    pub(super) io_error: bool,
    pub(super) hermetic_envs: RefCell<Vec<Vec<(String, String)>>>,
    pub(super) pinned: bool,
    pub(super) version: Option<String>,
    pub(super) launches: RefCell<Vec<Launch>>,
    /// Files the next launch writes, as a backend that changes the tree would.
    pub(super) writes: RefCell<Vec<(std::path::PathBuf, Vec<u8>)>>,
    /// Files the next launch writes into its `--output-folder`, as a staged generator would.
    pub(super) staged_outputs: RefCell<Vec<(String, Vec<u8>)>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Launch {
    pub(super) argv: Vec<String>,
    pub(super) cwd: std::path::PathBuf,
    pub(super) env: Vec<(String, String)>,
    pub(super) hermetic: bool,
}

pub(super) fn fake_pinned(name: &str) -> String {
    if cfg!(windows) {
        format!("C:\\hermetic\\{name}")
    } else {
        format!("/hermetic/{name}")
    }
}

impl ScriptRunner {
    pub(super) fn new(codes: &[(&str, Option<i32>)]) -> Self {
        ScriptRunner {
            calls: Rc::new(RefCell::new(Vec::new())),
            codes: RefCell::new(
                codes
                    .iter()
                    .map(|(key, code)| ((*key).to_owned(), *code))
                    .collect(),
            ),
            io_error: false,
            hermetic_envs: RefCell::new(Vec::new()),
            pinned: true,
            version: Some(dx_update::backend::UV_VERSION_LINE.to_owned()),
            launches: RefCell::new(Vec::new()),
            writes: RefCell::new(Vec::new()),
            staged_outputs: RefCell::new(Vec::new()),
        }
    }

    fn launch(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        hermetic: bool,
    ) -> io::Result<ChildStatus> {
        if self.io_error {
            return Err(io::Error::other("fake launch failure"));
        }
        self.launches.borrow_mut().push(Launch {
            argv: argv.to_vec(),
            cwd: cwd.to_path_buf(),
            env: env
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            hermetic,
        });
        self.calls.borrow_mut().push(argv.to_vec());
        for (path, bytes) in self.writes.borrow_mut().drain(..) {
            std::fs::write(&path, bytes)?;
        }
        let folder = argv
            .iter()
            .skip_while(|arg| *arg != "--output-folder" && *arg != "-C")
            .nth(1)
            .cloned()
            .or_else(|| {
                argv.iter().find_map(|arg| {
                    arg.strip_prefix("--run_under=cd '")?
                        .strip_suffix("' &&")
                        .map(str::to_owned)
                })
            });
        if let Some(folder) = folder {
            for (name, bytes) in self.staged_outputs.borrow_mut().drain(..) {
                std::fs::write(Path::new(&folder).join(name), bytes)?;
            }
        }
        let key = Self::key_for(argv);
        let code = self.codes.borrow().get(&key).copied().unwrap_or(Some(0));
        Ok(ChildStatus { code, signal: None })
    }

    pub(super) fn key_for(argv: &[String]) -> String {
        if argv.first() == Some(&fake_pinned("uv")) {
            let dir = argv
                .iter()
                .skip_while(|arg| *arg != "--directory")
                .nth(1)
                .cloned()
                .unwrap_or_default();
            format!("consumer-uv:{dir}")
        } else if let Some(dir) = argv
            .contains(&"@pnpm//:pnpm".to_owned())
            .then(|| argv.iter().skip_while(|arg| *arg != "--dir").nth(1))
            .flatten()
            .filter(|dir| dx_path::posix_str(dir).contains("/consumer-npm/"))
        {
            let name = Path::new(dir)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("consumer-npm:{name}")
        } else if let Some(repository) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--repo=@"))
            .filter(|_| argv.contains(&"fetch".to_owned()))
        {
            format!("consumer-cargo:{repository}")
        } else if argv.contains(&"//rust/tests/fixtures/hello:hello".to_owned()) {
            "cargo".to_owned()
        } else if argv.contains(&"@pnpm//:pnpm".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("javascript/private/quality"))
        {
            "npm-tools".to_owned()
        } else if argv.contains(&"@pnpm//:pnpm".to_owned()) {
            "npm".to_owned()
        } else if argv.contains(&"@maven//:pin".to_owned()) {
            "maven".to_owned()
        } else if let Some(repository) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix('@')?.strip_suffix("//:pin"))
        {
            format!("consumer-maven:{repository}")
        } else if argv.iter().any(|arg| arg.contains("paket2bazel")) {
            "nuget".to_owned()
        } else if argv.contains(&"@rules_go//go".to_owned()) {
            "go".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("python/tests/fixtures/hello"))
        {
            "uv".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv.iter().any(|arg| arg.contains("core/quality/tools/python"))
        {
            "uv-tools".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("python/tests/fixtures/adopt-python"))
        {
            "uv-adopt".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("examples/adopt-polyglot"))
        {
            "uv-adopt-polyglot".to_owned()
        } else if argv
            .iter()
            .any(|arg| arg.contains("javascript/tests/fixtures/adopt-js-ts"))
        {
            "npm-adopt".to_owned()
        } else if argv
            .iter()
            .any(|arg| arg.contains("examples/adopt-polyglot"))
        {
            "npm-adopt-polyglot".to_owned()
        } else {
            argv.join(" ")
        }
    }
}

impl Runner for ScriptRunner {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.launch(argv, cwd, env, false)
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.hermetic_envs.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        self.launch(argv, cwd, env, true)
    }

    fn pinned_tool(&self, var: &str) -> dx_process::PinnedVar {
        let name = match var {
            "DX_UV_BIN" => "uv",
            "DX_UV_PYTHON" => "python3",
            _ => return dx_process::PinnedVar::Unset,
        };
        if self.pinned {
            dx_process::PinnedVar::Path(fake_pinned(name).into())
        } else {
            dx_process::PinnedVar::Unset
        }
    }

    fn tool_version(&self, _path: &Path) -> Option<String> {
        self.version.clone()
    }
}
