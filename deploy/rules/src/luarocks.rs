//! LuaRocks publisher: rockspec identity checks, local staging and the upload.

use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_profile, env_or_empty,
    placeholder_refusal, print_redacted, scrubbed_command,
};
use crate::stage::*;
use std::path::Path;

/// Owner approval gate for luarocks_deploy.
pub(crate) const LUAROCKS_GATE: OwnerGate = OwnerGate {
    tool: "luarocks_deploy",
    action: "upload",
    credentials: &["LUAROCKS_API_KEY"],
    approved: "LUAROCKS_PUBLISH_APPROVED",
    live: "LUAROCKS_PUBLISH_LIVE",
};

/// Extra env names `luarocks` needs to find its config and Lua install.
const LUAROCKS_CHILD_ENV_EXTRA: &[&str] = &[
    "LUAROCKS_CONFIG",
    "LUAROCKS_SYSCONFDIR",
    "LUA_PATH",
    "LUA_CPATH",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
];

/// Returns the string a rockspec assigns to one top-level variable.
fn rockspec_string<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        if name.trim_end() != key {
            return None;
        }
        let value = value.trim().trim_end_matches(',');
        let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        value.strip_prefix(quote)?.strip_suffix(quote)
    })
}

/// Reads the package and version a rockspec declares, checked against its file name.
///
/// The file must be named `<package>-<version>.rockspec`, as `luarocks upload` requires.
pub fn luarocks_read_identity(rockspec: &Path) -> Result<(String, String), String> {
    let file_name = rockspec
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".rockspec")
        .ok_or_else(|| format!("{file_name} is not a .rockspec file"))?;
    let text = std::fs::read_to_string(rockspec)
        .map_err(|error| format!("cannot read {}: {error}", rockspec.display()))?;
    let package = rockspec_string(&text, "package")
        .ok_or_else(|| format!("{file_name} has no top-level package string"))?;
    let version = rockspec_string(&text, "version")
        .ok_or_else(|| format!("{file_name} has no top-level version string"))?;
    if stem != format!("{package}-{version}") {
        return Err(format!(
            "{file_name} declares {package} {version}; the file must be named {package}-{version}.rockspec"
        ));
    }
    Ok((package.to_owned(), version.to_owned()))
}

/// Checks the rockspec identity against the declared package and version.
///
/// A declared version matches the full `<version>-<revision>` or the part before the revision.
pub fn luarocks_check_identity(
    actual: &(String, String),
    expected_package: &str,
    expected_version: &str,
) -> Result<(), String> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(format!(
            "package mismatch: rockspec declares {}, BUILD declares {expected_package}",
            actual.0
        ));
    }
    let upstream = actual
        .1
        .rsplit_once('-')
        .map_or(actual.1.as_str(), |(v, _)| v);
    if !expected_version.is_empty() && actual.1 != expected_version && upstream != expected_version
    {
        return Err(format!(
            "version mismatch: rockspec declares {}, BUILD declares {expected_version}",
            actual.1
        ));
    }
    Ok(())
}

/// Packs and uploads the rockspec with `luarocks upload`.
fn luarocks_live_upload(
    luarocks: &Path,
    rockspec: &Path,
    key: &str,
    package: &str,
    version: &str,
) -> i32 {
    let mut cmd = scrubbed_command(
        luarocks,
        &[PUBLISH_CHILD_ENV_KEEP, LUAROCKS_CHILD_ENV_EXTRA],
        &[],
    );
    cmd.arg("upload")
        .arg(rockspec)
        .arg(format!("--api-key={key}"));
    match cmd.output() {
        Ok(out) if out.status.success() => {
            print_redacted(&out, key);
            println!("luarocks_deploy: uploaded {package} {version}");
            0
        }
        Ok(out) => {
            print_redacted(&out, key);
            eprintln!("luarocks_deploy: luarocks failed with {}", out.status);
            1
        }
        Err(error) => {
            eprintln!("luarocks_deploy: {error}");
            1
        }
    }
}

