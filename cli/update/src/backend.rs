use dx_adopt::dependency_sets::{self, Ecosystem, ResolvedSet, Selection};
use dx_error::DxError;
use std::path::Path;

mod bazel;
mod cabal;
mod cargo;
mod composer;
mod cpan;
mod go;
mod hex;
mod julia;
mod maven;
mod npm;
mod opam;
mod pub_dev;
mod resolver;
mod ruby;
mod swiftpm;
mod uv;

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
    GoModDownload,
    MixDeps,
    Rebar3,
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
    pub fn argv(&self, scratch: &Path) -> Result<Vec<String>, BackendError> {
        let mut argv = self.argv_head.clone();
        match self.tool {
            StagedTool::GoModDownload => {
                argv.push("-C".to_owned());
                argv.push(scratch.join("out").to_string_lossy().into_owned());
                argv.extend(["mod", "download", "all"].map(str::to_owned));
            }
            StagedTool::MixDeps | StagedTool::Rebar3 => {
                // mix and rebar3 have no -C; run the target from the staged directory.
                let target = argv
                    .iter()
                    .position(|arg| arg == "--")
                    .map_or(0, |at| at - 1);
                let dir = scratch.join("out").to_string_lossy().into_owned();
                let under = format!("--run_under=cd {} &&", dx_process::shell::quote(&dir));
                argv.insert(target, under);
            }
        }
        Ok(argv)
    }

    /// The environment for a scratch directory: the planned env plus what the tool forces.
    #[must_use]
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
        if self.tool == StagedTool::Rebar3 {
            let under = |name: &str| scratch.join(name).to_string_lossy().into_owned();
            env.extend([
                ("REBAR_CACHE_DIR".to_owned(), under("rebar_cache")),
                ("REBAR_GLOBAL_CONFIG_DIR".to_owned(), under("rebar_config")),
                ("REBAR_BASE_DIR".to_owned(), under("rebar_base")),
                ("HEX_HOME".to_owned(), under("hex_home")),
            ]);
        }
        env
    }

    /// Names the tool in messages.
    #[must_use]
    pub fn tool_name(&self) -> &'static str {
        match self.tool {
            StagedTool::GoModDownload => "go",
            StagedTool::MixDeps => "mix",
            StagedTool::Rebar3 => "rebar3",
        }
    }

    /// The directory under `scratch` the inputs are copied into and the outputs are read from.
    #[must_use]
    pub fn inputs_dir(&self, scratch: &Path) -> std::path::PathBuf {
        scratch.join("out")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BackendError {
    #[error("{detail}")]
    Hubs { detail: String },
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
    #[error(
        "no_cache_dir: cannot run {tool} for {set}: set XDG_CACHE_HOME, HOME or LOCALAPPDATA to give it a cache directory"
    )]
    NoCacheDir { set: String, tool: &'static str },
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
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            BackendError::Unsupported { .. } => "unsupported",
            BackendError::OfflineRequired { .. } => "offline_required",
            BackendError::FrozenLocked { .. } => "frozen_locked",
            BackendError::NonUtf8Path { .. } => "invalid_path",
            BackendError::BackendUnavailable { .. } => "backend_unavailable",
            BackendError::NoCacheDir { .. } => "no_cache_dir",
            BackendError::Hubs { .. } => "setup_hubs",
        }
    }
}

impl From<BackendError> for DxError {
    fn from(error: BackendError) -> Self {
        match error {
            BackendError::BackendUnavailable { .. } | BackendError::NoCacheDir { .. } => {
                DxError::infra(error.to_string())
            }
            BackendError::Hubs { .. }
            | BackendError::Unsupported { .. }
            | BackendError::OfflineRequired { .. }
            | BackendError::FrozenLocked { .. }
            | BackendError::NonUtf8Path { .. } => DxError::failure(error.to_string()),
        }
    }
}

pub const UV_BIN_VAR: &str = "DX_UV_BIN";

pub const UV_PYTHON_VAR: &str = "DX_UV_PYTHON";

pub const UV_LABEL: &str = "@dx_tools//:uv";

/// The label of the Python interpreter uv runs under.
#[must_use]
pub fn uv_python_label() -> Option<&'static str> {
    dx_families::command("python", "python_interpreter")
}

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

