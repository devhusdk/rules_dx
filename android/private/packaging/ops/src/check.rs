//! Checks that a package is an unsigned APK or a valid App Bundle.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{Captured, Outcome, Scratch, capture, reject, usage, zip_names};

pub const PREFIX: &str = "dx-android-check";

const USAGE: &str =
    "usage: check <bundletool> <package> [--abi <abi>]... [--resource <type/name>]...";

/// What a check requires of an App Bundle.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Wants {
    pub abis: Vec<String>,
    pub resources: Vec<String>,
}

/// Reads the `--abi` and `--resource` flags.
pub fn parse(args: &[String]) -> Result<Wants, String> {
    let mut wants = Wants::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let (list, error) = match flag.as_str() {
            "--abi" => (&mut wants.abis, "--abi needs an ABI name"),
            "--resource" => (&mut wants.resources, "--resource needs a type/name"),
            other => return Err(format!("unknown argument '{other}'")),
        };
        match rest.next() {
            Some(value) if !value.is_empty() => list.push(value.clone()),
            _ => return Err(error.to_owned()),
        }
    }
    Ok(wants)
}

/// Whether `name` is a v1 signature file.
pub fn signature_entry(name: &str) -> bool {
    name.strip_prefix("META-INF/").is_some_and(|file| {
        !file.contains('/')
            && file
                .rsplit_once('.')
                .is_some_and(|(stem, end)| !stem.is_empty() && ["RSA", "DSA", "EC", "SF"].contains(&end))
    })
}

/// Whether `name` is a native library of `abi` in the base module.
pub fn base_library(name: &str, abi: &str) -> bool {
    name.strip_prefix("base/lib/")
        .and_then(|rest| rest.strip_prefix(abi))
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|file| !file.contains('/') && file.len() > 3 && file.ends_with(".so"))
}

/// Whether every entry of the zip file reads back with its recorded checksum.
pub fn entries_intact(package: &Path) -> bool {
    let Ok(file) = std::fs::File::open(package) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    (0..archive.len()).all(|index| {
        archive
            .by_index(index)
            .is_ok_and(|mut entry| std::io::copy(&mut entry, &mut std::io::sink()).is_ok())
    })
}

fn scratch() -> Result<Scratch, crate::util::Failure> {
    let root = ["TEST_TMPDIR", "TMPDIR"]
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
        .ok_or_else(|| usage("TEST_TMPDIR or TMPDIR names the scratch directory"))?;
    Scratch::beside(&PathBuf::from(root).join("check"))
        .map_err(|error| usage(format!("cannot create a scratch directory: {error}")))
}

fn check_apk(package: &Path, wants: &Wants) -> Outcome {
    if !wants.abis.is_empty() || !wants.resources.is_empty() {
        return Err(usage("--abi and --resource apply to App Bundles only"));
    }
    let shown = package.display();
    let bytes = std::fs::read(package).unwrap_or_default();
    let names = zip_names(&bytes)
        .ok_or_else(|| reject(format!("{shown} is not a readable zip package")))?;
    if !entries_intact(package) {
        return Err(reject(format!("{shown} has corrupt entries")));
    }
    if !names.iter().any(|name| name == "AndroidManifest.xml") {
        return Err(reject(format!("{shown} has no AndroidManifest.xml")));
    }
    if names.iter().any(|name| signature_entry(name)) {
        return Err(reject(format!(
            "{shown} already carries a signature: release signing needs the unsigned package"
        )));
    }
    let marker = b"APK Sig Block 42";
    if bytes.windows(marker.len()).any(|window| window == marker) {
        return Err(reject(format!(
            "{shown} already carries an APK signing block: release signing needs the unsigned package"
        )));
    }
    println!("{PREFIX}: {shown} is an unsigned package ready to sign");
    Ok(0)
}

fn check_aab(bundletool: &Path, package: &Path, wants: &Wants) -> Outcome {
    let shown = package.display();
    let work = scratch()?;
    let bundle = format!("--bundle={shown}");
    let run = |args: &[&str]| -> Captured {
        capture(
            Command::new(bundletool)
                .arg(format!("--jvm_flag=-Djava.io.tmpdir={}", work.path().display()))
                .args(args),
        )
    };
    let validated = run(&["validate", &bundle]);
    if !validated.success() {
        return Err(reject(format!(
            "bundletool validate rejects {shown}: {}",
            validated.first_reason()
        )));
    }
    if !run(&["dump", "manifest", &bundle, "--module=base"]).success() {
        return Err(reject(format!("{shown} has no readable base module manifest")));
    }
    let names = std::fs::read(package)
        .ok()
        .and_then(|bytes| zip_names(&bytes))
        .ok_or_else(|| reject(format!("{shown} is not a readable zip bundle")))?;
    for abi in &wants.abis {
        if !names.iter().any(|name| base_library(name, abi)) {
            return Err(reject(format!("{shown} packages no base/lib/{abi}/*.so")));
        }
    }
    for resource in &wants.resources {
        let found = run(&["dump", "resources", &bundle, &format!("--resource={resource}")]);
        let name = resource.split_once('/').map_or(resource.as_str(), |(_, name)| name);
        if !found.success() || !found.text.contains(name) {
            return Err(reject(format!("{shown} does not resolve resource {resource}")));
        }
    }
    println!("{PREFIX}: {shown} is a valid app bundle");
    Ok(0)
}

pub fn run(args: &[String]) -> Outcome {
    let [bundletool, package, rest @ ..] = args else {
        return Err(usage(USAGE));
    };
    let wants = parse(rest).map_err(usage)?;
    let bundletool = PathBuf::from(bundletool);
    let package = PathBuf::from(package);
    if !bundletool.is_file() {
        return Err(usage(format!("no bundletool at {}", bundletool.display())));
    }
    if !package.is_file() {
        return Err(usage(format!("no package at {}", package.display())));
    }
    match package.extension().and_then(|end| end.to_str()) {
        Some("apk") => check_apk(&package, &wants),
        Some("aab") => check_aab(&bundletool, &package, &wants),
        _ => Err(usage("package must end in .apk or .aab")),
    }
}
