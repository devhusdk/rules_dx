use dx_adopt::dependency_sets::{self, Ecosystem, ResolvedSet, Selection};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendPlan {
    Run {
        argv: Vec<String>,
        env: Vec<(String, String)>,
    },
    /// Runs a generator against a staged copy of its inputs; the caller compares the output.
    Staged(StagedRun),
    /// Commands run in order; the first failure stops the rest.
    Sequence(Vec<Run>),
    Noop,
}

/// One command of a [`BackendPlan::Sequence`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Run {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// The generator a [`StagedRun`] launches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagedTool {
    Paket2Bazel,
    GoModDownload,
}

/// A generator run on copies of `inputs` in a scratch directory, never in the workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedRun {
    pub tool: StagedTool,
    /// The command up to, not including, the paths into the scratch directory.
    pub argv_head: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Workspace-relative files copied into the scratch directory by file name.
    pub inputs: Vec<String>,
    /// Workspace-relative locks the run may produce; compared by file name against `out/`.
    pub outputs: Vec<String>,
}

/// Host variables that redirect or weaken go module fetching; dx cannot override them.
const GO_HOST_ENV: [&str; 9] = [
    "GOPROXY",
    "GOPRIVATE",
    "GONOPROXY",
    "GONOSUMDB",
    "GONOSUMCHECK",
    "GOSUMDB",
    "GOINSECURE",
    "GOVCS",
    "GOAUTH",
];

/// Host variables that change which packages uv resolves or from where; dx cannot override them.
const UV_HOST_ENV: [&str; 11] = [
    "UV_INDEX_URL",
    "UV_EXTRA_INDEX_URL",
    "UV_DEFAULT_INDEX",
    "UV_INDEX",
    "UV_FIND_LINKS",
    "UV_NO_INDEX",
    "UV_INDEX_STRATEGY",
    "UV_CONFIG_FILE",
    "UV_EXCLUDE_NEWER",
    "UV_INSECURE_HOST",
    "UV_NO_BUILD_ISOLATION",
];

const UV_HOST_ENV_REASON: &str = "unset UV_INDEX_URL, UV_EXTRA_INDEX_URL, UV_DEFAULT_INDEX, UV_INDEX, UV_FIND_LINKS, UV_NO_INDEX, UV_INDEX_STRATEGY, UV_CONFIG_FILE, UV_EXCLUDE_NEWER, UV_INSECURE_HOST and UV_NO_BUILD_ISOLATION: uv reads them from the environment even with --no-config, and they change which packages resolve";

impl StagedRun {
    /// The full command for a scratch directory (an absolute path).
    pub fn argv(&self, scratch: &Path) -> Vec<String> {
        let mut argv = self.argv_head.clone();
        match self.tool {
            StagedTool::Paket2Bazel => {
                argv.push("--dependencies-file".to_owned());
                argv.push(
                    scratch
                        .join("paket.dependencies")
                        .to_string_lossy()
                        .into_owned(),
                );
                argv.push("--output-folder".to_owned());
                argv.push(scratch.join("out").to_string_lossy().into_owned());
            }
            StagedTool::GoModDownload => {
                argv.push("-C".to_owned());
                argv.push(scratch.join("out").to_string_lossy().into_owned());
                argv.extend(["mod", "download", "all"].map(str::to_owned));
            }
        }
        argv
    }

    /// The environment for a scratch directory: the planned env plus what the tool forces.
    pub fn env(&self, scratch: &Path) -> Vec<(String, String)> {
        let mut env = self.env.clone();
        if self.tool == StagedTool::GoModDownload {
            let under = |name: &str| scratch.join(name).to_string_lossy().into_owned();
            env.extend([
                ("GOENV".to_owned(), "off".to_owned()),
                ("GOWORK".to_owned(), "off".to_owned()),
                ("GOTOOLCHAIN".to_owned(), "local".to_owned()),
                ("GOFLAGS".to_owned(), "-modcacherw".to_owned()),
                ("GOMODCACHE".to_owned(), under("gomodcache")),
                ("GOPATH".to_owned(), under("gopath")),
                ("GOCACHE".to_owned(), under("gocache")),
            ]);
        }
        env
    }

