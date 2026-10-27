//! Verified install of a released dx archive into `DIR/versions/<tag>` with `DIR/bin` as the entry point.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dx_atomic_fs::commit::{acquire_commit_lock, publish_pointer_swap, ManagedRoot, PointerSwap};
use dx_atomic_fs::{classify, EntryKind, PointerKind};

use crate::trust::{self, Cell};

pub const RECEIPT_SCHEMA: &str = "dx-install/v1";
pub const RECEIPT_NAME: &str = "RECEIPT.json";
pub const VERSIONS_DIR: &str = "versions";
pub const BIN_DIR: &str = "bin";
const CURRENT_FILE: &str = "CURRENT";
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// What an install places; the receipt records its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payload {
    pub name: &'static str,
}

pub const DX_PAYLOAD: Payload = Payload { name: "dx" };

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallArgs {
    pub tag: String,
    pub archive: PathBuf,
    pub dir: Option<PathBuf>,
    pub apply: bool,
}

/// Process and environment access, faked in tests.
pub trait Host {
    fn cell(&self) -> Result<&'static Cell, String>;
    fn sha256(&self, path: &Path) -> io::Result<String>;
    fn gh_bin(&self) -> String;
    fn have(&self, bin: &str) -> bool;
    fn run(&self, argv: &[String]) -> Result<String, String>;
    fn on_path(&self, dir: &Path) -> bool;
    fn default_dir(&self) -> Result<PathBuf, String>;
}

pub struct SystemHost;

impl Host for SystemHost {
    fn cell(&self) -> Result<&'static Cell, String> {
        trust::host_cell()
    }

    fn sha256(&self, path: &Path) -> io::Result<String> {
        dx_digest::sha256_file_hex(path)
    }

    fn gh_bin(&self) -> String {
        std::env::var("DX_INSTALL_GH").unwrap_or_else(|_| "gh".to_owned())
    }

    fn have(&self, bin: &str) -> bool {
        crate::exe_available(bin)
    }

    fn run(&self, argv: &[String]) -> Result<String, String> {
        let Some((program, args)) = argv.split_first() else {
            return Err("empty command".to_owned());
        };
        let out = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("cannot run {program}: {e}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "{program} failed ({}): {}",
                out.status,
                stderr.trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn on_path(&self, dir: &Path) -> bool {
        std::env::var_os("PATH").is_some_and(|paths| path_lists(&paths, dir))
    }

    fn default_dir(&self) -> Result<PathBuf, String> {
        default_dir_from(cfg!(windows), &|key| std::env::var_os(key))
    }
}

fn path_lists(paths: &std::ffi::OsStr, dir: &Path) -> bool {
    let want = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    std::env::split_paths(paths).any(|entry| std::fs::canonicalize(&entry).unwrap_or(entry) == want)
}

/// `%LOCALAPPDATA%\dx` on Windows, else `$XDG_DATA_HOME/dx` or `~/.local/share/dx`.
pub fn default_dir_from(
    windows: bool,
    env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    let absolute = |key: &str| {
        env(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    if windows {
        return absolute("LOCALAPPDATA")
            .map(|base| base.join("dx"))
            .ok_or_else(|| "LOCALAPPDATA is not set; pass --dir".to_owned());
    }
    if let Some(base) = absolute("XDG_DATA_HOME") {
        return Ok(base.join("dx"));
    }
    absolute("HOME")
        .map(|home| home.join(".local").join("share").join("dx"))
        .ok_or_else(|| "HOME is not set; pass --dir".to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Fresh,
    Upgraded { from: String },
    Switched { from: String },
    NoOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub tag: String,
    pub cell: &'static str,
    pub dir: PathBuf,
    pub bin_dir: PathBuf,
    pub applied: bool,
    pub outcome: Outcome,
    pub checks: Vec<Check>,
    pub on_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallError {
    pub check: &'static str,
    pub message: String,
    pub checks: Vec<Check>,
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.check, self.message)
    }
}

impl std::error::Error for InstallError {}

struct Run {
    checks: Vec<Check>,
}

impl Run {
    fn ok(&mut self, name: &'static str, detail: String) {
        self.checks.push(Check { name, detail });
    }

    fn fail(&self, check: &'static str, message: impl Into<String>) -> InstallError {
        InstallError {
            check,
            message: message.into(),
            checks: self.checks.clone(),
        }
    }
}

/// Removes a directory tree on drop unless disarmed.
struct Cleanup(Option<PathBuf>);

impl Cleanup {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn nonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(
        "{}-{nanos:x}{:x}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// One installed version's receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub payload: String,
    pub tag: String,
    pub cell: String,
    pub sha256: String,
    pub entries: Vec<dx_archive::Member>,
}

impl Receipt {
    pub fn to_json(&self) -> String {
        let entries: Vec<serde_json::Value> = self
            .entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "name": entry.name,
                    "size": entry.size,
                    "executable": entry.executable,
                })
            })
            .collect();
        let value = serde_json::json!({
            "schema": RECEIPT_SCHEMA,
            "payload": self.payload,
            "tag": self.tag,
            "cell": self.cell,
            "sha256": self.sha256,
            "entries": entries,
        });
        format!("{value:#}\n")
    }

    pub fn parse(text: &str) -> Result<Receipt, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("receipt is not JSON: {e}"))?;
        let field = |key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("receipt has no {key:?}"))
        };
        let schema = field("schema")?;
        if schema != RECEIPT_SCHEMA {
            return Err(format!(
                "receipt schema {schema:?} is not {RECEIPT_SCHEMA:?}"
            ));
        }
        let mut entries = Vec::new();
        for entry in value
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "receipt has no \"entries\"".to_owned())?
        {
            let name = entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "receipt entry has no \"name\"".to_owned())?;
            entries.push(dx_archive::Member {
                name: name.to_owned(),
                size: entry
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                executable: entry
                    .get("executable")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            });
        }
        Ok(Receipt {
            payload: field("payload")?,
            tag: field("tag")?,
            cell: field("cell")?,
            sha256: field("sha256")?,
            entries,
        })
    }
}

