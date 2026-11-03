//! `fill_pins_bin <tag> [--dir DIR] [--pins PATH]` fills `cli/prebuilt/pins.bzl` from a release.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const REPO: &str = "devhusdk/rules_dx";

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("dx-ci: wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("dx-ci: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("BUILD_WORKING_DIRECTORY") {
        std::env::set_current_dir(&dir).map_err(|e| format!("cannot enter {dir:?}: {e}"))?;
    }
    let mut args = std::env::args().skip(1);
    let tag = args
        .next()
        .ok_or("usage: fill_pins_bin <tag> [--dir DIR] [--pins PATH]")?;
    let mut dir = None;
    let mut pins = PathBuf::from("cli/prebuilt/pins.bzl");
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--dir" => dir = Some(PathBuf::from(value)),
            "--pins" => pins = PathBuf::from(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let scratch = std::env::temp_dir().join(format!("dx-fill-pins-{}", std::process::id()));
    let dir = match dir {
        Some(dir) => dir,
        None => {
            download(&tag, &scratch)?;
            scratch.clone()
        }
    };
    let text =
        std::fs::read_to_string(&pins).map_err(|e| format!("read {}: {e}", pins.display()))?;
    let result = fill_pins::fill(&tag, &dir, &text, |manifest, bundle| {
        verify(&tag, manifest, bundle)
    });
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::write(&pins, result?).map_err(|e| format!("write {}: {e}", pins.display()))?;
    Ok(pins)
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