    /// Names the tool in messages.
    pub fn tool_name(&self) -> &'static str {
        match self.tool {
            StagedTool::Paket2Bazel => "paket2bazel",
            StagedTool::GoModDownload => "go",
        }
    }

    /// Whether a generated file is a lock; paket2bazel's per-group extensions are not.
    pub fn is_lock(&self, name: &str) -> bool {
        !(self.tool == StagedTool::Paket2Bazel && name.ends_with("_extension.bzl"))
    }

    /// A generated file's bytes as the consumer should store them.
    pub fn rewrite_generated(&self, bytes: Vec<u8>) -> Vec<u8> {
        if self.tool != StagedTool::Paket2Bazel {
            return bytes;
        }
        const FROM: &[u8] = b"load(\"@rules_dotnet//dotnet:defs.bzl\"";
        const TO: &[u8] = b"load(\"@rules_dx//dotnet:defs.bzl\"";
        let mut out = Vec::with_capacity(bytes.len());
        let mut rest = bytes.as_slice();
        while let Some(at) = rest.windows(FROM.len()).position(|w| w == FROM) {
            out.extend_from_slice(&rest[..at]);
            out.extend_from_slice(TO);
            rest = &rest[at + FROM.len()..];
        }
        out.extend_from_slice(rest);
        out
    }

    /// The directory under `scratch` the inputs are copied into and the outputs are read from.
    pub fn inputs_dir(&self, scratch: &Path) -> std::path::PathBuf {
        match self.tool {
            StagedTool::Paket2Bazel => scratch.to_path_buf(),
            StagedTool::GoModDownload => scratch.join("out"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BackendError {
    #[error("unsupported selective update for {set}: {reason}")]
    Unsupported { set: String, reason: &'static str },
    #[error("offline_required: cannot update {set} without network (re-run without --offline once connected)")]
    OfflineRequired { set: String },
    #[error("frozen_locked: cannot change {set} resolution while frozen (re-run without --frozen to allow resolver changes)")]
    FrozenLocked { set: String },
    #[error("invalid_path: cannot update {set}: {path} is not valid UTF-8")]
    NonUtf8Path { set: String, path: String },
    #[error("backend_unavailable: cannot run {tool} for {set}: set {var} to the absolute path of {what}; PATH is never searched{}", resolution_suffix(.detail))]
    BackendUnavailable {
        set: String,
        tool: &'static str,
        var: &'static str,
        what: &'static str,
        /// Why the Bazel fallback could not resolve the tool, when it was tried.
        detail: Option<String>,
    },
}

fn resolution_suffix(detail: &Option<String>) -> String {
    detail
        .as_ref()
        .map(|detail| format!("; Bazel resolution failed: {detail}"))
        .unwrap_or_default()
}

impl BackendError {
    /// The stable machine code each message starts with (`unsupported` for
    /// the selective-update refusal, whose message reads "unsupported selective update").
    pub fn code(&self) -> &'static str {
        match self {
            BackendError::Unsupported { .. } => "unsupported",
            BackendError::OfflineRequired { .. } => "offline_required",
            BackendError::FrozenLocked { .. } => "frozen_locked",
            BackendError::NonUtf8Path { .. } => "invalid_path",
            BackendError::BackendUnavailable { .. } => "backend_unavailable",
        }
    }
}

pub const UV_BIN_VAR: &str = "DX_UV_BIN";

pub const UV_PYTHON_VAR: &str = "DX_UV_PYTHON";

pub const UV_LABEL: &str = "@dx_tools//:uv";

pub const UV_PYTHON_LABEL: &str = "@rules_dx//cli/update:python_interpreter";

/// Shell command that exports the pinned uv path for a local run.
pub const UV_RESOLVE_COMMAND: &str = "export \"$(bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:uv --var DX_UV_BIN --print)\"";

/// Shell command that exports the pinned uv Python interpreter path for a local run.
pub const UV_PYTHON_RESOLVE_COMMAND: &str = "export \"$(bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//cli/update:python_interpreter --var DX_UV_PYTHON --print)\"";

pub const UV_VERSION_LINE: &str = "uv 0.13.7";

const UV_BIN_WHAT: &str = "the pinned uv 0.13.7 from @dx_tools//:uv";

const UV_PYTHON_WHAT: &str = "a pinned Python interpreter";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UvClosure {
    pub uv: Option<String>,
    pub python: Option<String>,
    /// Why the Bazel fallback could not resolve uv, when it was tried.
    pub uv_detail: Option<String>,
    /// Why the Bazel fallback could not resolve the interpreter, when it was tried.
    pub python_detail: Option<String>,
    /// Absolute path from `DX_YARN_BIN`, for yarn.lock npm sets.
    pub yarn: Option<String>,
    /// Absolute path from `DX_NPM_BIN`, for package-lock.json npm sets.
    pub npm: Option<String>,
}

pub const YARN_BIN_VAR: &str = "DX_YARN_BIN";

pub const NPM_BIN_VAR: &str = "DX_NPM_BIN";

fn node_tool(
    set: &ResolvedSet,
    path: &Option<String>,
    tool: &'static str,
    var: &'static str,
    what: &'static str,
) -> Result<String, BackendError> {
    match path {
        Some(path) if Path::new(path).is_absolute() => Ok(path.clone()),
        _ => Err(BackendError::BackendUnavailable {
            set: set.name.clone(),
            tool,
            var,
            what,
            detail: None,
        }),
    }
}

/// The set directory as UTF-8, or an error naming the path.
fn set_dir(workspace: &Path, set: &ResolvedSet) -> Result<String, BackendError> {
    let path = match set.dir.as_str() {
        "." => workspace.to_path_buf(),
        dir => workspace.join(dir),
    };
    match path.to_str() {
        Some(dir) => Ok(dir.to_owned()),
        None => Err(BackendError::NonUtf8Path {
            set: set.name.clone(),
            path: path.display().to_string(),
        }),
    }
}

/// Builds the yarn or npm argv for a configured npm set; `None` means pnpm.
fn configured_node_argv(
    workspace: &Path,
    set: &ResolvedSet,
    closure: &UvClosure,
    command: &[String],
) -> Result<Option<Vec<String>>, BackendError> {
    let dir = set_dir(workspace, set)?;
    match dependency_sets::npm_backend(set) {
        Some(dependency_sets::NpmBackend::Yarn) => {
            let yarn = node_tool(
                set,
                &closure.yarn,
                "yarn",
                YARN_BIN_VAR,
                "a yarn 2+ (berry) executable",
            )?;
            let mut argv = strings(&["sh", "-c", "cd \"$0\" && exec \"$@\""]);
            argv.push(dir);
            argv.push(yarn);
            argv.extend(command.iter().cloned());
            Ok(Some(argv))
        }
        Some(dependency_sets::NpmBackend::Npm) => {
            let npm = node_tool(set, &closure.npm, "npm", NPM_BIN_VAR, "an npm executable")?;
            let mut argv = vec![npm, "--prefix".to_owned(), dir];
            argv.extend(command.iter().cloned());
            Ok(Some(argv))
        }
        _ => Ok(None),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckPlan {
    Run {
        argv: Vec<String>,
        env: Vec<(String, String)>,
    },
    Pinned,
    Unavailable,
    Staged(StagedRun),
    Sequence(Vec<Run>),
}

pub const NPM_CONFIG_ENV: [&str; 2] = ["npm_config_globalconfig", "npm_config_userconfig"];

/// Names the OS null device the npm config variables point at.
pub fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn npm_env() -> Vec<(String, String)> {
    NPM_CONFIG_ENV
        .iter()
        .map(|key| ((*key).to_owned(), null_device().to_owned()))
        .collect()
}

/// The env crate_universe reads to decide whether to repin; `repin` may also be one crate name,
/// which crate_universe turns into `cargo update --package <name>`. Both triggers are forced so a host
/// `REPIN` or `CARGO_BAZEL_REPIN` cannot turn a check into a write.
pub fn cargo_env(repin: &str, repository: &str) -> Vec<(String, String)> {
    vec![
        ("CARGO_BAZEL_ISOLATED".to_owned(), "true".to_owned()),
        ("CARGO_BAZEL_REPIN".to_owned(), repin.to_owned()),
        ("CARGO_BAZEL_REPIN_ONLY".to_owned(), repository.to_owned()),
        ("REPIN".to_owned(), repin.to_owned()),
    ]
}

/// The env rules_jvm_external's pin script reads. Only the maven-specific variable is set: `REPIN`
/// would also trigger crate_universe. A check sets nothing, and `host_env_refusal` keeps the host
/// from supplying either variable, because any value suppresses the stale-lock failure.
pub fn maven_env(apply: bool) -> Vec<(String, String)> {
    if apply {
        vec![("RULES_JVM_EXTERNAL_REPIN".to_owned(), "1".to_owned())]
    } else {
        Vec::new()
    }
}

const MAVEN_REPIN_ENV: [&str; 2] = ["REPIN", "RULES_JVM_EXTERNAL_REPIN"];

/// Coursier settings that would redirect where the pin resolves from; dx cannot neutralise them.
const MAVEN_COURSIER_ENV: [&str; 5] = [
    "COURSIER_MIRRORS",
    "COURSIER_REPOSITORIES",
    "COURSIER_CREDENTIALS",
    "COURSIER_CONFIG_DIR",
    "SCALA_CLI_CONFIG",
];

const GO_HOST_ENV_REASON: &str = "unset GOPROXY, GOPRIVATE, GONOPROXY, GONOSUMDB, GONOSUMCHECK, GOSUMDB, GOINSECURE, GOVCS and GOAUTH: go sets always use the default public proxy and checksum database, dx does not read go_deps.config(go_env=...), and private modules are not supported; host values can also weaken checksum verification";

/// Refuses sets whose backend would read host env dx cannot override (the runner only adds env).
/// `lookup` tells whether a variable is set in the host environment.
pub fn host_env_refusal(
    set: &ResolvedSet,
    check: bool,
    lookup: &dyn Fn(&str) -> bool,
) -> Option<BackendError> {
    if set.ecosystem == Ecosystem::Go {
        return GO_HOST_ENV
            .iter()
            .any(|key| lookup(key))
            .then(|| BackendError::Unsupported {
                set: set.name.clone(),
                reason: GO_HOST_ENV_REASON,
            });
    }
    if set.ecosystem == Ecosystem::Uv {
        return UV_HOST_ENV
            .iter()
            .any(|key| lookup(key))
            .then(|| BackendError::Unsupported {
                set: set.name.clone(),
                reason: UV_HOST_ENV_REASON,
            });
    }
    if set.ecosystem != Ecosystem::Maven {
        return None;
    }
    let reason = if check && MAVEN_REPIN_ENV.iter().any(|key| lookup(key)) {
        "unset REPIN and RULES_JVM_EXTERNAL_REPIN: any value stops rules_jvm_external from failing a stale lock"
    } else if MAVEN_COURSIER_ENV.iter().any(|key| lookup(key)) {
        "unset COURSIER_MIRRORS, COURSIER_REPOSITORIES, COURSIER_CREDENTIALS, COURSIER_CONFIG_DIR and SCALA_CLI_CONFIG: they redirect where maven artifacts resolve from"
    } else {
        return None;
    };
    Some(BackendError::Unsupported {
        set: set.name.clone(),
        reason,
    })
}

/// Fetches a configured hub repository (cargo or maven), re-evaluating its module extension.
fn configured_fetch_argv(
    head: &dx_process::BazelHead,
    repository: &str,
    offline: bool,
) -> Vec<String> {
    let repo = format!("--repo=@{repository}");
    let mut args = Vec::new();
    if offline {
        args.push("--repository_disable_download");
    }
    args.push(&repo);
    bazel(head, "fetch", &args)
}

fn set_repository<'a>(set: &'a ResolvedSet, reason: &'static str) -> Result<&'a str, BackendError> {
    set.repository
        .as_deref()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason,
        })
}

fn cargo_repository(set: &ResolvedSet) -> Result<&str, BackendError> {
    set_repository(
        set,
        "cargo sets must name their crate_universe repository hub in MODULE.bazel",
    )
}

fn maven_repository(set: &ResolvedSet) -> Result<&str, BackendError> {
    set_repository(
        set,
        "maven sets must name their rules_jvm_external repository hub in MODULE.bazel",
    )
}

const MAVEN_ONE_PACKAGE: &str =
    "maven pins are exact in MODULE.bazel; select the set without a package";

const CARGO_CHECK_SELECTIVE: &str =
    "crate_universe checks the whole Cargo.lock; select the set without a package";
const CARGO_ONE_PACKAGE: &str = "crate_universe repins one crate per run; select one package per cargo set, or the set without a package";
const CARGO_RESERVED: &str =
    "crate_universe reads this crate name as a repin mode; select the set without a package";

/// Values crate_universe reads as a repin mode (or as "skip the repin") instead of a crate name.
fn cargo_reserved_repin(name: &str) -> bool {
    const WORDS: [&str; 13] = [
        "eager",
        "full",
        "all",
        "1",
        "yes",
        "true",
        "on",
        "workspace",
        "minimal",
        "false",
        "no",
        "0",
        "off",
    ];
    WORDS.iter().any(|word| word.eq_ignore_ascii_case(name))
}

fn configured_pnpm_argv(
    workspace: &Path,
    head: &dx_process::BazelHead,
    set: &ResolvedSet,
    offline: bool,
    command: &[String],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let mut run = Vec::new();
    if offline {
        run.push("--repository_disable_download");
    }
    run.extend(["@pnpm//:pnpm", "--", "--dir"]);
    let ignore = if dependency_sets::npm_ignore_workspace(workspace, set) {
        vec!["--ignore-workspace".to_owned()]
    } else {
        Vec::new()
    };
    Ok([
        bazel(head, "run", &run),
        vec![dir],
        ignore,
        command.to_vec(),
    ]
    .concat())
}

/// Refuses a package update in a pinned set; dx never rewrites its lock.
fn pinned_selective(set: &ResolvedSet) -> BackendError {
    BackendError::Unsupported {
        set: set.name.clone(),
        reason: match set.ecosystem {
            Ecosystem::Ruby => {
                "dx does not resolve gems; edit Gemfile and regenerate Gemfile.lock with `bundle lock`"
            }
            Ecosystem::Cpan => {
                "dx refreshes cpanfile.snapshot with `carton update`; edit cpanfile and run `carton install`"
            }
            Ecosystem::Conan => {
                "dx does not resolve Conan packages; edit conanfile and regenerate conan.lock with `conan lock create`"
            }
            Ecosystem::Julia => {
                "dx does not resolve Julia packages; edit Project.toml and regenerate Manifest.toml and Manifest.bazel.json with Pkg"
            }
            Ecosystem::Composer => {
                "dx does not resolve Composer packages; edit composer.json and regenerate composer.lock with `composer update`"
            }
            Ecosystem::Opam => {
                "dx does not resolve opam packages; edit the .opam file and regenerate the .opam.locked file with `opam lock`"
            }
            _ => {
                "dx does not resolve PowerShell modules; edit PSGallery.requirements.psd1 and regenerate PSGallery.lock.json by hand"
            }
        },
    }
}

/// A process to run in a directory other than the workspace root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirRun {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    /// The directory to run in, relative to the workspace.
    pub dir: String,
}

/// The `bazel mod deps` run that refreshes the lock of a configured bazel-modules set.
///
/// The set's only lock must sit beside its manifest, or Bazel would write a file dx
/// does not track. A nested workspace gets its own short-lived server and none of the
/// user's `--output_base` or `--config`, which belong to the root workspace.
pub fn plan_configured_bazel_lock(
    set: &ResolvedSet,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<DirRun, BackendError> {
    if frozen {
        return Err(BackendError::FrozenLocked {
            set: set.name.clone(),
        });
    }
    if offline {
        return Err(BackendError::OfflineRequired {
            set: set.name.clone(),
        });
    }
    let expected = match set.dir.as_str() {
        "." => "MODULE.bazel.lock".to_owned(),
        dir => format!("{dir}/MODULE.bazel.lock"),
    };
    if set.locks != [expected] {
        return Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "bazel-modules locks must be exactly the MODULE.bazel.lock beside the manifest",
        });
    }
    let tail = strings(&["deps", "--lockfile_mode=update"]);
    if set.dir == "." {
        let mut argv = head.argv("mod");
        argv.extend(tail);
        return Ok(DirRun {
            argv,
            env: Vec::new(),
            dir: ".".to_owned(),
        });
    }
    // A nested workspace must not inherit the root's startup options.
    #[allow(clippy::disallowed_methods)]
    let mut argv = dx_process::startup_argv(&strings(&["--max_idle_secs=15"]));
    argv.push("mod".to_owned());
    argv.extend(tail);
    Ok(DirRun {
        argv,
        env: Vec::new(),
        dir: set.dir.clone(),
    })
}

/// The `bundle lock --update <gem> --conservative` run that refreshes a ruby set's lock.
///
/// It runs the pinned toolchain Ruby through Bazel in the Gemfile's directory, never a
/// host Ruby, and touches only that gem and what it needs.
pub fn plan_ruby_lock(
    set: &ResolvedSet,
    gem: &str,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<DirRun, BackendError> {
    if frozen {
        return Err(BackendError::FrozenLocked {
            set: set.name.clone(),
        });
    }
    if offline {
        return Err(BackendError::OfflineRequired {
            set: set.name.clone(),
        });
    }
    let mut argv = bazel(head, "run", &["@ruby//:bundle", "--"]);
    argv.extend(strings(&["lock", "--update", gem, "--conservative"]));
    Ok(DirRun {
        argv,
        env: Vec::new(),
        dir: set.dir.clone(),
    })
}

pub fn plan_configured(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match (set.ecosystem, request) {
        (Ecosystem::Bazel, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::Bazel, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "bazel module pins change with `dx update <set>:<module> <version>`; refresh the lock with `bazel mod deps`",
        }),
        (Ecosystem::Ruby | Ecosystem::PowerShell | Ecosystem::Conan | Ecosystem::Julia | Ecosystem::Hex | Ecosystem::Opam, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::Ruby | Ecosystem::PowerShell | Ecosystem::Conan | Ecosystem::Julia | Ecosystem::Hex | Ecosystem::Opam, Selection::Packages(_)) => {
            Err(pinned_selective(set))
        }
        (Ecosystem::Pub, request) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let mut command = vec!["pub", "upgrade"];
            if let Selection::Packages(packages) = request {
                command.extend(packages.iter().map(String::as_str));
            }
            Ok(BackendPlan::Run {
                argv: pub_argv(workspace, set, head, &command)?,
                env: pub_env(),
            })
        }
        (Ecosystem::Composer, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::Composer, Selection::Packages(packages)) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let mut command = vec!["update"];
            command.extend(packages.iter().map(String::as_str));
            command.extend(["--with-all-dependencies", "--no-install"]);
            let mut runs = vec![Run {
                argv: composer_argv(workspace, set, head, "@rules_dx//php/tools:composer", &command)?,
                env: composer_env(),
            }];
            let digests = set
                .locks
                .iter()
                .find(|lock| lock.ends_with(".json"))
                .and_then(|lock| Path::new(lock).file_name())
                .and_then(|name| name.to_str());
            if let Some(digests) = digests {
                runs.push(Run {
                    argv: composer_argv(
                        workspace,
                        set,
                        head,
                        "@rules_dx//php/tools/digests:refresh_digests",
                        &["composer.lock", digests],
                    )?,
                    env: composer_env(),
                });
            }
            Ok(BackendPlan::Sequence(runs))
        }
        (Ecosystem::Cpan, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::Cpan, Selection::Packages(modules)) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let Some(compiler) = dependency_sets::cpan_compiler_label(workspace, set) else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a cpan_compiler next to the cpanfile; it keeps cpanfile.snapshot.lock.json in step with cpanfile.snapshot",
                });
            };
            let mut command = vec!["update"];
            command.extend(modules.iter().map(String::as_str));
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: carton_argv(workspace, set, head, &command)?,
                    env: carton_env(),
                },
                Run {
                    argv: bazel(head, "run", &[&compiler, "--", "--incremental"]),
                    env: carton_env(),
                },
            ]))
        }
        (Ecosystem::Uv, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            Ok(BackendPlan::Run {
                argv: configured_uv_argv(set, closure, false, false)?,
                env: vec![],
            })
        }
        (Ecosystem::Uv, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
        (Ecosystem::Npm, request) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let backend = dependency_sets::npm_backend(set);
            let command = match (backend, request) {
                (Some(dependency_sets::NpmBackend::Yarn), Selection::Full) => {
                    strings(&["install", "--mode=update-lockfile"])
                }
                (Some(dependency_sets::NpmBackend::Yarn), Selection::Packages(packages)) => {
                    [
                        strings(&["up"]),
                        packages.clone(),
                        strings(&["--mode=update-lockfile"]),
                    ]
                    .concat()
                }
                (Some(dependency_sets::NpmBackend::Npm), Selection::Full) => {
                    strings(&["install", "--package-lock-only", "--ignore-scripts"])
                }
                (Some(dependency_sets::NpmBackend::Npm), Selection::Packages(packages)) => [
                    strings(&["update"]),
                    packages.clone(),
                    strings(&["--package-lock-only", "--ignore-scripts"]),
                ]
                .concat(),
                (_, Selection::Full) => strings(&["install", "--lockfile-only"]),
                (_, Selection::Packages(packages)) => [
                    strings(&["update"]),
                    packages.clone(),
                    strings(&["--lockfile-only"]),
                ]
                .concat(),
            };
            let argv = match configured_node_argv(workspace, set, closure, &command)? {
                Some(argv) => argv,
                None => configured_pnpm_argv(workspace, head, set, false, &command)?,
            };
            Ok(BackendPlan::Run {
                argv,
                env: npm_env(),
            })
        }
        (Ecosystem::Cargo, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let repository = cargo_repository(set)?;
            let visible = dependency_sets::cargo_fetch_repository(workspace, set)
                .unwrap_or_else(|| repository.to_owned());
            Ok(BackendPlan::Run {
                argv: configured_fetch_argv(head, &visible, false),
                env: cargo_env("workspace", repository),
            })
        }
        (Ecosystem::Maven, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let target = format!("@{}//:pin", maven_repository(set)?);
            // The pin script needs no rules_dx diagnostics settings, and a consumer
            // without rules_dx cannot resolve them.
            let pin_head = dx_process::BazelHead {
                settings: Vec::new(),
                ..head.clone()
            };
            Ok(BackendPlan::Run {
                argv: bazel(&pin_head, "run", &[&target]),
                env: maven_env(true),
            })
        }
        (Ecosystem::Maven, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: MAVEN_ONE_PACKAGE,
        }),
        // paket.lock never changes, so a frozen run is a pinned fetch and stays allowed.
        (Ecosystem::Nuget, Selection::Full) => {
            Ok(BackendPlan::Staged(nuget_staged(set, head, offline)?))
        }
        (Ecosystem::Nuget, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: NUGET_PINS_EXACT,
        }),
        (Ecosystem::Go, Selection::Full) => {
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            Ok(BackendPlan::Staged(go_staged(set, head, offline)?))
        }
        (Ecosystem::Go, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: GO_WHOLE_GRAPH,
        }),
        (Ecosystem::Cargo, Selection::Packages(packages)) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let repository = cargo_repository(set)?;
            let visible = dependency_sets::cargo_fetch_repository(workspace, set)
                .unwrap_or_else(|| repository.to_owned());
            let [package] = packages.as_slice() else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: CARGO_ONE_PACKAGE,
                });
            };
            if cargo_reserved_repin(package) {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: CARGO_RESERVED,
                });
            }
            Ok(BackendPlan::Run {
                argv: configured_fetch_argv(head, &visible, false),
                env: cargo_env(package, repository),
            })
        }
    }
}