fn read_receipt(version_dir: &Path) -> Result<Receipt, String> {
    let path = version_dir.join(RECEIPT_NAME);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Receipt::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The tag `bin` currently runs, if any.
fn current_tag(dir: &Path, cell: &Cell, windows: bool) -> Result<Option<String>, String> {
    if windows {
        return match std::fs::read_to_string(dir.join(BIN_DIR).join(CURRENT_FILE)) {
            Ok(text) => Ok(Some(text.trim().to_owned())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("cannot read the current version: {e}")),
        };
    }
    let entry = dir.join(BIN_DIR).join(cell.exe);
    match classify(&entry).map_err(|e| format!("cannot inspect {}: {e}", entry.display()))? {
        EntryKind::Missing => Ok(None),
        EntryKind::Link => {
            let target = std::fs::read_link(&entry)
                .map_err(|e| format!("cannot read {}: {e}", entry.display()))?;
            let parts: Vec<String> = target
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            match parts.as_slice() {
                [up, versions, tag, exe]
                    if up == ".." && versions == VERSIONS_DIR && exe == cell.exe =>
                {
                    Ok(Some(tag.clone()))
                }
                _ => Err(format!(
                    "{} points at {}, which dx install did not create; remove it first",
                    entry.display(),
                    target.display()
                )),
            }
        }
        EntryKind::File | EntryKind::Directory => Err(format!(
            "{} exists and is not a dx install link; remove it first",
            entry.display()
        )),
    }
}

/// Runs the install; without `apply` it checks everything and writes nothing under DIR.
pub fn install(args: &InstallArgs, host: &dyn Host) -> Result<Report, InstallError> {
    install_with(args, host, DX_PAYLOAD, cfg!(windows))
}

fn install_with(
    args: &InstallArgs,
    host: &dyn Host,
    payload: Payload,
    windows: bool,
) -> Result<Report, InstallError> {
    let mut run = Run { checks: Vec::new() };
    let version = trust::validate_tag(&args.tag).map_err(|e| run.fail("tag", e))?;
    run.ok("tag", args.tag.clone());

    let cell = host.cell().map_err(|e| run.fail("cell", e))?;
    let want = cell.archive_name();
    let archive_name = args
        .archive
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if archive_name != want {
        return Err(run.fail(
            "cell",
            format!("this host needs {want}, got {archive_name:?}"),
        ));
    }
    run.ok("cell", cell.name.to_owned());

    let dir = match &args.dir {
        Some(dir) => dir.clone(),
        None => host.default_dir().map_err(|e| run.fail("dir", e))?,
    };
    let dir = std::path::absolute(&dir)
        .map_err(|e| run.fail("dir", format!("cannot resolve {}: {e}", dir.display())))?;

    let assets = args.archive.parent().unwrap_or(Path::new("."));
    let manifest_path = assets.join(trust::MANIFEST_NAME);
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| {
        run.fail(
            "sha256",
            format!("cannot read {}: {e}", manifest_path.display()),
        )
    })?;
    let sums = trust::parse_sha256sums(&manifest).map_err(|e| run.fail("sha256", e))?;
    let expected = sums.get(&want).ok_or_else(|| {
        run.fail(
            "sha256",
            format!("{} does not list {want}", trust::MANIFEST_NAME),
        )
    })?;
    let actual = host.sha256(&args.archive).map_err(|e| {
        run.fail(
            "sha256",
            format!("cannot hash {}: {e}", args.archive.display()),
        )
    })?;
    if &actual != expected {
        return Err(run.fail(
            "sha256",
            format!("{want} has sha256 {actual}, the manifest says {expected}"),
        ));
    }
    run.ok("sha256", actual.clone());

    let bundle = assets.join(trust::BUNDLE_NAME);
    if !bundle.is_file() {
        return Err(run.fail(
            "provenance",
            format!("missing {}; download it with the archive", bundle.display()),
        ));
    }
    let gh = host.gh_bin();
    if !host.have(&gh) {
        return Err(run.fail(
            "provenance",
            format!("{gh} not found; install GitHub CLI to verify the release"),
        ));
    }
    host.run(&trust::gh_verify_argv(
        &gh,
        &args.archive,
        &bundle,
        &args.tag,
    ))
    .map_err(|e| run.fail("provenance", e))?;
    run.ok("provenance", trust::PROVENANCE_BUILDER_RELEASE.to_owned());

    let root = ManagedRoot::new(dir.clone());
    let _lock = if args.apply {
        root.ensure_dir()
            .map_err(|e| run.fail("lock", e.to_string()))?;
        let lock = acquire_commit_lock(&root, LOCK_TIMEOUT).map_err(|e| {
            run.fail(
                "lock",
                format!("busy: another dx install holds {}: {e}", dir.display()),
            )
        })?;
        run.ok("lock", root.lock_path().display().to_string());
        Some(lock)
    } else {
        None
    };
    if args.apply {
        clear_old_files(&dir.join(BIN_DIR));
    }

    let staging = if args.apply {
        dir.join(format!(".staging-{}", nonce()))
    } else {
        std::env::temp_dir().join(format!("dx-install-{}", nonce()))
    };
    let mut cleanup = Cleanup(Some(staging.clone()));
    let format = dx_archive::Format::from_name(cell.suffix)
        .ok_or_else(|| run.fail("extract", format!("unknown suffix {}", cell.suffix)))?;
    let entries = dx_archive::extract(
        &args.archive,
        format,
        &staging,
        dx_archive::Limits::default(),
    )
    .map_err(|e| run.fail("extract", format!("{}: {e}", args.archive.display())))?;
    if !entries.iter().any(|entry| entry.name == cell.exe) {
        return Err(run.fail("extract", format!("{want} has no top-level {}", cell.exe)));
    }
    run.ok("extract", format!("{} files", entries.len()));

    let staged_exe = staging.join(cell.exe);
    let stdout = host
        .run(&[
            staged_exe.to_string_lossy().into_owned(),
            "--version".to_owned(),
        ])
        .map_err(|e| run.fail("version", e))?;
    let want_version = format!("{} {version}", payload.name);
    if stdout.trim() != want_version {
        return Err(run.fail(
            "version",
            format!(
                "staged {} --version printed {:?}, want {want_version:?}",
                cell.exe,
                stdout.trim()
            ),
        ));
    }
    run.ok("version", want_version);

    let current = current_tag(&dir, cell, windows).map_err(|e| run.fail("layout", e))?;
    let version_dir = dir.join(VERSIONS_DIR).join(&args.tag);
    let existing = match classify(&version_dir) {
        Ok(EntryKind::Missing) => None,
        Ok(EntryKind::Directory) => {
            let receipt = read_receipt(&version_dir).map_err(|e| run.fail("layout", e))?;
            if receipt.sha256 != actual || receipt.cell != cell.name {
                return Err(run.fail(
                    "layout",
                    format!(
                        "{} holds {} with sha256 {}; refusing to replace it with sha256 {actual}",
                        version_dir.display(),
                        receipt.tag,
                        receipt.sha256
                    ),
                ));
            }
            Some(receipt)
        }
        Ok(_) => {
            return Err(run.fail(
                "layout",
                format!("{} is not a directory", version_dir.display()),
            ))
        }
        Err(e) => {
            return Err(run.fail(
                "layout",
                format!("cannot inspect {}: {e}", version_dir.display()),
            ))
        }
    };
    let outcome = match (&current, &existing) {
        (Some(tag), Some(_)) if tag == &args.tag => Outcome::NoOp,
        (Some(tag), Some(_)) => Outcome::Switched { from: tag.clone() },
        (Some(tag), None) => Outcome::Upgraded { from: tag.clone() },
        (None, _) => Outcome::Fresh,
    };
    run.ok("layout", version_dir.display().to_string());

    let bin_dir = dir.join(BIN_DIR);
    if args.apply && outcome != Outcome::NoOp {
        let receipt = Receipt {
            payload: payload.name.to_owned(),
            tag: args.tag.clone(),
            cell: cell.name.to_owned(),
            sha256: actual.clone(),
            entries,
        };
        let created = existing.is_none();
        if created {
            std::fs::write(staging.join(RECEIPT_NAME), receipt.to_json())
                .and_then(|()| std::fs::create_dir_all(dir.join(VERSIONS_DIR)))
                .and_then(|()| std::fs::rename(&staging, &version_dir))
                .map_err(|e| {
                    run.fail(
                        "commit",
                        format!("cannot place {}: {e}", version_dir.display()),
                    )
                })?;
            cleanup.disarm();
        }
        let previous = match &current {
            Some(tag) => read_receipt(&dir.join(VERSIONS_DIR).join(tag)).ok(),
            None => None,
        };
        let swapped = if windows {
            swap_files(
                &bin_dir,
                &version_dir,
                &receipt,
                previous.as_ref(),
                &hard_link,
            )
            .and_then(|()| {
                dx_atomic_fs::write_atomic(
                    &bin_dir.join(CURRENT_FILE),
                    format!("{}\n", args.tag).as_bytes(),
                )
                .map_err(|e| format!("cannot record the current version: {e}"))
            })
        } else {
            swap_link(&bin_dir, cell, &args.tag)
        };
        if let Err(e) = swapped {
            if created {
                let _ = std::fs::remove_dir_all(&version_dir);
            }
            return Err(run.fail("commit", e));
        }
        run.ok("commit", bin_dir.join(cell.exe).display().to_string());
        let keep: BTreeSet<String> = [Some(args.tag.clone()), current.clone()]
            .into_iter()
            .flatten()
            .collect();
        prune_versions(&dir.join(VERSIONS_DIR), &keep);
    }
    drop(cleanup);

    Ok(Report {
        tag: args.tag.clone(),
        cell: cell.name,
        on_path: host.on_path(&bin_dir),
        dir,
        bin_dir,
        applied: args.apply,
        outcome,
        checks: run.checks,
    })
}

