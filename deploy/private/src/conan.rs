//! Conan publisher: recipe identity checks, local staging, `conan create` and `conan upload`.

use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, exit_text,
    placeholder_refusal, print_redacted, resolve_version, run_publish, scrubbed_command,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::RunError;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Why a conan deployment failed.
#[derive(Debug)]
pub enum ConanError {
    Paths(io::Error),
    Stage(StageError),
    TempDir(io::Error),
    Io(PathIoError),
    Run(RunError),
    NameMismatch {
        declared: String,
        target: String,
    },
    NoVersion,
    /// An already-prefixed refusal from the shared version, placeholder or owner checks.
    Refused(String),
    ConanFailed {
        step: String,
        status: String,
    },
    NoConan,
    Export(io::Error),
}

impl fmt::Display for ConanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paths(source) | Self::TempDir(source) | Self::Export(source) => {
                write!(f, "conan_deploy: {source}")
            }
            Self::Stage(source) => write!(f, "conan_deploy: {source}"),
            Self::Io(source) => write!(f, "conan_deploy: {source}"),
            Self::Run(source) => write!(f, "conan_deploy: {source}"),
            Self::NameMismatch { declared, target } => write!(
                f,
                "conan_deploy: conanfile.py declares name {declared}, target is {target}; refusing"
            ),
            Self::NoVersion => f.write_str(
                "conan_deploy: conanfile.py declares no plain version and BUILD sets none",
            ),
            Self::Refused(message) => f.write_str(message),
            Self::ConanFailed { step, status } => {
                write!(f, "conan_deploy: conan {step} failed with {status}")
            }
            Self::NoConan => f.write_str("conan_deploy: no conan executable"),
        }
    }
}

impl std::error::Error for ConanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Paths(source) | Self::TempDir(source) | Self::Export(source) => Some(source),
            Self::Stage(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Run(source) => Some(source),
            Self::NameMismatch { .. }
            | Self::NoVersion
            | Self::Refused(_)
            | Self::ConanFailed { .. }
            | Self::NoConan => None,
        }
    }
}

/// Owner approval gate for `conan_deploy`.
pub(crate) const CONAN_GATE: OwnerGate = OwnerGate {
    tool: "conan_deploy",
    action: "upload",
    credentials: &["CONAN_LOGIN_USERNAME", "CONAN_PASSWORD"],
    approved: "CONAN_PUBLISH_APPROVED",
};

/// What a Conan deployment pins besides the recipe files.
pub struct ConanLaunch<'a> {
    pub recipe_rlocs: &'a [String],
    pub deploy_name: &'a str,
    pub package: &'a str,
    pub version: &'a str,
    pub remote: &'a str,
    pub remote_url: &'a str,
    pub conan_rloc: &'a str,
}

/// Returns the string a recipe assigns to one class attribute.
fn recipe_string<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let (name, value) = line.split_once('=')?;
        if name.trim_end() != key {
            return None;
        }
        let value = value.trim();
        let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        value.strip_prefix(quote)?.strip_suffix(quote)
    })
}

/// Checks the recipe's declared name and returns the version to publish.
fn recipe_version(text: &str, package: &str, version: &str) -> Result<String, ConanError> {
    if let Some(name) = recipe_string(text, "name")
        && name != package
    {
        return Err(ConanError::NameMismatch {
            declared: name.to_owned(),
            target: package.to_owned(),
        });
    }
    match recipe_string(text, "version") {
        Some(declared) => resolve_version("conan_deploy", "conanfile.py", Some(declared), version)
            .map_err(ConanError::Refused),
        None if !version.is_empty() => Ok(version.to_owned()),
        None => Err(ConanError::NoVersion),
    }
}

