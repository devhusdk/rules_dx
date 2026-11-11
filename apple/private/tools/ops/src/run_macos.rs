//! Unpacks a built macOS app and runs it.

use std::path::Path;

use crate::util::{Env, Outcome, Scratch, exit_code, missing, reject, usage};
use crate::xcode::{DITTO, PLUTIL, Tools};

pub const PREFIX: &str = "apple_macos_app";

/// Runs the app in the zip at `args[0]` with the remaining arguments.
pub fn run(tools: &Tools, args: &[String], env: Env) -> Outcome {
    let Some((zipped, rest)) = args.split_first() else {
        return Err(usage("run the app with bazel run"));
    };
    if !Path::new(zipped).is_file() {
        return Err(usage(format!("{zipped} is not in the runfiles")));
    }
    let work = Scratch::new(env, "macos-app")?;
    let target = work.path().to_string_lossy().into_owned();
    let unpacked = tools.tool(DITTO, &["-x", "-k", zipped, &target]);
    match unpacked.code {
        Some(0) => {}
        None => {
            return Err(missing(format!(
                "cannot start {DITTO}: {}; running the app needs macOS",
                unpacked.stderr.trim()
            )));
        }
        Some(code) => {
            return Err(crate::util::Failure {
                code: exit_code(Some(code)),
                message: format!("{zipped} does not unpack: {}", unpacked.stderr.trim()),
            });
        }
    }
    let mut apps: Vec<_> = std::fs::read_dir(work.path())
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|end| end == "app"))
                .collect()
        })
        .unwrap_or_default();
    apps.sort();
    let Some(app) = apps.first() else {
        return Err(reject("the archive holds no .app"));
    };
    let plist = app.join("Contents/Info.plist").to_string_lossy().into_owned();
    let read = tools.tool(
        PLUTIL,
        &["-extract", "CFBundleExecutable", "raw", "-o", "-", &plist],
    );
    let executable = read.stdout.trim();
    if !read.success() || executable.is_empty() {
        return Err(reject(format!(
            "{plist} names no CFBundleExecutable: {}",
            read.stderr.trim()
        )));
    }
    let binary = app.join("Contents/MacOS").join(executable);
    let binary = binary.to_string_lossy().into_owned();
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    Ok(exit_code(tools.run_program(&binary, &rest)))
}
