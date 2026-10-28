use super::selector::SetRequest;
use super::sets::SetId;
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
    Noop,
}

/// A generator run on copies of `inputs` in a scratch directory, never in the workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedRun {
    /// The command up to, not including, the paths into the scratch directory.
    pub argv_head: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Workspace-relative files copied into the scratch directory by file name.
    pub inputs: Vec<String>,
    /// Workspace-relative locks the run may produce; compared by file name against `out/`.
    pub outputs: Vec<String>,
}

impl StagedRun {
    /// The full command for a scratch directory (an absolute path).
    pub fn argv(&self, scratch: &Path) -> Vec<String> {
        let mut argv = self.argv_head.clone();
        argv.push("--dependencies-file".to_owned());
        argv.push(
            scratch
                .join("paket.dependencies")
                .to_string_lossy()
                .into_owned(),
        );
        argv.push("--output-folder".to_owned());
        argv.push(scratch.join("out").to_string_lossy().into_owned());
        argv
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

pub const UV_VERSION_LINE: &str = "uv 0.12.23";

const UV_BIN_WHAT: &str = "the pinned uv 0.12.23 from @dx_tools//:uv";

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

/// Builds the yarn or npm argv for a configured npm set; `None` means pnpm.
fn configured_node_argv(
    workspace: &Path,
    set: &ResolvedSet,
    closure: &UvClosure,
    command: &[String],
) -> Result<Option<Vec<String>>, BackendError> {
    let dir = match set.dir.as_str() {
        "." => workspace.to_string_lossy().into_owned(),
        dir => workspace.join(dir).to_string_lossy().into_owned(),
    };
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

pub fn plan(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<BackendPlan, BackendError> {
    let planned: Result<BackendPlan, BackendError> = match (set, request) {
        (SetId::Cargo, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: bazel(head, "build", &["//rust/tests/fixtures/hello:hello"]),
            // tools/dx and tools/dx.cmd mirror this env when //:dx fails to build.
            env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Cargo, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "crate_universe repin refreshes the whole Cargo lock; use `dx update cargo` for the set",
        }),
        (SetId::Npm, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: pnpm_argv(head, workspace, &["update", "--lockfile-only"]),
            env: vec![],
        }),
        (SetId::Npm, SetRequest::Packages(packages)) => Ok(BackendPlan::Run {
            argv: [
                pnpm_argv(head, workspace, &["update"]),
                packages.clone(),
                strings(&["--lockfile-only"]),
            ]
            .concat(),
            env: vec![],
        }),
        (SetId::NpmTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: pnpm_argv(
                head,
                &workspace.join("quality/tools/javascript"),
                &["install", "--lockfile-only"],
            ),
            env: vec![],
        }),
        (SetId::NpmTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "npm-tools pins are exact in quality/tools/javascript; use `dx update npm-tools` for the set",
        }),
        (SetId::Uv, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "python/tests/fixtures/hello"]),
            env: vec![],
        }),
        (SetId::Uv, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv` for the set",
        }),
        (SetId::UvTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "quality/tools/python"]),
            env: vec![],
        }),
        (SetId::UvTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-tools` for the set",
        }),
        (SetId::NpmAdopt, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&[
                "pnpm",
                "--dir",
                "examples/adopt-js-ts",
                "install",
                "--lockfile-only",
            ]),
            env: vec![],
        }),
        (SetId::NpmAdopt, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "npm-adopt pins are exact in examples/adopt-js-ts; use `dx update npm-adopt` for the set",
        }),
        (SetId::NpmAdoptPolyglot, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&[
                "pnpm",
                "--dir",
                "examples/adopt-polyglot",
                "install",
                "--lockfile-only",
            ]),
            env: vec![],
        }),
        (SetId::NpmAdoptPolyglot, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "npm-adopt-polyglot pins are exact in examples/adopt-polyglot; use `dx update npm-adopt-polyglot` for the set",
        }),
        (SetId::UvAdopt, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-python"]),
            env: vec![],
        }),
        (SetId::UvAdopt, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-adopt` for the set",
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-polyglot"]),
            env: vec![],
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-adopt-polyglot` for the set",
        }),
        (SetId::Maven, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: bazel(head, "run", &["@maven//:pin"]),
            env: vec![("REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Maven, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "maven pins are exact in MODULE.bazel; use `dx update maven` for the set",
        }),
        (SetId::NuGet, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: bazel(
                head,
                "run",
                &[
                    "@rules_dotnet//tools/paket2bazel",
                    "--",
                    "--dependencies-file",
                    "third_party/dotnet/paket.dependencies",
                    "--output-folder",
                    "third_party/dotnet/deps",
                ],
            ),
            env: vec![],
        }),
        (SetId::NuGet, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "nuget pins are exact in paket.dependencies; use `dx update nuget` for the set",
        }),
        (SetId::Go, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Go, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "go pins track Gazelle for the shared go_deps extension; widen explicitly via `dx bump go:<module> <version>`",
        }),
        (SetId::Ruby, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Ruby, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "ruby pins are exact in third_party/ruby/Gemfile; widen the requirement there and regenerate the lock with `bundle lock`",
        }),
        (SetId::PowerShell, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::PowerShell, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:                 "powershell pins are exact in third_party/powershell/PSGallery.requirements.psd1; widen the requirement there and hand-regenerate PSGallery.lock.json, because consumer builds never run Install-Module",
        }),
    };
    if matches!(planned, Ok(BackendPlan::Run { .. })) {
        if offline {
            return Err(BackendError::OfflineRequired {
                set: set.name().to_owned(),
            });
        }
        if frozen {
            return Err(BackendError::FrozenLocked {
                set: set.name().to_owned(),
            });
        }
    }
    planned
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
}