/// The `commands` label the `lang.json` of `lang` names for `tool`.
pub(crate) fn lang_command(
    set: &ResolvedSet,
    lang: &str,
    tool: &str,
) -> Result<&'static str, BackendError> {
    dx_families::command(lang, tool).ok_or_else(|| BackendError::Unsupported {
        set: set.name.clone(),
        reason: "the lang.json of this ecosystem names no command for its tool",
    })
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
    match dependency_sets::npm_backend(set)? {
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
#[must_use]
pub fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn npm_env() -> Vec<(String, String)> {
    NPM_CONFIG_ENV
        .iter()
        .map(|key| ((*key).to_owned(), null_device().to_owned()))
        .collect()
}

/// The env `crate_universe` reads to decide whether to repin; `repin` may also be one crate name,
/// which `crate_universe` turns into `cargo update --package <name>`. Both triggers are forced so a host
/// `REPIN` or `CARGO_BAZEL_REPIN` cannot turn a check into a write. A check (`repin == "false"`) also
/// sets `CARGO_BAZEL_DIGEST_CHECK`, because `crate_universe` otherwise renders from a stale lock unchecked.
#[must_use]
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

/// The env `rules_jvm_external`'s pin script reads. Only the maven-specific variable is set: `REPIN`
/// would also trigger `crate_universe`. A check sets nothing, and `host_env_refusal` keeps the host
/// from supplying either variable, because any value suppresses the stale-lock failure.
#[must_use]
pub fn maven_env(apply: bool) -> Vec<(String, String)> {
    if apply {
        vec![("RULES_JVM_EXTERNAL_REPIN".to_owned(), "1".to_owned())]
    } else {
        Vec::new()
    }
}

/// `bazel run @<repository>//:pin`, `rules_jvm_external`'s pin script. It needs no `rules_dx`
/// diagnostics settings, and a consumer without `rules_dx` cannot resolve them.
#[must_use]
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
    match set.ecosystem {
        Ecosystem::Go => go::host_env_refusal(set, lookup),
        Ecosystem::Uv => uv::host_env_refusal(set, lookup),
        Ecosystem::Maven => maven::host_env_refusal(set, check, lookup),
        Ecosystem::Npm
        | Ecosystem::Bazel
        | Ecosystem::Ruby
        | Ecosystem::PowerShell
        | Ecosystem::Cargo
        | Ecosystem::Nuget
        | Ecosystem::Cpan
        | Ecosystem::Pub
        | Ecosystem::Julia
        | Ecosystem::Hex
        | Ecosystem::Composer
        | Ecosystem::Dub
        | Ecosystem::Fpm
        | Ecosystem::Zig
        | Ecosystem::LuaRocks
        | Ecosystem::Opam
        | Ecosystem::SwiftPm
        | Ecosystem::Cabal
        | Ecosystem::Ansible
        | Ecosystem::Bicep => None,
    }
}

/// Fetches a configured hub repository (cargo or maven), re-evaluating its module extension.
/// `bazel fetch` of one `crate_universe` repository; `cargo_env` decides whether it repins.
#[must_use]
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
    "crate_universe checks the whole lock file; select the set without a package";
const CARGO_ONE_PACKAGE: &str = "crate_universe repins one crate per run; select one package per cargo set, or the set without a package";
const CARGO_RESERVED: &str =
    "crate_universe reads this crate name as a repin mode; select the set without a package";

/// Values `crate_universe` reads as a repin mode (or as "skip the repin") instead of a crate name.
#[must_use]
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
    let ignore = dependency_sets::npm_ignore_workspace(workspace, set)?;
    Ok(pnpm_run_argv(head, offline, &dir, ignore, command))
}

/// `@<pnpm hub>//:pnpm`, the hermetic pnpm binary.
#[must_use]
pub fn pnpm_label() -> String {
    let hub = dx_families::hub("pnpm").map_or("", |hub| hub.hub.as_str());
    format!("@{hub}//:pnpm")
}

/// `bazel run <pnpm label> -- --dir <dir> <command>`, the hermetic pnpm.
#[must_use]
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
    let pnpm = pnpm_label();
    run.extend([pnpm.as_str(), "--", "--dir"]);
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

