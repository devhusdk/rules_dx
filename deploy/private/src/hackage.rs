//! Hackage publisher: sdist identity checks, local staging and the cabal upload.

use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, exit_text,
    pages_redact, placeholder_refusal, print_redacted, run_publish, scrubbed_command,
};
use crate::stage::*;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Owner approval gate for hackage_deploy.
pub(crate) const HACKAGE_GATE: OwnerGate = OwnerGate {
    tool: "hackage_deploy",
    action: "upload",
    credentials: &["HACKAGE_API_TOKEN"],
    approved: "HACKAGE_PUBLISH_APPROVED",
};

/// Extra env names `cabal` needs to find its config and package caches.
const CABAL_CHILD_ENV_EXTRA: &[&str] = &["CABAL_DIR", "XDG_CONFIG_HOME", "XDG_CACHE_HOME"];

/// Returns the value of one top-level field in a .cabal file.
fn cabal_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter(|line| !line.starts_with(char::is_whitespace))
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim().eq_ignore_ascii_case(key).then(|| value.trim())
        })
}

/// Why an sdist does not carry the declared package and version.
#[derive(Debug)]
pub enum HackageError {
    NotTarGz {
        file_name: String,
    },
    BadFileName {
        file_name: String,
    },
    Open {
        path: PathBuf,
        source: io::Error,
    },
    ReadCabal {
        member: String,
        source: io::Error,
    },
    InvalidSdist {
        file_name: String,
        source: io::Error,
    },
    CabalMismatch {
        member: String,
        name: String,
        declared: String,
        package: String,
        version: String,
    },
    NoCabal {
        file_name: String,
        member: String,
    },
    PackageMismatch {
        actual: String,
        expected: String,
    },
    VersionMismatch {
        actual: String,
        expected: String,
    },
}

impl fmt::Display for HackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTarGz { file_name } => write!(f, "{file_name} is not a .tar.gz sdist"),
            Self::BadFileName { file_name } => {
                write!(f, "{file_name} is not named <package>-<version>.tar.gz")
            }
            Self::Open { path, source } => write!(f, "cannot open {}: {source}", path.display()),
            Self::ReadCabal { member, source } => write!(f, "cannot read {member}: {source}"),
            Self::InvalidSdist { file_name, source } => {
                write!(f, "{file_name} is not a valid sdist: {source}")
            }
            Self::CabalMismatch {
                member,
                name,
                declared,
                package,
                version,
            } => write!(
                f,
                "{member} declares {name} {declared}, but the sdist is named {package} {version}"
            ),
            Self::NoCabal { file_name, member } => write!(f, "{file_name} has no {member}"),
            Self::PackageMismatch { actual, expected } => write!(
                f,
                "package mismatch: sdist declares {actual}, BUILD declares {expected}"
            ),
            Self::VersionMismatch { actual, expected } => write!(
                f,
                "version mismatch: .cabal declares {actual}, BUILD declares {expected}"
            ),
        }
    }
}

impl std::error::Error for HackageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::ReadCabal { source, .. }
            | Self::InvalidSdist { source, .. } => Some(source),
            Self::NotTarGz { .. }
            | Self::BadFileName { .. }
            | Self::CabalMismatch { .. }
            | Self::NoCabal { .. }
            | Self::PackageMismatch { .. }
            | Self::VersionMismatch { .. } => None,
        }
    }
}

