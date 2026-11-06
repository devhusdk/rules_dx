//! Hackage publisher: sdist identity checks, local staging and the cabal upload.

use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_profile, env_or_empty,
    placeholder_refusal, print_redacted, scrubbed_command,
};
use crate::stage::*;
use std::io::{self, Read};
use std::path::Path;

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

/// Reads the package name and version the sdist's .cabal file declares.
///
/// The sdist must be named `<package>-<version>.tar.gz` and hold
/// `<package>-<version>/<package>.cabal`, which must agree with the file name.
pub fn hackage_read_identity(sdist: &Path) -> Result<(String, String), String> {
    let file_name = sdist
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".tar.gz")
        .ok_or_else(|| format!("{file_name} is not a .tar.gz sdist"))?;
    let (package, version) = stem
        .rsplit_once('-')
        .filter(|(package, version)| !package.is_empty() && !version.is_empty())
        .ok_or_else(|| format!("{file_name} is not named <package>-<version>.tar.gz"))?;
    let want = format!("{stem}/{package}.cabal");
    let open = std::fs::File::open(sdist)
        .map_err(|error| format!("cannot open {}: {error}", sdist.display()))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(open));
    let entries = archive
        .entries()
        .map_err(|error| format!("{file_name} is not a valid sdist: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{file_name} is not a valid sdist: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("{file_name} has an unreadable entry name: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if path != want {
            continue;
        }
        let mut text = String::new();
        entry
            .take(1 << 20)
            .read_to_string(&mut text)
            .map_err(|error| format!("cannot read {want}: {error}"))?;
        let name = cabal_field(&text, "name").unwrap_or_default();
        let declared = cabal_field(&text, "version").unwrap_or_default();
        if name != package || declared != version {
            return Err(format!(
                "{want} declares {name} {declared}, but the sdist is named {package} {version}"
            ));
        }
        return Ok((name.to_owned(), declared.to_owned()));
    }
    Err(format!("{file_name} has no {want}"))
}

/// Checks the sdist identity against the declared package and version.
pub fn hackage_check_identity(
    actual: &(String, String),
    expected_package: &str,
    expected_version: &str,
) -> Result<(), String> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(format!(
            "package mismatch: sdist declares {}, BUILD declares {expected_package}",
            actual.0
        ));
    }
    if !expected_version.is_empty() && actual.1 != expected_version {
        return Err(format!(
            "version mismatch: .cabal declares {}, BUILD declares {expected_version}",
            actual.1
        ));
    }
    Ok(())
}

/// Uploads the sdist and publishes it with `cabal upload --publish`.
fn hackage_live_upload(
    cabal: &Path,
    sdist: &Path,
    token: &str,
    package: &str,
    version: &str,
) -> i32 {
    let mut cmd = scrubbed_command(cabal, &[PUBLISH_CHILD_ENV_KEEP, CABAL_CHILD_ENV_EXTRA], &[]);
    cmd.arg("upload")
        .arg("--publish")
        .arg(format!("--token={token}"))
        .arg(sdist);
    match cmd.output() {
        Ok(out) if out.status.success() => {
            print_redacted(&out, token);
            println!("hackage_deploy: uploaded {package} {version}");
            0
        }
        Ok(out) => {
            print_redacted(&out, token);
            eprintln!("hackage_deploy: cabal failed with {}", out.status);
            1
        }
        Err(error) => {
            eprintln!("hackage_deploy: {error}");
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
    if HACKAGE_GATE.requested(env_or_empty) {
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
                "hackage_deploy: staged {package} {version} (profile: {})",
                deploy_profile()
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

    fn write_sdist(dir: &Path, file_name: &str, entry: &str, cabal: &str) -> std::path::PathBuf {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(cabal.len() as u64);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, entry, cabal.as_bytes())
            .expect("append cabal");
        let bytes = builder.into_inner().expect("finish tar");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &bytes).expect("gzip");
        let path = dir.join(file_name);
        std::fs::write(&path, encoder.finish().expect("finish gzip")).expect("write sdist");
        path
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
        assert_eq!(hackage_check_identity(&identity, "demo", "1.2.0"), Ok(()));
        assert_eq!(hackage_check_identity(&identity, "", ""), Ok(()));
        assert!(
            hackage_check_identity(&identity, "", "1.2.1")
                .expect_err("version")
                .contains("BUILD declares 1.2.1")
        );
        assert!(
            hackage_check_identity(&identity, "other", "")
                .expect_err("package")
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
        let error = hackage_read_identity(&sdist).expect_err("mismatch");
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
                .contains("not a .tar.gz")
        );
        let unversioned = write_sdist(dir.path(), "demo.tar.gz", "demo/demo.cabal", CABAL);
        assert!(
            hackage_read_identity(&unversioned)
                .expect_err("name")
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