/// `bazel run <resolver> -- --dir <dir>`, the pinned resolver that writes a lock beside its manifest.
#[must_use]
pub fn resolver_run_argv(head: &dx_process::BazelHead, resolver: &str, dir: &str) -> Vec<String> {
    bazel(head, "run", &[resolver, "--", "--dir", dir])
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
    #[expect(
        clippy::disallowed_methods,
        reason = "the argv carries no build settings"
    )]
    let mut argv = dx_process::startup_argv(&strings(&["--max_idle_secs=15"]));
    argv.push("mod".to_owned());
    argv.extend(tail);
    Ok(DirRun {
        argv,
        env: Vec::new(),
        dir: set.dir.clone(),
    })
}

/// The pinned ruby resolver run that re-resolves `gem` of a ruby set and writes its locks.
pub fn plan_ruby_lock(
    workspace: &Path,
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
    let resolver = dx_families::hub("ruby")
        .and_then(|hub| hub.resolver.as_deref())
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the ruby registry entry names no resolver",
        })?;
    let dir = workspace.join(&set.dir);
    let mut argv = resolver_run_argv(head, resolver, &dir.to_string_lossy());
    argv.extend(strings(&["--upgrade", gem]));
    Ok(DirRun {
        argv,
        env: resolver_env(workspace, head),
        dir: set.dir.clone(),
    })
}

/// The variable that names the downloader config Bazel uses; lock tools fetch through it.
pub const DOWNLOADER_CONFIG_ENV: &str = "DX_DOWNLOADER_CONFIG";

/// The environment a resolver run adds: the absolute downloader config `bazel run` uses, if any.
#[must_use]
pub fn resolver_env(workspace: &Path, head: &dx_process::BazelHead) -> Vec<(String, String)> {
    dx_process::downloader_config(
        &|name| std::env::var(name).ok(),
        workspace,
        &dx_process::rc::read_file,
        dx_process::current().read_user_rc,
        &head.startup,
        &head.options,
    )
    .map(|path| {
        vec![(
            DOWNLOADER_CONFIG_ENV.to_owned(),
            path.to_string_lossy().into_owned(),
        )]
    })
    .unwrap_or_default()
}

fn with_env(env: &mut Vec<(String, String)>, extra: &[(String, String)]) {
    env.extend(extra.iter().cloned());
}

/// Adds `extra` to the environment of every command in `plan`.
fn plan_with_env(mut plan: BackendPlan, extra: &[(String, String)]) -> BackendPlan {
    match &mut plan {
        BackendPlan::Run { env, .. } => with_env(env, extra),
        BackendPlan::Staged(run) => with_env(&mut run.env, extra),
        BackendPlan::Sequence(runs) => runs
            .iter_mut()
            .for_each(|run| with_env(&mut run.env, extra)),
        BackendPlan::Noop => {}
    }
    plan
}

/// Adds `extra` to the environment of every command in `plan`.
fn check_with_env(mut plan: CheckPlan, extra: &[(String, String)]) -> CheckPlan {
    match &mut plan {
        CheckPlan::Run { env, .. } => with_env(env, extra),
        CheckPlan::Staged(run) => with_env(&mut run.env, extra),
        CheckPlan::Sequence(runs) => runs
            .iter_mut()
            .for_each(|run| with_env(&mut run.env, extra)),
        CheckPlan::Pinned | CheckPlan::Unavailable => {}
    }
    plan
}

/// The plan that refreshes `set`. Every command it runs gets [`resolver_env`].
pub fn plan_configured(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    let plan = plan_backend(workspace, set, request, offline, frozen, head, closure)?;
    Ok(plan_with_env(plan, &resolver_env(workspace, head)))
}

