//! Forwards a repin request to dx update, which owns the pinned versions.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let Some(workspace) =
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty())
    else {
        eprintln!("repin-all: BUILD_WORKSPACE_DIRECTORY is unset; run it with `bazel run //tools:repin-all`");
        return ExitCode::from(2);
    };
    let Some(dx) = resolve_dx() else {
        eprintln!(
            "repin-all: cannot locate dx in runfiles; run it with `bazel run //tools:repin-all`"
        );
        return ExitCode::from(2);
    };
    let user_args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut command = Command::new(&dx);
    command.arg("update");
    // The outer `bazel run` may use a non-default output base; hand it to dx so its bazel calls
    // reuse that server instead of starting a second one.
    if !names_output_base(&user_args) {
        if let Some(output_base) = dx_process::run_output_base() {
            let mut option = OsString::from("--bazel-startup-option=--output_base=");
            option.push(output_base);
            command.arg(option);
        }
    }
    command
        .args(&user_args)
        .current_dir(&workspace)
        .env_remove("RUNFILES_DIR")
        .env_remove("RUNFILES_MANIFEST_FILE")
        .env_remove("JAVA_RUNFILES");
    let code = match command.status() {
        Ok(done) => done.code().unwrap_or(1),
        Err(error) => {
            eprintln!("repin-all: cannot run {}: {error}", dx.display());
            1
        }
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn resolve_dx() -> Option<PathBuf> {
    let rel = std::env::var("DX_REPIN_DX")
        .ok()
        .filter(|rel| !rel.is_empty())?;
    let runfiles = runfiles::Runfiles::create().ok()?;
    runfiles.rlocation(rel).filter(|path| path.exists())
}

/// Reports whether the user already passed a `--bazel-startup-option` naming `output_base`.
fn names_output_base(args: &[OsString]) -> bool {
    let mut previous_is_flag = false;
    for arg in args {
        let text = arg.to_string_lossy();
        if let Some(value) = text.strip_prefix("--bazel-startup-option=") {
            if value.contains("output_base") {
                return true;
            }
        } else if previous_is_flag && text.contains("output_base") {
            return true;
        }
        previous_is_flag = text == "--bazel-startup-option";
    }
    false
}