pub fn check(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
    _frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<CheckPlan, BackendError> {
    if matches!((set, request), (SetId::Uv, SetRequest::Full)) {
        let mut argv = strings(&["uv", "lock", "--check", "--directory"]);
        argv.push("python/tests/fixtures/hello".to_owned());
        if offline {
            argv.push("--offline".to_owned());
        }
        return Ok(CheckPlan::Run {
            argv,
            env: Vec::new(),
        });
    }
    if matches!(
        (set, request),
        (
            SetId::Go | SetId::Ruby | SetId::PowerShell,
            SetRequest::Full
        )
    ) {
        return Ok(CheckPlan::Pinned);
    }
    match plan(workspace, set, request, offline, false, head) {
        Ok(_) => Ok(CheckPlan::Unavailable),
        Err(error) => Err(error),
    }
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

/// Refuses sets whose backend would read host env dx cannot override (the runner only adds env).
/// `lookup` tells whether a variable is set in the host environment.
pub fn host_env_refusal(
    set: &ResolvedSet,
    check: bool,
    lookup: &dyn Fn(&str) -> bool,
) -> Option<BackendError> {
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
        "cargo sets must name their crate_universe repository in dx.toml",
    )
}

fn maven_repository(set: &ResolvedSet) -> Result<&str, BackendError> {
    set_repository(
        set,
        "maven sets must name their rules_jvm_external repository in dx.toml",
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
) -> Vec<String> {
    let mut run = Vec::new();
    if offline {
        run.push("--repository_disable_download");
    }
    run.extend(["@pnpm//:pnpm", "--", "--dir"]);
    [
        bazel(head, "run", &run),
        vec![match set.dir.as_str() {
            "." => workspace.to_string_lossy().into_owned(),
            dir => workspace.join(dir).to_string_lossy().into_owned(),
        }],
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
            _ => {
                "dx does not resolve PowerShell modules; edit PSGallery.requirements.psd1 and regenerate PSGallery.lock.json by hand"
            }
        },
    }
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
            reason: "bazel module pins change with `dx bump <set>:<module> <version>`; refresh the lock with `bazel mod deps`",
        }),
        (Ecosystem::Ruby | Ecosystem::PowerShell, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::Ruby | Ecosystem::PowerShell, Selection::Packages(_)) => {
            Err(pinned_selective(set))
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
                None => configured_pnpm_argv(workspace, head, set, false, &command),
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
            Ok(BackendPlan::Run {
                argv: configured_fetch_argv(head, repository, false),
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
                argv: configured_fetch_argv(head, repository, false),
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
            reason: "bazel module pins change with `dx bump <set>:<module> <version>`; refresh the lock with `bazel mod deps`",
        }),
        (Ecosystem::Ruby | Ecosystem::PowerShell, Selection::Full) => Ok(CheckPlan::Pinned),
        (Ecosystem::Ruby | Ecosystem::PowerShell, Selection::Packages(_)) => {
            Err(pinned_selective(set))
        }
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
                None => configured_pnpm_argv(workspace, head, set, offline, &command),
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
            Ok(CheckPlan::Run {
                argv: configured_fetch_argv(head, repository, offline),
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
        argv_head: bazel(
            &pin_head,
            "run",
            &["@rules_dotnet//tools/paket2bazel", "--"],
        ),
        env: Vec::new(),
        inputs,
        outputs,
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

fn bazel(head: &dx_process::BazelHead, verb: &str, args: &[&str]) -> Vec<String> {
    let mut argv = head.argv(verb);
    argv.extend(strings(args));
    argv
}

fn pnpm_argv(head: &dx_process::BazelHead, dir: &Path, command: &[&str]) -> Vec<String> {
    [
        bazel(head, "run", &["@pnpm//:pnpm", "--", "--dir"]),
        strings(&[&dir.to_string_lossy()]),
        strings(command),
    ]
    .concat()
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

    #[allow(clippy::disallowed_methods)] // test helper mirrors the backend argv
    fn bazel_head() -> Vec<String> {
        dx_process::startup_argv(&[])
    }

    fn bazel_argv(verb: &str, rest: &[String]) -> Vec<String> {
        [
            bazel_head(),
            vec![verb.to_owned()],
            dx_process::build_verb_options(verb, &[]),
            rest.to_vec(),
        ]
        .concat()
    }

    #[test]
    fn runs_bazel_marks_bazel_updaters() {
        let planned =
            |set: SetId| match plan(ws(), set, &SetRequest::Full, false, false, &no_head()) {
                Ok(BackendPlan::Run { argv, .. }) => argv,
                other => panic!("{}: {other:?}", set.name()),
            };
        for set in [
            SetId::Cargo,
            SetId::Npm,
            SetId::NpmTools,
            SetId::Maven,
            SetId::NuGet,
        ] {
            assert!(runs_bazel(&planned(set)), "{}", set.name());
        }
        for set in [
            SetId::Uv,
            SetId::UvTools,
            SetId::UvAdopt,
            SetId::UvAdoptPolyglot,
            SetId::NpmAdopt,
            SetId::NpmAdoptPolyglot,
        ] {
            assert!(!runs_bazel(&planned(set)), "{}", set.name());
        }
        match check(ws(), SetId::Uv, &SetRequest::Full, false, false, &no_head()) {
            Ok(CheckPlan::Run { argv, .. }) => assert!(!runs_bazel(&argv)),
            other => panic!("uv check: {other:?}"),
        }
        assert!(!runs_bazel(&[]));
    }

    #[test]
    fn full_plans_are_pinned_and_hermetic() {
        let cargo = plan(
            ws(),
            SetId::Cargo,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("cargo");
        assert_eq!(
            cargo,
            BackendPlan::Run {
                argv: bazel_argv("build", &strings(&["//rust/tests/fixtures/hello:hello"])),
                env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
            }
        );
        let npm = plan(
            ws(),
            SetId::Npm,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("npm");
        assert_eq!(
            npm,
            BackendPlan::Run {
                argv: bazel_argv(
                    "run",
                    &[
                        "@pnpm//:pnpm".to_owned(),
                        "--".to_owned(),
                        "--dir".to_owned(),
                        ws_arg(),
                        "update".to_owned(),
                        "--lockfile-only".to_owned(),
                    ],
                ),
                env: vec![],
            }
        );
        let maven = plan(
            ws(),
            SetId::Maven,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("maven");
        assert_eq!(
            maven,
            BackendPlan::Run {
                argv: bazel_argv("run", &strings(&["@maven//:pin"])),
                env: vec![("REPIN".to_owned(), "1".to_owned())],
            }
        );
        let nuget = plan(
            ws(),
            SetId::NuGet,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("nuget");
        match nuget {
            BackendPlan::Run { argv, env } => {
                assert!(argv.starts_with(&bazel_head()));
                assert!(argv.contains(&"@rules_dotnet//tools/paket2bazel".to_owned()));
                assert!(argv.contains(&"third_party/dotnet/paket.dependencies".to_owned()));
                assert!(argv.contains(&"third_party/dotnet/deps".to_owned()));
                assert!(env.is_empty());
            }
            BackendPlan::Noop | BackendPlan::Staged(_) => panic!("nuget runs paket2bazel"),
        }
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false, false, &no_head()).expect("go"),
            BackendPlan::Noop
        );
        let npm_tools = plan(
            ws(),
            SetId::NpmTools,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("npm-tools");
        match npm_tools {
            BackendPlan::Run { argv, env } => {
                assert_eq!(
                    argv,
                    bazel_argv(
                        "run",
                        &[
                            "@pnpm//:pnpm".to_owned(),
                            "--".to_owned(),
                            "--dir".to_owned(),
                            dir_arg("quality/tools/javascript"),
                            "install".to_owned(),
                            "--lockfile-only".to_owned(),
                        ],
                    )
                );
                assert!(env.is_empty());
            }
            BackendPlan::Noop | BackendPlan::Staged(_) => panic!("npm-tools runs pnpm"),
        }
        let uv = plan(ws(), SetId::Uv, &SetRequest::Full, false, false, &no_head()).expect("uv");
        assert_eq!(
            uv,
            BackendPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--directory".to_owned(),
                    "python/tests/fixtures/hello".to_owned(),
                ],
                env: vec![],
            }
        );
        let uv_tools = plan(
            ws(),
            SetId::UvTools,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("uv-tools");
        assert_eq!(
            uv_tools,
            BackendPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--directory".to_owned(),
                    "quality/tools/python".to_owned(),
                ],
                env: vec![],
            }
        );
        let npm_adopt = plan(
            ws(),
            SetId::NpmAdopt,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("npm-adopt");
        assert_eq!(
            npm_adopt,
            BackendPlan::Run {
                argv: vec![
                    "pnpm".to_owned(),
                    "--dir".to_owned(),
                    "examples/adopt-js-ts".to_owned(),
                    "install".to_owned(),
                    "--lockfile-only".to_owned(),
                ],
                env: vec![],
            }
        );
        let uv_adopt = plan(
            ws(),
            SetId::UvAdopt,
            &SetRequest::Full,
            false,
            false,
            &no_head(),
        )
        .expect("uv-adopt");
        assert_eq!(
            uv_adopt,
            BackendPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--directory".to_owned(),
                    "examples/adopt-python".to_owned(),
                ],
                env: vec![],
            }
        );
    }

    #[test]
    fn npm_selective_delegates_packages_to_pnpm() {
        let selective = plan(
            ws(),
            SetId::Npm,
            &SetRequest::Packages(vec!["jest".to_owned(), "react".to_owned()]),
            false,
            false,
            &no_head(),
        )
        .expect("npm selective");
        assert_eq!(
            selective,
            BackendPlan::Run {
                argv: bazel_argv(
                    "run",
                    &[
                        "@pnpm//:pnpm".to_owned(),
                        "--".to_owned(),
                        "--dir".to_owned(),
                        ws_arg(),
                        "update".to_owned(),
                        "jest".to_owned(),
                        "react".to_owned(),
                        "--lockfile-only".to_owned(),
                    ],
                ),
                env: vec![],
            }
        );
    }

    #[test]
    fn pnpm_plans_name_the_workspace_bazels_run_cwd_loses() {
        for (set, request, expected) in [
            (SetId::Npm, SetRequest::Full, ws_arg()),
            (
                SetId::Npm,
                SetRequest::Packages(vec!["devalue".to_owned()]),
                ws_arg(),
            ),
            (
                SetId::NpmTools,
                SetRequest::Full,
                dir_arg("quality/tools/javascript"),
            ),
        ] {
            let argv = match plan(ws(), set, &request, false, false, &no_head()).expect(set.name())
            {
                BackendPlan::Run { argv, .. } => argv,
                BackendPlan::Noop | BackendPlan::Staged(_) => panic!("{} runs pnpm", set.name()),
            };
            let at = argv
                .iter()
                .position(|arg| arg == "--dir")
                .unwrap_or_else(|| panic!("{} names --dir", set.name()));
            assert_eq!(argv[at + 1], expected, "{set:?}");
            assert!(
                argv.contains(&"--lockfile-only".to_owned()),
                "{set:?} must not populate node_modules"
            );
        }
    }

    #[test]
    fn go_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false, false, &no_head()).expect("go full"),
            BackendPlan::Noop
        );
    }

    #[test]
    fn ruby_full_is_pinned_noop_success() {
        assert_eq!(
            plan(
                ws(),
                SetId::Ruby,
                &SetRequest::Full,
                false,
                false,
                &no_head()
            )
            .expect("ruby full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::Ruby,
            &SetRequest::Packages(vec!["rspec-core".to_owned()]),
            false,
            false,
            &no_head(),
        )
        .expect_err("ruby selective is seed-host only");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("bundle lock"), "{error}");
    }

    #[test]
    fn powershell_full_is_pinned_noop_success() {
        assert_eq!(
            plan(
                ws(),
                SetId::PowerShell,
                &SetRequest::Full,
                false,
                false,
                &no_head()
            )
            .expect("powershell full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::PowerShell,
            &SetRequest::Packages(vec!["Pester".to_owned()]),
            false,
            false,
            &no_head(),
        )
        .expect_err("powershell selective is seed-host only");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("Install-Module"), "{error}");
    }

    #[test]
    fn non_npm_selective_reports_unsupported_never_full() {
        let rows: [(SetId, &str, &str); 14] = [
            (SetId::Cargo, "anyhow", "use `dx update cargo` for the set"),
            (
                SetId::Maven,
                "junit:junit",
                "use `dx update maven` for the set",
            ),
            (
                SetId::Maven,
                "org.junit.jupiter:junit-jupiter-api",
                "use `dx update maven` for the set",
            ),
            (
                SetId::NuGet,
                "FSharp.Core",
                "use `dx update nuget` for the set",
            ),
            (
                SetId::Go,
                "github.com/google/go-cmp/cmp",
                "dx bump go:<module> <version>",
            ),
            (SetId::Ruby, "rspec-core", "bundle lock"),
            (SetId::PowerShell, "Pester", "Install-Module"),
            (
                SetId::NpmTools,
                "eslint",
                "use `dx update npm-tools` for the set",
            ),
            (SetId::Uv, "pytest", "use `dx update uv` for the set"),
            (
                SetId::UvTools,
                "pylint",
                "use `dx update uv-tools` for the set",
            ),
            (
                SetId::NpmAdopt,
                "jest",
                "use `dx update npm-adopt` for the set",
            ),
            (
                SetId::NpmAdoptPolyglot,
                "jest",
                "use `dx update npm-adopt-polyglot` for the set",
            ),
            (
                SetId::UvAdopt,
                "pytest",
                "use `dx update uv-adopt` for the set",
            ),
            (
                SetId::UvAdoptPolyglot,
                "pytest",
                "use `dx update uv-adopt-polyglot` for the set",
            ),
        ];
        for (set, package, hint) in rows {
            let request = SetRequest::Packages(vec![package.to_owned()]);
            let error =
                plan(ws(), set, &request, false, false, &no_head()).expect_err("unsupported");
            assert!(matches!(error, BackendError::Unsupported { .. }), "{set:?}");
            let message = error.to_string();
            assert!(message.contains(set.name()), "{set:?}: {message}");
            assert!(message.contains(hint), "{set:?}: {message}");
        }
    }

    #[test]
    fn plan_offline_and_frozen_fail_exactly_when_a_backend_would_run() {
        let head = no_head();
        for set in SetId::ALL {
            for request in [SetRequest::Full, SetRequest::Packages(vec!["x".to_owned()])] {
                let plain = plan(ws(), set, &request, false, false, &head);
                let offline = plan(ws(), set, &request, true, false, &head);
                let frozen = plan(ws(), set, &request, false, true, &head);
                let both = plan(ws(), set, &request, true, true, &head);
                if matches!(plain, Ok(BackendPlan::Run { .. })) {
                    let name = set.name().to_owned();
                    assert_eq!(
                        offline,
                        Err(BackendError::OfflineRequired { set: name.clone() })
                    );
                    assert_eq!(
                        frozen,
                        Err(BackendError::FrozenLocked { set: name.clone() })
                    );
                    assert_eq!(both, Err(BackendError::OfflineRequired { set: name }));
                } else {
                    assert_eq!(offline, plain, "{set:?} {request:?}");
                    assert_eq!(frozen, plain, "{set:?} {request:?}");
                    assert_eq!(both, plain, "{set:?} {request:?}");
                }
            }
        }
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

    #[test]
    fn argv_never_names_a_dx_lockfile() {
        for set in SetId::ALL {
            if let Ok(BackendPlan::Run { argv, .. }) =
                plan(ws(), set, &SetRequest::Full, false, false, &no_head())
            {
                for arg in argv {
                    assert!(
                        !arg.contains("dx.lock"),
                        "{set:?} argv must not name a dx lockfile: {arg}"
                    );
                }
            }
        }
    }

    #[test]
    fn uv_full_check_is_a_read_only_lock_check() {
        assert_eq!(
            check(ws(), SetId::Uv, &SetRequest::Full, false, false, &no_head()).expect("uv check"),
            CheckPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--check".to_owned(),
                    "--directory".to_owned(),
                    "python/tests/fixtures/hello".to_owned(),
                ],
                env: vec![],
            }
        );
        assert_eq!(
            check(ws(), SetId::Uv, &SetRequest::Full, true, false, &no_head())
                .expect("uv check offline"),
            CheckPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--check".to_owned(),
                    "--directory".to_owned(),
                    "python/tests/fixtures/hello".to_owned(),
                    "--offline".to_owned(),
                ],
                env: vec![],
            }
        );
    }

    fn configured_set(dir: &str) -> ResolvedSet {
        ResolvedSet {
            name: "frontend".to_owned(),
            ecosystem: Ecosystem::Uv,
            dir: dir.to_owned(),
            manifests: vec![format!("{dir}/pyproject.toml")],
            locks: vec![format!("{dir}/uv.lock")],
            writable: true,
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
    fn configured_bazel_selective_points_at_bump() {
        let set = ResolvedSet {
            name: "bazel-modules".to_owned(),
            ecosystem: Ecosystem::Bazel,
            dir: "tools".to_owned(),
            manifests: vec!["tools/MODULE.bazel".to_owned()],
            locks: vec!["tools/MODULE.bazel.lock".to_owned()],
            writable: true,
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
                            && reason.contains("dx bump <set>:<module> <version>")
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
                    writable: true,
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
                    writable: true,
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
    fn every_bazel_plan_starts_with_launcher_and_startup_opts() {
        let startup = vec!["--output_base=/ob".to_owned()];
        let head = dx_process::BazelHead {
            startup: startup.clone(),
            options: vec![],
            settings: vec![],
        };
        let expected_head = [
            vec!["bazel".to_owned()],
            strings(dx_process::WORKFLOW_STARTUP_OPTS),
            startup.clone(),
        ]
        .concat();
        let bazel_targets = [
            "@pnpm//:pnpm",
            "@maven//:pin",
            "@rules_dotnet//tools/paket2bazel",
            "//rust/tests/fixtures/hello:hello",
        ];
        let requests = SetId::ALL
            .iter()
            .map(|set| (*set, SetRequest::Full))
            .chain([(SetId::Npm, SetRequest::Packages(vec!["jest".to_owned()]))]);
        let mut bazel_plans = 0;
        for (set, request) in requests {
            let Ok(BackendPlan::Run { argv, .. }) = plan(ws(), set, &request, false, false, &head)
            else {
                continue;
            };
            if argv.iter().any(|arg| bazel_targets.contains(&arg.as_str())) {
                bazel_plans += 1;
                assert!(argv.starts_with(&expected_head), "{set:?}: {argv:?}");
            } else {
                assert!(
                    matches!(argv[0].as_str(), "uv" | "pnpm"),
                    "{set:?}: {argv:?}"
                );
                assert!(
                    !argv.contains(&"--nohome_rc".to_owned()),
                    "{set:?}: {argv:?}"
                );
                assert!(!argv.contains(&startup[0]), "{set:?}: {argv:?}");
            }
        }
        assert_eq!(bazel_plans, 6);
        assert_eq!(
            check(ws(), SetId::Cargo, &SetRequest::Full, false, false, &head).expect("cargo check"),
            CheckPlan::Unavailable
        );
    }

    #[test]
    fn pinned_sets_check_without_launching() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, false, &no_head())
                    .expect("pinned check"),
                CheckPlan::Pinned,
                "{set:?}"
            );
            assert_eq!(
                check(ws(), set, &SetRequest::Full, true, false, &no_head())
                    .expect("pinned check offline"),
                CheckPlan::Pinned,
                "{set:?}"
            );
        }
    }

    #[test]
    fn check_propagates_unsupported_and_offline_errors() {
        let error = check(
            ws(),
            SetId::Uv,
            &SetRequest::Packages(vec!["pytest".to_owned()]),
            false,
            false,
            &no_head(),
        )
        .expect_err("uv selective stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        let error = check(
            ws(),
            SetId::Go,
            &SetRequest::Packages(vec!["example.com/mod".to_owned()]),
            false,
            false,
            &no_head(),
        )
        .expect_err("go selective stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        let error = check(
            ws(),
            SetId::Cargo,
            &SetRequest::Full,
            true,
            false,
            &no_head(),
        )
        .expect_err("cargo check offline needs network");
        assert!(
            matches!(error, BackendError::OfflineRequired { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn check_is_unavailable_for_every_other_runnable_set() {
        for set in [
            SetId::Cargo,
            SetId::Npm,
            SetId::Maven,
            SetId::NuGet,
            SetId::NpmTools,
            SetId::UvTools,
            SetId::NpmAdopt,
            SetId::NpmAdoptPolyglot,
            SetId::UvAdopt,
            SetId::UvAdoptPolyglot,
        ] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, false, &no_head()).expect("check plans"),
                CheckPlan::Unavailable,
                "{set:?}"
            );
        }
        assert_eq!(
            check(
                ws(),
                SetId::Npm,
                &SetRequest::Packages(vec!["jest".to_owned()]),
                false,
                false,
                &no_head(),
            )
            .expect("npm selective check plans"),
            CheckPlan::Unavailable,
        );
    }

    #[test]
    fn offline_forces_cache_only_except_pinned_noops() {
        for set in [
            SetId::Cargo,
            SetId::Npm,
            SetId::Maven,
            SetId::NuGet,
            SetId::NpmTools,
            SetId::Uv,
            SetId::UvTools,
            SetId::NpmAdopt,
            SetId::NpmAdoptPolyglot,
            SetId::UvAdopt,
            SetId::UvAdoptPolyglot,
        ] {
            let error = plan(ws(), set, &SetRequest::Full, true, false, &no_head())
                .expect_err("offline needs network");
            assert!(
                matches!(error, BackendError::OfflineRequired { .. }),
                "{set:?}: {error:?}"
            );
            assert!(error.to_string().contains("offline_required"), "{error}");
            assert!(error.to_string().contains(set.name()), "{error}");
        }
        let selective = plan(
            ws(),
            SetId::Npm,
            &SetRequest::Packages(vec!["jest".to_owned()]),
            true,
            false,
            &no_head(),
        )
        .expect_err("npm selective offline needs network");
        assert!(
            matches!(selective, BackendError::OfflineRequired { .. }),
            "{selective:?}"
        );
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                plan(ws(), set, &SetRequest::Full, true, false, &no_head())
                    .expect("pinned noop offline"),
                BackendPlan::Noop,
                "{set:?}"
            );
        }
        for (set, packages) in [
            (
                SetId::Cargo,
                SetRequest::Packages(vec!["anyhow".to_owned()]),
            ),
            (
                SetId::Maven,
                SetRequest::Packages(vec!["junit:junit".to_owned()]),
            ),
            (
                SetId::NuGet,
                SetRequest::Packages(vec!["FSharp.Core".to_owned()]),
            ),
            (
                SetId::Go,
                SetRequest::Packages(vec!["example.com/mod".to_owned()]),
            ),
            (
                SetId::Ruby,
                SetRequest::Packages(vec!["rspec-core".to_owned()]),
            ),
            (
                SetId::PowerShell,
                SetRequest::Packages(vec!["Pester".to_owned()]),
            ),
            (
                SetId::NpmTools,
                SetRequest::Packages(vec!["eslint".to_owned()]),
            ),
            (SetId::Uv, SetRequest::Packages(vec!["pytest".to_owned()])),
            (
                SetId::UvTools,
                SetRequest::Packages(vec!["pylint".to_owned()]),
            ),
            (
                SetId::NpmAdopt,
                SetRequest::Packages(vec!["jest".to_owned()]),
            ),
            (
                SetId::NpmAdoptPolyglot,
                SetRequest::Packages(vec!["jest".to_owned()]),
            ),
            (
                SetId::UvAdopt,
                SetRequest::Packages(vec!["pytest".to_owned()]),
            ),
            (
                SetId::UvAdoptPolyglot,
                SetRequest::Packages(vec!["pytest".to_owned()]),
            ),
        ] {
            let error = plan(ws(), set, &packages, true, false, &no_head())
                .expect_err("unsupported stays unsupported");
            assert!(
                matches!(error, BackendError::Unsupported { .. }),
                "{set:?}: {error:?}"
            );
        }
    }

    #[test]
    fn frozen_forbids_resolution_changes_but_keeps_pinned_noops() {
        for set in [
            SetId::Cargo,
            SetId::Npm,
            SetId::Maven,
            SetId::NuGet,
            SetId::NpmTools,
            SetId::Uv,
            SetId::UvTools,
            SetId::NpmAdopt,
            SetId::NpmAdoptPolyglot,
            SetId::UvAdopt,
            SetId::UvAdoptPolyglot,
        ] {
            let error = plan(ws(), set, &SetRequest::Full, false, true, &no_head())
                .expect_err("frozen keeps pins");
            assert!(
                matches!(error, BackendError::FrozenLocked { .. }),
                "{set:?}: {error:?}"
            );
            assert!(error.to_string().contains("frozen_locked"), "{error}");
            assert!(error.to_string().contains(set.name()), "{error}");
        }
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                plan(ws(), set, &SetRequest::Full, false, true, &no_head())
                    .expect("pinned noop frozen"),
                BackendPlan::Noop,
                "{set:?}"
            );
        }
        let both = plan(
            ws(),
            SetId::Cargo,
            &SetRequest::Full,
            true,
            true,
            &no_head(),
        )
        .expect_err("offline wins");
        assert!(
            matches!(both, BackendError::OfflineRequired { .. }),
            "{both:?}"
        );
    }

    #[test]
    fn frozen_check_passes_without_resolution_changes() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, true, &no_head())
                    .expect("pinned check frozen"),
                CheckPlan::Pinned,
                "{set:?}"
            );
        }
        let uv = check(ws(), SetId::Uv, &SetRequest::Full, false, true, &no_head())
            .expect("uv check frozen");
        assert!(
            matches!(uv, CheckPlan::Run { .. }),
            "uv lock --check verifies without resolving: {uv:?}"
        );
        assert_eq!(
            check(
                ws(),
                SetId::Cargo,
                &SetRequest::Full,
                false,
                true,
                &no_head()
            )
            .expect("cargo check frozen"),
            CheckPlan::Unavailable
        );
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
            writable: true,
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
    fn configured_npm_yarn_and_npm_locks_use_their_own_backends() {
        let head = dx_process::BazelHead {
            startup: vec![],
            ..Default::default()
        };
        let mut yarn = npm_set("frontend", "apps/web");
        yarn.locks = vec!["apps/web/yarn.lock".to_owned()];
        let mut npm = yarn.clone();
        npm.locks = vec!["apps/web/package-lock.json".to_owned()];
        let tools = UvClosure {
            yarn: Some(abs("yarn")),
            npm: Some(abs("npm")),
            ..UvClosure::default()
        };
        let ws = Path::new("/ws");
        let plan = |set: &ResolvedSet, closure: &UvClosure| {
            plan_configured(ws, set, &Selection::Full, false, false, &head, closure)
        };
        match plan(&yarn, &tools) {
            Ok(BackendPlan::Run { argv, .. }) => {
                assert_eq!(argv[0], "sh");
                assert!(argv.contains(&abs("yarn")), "{argv:?}");
                assert!(
                    argv.contains(&"--mode=update-lockfile".to_owned()),
                    "{argv:?}"
                );
            }
            other => panic!("{other:?}"),
        }
        match plan(&npm, &tools) {
            Ok(BackendPlan::Run { argv, .. }) => {
                assert_eq!(argv[0], abs("npm"));
                assert!(argv.contains(&"--package-lock-only".to_owned()), "{argv:?}");
            }
            other => panic!("{other:?}"),
        }
        for (set, var) in [(&yarn, YARN_BIN_VAR), (&npm, NPM_BIN_VAR)] {
            match plan(set, &UvClosure::default()) {
                Err(BackendError::BackendUnavailable { var: got, .. }) => assert_eq!(got, var),
                other => panic!("{other:?}"),
            }
        }
        match check_configured(ws, &npm, &Selection::Full, false, false, &head, &tools) {
            Ok(CheckPlan::Run { argv, .. }) => {
                assert!(argv.contains(&"ci".to_owned()), "{argv:?}");
            }
            other => panic!("{other:?}"),
        }
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

    #[test]
    fn bazel_backends_put_head_options_after_verb_options() {
        let head = dx_process::BazelHead {
            startup: vec!["--output_base=/ob".to_owned()],
            options: vec!["--config=dx_dev".to_owned()],
            settings: vec![],
        };
        let config = "--config=dx_dev".to_owned();
        let packages = SetRequest::Packages(vec!["jest".to_owned()]);
        let cases = [
            (SetId::Cargo, SetRequest::Full),
            (SetId::Npm, SetRequest::Full),
            (SetId::Npm, packages),
            (SetId::NpmTools, SetRequest::Full),
            (SetId::Maven, SetRequest::Full),
            (SetId::NuGet, SetRequest::Full),
        ];
        for (set, request) in cases {
            let Ok(BackendPlan::Run { argv, .. }) = plan(ws(), set, &request, false, false, &head)
            else {
                panic!("{set:?} should plan");
            };
            let verb = argv
                .iter()
                .position(|arg| arg == "build" || arg == "run")
                .expect("verb");
            let at = argv.iter().position(|arg| *arg == config).expect("config");
            let opts = dx_process::build_verb_options(&argv[verb], &head.options).len();
            assert_eq!(at, verb + 1 + opts, "{set:?}: {argv:?}");
            assert!(
                argv.iter().position(|arg| arg == "--output_base=/ob") < Some(verb),
                "{set:?}: {argv:?}"
            );
        }
        for set in [SetId::Uv, SetId::Go] {
            if let Ok(BackendPlan::Run { argv, .. }) =
                plan(ws(), set, &SetRequest::Full, false, false, &head)
            {
                assert!(!argv.contains(&config), "{set:?}: {argv:?}");
            }
        }
        let offline = plan_configured(
            ws(),
            &npm_set("frontend", "apps/web"),
            &Selection::Full,
            false,
            false,
            &head,
            &UvClosure::default(),
        )
        .expect("configured npm");
        let BackendPlan::Run { argv, .. } = offline else {
            panic!("configured npm runs pnpm");
        };
        assert_eq!(
            argv,
            npm_run(&head, &[], "apps/web", &["install", "--lockfile-only"])
        );
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
            writable: true,
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
                "third_party/dotnet/deps/paket.main_extension.bzl".to_owned(),
            ],
            writable: true,
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
            argv_head: [
                dx_process::BazelHead {
                    settings: Vec::new(),
                    ..head.clone()
                }
                .argv("run"),
                strings(&["@rules_dotnet//tools/paket2bazel", "--"]),
            ]
            .concat(),
            env: Vec::new(),
            inputs: vec![
                "third_party/dotnet/paket.dependencies".to_owned(),
                "third_party/dotnet/paket.lock".to_owned(),
            ],
            outputs: vec![
                "third_party/dotnet/deps/paket.main.bzl".to_owned(),
                "third_party/dotnet/deps/paket.main_extension.bzl".to_owned(),
            ],
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
            .any(|arg| arg.contains("rules_dx")));
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

    fn maven_set(name: &str, repository: &str) -> ResolvedSet {
        ResolvedSet {
            name: name.to_owned(),
            ecosystem: Ecosystem::Maven,
            dir: ".".to_owned(),
            manifests: vec!["MODULE.bazel".to_owned()],
            locks: vec![format!("{repository}_install.json")],
            writable: true,
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

    #[test]
    fn configured_cargo_package_update_refuses_modes_and_many_crates() {
        let set = cargo_set("crates", "apps/rs", "crates");
        let plan = |names: &[&str]| {
            plan_configured(
                ws(),
                &set,
                &Selection::Packages(names.iter().map(|n| (*n).to_owned()).collect()),
                false,
                false,
                &no_head(),
                &UvClosure::default(),
            )
        };
        for name in ["all", "Workspace", "OFF", "0", "1", "minimal"] {
            assert_eq!(
                plan(&[name]),
                Err(BackendError::Unsupported {
                    set: "crates".to_owned(),
                    reason: CARGO_RESERVED,
                }),
                "{name}"
            );
        }
        assert_eq!(
            plan(&["serde", "tokio"]),
            Err(BackendError::Unsupported {
                set: "crates".to_owned(),
                reason: CARGO_ONE_PACKAGE,
            })
        );
    }
}