pub fn check_configured(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    _frozen: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match (set.ecosystem, request) {
        (Ecosystem::Bazel, Selection::Full) => Ok(CheckPlan::Pinned),
        (Ecosystem::Bazel, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "bazel module pins change with `dx update <set>:<module> <version>`; refresh the lock with `bazel mod deps`",
        }),
        (Ecosystem::Ruby | Ecosystem::PowerShell | Ecosystem::Conan | Ecosystem::Julia | Ecosystem::Hex | Ecosystem::Opam, Selection::Full) => Ok(CheckPlan::Pinned),
        (Ecosystem::Ruby | Ecosystem::PowerShell | Ecosystem::Conan | Ecosystem::Julia | Ecosystem::Hex | Ecosystem::Opam, Selection::Packages(_)) => {
            Err(pinned_selective(set))
        }
        (Ecosystem::Pub, Selection::Full) => {
            let mut command = vec!["pub", "get", "--enforce-lockfile"];
            if offline {
                command.push("--offline");
            }
            Ok(CheckPlan::Run {
                argv: pub_argv(workspace, set, head, &command)?,
                env: pub_env(),
            })
        }
        (Ecosystem::Pub, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "dart pub get checks the whole pubspec.lock; select the set without a package",
        }),
        (Ecosystem::Composer, Selection::Full) => Ok(CheckPlan::Run {
            argv: composer_argv(
                workspace,
                set,
                head,
                "@rules_dx//php/tools:composer",
                &["validate", "--check-lock", "--no-check-publish"],
            )?,
            env: composer_env(),
        }),
        (Ecosystem::Composer, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "composer validates the whole composer.lock; select the set without a package",
        }),
        (Ecosystem::Cpan, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            Ok(CheckPlan::Run {
                argv: carton_argv(workspace, set, head, &["install", "--deployment"])?,
                env: carton_env(),
            })
        }
        (Ecosystem::Cpan, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "carton checks the whole cpanfile.snapshot; select the set without a package",
        }),
        (Ecosystem::Uv, Selection::Full) => Ok(CheckPlan::Run {
            argv: configured_uv_argv(set, closure, true, offline)?,
            env: vec![],
        }),
        (Ecosystem::Uv, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
        (Ecosystem::Npm, Selection::Full) => {
            let backend = dependency_sets::npm_backend(set);
            let mut command = match backend {
                Some(dependency_sets::NpmBackend::Yarn) => {
                    strings(&["install", "--immutable", "--mode=update-lockfile"])
                }
                Some(dependency_sets::NpmBackend::Npm) => {
                    strings(&["ci", "--dry-run", "--ignore-scripts"])
                }
                _ => strings(&["install", "--lockfile-only", "--frozen-lockfile"]),
            };
            if offline && backend != Some(dependency_sets::NpmBackend::Yarn) {
                command.push("--offline".to_owned());
            }
            let argv = match configured_node_argv(workspace, set, closure, &command)? {
                Some(argv) => argv,
                None => configured_pnpm_argv(workspace, head, set, offline, &command)?,
            };
            Ok(CheckPlan::Run {
                argv,
                env: npm_env(),
            })
        }
        (Ecosystem::Npm, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the npm package manager checks the whole lock; select the set without a package",
        }),
        (Ecosystem::Cargo, Selection::Full) => {
            let repository = cargo_repository(set)?;
            let visible = dependency_sets::cargo_fetch_repository(workspace, set)
                .unwrap_or_else(|| repository.to_owned());
            Ok(CheckPlan::Run {
                argv: configured_fetch_argv(head, &visible, offline),
                env: cargo_env("false", repository),
            })
        }
        (Ecosystem::Cargo, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: CARGO_CHECK_SELECTIVE,
        }),
        (Ecosystem::Maven, Selection::Full) => {
            let repository = maven_repository(set)?;
            Ok(CheckPlan::Run {
                argv: configured_fetch_argv(head, repository, offline),
                env: maven_env(false),
            })
        }
        (Ecosystem::Maven, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: MAVEN_ONE_PACKAGE,
        }),
        (Ecosystem::Go, Selection::Full) => Ok(CheckPlan::Staged(go_staged(set, head, offline)?)),
        (Ecosystem::Go, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: GO_WHOLE_GRAPH,
        }),
        (Ecosystem::Nuget, Selection::Full) => {
            Ok(CheckPlan::Staged(nuget_staged(set, head, offline)?))
        }
        (Ecosystem::Nuget, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: NUGET_PINS_EXACT,
        }),
    }
}

const NUGET_PINS_EXACT: &str = "nuget pins are exact in paket.lock; edit paket.dependencies, run `paket update`, then `dx update <set>`";

/// Plans `paket2bazel` on a staged copy of paket.dependencies and paket.lock.
fn nuget_staged(
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    offline: bool,
) -> Result<StagedRun, BackendError> {
    // Paket has no offline switch, so dx cannot keep it off the network.
    if offline {
        return Err(BackendError::OfflineRequired {
            set: set.name.clone(),
        });
    }
    // The run needs no rules_dx diagnostics settings, and a consumer without rules_dx
    // cannot resolve them.
    let pin_head = dx_process::BazelHead {
        settings: Vec::new(),
        ..head.clone()
    };
    let lock = set
        .locks
        .iter()
        .find(|lock| lock.rsplit('/').next() == Some(dependency_sets::PAKET_LOCK))
        .cloned()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: "a nuget set must list paket.lock in locks",
        })?;
    let mut inputs = set.manifests.clone();
    inputs.push(lock.clone());
    let outputs = set
        .locks
        .iter()
        .filter(|path| **path != lock)
        .cloned()
        .collect();
    Ok(StagedRun {
        tool: StagedTool::Paket2Bazel,
        argv_head: bazel(&pin_head, "run", &["@rules_dx//dotnet:paket2bazel", "--"]),
        env: Vec::new(),
        inputs,
        outputs,
    })
}