/// Runs one conan command in an isolated CONAN_HOME.
fn conan_run(
    conan: &Path,
    home: &Path,
    dir: &Path,
    args: &[&str],
    secrets: &[(&str, &str)],
) -> Result<(), ConanError> {
    let home = home.to_string_lossy();
    let mut extra = vec![("CONAN_HOME", home.as_ref())];
    extra.extend_from_slice(secrets);
    let mut cmd = scrubbed_command(conan, &[PUBLISH_CHILD_ENV_KEEP], &extra);
    cmd.current_dir(dir).args(args);
    let out = run_publish(&mut cmd).map_err(ConanError::Run)?;
    let password = secrets
        .iter()
        .find(|(key, _)| *key == "CONAN_PASSWORD")
        .map_or("", |(_, value)| *value);
    print_redacted(&out, password);
    if out.success() {
        return Ok(());
    }
    Err(ConanError::ConanFailed {
        step: args.first().copied().unwrap_or_default().to_owned(),
        status: exit_text(&out),
    })
}

/// Creates the package from the staged recipe and uploads it to the remote.
fn conan_publish(
    conan: &Path,
    staging: &Path,
    spec: &ConanLaunch<'_>,
    version: &str,
    secrets: &[(&str, &str)],
) -> Result<(), ConanError> {
    let home = staging.join(".conan-home");
    let source = staging.join("recipe");
    let reference = format!("{}/{version}", spec.package);
    let steps: [&[&str]; 4] = [
        &["profile", "detect", "--force"],
        &["remote", "add", "--force", spec.remote, spec.remote_url],
        &["create", ".", "--name", spec.package, "--version", version],
        &["upload", &reference, "-r", spec.remote, "--confirm"],
    ];
    for args in steps {
        let step_secrets = if args.first() == Some(&"upload") {
            secrets
        } else {
            &[]
        };
        conan_run(conan, &home, &source, args, step_secrets)?;
    }
    Ok(())
}