/// Runs the LuaRocks deploy tool.
pub fn luarocks_main(
    rockspec_rloc: &str,
    target_name: &str,
    expected_package: &str,
    expected_version: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "luarocks_deploy",
        "the upload is exactly the rockspec pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let Some(rockspec) = run.resolve(rockspec_rloc) else {
        return 1;
    };
    let identity = match luarocks_read_identity(&rockspec).and_then(|identity| {
        luarocks_check_identity(&identity, expected_package, expected_version).map(|()| identity)
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("luarocks_deploy: {target_name}: {error}");
            return 1;
        }
    };
    let (package, version) = (identity.0.as_str(), identity.1.as_str());
    if LUAROCKS_GATE.requested(env_or_empty) {
        let upstream = version.rsplit_once('-').map_or(version, |(v, _)| v);
        if let Some(refusal) = placeholder_refusal("luarocks_deploy", upstream) {
            eprintln!("{refusal}");
            return 1;
        }
        let secrets = match LUAROCKS_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(luarocks) = which_on_path("luarocks") else {
            eprintln!("luarocks_deploy: 'luarocks' CLI not found on PATH");
            return 1;
        };
        return luarocks_live_upload(
            &luarocks,
            &rockspec,
            secrets.get("LUAROCKS_API_KEY"),
            package,
            version,
        );
    }
    let staged = basename(&rockspec).and_then(|base| {
        let dir = run.outdir().join(format!("{target_name}-rock"));
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(base);
        copy_verified(&rockspec, &dest)?;
        Ok(dest)
    });
    match staged {
        Ok(dest) => {
            println!(
                "luarocks_deploy: staged {package} {version} (profile: {})",
                deploy_profile()
            );
            println!("  rockspec: {}", dest.display());
            0
        }
        Err(error) => {
            eprintln!("luarocks_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    const ROCKSPEC: &str = "package = \"demo\"\nversion = \"1.2.0-1\"\nsource = {\n  url = \"x\",\n}\ndescription = {\n  version = \"9\",\n}\n";

    fn write_rockspec(dir: &Path, file_name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.join(file_name);
        std::fs::write(&path, body).expect("write rockspec");
        path
    }

    #[test]
    fn reads_identity_from_the_rockspec() {
        let dir = scratch_dir();
        let path = write_rockspec(dir.path(), "demo-1.2.0-1.rockspec", ROCKSPEC);
        let identity = luarocks_read_identity(&path).expect("identity");
        assert_eq!(identity, ("demo".to_owned(), "1.2.0-1".to_owned()));
        for version in ["", "1.2.0-1", "1.2.0"] {
            assert_eq!(luarocks_check_identity(&identity, "demo", version), Ok(()));
        }
        assert!(
            luarocks_check_identity(&identity, "", "1.2.1")
                .expect_err("version")
                .contains("BUILD declares 1.2.1")
        );
        assert!(
            luarocks_check_identity(&identity, "other", "")
                .expect_err("package")
                .contains("BUILD declares other")
        );
    }

    #[test]
    fn rejects_a_rockspec_that_disagrees_with_its_file_name() {
        let dir = scratch_dir();
        let wrong = write_rockspec(dir.path(), "demo-1.3.0-1.rockspec", ROCKSPEC);
        assert!(
            luarocks_read_identity(&wrong)
                .expect_err("name")
                .contains("must be named demo-1.2.0-1.rockspec")
        );
        let bare = write_rockspec(dir.path(), "demo-1.rockspec", "package = \"demo\"\n");
        assert!(
            luarocks_read_identity(&bare)
                .expect_err("version")
                .contains("no top-level version")
        );
        let other = write_rockspec(dir.path(), "demo.txt", ROCKSPEC);
        assert!(
            luarocks_read_identity(&other)
                .expect_err("suffix")
                .contains("not a .rockspec")
        );
    }

    #[test]
    fn default_run_stages_the_rockspec_and_live_run_needs_approval() {
        if env_test_in_child(
            "luarocks::tests::default_run_stages_the_rockspec_and_live_run_needs_approval",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        write_rockspec(&runfiles, "demo-1.2.0-1.rockspec", ROCKSPEC);
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles_dir = runfiles.to_string_lossy().into_owned();
        let launch = |live: Option<&str>| {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles_dir.as_str())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("LUAROCKS_PUBLISH_LIVE", live),
                    ("LUAROCKS_PUBLISH_APPROVED", None),
                    ("LUAROCKS_API_KEY", None),
                ],
                || luarocks_main("demo-1.2.0-1.rockspec", "demo", "demo", "1.2.0", &argv),
            )
        };
        assert_eq!(launch(None), 0);
        assert!(outdir.join("demo-rock/demo-1.2.0-1.rockspec").is_file());
        assert_eq!(launch(Some("1")), 1);
    }
}
