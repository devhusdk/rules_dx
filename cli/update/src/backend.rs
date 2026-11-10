use dx_adopt::dependency_sets::{self, Ecosystem, ResolvedSet, Selection};
use dx_error::DxError;
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
    MixDeps,
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
            StagedTool::MixDeps => {
                // mix has no -C; run the target from the staged directory.
                let target = argv
                    .iter()
                    .position(|arg| arg == "--")
                    .map_or(0, |at| at - 1);
                let dir = scratch.join("out").to_string_lossy().into_owned();
                let under = format!("--run_under=cd '{}' &&", dir.replace('\'', "'\\''"));
                argv.insert(target, under);
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
        if self.tool == StagedTool::MixDeps {
            let under = |name: &str| scratch.join(name).to_string_lossy().into_owned();
            env.extend([
                ("MIX_HOME".to_owned(), under("mix_home")),
                ("HEX_HOME".to_owned(), under("hex_home")),
                ("MIX_DEPS_PATH".to_owned(), under("deps")),
                ("MIX_BUILD_PATH".to_owned(), under("build")),
            ]);
        }
        env
    }

    /// Names the tool in messages.
    pub fn tool_name(&self) -> &'static str {
        match self.tool {
            StagedTool::Paket2Bazel => "paket2bazel",
            StagedTool::GoModDownload => "go",
            StagedTool::MixDeps => "mix",
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
            StagedTool::GoModDownload | StagedTool::MixDeps => scratch.join("out"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BackendError {
    #[error("unsupported selective update for {set}: {reason}")]
    Unsupported { set: String, reason: &'static str },
    #[error(
        "offline_required: cannot update {set} without network (re-run without --offline once connected)"
    )]
    OfflineRequired { set: String },
    #[error(
        "frozen_locked: cannot change {set} resolution while frozen (re-run without --frozen to allow resolver changes)"
    )]
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

