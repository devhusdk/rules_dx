//! Runs `pub get` or `pub upgrade` of the pinned Dart SDK on a copy of a set and writes its
//! lockfile with a marker that records the manifest digest.
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
    /// The `PATH` of `pub`, which runs `chmod` when it unpacks a package with executables.
    pub path: Option<PathBuf>,
    /// Runs `pub upgrade` on these packages, or on every package when empty, instead of `pub get`.
    pub upgrade: Option<Vec<String>>,
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

/// The `dart` arguments that run `pub get`, or `pub upgrade` of `upgrade`.
#[must_use]
pub fn pub_args(upgrade: Option<&[String]>) -> Vec<&str> {
    let mut args = vec!["--suppress-analytics", "pub"];
    match upgrade {
        None => args.push("get"),
        Some(names) => {
            args.push("upgrade");
            args.extend(names.iter().map(String::as_str));
        }
    }
    args.push("--no-precompile");
    args
}

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
/// Without `upgrade`, a lock whose marker matches the manifest digest is fresh without running
/// `dart`.
///
/// # Errors
///
/// Fails when the manifest cannot be read, `pub` fails, the request both checks and upgrades, or
/// the lock cannot be written.
pub fn lock(dart: &Path, request: &Request) -> Result<Outcome, String> {
    let manifest_path = request.dir.join(&request.manifest);
    let manifest = std::fs::read(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let digest = dx_digest::sha256_hex(&manifest);
    let lock_path = request.dir.join(&request.lock);
    let current = std::fs::read_to_string(&lock_path).ok();
    let upgrade = request.upgrade.as_deref();
    if request.check && upgrade.is_some() {
        return Err("--check and --upgrade cannot be combined".to_owned());
    }
    if upgrade.is_none() && current.as_deref().and_then(recorded) == Some(digest.as_str()) {
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
    let args = pub_args(upgrade);
    let verb = if upgrade.is_some() { "upgrade" } else { "get" };
    command.args(&args);
    Scratch::run(&mut command, RESOLVER_TIMEOUT)
        .map_err(|error| format!("dart pub {verb} failed: {error}"))?;
    let resolved_path = scratch.set().join(&request.lock);
    let resolved = std::fs::read_to_string(&resolved_path)
        .map_err(|error| format!("dart pub {verb} wrote no {}: {error}", request.lock))?;
    let resolved = with_marker(&without_marker(&resolved), &digest);
    std::fs::write(&lock_path, &resolved)
        .map_err(|error| format!("{}: {error}", lock_path.display()))?;
    Ok(Outcome::Written)
}

/// The flag that upgrades every package of the set.
pub const UPGRADE_ALL: &str = "--upgrade-all";

/// The parsed arguments of the step.
#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    /// The `dart` executable.
    pub dart: PathBuf,
    /// The `chmod` executable.
    pub chmod: PathBuf,
    /// The shared lock step flags.
    pub args: dx_lock::Args,
    /// The packages to upgrade; empty upgrades every package.
    pub upgrade: Option<Vec<String>>,
}

/// Parses `--dart=<path> --chmod=<path>`, the shared lock step flags and `--upgrade-all`.
///
/// # Errors
///
/// Fails on a missing tool path, a bad shared flag, or `--upgrade-all` beside `--upgrade`.
pub fn parse(args: &[String]) -> Result<Parsed, dx_lock::Error> {
    let (paths, rest) = dx_lock::tool_paths(args, &["--dart", "--chmod"])?;
    let mut paths = paths.into_iter();
    let (Some(dart), Some(chmod)) = (paths.next(), paths.next()) else {
        return Err(dx_lock::Error::new("--dart and --chmod are required"));
    };
    let all = rest.iter().any(|arg| arg == UPGRADE_ALL);
    let args = dx_lock::Args::parse(rest.into_iter().filter(|arg| arg != UPGRADE_ALL))?;
    let upgrade = match (all, args.upgrade.is_empty()) {
        (true, false) => {
            return Err(dx_lock::Error::new(
                "--upgrade-all and --upgrade cannot be combined",
            ));
        }
        (true, true) => Some(Vec::new()),
        (false, false) => Some(args.upgrade.clone()),
        (false, true) => None,
    };
    Ok(Parsed {
        dart,
        chmod,
        args,
        upgrade,
    })
}

/// Runs the pub lock step on `args`, the arguments after `dx dart_lock`:
/// `--dart=<path> --chmod=<path> --dir <set directory> [--check] [--upgrade <name>]... [--upgrade-all]`.
/// 0 when the lock is fresh or written, 1 when stale, 2 on failure.
pub fn run_process(args: &[String]) -> i32 {
    let Parsed {
        dart,
        chmod,
        args,
        upgrade,
    } = match parse(args) {
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
        upgrade,
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
