//! The Xcode tools: the one place that names host paths, and the Xcode that `DEVELOPER_DIR` selects.

use std::path::{Path, PathBuf};

use crate::util::{Cmd, Env, Failure, Output, Runner, missing, usage};

pub const XCRUN: &str = "/usr/bin/xcrun";
pub const CODESIGN: &str = "/usr/bin/codesign";
pub const SECURITY: &str = "/usr/bin/security";
pub const PLUTIL: &str = "/usr/bin/plutil";
pub const DITTO: &str = "/usr/bin/ditto";
pub const OPENSSL: &str = "/usr/bin/openssl";
pub const SPCTL: &str = "/usr/sbin/spctl";

/// Where installed Xcode applications live.
pub const APPLICATIONS: &str = "/Applications";

/// The text of `CFBundleShortVersionString` in a `version.plist`.
pub fn short_version(plist: &str) -> Option<&str> {
    let after = plist.split_once("<key>CFBundleShortVersionString</key>")?.1;
    let value = after.split_once("<string>")?.1;
    Some(value.split_once("</string>")?.0.trim())
}

fn components(version: &str) -> [u64; 3] {
    let mut found = [0; 3];
    for (slot, part) in found.iter_mut().zip(version.split('.')) {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        match digits.parse() {
            Ok(number) if !digits.is_empty() => *slot = number,
            _ => break,
        }
    }
    found
}

/// Whether the installed `have` (`15.2`) is the configured `want` (`15.2.0.15C500b`).
pub fn version_matches(want: &str, have: &str) -> bool {
    !want.is_empty() && components(want) == components(have)
}

fn installed(developer_dir: &Path, want: &str) -> bool {
    let plist = developer_dir
        .parent()
        .map(|contents| contents.join("version.plist"))
        .and_then(|path| std::fs::read_to_string(path).ok());
    plist
        .as_deref()
        .and_then(short_version)
        .is_some_and(|have| version_matches(want, have))
}

/// The developer directory of the Xcode installed as `version`.
///
/// `preset` is a `DEVELOPER_DIR` already in the environment; it wins when it is that Xcode.
pub fn developer_dir(
    version: &str,
    preset: Option<&str>,
    applications: &Path,
) -> Result<PathBuf, Failure> {
    if version.is_empty() {
        return Err(usage("no Xcode version is configured for this program"));
    }
    if let Some(preset) = preset.map(PathBuf::from).filter(|dir| installed(dir, version)) {
        return Ok(preset);
    }
    let mut apps: Vec<PathBuf> = std::fs::read_dir(applications)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|end| end == "app"))
                .collect()
        })
        .unwrap_or_default();
    apps.sort();
    apps.into_iter()
        .map(|app| app.join("Contents/Developer"))
        .find(|dir| installed(dir, version))
        .ok_or_else(|| {
            missing(format!(
                "Xcode {version} is not installed under {}; install it or select another xcode_version",
                applications.display()
            ))
        })
}

/// The developer directory the launcher's `DX_XCODE_VERSION` selects.
pub fn for_launcher(env: Env) -> Result<PathBuf, Failure> {
    let version = env("DX_XCODE_VERSION").unwrap_or_default();
    developer_dir(&version, env("DEVELOPER_DIR").as_deref(), Path::new(APPLICATIONS))
}

/// The developer directory a rules_apple test runner selects.
pub fn for_test_runner(env: Env) -> Result<PathBuf, Failure> {
    if let Some(version) = env("XCODE_VERSION_OVERRIDE") {
        return developer_dir(&version, env("DEVELOPER_DIR").as_deref(), Path::new(APPLICATIONS));
    }
    env("DEVELOPER_DIR").map(PathBuf::from).ok_or_else(|| {
        usage("neither XCODE_VERSION_OVERRIDE nor DEVELOPER_DIR is set; the test runner selects no Xcode")
    })
}

/// The Xcode tools of one Xcode, started through a runner.
pub struct Tools<'a> {
    pub developer_dir: Option<PathBuf>,
    pub runner: &'a dyn Runner,
}

impl Tools<'_> {
    fn cmd(&self, program: &str, args: &[&str]) -> Cmd {
        Cmd {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: self
                .developer_dir
                .iter()
                .map(|dir| ("DEVELOPER_DIR".to_owned(), dir.to_string_lossy().into_owned()))
                .collect(),
        }
    }

    /// Runs `xcrun <args>` and captures it.
    pub fn xcrun(&self, args: &[&str]) -> Output {
        self.runner.capture(&self.cmd(XCRUN, args))
    }

    /// Runs `xcrun <args>` with this process's streams.
    pub fn xcrun_inherit(&self, args: &[&str]) -> Option<i32> {
        self.runner.inherit(&self.cmd(XCRUN, args))
    }

    /// Runs the system tool `program` and captures it.
    pub fn tool(&self, program: &str, args: &[&str]) -> Output {
        self.runner.capture(&self.cmd(program, args))
    }

    /// Runs an arbitrary program with this process's streams.
    pub fn run_program(&self, program: &str, args: &[&str]) -> Option<i32> {
        self.runner.inherit(&self.cmd(program, args))
    }
}
