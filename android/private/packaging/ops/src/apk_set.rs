//! Builds the split APKs one App Bundle installs on one device spec.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{Captured, Failure, Outcome, Scratch, apks_in, capture, reject, usage};

pub const PREFIX: &str = "dx-android-apk-set";

/// The first bundletool error line, without its tag.
pub fn reason(log: &str) -> &str {
    log.lines()
        .find_map(|line| {
            let rest = line.strip_prefix("[BT:")?;
            let (_, rest) = rest.split_once(']')?;
            rest.strip_prefix(" Error: ")
        })
        .unwrap_or_default()
}

fn rejected(ran: &Captured, spec: &Path, aab: &Path, what: String) -> Failure {
    let why = reason(&ran.text);
    if ran.text.contains("IncompatibleDeviceException") {
        return reject(format!(
            "device spec {} does not match {}: {why}",
            spec.display(),
            aab.display()
        ));
    }
    reject(format!("{what}: {why}"))
}

pub fn run(args: &[String]) -> Outcome {
    let [bundletool, aab, spec, keystore, apks, out_dir] = args else {
        return Err(usage(
            "usage: apk-set <bundletool> <aab> <device_spec.json> <keystore> <out.apks> <out_dir>",
        ));
    };
    let [bundletool, aab, spec, keystore, apks, out_dir] =
        [bundletool, aab, spec, keystore, apks, out_dir].map(PathBuf::from);
    for (path, what) in [
        (&bundletool, "bundletool"),
        (&aab, "app bundle"),
        (&spec, "device spec"),
        (&keystore, "debug keystore"),
    ] {
        if !path.is_file() {
            return Err(usage(format!("no {what} at {}", path.display())));
        }
    }
    let work = Scratch::beside(&apks)
        .map_err(|error| reject(format!("cannot create a scratch directory: {error}")))?;
    let tmp = format!("--jvm_flag=-Djava.io.tmpdir={}", work.path().display());
    let built = capture(
        Command::new(&bundletool)
            .arg(&tmp)
            .arg("build-apks")
            .arg(format!("--bundle={}", aab.display()))
            .arg(format!("--device-spec={}", spec.display()))
            .arg(format!("--ks={}", keystore.display()))
            .args([
                "--ks-pass=pass:android",
                "--ks-key-alias=androiddebugkey",
                "--key-pass=pass:android",
                "--overwrite",
            ])
            .arg(format!("--output={}", apks.display())),
    );
    if !built.success() {
        let what = format!("bundletool build-apks rejects {}", aab.display());
        return Err(rejected(&built, &spec, &aab, what));
    }
    std::fs::create_dir_all(&out_dir)
        .map_err(|error| reject(format!("cannot create {}: {error}", out_dir.display())))?;
    let extracted = capture(
        Command::new(&bundletool)
            .arg(&tmp)
            .arg("extract-apks")
            .arg(format!("--apks={}", apks.display()))
            .arg(format!("--device-spec={}", spec.display()))
            .arg(format!("--output-dir={}", out_dir.display())),
    );
    if !extracted.success() {
        let what = format!("bundletool extract-apks rejects {}", apks.display());
        return Err(rejected(&extracted, &spec, &aab, what));
    }
    if apks_in(&out_dir).is_empty() {
        return Err(reject(format!(
            "device spec {} selects no APK from {}",
            spec.display(),
            aab.display()
        )));
    }
    Ok(0)
}