const GO_WHOLE_GRAPH: &str =
    "go sums cover the whole module graph; edit the require in go.mod and run `dx update <set>`";

/// Plans `go mod download all` on a staged copy of go.mod and go.sum.
fn go_staged(
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    offline: bool,
) -> Result<StagedRun, BackendError> {
    // Every run starts from an empty module cache, so it always needs the network.
    if offline {
        return Err(BackendError::OfflineRequired {
            set: set.name.clone(),
        });
    }
    let pin_head = dx_process::BazelHead {
        settings: Vec::new(),
        ..head.clone()
    };
    let sum = set
        .locks
        .iter()
        .find(|lock| lock.rsplit('/').next() == Some(dependency_sets::GO_SUM))
        .cloned()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: "a go set must list go.sum in locks",
        })?;
    let mut inputs = set.manifests.clone();
    inputs.push(sum.clone());
    Ok(StagedRun {
        tool: StagedTool::GoModDownload,
        argv_head: bazel(&pin_head, "run", &["@rules_go//go", "--"]),
        env: Vec::new(),
        inputs,
        outputs: vec![sum],
    })
}

fn closure_path(
    set: &ResolvedSet,
    path: &Option<String>,
    detail: &Option<String>,
    var: &'static str,
    what: &'static str,
) -> Result<String, BackendError> {
    match path {
        Some(path) if Path::new(path).is_absolute() => Ok(path.clone()),
        _ => Err(BackendError::BackendUnavailable {
            set: set.name.clone(),
            tool: "uv",
            var,
            what,
            detail: detail.clone(),
        }),
    }
}

fn configured_uv_argv(
    set: &ResolvedSet,
    closure: &UvClosure,
    check: bool,
    offline: bool,
) -> Result<Vec<String>, BackendError> {
    let uv = closure_path(
        set,
        &closure.uv,
        &closure.uv_detail,
        UV_BIN_VAR,
        UV_BIN_WHAT,
    )?;
    let python = closure_path(
        set,
        &closure.python,
        &closure.python_detail,
        UV_PYTHON_VAR,
        UV_PYTHON_WHAT,
    )?;
    let mut argv = vec![uv, "lock".to_owned()];
    if check {
        argv.push("--check".to_owned());
    }
    argv.extend(strings(&[
        "--directory",
        &set.dir,
        "--no-config",
        "--no-cache",
        "--no-python-downloads",
        "--python",
        &python,
    ]));
    if offline {
        argv.push("--offline".to_owned());
    }
    Ok(argv)
}

/// Reports whether the planned argv runs Bazel.
pub fn runs_bazel(argv: &[String]) -> bool {
    argv.first()
        .is_some_and(|program| program == dx_process::launcher_argv0())
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// A `carton` run from the toolchain in the set's directory.
///
/// `bazel run` starts in the runfiles tree, so `--run_under` moves into the set first.
fn carton_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    command: &[&str],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let under = format!("--run_under=cd '{}' &&", dir.replace('\'', "'\\''"));
    let mut argv = bazel(head, "run", &[&under, "@rules_dx//perl/tools:carton", "--"]);
    argv.extend(strings(command));
    Ok(argv)
}

fn composer_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    label: &str,
    command: &[&str],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let under = format!("--run_under=cd '{}' &&", dir.replace('\'', "'\\''"));
    let mut argv = bazel(head, "run", &[&under, label, "--"]);
    argv.extend(strings(command));
    Ok(argv)
}

/// Keeps Composer's home and download cache out of the workspace.
fn composer_env() -> Vec<(String, String)> {
    let root = std::env::temp_dir().join("dx-composer");
    vec![
        (
            "COMPOSER_HOME".to_owned(),
            root.join("home").to_string_lossy().into_owned(),
        ),
        (
            "COMPOSER_CACHE_DIR".to_owned(),
            root.join("cache").to_string_lossy().into_owned(),
        ),
    ]
}

/// Keeps carton's `local/` install tree out of the workspace.
fn carton_env() -> Vec<(String, String)> {
    let path = std::env::temp_dir().join("dx-carton-local");
    vec![(
        "PERL_CARTON_PATH".to_owned(),
        path.to_string_lossy().into_owned(),
    )]
}

/// A `dart pub` run from the pinned Dart SDK in the set's directory.
///
/// Flutter sets are refused: the pinned Flutter toolchain ships no `flutter` command.
fn pub_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    command: &[&str],
) -> Result<Vec<String>, BackendError> {
    let flutter = set.manifests.first().is_some_and(|manifest| {
        std::fs::read_to_string(workspace.join(manifest))
            .is_ok_and(|text| text.lines().any(|line| line.trim() == "sdk: flutter"))
    });
    if flutter {
        return Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the pinned Flutter toolchain has no flutter command; run `flutter pub get` or `flutter pub upgrade` in the set directory",
        });
    }
    let dir = set_dir(workspace, set)?;
    let under = format!("--run_under=cd '{}' &&", dir.replace('\'', "'\\''"));
    let mut argv = bazel(head, "run", &[&under, "@rules_dx//dart/tools:dart", "--"]);
    argv.extend(strings(command));
    Ok(argv)
}

/// Keeps pub's package cache out of the workspace.
fn pub_env() -> Vec<(String, String)> {
    let path = std::env::temp_dir().join("dx-pub-cache");
    vec![("PUB_CACHE".to_owned(), path.to_string_lossy().into_owned())]
}