fn swap_link(bin_dir: &Path, cell: &Cell, tag: &str) -> Result<(), String> {
    std::fs::create_dir_all(bin_dir)
        .map_err(|e| format!("cannot create {}: {e}", bin_dir.display()))?;
    let target: PathBuf = [Path::new(".."), Path::new(VERSIONS_DIR), Path::new(tag)]
        .iter()
        .collect::<PathBuf>()
        .join(cell.exe);
    publish_pointer_swap(&PointerSwap {
        current: bin_dir.join(cell.exe),
        stage: bin_dir.join(format!(".{}.stage", cell.exe)),
        target,
        kind: PointerKind::File,
    })
    .map_err(|e| e.to_string())
}

fn hard_link(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::hard_link(source, destination)
}

fn is_entry_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.contains('/') && (lower.ends_with(".exe") || lower.ends_with(".dll"))
}

fn entry_files(receipt: &Receipt) -> Vec<String> {
    receipt
        .entries
        .iter()
        .filter(|entry| is_entry_file(&entry.name))
        .map(|entry| entry.name.clone())
        .collect()
}

fn old_name(bin_dir: &Path, name: &str) -> PathBuf {
    bin_dir.join(format!(".{name}.old-{}", nonce()))
}

/// Places every top-level exe and DLL of `receipt` in `bin_dir`, rolling back on error.
fn swap_files(
    bin_dir: &Path,
    version_dir: &Path,
    receipt: &Receipt,
    previous: Option<&Receipt>,
    link: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<(), String> {
    std::fs::create_dir_all(bin_dir)
        .map_err(|e| format!("cannot create {}: {e}", bin_dir.display()))?;
    let names = entry_files(receipt);
    let mut staged = Vec::new();
    for name in &names {
        let stage = bin_dir.join(format!(".{name}.stage"));
        let _ = std::fs::remove_file(&stage);
        if let Err(e) = dx_atomic_fs::materialize_file_with(
            &version_dir.join(name),
            &stage,
            dx_atomic_fs::LinkPolicy::LinkOrCopy,
            link,
        ) {
            for stage in &staged {
                let _ = std::fs::remove_file(stage);
            }
            return Err(format!("cannot stage {}: {e}", stage.display()));
        }
        staged.push(stage);
    }
    let mut moved: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let mut failure = None;
    for (name, stage) in names.iter().zip(&staged) {
        let live = bin_dir.join(name);
        let aside = if live.exists() {
            let aside = old_name(bin_dir, name);
            if let Err(e) = std::fs::rename(&live, &aside) {
                failure = Some(format!("cannot move {} aside: {e}", live.display()));
                break;
            }
            Some(aside)
        } else {
            None
        };
        if let Err(e) = std::fs::rename(stage, &live) {
            if let Some(aside) = &aside {
                let _ = std::fs::rename(aside, &live);
            }
            failure = Some(format!("cannot place {}: {e}", live.display()));
            break;
        }
        moved.push((live, aside));
    }
    if let Some(failure) = failure {
        for (live, aside) in moved.into_iter().rev() {
            match aside {
                Some(aside) => {
                    let _ = std::fs::rename(&aside, &live);
                }
                None => {
                    let _ = std::fs::remove_file(&live);
                }
            }
        }
        for stage in &staged {
            let _ = std::fs::remove_file(stage);
        }
        return Err(failure);
    }
    if let Some(previous) = previous {
        for stale in entry_files(previous) {
            if names.iter().any(|name| name.eq_ignore_ascii_case(&stale)) {
                continue;
            }
            let live = bin_dir.join(&stale);
            let aside = old_name(bin_dir, &stale);
            if std::fs::rename(&live, &aside).is_ok() {
                let _ = std::fs::remove_file(&aside);
            }
        }
    }
    clear_old_files(bin_dir);
    Ok(())
}

/// Best-effort removal of `.<name>.old-*` files; locked ones stay for a later run.
fn clear_old_files(bin_dir: &Path) {
    let Ok(read) = std::fs::read_dir(bin_dir) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name.contains(".old-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Best-effort removal of version directories outside `keep`.
fn prune_versions(versions: &Path, keep: &BTreeSet<String>) {
    let Ok(read) = std::fs::read_dir(versions) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if keep.contains(&name) || trust::validate_tag(&name).is_err() {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