impl From<BackendError> for DxError {
    fn from(error: BackendError) -> Self {
        match error {
            BackendError::BackendUnavailable { .. } => DxError::infra(error.to_string()),
            BackendError::Unsupported { .. }
            | BackendError::OfflineRequired { .. }
            | BackendError::FrozenLocked { .. }
            | BackendError::NonUtf8Path { .. } => DxError::failure(error.to_string()),
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
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn npm_env() -> Vec<(String, String)> {
    NPM_CONFIG_ENV
        .iter()
        .map(|key| ((*key).to_owned(), null_device().to_owned()))
        .collect()
}

/// The env crate_universe reads to decide whether to repin; `repin` may also be one crate name,
/// which crate_universe turns into `cargo update --package <name>`. Both triggers are forced so a host
/// `REPIN` or `CARGO_BAZEL_REPIN` cannot turn a check into a write. A check (`repin == "false"`) also
/// sets `CARGO_BAZEL_DIGEST_CHECK`, because crate_universe otherwise renders from a stale lock unchecked.
pub fn cargo_env(repin: &str, repository: &str) -> Vec<(String, String)> {
    let mut env = vec![
        ("CARGO_BAZEL_ISOLATED".to_owned(), "true".to_owned()),
        ("CARGO_BAZEL_REPIN".to_owned(), repin.to_owned()),
        ("CARGO_BAZEL_REPIN_ONLY".to_owned(), repository.to_owned()),
        ("REPIN".to_owned(), repin.to_owned()),
    ];
    if repin == "false" {
        env.push(("CARGO_BAZEL_DIGEST_CHECK".to_owned(), "1".to_owned()));
    }
    env
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

/// `bazel run @<repository>//:pin`, rules_jvm_external's pin script. It needs no rules_dx
/// diagnostics settings, and a consumer without rules_dx cannot resolve them.
pub fn maven_pin_argv(head: &dx_process::BazelHead, repository: &str) -> Vec<String> {
    let pin_head = dx_process::BazelHead {
        settings: Vec::new(),
        ..head.clone()
    };
    bazel(&pin_head, "run", &[&format!("@{repository}//:pin")])
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
/// `bazel fetch` of one crate_universe repository; `cargo_env` decides whether it repins.
pub fn configured_fetch_argv(
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
pub fn cargo_reserved_repin(name: &str) -> bool {
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
    let ignore = dependency_sets::npm_ignore_workspace(workspace, set);
    Ok(pnpm_run_argv(head, offline, &dir, ignore, command))
}

/// `bazel run @pnpm//:pnpm -- --dir <dir> <command>`, the hermetic pnpm.
pub fn pnpm_run_argv(
    head: &dx_process::BazelHead,
    offline: bool,
    dir: &str,
    ignore_workspace: bool,
    command: &[String],
) -> Vec<String> {
    let mut run = Vec::new();
    if offline {
        run.push("--repository_disable_download");
    }
    run.extend(["@pnpm//:pnpm", "--", "--dir"]);
    let ignore = if ignore_workspace {
        vec!["--ignore-workspace".to_owned()]
    } else {
        Vec::new()
    };
    [
        bazel(head, "run", &run),
        vec![dir.to_owned()],
        ignore,
        command.to_vec(),
    ]
    .concat()
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
            Ecosystem::Fpm => {
                "dx does not resolve fpm packages; edit fpm.toml and regenerate fpm.lock by hand"
            }
            Ecosystem::Dub => {
                "dx does not resolve DUB packages; edit dub.json, run `dub upgrade`, and refresh dub.digests.json"
            }
            Ecosystem::Zig => {
                "dx does not resolve Zig packages; edit build.zig.zon and refresh zig.digests.json"
            }
            Ecosystem::Composer => {
                "dx does not resolve Composer packages; edit composer.json and regenerate composer.lock with `composer update`"
            }
            Ecosystem::Opam => {
                "dx does not resolve opam packages; edit the .opam file and regenerate the .opam.locked file with `opam lock`"
            }
            Ecosystem::Bicep => {
                "dx does not pick Bicep module versions; edit the ref in MODULE.bazel and set digest from `bazel run //bicep/tools:pin -- <ref>`"
            }
            Ecosystem::SwiftPm => {
                "dx does not resolve Swift packages; edit Package.swift and regenerate Package.resolved with `swift package resolve`"
            }
            Ecosystem::LuaRocks => {
                "dx does not resolve LuaRocks rocks; edit luarocks.lock and luarocks.digests.json by hand"
            }
            _ => {
                "dx does not resolve PowerShell modules; edit PSGallery.requirements.psd1 and regenerate PSGallery.lock.json by hand"
            }
        },
    }
}

/// Runs the ansible_lock target once per collection, so each one moves to its latest Galaxy version.
///
/// A package that is not in the lock yet is added with its dependencies.
fn ansible_plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<BackendPlan, BackendError> {
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
    let unsupported = |reason: &'static str| BackendError::Unsupported {
        set: set.name.clone(),
        reason,
    };
    let lock = set
        .locks
        .first()
        .ok_or_else(|| unsupported("the ansible.lock tag names no lock"))?;
    let dir = lock.rsplit_once('/').map_or(".", |(dir, _)| dir);
    let Some(target) = dependency_sets::target_label_in(workspace, dir, "ansible_lock") else {
        return Err(unsupported(
            "add an ansible_lock next to the collections lock; it writes lock.json from Galaxy",
        ));
    };
    let names = match request {
        Selection::Packages(packages) => packages.clone(),
        Selection::Full => {
            let text = std::fs::read_to_string(workspace.join(lock))
                .map_err(|_| unsupported("the collections lock cannot be read"))?;
            let json: serde_json::Value = serde_json::from_str(&text)
                .map_err(|_| unsupported("the collections lock is not valid JSON"))?;
            json.get("collections")
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter_map(|entry| entry.get("name")?.as_str().map(str::to_owned))
                .collect()
        }
    };
    let runs: Vec<Run> = names
        .iter()
        .map(|name| Run {
            argv: bazel(head, "run", &[target.as_str(), "--", name.as_str()]),
            env: Vec::new(),
        })
        .collect();
    if runs.is_empty() {
        return Ok(BackendPlan::Noop);
    }
    Ok(BackendPlan::Sequence(runs))
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
        (
            Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Conan
            | Ecosystem::Dub
            | Ecosystem::Fpm
            | Ecosystem::Zig
            | Ecosystem::LuaRocks
            | Ecosystem::Bicep,
            Selection::Full,
        ) => Ok(BackendPlan::Noop),
        (
            Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Conan
            | Ecosystem::Dub
            | Ecosystem::Fpm
            | Ecosystem::Zig
            | Ecosystem::LuaRocks
            | Ecosystem::Bicep,
            Selection::Packages(_),
        ) => Err(pinned_selective(set)),
        (Ecosystem::Ansible, request) => ansible_plan(workspace, set, request, offline, frozen, head),
        (Ecosystem::Hex, _) if is_rebar(set) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "rebar.lock refresh: #9884",
        }),
        (Ecosystem::Hex, request) => {
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let packages = match request {
                Selection::Full => vec!["--all".to_owned()],
                Selection::Packages(packages) => packages.clone(),
            };
            Ok(BackendPlan::Staged(hex_staged(
                set,
                head,
                offline,
                "deps.update",
                &packages,
            )?))
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
                argv: composer_argv(
                    workspace,
                    set,
                    head,
                    "@rules_dx//php/tools:composer",
                    &command,
                )?,
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
        (Ecosystem::Cabal, Selection::Full) => {
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
            let Some(resolve) =
                dependency_sets::julia_target_label(workspace, set, "haskell_deps_resolve")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a haskell_deps_resolve next to cabal.project; it keeps cabal.project.freeze and haskell_deps.json in step",
                });
            };
            Ok(BackendPlan::Run {
                argv: bazel(head, "run", &[resolve.as_str()]),
                env: Vec::new(),
            })
        }
        (Ecosystem::Cabal, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "haskell_deps_resolve resolves the whole cabal project again; select the set without a package",
        }),
        (Ecosystem::Julia, request) => {
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
            let Some(resolve) =
                dependency_sets::julia_target_label(workspace, set, "julia_pkg_resolve")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a julia_pkg_resolve next to Manifest.bazel.json; it keeps Manifest.bazel.json in step with Manifest.toml",
                });
            };
            let dir = set_dir(workspace, set)?;
            let mut update = vec!["@rules_dx//julia/private/tools:pkg_update", "--", dir.as_str()];
            if let Selection::Packages(packages) = request {
                update.extend(packages.iter().map(String::as_str));
            }
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: bazel(head, "run", &update),
                    env: julia_env(),
                },
                Run {
                    argv: bazel(head, "run", &[resolve.as_str()]),
                    env: julia_env(),
                },
            ]))
        }
        (Ecosystem::Opam, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::SwiftPm, selection) => {
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
            let mut command = vec!["package".to_owned(), "update".to_owned()];
            if let Selection::Packages(packages) = selection {
                command.extend(packages.iter().map(|name| swift_identity(name)));
            }
            Ok(BackendPlan::Run {
                argv: swift_argv(workspace, set, head, true, &command)?,
                env: Vec::new(),
            })
        }
        (Ecosystem::Opam, Selection::Packages(packages)) => {
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
            let Some(manifest) = set.manifests.first().and_then(|m| m.rsplit('/').next()) else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "the set has no .opam file",
                });
            };
            let mut upgrade = vec!["upgrade"];
            upgrade.extend(packages.iter().map(String::as_str));
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: opam_argv(workspace, set, head, &upgrade)?,
                    env: opam_env(),
                },
                Run {
                    argv: opam_argv(workspace, set, head, &["lock", manifest])?,
                    env: opam_env(),
                },
            ]))
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
                (Some(dependency_sets::NpmBackend::Yarn), Selection::Packages(packages)) => [
                    strings(&["up"]),
                    packages.clone(),
                    strings(&["--mode=update-lockfile"]),
                ]
                .concat(),
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
            Ok(BackendPlan::Run {
                argv: maven_pin_argv(head, maven_repository(set)?),
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
        (
            Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Conan
            | Ecosystem::Dub
            | Ecosystem::Fpm
            | Ecosystem::Zig
            | Ecosystem::LuaRocks
            | Ecosystem::Ansible
            | Ecosystem::Bicep,
            Selection::Full,
        ) => Ok(CheckPlan::Pinned),
        (
            Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Conan
            | Ecosystem::Dub
            | Ecosystem::Fpm
            | Ecosystem::Zig
            | Ecosystem::LuaRocks
            | Ecosystem::Ansible
            | Ecosystem::Bicep,
            Selection::Packages(_),
        ) => Err(pinned_selective(set)),
        // Nothing refreshes a rebar.lock yet, so its check has nothing to run.
        (Ecosystem::Hex, Selection::Full) if is_rebar(set) => Ok(CheckPlan::Pinned),
        (Ecosystem::Hex, Selection::Packages(_)) if is_rebar(set) => Err(pinned_selective(set)),
        (Ecosystem::Hex, Selection::Full) => Ok(CheckPlan::Staged(hex_staged(
            set,
            head,
            offline,
            "deps.get",
            &[],
        )?)),
        (Ecosystem::Hex, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "mix deps.get checks the whole mix.lock; select the set without a package",
        }),
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
        (Ecosystem::Cabal, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            let Some(test) =
                dependency_sets::julia_target_label(workspace, set, "haskell_deps_resolve_test")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a haskell_deps_resolve_test next to cabal.project; dx checks the set by running it",
                });
            };
            Ok(CheckPlan::Run {
                argv: bazel(head, "test", &[test.as_str()]),
                env: Vec::new(),
            })
        }
        (Ecosystem::Cabal, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "haskell_deps_resolve_test checks the whole cabal project; select the set without a package",
        }),
        (Ecosystem::Julia, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            let Some(test) =
                dependency_sets::julia_target_label(workspace, set, "julia_pkg_resolve_test")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a julia_pkg_resolve_test next to Manifest.bazel.json; dx checks the set by running it",
                });
            };
            Ok(CheckPlan::Run {
                argv: bazel(head, "test", &[test.as_str()]),
                env: Vec::new(),
            })
        }
        (Ecosystem::Julia, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "julia_pkg_resolve_test checks the whole Manifest.toml; select the set without a package",
        }),
        (Ecosystem::SwiftPm, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            Ok(CheckPlan::Run {
                argv: swift_argv(
                    workspace,
                    set,
                    head,
                    false,
                    &strings(&["package", "resolve"]),
                )?,
                env: Vec::new(),
            })
        }
        (Ecosystem::SwiftPm, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "swift package resolve checks the whole Package.resolved; select the set without a package",
        }),
        (Ecosystem::Opam, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            Ok(CheckPlan::Run {
                argv: opam_argv(
                    workspace,
                    set,
                    head,
                    &[
                        "install",
                        "--locked",
                        "--deps-only",
                        "--dry-run",
                        "--show-actions",
                        ".",
                    ],
                )?,
                env: opam_env(),
            })
        }
        (Ecosystem::Opam, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "opam checks the whole .opam.locked file; select the set without a package",
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

fn is_rebar(set: &ResolvedSet) -> bool {
    dependency_sets::hex_backend(set) == Some(dependency_sets::HexBackend::Rebar3)
}

/// Plans a mix deps command on a staged copy of mix.exs and mix.lock, with the pinned Elixir
/// and empty mix and hex homes.
fn hex_staged(
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    offline: bool,
    command: &str,
    packages: &[String],
) -> Result<StagedRun, BackendError> {
    if offline {
        return Err(BackendError::OfflineRequired {
            set: set.name.clone(),
        });
    }
    let pin_head = dx_process::BazelHead {
        settings: Vec::new(),
        ..head.clone()
    };
    let lock = set
        .locks
        .iter()
        .find(|lock| lock.rsplit('/').next() == Some(dependency_sets::MIX_LOCK))
        .cloned()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: "a hex set must list mix.lock in locks; rebar.lock sets are not refreshed",
        })?;
    let mut inputs = set.manifests.clone();
    inputs.push(lock.clone());
    let mut argv_head = bazel(
        &pin_head,
        "run",
        &["@rules_dx//elixir/private/tools:mix", "--", command],
    );
    argv_head.extend(packages.iter().cloned());
    Ok(StagedRun {
        tool: StagedTool::MixDeps,
        argv_head,
        env: Vec::new(),
        inputs,
        outputs: vec![lock],
    })
}