fn plan_backend(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match backend_of(workspace, set, set.ecosystem.plan_backend()) {
        "uv" => uv::plan(workspace, set, request, offline, frozen, head, closure),
        "npm" => npm::plan(workspace, set, request, offline, frozen, head, closure),
        "bazel" => bazel::plan(workspace, set, request, offline, frozen, head, closure),
        "cargo" => cargo::plan(workspace, set, request, offline, frozen, head, closure),
        "maven" => maven::plan(workspace, set, request, offline, frozen, head, closure),
        "go" => go::plan(workspace, set, request, offline, frozen, head, closure),
        "cpan" => cpan::plan(workspace, set, request, offline, frozen, head, closure),
        "pub_dev" => pub_dev::plan(workspace, set, request, offline, frozen, head, closure),
        "julia" => julia::plan(workspace, set, request, offline, frozen, head, closure),
        "hex" => hex::plan(workspace, set, request, offline, frozen, head, closure),
        "composer" => composer::plan(workspace, set, request, offline, frozen, head, closure),
        "opam" => opam::plan(workspace, set, request, offline, frozen, head, closure),
        "swiftpm" => swiftpm::plan(workspace, set, request, offline, frozen, head, closure),
        "cabal" => cabal::plan(workspace, set, request, offline, frozen, head, closure),
        "ruby" => ruby::plan(workspace, set, request, offline, frozen, head, closure),
        "resolver" => resolver::plan(workspace, set, request, offline, frozen, head),
        _ => Err(no_backend(set)),
    }
}

/// The plan that checks `set`. Every command it runs gets [`resolver_env`].
pub fn check_configured(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    _frozen: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    let plan = check_backend(workspace, set, request, offline, head, closure)?;
    Ok(check_with_env(plan, &resolver_env(workspace, head)))
}

fn check_backend(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match backend_of(workspace, set, set.ecosystem.check_backend()) {
        "uv" => uv::check(workspace, set, request, offline, head, closure),
        "npm" => npm::check(workspace, set, request, offline, head, closure),
        "bazel" => bazel::check(workspace, set, request, offline, head, closure),
        "cargo" => cargo::check(workspace, set, request, offline, head, closure),
        "maven" => maven::check(workspace, set, request, offline, head, closure),
        "go" => go::check(workspace, set, request, offline, head, closure),
        "cpan" => cpan::check(workspace, set, request, offline, head, closure),
        "pub_dev" => pub_dev::check(workspace, set, request, offline, head, closure),
        "julia" => julia::check(workspace, set, request, offline, head, closure),
        "hex" => hex::check(workspace, set, request, offline, head, closure),
        "composer" => composer::check(workspace, set, request, offline, head, closure),
        "opam" => opam::check(workspace, set, request, offline, head, closure),
        "swiftpm" => swiftpm::check(workspace, set, request, offline, head, closure),
        "cabal" => cabal::check(workspace, set, request, offline, head, closure),
        "ruby" => ruby::check(workspace, set, request, offline, head, closure),
        "resolver" => resolver::check(workspace, set, request, offline, head),
        _ => Err(no_backend(set)),
    }
}

/// The backend that runs for `set`, given its `ecosystems.json` column.
///
/// The dart lock step has only the Dart SDK, so pub sets on the Flutter SDK run `pub` from
/// rules_flutter. Maven sets the java lock step writes go through the registry resolver; sets
/// pinned by `rules_jvm_external` keep their `pin` run.
fn backend_of(workspace: &Path, set: &ResolvedSet, column: &'static str) -> &'static str {
    if set.ecosystem == Ecosystem::Pub && uses_flutter(workspace, set) {
        "pub_dev"
    } else if dependency_sets::maven_resolver_set(workspace, set) {
        "resolver"
    } else {
        column
    }
}

/// Whether the pubspec of `set` depends on the Flutter SDK.
fn uses_flutter(workspace: &Path, set: &ResolvedSet) -> bool {
    set.manifests.first().is_some_and(|manifest| {
        std::fs::read_to_string(workspace.join(manifest))
            .is_ok_and(|text| text.lines().any(|line| line.trim() == "sdk: flutter"))
    })
}

/// Refuses a set whose `ecosystems.json` row names a backend this crate does not have.
fn no_backend(set: &ResolvedSet) -> BackendError {
    BackendError::Unsupported {
        set: set.name.clone(),
        reason: "the ecosystems.json row of this ecosystem names no update backend",
    }
}

fn hex_lock_missing() -> Result<&'static str, BackendError> {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    if let Some(text) = TEXT.get() {
        return Ok(text);
    }
    let text = format!(
        "a hex set must list {} in locks; rebar.lock sets are not refreshed",
        dx_adopt::lock_hubs::lock_of("hex")?
    );
    Ok(TEXT.get_or_init(|| text))
}

