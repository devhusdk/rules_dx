//! Runs `pub get` of the pinned Dart SDK on a copy of a set and writes its lockfile.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::{Path, PathBuf};

use dx_lock::{RESOLVER_TIMEOUT, Scratch};

/// One lock step for one dependency set.
pub struct Request {
    /// The set directory.
    pub dir: PathBuf,
    /// The manifest file name.
    pub manifest: String,
    /// The lock file name.
    pub lock: String,
    /// Compare only.
    pub check: bool,
}

/// What the lock step did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The lock already matched.
    Fresh,
    /// The lock was written.
    Written,
    /// `check` found the lock missing or different.
    Stale,
}

/// The `pub` arguments the step runs.
pub const PUB_ARGS: [&str; 4] = ["--suppress-analytics", "pub", "get", "--no-precompile"];

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::copy(from, to)
        .map(drop)
        .map_err(|error| format!("{}: {error}", from.display()))
}

/// Resolves the set's manifest with `dart` and writes, or with `check` compares, its lock.
///
/// # Errors
///
/// Fails when the manifest cannot be read, `pub get` fails, or the lock cannot be written.
pub fn lock(dart: &Path, request: &Request) -> Result<Outcome, String> {
    let scratch = Scratch::new().map_err(|error| error.to_string())?;
    copy(
        &request.dir.join(&request.manifest),
        &scratch.set().join(&request.manifest),
    )?;
    let lock_path = request.dir.join(&request.lock);
    let current = std::fs::read(&lock_path).ok();
    if let Some(bytes) = &current {
        std::fs::write(scratch.set().join(&request.lock), bytes)
            .map_err(|error| format!("{}: {error}", request.lock))?;
    }
    let mut command = scratch.command(dart);
    command.env("PUB_CACHE", scratch.tmp().join("pub-cache"));
    if let Some(root) = std::env::var_os("SYSTEMROOT") {
        command.env("SYSTEMROOT", root);
    }
    command.args(PUB_ARGS);
    Scratch::run(&mut command, RESOLVER_TIMEOUT)
        .map_err(|error| format!("dart pub get failed: {error}"))?;
    let resolved_path = scratch.set().join(&request.lock);
    let resolved = std::fs::read(&resolved_path)
        .map_err(|error| format!("dart pub get wrote no {}: {error}", request.lock))?;
    if current.as_deref() == Some(resolved.as_slice()) {
        Ok(Outcome::Fresh)
    } else if request.check {
        Ok(Outcome::Stale)
    } else {
        std::fs::write(&lock_path, &resolved)
            .map_err(|error| format!("{}: {error}", lock_path.display()))?;
        Ok(Outcome::Written)
    }
}

fn parse(args: &[String]) -> Result<(PathBuf, dx_lock::Args), dx_lock::Error> {
    let (paths, rest) = dx_lock::tool_paths(args, &["--dart"])?;
    let Some(dart) = paths.into_iter().next() else {
        return Err(dx_lock::Error::new("--dart is required"));
    };
    let args = dx_lock::Args::parse(rest)?;
    if !args.upgrade.is_empty() {
        return Err(dx_lock::Error::new("--upgrade is not supported"));
    }
    Ok((dart, args))
}

/// Runs the pub lock step on `args`, the arguments after `dx dart_lock`:
/// `--dart=<path> --dir <set directory> [--check]`. 0 when the lock is fresh or written, 1 when
/// stale, 2 on failure.
pub fn run_process(args: &[String]) -> i32 {
    let (dart, args) = match parse(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("dart_lock: {error}");
            return 2;
        }
    };
    let Some(hub) = dx_families::hub("pub") else {
        return dx_lock::missing_hub("pub");
    };
    let request = Request {
        dir: args.dir,
        manifest: hub.manifest.clone(),
        lock: hub.lock.clone().unwrap_or_default(),
        check: args.check,
    };
    match lock(&dart, &request) {
        Ok(Outcome::Fresh | Outcome::Written) => 0,
        Ok(Outcome::Stale) => {
            eprintln!("{} is stale; run dx generate --apply", request.lock);
            1
        }
        Err(error) => {
            eprintln!("{}: {error}", request.lock);
            2
        }
    }
}
