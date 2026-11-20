//! LuaRocks publisher: rockspec identity checks, local staging and the upload.

use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, exit_text,
    pages_redact, placeholder_refusal, print_redacted, run_publish, scrubbed_command,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use std::fmt;
use std::path::Path;

/// Owner approval gate for luarocks_deploy.
pub(crate) const LUAROCKS_GATE: OwnerGate = OwnerGate {
    tool: "luarocks_deploy",
    action: "upload",
    credentials: &["LUAROCKS_API_TOKEN"],
    approved: "LUAROCKS_PUBLISH_APPROVED",
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

/// Why a rockspec failed its identity check.
#[derive(Debug)]
pub enum LuarocksError {
    NotRockspec {
        file_name: String,
    },
    Read(PathIoError),
    NoPackage {
        file_name: String,
    },
    NoVersion {
        file_name: String,
    },
    FileName {
        file_name: String,
        package: String,
        version: String,
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

impl fmt::Display for LuarocksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRockspec { file_name } => write!(f, "{file_name} is not a .rockspec file"),
            Self::Read(source) => write!(f, "{source}"),
            Self::NoPackage { file_name } => {
                write!(f, "{file_name} has no top-level package string")
            }
            Self::NoVersion { file_name } => {
                write!(f, "{file_name} has no top-level version string")
            }
            Self::FileName {
                file_name,
                package,
                version,
            } => write!(
                f,
                "{file_name} declares {package} {version}; the file must be named {package}-{version}.rockspec"
            ),
            Self::PackageMismatch { actual, expected } => write!(
                f,
                "package mismatch: rockspec declares {actual}, BUILD declares {expected}"
            ),
            Self::VersionMismatch { actual, expected } => write!(
                f,
                "version mismatch: rockspec declares {actual}, BUILD declares {expected}"
            ),
        }
    }
}

impl std::error::Error for LuarocksError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) => Some(source),
            Self::NotRockspec { .. }
            | Self::NoPackage { .. }
            | Self::NoVersion { .. }
            | Self::FileName { .. }
            | Self::PackageMismatch { .. }
            | Self::VersionMismatch { .. } => None,
        }
    }
}

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
pub fn luarocks_read_identity(rockspec: &Path) -> Result<(String, String), LuarocksError> {
    let file_name = rockspec
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".rockspec")
        .ok_or_else(|| LuarocksError::NotRockspec {
            file_name: file_name.to_owned(),
        })?;
    let text = path_io::read_to_string(rockspec).map_err(LuarocksError::Read)?;
    let package = rockspec_string(&text, "package").ok_or_else(|| LuarocksError::NoPackage {
        file_name: file_name.to_owned(),
    })?;
    let version = rockspec_string(&text, "version").ok_or_else(|| LuarocksError::NoVersion {
        file_name: file_name.to_owned(),
    })?;
    if stem != format!("{package}-{version}") {
        return Err(LuarocksError::FileName {
            file_name: file_name.to_owned(),
            package: package.to_owned(),
            version: version.to_owned(),
        });
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
) -> Result<(), LuarocksError> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(LuarocksError::PackageMismatch {
            actual: actual.0.clone(),
            expected: expected_package.to_owned(),
        });
    }
    let upstream = actual
        .1
        .rsplit_once('-')
        .map_or(actual.1.as_str(), |(v, _)| v);
    if !expected_version.is_empty() && actual.1 != expected_version && upstream != expected_version
    {
        return Err(LuarocksError::VersionMismatch {
            actual: actual.1.clone(),
            expected: expected_version.to_owned(),
        });
    }
    Ok(())
}

/// Builds the `luarocks upload` launch reading its key from beside `config`.
fn luarocks_upload_command(
    luarocks: &Path,
    rockspec: &Path,
    config: &Path,
) -> std::process::Command {
    let config = config.to_string_lossy();
    let mut cmd = scrubbed_command(
        luarocks,
        &[PUBLISH_CHILD_ENV_KEEP, LUAROCKS_CHILD_ENV_EXTRA],
        &[("LUAROCKS_CONFIG", config.as_ref())],
    );
    cmd.arg("upload").arg(rockspec);
    cmd
}

/// Writes the private user config and upload config that carry the key.
fn write_luarocks_config(dir: &PrivateConfigDir, key: &str) -> std::io::Result<std::path::PathBuf> {
    dir.write("upload_config.lua", &format!("key = \"{key}\"\n"))?;
    dir.write("config.lua", "-- dx_deploy private config\n")
}