/// Reads the package name and version the sdist's .cabal file declares.
///
/// The sdist must be named `<package>-<version>.tar.gz` and hold
/// `<package>-<version>/<package>.cabal`, which must agree with the file name.
pub fn hackage_read_identity(sdist: &Path) -> Result<(String, String), HackageError> {
    let file_name = sdist
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".tar.gz")
        .ok_or_else(|| HackageError::NotTarGz {
            file_name: file_name.to_owned(),
        })?;
    let (package, version) = stem
        .rsplit_once('-')
        .filter(|(package, version)| !package.is_empty() && !version.is_empty())
        .ok_or_else(|| HackageError::BadFileName {
            file_name: file_name.to_owned(),
        })?;
    let member = format!("{stem}/{package}.cabal");
    let open = std::fs::File::open(sdist).map_err(|source| HackageError::Open {
        path: sdist.to_path_buf(),
        source,
    })?;
    let mut text: Option<io::Result<String>> = None;
    dx_archive::visit(
        open,
        dx_archive::Format::TarGz,
        crate::archive::LISTING_LIMITS,
        |path, _, body| {
            if text.is_some() || path != member {
                return Ok(());
            }
            let mut read = String::new();
            text = Some(body.take(1 << 20).read_to_string(&mut read).map(|_| read));
            Ok(())
        },
    )
    .map_err(|source| HackageError::InvalidSdist {
        file_name: file_name.to_owned(),
        source,
    })?;
    let Some(text) = text else {
        return Err(HackageError::NoCabal {
            file_name: file_name.to_owned(),
            member,
        });
    };
    let text = text.map_err(|source| HackageError::ReadCabal {
        member: member.clone(),
        source,
    })?;
    let name = cabal_field(&text, "name").unwrap_or_default();
    let declared = cabal_field(&text, "version").unwrap_or_default();
    if name != package || declared != version {
        return Err(HackageError::CabalMismatch {
            member,
            name: name.to_owned(),
            declared: declared.to_owned(),
            package: package.to_owned(),
            version: version.to_owned(),
        });
    }
    Ok((name.to_owned(), declared.to_owned()))
}

/// Checks the sdist identity against the declared package and version.
pub fn hackage_check_identity(
    actual: &(String, String),
    expected_package: &str,
    expected_version: &str,
) -> Result<(), HackageError> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(HackageError::PackageMismatch {
            actual: actual.0.clone(),
            expected: expected_package.to_owned(),
        });
    }
    if !expected_version.is_empty() && actual.1 != expected_version {
        return Err(HackageError::VersionMismatch {
            actual: actual.1.clone(),
            expected: expected_version.to_owned(),
        });
    }
    Ok(())
}

/// Builds the `cabal upload --publish` launch reading its token from `config`.
fn hackage_upload_command(cabal: &Path, sdist: &Path, config: &Path) -> std::process::Command {
    let config = config.to_string_lossy();
    let mut cmd = scrubbed_command(
        cabal,
        &[PUBLISH_CHILD_ENV_KEEP, CABAL_CHILD_ENV_EXTRA],
        &[("CABAL_CONFIG", config.as_ref())],
    );
    cmd.arg("upload").arg("--publish").arg(sdist);
    cmd
}

/// Writes the private cabal config that carries the token.
fn write_cabal_config(dir: &PrivateConfigDir, token: &str) -> io::Result<std::path::PathBuf> {
    dir.write(
        "config",
        &format!(
            "hackage-token: {token}\nrepository hackage.haskell.org\n  url: https://hackage.haskell.org/\n"
        ),
    )
}

/// Uploads the sdist and publishes it with `cabal upload --publish`.
fn hackage_live_upload(
    cabal: &Path,
    sdist: &Path,
    token: &str,
    package: &str,
    version: &str,
) -> i32 {
    if let Some(refusal) = plain_token_refusal("hackage_deploy", token, &[]) {
        eprintln!("{refusal}");
        return 1;
    }
    let dir = match PrivateConfigDir::create("dx-hackage-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("hackage_deploy: cannot create the private config dir: {error}");
            return 1;
        }
    };
    let config = match write_cabal_config(&dir, token) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("hackage_deploy: cannot write the private config: {error}");
            return 1;
        }
    };
    let mut cmd = hackage_upload_command(cabal, sdist, &config);
    match run_publish(&mut cmd) {
        Ok(out) if out.success() => {
            print_redacted(&out, token);
            println!("hackage_deploy: uploaded {package} {version}");
            0
        }
        Ok(out) => {
            print_redacted(&out, token);
            eprintln!("hackage_deploy: cabal failed with {}", exit_text(&out));
            1
        }
        Err(error) => {
            eprintln!(
                "hackage_deploy: {}",
                pages_redact(&error.to_string(), token)
            );
            1
        }
    }
}

