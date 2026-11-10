//! CPAN publisher: dist identity checks, local staging and the PAUSE upload.

use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty,
    exit_text, placeholder_refusal, print_redacted, run_publish, scrubbed_command,
};
use crate::stage::*;
use std::path::{Path, PathBuf};

/// Owner approval gate for cpan_deploy.
pub(crate) const CPAN_GATE: OwnerGate = OwnerGate {
    tool: "cpan_deploy",
    action: "upload",
    credentials: &["CPAN_API_USER", "CPAN_API_TOKEN"],
    approved: "CPAN_PUBLISH_APPROVED",
};

/// Extra env names `perl` needs to find its libraries.
const PERL_CHILD_ENV_EXTRA: &[&str] = &["PERL5LIB", "PERL_LOCAL_LIB_ROOT", "PERL_MB_OPT", "PERL_MM_OPT"];

/// Perl program that uploads the file in `$ARGV[0]` to PAUSE.
const UPLOAD_SCRIPT: &str = "use CPAN::Uploader; CPAN::Uploader->upload_file($ARGV[0], { user => $ENV{PAUSE_USER}, password => $ENV{PAUSE_PASSWORD} });";

/// Reads the distribution name and version from a `<Dist>-<version>.tar.gz` file name and checks the archive.
///
/// The archive must hold `<Dist>-<version>/META.json` or `<Dist>-<version>/META.yml`.
pub fn cpan_read_identity(dist: &Path) -> Result<(String, String), String> {
    let file_name = dist
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".tar.gz")
        .ok_or_else(|| format!("{file_name} is not a .tar.gz dist"))?;
    let (name, version) = stem
        .rsplit_once('-')
        .filter(|(name, version)| !name.is_empty() && !version.is_empty())
        .ok_or_else(|| format!("{file_name} is not named <Dist>-<version>.tar.gz"))?;
    let wants = [format!("{stem}/META.json"), format!("{stem}/META.yml")];
    let open = std::fs::File::open(dist)
        .map_err(|error| format!("cannot open {}: {error}", dist.display()))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(open));
    let entries = archive
        .entries()
        .map_err(|error| format!("{file_name} is not a valid dist: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{file_name} is not a valid dist: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("{file_name} has an unreadable entry name: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if wants.contains(&path) {
            return Ok((name.to_owned(), version.to_owned()));
        }
    }
    Err(format!("{file_name} has no {} or {}", wants[0], wants[1]))
}

/// Checks the dist identity against the declared package and version.
pub fn cpan_check_identity(
    actual: &(String, String),
    expected_package: &str,
    expected_version: &str,
) -> Result<(), String> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(format!(
            "package mismatch: dist is {}, BUILD declares {expected_package}",
            actual.0
        ));
    }
    if !expected_version.is_empty() && actual.1 != expected_version {
        return Err(format!(
            "version mismatch: dist is {}, BUILD declares {expected_version}",
            actual.1
        ));
    }
    Ok(())
}

/// Uploads the dist to PAUSE with `CPAN::Uploader`; the credentials go through the env.
fn cpan_live_upload(perl: &Path, dist: &Path, user: &str, token: &str) -> Result<(), String> {
    let mut cmd = scrubbed_command(
        perl,
        &[PUBLISH_CHILD_ENV_KEEP, PERL_CHILD_ENV_EXTRA],
        &[("PAUSE_USER", user), ("PAUSE_PASSWORD", token)],
    );
    cmd.arg("-e").arg(UPLOAD_SCRIPT).arg(dist);
    match run_publish(&mut cmd) {
        Ok(out) if out.success() => {
            print_redacted(&out, token);
            Ok(())
        }
        Ok(out) => {
            print_redacted(&out, token);
            Err(format!("cpan_deploy: perl failed with {}", exit_text(&out)))
        }
        Err(error) => Err(format!("cpan_deploy: {error}")),
    }
}

/// Finds the pinned perl, or `perl` on PATH when none is pinned.
fn find_perl(run: &DeployRun, perl_rloc: &str) -> Option<PathBuf> {
    if perl_rloc.is_empty() {
        let perl = which_on_path("perl");
        if perl.is_none() {
            eprintln!("cpan_deploy: 'perl' not found on PATH; set perl");
        }
        return perl;
    }
    run.resolve(perl_rloc)
}

