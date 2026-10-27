#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod archive;
mod cloudflare;
mod config;
mod crates;
mod github;
pub mod http;
mod maven;
mod npm;
mod nuget;
mod oci;
mod octopus;
mod promotion;
mod pypi;
mod registry;
mod run;
mod stage;
#[cfg(test)]
pub(crate) mod test_support;

pub use archive::*;
pub use cloudflare::*;
pub use config::*;
pub use crates::*;
pub use github::*;
pub use maven::*;
pub use npm::*;
pub use nuget::*;
pub use oci::*;
pub use octopus::*;
pub use promotion::*;
pub use pypi::*;
pub use run::{deploy_profile, DeployRun};
pub use stage::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use std::io;
    use std::path::Path;

    #[test]
    fn command_formatters_cover_optional_segments() {
        assert_eq!(
            promotion_rollback_command("demo", "1.0.1"),
            "rollback demo to 1.0.1 (restore the pinned artifact for that version)"
        );
        assert_eq!(
            github_release_command("v1", &[]),
            "gh release create v1 --draft --verify-tag"
        );
    }

    #[test]
    fn launchers_reject_extra_arguments() {
        let argv = vec!["prog".to_owned(), "out".to_owned(), "extra".to_owned()];
        let spec = OctopusLaunch {
            package_rloc: "demo.tar.gz",
            deploy_name: "demo",
            project: "demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to_raw: "",
            space_default: "",
            url_default: "https://octopus.example.invalid",
        };
        assert_eq!(archive_main("a", "b", "c", &argv), 1);
        assert_eq!(pypi_main("a", "b", "c", "https://pypi.invalid", &argv), 1);
        assert_eq!(
            nuget_main("a", "b", "", "", "https://nuget.invalid", &argv),
            1
        );
        assert_eq!(crates_main("a", "b", "1.0.0", &argv), 1);
        assert_eq!(github_main(&["a".to_owned()], "b", "v1.0.0", &argv), 1);
        assert_eq!(
            maven_main(
                "a",
                "b",
                "com.example",
                "demo",
                "1.0.0",
                "https://maven.invalid",
                &argv
            ),
            1
        );
        assert_eq!(
            oci_main(
                "a",
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
                &argv
            ),
            1
        );
        assert_eq!(octopus_main(&spec, &argv), 1);
        assert_eq!(
            promotion_main("a", "b", "staging", "production", "1.0.0", &argv),
            1
        );
        assert_eq!(npm_deploy_main(&argv, "a", "b"), 1);
    }

    #[test]
    fn launchers_report_missing_pinned_inputs() {
        if env_test_in_child("tests::launchers_report_missing_pinned_inputs") {
            return;
        }
        let outdir = tempfile_stage_dir("dx-deploy-missing-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(outdir.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                let missing = "dx-absent-pinned-input-xyz";
                assert_eq!(archive_main(missing, missing, missing, &argv), 1);
                assert_eq!(
                    pypi_main(missing, missing, "demo", "https://pypi.invalid", &argv),
                    1
                );
                assert_eq!(
                    nuget_main(missing, "demo", "", "", "https://nuget.invalid", &argv),
                    1
                );
                assert_eq!(crates_main(missing, "demo", "1.0.0", &argv), 1);
                assert_eq!(
                    github_main(&[missing.to_owned()], "demo", "v1.0.0", &argv),
                    1
                );
                assert_eq!(
                    maven_main(
                        missing,
                        missing,
                        "com.example",
                        "demo",
                        "1.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    1
                );
                assert_eq!(
                    oci_main(
                        missing,
                        "registry.invalid",
                        "demo/app",
                        "1.0.0",
                        "linux/amd64",
                        &argv
                    ),
                    1
                );
                let spec = OctopusLaunch {
                    package_rloc: missing,
                    deploy_name: "demo",
                    project: "demo",
                    channel: "Default",
                    version: "1.0.0",
                    deploy_to_raw: "",
                    space_default: "",
                    url_default: "https://octopus.example.invalid",
                };
                assert_eq!(octopus_main(&spec, &argv), 1);
                assert_eq!(
                    promotion_main(missing, "demo", "staging", "production", "1.0.0", &argv),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn owner_gates_accept_only_exact_approval() {
        let gates = [
            &PYPI_GATE,
            &NUGET_GATE,
            &CRATES_GATE,
            &GITHUB_GATE,
            &maven::MAVEN_GATE,
            &OCI_GATE,
            &OCTOPUS_GATE,
            &PROMOTION_GATE,
            &NPM_GATE,
        ];
        let tools: std::collections::BTreeSet<&str> = gates.iter().map(|gate| gate.tool).collect();
        let approvals: std::collections::BTreeSet<&str> =
            gates.iter().map(|gate| gate.approved).collect();
        assert_eq!(tools.len(), gates.len());
        assert_eq!(approvals.len(), gates.len());
        for gate in gates {
            let base: std::collections::HashMap<&str, &str> =
                gate.credentials.iter().map(|key| (*key, "x")).collect();
            let run = |env: &std::collections::HashMap<&str, &str>| {
                gate.refusal(|key| env.get(key).copied().unwrap_or_default().to_owned())
            };
            let approval_text = format!("{}=1 (owner approval)", gate.approved);
            let message = run(&base).unwrap_or_default();
            assert!(
                message.starts_with(&format!("{}: ", gate.tool)),
                "{message}"
            );
            assert!(message.contains(&approval_text), "{message}");
            for value in ["", "0", "true", "yes", " 1", "1 "] {
                let mut env = base.clone();
                env.insert(gate.approved, value);
                let message = run(&env).unwrap_or_default();
                assert!(
                    message.starts_with(&format!("{}: ", gate.tool)),
                    "{value:?}: {message}"
                );
                assert!(message.contains(&approval_text), "{value:?}: {message}");
            }
            let mut env = base.clone();
            env.insert(gate.approved, "1");
            assert_eq!(run(&env), None, "{}", gate.tool);
            for credential in gate.credentials {
                let mut env = env.clone();
                env.insert(credential, "");
                let message = run(&env).unwrap_or_default();
                assert!(
                    message.starts_with(&format!("{}: ", gate.tool)),
                    "{message}"
                );
                assert!(message.contains(credential), "{message}");
                assert!(
                    message.contains(&format!("{}=1", gate.approved)),
                    "{message}"
                );
            }
            let lookup = |env: &std::collections::HashMap<&str, &str>, key: &str| {
                env.get(key).copied().unwrap_or_default().to_owned()
            };
            for env in [base.clone(), env.clone()] {
                let refusal = run(&env);
                match gate.authorize(|key| lookup(&env, key)) {
                    Ok(secrets) => {
                        assert_eq!(refusal, None, "{}", gate.tool);
                        for credential in gate.credentials {
                            assert_eq!(secrets.get(credential), "x");
                        }
                    }
                    Err(error) => assert_eq!(Some(error), refusal, "{}", gate.tool),
                }
            }
            for value in ["", "0", "true", "yes", " 1", "1 "] {
                let mut env = base.clone();
                env.insert(gate.live, value);
                assert!(!gate.requested(|key| lookup(&env, key)), "{value:?}");
            }
            let mut env = base.clone();
            env.insert(gate.live, "1");
            assert!(gate.requested(|key| lookup(&env, key)), "{}", gate.tool);
        }
        let lives: std::collections::BTreeSet<&str> = gates.iter().map(|gate| gate.live).collect();
        assert_eq!(lives.len(), gates.len());
    }

    #[test]
    fn launchers_refuse_live_publish_without_owner_approval() {
        if env_test_in_child("tests::launchers_refuse_live_publish_without_owner_approval") {
            return;
        }
        let outdir = tempfile_stage_dir("dx-deploy-live-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles = outdir.join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        assert!(runfiles.is_dir());
        let missing = "dx-absent-pinned-input-xyz";
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(outdir.join("runfiles").to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("PYPI_PUBLISH_LIVE", Some("1")),
                ("PYPI_API_TOKEN", Some("")),
                ("PYPI_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    pypi_main(missing, missing, "demo", "https://pypi.invalid", &argv),
                    1,
                    "pypi needs a resolvable wheel before the token gate"
                );
            },
        );
        with_env(
            &[
                ("NUGET_PUBLISH_LIVE", Some("1")),
                ("NUGET_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    nuget_main(missing, "demo", "", "", "https://nuget.invalid", &argv),
                    1
                );
            },
        );
        with_env(
            &[
                ("GH_RELEASE_LIVE", Some("1")),
                ("GH_RELEASE_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    github_main(&[missing.to_owned()], "demo", "v0.0.0-dryrun", &argv),
                    1
                );
            },
        );
        with_env(
            &[
                ("MAVEN_PUBLISH_LIVE", Some("1")),
                ("MAVEN_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    maven_main(
                        missing,
                        missing,
                        "com.example",
                        "demo",
                        "0.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    1
                );
            },
        );
        with_env(
            &[
                ("OCI_PUBLISH_LIVE", Some("1")),
                ("OCI_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    oci_main(
                        missing,
                        "registry.invalid",
                        "demo/app",
                        "0.0.0",
                        "linux/amd64",
                        &argv
                    ),
                    1
                );
            },
        );
        with_env(
            &[
                ("CRATES_PUBLISH_LIVE", Some("1")),
                ("CRATES_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(crates_main(missing, "demo", "0.0.0", &argv), 1);
            },
        );
        let crates_runfiles = outdir.join("crates_runfiles");
        stage_runfiles_inputs(&crates_runfiles);
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(crates_runfiles.to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("CRATES_PUBLISH_LIVE", Some("1")),
                ("CRATES_ALLOW_DIRTY", Some("1")),
                ("CRATES_PUBLISH_APPROVED", None),
                ("CARGO_REGISTRY_TOKEN", None),
            ],
            || {
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", &argv),
                    1,
                    "CRATES_ALLOW_DIRTY=1 must refuse a live publish"
                );
            },
        );
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(outdir.join("runfiles").to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("NPM_PUBLISH_LIVE", Some("1")),
                ("NPM_PUBLISH_APPROVED", Some("1")),
                ("NPM_TOKEN", None),
                ("NPM_PUBLISH_DRY_RUN", None),
            ],
            || {
                assert_eq!(npm_deploy_main(&argv, missing, missing), 1);
            },
        );
        with_env(
            &[
                ("PROMOTION_LIVE", Some("1")),
                ("PROMOTION_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    promotion_main(missing, "demo", "staging", "production", "0.0.0", &argv),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn archive_and_promotion_launchers_stage_locally() {
        if env_test_in_child("tests::archive_and_promotion_launchers_stage_locally") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let app = runfiles.join("app_name.txt");
        std::fs::write(&app, b"demo_app\n").expect("write app");
        let tarball = runfiles.join("release_demo.tar.gz");
        let tarball_bytes = archive_bytes(b"payload", "release_demo.tar.gz", false).expect("tar");
        std::fs::write(&tarball, &tarball_bytes).expect("write tarball");
        let digest = dx_digest::sha256_file_hex(&tarball).expect("digest");
        let checksum = runfiles.join("release_demo.tar.gz.sha256");
        std::fs::write(&checksum, format!("{digest}  release_demo.tar.gz\n")).expect("write sum");

        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
            ],
            || {
                let outdir = tempfile_stage_dir("dx-deploy-archive-").expect("outdir");
                let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
                assert_eq!(
                    archive_main(
                        "app_name.txt",
                        "release_demo.tar.gz",
                        "release_demo.tar.gz.sha256",
                        &argv
                    ),
                    0
                );
                assert!(outdir.join("release_demo.tar.gz").is_file());
                std::fs::remove_dir_all(&outdir).expect("cleanup");

                let promotion_out = tempfile_stage_dir("dx-deploy-promotion-").expect("outdir");
                let promotion_argv = vec![
                    "prog".to_owned(),
                    promotion_out.to_string_lossy().into_owned(),
                ];
                assert_eq!(
                    promotion_main(
                        "release_demo.tar.gz",
                        "promotion_demo",
                        "staging",
                        "production",
                        "1.2.3",
                        &promotion_argv
                    ),
                    0
                );
                assert!(promotion_out.join("promotion_demo-promotion").is_dir());
                std::fs::remove_dir_all(&promotion_out).expect("cleanup");
            },
        );
    }

    fn stage_runfiles_inputs(root: &Path) {
        std::fs::create_dir_all(root).expect("runfiles");
        let maven_pom = crate::maven::tests::central_pom("com.example", "maven_demo", "1.0.0");
        for (name, body) in [
            ("pypi_demo-0.0.0-py3-none-any.whl", &b"wheel"[..]),
            ("pypi_demo-0.0.0.tar.gz", &b"sdist"[..]),
            ("maven_demo-0.0.0.jar", &b"jar"[..]),
            ("maven_demo-0.0.0.pom", maven_pom.as_bytes()),
            ("release_demo.tar.gz", &b"package"[..]),
            (
                "Cargo.toml",
                &b"[package]\nname = \"crates_demo\"\nversion = \"1.0.0\"\n"[..],
            ),
            (
                "lib.rs",
                &b"pub fn hello() -> &'static str { \"hello\" }\n"[..],
            ),
        ] {
            std::fs::write(root.join(name), body).expect("write runfile input");
        }
        let layer = root.join("oci_demo_layer.tar");
        std::fs::write(&layer, oci_test_layer(true)).expect("write oci layer");
        archive_image(
            &[layer.as_path()],
            "linux/amd64",
            &["/app/hello"],
            &root.join("oci_demo.tar"),
        )
        .expect("write oci image");
        write_nupkg(
            &root.join("nuget_demo.0.0.0.nupkg"),
            &[("nuget_demo.nuspec", &nuspec("nuget_demo", "1.0.0"))],
        );
    }

    #[test]
    fn launchers_stage_locally_without_publishing() {
        if env_test_in_child("tests::launchers_stage_locally_without_publishing") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        stage_runfiles_inputs(&runfiles);
        let outdir = tempfile_stage_dir("dx-deploy-stage-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("PYPI_PUBLISH_LIVE", None),
                ("NUGET_PUBLISH_LIVE", None),
                ("CRATES_PUBLISH_LIVE", None),
                ("GH_RELEASE_LIVE", None),
                ("GH_RELEASE_DRY_RUN", Some("1")),
                ("MAVEN_PUBLISH_LIVE", None),
                ("OCI_PUBLISH_LIVE", None),
                ("OCI_PUBLISH_DRY_RUN", Some("1")),
                ("OCTOPUS_PUBLISH_LIVE", None),
            ],
            || {
                assert_eq!(
                    pypi_main(
                        "pypi_demo-0.0.0-py3-none-any.whl",
                        "pypi_demo-0.0.0.tar.gz",
                        "pypi_demo",
                        "https://pypi.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    nuget_main(
                        "nuget_demo.0.0.0.nupkg",
                        "nuget_demo",
                        "",
                        "1.0.0",
                        "https://nuget.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", &argv),
                    0
                );
                assert_eq!(
                    github_main(
                        &["release_demo.tar.gz".to_owned()],
                        "github_demo",
                        "v1.0.0",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    maven_main(
                        "maven_demo-0.0.0.jar",
                        "maven_demo-0.0.0.pom",
                        "com.example",
                        "maven_demo",
                        "1.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    oci_main(
                        "oci_demo.tar",
                        "registry.invalid",
                        "demo/app",
                        "1.0.0",
                        "linux/amd64",
                        &argv
                    ),
                    0
                );
                let spec = OctopusLaunch {
                    package_rloc: "release_demo.tar.gz",
                    deploy_name: "octopus_demo",
                    project: "octopus_demo",
                    channel: "Default",
                    version: "1.0.0",
                    deploy_to_raw: "Production,Staging",
                    space_default: "Spaces",
                    url_default: "https://octopus.example.invalid",
                };
                assert_eq!(octopus_main(&spec, &argv), 0);
            },
        );
        assert!(outdir.join("pypi_demo-wheelhouse").is_dir());
        assert!(outdir.join("maven_demo-repo").is_dir());
        assert!(outdir.join("github_demo-release").is_dir());
        assert!(outdir.join("crates_demo-vendor").is_dir());
        assert!(outdir.join("nuget_demo-feed").is_dir());
        assert!(outdir.join("octopus_demo-drop").is_dir());
        assert!(outdir.join("app-oci-layout").is_dir());
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn launcher_relative_outdir_resolves_against_invocation_dir() {
        if env_test_in_child("tests::launcher_relative_outdir_resolves_against_invocation_dir") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        stage_runfiles_inputs(&runfiles);
        let cwd = scratch.path().join("cwd");
        let invocation = scratch.path().join("inv");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&invocation).expect("inv");
        std::env::set_current_dir(&cwd).expect("chdir");
        let argv = vec!["prog".to_owned(), "dist".to_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                (
                    "BUILD_WORKING_DIRECTORY",
                    Some(invocation.to_string_lossy().as_ref()),
                ),
                ("GH_RELEASE_LIVE", None),
                ("GH_RELEASE_DRY_RUN", Some("1")),
            ],
            || {
                assert_eq!(
                    github_main(&["release_demo.tar.gz".to_owned()], "demo", "v1.0.0", &argv),
                    0
                );
            },
        );
        assert!(invocation.join("dist").join("demo-release").is_dir());
        assert!(!cwd.join("dist").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let bin = scratch.path().join("bin");
            let workspace = scratch.path().join("ws");
            std::fs::create_dir_all(&bin).expect("bin");
            std::fs::create_dir_all(&workspace).expect("ws");
            let log = scratch.path().join("gh.log");
            let env_log = scratch.path().join("gh.env");
            let script = bin.join("gh");
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\npwd -P > '{}'\nprintf '%s\\n' \"$@\" >> '{}'\nenv > '{}'\n",
                    log.display(),
                    log.display(),
                    env_log.display()
                ),
            )
            .expect("script");
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
            std::env::set_current_dir(scratch.path()).expect("chdir");
            let path = format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            );
            with_env(
                &[
                    ("PATH", Some(path.as_str())),
                    ("RUNFILES_DIR", Some("runfiles")),
                    ("RUNFILES_MANIFEST_FILE", None),
                    (
                        "BUILD_WORKING_DIRECTORY",
                        Some(invocation.to_string_lossy().as_ref()),
                    ),
                    (
                        "BUILD_WORKSPACE_DIRECTORY",
                        Some(workspace.to_string_lossy().as_ref()),
                    ),
                    ("GH_REPO", None),
                    ("GH_RELEASE_LIVE", Some("1")),
                    ("GH_RELEASE_APPROVED", Some("1")),
                    ("GH_TOKEN", Some("gh-sekret")),
                    ("DX_LEAK_PROBE", Some("1")),
                ],
                || {
                    assert_eq!(
                        github_main(&["release_demo.tar.gz".to_owned()], "demo", "v1.0.0", &argv),
                        0
                    );
                },
            );
            let logged = std::fs::read_to_string(&log).expect("gh log");
            let lines: Vec<&str> = logged.lines().collect();
            let workspace_real = std::fs::canonicalize(&workspace).expect("canonical ws");
            assert_eq!(lines[0], workspace_real.to_string_lossy(), "{logged}");
            assert_eq!(&lines[1..4], ["release", "create", "v1.0.0"], "{logged}");
            let asset = Path::new(lines[4]);
            assert!(asset.is_absolute(), "{logged}");
            assert_eq!(
                std::fs::canonicalize(asset).expect("canonical asset"),
                std::fs::canonicalize(runfiles.join("release_demo.tar.gz")).expect("canonical"),
                "{logged}"
            );
            assert_eq!(&lines[5..], ["--draft", "--verify-tag"], "{logged}");
            let env = std::fs::read_to_string(&env_log).expect("gh env");
            assert!(env.lines().any(|line| line == "GH_TOKEN=gh-sekret"));
            assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
            assert!(!logged.contains("gh-sekret"), "{logged}");
        }
    }

    #[test]
    fn build_helpers_reject_unusable_inputs() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let image = scratch.path().join("oci_demo.bin");
        std::fs::write(&image, b"image").expect("write image");
        let error = oci_build_layout(
            &image,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.0.0",
            "linux/amd64",
        )
        .expect_err("non-tar image");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        let package = scratch.path().join("release_demo.zip");
        std::fs::write(&package, b"package").expect("write package");
        let error = octopus_build_drop(
            &package,
            &outdir,
            &OctopusDrop {
                deploy_name: "octopus_demo",
                project: "octopus_demo",
                channel: "Default",
                version: "1.0.0",
                deploy_to: &[],
                space: "",
                server: "https://octopus.example.invalid",
            },
        )
        .expect_err("non-tar.gz package");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    /// Pins the crate-root entry points the generated launchers call, so the
    /// module (and later crate) split cannot silently change the facade.
    #[test]
    fn crate_root_keeps_launcher_entry_points() {
        type Main3 = fn(&str, &str, &str, &[String]) -> i32;
        type Main4 = fn(&str, &str, &str, &str, &[String]) -> i32;
        type Main5 = fn(&str, &str, &str, &str, &str, &[String]) -> i32;
        type Main6 = fn(&str, &str, &str, &str, &str, &str, &[String]) -> i32;
        let archive: Main3 = crate::archive_main;
        let pypi: Main4 = crate::pypi_main;
        let nuget: Main5 = crate::nuget_main;
        let crates: Main3 = crate::crates_main;
        let github: fn(&[String], &str, &str, &[String]) -> i32 = crate::github_main;
        let maven: Main6 = crate::maven_main;
        let oci: Main5 = crate::oci_main;
        let promotion: Main5 = crate::promotion_main;
        let octopus: fn(&crate::OctopusLaunch<'_>, &[String]) -> i32 = crate::octopus_main;
        let npm_deploy: fn(&[String], &str, &str) -> i32 = crate::npm_deploy_main;
        let npm_pack: fn(&[String]) -> i32 = crate::npm_pack_main;
        let pages: fn(&crate::PagesLaunch<'_>, &[String]) -> i32 = crate::cloudflare_pages_main;
        let _ = (
            archive, pypi, nuget, crates, github, maven, oci, promotion, octopus, npm_deploy,
            npm_pack, pages,
        );
        let _ = crate::OctopusLaunch {
            package_rloc: "",
            deploy_name: "",
            project: "",
            channel: "",
            version: "",
            deploy_to_raw: "",
            space_default: "",
            url_default: "",
        };
        let _ = crate::PagesLaunch {
            deploy_name: "",
            account_id: "",
            project: "",
            branch: "",
            production_branch: "",
            site_rloc: "",
            wrangler_rloc: "",
        };
    }
}
