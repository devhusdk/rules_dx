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
}

pub fn plan(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
    frozen: bool,
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
            argv: strings(&["bazel", "build", "//rust/tests/fixtures/hello:hello"]),
            env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Cargo, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason: "crate_universe repin refreshes the whole Cargo lock; use `dx update --apply cargo` for the set",
        }),
        (SetId::Npm, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: pnpm_argv(workspace, &["update", "--lockfile-only"]),
            env: vec![],
        }),
        (SetId::Npm, SetRequest::Packages(packages)) => Ok(BackendPlan::Run {
            argv: [
                pnpm_argv(workspace, &["update"]),
                packages.clone(),
                strings(&["--lockfile-only"]),
            ]
            .concat(),
            env: vec![],
        }),
        (SetId::NpmTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: pnpm_argv(
                &workspace.join("quality/tools/javascript"),
                &["install", "--lockfile-only"],
            ),
            env: vec![],
        }),
        (SetId::NpmTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "npm-tools pins are exact in quality/tools/javascript; use `dx update --apply npm-tools` for the set",
        }),
        (SetId::Uv, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "python/tests/fixtures/hello"]),
            env: vec![],
        }),
        (SetId::Uv, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update --apply uv` for the set",
        }),
        (SetId::UvTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "quality/tools/python"]),
            env: vec![],
        }),
        (SetId::UvTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update --apply uv-tools` for the set",
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
                "npm-adopt pins are exact in examples/adopt-js-ts; use `dx update --apply npm-adopt` for the set",
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
                "npm-adopt-polyglot pins are exact in examples/adopt-polyglot; use `dx update --apply npm-adopt-polyglot` for the set",
        }),
        (SetId::UvAdopt, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-python"]),
            env: vec![],
        }),
        (SetId::UvAdopt, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update --apply uv-adopt` for the set",
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-polyglot"]),
            env: vec![],
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update --apply uv-adopt-polyglot` for the set",
        }),
        (SetId::Maven, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["bazel", "run", "@maven//:pin"]),
            env: vec![("REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Maven, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "maven pins are exact in MODULE.bazel; use `dx update --apply maven` for the set",
        }),
        (SetId::NuGet, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&[
                "bazel",
                "run",
                "@rules_dotnet//tools/paket2bazel",
                "--",
                "--dependencies-file",
                "third_party/dotnet/paket.dependencies",
                "--output-folder",
                "third_party/dotnet/deps",
            ]),
            env: vec![],
        }),
        (SetId::NuGet, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason:
                "nuget pins are exact in paket.dependencies; use `dx update --apply nuget` for the set",
        }),
        (SetId::Go, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Go, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name(),
            reason: "go pins track Gazelle for the shared go_deps extension; widen explicitly via `dx bump --apply gomod:<module> <version>`",
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
    frozen: bool,
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
    match plan(workspace, set, request, offline, frozen) {
        Ok(_) => Ok(CheckPlan::Unavailable),
        Err(error) => Err(error),
    }
}

pub fn plan_configured(
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
) -> Result<BackendPlan, BackendError> {
    match (set.ecosystem, request) {
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
                argv: configured_uv_argv(&set.dir, false, false),
                env: vec![],
            })
        }
        (Ecosystem::Uv, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
    }
}