/// Runs the CPAN deploy tool.
pub fn cpan_main(
    dist_rloc: &str,
    target_name: &str,
    expected_package: &str,
    expected_version: &str,
    perl_rloc: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "cpan_deploy",
        "the upload is exactly the dist pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let Some(dist) = run.resolve(dist_rloc) else {
        return 1;
    };
    let identity = match cpan_read_identity(&dist).and_then(|identity| {
        cpan_check_identity(&identity, expected_package, expected_version).map(|()| identity)
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("cpan_deploy: {target_name}: {error}");
            return 1;
        }
    };
    let (package, version) = (identity.0.as_str(), identity.1.as_str());
    if CPAN_GATE.requested(env_or_empty) {
        if let Some(refusal) = placeholder_refusal("cpan_deploy", version) {
            eprintln!("{refusal}");
            return 1;
        }
        let secrets = match CPAN_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(perl) = find_perl(&run, perl_rloc) else {
            return 1;
        };
        return match cpan_live_upload(
            &perl,
            &dist,
            secrets.get("CPAN_API_USER"),
            secrets.get("CPAN_API_TOKEN"),
        ) {
            Ok(()) => {
                println!("cpan_deploy: uploaded {package} {version}");
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
    }
    let staged = basename(&dist).and_then(|base| {
        let dir = run.outdir().join(format!("{target_name}-dist"));
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(base);
        copy_verified(&dist, &dest)?;
        Ok(dest)
    });
    match staged {
        Ok(dest) => {
            println!(
                "cpan_deploy: staged {package} {version} (build_mode: {})",
                deploy_build_mode()
            );
            println!("  dist: {}", dest.display());
            0
        }
        Err(error) => {
            eprintln!("cpan_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn write_dist(dir: &Path, file_name: &str, entry: &str) -> PathBuf {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(2);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, entry, &b"{}"[..])
            .expect("append meta");
        let bytes = builder.into_inner().expect("finish tar");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &bytes).expect("gzip");
        let path = dir.join(file_name);
        std::fs::write(&path, encoder.finish().expect("finish gzip")).expect("write dist");
        path
    }

    #[test]
    fn reads_identity_from_the_file_name_and_meta() {
        let dir = scratch_dir();
        let dist = write_dist(dir.path(), "Demo-Dist-1.2.0.tar.gz", "Demo-Dist-1.2.0/META.json");
        let identity = cpan_read_identity(&dist).expect("identity");
        assert_eq!(identity, ("Demo-Dist".to_owned(), "1.2.0".to_owned()));
        assert_eq!(cpan_check_identity(&identity, "Demo-Dist", "1.2.0"), Ok(()));
        assert_eq!(cpan_check_identity(&identity, "", ""), Ok(()));
        assert!(
            cpan_check_identity(&identity, "", "1.2.1")
                .expect_err("version")
                .contains("BUILD declares 1.2.1")
        );
        assert!(
            cpan_check_identity(&identity, "Other", "")
                .expect_err("package")
                .contains("BUILD declares Other")
        );
        let yml = write_dist(dir.path(), "Demo-1.0.tar.gz", "Demo-1.0/META.yml");
        assert!(cpan_read_identity(&yml).is_ok());
    }

    #[test]
    fn rejects_bad_names_and_missing_meta() {
        let dir = scratch_dir();
        let plain = dir.path().join("Demo.zip");
        std::fs::write(&plain, b"x").expect("write");
        assert!(
            cpan_read_identity(&plain)
                .expect_err("suffix")
                .contains("not a .tar.gz")
        );
        let unversioned = write_dist(dir.path(), "Demo.tar.gz", "Demo/META.json");
        assert!(
            cpan_read_identity(&unversioned)
                .expect_err("name")
                .contains("<Dist>-<version>")
        );
        let missing = write_dist(dir.path(), "Demo-1.0.tar.gz", "Demo-1.0/Makefile.PL");
        assert!(
            cpan_read_identity(&missing)
                .expect_err("missing")
                .contains("has no Demo-1.0/META.json")
        );
    }

    #[test]
    fn default_run_stages_the_dist_and_live_run_needs_approval() {
        if env_test_in_child("cpan::tests::default_run_stages_the_dist_and_live_run_needs_approval")
        {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        write_dist(&runfiles, "Demo-1.2.0.tar.gz", "Demo-1.2.0/META.json");
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles_dir = runfiles.to_string_lossy().into_owned();
        let launch = |live: Option<&str>| {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles_dir.as_str())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("DX_DEPLOY_LIVE", live),
                    ("CPAN_PUBLISH_APPROVED", None),
                    ("CPAN_API_USER", None),
                    ("CPAN_API_TOKEN", None),
                ],
                || cpan_main("Demo-1.2.0.tar.gz", "demo", "Demo", "1.2.0", "", &argv),
            )
        };
        assert_eq!(launch(None), 0);
        assert!(outdir.join("demo-dist/Demo-1.2.0.tar.gz").is_file());
        assert_eq!(launch(Some("1")), 1);
    }
}
