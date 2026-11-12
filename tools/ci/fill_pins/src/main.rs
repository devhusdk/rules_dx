//! `fill_pins_bin <tag> [--check [--filled]] [--dir DIR] [--pins PATH] [--repo OWNER/NAME] [--manifest FILE --gazelle-manifest FILE]`
//! fills or, with `--check`, checks `cli/prebuilt/pins.bzl` against a release.
//! `--manifest` and `--gazelle-manifest` fill from local manifests without downloading or verifying them.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use dx_process::capture::run_checked;

const GH_TIMEOUT: Duration = Duration::from_secs(600);

const USAGE: &str = "usage: fill_pins_bin <tag> [--check [--filled]] [--dir DIR] [--pins PATH] [--repo OWNER/NAME] [--manifest FILE --gazelle-manifest FILE]";

fn main() -> ExitCode {
    match run() {
        Ok(message) => {
            println!("dx-ci: {message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("dx-ci: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    if let Some(dir) = std::env::var_os("BUILD_WORKING_DIRECTORY") {
        std::env::set_current_dir(&dir).map_err(|e| format!("cannot enter {dir:?}: {e}"))?;
    }
    let mut args = std::env::args().skip(1);
    let tag = args.next().ok_or(USAGE)?;
    let mut repo = "devhusdk/rules_dx".to_owned();
    let mut manifest = None;
    let mut gazelle_manifest = None;
    let mut filled = false;
    let mut dir = None;
    let mut pins = PathBuf::from("cli/prebuilt/pins.bzl");
    let mut check = false;
    while let Some(flag) = args.next() {
        if flag == "--check" {
            check = true;
            continue;
        }
        if flag == "--filled" {
            filled = true;
            continue;
        }
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--dir" => dir = Some(PathBuf::from(value)),
            "--pins" => pins = PathBuf::from(value),
            "--repo" => repo = value,
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--gazelle-manifest" => gazelle_manifest = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if filled && !check {
        return Err("--filled only works with --check".to_owned());
    }
    if manifest.is_some() != gazelle_manifest.is_some() {
        return Err("--manifest and --gazelle-manifest go together".to_owned());
    }
    if manifest.is_some() && (check || dir.is_some()) {
        return Err("--manifest cannot be combined with --check or --dir".to_owned());
    }
    let scratch = std::env::temp_dir().join(format!("dx-fill-pins-{}", std::process::id()));
    let text =
        std::fs::read_to_string(&pins).map_err(|e| format!("read {}: {e}", pins.display()))?;
    if let (Some(manifest), Some(gazelle)) = (manifest, gazelle_manifest) {
        let read = |path: &Path| {
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
        };
        let out =
            fill_pins::fill_from_manifest(&tag, &read(&manifest)?, &read(&gazelle)?, &text, &repo)?;
        std::fs::write(&pins, out).map_err(|e| format!("write {}: {e}", pins.display()))?;
        return Ok(format!("wrote {}", pins.display()));
    }
    let dir = dir.unwrap_or_else(|| scratch.clone());
    let fetch = |dir: &Path| match dir == scratch {
        true => download(&tag, &repo, dir),
        false => Ok(()),
    };
    let verify = |manifest: &Path, bundle: &Path| verify(&tag, &repo, manifest, bundle);
    let message = if check {
        fill_pins::check(&tag, &dir, &text, &repo, filled, || fetch(&dir), verify)
            .map(|()| format!("{} matches {tag}", pins.display()))
    } else {
        fetch(&dir)?;
        fill_pins::fill(&tag, &dir, &text, &repo, verify).and_then(|out| {
            std::fs::write(&pins, out).map_err(|e| format!("write {}: {e}", pins.display()))?;
            Ok(format!("wrote {}", pins.display()))
        })
    };
    let _ = std::fs::remove_dir_all(&scratch);
    message
}

fn download(tag: &str, repo: &str, dir: &Path) -> Result<(), String> {
    let mut command = Command::new("gh");
    command
        .args(["release", "download", tag, "--repo", repo, "--dir"])
        .arg(dir);
    for name in [
        fill_pins::MANIFEST,
        fill_pins::GAZELLE_MANIFEST,
        fill_pins::BUNDLE,
    ] {
        command.args(["--pattern", name]);
    }
    run_command(&mut command)
}

fn verify(tag: &str, repo: &str, manifest: &Path, bundle: &Path) -> Result<(), String> {
    let mut command = Command::new("gh");
    command
        .args(["attestation", "verify"])
        .arg(manifest)
        .arg("--bundle")
        .arg(bundle)
        .args(["--repo", repo])
        .args([
            "--cert-identity",
            &format!("https://github.com/{repo}/.github/workflows/release.yml@refs/tags/{tag}"),
        ])
        .args([
            "--cert-oidc-issuer",
            "https://token.actions.githubusercontent.com",
        ])
        .args(["--source-ref", &format!("refs/tags/{tag}")])
        .arg("--deny-self-hosted-runners");
    run_command(&mut command)
}

fn run_command(command: &mut Command) -> Result<(), String> {
    run_checked(command, GH_TIMEOUT)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
