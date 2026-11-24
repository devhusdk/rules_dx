#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::{Path, PathBuf};

use dx_lock::{RESOLVER_TIMEOUT, Scratch};

pub struct Request {
    pub dir: PathBuf,
    pub manifest: String,
    pub lock: String,
    pub check: bool,
    pub path: Option<PathBuf>,
    pub upgrade: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Fresh,
    Written,
    Stale,
}

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

pub const MARKER: &str = "# dx inputs sha256: ";

fn recorded(lock: &str) -> Option<&str> {
    lock.lines().find_map(|line| line.strip_prefix(MARKER))
}

fn without_marker(lock: &str) -> String {
    lock.split_inclusive('\n')
        .filter(|line| !line.starts_with(MARKER))
        .collect()
}

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

/// # Errors
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

pub const UPGRADE_ALL: &str = "--upgrade-all";

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub dart: PathBuf,
    pub chmod: PathBuf,
    pub args: dx_lock::Args,
    pub upgrade: Option<Vec<String>>,
}

/// # Errors
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