fn hex_whole_lock() -> Result<&'static str, BackendError> {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    if let Some(text) = TEXT.get() {
        return Ok(text);
    }
    let text = format!(
        "mix deps.get checks the whole {}; select the set without a package",
        dx_adopt::lock_hubs::lock_of("hex")?
    );
    Ok(TEXT.get_or_init(|| text))
}

fn go_sum_missing() -> Result<&'static str, BackendError> {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    if let Some(text) = TEXT.get() {
        return Ok(text);
    }
    let text = format!(
        "a go set must list {} in locks",
        dx_adopt::lock_hubs::lock_of("go")?
    );
    Ok(TEXT.get_or_init(|| text))
}

fn go_whole_graph() -> Result<&'static str, BackendError> {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    if let Some(text) = TEXT.get() {
        return Ok(text);
    }
    let text = format!(
        "go sums cover the whole module graph; edit the require in {} and run `dx update <set>`",
        dx_adopt::lock_hubs::manifest_of("go")?
    );
    Ok(TEXT.get_or_init(|| text))
}

/// Plans `go mod download all` on a staged copy of the go manifest and sum file.
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
    let go_sum = dx_adopt::lock_hubs::lock_of("go")?;
    let sum_missing = go_sum_missing()?;
    let sum = set
        .locks
        .iter()
        .find(|lock| lock.rsplit('/').next() == Some(go_sum))
        .cloned()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: sum_missing,
        })?;
    let mut inputs = set.manifests.clone();
    inputs.push(sum.clone());
    Ok(StagedRun {
        tool: StagedTool::GoModDownload,
        argv_head: bazel(&pin_head, "run", &[lang_command(set, "go", "go")?, "--"]),
        env: Vec::new(),
        inputs,
        outputs: vec![sum],
    })
}

fn is_rebar(set: &ResolvedSet) -> Result<bool, BackendError> {
    Ok(dependency_sets::hex_backend(set)? == Some(dependency_sets::HexBackend::Rebar3))
}

/// Plans a mix deps command on a staged copy of the hex manifest and lock, with the pinned Elixir
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
    let mix_lock = dependency_sets::mix_lock()?;
    let lock_missing = hex_lock_missing()?;
    let lock = set
        .locks
        .iter()
        .find(|lock| lock.rsplit('/').next() == Some(mix_lock))
        .cloned()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: lock_missing,
        })?;
    let mut inputs = set.manifests.clone();
    inputs.push(lock.clone());
    let mut argv_head = bazel(
        &pin_head,
        "run",
        &[lang_command(set, "elixir", "mix")?, "--", command],
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

/// Plans a rebar3 command on a staged copy of rebar.config and rebar.lock, with the pinned
/// OTP and empty rebar3 and hex homes.
fn rebar_staged(
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    offline: bool,
    args: &[String],
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
        .find(|lock| lock.rsplit('/').next() == Some(dependency_sets::REBAR_LOCK))
        .cloned()
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: "a rebar set must list rebar.lock in locks",
        })?;
    let mut inputs = set.manifests.clone();
    inputs.push(lock.clone());
    let mut argv_head = bazel(
        &pin_head,
        "run",
        &[lang_command(set, "erlang", "rebar3")?, "--"],
    );
    argv_head.extend(args.iter().cloned());
    Ok(StagedRun {
        tool: StagedTool::Rebar3,
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
#[must_use]
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
    let under = format!("--run_under=cd {} &&", dx_process::shell::quote(&dir));
    let mut argv = bazel(
        head,
        "run",
        &[&under, lang_command(set, "perl", "carton")?, "--"],
    );
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
    let under = format!("--run_under=cd {} &&", dx_process::shell::quote(&dir));
    let mut argv = bazel(head, "run", &[&under, label, "--"]);
    argv.extend(strings(command));
    Ok(argv)
}

/// Keeps Composer's home and download cache out of the workspace.
fn composer_env(set: &ResolvedSet) -> Result<Vec<(String, String)>, BackendError> {
    Ok(vec![
        (
            "COMPOSER_HOME".to_owned(),
            tool_cache(set, "composer", "home")?,
        ),
        (
            "COMPOSER_CACHE_DIR".to_owned(),
            tool_cache(set, "composer", "cache")?,
        ),
    ])
}

/// A directory for one resolver's state under the user cache dir, so it stays out of the workspace
/// and out of the shared temp dir, where another user could create it first.
fn tool_cache(set: &ResolvedSet, tool: &'static str, name: &str) -> Result<String, BackendError> {
    let cache =
        dx_process::rc::user_cache_dir(&|var| std::env::var(var).ok()).ok_or_else(|| {
            BackendError::NoCacheDir {
                set: set.name.clone(),
                tool,
            }
        })?;
    let path = cache.join("dx").join("update").join(tool).join(name);
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| BackendError::NonUtf8Path {
            set: set.name.clone(),
            path: path.to_string_lossy().into_owned(),
        })
}