/// Runs the Hackage deploy tool.
pub fn hackage_main(
    sdist_rloc: &str,
    target_name: &str,
    expected_package: &str,
    expected_version: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "hackage_deploy",
        "the upload is exactly the sdist pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let Some(sdist) = run.resolve(sdist_rloc) else {
        return 1;
    };
    let identity = match hackage_read_identity(&sdist).and_then(|identity| {
        hackage_check_identity(&identity, expected_package, expected_version).map(|()| identity)
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("hackage_deploy: {target_name}: {error}");
            return 1;
        }
    };
    let (package, version) = (identity.0.as_str(), identity.1.as_str());
    if OwnerGate::requested(env_or_empty) {
        if let Some(refusal) = placeholder_refusal("hackage_deploy", version) {
            eprintln!("{refusal}");
            return 1;
        }
        let secrets = match HACKAGE_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(cabal) = which_on_path("cabal") else {
            eprintln!("hackage_deploy: 'cabal' CLI not found on PATH");
            return 1;
        };
        return hackage_live_upload(
            &cabal,
            &sdist,
            secrets.get("HACKAGE_API_TOKEN"),
            package,
            version,
        );
    }
    let staged = basename(&sdist).and_then(|base| {
        let dir = run.outdir().join(format!("{target_name}-sdist"));
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(base);
        copy_verified(&sdist, &dest)?;
        Ok(dest)
    });
    report_staged(staged, package, version)
}

