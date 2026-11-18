//! `fill_pins_bin <tag> [--check [--filled]] [--dir DIR] [--pins PATH] [--repo OWNER/NAME] [--manifest FILE]...`
//! fills or, with `--check`, checks `cli/prebuilt/pins.bzl` against a release.
//! Repeated `--manifest` fills from local manifests without downloading or verifying them;
//! together they must list every asset in `cli/prebuilt/tools.bzl` and nothing else.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use dx_process::capture::run_checked;

const GH_TIMEOUT: Duration = Duration::from_secs(600);

const USAGE: &str = "usage: fill_pins_bin <tag> [--check [--filled]] [--dir DIR] [--pins PATH] [--repo OWNER/NAME] [--manifest FILE]...";

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
        std::env::set_current_dir(&dir)
            .map_err(|e| format!("cannot enter {}: {e}", dir.display()))?;
    }
    let mut args = std::env::args().skip(1);
    let tag = args.next().ok_or(USAGE)?;
    let mut repo = "devhusdk/rules_dx".to_owned();
    let mut manifests = Vec::new();
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
            "--manifest" => manifests.push(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if filled && !check {
        return Err("--filled only works with --check".to_owned());
    }
    if !manifests.is_empty() && (check || dir.is_some()) {
        return Err("--manifest cannot be combined with --check or --dir".to_owned());
    }
    let text =
        std::fs::read_to_string(&pins).map_err(|e| format!("read {}: {e}", pins.display()))?;
    if !manifests.is_empty() {
        let texts = manifests
            .iter()
            .map(|path| Ok((path.display().to_string(), fill_pins::read(path)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let out = fill_pins::fill_from_manifests(&tag, &texts, &text, &repo)?;
        std::fs::write(&pins, out).map_err(|e| format!("write {}: {e}", pins.display()))?;
        return Ok(format!("wrote {}", pins.display()));
    }
    let scratch = tempfile::Builder::new()
        .prefix("dx-fill-pins-")
        .tempdir()
        .map_err(|e| format!("create scratch dir: {e}"))?;
    let dir = dir.unwrap_or_else(|| scratch.path().to_path_buf());
    let fetch = |dir: &Path| {
        if dir == scratch.path() {
            download(&tag, &repo, dir)
        } else {
            Ok(())
        }
    };
    let verify = |manifest: &Path, bundle: &Path| verify(&tag, &repo, manifest, bundle);
    if check {
        fill_pins::check(&tag, &dir, &text, &repo, filled, || fetch(&dir), verify)
            .map(|()| format!("{} matches {tag}", pins.display()))
    } else {
        fetch(&dir)?;
        fill_pins::fill(&tag, &dir, &text, &repo, verify).and_then(|out| {
            std::fs::write(&pins, out).map_err(|e| format!("write {}: {e}", pins.display()))?;
            Ok(format!("wrote {}", pins.display()))
        })
    }
}

fn download(tag: &str, repo: &str, dir: &Path) -> Result<(), String> {
    let mut command = Command::new("gh");
    command
        .args(["release", "download", tag, "--repo", repo, "--dir"])
        .arg(dir);
    for name in fill_pins::MANIFESTS.iter().chain([&fill_pins::BUNDLE]) {
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