/// Packs and uploads the rockspec with `luarocks upload`.
fn luarocks_live_upload(
    luarocks: &Path,
    rockspec: &Path,
    key: &str,
    package: &str,
    version: &str,
) -> i32 {
    if let Some(refusal) = plain_token_refusal("luarocks_deploy", key, &['"', '\\']) {
        eprintln!("{refusal}");
        return 1;
    }
    let dir = match PrivateConfigDir::create("dx-luarocks-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("luarocks_deploy: cannot create the private config dir: {error}");
            return 1;
        }
    };
    let config = match write_luarocks_config(&dir, key) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("luarocks_deploy: cannot write the private config: {error}");
            return 1;
        }
    };
    let mut cmd = luarocks_upload_command(luarocks, rockspec, &config);
    match run_publish(&mut cmd) {
        Ok(out) if out.success() => {
            print_redacted(&out, key);
            println!("luarocks_deploy: uploaded {package} {version}");
            0
        }
        Ok(out) => {
            print_redacted(&out, key);
            eprintln!("luarocks_deploy: luarocks failed with {}", exit_text(&out));
            1
        }
        Err(error) => {
            eprintln!("luarocks_deploy: {}", pages_redact(&error.to_string(), key));
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
    if OwnerGate::requested(env_or_empty) {
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
            secrets.get("LUAROCKS_API_TOKEN"),
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
                "luarocks_deploy: staged {package} {version} (build_mode: {})",
                deploy_build_mode()
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

    #[test]
    fn upload_keeps_the_key_off_argv_and_errors() {
        let key = "fake-luarocks-key-5519";
        let dir = tempfile::tempdir().unwrap();
        let private = PrivateConfigDir::create("dx-luarocks-test-").unwrap();
        let config = write_luarocks_config(&private, key).unwrap();
        let mut cmd = luarocks_upload_command(
            &dir.path().join("no-such-luarocks"),
            Path::new("r.rockspec"),
            &config,
        );
        assert!(
            cmd.get_args()
                .all(|arg| !arg.to_string_lossy().contains(key))
        );
        let env = cmd
            .get_envs()
            .find(|(name, _)| *name == "LUAROCKS_CONFIG")
            .and_then(|(_, value)| value)
            .unwrap();
        assert_eq!(Path::new(env), config);
        let upload = std::fs::read_to_string(private.path().join("upload_config.lua")).unwrap();
        assert!(upload.contains(key));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(private.path().join("upload_config.lua"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let error = run_publish(&mut cmd).unwrap_err();
        assert!(!error.to_string().contains(key));
    }

    #[test]
    fn upload_refuses_keys_a_lua_string_cannot_carry() {
        for bad in ["a\"b", "a\\b", "a\nb"] {
            assert!(plain_token_refusal("luarocks_deploy", bad, &['"', '\\']).is_some());
        }
    }

    #[cfg(unix)]
    #[test]
    fn stub_luarocks_output_never_holds_the_key() {
        use std::os::unix::fs::PermissionsExt as _;
        let key = "fake-luarocks-key-8841";
        let dir = tempfile::tempdir().unwrap();
        let stub = dir.path().join("luarocks");
        std::fs::write(&stub, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let private = PrivateConfigDir::create("dx-luarocks-test-").unwrap();
        let config = write_luarocks_config(&private, key).unwrap();
        let mut cmd = luarocks_upload_command(&stub, Path::new("r.rockspec"), &config);
        let out = run_publish(&mut cmd).unwrap();
        assert!(out.success());
        let shown = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(shown.contains("upload"));
        assert!(!shown.contains(key));
    }

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
            assert!(luarocks_check_identity(&identity, "demo", version).is_ok());
        }
        let mismatch = luarocks_check_identity(&identity, "", "1.2.1").expect_err("version");
        assert!(matches!(mismatch, LuarocksError::VersionMismatch { .. }));
        assert_eq!(
            mismatch.to_string(),
            "version mismatch: rockspec declares 1.2.0-1, BUILD declares 1.2.1"
        );
        let mismatch = luarocks_check_identity(&identity, "other", "").expect_err("package");
        assert!(matches!(mismatch, LuarocksError::PackageMismatch { .. }));
        assert_eq!(
            mismatch.to_string(),
            "package mismatch: rockspec declares demo, BUILD declares other"
        );
    }

    #[test]
    fn rejects_a_rockspec_that_disagrees_with_its_file_name() {
        let dir = scratch_dir();
        let wrong = write_rockspec(dir.path(), "demo-1.3.0-1.rockspec", ROCKSPEC);
        let name = luarocks_read_identity(&wrong).expect_err("name");
        assert!(matches!(name, LuarocksError::FileName { .. }));
        assert_eq!(
            name.to_string(),
            "demo-1.3.0-1.rockspec declares demo 1.2.0-1; the file must be named demo-1.2.0-1.rockspec"
        );
        let bare = write_rockspec(dir.path(), "demo-1.rockspec", "package = \"demo\"\n");
        let version = luarocks_read_identity(&bare).expect_err("version");
        assert!(matches!(version, LuarocksError::NoVersion { .. }));
        assert_eq!(
            version.to_string(),
            "demo-1.rockspec has no top-level version string"
        );
        let unnamed = write_rockspec(dir.path(), "demo-2.rockspec", "version = \"2\"\n");
        assert!(matches!(
            luarocks_read_identity(&unnamed).expect_err("package"),
            LuarocksError::NoPackage { .. }
        ));
        let other = write_rockspec(dir.path(), "demo.txt", ROCKSPEC);
        let suffix = luarocks_read_identity(&other).expect_err("suffix");
        assert!(matches!(suffix, LuarocksError::NotRockspec { .. }));
        assert_eq!(suffix.to_string(), "demo.txt is not a .rockspec file");
        let missing = dir.path().join("missing-1.0-1.rockspec");
        let error = luarocks_read_identity(&missing).expect_err("missing");
        assert!(matches!(&error, LuarocksError::Read(io) if io.path() == missing));
        assert!(error.to_string().starts_with("cannot read "), "{error}");
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
                    ("DX_DEPLOY_LIVE", live),
                    ("LUAROCKS_PUBLISH_APPROVED", None),
                    ("LUAROCKS_API_TOKEN", None),
                ],
                || luarocks_main("demo-1.2.0-1.rockspec", "demo", "demo", "1.2.0", &argv),
            )
        };
        assert_eq!(launch(None), 0);
        assert!(outdir.join("demo-rock/demo-1.2.0-1.rockspec").is_file());
        assert_eq!(launch(Some("1")), 1);
    }
}