fn report_staged(staged: io::Result<std::path::PathBuf>, package: &str, version: &str) -> i32 {
    match staged {
        Ok(dest) => {
            println!(
                "hackage_deploy: staged {package} {version} (build_mode: {})",
                deploy_build_mode()
            );
            println!("  sdist: {}", dest.display());
            0
        }
        Err(error) => {
            eprintln!("hackage_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn upload_keeps_the_token_off_argv_and_errors() {
        let token = "fake-hackage-token-7731";
        let dir = tempfile::tempdir().unwrap();
        let private = PrivateConfigDir::create("dx-hackage-test-").unwrap();
        let config = write_cabal_config(&private, token).unwrap();
        let mut cmd = hackage_upload_command(
            &dir.path().join("no-such-cabal"),
            Path::new("p.tar.gz"),
            &config,
        );
        assert!(
            cmd.get_args()
                .all(|arg| !arg.to_string_lossy().contains(token))
        );
        let env = cmd
            .get_envs()
            .find(|(name, _)| *name == "CABAL_CONFIG")
            .and_then(|(_, value)| value)
            .unwrap();
        assert_eq!(Path::new(env), config);
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains(token));
        assert!(text.contains("url: https://hackage.haskell.org/\n"));
        assert!(!text.contains("http://"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&config).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let error = run_publish(&mut cmd).unwrap_err();
        assert!(!error.to_string().contains(token));
    }

    #[test]
    fn upload_refuses_whitespace_tokens() {
        assert!(plain_token_refusal("hackage_deploy", "a b", &[]).is_some());
        assert!(plain_token_refusal("hackage_deploy", "a\nb", &[]).is_some());
        assert!(plain_token_refusal("hackage_deploy", "abc", &[]).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn stub_cabal_output_never_holds_the_token() {
        use std::os::unix::fs::PermissionsExt as _;
        let token = "fake-hackage-token-4402";
        let dir = tempfile::tempdir().unwrap();
        let stub = dir.path().join("cabal");
        std::fs::write(&stub, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let private = PrivateConfigDir::create("dx-hackage-test-").unwrap();
        let config = write_cabal_config(&private, token).unwrap();
        let mut cmd = hackage_upload_command(&stub, Path::new("p.tar.gz"), &config);
        let out = run_publish(&mut cmd).unwrap();
        assert!(out.success());
        let shown = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(shown.contains("upload"));
        assert!(!shown.contains(token));
    }

    fn write_sdist(dir: &Path, file_name: &str, entry: &str, cabal: &str) -> std::path::PathBuf {
        let mut tar = dx_archive::TarWriter::new();
        tar.add_file(entry, cabal.as_bytes(), 0o644)
            .expect("append");
        let path = dir.join(file_name);
        std::fs::write(&path, tar.finish().expect("finish tar.gz")).expect("write sdist");
        path
    }

    #[test]
    fn refuses_an_sdist_with_a_dot_dot_member() {
        let dir = scratch_dir();
        let sdist = dir.path().join("demo-1.2.0.tar.gz");
        std::fs::write(&sdist, raw_tar_gz("../demo.cabal", CABAL.as_bytes())).expect("write");
        let error = hackage_read_identity(&sdist)
            .expect_err("dot dot")
            .to_string();
        assert!(error.contains("is not a valid sdist"), "{error}");
        assert!(error.contains("'..'"), "{error}");
    }

    const CABAL: &str =
        "cabal-version: 2.4\nName: demo\nversion:  1.2.0\n\nlibrary\n  version: 9\n";

    #[test]
    fn reads_identity_from_the_cabal_file() {
        let dir = scratch_dir();
        let sdist = write_sdist(
            dir.path(),
            "demo-1.2.0.tar.gz",
            "demo-1.2.0/demo.cabal",
            CABAL,
        );
        let identity = hackage_read_identity(&sdist).expect("identity");
        assert_eq!(identity, ("demo".to_owned(), "1.2.0".to_owned()));
        hackage_check_identity(&identity, "demo", "1.2.0").expect("declared identity");
        hackage_check_identity(&identity, "", "").expect("no declared identity");
        assert!(
            hackage_check_identity(&identity, "", "1.2.1")
                .expect_err("version")
                .to_string()
                .contains("BUILD declares 1.2.1")
        );
        assert!(
            hackage_check_identity(&identity, "other", "")
                .expect_err("package")
                .to_string()
                .contains("BUILD declares other")
        );
    }

    #[test]
    fn rejects_a_cabal_version_that_differs_from_the_file_name() {
        let dir = scratch_dir();
        let sdist = write_sdist(
            dir.path(),
            "demo-1.3.0.tar.gz",
            "demo-1.3.0/demo.cabal",
            CABAL,
        );
        let error = hackage_read_identity(&sdist)
            .expect_err("mismatch")
            .to_string();
        assert!(error.contains("declares demo 1.2.0, but the sdist is named demo 1.3.0"));
    }

    #[test]
    fn rejects_bad_names_and_missing_cabal_files() {
        let dir = scratch_dir();
        let plain = dir.path().join("demo.zip");
        std::fs::write(&plain, b"x").expect("write");
        assert!(
            hackage_read_identity(&plain)
                .expect_err("suffix")
                .to_string()
                .contains("not a .tar.gz")
        );
        let unversioned = write_sdist(dir.path(), "demo.tar.gz", "demo/demo.cabal", CABAL);
        assert!(
            hackage_read_identity(&unversioned)
                .expect_err("name")
                .to_string()
                .contains("<package>-<version>")
        );
        let missing = write_sdist(
            dir.path(),
            "demo-1.2.0.tar.gz",
            "demo-1.2.0/other.cabal",
            CABAL,
        );
        assert!(
            hackage_read_identity(&missing)
                .expect_err("missing")
                .to_string()
                .contains("has no demo-1.2.0/demo.cabal")
        );
    }

    #[test]
    fn default_run_stages_the_sdist_and_live_run_needs_approval() {
        if env_test_in_child(
            "hackage::tests::default_run_stages_the_sdist_and_live_run_needs_approval",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        write_sdist(
            &runfiles,
            "demo-1.2.0.tar.gz",
            "demo-1.2.0/demo.cabal",
            CABAL,
        );
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles_dir = runfiles.to_string_lossy().into_owned();
        let launch = |live: Option<&str>| {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles_dir.as_str())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("DX_DEPLOY_LIVE", live),
                    ("HACKAGE_PUBLISH_APPROVED", None),
                    ("HACKAGE_API_TOKEN", None),
                ],
                || hackage_main("demo-1.2.0.tar.gz", "demo", "demo", "1.2.0", &argv),
            )
        };
        assert_eq!(launch(None), 0);
        assert!(outdir.join("demo-sdist/demo-1.2.0.tar.gz").is_file());
        assert_eq!(launch(Some("1")), 1);
    }
}