fn closure_path(
    name: &str,
    path: &Option<String>,
    detail: &Option<String>,
    var: &'static str,
    what: &'static str,
) -> Result<String, BackendError> {
    match path {
        Some(path) if Path::new(path).is_absolute() => Ok(path.clone()),
        _ => Err(BackendError::BackendUnavailable {
            set: name.to_owned(),
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
    uv_lock_argv(&set.name, &set.dir, closure, check, offline)
}

/// `uv lock` in `dir` with the pinned uv and interpreter; `name` names the set in errors.
pub fn uv_lock_argv(
    name: &str,
    dir: &str,
    closure: &UvClosure,
    check: bool,
    offline: bool,
) -> Result<Vec<String>, BackendError> {
    let uv = closure_path(
        name,
        &closure.uv,
        &closure.uv_detail,
        UV_BIN_VAR,
        UV_BIN_WHAT,
    )?;
    let python = closure_path(
        name,
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
        dir,
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

/// Runs the script in a temp copy of the set directory, then compares or copies back `Package.resolved`.
const SWIFT_SCRIPT: &str = r#"dir=$1; apply=$2; shift 2; tmp=$(mktemp -d) || exit 1; cp -R "$dir"/. "$tmp"; (cd "$tmp" && "$@"); code=$?; if [ $code -eq 0 ] && ! cmp -s "$tmp/Package.resolved" "$dir/Package.resolved"; then if [ "$apply" = apply ]; then cp "$tmp/Package.resolved" "$dir/Package.resolved"; code=$?; else echo "Package.resolved is out of date; run dx update" >&2; code=1; fi; fi; rm -rf "$tmp"; exit $code"#;

/// The SwiftPM identity of a package named by host and path: the lowercase last segment.
fn swift_identity(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase()
}

/// The managed Swift run on a temp copy of the set directory.
fn swift_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    apply: bool,
    command: &[String],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let mode = if apply { "apply" } else { "check" };
    let under = format!(
        "--run_under=sh -c '{SWIFT_SCRIPT}' sh '{}' {mode}",
        dir.replace('\'', "'\\''")
    );
    let mut argv = bazel(head, "run", &[&under, "@rules_dx//swift/tools:swift", "--"]);
    argv.extend(command.iter().cloned());
    Ok(argv)
}

/// Keeps carton's `local/` install tree out of the workspace.
fn carton_env() -> Vec<(String, String)> {
    let path = std::env::temp_dir().join("dx-carton-local");
    vec![(
        "PERL_CARTON_PATH".to_owned(),
        path.to_string_lossy().into_owned(),
    )]
}

/// Keeps the Julia package depot out of the workspace.
fn julia_env() -> Vec<(String, String)> {
    let path = std::env::temp_dir().join("dx-julia-depot");
    vec![(
        "JULIA_DEPOT_PATH".to_owned(),
        path.to_string_lossy().into_owned(),
    )]
}

/// An `opam` run from the pinned binary and opam-repository snapshot in the set's directory.
fn opam_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    command: &[&str],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let under = format!("--run_under=cd '{}' &&", dir.replace('\'', "'\\''"));
    let mut argv = bazel(head, "run", &[&under, "@rules_dx//ocaml/tools:opam", "--"]);
    argv.extend(strings(command));
    Ok(argv)
}

/// Keeps the opam root, with the snapshot as its only repository, out of the workspace.
fn opam_env() -> Vec<(String, String)> {
    let path = std::env::temp_dir().join("dx-opam-root");
    vec![("OPAMROOT".to_owned(), path.to_string_lossy().into_owned())]
}

/// A `dart pub` run from the pinned Dart SDK in the set's directory.
///
/// Flutter sets run the Dart SDK inside the pinned Flutter toolchain through
/// `@rules_flutter//flutter:pub`, which points `FLUTTER_ROOT` at a tree for the pinned version.
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
    let dir = set_dir(workspace, set)?;
    if flutter {
        let mut argv = bazel(head, "run", &["@rules_flutter//flutter:pub", "--"]);
        argv.extend(strings(command));
        argv.extend(["--directory".to_owned(), dir]);
        return Ok(argv);
    }
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
#[path = "backend_tests.rs"]
mod tests;
