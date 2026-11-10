//! Runs one adb operation against one device.

use std::path::PathBuf;
use std::process::Command;

use crate::util::{Outcome, apks_in, missing, usage};

pub const PREFIX: &str = "dx-android-adb";

/// The flags of one adb launcher.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub adb: String,
    pub serial: String,
    pub op: String,
    pub apk: String,
    pub apks: String,
    pub app: String,
    pub activity: String,
}

/// Reads `--name=value` flags; a later flag replaces an earlier one.
pub fn parse(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags::default();
    for arg in args {
        let (name, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("unknown argument '{arg}'"))?;
        let slot = match name {
            "--adb" => &mut flags.adb,
            "--serial" => &mut flags.serial,
            "--op" => &mut flags.op,
            "--apk" => &mut flags.apk,
            "--apks" => &mut flags.apks,
            "--app" => &mut flags.app,
            "--activity" => &mut flags.activity,
            _ => return Err(format!("unknown argument '{arg}'")),
        };
        value.clone_into(slot);
    }
    Ok(flags)
}

/// The adb arguments of one operation.
pub fn adb_args(flags: &Flags) -> Result<Vec<String>, String> {
    if flags.serial.is_empty() {
        return Err("--serial names the target device or emulator".to_owned());
    }
    let mut args = vec!["-s".to_owned(), flags.serial.clone()];
    match flags.op.as_str() {
        "install" => {
            if flags.apk.is_empty() {
                return Err("--apk names the APK file".to_owned());
            }
            args.extend(["install".to_owned(), "-r".to_owned(), flags.apk.clone()]);
        }
        "install-multiple" => {
            let dir = PathBuf::from(&flags.apks);
            if flags.apks.is_empty() || !dir.is_dir() {
                return Err("--apks names the APK set directory".to_owned());
            }
            let apks = apks_in(&dir);
            if apks.is_empty() {
                return Err(format!("{} holds no APK", dir.display()));
            }
            args.extend(["install-multiple".to_owned(), "-r".to_owned()]);
            args.extend(apks.iter().map(|apk| apk.to_string_lossy().into_owned()));
        }
        "start" => {
            if flags.app.is_empty() || flags.activity.is_empty() {
                return Err("--app and --activity name the component".to_owned());
            }
            args.extend(["shell", "am", "start", "-n"].map(str::to_owned));
            args.push(format!("{}/{}", flags.app, flags.activity));
        }
        "log" => args.extend(["logcat".to_owned(), "-d".to_owned()]),
        op => return Err(format!("unknown op '{op}'")),
    }
    Ok(args)
}

pub fn run(args: &[String]) -> Outcome {
    let mut flags = parse(args).map_err(usage)?;
    if let Some(named) = std::env::var("ADB").ok().filter(|value| !value.is_empty()) {
        flags.adb = named;
    }
    if flags.adb.is_empty() {
        return Err(missing(
            "no adb: add platform-tools to extra_packages of the android_sdk extension, or set ADB to an adb path",
        ));
    }
    let adb = PathBuf::from(&flags.adb);
    if !adb.is_file() {
        return Err(missing(format!("no adb executable at {}", adb.display())));
    }
    let adb_args = adb_args(&flags).map_err(usage)?;
    let status = Command::new(&adb)
        .args(adb_args)
        .status()
        .map_err(|error| missing(format!("cannot run {}: {error}", adb.display())))?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1))
}
