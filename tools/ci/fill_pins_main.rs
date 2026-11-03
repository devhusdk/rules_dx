//! `fill_pins_bin <tag> [--check] [--dir DIR] [--pins PATH]` fills or, with `--check`, checks `cli/prebuilt/pins.bzl` against a release.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const REPO: &str = "devhusdk/rules_dx";

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
    let tag = args
        .next()
        .ok_or("usage: fill_pins_bin <tag> [--check] [--dir DIR] [--pins PATH]")?;
    let mut dir = None;
    let mut pins = PathBuf::from("cli/prebuilt/pins.bzl");
    let mut check = false;
    while let Some(flag) = args.next() {
        if flag == "--check" {
            check = true;
            continue;
        }
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--dir" => dir = Some(PathBuf::from(value)),
            "--pins" => pins = PathBuf::from(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let scratch = std::env::temp_dir().join(format!("dx-fill-pins-{}", std::process::id()));
    let text =
        std::fs::read_to_string(&pins).map_err(|e| format!("read {}: {e}", pins.display()))?;
    let dir = dir.unwrap_or_else(|| scratch.clone());
    let fetch = |dir: &Path| match dir == scratch {
        true => download(&tag, dir),
        false => Ok(()),
    };
    let verify = |manifest: &Path, bundle: &Path| verify(&tag, manifest, bundle);
    let message = if check {
        fill_pins::check(&tag, &dir, &text, || fetch(&dir), verify)
            .map(|()| format!("{} matches {tag}", pins.display()))
    } else {
        fetch(&dir)?;
        fill_pins::fill(&tag, &dir, &text, verify).and_then(|out| {
            std::fs::write(&pins, out).map_err(|e| format!("write {}: {e}", pins.display()))?;
            Ok(format!("wrote {}", pins.display()))
        })
    };
    let _ = std::fs::remove_dir_all(&scratch);
    message
}

fn download(tag: &str, dir: &Path) -> Result<(), String> {
    let mut command = Command::new("gh");
    command
        .args(["release", "download", tag, "--repo", REPO, "--dir"])
        .arg(dir);
    for name in [fill_pins::MANIFEST, fill_pins::BUNDLE] {
        command.args(["--pattern", name]);
    }
    run_command(&mut command)
}

fn verify(tag: &str, manifest: &Path, bundle: &Path) -> Result<(), String> {
    let mut command = Command::new("gh");
    command
        .args(["attestation", "verify"])
        .arg(manifest)
        .arg("--bundle")
        .arg(bundle)
        .args(["--repo", REPO])
        .args([
            "--cert-identity",
            &format!("https://github.com/{REPO}/.github/workflows/release.yml@refs/tags/{tag}"),
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
    let output = command.output().map_err(|e| format!("run gh: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "gh failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}
