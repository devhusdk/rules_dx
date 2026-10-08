use super::config::{record_dir, BackendKind, DependencySet};
use super::selector::SetRequest;
use super::sets::SetId;
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
    Unsupported { set: String, reason: String },
    #[error("offline_required: cannot update {set} without network (re-run without --offline once connected)")]
    OfflineRequired { set: String },
}

pub fn plan(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
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
                    return Err(BackendError::OfflineRequired {
                        set: set.name().to_owned(),
                    });
                }
            }
        }
    }
    match (set, request) {
        (SetId::Cargo, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["bazel", "build", "//rust/tests/fixtures/hello:hello"]),
            env: vec![("CARGO_BAZEL_REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Cargo, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "crate_universe repin refreshes the whole Cargo lock; use `dx update cargo` for the set".to_owned(),
        }),
        (set @ (SetId::Npm | SetId::NpmTools | SetId::NpmAdopt | SetId::NpmAdoptPolyglot), request) => {
            let name = set.name().to_owned();
            match super::config::builtin_sets()
                .into_iter()
                .find(|record| record.name == name)
            {
                Some(record) => plan_named(workspace, &record, request, offline),
                None => Err(BackendError::Unsupported {
                    set: name,
                    reason: "builtin npm record is missing".to_owned(),
                }),
            }
        }
        (SetId::Uv, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "python/tests/fixtures/hello"]),
            env: vec![],
        }),
        (SetId::Uv, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv` for the set".to_owned(),
        }),
        (SetId::UvTools, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "quality/tools/python"]),
            env: vec![],
        }),
        (SetId::UvTools, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-tools` for the set".to_owned(),
        }),
        (SetId::UvAdopt, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-python"]),
            env: vec![],
        }),
        (SetId::UvAdopt, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-adopt` for the set".to_owned(),
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["uv", "lock", "--directory", "examples/adopt-polyglot"]),
            env: vec![],
        }),
        (SetId::UvAdoptPolyglot, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "uv repin refreshes the whole uv lock; use `dx update uv-adopt-polyglot` for the set".to_owned(),
        }),
        (SetId::Maven, SetRequest::Full) => Ok(BackendPlan::Run {
            argv: strings(&["bazel", "run", "@maven//:pin"]),
            env: vec![("REPIN".to_owned(), "1".to_owned())],
        }),
        (SetId::Maven, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason:
                "maven pins are exact in MODULE.bazel; use `dx update maven` for the set".to_owned(),
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
            set: set.name().to_owned(),
            reason:
                "nuget pins are exact in paket.dependencies; use `dx update nuget` for the set".to_owned(),
        }),
        (SetId::Go, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Go, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "go pins track Gazelle for the shared go_deps extension; widen explicitly via `dx bump gomod:<module> <version>`".to_owned(),
        }),
        (SetId::Ruby, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::Ruby, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "ruby pins are exact in third_party/ruby/Gemfile; widen the requirement there and regenerate the lock with `bundle lock`".to_owned(),
        }),
        (SetId::PowerShell, SetRequest::Full) => Ok(BackendPlan::Noop),
        (SetId::PowerShell, SetRequest::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name().to_owned(),
            reason: "powershell pins are exact in third_party/powershell/PSGallery.requirements.psd1; widen the requirement there and hand-regenerate PSGallery.lock.json, because consumer builds never run Install-Module".to_owned(),
        }),
    }
}

fn unmigrated(record: &DependencySet) -> BackendError {
    BackendError::Unsupported {
        set: record.name.clone(),
        reason: format!(
            "{} records resolve but only npm records plan in this migration; widen {} by hand",
            record.backend.name(),
            record.name,
        ),
    }
}

fn npm_option(record: &DependencySet, key: &str, fallback: &str) -> String {
    record
        .options
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_owned())
}

pub fn plan_named(
    workspace: &Path,
    record: &DependencySet,
    request: &SetRequest,
    offline: bool,
) -> Result<BackendPlan, BackendError> {
    if record.backend != BackendKind::Npm {
        return Err(unmigrated(record));
    }
    let mode = npm_option(record, "mode", "update");
    if mode != "update" && mode != "install" {
        return Err(BackendError::Unsupported {
            set: record.name.clone(),
            reason: format!(
                "unknown npm mode {mode:?} for {}: want update or install",
                record.name,
            ),
        });
    }
    let runner = npm_option(record, "runner", "host");
    if runner != "bazel" && runner != "host" {
        return Err(BackendError::Unsupported {
            set: record.name.clone(),
            reason: format!(
                "unknown npm runner {runner:?} for {}: want bazel or host",
                record.name,
            ),
        });
    }
    if offline && (matches!(request, SetRequest::Full) || mode == "update") {
        return Err(BackendError::OfflineRequired {
            set: record.name.clone(),
        });
    }
    let dir = record_dir(record);
    let command: Vec<String> = match (mode.as_str(), request) {
        ("update", SetRequest::Full) => strings(&["update", "--lockfile-only"]),
        ("update", SetRequest::Packages(packages)) => [
            vec!["update".to_owned()],
            packages.clone(),
            vec!["--lockfile-only".to_owned()],
        ]
        .concat(),
        ("install", SetRequest::Full) => strings(&["install", "--lockfile-only"]),
        _ => {
            let where_exact = if dir.is_empty() {
                ".".to_owned()
            } else {
                dir.clone()
            };
            return Err(BackendError::Unsupported {
                set: record.name.clone(),
                reason: format!(
                    "{} pins are exact in {where_exact}; use `dx update {}` for the set",
                    record.name, record.name,
                ),
            });
        }
    };
    if runner == "bazel" {
        let root = if dir.is_empty() {
            workspace.to_owned()
        } else {
            workspace.join(&dir)
        };
        let words: Vec<&str> = command.iter().map(String::as_str).collect();
        Ok(BackendPlan::Run {
            argv: pnpm_argv(&root, &words),
            env: vec![],
        })
    } else {
        let here = if dir.is_empty() { ".".to_owned() } else { dir };
        Ok(BackendPlan::Run {
            argv: [vec!["pnpm".to_owned(), "--dir".to_owned(), here], command].concat(),
            env: vec![],
        })
    }
}

pub fn check_named(
    workspace: &Path,
    record: &DependencySet,
    request: &SetRequest,
    offline: bool,
) -> Result<CheckPlan, BackendError> {
    if record.backend != BackendKind::Npm {
        return Err(unmigrated(record));
    }
    match plan_named(workspace, record, request, offline) {
        Ok(_) => Ok(CheckPlan::Unavailable),
        Err(error) => Err(error),
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
}

pub fn check(
    workspace: &Path,
    set: SetId,
    request: &SetRequest,
    offline: bool,
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
    match plan(workspace, set, request, offline) {
        Ok(_) => Ok(CheckPlan::Unavailable),
        Err(error) => Err(error),
    }
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
        let cargo = plan(ws(), SetId::Cargo, &SetRequest::Full, false).expect("cargo");
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
        let npm = plan(ws(), SetId::Npm, &SetRequest::Full, false).expect("npm");
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
        let maven = plan(ws(), SetId::Maven, &SetRequest::Full, false).expect("maven");
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
        let nuget = plan(ws(), SetId::NuGet, &SetRequest::Full, false).expect("nuget");
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
            plan(ws(), SetId::Go, &SetRequest::Full, false).expect("go"),
            BackendPlan::Noop
        );
        let npm_tools = plan(ws(), SetId::NpmTools, &SetRequest::Full, false).expect("npm-tools");
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
        let uv = plan(ws(), SetId::Uv, &SetRequest::Full, false).expect("uv");
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
        let uv_tools = plan(ws(), SetId::UvTools, &SetRequest::Full, false).expect("uv-tools");
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
        let npm_adopt = plan(ws(), SetId::NpmAdopt, &SetRequest::Full, false).expect("npm-adopt");
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
        let uv_adopt = plan(ws(), SetId::UvAdopt, &SetRequest::Full, false).expect("uv-adopt");
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
            let argv = match plan(ws(), set, &request, false).expect(set.name()) {
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
        )
        .expect_err("go selective is wont-fix");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("go"));
        assert!(error.to_string().contains("dx bump gomod"));
    }

    #[test]
    fn go_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Go, &SetRequest::Full, false).expect("go full"),
            BackendPlan::Noop
        );
    }

    #[test]
    fn ruby_full_is_pinned_noop_success() {
        assert_eq!(
            plan(ws(), SetId::Ruby, &SetRequest::Full, false).expect("ruby full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::Ruby,
            &SetRequest::Packages(vec!["rspec-core".to_owned()]),
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
            plan(ws(), SetId::PowerShell, &SetRequest::Full, false).expect("powershell full"),
            BackendPlan::Noop
        );
        let error = plan(
            ws(),
            SetId::PowerShell,
            &SetRequest::Packages(vec!["Pester".to_owned()]),
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
            let error = plan(ws(), set, &packages, false).expect_err("unsupported");
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
            if let Ok(BackendPlan::Run { argv, .. }) = plan(ws(), set, &SetRequest::Full, false) {
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
            check(ws(), SetId::Uv, &SetRequest::Full, false).expect("uv check"),
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
            check(ws(), SetId::Uv, &SetRequest::Full, true).expect("uv check offline"),
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

    #[test]
    fn pinned_sets_check_without_launching() {
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                check(ws(), set, &SetRequest::Full, false).expect("pinned check"),
                CheckPlan::Pinned,
                "{set:?}"
            );
            assert_eq!(
                check(ws(), set, &SetRequest::Full, true).expect("pinned check offline"),
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
        )
        .expect_err("go selective stays unsupported");
        assert!(
            matches!(error, BackendError::Unsupported { .. }),
            "{error:?}"
        );
        let error = check(ws(), SetId::Cargo, &SetRequest::Full, true)
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
                check(ws(), set, &SetRequest::Full, false).expect("check plans"),
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
            )
            .expect("npm selective check plans"),
            CheckPlan::Unavailable,
        );
    }

    #[test]
    fn named_npm_plans_match_legacy_builtin_plans() {
        use super::super::config;
        for set in [
            SetId::Npm,
            SetId::NpmTools,
            SetId::NpmAdopt,
            SetId::NpmAdoptPolyglot,
        ] {
            let record = config::builtin_record(set.name()).expect("builtin npm record");
            for request in [
                SetRequest::Full,
                SetRequest::Packages(vec!["jest".to_owned()]),
            ] {
                for offline in [false, true] {
                    assert_eq!(
                        plan(ws(), set, &request, offline),
                        plan_named(ws(), &record, &request, offline),
                        "{set:?} {request:?} offline={offline}",
                    );
                }
            }
        }
    }

    #[test]
    fn consumer_npm_record_plans_host_update() {
        use super::super::config::{record_dir, DependencySet};
        use std::collections::BTreeMap;
        let record = DependencySet {
            name: "web".to_owned(),
            backend: super::super::config::BackendKind::Npm,
            manifests: vec!["apps/web/package.json".to_owned()],
            locks: vec!["apps/web/pnpm-lock.yaml".to_owned()],
            scopes: vec!["apps/web".to_owned()],
            options: BTreeMap::new(),
        };
        assert_eq!(record_dir(&record), "apps/web");
        assert_eq!(
            plan_named(ws(), &record, &SetRequest::Full, false).expect("web full"),
            BackendPlan::Run {
                argv: vec![
                    "pnpm".to_owned(),
                    "--dir".to_owned(),
                    "apps/web".to_owned(),
                    "update".to_owned(),
                    "--lockfile-only".to_owned(),
                ],
                env: vec![],
            }
        );
        assert_eq!(
            plan_named(
                ws(),
                &record,
                &SetRequest::Packages(vec!["react".to_owned()]),
                false,
            )
            .expect("web selective"),
            BackendPlan::Run {
                argv: vec![
                    "pnpm".to_owned(),
                    "--dir".to_owned(),
                    "apps/web".to_owned(),
                    "update".to_owned(),
                    "react".to_owned(),
                    "--lockfile-only".to_owned(),
                ],
                env: vec![],
            }
        );
        let error = plan_named(ws(), &record, &SetRequest::Full, true)
            .expect_err("web offline needs network");
        assert_eq!(
            error,
            BackendError::OfflineRequired {
                set: "web".to_owned(),
            }
        );
        assert_eq!(
            check_named(ws(), &record, &SetRequest::Full, false).expect("web check"),
            CheckPlan::Unavailable,
        );
    }

    #[test]
    fn consumer_npm_install_record_rejects_selective_explicitly() {
        use super::super::config::DependencySet;
        use std::collections::BTreeMap;
        let mut options = BTreeMap::new();
        options.insert("mode".to_owned(), "install".to_owned());
        let record = DependencySet {
            name: "shop".to_owned(),
            backend: super::super::config::BackendKind::Npm,
            manifests: vec!["shop/package.json".to_owned()],
            locks: vec!["shop/pnpm-lock.yaml".to_owned()],
            scopes: vec!["shop".to_owned()],
            options,
        };
        assert_eq!(
            plan_named(ws(), &record, &SetRequest::Full, false).expect("shop full"),
            BackendPlan::Run {
                argv: vec![
                    "pnpm".to_owned(),
                    "--dir".to_owned(),
                    "shop".to_owned(),
                    "install".to_owned(),
                    "--lockfile-only".to_owned(),
                ],
                env: vec![],
            }
        );
        let error = plan_named(
            ws(),
            &record,
            &SetRequest::Packages(vec!["react".to_owned()]),
            false,
        )
        .expect_err("install selective stays unsupported");
        assert_eq!(
            error,
            BackendError::Unsupported {
                set: "shop".to_owned(),
                reason: "shop pins are exact in shop; use `dx update shop` for the set".to_owned(),
            }
        );
        let offline = plan_named(
            ws(),
            &record,
            &SetRequest::Packages(vec!["react".to_owned()]),
            true,
        )
        .expect_err("install selective stays unsupported offline");
        assert!(matches!(offline, BackendError::Unsupported { .. }));
    }

    #[test]
    fn non_npm_records_plan_explicitly_unsupported() {
        use super::super::config::{BackendKind, DependencySet};
        use std::collections::BTreeMap;
        for (backend, name) in [
            (BackendKind::Cargo, "vendor"),
            (BackendKind::Uv, "service"),
            (BackendKind::Go, "tool"),
            (BackendKind::Maven, "service"),
            (BackendKind::NuGet, "service"),
            (BackendKind::Ruby, "service"),
            (BackendKind::PowerShell, "service"),
        ] {
            let record = DependencySet {
                name: name.to_owned(),
                backend,
                manifests: vec!["service/manifest".to_owned()],
                locks: vec!["service/lock".to_owned()],
                scopes: vec!["service".to_owned()],
                options: BTreeMap::new(),
            };
            let error = plan_named(ws(), &record, &SetRequest::Full, false)
                .expect_err("unmigrated backend plans explicitly");
            assert!(
                matches!(error, BackendError::Unsupported { .. }),
                "{backend:?}"
            );
            assert!(error.to_string().contains(name));
            assert!(error.to_string().contains(backend.name()));
            let error = check_named(ws(), &record, &SetRequest::Full, false)
                .expect_err("unmigrated backend checks explicitly");
            assert!(
                matches!(error, BackendError::Unsupported { .. }),
                "{backend:?}"
            );
        }
    }

    #[test]
    fn bad_npm_mode_and_runner_fail_closed() {
        use super::super::config::DependencySet;
        use std::collections::BTreeMap;
        let mut options = BTreeMap::new();
        options.insert("mode".to_owned(), "wipe".to_owned());
        let record = DependencySet {
            name: "web".to_owned(),
            backend: super::super::config::BackendKind::Npm,
            manifests: vec!["apps/web/package.json".to_owned()],
            locks: vec!["apps/web/pnpm-lock.yaml".to_owned()],
            scopes: vec!["apps/web".to_owned()],
            options,
        };
        let error =
            plan_named(ws(), &record, &SetRequest::Full, false).expect_err("bad mode fails closed");
        assert!(error.to_string().contains("wipe"));
        let mut options = BTreeMap::new();
        options.insert("runner".to_owned(), "sudo".to_owned());
        let record = DependencySet { options, ..record };
        let error = plan_named(ws(), &record, &SetRequest::Full, false)
            .expect_err("bad runner fails closed");
        assert!(error.to_string().contains("sudo"));
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
                plan(ws(), set, &SetRequest::Full, true).expect_err("offline needs network");
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
        )
        .expect_err("npm selective offline needs network");
        assert!(
            matches!(selective, BackendError::OfflineRequired { .. }),
            "{selective:?}"
        );
        for set in [SetId::Go, SetId::Ruby, SetId::PowerShell] {
            assert_eq!(
                plan(ws(), set, &SetRequest::Full, true).expect("pinned noop offline"),
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
                plan(ws(), set, &packages, true).expect_err("unsupported stays unsupported");
            assert!(
                matches!(error, BackendError::Unsupported { .. }),
                "{set:?}: {error:?}"
            );
        }
    }
}
