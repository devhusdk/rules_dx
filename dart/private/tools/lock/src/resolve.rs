//! Runs `pub get` of the pinned Dart SDK on a copy of a set and writes its lockfile with a
//! marker that records the manifest digest.
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
    /// The `PATH` of `pub get`, which runs `chmod` when it unpacks a package with executables.
    pub path: Option<PathBuf>,
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

/// The lock line that records the sha256 of the manifest the lock was resolved from.
pub const MARKER: &str = "# dx inputs sha256: ";

fn recorded(lock: &str) -> Option<&str> {
    lock.lines().find_map(|line| line.strip_prefix(MARKER))
}

fn without_marker(lock: &str) -> String {
    lock.split_inclusive('\n')
        .filter(|line| !line.starts_with(MARKER))
        .collect()
}

/// Places the marker after the lock's leading comment lines.
fn with_marker(lock: &str, digest: &str) -> String {
    let mut out = String::with_capacity(lock.len() + MARKER.len() + digest.len() + 1);
    let mut placed = false;
    for line in lock.split_inclusive('\n') {
        if !placed && !line.starts_with('#') {
            out.push_str(&format!("{MARKER}{digest}\n"));
            placed = true;
        }
        out.push_str(line);
    }
    if !placed {
        out.push_str(&format!("{MARKER}{digest}\n"));
    }
    out
}

/// Writes the set's lock with `dart`, or with `check` compares the lock's marker to the manifest.
///
/// A lock whose marker matches the manifest digest is fresh without running `dart`.
///
/// # Errors
///
/// Fails when the manifest cannot be read, `pub get` fails, or the lock cannot be written.
pub fn lock(dart: &Path, request: &Request) -> Result<Outcome, String> {
    let manifest_path = request.dir.join(&request.manifest);
    let manifest = std::fs::read(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let digest = dx_digest::sha256_hex(&manifest);
    let lock_path = request.dir.join(&request.lock);
    let current = std::fs::read_to_string(&lock_path).ok();
    if current.as_deref().and_then(recorded) == Some(digest.as_str()) {
        return Ok(Outcome::Fresh);
    }
    if request.check {
        return Ok(Outcome::Stale);
    }
    let scratch = Scratch::new().map_err(|error| error.to_string())?;
    std::fs::write(scratch.set().join(&request.manifest), &manifest)
        .map_err(|error| format!("{}: {error}", request.manifest))?;
    if let Some(text) = &current {
        std::fs::write(scratch.set().join(&request.lock), without_marker(text))
            .map_err(|error| format!("{}: {error}", request.lock))?;
    }
    let mut command = scratch.command(dart);
    command.env("PUB_CACHE", scratch.tmp().join("pub-cache"));
    if let Some(path) = &request.path {
        command.env("PATH", path);
    }
    if let Some(root) = std::env::var_os("SYSTEMROOT") {
        command.env("SYSTEMROOT", root);
    }
    command.args(PUB_ARGS);
    Scratch::run(&mut command, RESOLVER_TIMEOUT)
        .map_err(|error| format!("dart pub get failed: {error}"))?;
    let resolved_path = scratch.set().join(&request.lock);
    let resolved = std::fs::read_to_string(&resolved_path)
        .map_err(|error| format!("dart pub get wrote no {}: {error}", request.lock))?;
    let resolved = with_marker(&without_marker(&resolved), &digest);
    std::fs::write(&lock_path, &resolved)
        .map_err(|error| format!("{}: {error}", lock_path.display()))?;
    Ok(Outcome::Written)
}

fn parse(args: &[String]) -> Result<(PathBuf, PathBuf, dx_lock::Args), dx_lock::Error> {
    let (paths, rest) = dx_lock::tool_paths(args, &["--dart", "--chmod"])?;
    let mut paths = paths.into_iter();
    let (Some(dart), Some(chmod)) = (paths.next(), paths.next()) else {
        return Err(dx_lock::Error::new("--dart and --chmod are required"));
    };
    let args = dx_lock::Args::parse(rest)?;
    if !args.upgrade.is_empty() {
        return Err(dx_lock::Error::new("--upgrade is not supported"));
    }
    Ok((dart, chmod, args))
}

/// Runs the pub lock step on `args`, the arguments after `dx dart_lock`:
/// `--dart=<path> --chmod=<path> --dir <set directory> [--check]`. 0 when the lock is fresh or written, 1 when
/// stale, 2 on failure.
pub fn run_process(args: &[String]) -> i32 {
    let (dart, chmod, args) = match parse(args) {
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
        path: chmod.parent().map(Path::to_path_buf),
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