pub fn check_configured(
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    _frozen: bool,
) -> Result<CheckPlan, BackendError> {
    match (set.ecosystem, request) {
        (Ecosystem::Uv, Selection::Full) => Ok(CheckPlan::Run {
            argv: configured_uv_argv(&set.dir, true, offline),
            env: vec![],
        }),
        (Ecosystem::Uv, Selection::Packages(_)) => Err(BackendError::UnsupportedOwned {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
    }
}

fn configured_uv_argv(dir: &str, check: bool, offline: bool) -> Vec<String> {
    let mut argv = strings(&["uv", "lock"]);
    if check {
        argv.push("--check".to_owned());
    }
    argv.push("--directory".to_owned());
    argv.push(dir.to_owned());
    if offline {
        argv.push("--offline".to_owned());
    }
    argv
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn pnpm_argv(dir: &Path, command: &[&str]) -> Vec<String> {
    [
        strings(&["bazel", "run", "@pnpm//:pnpm", "--", "--dir"]),
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

    #[test]
    fn full_plans_are_pinned_and_hermetic() {
        let cargo = plan(ws(), SetId::Cargo, &SetRequest::Full, false, false).expect("cargo");
        assert_eq!(
            cargo,
            BackendPlan::Run {
                argv: vec![
                    "bazel".to_owned(),
                    "build".to_owned(),
                    "//rust/tests/fixtures/hello:hello".to_owned(),
                ],
                env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
            }
        );
        let npm = plan(ws(), SetId::Npm, &SetRequest::Full, false, false).expect("npm");
        assert_eq!(
            npm,
            BackendPlan::Run {
                argv: vec![
                    "bazel".to_owned(),
                    "run".to_owned(),
                    "@pnpm//:pnpm".to_owned(),
                    "--".to_owned(),
                    "--dir".to_owned(),
                    ws_arg(),
                    "update".to_owned(),
                    "--lockfile-only".to_owned(),
                ],
                env: vec![],
            }
        );
        let maven = plan(ws(), SetId::Maven, &SetRequest::Full, false, false).expect("maven");
        assert_eq!(
            maven,
            BackendPlan::Run {
                argv: vec![
                    "bazel".to_owned(),
                    "run".to_owned(),
                    "@maven//:pin".to_owned(),
                ],
                env: vec![("REPIN".to_owned(), "1".to_owned())],
            }
        );
        let nuget = plan(ws(), SetId::NuGet, &SetRequest::Full, false, false).expect("nuget");
        match nuget {
            BackendPlan::Run { argv, env } => {
                assert_eq!(argv[0], "bazel");
                assert!(argv.contains(&"@rules_dotnet//tools/paket2bazel".to_owned()));
                assert!(argv.contains(&"third_party/dotnet/paket.dependencies".to_owned()));
                assert!(argv.contains(&"third_party/dotnet/deps".to_owned()));
                assert!(env.is_empty());
            }
            BackendPlan::Noop => panic!("nuget runs paket2bazel"),
        }
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false, false).expect("go"),
            BackendPlan::Noop
        );
        let npm_tools =
            plan(ws(), SetId::NpmTools, &SetRequest::Full, false, false).expect("npm-tools");
        match npm_tools {
            BackendPlan::Run { argv, env } => {
                assert_eq!(
                    argv,
                    vec![
                        "bazel".to_owned(),
                        "run".to_owned(),
                        "@pnpm//:pnpm".to_owned(),
                        "--".to_owned(),
                        "--dir".to_owned(),
                        dir_arg("quality/tools/javascript"),
                        "install".to_owned(),
                        "--lockfile-only".to_owned(),
                    ]
                );
                assert!(env.is_empty());
            }
            BackendPlan::Noop => panic!("npm-tools runs pnpm"),
        }
        let uv = plan(ws(), SetId::Uv, &SetRequest::Full, false, false).expect("uv");
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
            plan(ws(), SetId::UvTools, &SetRequest::Full, false, false).expect("uv-tools");
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
            plan(ws(), SetId::NpmAdopt, &SetRequest::Full, false, false).expect("npm-adopt");
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
            plan(ws(), SetId::UvAdopt, &SetRequest::Full, false, false).expect("uv-adopt");
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
        )
        .expect("npm selective");
        assert_eq!(
            selective,
            BackendPlan::Run {
                argv: vec![
                    "bazel".to_owned(),
                    "run".to_owned(),
                    "@pnpm//:pnpm".to_owned(),
                    "--".to_owned(),
                    "--dir".to_owned(),
                    ws_arg(),
                    "update".to_owned(),
                    "jest".to_owned(),
                    "react".to_owned(),
                    "--lockfile-only".to_owned(),
                ],
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
            let argv = match plan(ws(), set, &request, false, false).expect(set.name()) {
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
        )
        .expect_err("cargo selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("cargo"));
        assert!(error
            .to_string()
            .contains("use `dx update --apply cargo` for the set"));
    }

    #[test]
    fn nuget_selective_reports_unsupported_never_full() {
        let error = plan(
            ws(),
            SetId::NuGet,
            &SetRequest::Packages(vec!["FSharp.Core".to_owned()]),
            false,
            false,
        )
        .expect_err("nuget selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("nuget"));
        assert!(error
            .to_string()
            .contains("use `dx update --apply nuget` for the set"));
    }

    #[test]
    fn go_selective_reports_unsupported_never_full() {
        let error = plan(
            ws(),
            SetId::Go,
            &SetRequest::Packages(vec!["github.com/google/go-cmp/cmp".to_owned()]),
            false,
            false,
        )
        .expect_err("go selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("go"));
        assert!(error.to_string().contains("dx bump --apply gomod"));
    }

    #[test]
    fn go_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false, false).expect("go full"),
            BackendPlan::Noop
        );
    }

    #[test]
    fn ruby_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Ruby, &SetRequest::Full, false, false).expect("ruby full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::Ruby,
            &SetRequest::Packages(vec!["rspec-core".to_owned()]),
            false,
            false,
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
            plan(ws(), SetId::PowerShell, &SetRequest::Full, false, false)
                .expect("powershell full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::PowerShell,
            &SetRequest::Packages(vec!["Pester".to_owned()]),
            false,
            false,
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
            let error = plan(ws(), set, &packages, false, false).expect_err("unsupported");
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
            )
            .expect_err("maven selective unsupported");
            let message = error.to_string();
            assert!(
                matches!(error, BackendError::Unsupported { .. }),
                "{artifact}"
            );
            assert!(message.contains("maven"), "{artifact}: {message}");
            assert!(
                message.contains("use `dx update --apply maven` for the set"),
                "{artifact}: {message}"
            );
        }
    }

    #[test]
    fn argv_never_names_a_dx_lockfile() {
        for set in SetId::ALL {
            if let Ok(BackendPlan::Run { argv, .. }) =
                plan(ws(), set, &SetRequest::Full, false, false)
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
            check(ws(), SetId::Uv, &SetRequest::Full, false, false).expect("uv check"),
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
            check(ws(), SetId::Uv, &SetRequest::Full, true, false).expect("uv check offline"),
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

    #[test]
    fn configured_uv_plans_use_the_set_directory() {
        let set = configured_set("apps/frontend");
        assert_eq!(
            plan_configured(&set, &Selection::Full, false, false).expect("configured update"),
            BackendPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--directory".to_owned(),
                    "apps/frontend".to_owned(),
                ],
                env: vec![],
            }
        );
        assert_eq!(
            check_configured(&set, &Selection::Full, false, false).expect("configured check"),
            CheckPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--check".to_owned(),
                    "--directory".to_owned(),
                    "apps/frontend".to_owned(),
                ],
                env: vec![],
            }
        );
        assert_eq!(
            check_configured(&set, &Selection::Full, true, false)
                .expect("configured check offline"),
            CheckPlan::Run {
                argv: vec![
                    "uv".to_owned(),
                    "lock".to_owned(),
                    "--check".to_owned(),
                    "--directory".to_owned(),
                    "apps/frontend".to_owned(),
                    "--offline".to_owned(),
                ],
                env: vec![],
            }
        );
    }

    #[test]
    fn configured_uv_selective_and_offline_update_fail_closed() {
        let set = configured_set("apps/frontend");
        let error = plan_configured(
            &set,
            &Selection::Packages(vec!["anyio".to_owned()]),
            false,
            false,
        )
        .expect_err("configured selective stays unsupported");
        assert!(
            matches!(error, BackendError::UnsupportedOwned { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("frontend"));
        let error = check_configured(
            &set,
            &Selection::Packages(vec!["anyio".to_owned()]),
            false,
            false,
        )
        .expect_err("configured selective check stays unsupported");
        assert!(
            matches!(error, BackendError::UnsupportedOwned { .. }),
            "{error:?}"
        );
        let error = plan_configured(&set, &Selection::Full, true, false)
            .expect_err("offline update needs network");
        assert!(
            matches!(error, BackendError::OfflineRequiredOwned { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("offline_required"));
    }

    #[test]
    fn pinned_sets_check_without_launching() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, false).expect("pinned check"),
                CheckPlan::Pinned,
                "{set:?}"
            );
            assert_eq!(
                check(ws(), set, &SetRequest::Full, true, false).expect("pinned check offline"),
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
        )
        .expect_err("go selective stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        let error = check(ws(), SetId::Cargo, &SetRequest::Full, true, false)
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
                check(ws(), set, &SetRequest::Full, false, false).expect("check plans"),
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
            let error =
                plan(ws(), set, &SetRequest::Full, true, false).expect_err("offline needs network");
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
        )
        .expect_err("npm selective offline needs network");
        assert!(
            matches!(selective, BackendError::OfflineRequired { .. }),
            "{selective:?}"
        );
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                plan(ws(), set, &SetRequest::Full, true, false).expect("pinned noop offline"),
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
            let error =
                plan(ws(), set, &packages, true, false).expect_err("unsupported stays unsupported");
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
            let error =
                plan(ws(), set, &SetRequest::Full, false, true).expect_err("frozen keeps pins");
            assert!(
                matches!(error, BackendError::FrozenLocked { .. }),
                "{set:?}: {error:?}"
            );
            assert!(error.to_string().contains("frozen_locked"), "{error}");
            assert!(error.to_string().contains(set.name()), "{error}");
        }
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                plan(ws(), set, &SetRequest::Full, false, true).expect("pinned noop frozen"),
                BackendPlan::Noop,
                "{set:?}"
            );
        }
        let both =
            plan(ws(), SetId::Cargo, &SetRequest::Full, true, true).expect_err("offline wins");
        assert!(
            matches!(both, BackendError::OfflineRequired { .. }),
            "{both:?}"
        );
    }

    #[test]
    fn frozen_check_passes_without_resolution_changes() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false, true).expect("pinned check frozen"),
                CheckPlan::Pinned,
                "{set:?}"
            );
        }
        let uv = check(ws(), SetId::Uv, &SetRequest::Full, false, true).expect("uv check frozen");
        assert!(
            matches!(uv, CheckPlan::Run { .. }),
            "uv lock --check verifies without resolving: {uv:?}"
        );
        let error =
            check(ws(), SetId::Cargo, &SetRequest::Full, false, true).expect_err("cargo frozen");
        assert!(
            matches!(error, BackendError::FrozenLocked { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn frozen_configured_update_is_locked_while_check_passes_through() {
        let set = configured_set("apps/frontend");
        let error = plan_configured(&set, &Selection::Full, false, true)
            .expect_err("configured update frozen");
        assert!(
            matches!(error, BackendError::FrozenLockedOwned { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("frozen_locked"), "{error}");
        assert!(error.to_string().contains("frontend"), "{error}");
        let checked =
            check_configured(&set, &Selection::Full, false, true).expect("configured check frozen");
        assert!(
            matches!(checked, CheckPlan::Run { .. }),
            "uv lock --check verifies without resolving: {checked:?}"
        );
    }
}
