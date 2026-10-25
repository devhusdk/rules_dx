use super::selector::SetRequest;
use super::sets::SetId;
use dx_adopt::dependency_sets::{Ecosystem, ResolvedSet, Selection};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendPlan {
    Run {
        argv: Vec<String>,
        env: Vec<(String, String)>,
    },
    Noop,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BackendError {
    #[error("unsupported selective update for {set}: {reason}")]
    Unsupported {
        set: &'static str,
        reason: &'static str,
    },
    #[error("unsupported selective update for {set}: {reason}")]
    UnsupportedOwned { set: String, reason: &'static str },
    #[error("offline_required: cannot update {set} without network (re-run without --offline once connected)")]
    OfflineRequired { set: &'static str },
    #[error("offline_required: cannot update {set} without network (re-run without --offline once connected)")]
    OfflineRequiredOwned { set: String },
    #[error("frozen_locked: cannot change {set} resolution while frozen (re-run without --frozen to allow resolver changes)")]
    FrozenLocked { set: &'static str },
    #[error("frozen_locked: cannot change {set} resolution while frozen (re-run without --frozen to allow resolver changes)")]
    FrozenLockedOwned { set: String },
    #[error("backend_unavailable: cannot run {tool} for {set}: set {var} to the absolute path of {what}; PATH is never searched")]
    BackendUnavailable {
        set: String,
        tool: &'static str,
        var: &'static str,
        what: &'static str,
    },
}

pub const UV_BIN_VAR: &str = "DX_UV_BIN";

pub const UV_PYTHON_VAR: &str = "DX_UV_PYTHON";

pub const UV_VERSION_LINE: &str = "uv 0.12.23";

const UV_BIN_WHAT: &str = "the pinned uv 0.12.23 from @dx_tools//:uv";

const UV_PYTHON_WHAT: &str = "a pinned Python interpreter";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UvClosure {
    pub uv: Option<String>,
    pub python: Option<String>,
}

pub fn plan(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
    frozen: bool,
    startup: &[String],
) -> Result<BackendPlan, BackendError> {
    if offline {
        match (set, request) {
            (SetId::Go, SetRequest::Full) => return Ok(BackendPlan::Noop),
            (SetId::Cargo, SetRequest::Packages(_))
            | (SetId::Maven, SetRequest::Packages(_))
            | (SetId::NuGet, SetRequest::Packages(_))
            | (SetId::Go, SetRequest::Packages(_))
            | (SetId::NpmTools, SetRequest::Packages(_))
            | (SetId::Uv, SetRequest::Packages(_))
            | (SetId::UvTools, SetRequest::Packages(_))
            | (SetId::NpmAdopt, SetRequest::Packages(_))
            | (SetId::NpmAdoptPolyglot, SetRequest::Packages(_))
            | (SetId::UvAdopt, SetRequest::Packages(_))
            | (SetId::UvAdoptPolyglot, SetRequest::Packages(_)) => {}
            _ => {
                let would_run = matches!(
                    (set, request),
                    (SetId::Cargo, SetRequest::Full)
                        | (SetId::Npm, _)
                        | (SetId::Maven, SetRequest::Full)
                        | (SetId::NuGet, SetRequest::Full)
                        | (SetId::NpmTools, SetRequest::Full)
                        | (SetId::Uv, SetRequest::Full)
                        | (SetId::UvTools, SetRequest::Full)
                        | (SetId::NpmAdopt, SetRequest::Full)
                        | (SetId::NpmAdoptPolyglot, SetRequest::Full)
                        | (SetId::UvAdopt, SetRequest::Full)
                        | (SetId::UvAdoptPolyglot, SetRequest::Full)
                );
                if would_run {
                    return Err(BackendError::OfflineRequired { set: set.name() });
                }
            }
        }
    }
    let planned: Result<BackendPlan, BackendError> = match (set, request) {
        (SetId::Cargo, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: bazel(startup, "build", &["//rust/tests/fixtures/hello:hello"]),
            env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Cargo, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason: "crate_universe repin refreshes the whole Cargo lock; use `dx update cargo` for the set",
        }),
        (SetId::Npm, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: pnpm_argv(startup, workspace, &["update", "--lockfile-only"]),
            env: vec![],
        }),
        (SetId::Npm, SetRequest::Packages(packages)) => Ok(BackendPlan::Run {
            argv: [
                pnpm_argv(startup, workspace, &["update"]),
                packages.clone(),
                strings(&["--lockfile-only"]),
            ]
            .concat(),
            env: vec![],
        }),
        (SetId::NpmTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: pnpm_argv(
                startup,
                &workspace.join("quality/tools/javascript"),
                &["install", "--lockfile-only"],
            ),
            env: vec![],
        }),
        (SetId::NpmTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "npm-tools pins are exact in quality/tools/javascript; use `dx update npm-tools` for the set",
        }),
        (SetId::Uv, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "python/tests/fixtures/hello"]),
            env: vec![],
        }),
        (SetId::Uv, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv` for the set",
        }),
        (SetId::UvTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "quality/tools/python"]),
            env: vec![],
        }),
        (SetId::UvTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
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
            set: set.name(),
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
            set: set.name(),
            reason:
                "npm-adopt-polyglot pins are exact in examples/adopt-polyglot; use `dx update npm-adopt-polyglot` for the set",
        }),
        (SetId::UvAdopt, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-python"]),
            env: vec![],
        }),
        (SetId::UvAdopt, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-adopt` for the set",
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-polyglot"]),
            env: vec![],
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-adopt-polyglot` for the set",
        }),
        (SetId::Maven, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: bazel(startup, "run", &["@maven//:pin"]),
            env: vec![("REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Maven, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "maven pins are exact in MODULE.bazel; use `dx update maven` for the set",
        }),
        (SetId::NuGet, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: bazel(
                startup,
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
            set: set.name(),
            reason:
                "nuget pins are exact in paket.dependencies; use `dx update nuget` for the set",
        }),
        (SetId::Go, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Go, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason: "go pins track Gazelle for the shared go_deps extension; widen explicitly via `dx bump go:<module> <version>`",
        }),
        (SetId::Ruby, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Ruby, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason: "ruby pins are exact in third_party/ruby/Gemfile; widen the requirement there and regenerate the lock with `bundle lock`",
        }),
        (SetId::PowerShell, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::PowerShell, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:                 "powershell pins are exact in third_party/powershell/PSGallery.requirements.psd1; widen the requirement there and hand-regenerate PSGallery.lock.json, because consumer builds never run Install-Module",
        }),
    };
    if frozen && matches!(planned, Ok(BackendPlan::Run { .. })) {
        return Err(BackendError::FrozenLocked { set: set.name() });
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
}

pub fn check(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
    _frozen: bool,
    startup: &[String],
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
    match plan(workspace, set, request, offline, false, startup) {
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

fn configured_pnpm_argv(
    workspace: &Path,
    startup: &[String],
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
        bazel(startup, "run", &run),
        vec![match set.dir.as_str() {
            "." => workspace.to_string_lossy().into_owned(),
            dir => workspace.join(dir).to_string_lossy().into_owned(),
        }],
        command.to_vec(),
    ]
    .concat()
}

pub fn plan_configured(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    startup: &[String],
    closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match (set.ecosystem, request) {
        (Ecosystem::Bazel, Selection::Full) => Ok(BackendPlan::Noop),
        (Ecosystem::Bazel, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason:
                "bazel module pins change with `dx bump` or `bazel mod deps`; nothing to resolve",
        }),
        (Ecosystem::Uv, Selection::Full) => {
            if offline {
                return Err(BackendError::OfflineRequiredOwned {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLockedOwned {
                    set: set.name.clone(),
                });
            }
            Ok(BackendPlan::Run {
                argv: configured_uv_argv(set, closure, false, false)?,
                env: vec![],
            })
        }
        (Ecosystem::Uv, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
        (Ecosystem::Npm, request) => {
            if offline {
                return Err(BackendError::OfflineRequiredOwned {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLockedOwned {
                    set: set.name.clone(),
                });
            }
            let command = match request {
                Selection::Full => strings(&["install", "--lockfile-only"]),
                Selection::Packages(packages) => [
                    strings(&["update"]),
                    packages.clone(),
                    strings(&["--lockfile-only"]),
                ]
                .concat(),
            };
            Ok(BackendPlan::Run {
                argv: configured_pnpm_argv(workspace, startup, set, false, &command),
                env: npm_env(),
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
    startup: &[String],
    closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match (set.ecosystem, request) {
        (Ecosystem::Bazel, Selection::Full) => Ok(CheckPlan::Pinned),
        (Ecosystem::Bazel, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason:
                "bazel module pins change with `dx bump` or `bazel mod deps`; nothing to resolve",
        }),
        (Ecosystem::Uv, Selection::Full) => Ok(CheckPlan::Run {
            argv: configured_uv_argv(set, closure, true, offline)?,
            env: vec![],
        }),
        (Ecosystem::Uv, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
        (Ecosystem::Npm, Selection::Full) => {
            let mut command = strings(&["install", "--lockfile-only", "--frozen-lockfile"]);
            if offline {
                command.push("--offline".to_owned());
            }
            Ok(CheckPlan::Run {
                argv: configured_pnpm_argv(workspace, startup, set, offline, &command),
                env: npm_env(),
            })
        }
        (Ecosystem::Npm, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason: "pnpm checks the whole lock; select the set without a package",
        }),
    }
}

fn closure_path(
    set: &ResolvedSet,
    path: &Option<String>,
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
        }),
    }
}

fn configured_uv_argv(
    set: &ResolvedSet,
    closure: &UvClosure,
    check: bool,
    offline: bool,
) -> Result<Vec<String>, BackendError> {
    let uv = closure_path(set, &closure.uv, UV_BIN_VAR, UV_BIN_WHAT)?;
    let python = closure_path(set, &closure.python, UV_PYTHON_VAR, UV_PYTHON_WHAT)?;
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

fn bazel(startup: &[String], verb: &str, args: &[&str]) -> Vec<String> {
    let mut argv = dx_process::startup_argv(startup);
    argv.push(verb.to_owned());
    argv.extend(dx_process::build_cache_options(verb));
    argv.extend(strings(args));
    argv
}

fn pnpm_argv(startup: &[String], dir: &Path, command: &[&str]) -> Vec<String> {
    [
        bazel(startup, "run", &["@pnpm//:pnpm", "--", "--dir"]),
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

    fn bazel_head() -> Vec<String> {
        dx_process::startup_argv(&[])
    }

    fn bazel_argv(verb: &str, rest: &[String]) -> Vec<String> {
        [
            bazel_head(),
            vec![verb.to_owned()],
            dx_process::build_cache_options(verb),
            rest.to_vec(),
        ]
        .concat()
    }

    #[test]
    fn runs_bazel_marks_bazel_updaters() {
        let planned = |set: SetId| match plan(ws(), set, &SetRequest::Full, false, false, &[]) {
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
        match check(ws(), SetId::Uv, &SetRequest::Full, false, false, &[]) {
            Ok(CheckPlan::Run { argv, .. }) => assert!(!runs_bazel(&argv)),
            other => panic!("uv check: {other:?}"),
        }
        assert!(!runs_bazel(&[]));
    }

    #[test]
    fn full_plans_are_pinned_and_hermetic() {
        let cargo = plan(ws(), SetId::Cargo, &SetRequest::Full, false, false, &[]).expect("cargo");
        assert_eq!(
            cargo,
            BackendPlan::Run {
                argv: bazel_argv("build", &strings(&["//rust/tests/fixtures/hello:hello"])),
                env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
            }
        );
        let npm = plan(ws(), SetId::Npm, &SetRequest::Full, false, false, &[]).expect("npm");
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
        let maven = plan(ws(), SetId::Maven, &SetRequest::Full, false, false, &[]).expect("maven");
        assert_eq!(
            maven,
            BackendPlan::Run {
                argv: bazel_argv("run", &strings(&["@maven//:pin"])),
                env: vec![("REPIN".to_owned(), "1".to_owned())],
            }
        );
        let nuget = plan(ws(), SetId::NuGet, &SetRequest::Full, false, false, &[]).expect("nuget");
        match nuget {
            BackendPlan::Run { argv, env } => {
                assert!(argv.starts_with(&bazel_head()));
                assert!(argv.contains(&"@rules_dotnet//tools/paket2bazel".to_owned()));
                assert!(argv.contains(&"third_party/dotnet/paket.dependencies".to_owned()));
                assert!(argv.contains(&"third_party/dotnet/deps".to_owned()));
                assert!(env.is_empty());
            }
            BackendPlan::Noop => panic!("nuget runs paket2bazel"),
        }
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false, false, &[]).expect("go"),
            BackendPlan::Noop
        );
        let npm_tools =
            plan(ws(), SetId::NpmTools, &SetRequest::Full, false, false, &[]).expect("npm-tools");
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
            BackendPlan::Noop => panic!("npm-tools runs pnpm"),
        }
        let uv = plan(ws(), SetId::Uv, &SetRequest::Full, false, false, &[]).expect("uv");
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
        let uv_tools =
            plan(ws(), SetId::UvTools, &SetRequest::Full, false, false, &[]).expect("uv-tools");
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
        let npm_adopt =
            plan(ws(), SetId::NpmAdopt, &SetRequest::Full, false, false, &[]).expect("npm-adopt");
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
        let uv_adopt =
            plan(ws(), SetId::UvAdopt, &SetRequest::Full, false, false, &[]).expect("uv-adopt");
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
            &[],
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
            let argv = match plan(ws(), set, &request, false, false, &[]).expect(set.name()) {
                BackendPlan::Run { argv, .. } => argv,
                BackendPlan::Noop => panic!("{} runs pnpm", set.name()),
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
    fn cargo_selective_reports_unsupported_never_full() {
        let error = plan(
            ws(),
            SetId::Cargo,
            &SetRequest::Packages(vec!["anyhow".to_owned()]),
            false,
            false,
            &[],
        )
        .expect_err("cargo selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("cargo"));
        assert!(error
            .to_string()
            .contains("use `dx update cargo` for the set"));
    }

    #[test]
    fn nuget_selective_reports_unsupported_never_full() {
        let error = plan(
            ws(),
            SetId::NuGet,
            &SetRequest::Packages(vec!["FSharp.Core".to_owned()]),
            false,
            false,
            &[],
        )
        .expect_err("nuget selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("nuget"));
        assert!(error
            .to_string()
            .contains("use `dx update nuget` for the set"));
    }

    #[test]
    fn go_selective_reports_unsupported_never_full() {
        let error = plan(
            ws(),
            SetId::Go,
            &SetRequest::Packages(vec!["github.com/google/go-cmp/cmp".to_owned()]),
            false,
            false,
            &[],
        )
        .expect_err("go selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("go"));
        assert!(error.to_string().contains("dx bump go:"));
    }

    #[test]
    fn go_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false, false, &[]).expect("go full"),
            BackendPlan::Noop
        );
    }

    #[test]
    fn ruby_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Ruby, &SetRequest::Full, false, false, &[]).expect("ruby full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::Ruby,
            &SetRequest::Packages(vec!["rspec-core".to_owned()]),
            false,
            false,
            &[],
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
                &[]
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
            &[],
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
                SetRequest::Packages(vec!["rules_dx/go/tests/fixtures/hello".to_owned()]),
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
            let error = plan(ws(), set, &packages, false, false, &[]).expect_err("unsupported");
            assert!(matches!(error, BackendError::Unsupported { .. }), "{set:?}");
            assert!(error.to_string().contains(set.name()));
        }
    }

    #[test]
    fn maven_selective_reports_unsupported_with_set_hint() {
        for artifact in [
            "junit:junit".to_owned(),
            "org.junit.jupiter:junit-jupiter-api".to_owned(),
        ] {
            let error = plan(
                ws(),
                SetId::Maven,
                &SetRequest::Packages(vec![artifact.clone()]),
                false,
                false,
                &[],
            )
            .expect_err("maven selective unsupported");
            let message = error.to_string();
            assert!(
                matches!(error, BackendError::Unsupported { .. }),
                "{artifact}"
            );
            assert!(message.contains("maven"), "{artifact}: {message}");
            assert!(
                message.contains("use `dx update maven` for the set"),
                "{artifact}: {message}"
            );
        }
    }

    #[test]
    fn argv_never_names_a_dx_lockfile() {
        for set in SetId::ALL {
            if let Ok(BackendPlan::Run { argv, .. }) =
                plan(ws(), set, &SetRequest::Full, false, false, &[])
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
            check(ws(), SetId::Uv, &SetRequest::Full, false, false, &[]).expect("uv check"),
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
            check(ws(), SetId::Uv, &SetRequest::Full, true, false, &[]).expect("uv check offline"),
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
        }
    }

    fn closure() -> UvClosure {
        UvClosure {
            uv: Some(abs("uv")),
            python: Some(abs("python3")),
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
            plan_configured(ws(), &set, &Selection::Full, false, false, &[], &closure())
                .expect("configured update"),
            BackendPlan::Run {
                argv: uv_argv(false, false),
                env: vec![],
            }
        );
        assert_eq!(
            check_configured(ws(), &set, &Selection::Full, false, false, &[], &closure())
                .expect("configured check"),
            CheckPlan::Run {
                argv: uv_argv(true, false),
                env: vec![],
            }
        );
        assert_eq!(
            check_configured(ws(), &set, &Selection::Full, true, false, &[], &closure())
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
                },
                UV_BIN_VAR,
            ),
            (
                UvClosure {
                    uv: Some(abs("uv")),
                    python: None,
                },
                UV_PYTHON_VAR,
            ),
            (
                UvClosure {
                    uv: Some(abs("uv")),
                    python: Some("python3".to_owned()),
                },
                UV_PYTHON_VAR,
            ),
        ];
        for (closure, var) in cases {
            let error = plan_configured(ws(), &set, &Selection::Full, false, false, &[], &closure)
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
            let error = check_configured(ws(), &set, &Selection::Full, false, false, &[], &closure)
                .expect_err("check needs the pinned closure");
            assert!(
                matches!(error, BackendError::BackendUnavailable { var: got, .. } if got == var),
                "{error:?}"
            );
        }
    }

    #[test]
    fn configured_uv_selective_and_offline_update_fail_closed() {
        let set = configured_set("apps/frontend");
        let error = plan_configured(
            ws(), &set,
            &Selection::Packages(vec!["anyio".to_owned()]),
            false,
            false,
            &[], &closure(),
        )
        .expect_err("configured selective stays unsupported");
        assert!(
            matches!(error, BackendError::UnsupportedOwned { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("frontend"));
        let error = check_configured(
            ws(), &set,
            &Selection::Packages(vec!["anyio".to_owned()]),
            false,
            false,
            &[], &closure(),
        )
        .expect_err("configured selective check stays unsupported");
        assert!(
            matches!(error, BackendError::UnsupportedOwned { .. }),
            "{error:?}"
        );
        let error = plan_configured(ws(), &set, &Selection::Full, true, false, &[], &closure())
            .expect_err("offline update needs network");
        assert!(
            matches!(error, BackendError::OfflineRequiredOwned { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("offline_required"));
    }

    #[test]
    fn every_bazel_plan_starts_with_launcher_and_startup_opts() {
        let startup = vec!["--output_base=/ob".to_owned()];
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
            let Ok(BackendPlan::Run { argv, .. }) =
                plan(ws(), set, &request, false, false, &startup)
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
            check(
                ws(),
                SetId::Cargo,
                &SetRequest::Full,
                false,
                false,
                &startup
            )
            .expect("cargo check"),
            CheckPlan::Unavailable
        );
    }

    #[test]
    fn pinned_sets_check_without_launching() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, false, &[]).expect("pinned check"),
                CheckPlan::Pinned,
                "{set:?}"
            );
            assert_eq!(
                check(ws(), set, &SetRequest::Full, true, false, &[])
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
            &[],
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
            &[],
        )
        .expect_err("go selective stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        let error = check(ws(), SetId::Cargo, &SetRequest::Full, true, false, &[])
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
                check(ws(), set, &SetRequest::Full, false, false, &[]).expect("check plans"),
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
                &[],
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
            let error = plan(ws(), set, &SetRequest::Full, true, false, &[])
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
            &[],
        )
        .expect_err("npm selective offline needs network");
        assert!(
            matches!(selective, BackendError::OfflineRequired { .. }),
            "{selective:?}"
        );
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                plan(ws(), set, &SetRequest::Full, true, false, &[]).expect("pinned noop offline"),
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
            let error = plan(ws(), set, &packages, true, false, &[])
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
            let error = plan(ws(), set, &SetRequest::Full, false, true, &[])
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
                plan(ws(), set, &SetRequest::Full, false, true, &[]).expect("pinned noop frozen"),
                BackendPlan::Noop,
                "{set:?}"
            );
        }
        let both =
            plan(ws(), SetId::Cargo, &SetRequest::Full, true, true, &[]).expect_err("offline wins");
        assert!(
            matches!(both, BackendError::OfflineRequired { .. }),
            "{both:?}"
        );
    }

    #[test]
    fn frozen_check_passes_without_resolution_changes() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, true, &[]).expect("pinned check frozen"),
                CheckPlan::Pinned,
                "{set:?}"
            );
        }
        let uv =
            check(ws(), SetId::Uv, &SetRequest::Full, false, true, &[]).expect("uv check frozen");
        assert!(
            matches!(uv, CheckPlan::Run { .. }),
            "uv lock --check verifies without resolving: {uv:?}"
        );
        assert_eq!(
            check(ws(), SetId::Cargo, &SetRequest::Full, false, true, &[])
                .expect("cargo check frozen"),
            CheckPlan::Unavailable
        );
    }

    #[test]
    fn frozen_configured_update_is_locked_while_check_passes_through() {
        let set = configured_set("apps/frontend");
        let error = plan_configured(ws(), &set, &Selection::Full, false, true, &[], &closure())
            .expect_err("configured update frozen");
        assert!(
            matches!(error, BackendError::FrozenLockedOwned { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("frozen_locked"), "{error}");
        assert!(error.to_string().contains("frontend"), "{error}");
        let checked = check_configured(ws(), &set, &Selection::Full, false, true, &[], &closure())
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
        }
    }

    fn npm_env_expected() -> Vec<(String, String)> {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        vec![
            ("npm_config_globalconfig".to_owned(), null.to_owned()),
            ("npm_config_userconfig".to_owned(), null.to_owned()),
        ]
    }

    fn npm_run(startup: &[String], run: &[&str], dir: &str, command: &[&str]) -> Vec<String> {
        [
            dx_process::startup_argv(startup),
            vec!["run".to_owned()],
            dx_process::build_cache_options("run"),
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
        let startup = vec!["--output_base=/ob".to_owned()];
        let planned = plan_configured(
            ws(),
            &set,
            &Selection::Full,
            false,
            false,
            &startup,
            &UvClosure::default(),
        )
        .expect("npm update");
        assert_eq!(
            planned,
            BackendPlan::Run {
                argv: npm_run(&startup, &[], "apps/web", &["install", "--lockfile-only"]),
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
            &[],
            &UvClosure::default(),
        )
        .expect("npm selective update");
        assert_eq!(
            planned,
            BackendPlan::Run {
                argv: npm_run(
                    &[],
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
                    &[],
                    &UvClosure::default()
                )
                .expect("npm check"),
                CheckPlan::Run {
                    argv: npm_run(
                        &[],
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
            &[],
            &UvClosure::default(),
        )
        .expect("npm offline check");
        assert_eq!(
            checked,
            CheckPlan::Run {
                argv: npm_run(
                    &[],
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
            &[],
            &UvClosure::default(),
        )
        .expect_err("selective check");
        assert!(
            matches!(error, BackendError::UnsupportedOwned { ref set, .. } if set == "frontend"),
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
                &[],
                &UvClosure::default(),
            )
            .expect_err("offline update");
            assert_eq!(
                error,
                BackendError::OfflineRequiredOwned {
                    set: "frontend".to_owned()
                }
            );
            let error = plan_configured(
                ws(),
                &set,
                &request,
                false,
                true,
                &[],
                &UvClosure::default(),
            )
            .expect_err("frozen update");
            assert_eq!(
                error,
                BackendError::FrozenLockedOwned {
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
            &[],
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
}