/// Runs the script in a temp copy of the set directory, then compares or copies back the state file `$state`.
const SWIFT_SCRIPT: &str = r#"dir=$1; apply=$2; shift 2; tmp=$(mktemp -d) || exit 1; cp -R "$dir"/. "$tmp"; (cd "$tmp" && "$@"); code=$?; if [ $code -eq 0 ] && ! cmp -s "$tmp/$state" "$dir/$state"; then if [ "$apply" = apply ]; then cp "$tmp/$state" "$dir/$state"; code=$?; else echo "$state is out of date; run dx update" >&2; code=1; fi; fi; rm -rf "$tmp"; exit $code"#;

/// The `SwiftPM` identity of a package named by host and path: the lowercase last segment.
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
        "--run_under=sh -c 'state={}; {SWIFT_SCRIPT}' sh {} {mode}",
        dx_adopt::lock_hubs::state_of("swiftpm")?,
        dx_process::shell::quote(&dir)
    );
    let mut argv = bazel(
        head,
        "run",
        &[&under, lang_command(set, "swift", "swift")?, "--"],
    );
    argv.extend(command.iter().cloned());
    Ok(argv)
}

/// Keeps carton's `local/` install tree out of the workspace.
fn carton_env(set: &ResolvedSet) -> Result<Vec<(String, String)>, BackendError> {
    Ok(vec![(
        "PERL_CARTON_PATH".to_owned(),
        tool_cache(set, "carton", "local")?,
    )])
}

/// Keeps the Julia package depot out of the workspace.
fn julia_env(set: &ResolvedSet) -> Result<Vec<(String, String)>, BackendError> {
    Ok(vec![(
        "JULIA_DEPOT_PATH".to_owned(),
        tool_cache(set, "julia", "depot")?,
    )])
}

/// An `opam` run from the pinned binary and opam-repository snapshot in the set's directory.
fn opam_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    command: &[&str],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let under = format!("--run_under=cd {} &&", dx_process::shell::quote(&dir));
    let mut argv = bazel(
        head,
        "run",
        &[&under, lang_command(set, "ocaml", "opam")?, "--"],
    );
    argv.extend(strings(command));
    Ok(argv)
}

/// Keeps the opam root, with the snapshot as its only repository, out of the workspace.
fn opam_env(set: &ResolvedSet) -> Result<Vec<(String, String)>, BackendError> {
    Ok(vec![(
        "OPAMROOT".to_owned(),
        tool_cache(set, "opam", "root")?,
    )])
}

/// A `pub` run of a Flutter set through `@rules_flutter//flutter:pub`, which points
/// `FLUTTER_ROOT` at a tree for the pinned Flutter version.
fn pub_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    command: &[&str],
) -> Result<Vec<String>, BackendError> {
    let dir = set_dir(workspace, set)?;
    let mut argv = bazel(head, "run", &["@rules_flutter//flutter:pub", "--"]);
    argv.extend(strings(command));
    argv.extend(["--directory".to_owned(), dir]);
    Ok(argv)
}

/// Keeps pub's package cache out of the workspace.
fn pub_env(set: &ResolvedSet) -> Result<Vec<(String, String)>, BackendError> {
    Ok(vec![(
        "PUB_CACHE".to_owned(),
        tool_cache(set, "pub", "cache")?,
    )])
}

fn bazel(head: &dx_process::BazelHead, verb: &str, args: &[&str]) -> Vec<String> {
    let mut argv = head.argv(verb);
    argv.extend(strings(args));
    argv
}

impl From<dx_adopt::AdoptError> for BackendError {
    fn from(error: dx_adopt::AdoptError) -> Self {
        Self::Hubs {
            detail: error.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