fn bazel(head: &dx_process::BazelHead, verb: &str, args: &[&str]) -> Vec<String> {
    let mut argv = head.argv(verb);
    argv.extend(strings(args));
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> &'static Path {
        Path::new("/dx-workspace")
    }

    fn ws_arg() -> String {
        ws().to_string_lossy().into_owned()
    }

    fn dir_arg(rel: &str) -> String {
        ws().join(rel).to_string_lossy().into_owned()
    }

    fn no_head() -> dx_process::BazelHead {
        dx_process::BazelHead::default()
    }

    #[test]
    fn rewrite_generated_points_nuget_repo_at_rules_dx() {
        let mut run = StagedRun {
            tool: StagedTool::Paket2Bazel,
            argv_head: Vec::new(),
            env: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        let input = b"a\nload(\"@rules_dotnet//dotnet:defs.bzl\", \"nuget_repo\")\nb\n".to_vec();
        let want = b"a\nload(\"@rules_dx//dotnet:defs.bzl\", \"nuget_repo\")\nb\n".to_vec();
        assert_eq!(run.rewrite_generated(input.clone()), want);
        run.tool = StagedTool::GoModDownload;
        assert_eq!(run.rewrite_generated(input.clone()), input);
    }

    #[test]
    fn backend_error_message_starts_with_code() {
        let set = "go".to_owned();
        for error in [
            BackendError::Unsupported {
                set: set.clone(),
                reason: "why",
            },
            BackendError::OfflineRequired { set: set.clone() },
            BackendError::FrozenLocked { set: set.clone() },
            BackendError::BackendUnavailable {
                set,
                tool: "uv",
                var: UV_BIN_VAR,
                what: "uv",
                detail: None,
            },
        ] {
            assert!(error.to_string().starts_with(error.code()), "{error}");
        }
    }

    #[test]
    fn unavailable_renders_the_bazel_detail_and_keeps_the_pin_text() {
        let error = BackendError::BackendUnavailable {
            set: "s".to_owned(),
            tool: "uv",
            var: UV_BIN_VAR,
            what: "uv",
            detail: Some("exit 1: boom".to_owned()),
        };
        let text = error.to_string();
        assert!(text.contains("set DX_UV_BIN"), "{text}");
        assert!(text.contains("PATH is never searched"), "{text}");
        assert!(
            text.ends_with("; Bazel resolution failed: exit 1: boom"),
            "{text}"
        );
    }

    fn configured_set(dir: &str) -> ResolvedSet {
        ResolvedSet {
            name: "frontend".to_owned(),
            ecosystem: Ecosystem::Uv,
            dir: dir.to_owned(),
            manifests: vec![format!("{dir}/pyproject.toml")],
            locks: vec![format!("{dir}/uv.lock")],
            repository: None,
        }
    }

    fn closure() -> UvClosure {
        UvClosure {
            uv: Some(abs("uv")),
            python: Some(abs("python3")),
            ..UvClosure::default()
        }
    }

    fn abs(name: &str) -> String {
        if cfg!(windows) {
            format!("C:\\pinned\\{name}")
        } else {
            format!("/pinned/{name}")
        }
    }

    fn uv_argv(check: bool, offline: bool) -> Vec<String> {
        let mut argv = vec![abs("uv"), "lock".to_owned()];
        if check {
            argv.push("--check".to_owned());
        }
        argv.extend(strings(&[
            "--directory",
            "apps/frontend",
            "--no-config",
            "--no-cache",
            "--no-python-downloads",
            "--python",
        ]));
        argv.push(abs("python3"));
        if offline {
            argv.push("--offline".to_owned());
        }
        argv
    }

    #[test]
    fn configured_uv_plans_run_the_pinned_closure_in_the_set_directory() {
        let set = configured_set("apps/frontend");
        assert_eq!(
            plan_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                false,
                &no_head(),
                &closure()
            )
            .expect("configured update"),
            BackendPlan::Run {
                argv: uv_argv(false, false),
                env: vec![],
            }
        );
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                false,
                &no_head(),
                &closure()
            )
            .expect("configured check"),
            CheckPlan::Run {
                argv: uv_argv(true, false),
                env: vec![],
            }
        );
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &Selection::Full,
                true,
                false,
                &no_head(),
                &closure()
            )
            .expect("configured check offline"),
            CheckPlan::Run {
                argv: uv_argv(true, true),
                env: vec![],
            }
        );
    }

    #[test]
    fn configured_uv_without_a_pinned_closure_is_unavailable() {
        let set = configured_set("apps/frontend");
        let cases = [
            (UvClosure::default(), UV_BIN_VAR),
            (
                UvClosure {
                    uv: Some("uv".to_owned()),
                    python: Some(abs("python3")),
                    ..UvClosure::default()
                },
                UV_BIN_VAR,
            ),
            (
                UvClosure {
                    uv: Some(abs("uv")),
                    python: None,
                    ..UvClosure::default()
                },
                UV_PYTHON_VAR,
            ),
            (
                UvClosure {
                    uv: Some(abs("uv")),
                    python: Some("python3".to_owned()),
                    ..UvClosure::default()
                },
                UV_PYTHON_VAR,
            ),
        ];
        for (closure, var) in cases {
            let error = plan_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                false,
                &no_head(),
                &closure,
            )
            .expect_err("update needs the pinned closure");
            assert!(
                matches!(error, BackendError::BackendUnavailable { var: got, .. } if got == var),
                "{error:?}"
            );
            let message = error.to_string();
            assert!(message.starts_with("backend_unavailable:"), "{message}");
            assert!(message.contains("cannot run uv for frontend"), "{message}");
            assert!(message.contains("frontend"), "{message}");
            assert!(message.contains(var), "{message}");
            assert!(message.contains("PATH is never searched"), "{message}");
            let error = check_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                false,
                &no_head(),
                &closure,
            )
            .expect_err("check needs the pinned closure");
            assert!(
                matches!(error, BackendError::BackendUnavailable { var: got, .. } if got == var),
                "{error:?}"
            );
        }
    }

    #[test]
    fn configured_bazel_lock_plans_a_nested_mod_deps() {
        let set = bazel_set("tools", "tools/MODULE.bazel.lock");
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/x".to_owned()],
            options: vec!["--config=ci".to_owned()],
            settings: Vec::new(),
        };
        let plan = plan_configured_bazel_lock(&set, false, false, &head).expect("plan");
        assert_eq!(plan.dir, "tools");
        assert_eq!(
            plan.argv[plan.argv.len() - 3..],
            ["mod", "deps", "--lockfile_mode=update"]
        );
        assert!(plan.argv.iter().any(|arg| arg == "--max_idle_secs=15"));
        assert!(!plan.argv.iter().any(|arg| arg.contains("--output_base")));
        assert!(!plan.argv.iter().any(|arg| arg.contains("--config")));
    }

    #[test]
    fn configured_bazel_lock_at_the_root_uses_the_head() {
        let set = bazel_set(".", "MODULE.bazel.lock");
        let plan = plan_configured_bazel_lock(&set, false, false, &no_head()).expect("plan");
        assert_eq!(plan.dir, ".");
        assert_eq!(
            plan.argv[plan.argv.len() - 3..],
            ["mod", "deps", "--lockfile_mode=update"]
        );
    }

    #[test]
    fn configured_bazel_lock_refuses_frozen_offline_and_foreign_locks() {
        let set = bazel_set("tools", "tools/MODULE.bazel.lock");
        assert!(matches!(
            plan_configured_bazel_lock(&set, false, true, &no_head()),
            Err(BackendError::FrozenLocked { .. })
        ));
        assert!(matches!(
            plan_configured_bazel_lock(&set, true, false, &no_head()),
            Err(BackendError::OfflineRequired { .. })
        ));
        let foreign = bazel_set("tools", "other/MODULE.bazel.lock");
        assert!(matches!(
            plan_configured_bazel_lock(&foreign, false, false, &no_head()),
            Err(BackendError::Unsupported { .. })
        ));
    }

    fn ruby_set() -> ResolvedSet {
        ResolvedSet {
            name: "bundle".to_owned(),
            ecosystem: Ecosystem::Ruby,
            dir: "app".to_owned(),
            manifests: vec!["app/Gemfile".to_owned()],
            locks: vec!["app/Gemfile.lock".to_owned()],
            repository: None,
        }
    }

    #[test]
    fn ruby_lock_runs_bundler_from_the_toolchain_in_the_gemfile_dir() {
        let plan = plan_ruby_lock(&ruby_set(), "rake", false, false, &no_head()).expect("plan");
        assert_eq!(plan.dir, "app");
        assert!(plan.env.is_empty());
        let tail = plan.argv.iter().position(|arg| arg == "@ruby//:bundle");
        let tail = &plan.argv[tail.expect("bundle target")..];
        assert_eq!(
            tail,
            [
                "@ruby//:bundle",
                "--",
                "lock",
                "--update",
                "rake",
                "--conservative"
            ]
        );
    }

    #[test]
    fn ruby_lock_refuses_frozen_and_offline() {
        assert!(matches!(
            plan_ruby_lock(&ruby_set(), "rake", false, true, &no_head()),
            Err(BackendError::FrozenLocked { .. })
        ));
        assert!(matches!(
            plan_ruby_lock(&ruby_set(), "rake", true, false, &no_head()),
            Err(BackendError::OfflineRequired { .. })
        ));
    }

    fn composer_set(locks: &[&str]) -> ResolvedSet {
        ResolvedSet {
            name: "php".to_owned(),
            ecosystem: Ecosystem::Composer,
            dir: "php".to_owned(),
            manifests: vec!["php/composer.json".to_owned()],
            locks: locks.iter().map(|lock| (*lock).to_owned()).collect(),
            repository: None,
        }
    }

    fn composer_update(
        set: &ResolvedSet,
        offline: bool,
        frozen: bool,
    ) -> Result<BackendPlan, BackendError> {
        plan_configured(
            ws(),
            set,
            &Selection::Packages(vec!["psr/log".to_owned()]),
            offline,
            frozen,
            &no_head(),
            &UvClosure::default(),
        )
    }

    const COMPOSER_LOCKS: [&str; 2] = ["php/composer.lock", "php/composer.digests.json"];

    #[test]
    fn composer_update_runs_composer_then_the_digest_refresh() {
        let set = composer_set(&COMPOSER_LOCKS);
        let BackendPlan::Sequence(runs) = composer_update(&set, false, false).expect("plan") else {
            panic!("expected a sequence");
        };
        assert_eq!(runs.len(), 2);
        assert_eq!(
            runs[0].argv[runs[0].argv.len() - 6..],
            [
                "@rules_dx//php/tools:composer",
                "--",
                "update",
                "psr/log",
                "--with-all-dependencies",
                "--no-install",
            ]
        );
        assert_eq!(runs[0].env[0].0, "COMPOSER_HOME");
        assert_eq!(runs[0].env[1].0, "COMPOSER_CACHE_DIR");
        assert_eq!(
            runs[1].argv[runs[1].argv.len() - 4..],
            [
                "@rules_dx//php/tools/digests:refresh_digests",
                "--",
                "composer.lock",
                "composer.digests.json"
            ]
        );
    }

    #[test]
    fn composer_update_without_digests_runs_only_composer() {
        let set = composer_set(&["php/composer.lock"]);
        let BackendPlan::Sequence(runs) = composer_update(&set, false, false).expect("plan") else {
            panic!("expected a sequence");
        };
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn composer_update_refuses_frozen_and_offline() {
        let set = composer_set(&COMPOSER_LOCKS);
        assert!(matches!(
            composer_update(&set, true, false),
            Err(BackendError::OfflineRequired { .. })
        ));
        assert!(matches!(
            composer_update(&set, false, true),
            Err(BackendError::FrozenLocked { .. })
        ));
    }

    #[test]
    fn composer_check_validates_the_lock() {
        let set = composer_set(&COMPOSER_LOCKS);
        let plan = check_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect("plan");
        let CheckPlan::Run { argv, env } = plan else {
            panic!("expected a run");
        };
        assert_eq!(
            argv[argv.len() - 5..],
            [
                "@rules_dx//php/tools:composer",
                "--",
                "validate",
                "--check-lock",
                "--no-check-publish"
            ]
        );
        assert_eq!(env[0].0, "COMPOSER_HOME");
    }

    #[test]
    fn composer_check_refuses_a_package_selection() {
        let set = composer_set(&COMPOSER_LOCKS);
        let error = check_configured(
            ws(),
            &set,
            &Selection::Packages(vec!["psr/log".to_owned()]),
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect_err("refused");
        assert!(error.to_string().contains("whole composer.lock"));
    }

    fn cpan_set() -> ResolvedSet {
        ResolvedSet {
            name: "perl".to_owned(),
            ecosystem: Ecosystem::Cpan,
            dir: "app".to_owned(),
            manifests: vec!["app/cpanfile".to_owned()],
            locks: vec![
                "app/cpanfile.snapshot".to_owned(),
                "app/cpanfile.snapshot.lock.json".to_owned(),
            ],
            repository: None,
        }
    }

    fn cpan_workspace(build: &str) -> dx_test_scratch::TempDir {
        let dir = dx_test_scratch::scratch("cpan_backend");
        dx_test_scratch::write(dir.path(), "app/BUILD.bazel", build);
        dir
    }

    const COMPILER_BUILD: &str = "cpan_compiler(name = \"compiler\", cpanfile = \"cpanfile\", lockfile = \"cpanfile.snapshot.lock.json\")\n";

    fn cpan_update(
        workspace: &Path,
        offline: bool,
        frozen: bool,
    ) -> Result<BackendPlan, BackendError> {
        plan_configured(
            workspace,
            &cpan_set(),
            &Selection::Packages(vec!["Try::Tiny".to_owned()]),
            offline,
            frozen,
            &no_head(),
            &UvClosure::default(),
        )
    }

    #[test]
    fn cpan_update_runs_carton_then_the_compiler() {
        let dir = cpan_workspace(COMPILER_BUILD);
        let BackendPlan::Sequence(runs) = cpan_update(dir.path(), false, false).expect("plan")
        else {
            panic!("expected a sequence");
        };
        assert_eq!(runs.len(), 2);
        let carton = &runs[0].argv;
        let tail = carton
            .iter()
            .position(|arg| arg.starts_with("--run_under="));
        assert_eq!(
            carton[tail.expect("run_under") + 1..],
            ["@rules_dx//perl/tools:carton", "--", "update", "Try::Tiny"]
        );
        assert_eq!(runs[0].env[0].0, "PERL_CARTON_PATH");
        assert_eq!(
            runs[1].argv[runs[1].argv.len() - 3..],
            ["//app:compiler", "--", "--incremental"]
        );
    }

    #[test]
    fn cpan_update_refuses_without_a_compiler() {
        let dir = cpan_workspace("");
        let error = cpan_update(dir.path(), false, false).expect_err("refused");
        assert!(error
            .to_string()
            .contains("add a cpan_compiler next to the cpanfile"));
    }

    #[test]
    fn cpan_update_refuses_frozen_and_offline() {
        let dir = cpan_workspace(COMPILER_BUILD);
        assert!(matches!(
            cpan_update(dir.path(), true, false),
            Err(BackendError::OfflineRequired { .. })
        ));
        assert!(matches!(
            cpan_update(dir.path(), false, true),
            Err(BackendError::FrozenLocked { .. })
        ));
    }

    #[test]
    fn cpan_check_installs_the_snapshot_in_deployment_mode() {
        let plan = check_configured(
            ws(),
            &cpan_set(),
            &Selection::Full,
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect("plan");
        let CheckPlan::Run { argv, .. } = plan else {
            panic!("expected a run");
        };
        assert_eq!(argv[argv.len() - 2..], ["install", "--deployment"]);
        assert!(matches!(
            check_configured(
                ws(),
                &cpan_set(),
                &Selection::Full,
                true,
                false,
                &no_head(),
                &UvClosure::default()
            ),
            Err(BackendError::OfflineRequired { .. })
        ));
    }

    fn pub_set() -> ResolvedSet {
        ResolvedSet {
            name: "dart_deps".to_owned(),
            ecosystem: Ecosystem::Pub,
            dir: "app".to_owned(),
            manifests: vec!["app/pubspec.yaml".to_owned()],
            locks: vec!["app/pubspec.lock".to_owned()],
            repository: None,
        }
    }

    fn pub_workspace(pubspec: &str) -> dx_test_scratch::TempDir {
        let dir = dx_test_scratch::scratch("pub_backend");
        dx_test_scratch::write(dir.path(), "app/pubspec.yaml", pubspec);
        dir
    }

    fn pub_plan(
        workspace: &Path,
        request: &Selection,
        offline: bool,
        frozen: bool,
    ) -> Result<BackendPlan, BackendError> {
        plan_configured(
            workspace,
            &pub_set(),
            request,
            offline,
            frozen,
            &no_head(),
            &UvClosure::default(),
        )
    }

    #[test]
    fn pub_update_runs_dart_pub_upgrade() {
        let dir = pub_workspace("name: app\n");
        let BackendPlan::Run { argv, env } = pub_plan(
            dir.path(),
            &Selection::Packages(vec!["http".to_owned()]),
            false,
            false,
        )
        .expect("plan") else {
            panic!("expected a run");
        };
        assert_eq!(argv[argv.len() - 4..], ["--", "pub", "upgrade", "http"]);
        assert!(argv.contains(&"@rules_dx//dart/tools:dart".to_owned()));
        assert_eq!(env[0].0, "PUB_CACHE");
        let BackendPlan::Run { argv, .. } =
            pub_plan(dir.path(), &Selection::Full, false, false).expect("plan")
        else {
            panic!("expected a run");
        };
        assert_eq!(argv[argv.len() - 2..], ["pub", "upgrade"]);
    }

    #[test]
    fn pub_update_refuses_frozen_offline_and_flutter() {
        let dir = pub_workspace("name: app\n");
        assert!(matches!(
            pub_plan(dir.path(), &Selection::Full, true, false),
            Err(BackendError::OfflineRequired { .. })
        ));
        assert!(matches!(
            pub_plan(dir.path(), &Selection::Full, false, true),
            Err(BackendError::FrozenLocked { .. })
        ));
        let flutter = pub_workspace("dependencies:\n  flutter:\n    sdk: flutter\n");
        assert!(matches!(
            pub_plan(flutter.path(), &Selection::Full, false, false),
            Err(BackendError::Unsupported { .. })
        ));
    }

    #[test]
    fn pub_check_enforces_the_lockfile() {
        let dir = pub_workspace("name: app\n");
        let check = |offline| {
            check_configured(
                dir.path(),
                &pub_set(),
                &Selection::Full,
                offline,
                false,
                &no_head(),
                &UvClosure::default(),
            )
        };
        let Ok(CheckPlan::Run { argv, .. }) = check(false) else {
            panic!("expected a run");
        };
        assert_eq!(argv[argv.len() - 3..], ["pub", "get", "--enforce-lockfile"]);
        let Ok(CheckPlan::Run { argv, .. }) = check(true) else {
            panic!("expected a run");
        };
        assert_eq!(argv[argv.len() - 1], "--offline");
    }

    fn bazel_set(dir: &str, lock: &str) -> ResolvedSet {
        ResolvedSet {
            name: "bazel-modules".to_owned(),
            ecosystem: Ecosystem::Bazel,
            dir: dir.to_owned(),
            manifests: vec![format!("{dir}/MODULE.bazel")],
            locks: vec![lock.to_owned()],
            repository: None,
        }
    }

    #[test]
    fn configured_bazel_selective_points_at_update() {
        let set = ResolvedSet {
            name: "bazel-modules".to_owned(),
            ecosystem: Ecosystem::Bazel,
            dir: "tools".to_owned(),
            manifests: vec!["tools/MODULE.bazel".to_owned()],
            locks: vec!["tools/MODULE.bazel.lock".to_owned()],
            repository: None,
        };
        let request = Selection::Packages(vec!["rules_cc".to_owned()]);
        let planned = plan_configured(ws(), &set, &request, false, false, &no_head(), &closure())
            .expect_err("bazel pins are not resolved");
        let checked = check_configured(ws(), &set, &request, false, false, &no_head(), &closure())
            .expect_err("bazel pins are not checked per module");
        for error in [planned, checked] {
            assert!(
                matches!(
                    error,
                    BackendError::Unsupported { ref set, reason }
                        if set == "bazel-modules"
                            && reason.contains("dx update <set>:<module> <version>")
                            && reason.contains("bazel mod deps")
                ),
                "{error:?}"
            );
        }
    }

    fn pinned_sets() -> Vec<(ResolvedSet, &'static str, &'static str)> {
        vec![
            (
                ResolvedSet {
                    name: "gems".to_owned(),
                    ecosystem: Ecosystem::Ruby,
                    dir: "apps/rb".to_owned(),
                    manifests: vec!["apps/rb/Gemfile".to_owned()],
                    locks: vec!["apps/rb/Gemfile.lock".to_owned()],
                    repository: None,
                },
                "rspec-core",
                "`bundle lock`",
            ),
            (
                ResolvedSet {
                    name: "modules".to_owned(),
                    ecosystem: Ecosystem::PowerShell,
                    dir: "tools/ps".to_owned(),
                    manifests: vec!["tools/ps/PSGallery.requirements.psd1".to_owned()],
                    locks: vec!["tools/ps/PSGallery.lock.json".to_owned()],
                    repository: None,
                },
                "Pester",
                "PSGallery.lock.json by hand",
            ),
        ]
    }

    #[test]
    fn configured_pinned_sets_never_run_and_check_as_pinned_under_every_flag() {
        for (set, _, _) in pinned_sets() {
            for offline in [false, true] {
                for frozen in [false, true] {
                    assert_eq!(
                        plan_configured(
                            ws(),
                            &set,
                            &Selection::Full,
                            offline,
                            frozen,
                            &no_head(),
                            &closure()
                        ),
                        Ok(BackendPlan::Noop),
                        "{} offline={offline} frozen={frozen}",
                        set.name
                    );
                    assert_eq!(
                        check_configured(
                            ws(),
                            &set,
                            &Selection::Full,
                            offline,
                            frozen,
                            &no_head(),
                            &UvClosure::default()
                        ),
                        Ok(CheckPlan::Pinned),
                        "{} offline={offline} frozen={frozen}",
                        set.name
                    );
                }
            }
        }
    }

    #[test]
    fn configured_pinned_selective_names_the_lock_to_regenerate() {
        for (set, package, regenerate) in pinned_sets() {
            let request = Selection::Packages(vec![package.to_owned()]);
            let planned =
                plan_configured(ws(), &set, &request, false, false, &no_head(), &closure())
                    .expect_err("pinned sets resolve nothing");
            let checked =
                check_configured(ws(), &set, &request, false, false, &no_head(), &closure())
                    .expect_err("pinned sets check nothing per package");
            let manifest = set.manifests[0].rsplit('/').next().expect("file");
            let lock = set.locks[0].rsplit('/').next().expect("file");
            for error in [planned, checked] {
                assert!(
                    matches!(
                        error,
                        BackendError::Unsupported { set: ref name, reason }
                            if *name == set.name
                                && reason.contains(manifest)
                                && reason.contains(lock)
                                && reason.contains(regenerate)
                    ),
                    "{error:?}"
                );
                assert_eq!(error.code(), "unsupported");
            }
        }
    }

    #[test]
    fn configured_uv_selective_and_offline_update_fail_closed() {
        let set = configured_set("apps/frontend");
        let error = plan_configured(
            ws(),
            &set,
            &Selection::Packages(vec!["anyio".to_owned()]),
            false,
            false,
            &no_head(),
            &closure(),
        )
        .expect_err("configured selective stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("frontend"));
        let error = check_configured(
            ws(),
            &set,
            &Selection::Packages(vec!["anyio".to_owned()]),
            false,
            false,
            &no_head(),
            &closure(),
        )
        .expect_err("configured selective check stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        let error = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            true,
            false,
            &no_head(),
            &closure(),
        )
        .expect_err("offline update needs network");
        assert!(
            matches!(error, BackendError::OfflineRequired { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("offline_required"));
    }

    #[test]
    fn frozen_configured_update_is_locked_while_check_passes_through() {
        let set = configured_set("apps/frontend");
        let error = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            true,
            &no_head(),
            &closure(),
        )
        .expect_err("configured update frozen");
        assert!(
            matches!(error, BackendError::FrozenLocked { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("frozen_locked"), "{error}");
        assert!(error.to_string().contains("frontend"), "{error}");
        let checked = check_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            true,
            &no_head(),
            &closure(),
        )
        .expect("configured check frozen");
        assert!(
            matches!(checked, CheckPlan::Run { .. }),
            "uv lock --check verifies without resolving: {checked:?}"
        );
    }

    fn npm_set(name: &str, dir: &str) -> ResolvedSet {
        ResolvedSet {
            name: name.to_owned(),
            ecosystem: Ecosystem::Npm,
            dir: dir.to_owned(),
            manifests: vec![format!("{dir}/package.json")],
            locks: vec![format!("{dir}/pnpm-lock.yaml")],
            repository: None,
        }
    }

    fn npm_env_expected() -> Vec<(String, String)> {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        vec![
            ("npm_config_globalconfig".to_owned(), null.to_owned()),
            ("npm_config_userconfig".to_owned(), null.to_owned()),
        ]
    }

    fn npm_run(
        head: &dx_process::BazelHead,
        run: &[&str],
        dir: &str,
        command: &[&str],
    ) -> Vec<String> {
        [
            head.argv("run"),
            strings(run),
            strings(&["@pnpm//:pnpm", "--", "--dir"]),
            vec![dir_arg(dir)],
            strings(command),
        ]
        .concat()
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_set_directory_is_refused_before_any_tool_runs() {
        use std::os::unix::ffi::OsStrExt;
        let workspace = Path::new(std::ffi::OsStr::from_bytes(b"/ws\xff"));
        let set = npm_set("frontend", "apps/web");
        let err = plan_configured(
            workspace,
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect_err("non-UTF-8 dir");
        assert_eq!(err.code(), "invalid_path");
        assert!(err.to_string().contains("/ws\u{fffd}/apps/web"), "{err}");
    }

    #[test]
    fn configured_npm_update_runs_the_consumer_pnpm_in_the_set_directory() {
        let set = npm_set("frontend", "apps/web");
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/ob".to_owned()],
            options: vec![],
            settings: vec![],
        };
        let planned = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &head,
            &UvClosure::default(),
        )
        .expect("npm update");
        assert_eq!(
            planned,
            BackendPlan::Run {
                argv: npm_run(&head, &[], "apps/web", &["install", "--lockfile-only"]),
                env: npm_env_expected(),
            }
        );
        let BackendPlan::Run { argv, .. } = planned else {
            panic!("npm update runs pnpm");
        };
        assert!(runs_bazel(&argv));
        assert!(argv.contains(&"--output_base=/ob".to_owned()), "{argv:?}");
        let planned = plan_configured(
            ws(),
            &set,
            &Selection::Packages(vec!["@types/node".to_owned(), "react".to_owned()]),
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect("npm selective update");
        assert_eq!(
            planned,
            BackendPlan::Run {
                argv: npm_run(
                    &no_head(),
                    &[],
                    "apps/web",
                    &["update", "@types/node", "react", "--lockfile-only"]
                ),
                env: npm_env_expected(),
            }
        );
    }

    #[test]
    fn configured_npm_check_verifies_the_lock_without_writing() {
        let set = npm_set("frontend", "apps/web");
        for frozen in [false, true] {
            assert_eq!(
                check_configured(
                    ws(),
                    &set,
                    &Selection::Full,
                    false,
                    frozen,
                    &no_head(),
                    &UvClosure::default()
                )
                .expect("npm check"),
                CheckPlan::Run {
                    argv: npm_run(
                        &no_head(),
                        &[],
                        "apps/web",
                        &["install", "--lockfile-only", "--frozen-lockfile"]
                    ),
                    env: npm_env_expected(),
                },
                "frozen {frozen}"
            );
        }
        let checked = check_configured(
            ws(),
            &set,
            &Selection::Full,
            true,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect("npm offline check");
        assert_eq!(
            checked,
            CheckPlan::Run {
                argv: npm_run(
                    &no_head(),
                    &["--repository_disable_download"],
                    "apps/web",
                    &[
                        "install",
                        "--lockfile-only",
                        "--frozen-lockfile",
                        "--offline"
                    ]
                ),
                env: npm_env_expected(),
            }
        );
        let CheckPlan::Run { argv, .. } = checked else {
            panic!("npm check runs pnpm");
        };
        assert!(runs_bazel(&argv));
        let error = check_configured(
            ws(),
            &set,
            &Selection::Packages(vec!["react".to_owned()]),
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        )
        .expect_err("selective check");
        assert!(
            matches!(error, BackendError::Unsupported { ref set, .. } if set == "frontend"),
            "{error:?}"
        );
    }

    #[test]
    fn configured_npm_offline_and_frozen_updates_refuse_before_running() {
        let set = npm_set("frontend", "apps/web");
        for request in [
            Selection::Full,
            Selection::Packages(vec!["react".to_owned()]),
        ] {
            let error = plan_configured(
                ws(),
                &set,
                &request,
                true,
                false,
                &no_head(),
                &UvClosure::default(),
            )
            .expect_err("offline update");
            assert_eq!(
                error,
                BackendError::OfflineRequired {
                    set: "frontend".to_owned()
                }
            );
            let error = plan_configured(
                ws(),
                &set,
                &request,
                false,
                true,
                &no_head(),
                &UvClosure::default(),
            )
            .expect_err("frozen update");
            assert_eq!(
                error,
                BackendError::FrozenLocked {
                    set: "frontend".to_owned()
                }
            );
        }
    }

    #[test]
    fn configured_npm_root_set_runs_in_the_workspace_root() {
        let set = npm_set("root", ".");
        match plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &no_head(),
            &UvClosure::default(),
        ) {
            Ok(BackendPlan::Run { argv, .. }) => {
                let dir = argv
                    .iter()
                    .skip_while(|arg| *arg != "--dir")
                    .nth(1)
                    .cloned()
                    .unwrap_or_default();
                assert_eq!(dir, ws_arg());
            }
            other => panic!("{other:?}"),
        }
    }

    fn cargo_set(name: &str, dir: &str, repository: &str) -> ResolvedSet {
        let at = |file: &str| match dir {
            "." => file.to_owned(),
            _ => format!("{dir}/{file}"),
        };
        ResolvedSet {
            name: name.to_owned(),
            ecosystem: Ecosystem::Cargo,
            dir: dir.to_owned(),
            manifests: vec![at("Cargo.toml")],
            locks: vec![at("Cargo.lock"), at("cargo-bazel-lock.json")],
            repository: Some(repository.to_owned()),
        }
    }

    fn cargo_env_expected(repin: &str, repository: &str) -> Vec<(String, String)> {
        vec![
            ("CARGO_BAZEL_ISOLATED".to_owned(), "true".to_owned()),
            ("CARGO_BAZEL_REPIN".to_owned(), repin.to_owned()),
            ("CARGO_BAZEL_REPIN_ONLY".to_owned(), repository.to_owned()),
            ("REPIN".to_owned(), repin.to_owned()),
        ]
    }

    fn cargo_fetch(head: &dx_process::BazelHead, args: &[&str]) -> Vec<String> {
        [head.argv("fetch"), strings(args)].concat()
    }

    #[test]
    fn configured_cargo_update_repins_only_its_crate_universe_repository() {
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/ob".to_owned()],
            options: vec!["--config=dx_dev".to_owned()],
            settings: vec![],
        };
        for set in [
            cargo_set("crates", "apps/rs", "crates"),
            cargo_set("root", ".", "crates"),
        ] {
            let planned = plan_configured(
                ws(),
                &set,
                &Selection::Full,
                false,
                false,
                &head,
                &UvClosure::default(),
            )
            .expect("cargo update");
            assert_eq!(
                planned,
                BackendPlan::Run {
                    argv: cargo_fetch(&head, &["--repo=@crates"]),
                    env: cargo_env_expected("workspace", "crates"),
                },
                "{}",
                set.name
            );
            let BackendPlan::Run { argv, .. } = planned else {
                panic!("cargo update runs bazel");
            };
            assert!(runs_bazel(&argv));
            let verb = argv.iter().position(|arg| arg == "fetch").expect("verb");
            assert!(
                argv.iter().position(|arg| arg == "--output_base=/ob") < Some(verb),
                "{argv:?}"
            );
            assert_eq!(argv.last().map(String::as_str), Some("--repo=@crates"));
        }
    }

    #[test]
    fn configured_cargo_check_forces_repin_off() {
        let set = cargo_set("tools", "tools/rs", "tool_crates");
        for frozen in [false, true] {
            assert_eq!(
                check_configured(
                    ws(),
                    &set,
                    &Selection::Full,
                    false,
                    frozen,
                    &no_head(),
                    &UvClosure::default()
                )
                .expect("cargo check"),
                CheckPlan::Run {
                    argv: cargo_fetch(&no_head(), &["--repo=@tool_crates"]),
                    env: cargo_env_expected("false", "tool_crates"),
                },
                "frozen {frozen}"
            );
        }
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &Selection::Full,
                true,
                false,
                &no_head(),
                &UvClosure::default()
            )
            .expect("cargo offline check"),
            CheckPlan::Run {
                argv: cargo_fetch(
                    &no_head(),
                    &["--repository_disable_download", "--repo=@tool_crates"]
                ),
                env: cargo_env_expected("false", "tool_crates"),
            }
        );
    }

    #[test]
    fn configured_cargo_refuses_offline_frozen_and_package_updates() {
        let set = cargo_set("crates", "apps/rs", "crates");
        for (offline, frozen, want) in [
            (
                true,
                false,
                BackendError::OfflineRequired {
                    set: "crates".to_owned(),
                },
            ),
            (
                false,
                true,
                BackendError::FrozenLocked {
                    set: "crates".to_owned(),
                },
            ),
        ] {
            assert_eq!(
                plan_configured(
                    ws(),
                    &set,
                    &Selection::Full,
                    offline,
                    frozen,
                    &no_head(),
                    &UvClosure::default()
                ),
                Err(want)
            );
        }
        let packages = Selection::Packages(vec!["serde".to_owned()]);
        for (offline, frozen, want) in [
            (
                true,
                false,
                BackendError::OfflineRequired {
                    set: "crates".to_owned(),
                },
            ),
            (
                false,
                true,
                BackendError::FrozenLocked {
                    set: "crates".to_owned(),
                },
            ),
        ] {
            assert_eq!(
                plan_configured(
                    ws(),
                    &set,
                    &packages,
                    offline,
                    frozen,
                    &no_head(),
                    &UvClosure::default()
                ),
                Err(want)
            );
        }
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &packages,
                false,
                false,
                &no_head(),
                &UvClosure::default()
            ),
            Err(BackendError::Unsupported {
                set: "crates".to_owned(),
                reason: CARGO_CHECK_SELECTIVE,
            })
        );
    }

    fn nuget_set() -> ResolvedSet {
        ResolvedSet {
            name: "dotnet".to_owned(),
            ecosystem: Ecosystem::Nuget,
            dir: "third_party/dotnet".to_owned(),
            manifests: vec!["third_party/dotnet/paket.dependencies".to_owned()],
            locks: vec![
                "third_party/dotnet/paket.lock".to_owned(),
                "third_party/dotnet/deps/paket.main.bzl".to_owned(),
            ],
            repository: None,
        }
    }

    #[test]
    fn configured_nuget_stages_paket2bazel_without_rules_dx_settings() {
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/ob".to_owned()],
            options: vec!["--config=dx_dev".to_owned()],
            settings: vec!["--@rules_dx//config:x=true".to_owned()],
        };
        let set = nuget_set();
        let expected = StagedRun {
            tool: StagedTool::Paket2Bazel,
            argv_head: [
                dx_process::BazelHead {
                    settings: Vec::new(),
                    ..head.clone()
                }
                .argv("run"),
                strings(&["@rules_dx//dotnet:paket2bazel", "--"]),
            ]
            .concat(),
            env: Vec::new(),
            inputs: vec![
                "third_party/dotnet/paket.dependencies".to_owned(),
                "third_party/dotnet/paket.lock".to_owned(),
            ],
            outputs: vec!["third_party/dotnet/deps/paket.main.bzl".to_owned()],
        };
        let closure = UvClosure::default();
        // Frozen is allowed: paket.lock never changes, so the run is a pinned fetch.
        for frozen in [false, true] {
            assert_eq!(
                plan_configured(ws(), &set, &Selection::Full, false, frozen, &head, &closure),
                Ok(BackendPlan::Staged(expected.clone()))
            );
            assert_eq!(
                check_configured(ws(), &set, &Selection::Full, false, frozen, &head, &closure),
                Ok(CheckPlan::Staged(expected.clone()))
            );
        }
        assert!(expected.argv_head.contains(&"--output_base=/ob".to_owned()));
        assert!(!expected
            .argv_head
            .iter()
            .any(|arg| arg.starts_with("--") && arg.contains("rules_dx")));
        let argv = expected.argv(Path::new("/scratch/1"));
        assert_eq!(
            argv[expected.argv_head.len()..],
            strings(&[
                "--dependencies-file",
                "/scratch/1/paket.dependencies",
                "--output-folder",
                "/scratch/1/out"
            ])
        );
    }

    #[test]
    fn configured_nuget_requires_network_and_refuses_packages() {
        let set = nuget_set();
        let head = no_head();
        let closure = UvClosure::default();
        for frozen in [false, true] {
            let offline = Err(BackendError::OfflineRequired {
                set: "dotnet".to_owned(),
            });
            assert_eq!(
                plan_configured(ws(), &set, &Selection::Full, true, frozen, &head, &closure),
                offline
            );
            assert_eq!(
                check_configured(ws(), &set, &Selection::Full, true, frozen, &head, &closure),
                Err(BackendError::OfflineRequired {
                    set: "dotnet".to_owned()
                })
            );
        }
        let packages = Selection::Packages(vec!["Newtonsoft.Json".to_owned()]);
        let unsupported = Err(BackendError::Unsupported {
            set: "dotnet".to_owned(),
            reason: NUGET_PINS_EXACT,
        });
        assert_eq!(
            plan_configured(ws(), &set, &packages, false, false, &head, &closure),
            unsupported
        );
        assert_eq!(
            check_configured(ws(), &set, &packages, false, false, &head, &closure),
            Err(BackendError::Unsupported {
                set: "dotnet".to_owned(),
                reason: NUGET_PINS_EXACT,
            })
        );
    }

    fn go_set() -> ResolvedSet {
        ResolvedSet {
            name: "gomods".to_owned(),
            ecosystem: Ecosystem::Go,
            dir: "third_party/go".to_owned(),
            manifests: vec!["third_party/go/go.mod".to_owned()],
            locks: vec!["third_party/go/go.sum".to_owned()],
            repository: None,
        }
    }

    #[test]
    fn configured_go_stages_go_mod_download_with_forced_env() {
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/ob".to_owned()],
            options: vec!["--config=dx_dev".to_owned()],
            settings: vec!["--@rules_dx//config:x=true".to_owned()],
        };
        let set = go_set();
        let expected = StagedRun {
            tool: StagedTool::GoModDownload,
            argv_head: [
                dx_process::BazelHead {
                    settings: Vec::new(),
                    ..head.clone()
                }
                .argv("run"),
                strings(&["@rules_go//go", "--"]),
            ]
            .concat(),
            env: Vec::new(),
            inputs: vec![
                "third_party/go/go.mod".to_owned(),
                "third_party/go/go.sum".to_owned(),
            ],
            outputs: vec!["third_party/go/go.sum".to_owned()],
        };
        let closure = UvClosure::default();
        assert_eq!(
            plan_configured(ws(), &set, &Selection::Full, false, false, &head, &closure),
            Ok(BackendPlan::Staged(expected.clone()))
        );
        // Frozen check passes through; frozen apply would write go.sum.
        for frozen in [false, true] {
            assert_eq!(
                check_configured(ws(), &set, &Selection::Full, false, frozen, &head, &closure),
                Ok(CheckPlan::Staged(expected.clone()))
            );
        }
        assert_eq!(
            plan_configured(ws(), &set, &Selection::Full, false, true, &head, &closure),
            Err(BackendError::FrozenLocked {
                set: "gomods".to_owned()
            })
        );
        assert!(!expected
            .argv_head
            .iter()
            .any(|arg| arg.contains("rules_dx")));
        let scratch = Path::new("/scratch/1");
        let argv = expected.argv(scratch);
        assert_eq!(
            argv[expected.argv_head.len()..],
            strings(&["-C", "/scratch/1/out", "mod", "download", "all"])
        );
        assert_eq!(
            expected.env(scratch),
            vec![
                ("GOENV".to_owned(), "off".to_owned()),
                ("GOWORK".to_owned(), "off".to_owned()),
                ("GOTOOLCHAIN".to_owned(), "local".to_owned()),
                ("GOFLAGS".to_owned(), "-modcacherw".to_owned()),
                ("GOMODCACHE".to_owned(), "/scratch/1/gomodcache".to_owned()),
                ("GOPATH".to_owned(), "/scratch/1/gopath".to_owned()),
                ("GOCACHE".to_owned(), "/scratch/1/gocache".to_owned()),
            ]
        );
        assert_eq!(expected.inputs_dir(scratch), Path::new("/scratch/1/out"));
        assert_eq!(expected.tool_name(), "go");
    }

    #[test]
    fn configured_go_requires_network_and_refuses_packages() {
        let set = go_set();
        let head = no_head();
        let closure = UvClosure::default();
        for frozen in [false, true] {
            assert_eq!(
                check_configured(ws(), &set, &Selection::Full, true, frozen, &head, &closure),
                Err(BackendError::OfflineRequired {
                    set: "gomods".to_owned()
                })
            );
        }
        assert_eq!(
            plan_configured(ws(), &set, &Selection::Full, true, false, &head, &closure),
            Err(BackendError::OfflineRequired {
                set: "gomods".to_owned()
            })
        );
        let packages = Selection::Packages(vec!["github.com/google/go-cmp".to_owned()]);
        let want = BackendError::Unsupported {
            set: "gomods".to_owned(),
            reason: GO_WHOLE_GRAPH,
        };
        assert_eq!(
            plan_configured(ws(), &set, &packages, false, false, &head, &closure),
            Err(want.clone())
        );
        assert_eq!(
            check_configured(ws(), &set, &packages, false, false, &head, &closure),
            Err(want)
        );
    }

    #[test]
    fn go_host_env_that_redirects_or_weakens_fetching_is_refused() {
        let set = go_set();
        for key in GO_HOST_ENV {
            let refusal = host_env_refusal(&set, true, &|name| name == key);
            assert_eq!(
                refusal,
                Some(BackendError::Unsupported {
                    set: "gomods".to_owned(),
                    reason: GO_HOST_ENV_REASON,
                }),
                "{key}"
            );
        }
        assert_eq!(host_env_refusal(&set, false, &|_| false), None);
        assert_eq!(
            host_env_refusal(&set, false, &|name| name == "GOFLAGS"),
            None
        );
    }

    #[test]
    fn uv_host_env_that_changes_resolution_is_refused() {
        let set = ResolvedSet {
            name: "pypi".to_owned(),
            ecosystem: Ecosystem::Uv,
            dir: ".".to_owned(),
            manifests: vec!["pyproject.toml".to_owned()],
            locks: vec!["uv.lock".to_owned()],
            repository: None,
        };
        for key in UV_HOST_ENV {
            let refusal = host_env_refusal(&set, false, &|name| name == key);
            assert_eq!(
                refusal,
                Some(BackendError::Unsupported {
                    set: "pypi".to_owned(),
                    reason: UV_HOST_ENV_REASON,
                }),
                "{key}"
            );
        }
        assert_eq!(host_env_refusal(&set, true, &|_| false), None);
        assert_eq!(
            host_env_refusal(&set, false, &|name| name == "UV_CACHE_DIR"),
            None
        );
    }

    fn maven_set(name: &str, repository: &str) -> ResolvedSet {
        ResolvedSet {
            name: name.to_owned(),
            ecosystem: Ecosystem::Maven,
            dir: ".".to_owned(),
            manifests: vec!["MODULE.bazel".to_owned()],
            locks: vec![format!("{repository}_install.json")],
            repository: Some(repository.to_owned()),
        }
    }

    #[test]
    fn configured_maven_update_runs_only_its_pin_target() {
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/ob".to_owned()],
            options: vec!["--config=dx_dev".to_owned()],
            settings: vec!["--@rules_dx//config:x=true".to_owned()],
        };
        let set = maven_set("tools", "tool_maven");
        let plan = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &head,
            &UvClosure::default(),
        )
        .expect("maven update");
        assert_eq!(
            plan,
            BackendPlan::Run {
                argv: [
                    dx_process::BazelHead {
                        settings: Vec::new(),
                        ..head.clone()
                    }
                    .argv("run"),
                    strings(&["@tool_maven//:pin"])
                ]
                .concat(),
                env: vec![("RULES_JVM_EXTERNAL_REPIN".to_owned(), "1".to_owned())],
            }
        );
        let BackendPlan::Run { argv, env } = plan else {
            unreachable!()
        };
        assert!(argv.contains(&"--output_base=/ob".to_owned()), "{argv:?}");
        assert!(env.iter().all(|(key, _)| key != "REPIN"));
    }

    #[test]
    fn configured_maven_check_fetches_without_repin_env() {
        let set = maven_set("tools", "tool_maven");
        for (offline, args) in [
            (false, vec!["--repo=@tool_maven"]),
            (
                true,
                vec!["--repository_disable_download", "--repo=@tool_maven"],
            ),
        ] {
            assert_eq!(
                check_configured(
                    ws(),
                    &set,
                    &Selection::Full,
                    offline,
                    false,
                    &no_head(),
                    &UvClosure::default()
                )
                .expect("maven check"),
                CheckPlan::Run {
                    argv: cargo_fetch(&no_head(), &args),
                    env: Vec::new(),
                },
                "offline {offline}"
            );
        }
    }

    #[test]
    fn configured_maven_refuses_offline_frozen_and_packages() {
        let set = maven_set("jvm", "maven");
        for (offline, frozen, want) in [
            (
                true,
                false,
                BackendError::OfflineRequired {
                    set: "jvm".to_owned(),
                },
            ),
            (
                false,
                true,
                BackendError::FrozenLocked {
                    set: "jvm".to_owned(),
                },
            ),
        ] {
            assert_eq!(
                plan_configured(
                    ws(),
                    &set,
                    &Selection::Full,
                    offline,
                    frozen,
                    &no_head(),
                    &UvClosure::default()
                ),
                Err(want)
            );
        }
        let packages = Selection::Packages(vec!["com.google.guava:guava".to_owned()]);
        let unsupported = Err(BackendError::Unsupported {
            set: "jvm".to_owned(),
            reason: MAVEN_ONE_PACKAGE,
        });
        assert_eq!(
            plan_configured(
                ws(),
                &set,
                &packages,
                false,
                false,
                &no_head(),
                &UvClosure::default()
            ),
            unsupported
        );
        assert_eq!(
            check_configured(
                ws(),
                &set,
                &packages,
                false,
                false,
                &no_head(),
                &UvClosure::default()
            ),
            unsupported.map(|_: BackendPlan| CheckPlan::Pinned)
        );
    }

    #[test]
    fn maven_host_env_refusal_truth_table() {
        let maven = maven_set("jvm", "maven");
        let cargo = cargo_set("crates", "apps/rs", "crates");
        let with = |key: &'static str| move |name: &str| name == key;
        for key in ["REPIN", "RULES_JVM_EXTERNAL_REPIN"] {
            assert!(
                host_env_refusal(&maven, true, &with(key)).is_some(),
                "{key}"
            );
            assert_eq!(host_env_refusal(&maven, false, &with(key)), None, "{key}");
            assert_eq!(host_env_refusal(&cargo, true, &with(key)), None, "{key}");
        }
        for key in MAVEN_COURSIER_ENV {
            for check in [false, true] {
                assert!(
                    host_env_refusal(&maven, check, &with(key)).is_some(),
                    "{key}"
                );
            }
            assert_eq!(host_env_refusal(&cargo, true, &with(key)), None, "{key}");
        }
        assert_eq!(host_env_refusal(&maven, true, &|_| false), None);
    }

    #[test]
    fn configured_cargo_package_update_repins_one_crate() {
        let set = cargo_set("crates", "apps/rs", "crates");
        assert_eq!(
            plan_configured(
                ws(),
                &set,
                &Selection::Packages(vec!["serde".to_owned()]),
                false,
                false,
                &no_head(),
                &UvClosure::default()
            )
            .expect("cargo package plan"),
            BackendPlan::Run {
                argv: cargo_fetch(&no_head(), &["--repo=@crates"]),
                env: cargo_env_expected("serde", "crates"),
            }
        );
    }
}