/// Runs the Conan deploy tool.
pub fn conan_main(spec: &ConanLaunch<'_>, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "conan_deploy",
        "the recipe is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let mut files = Vec::new();
    for rloc in spec.recipe_rlocs.iter().filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        files.push(path);
    }
    let staging = match tempfile_stage_dir("conan-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("{}", ConanError::TempDir(error));
            return 1;
        }
    };
    let result = conan_deploy_checked(&run, spec, &files, &staging);
    dx_atomic_fs::discard_dir(&staging);
    match result {
        Ok(message) => {
            println!("{message}");
            0
        }
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

fn conan_deploy_checked(
    run: &DeployRun,
    spec: &ConanLaunch<'_>,
    files: &[PathBuf],
    staging: &Path,
) -> Result<String, ConanError> {
    let mapped =
        package_relative_paths(files, "conanfile.py", "conan_deploy").map_err(ConanError::Paths)?;
    let source = staging.join("recipe");
    stage_mapped(&mapped, &source).map_err(ConanError::Stage)?;
    let text = path_io::read_to_string(&source.join("conanfile.py")).map_err(ConanError::Io)?;
    let version = recipe_version(&text, spec.package, spec.version)?;
    let package = spec.package;
    if OwnerGate::requested(env_or_empty) {
        if let Some(error) = placeholder_refusal("conan_deploy", &version) {
            return Err(ConanError::Refused(error));
        }
        let secrets = CONAN_GATE
            .authorize(env_or_empty)
            .map_err(ConanError::Refused)?;
        let conan = run.resolve(spec.conan_rloc).ok_or(ConanError::NoConan)?;
        conan_publish(
            &conan,
            staging,
            spec,
            &version,
            &[
                ("CONAN_LOGIN_USERNAME", secrets.get("CONAN_LOGIN_USERNAME")),
                ("CONAN_PASSWORD", secrets.get("CONAN_PASSWORD")),
            ],
        )?;
        return Ok(format!(
            "conan_deploy: uploaded {package}/{version} to {}",
            spec.remote
        ));
    }
    let dest = run.outdir().join(format!("{}-recipe", spec.deploy_name));
    for (_, rel) in &mapped {
        let to = dest.join(rel);
        let staged = to
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| copy_verified(&source.join(rel), &to).map(|_| ()));
        staged.map_err(ConanError::Export)?;
    }
    Ok(format!(
        "conan_deploy: staged {package}/{version} (build_mode: {})\n  recipe: {}",
        deploy_build_mode(),
        dest.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    const RECIPE: &str = "from conan import ConanFile\n\nclass Demo(ConanFile):\n    name = \"demo\"\n    version = '1.2.0'\n";

    #[test]
    fn recipe_must_match_target() {
        assert_eq!(
            recipe_version(RECIPE, "demo", "").expect("declared version"),
            "1.2.0"
        );
        assert_eq!(
            recipe_version(RECIPE, "demo", "1.2.0").expect("matching version"),
            "1.2.0"
        );
        let error = recipe_version(RECIPE, "other", "").expect_err("name");
        assert!(
            matches!(error, ConanError::NameMismatch { .. }),
            "{error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("declares name demo, target is other"),
            "{error}"
        );
        let error = recipe_version(RECIPE, "demo", "2.0.0").expect_err("version");
        assert!(matches!(error, ConanError::Refused(_)), "{error:?}");
        assert!(
            error
                .to_string()
                .contains("declares version 1.2.0, BUILD declares 2.0.0"),
            "{error}"
        );
        let bare = "class Demo(ConanFile):\n    pass\n";
        assert_eq!(
            recipe_version(bare, "demo", "3.0.0").expect("BUILD version"),
            "3.0.0"
        );
        assert!(matches!(
            recipe_version(bare, "demo", ""),
            Err(ConanError::NoVersion)
        ));
    }

    #[test]
    fn default_run_stages_the_recipe_and_live_run_needs_approval() {
        if env_test_in_child(
            "conan::tests::default_run_stages_the_recipe_and_live_run_needs_approval",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(runfiles.join("src")).expect("runfiles");
        std::fs::write(runfiles.join("conanfile.py"), RECIPE).expect("recipe");
        std::fs::write(runfiles.join("src/demo.c"), "int x;\n").expect("source");
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles_dir = runfiles.to_string_lossy().into_owned();
        let rlocs = vec!["conanfile.py".to_owned(), "src/demo.c".to_owned()];
        let spec = ConanLaunch {
            recipe_rlocs: &rlocs,
            deploy_name: "demo",
            package: "demo",
            version: "1.2.0",
            remote: "dx",
            remote_url: "https://conan.invalid",
            conan_rloc: "",
        };
        let launch = |live: Option<&str>| {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles_dir.as_str())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("DX_DEPLOY_LIVE", live),
                    ("CONAN_PUBLISH_APPROVED", None),
                    ("CONAN_LOGIN_USERNAME", None),
                    ("CONAN_PASSWORD", None),
                ],
                || conan_main(&spec, &argv),
            )
        };
        assert_eq!(launch(None), 0);
        assert!(outdir.join("demo-recipe/conanfile.py").is_file());
        assert!(outdir.join("demo-recipe/src/demo.c").is_file());
        assert_eq!(launch(Some("1")), 1);
    }

    #[cfg(unix)]
    #[test]
    fn credentials_reach_only_the_upload_step() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = scratch_dir();
        let log = scratch.path().join("log");
        let conan = scratch.path().join("conan");
        std::fs::write(
            &conan,
            format!(
                "#!/bin/sh\necho \"$1:${{CONAN_PASSWORD:-none}}\" >> {}\n",
                log.display()
            ),
        )
        .expect("script");
        std::fs::set_permissions(&conan, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let source = scratch.path().join("staging/recipe");
        std::fs::create_dir_all(&source).expect("recipe dir");
        let spec = ConanLaunch {
            recipe_rlocs: &[],
            deploy_name: "demo",
            package: "demo",
            version: "1.2.0",
            remote: "dx",
            remote_url: "https://conan.invalid",
            conan_rloc: "",
        };
        let secrets = [
            ("CONAN_LOGIN_USERNAME", "user"),
            ("CONAN_PASSWORD", "fake-token"),
        ];
        conan_publish(
            &conan,
            &scratch.path().join("staging"),
            &spec,
            "1.2.0",
            &secrets,
        )
        .expect("publish");
        let seen = std::fs::read_to_string(&log).expect("log");
        assert_eq!(
            seen.lines().collect::<Vec<_>>(),
            [
                "profile:none",
                "remote:none",
                "create:none",
                "upload:fake-token"
            ]
        );
    }
}
