//! Keeps the `.dx` tree in step with `dx.toml`, skipping all work when nothing it reads changed.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use dx_config::LoadProblem;

use crate::tools_gen::TOOL_CONFIGS;
use crate::{GenerateError, Inputs, RootBuild, references, render};

/// Bump when the fingerprint layout changes.
const VERSION: &str = "v1";

/// Root files whose stats the fingerprint covers besides the outputs.
const SHARED_INPUTS: [&str; 4] = ["MODULE.bazel", "pyproject.toml", "BUILD.bazel", "BUILD"];

/// Top-level entries under `.dx` that dx does not own.
const KEPT: [&str; 2] = ["local", "pins"];

/// How long a sync waits for another one to finish.
const LOCK_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Skip detection and rendering when the fingerprint matches.
    Fast,
    /// Always detect and render.
    Full,
}

/// Workspace-relative paths that a sync changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub written: Vec<String>,
    pub deleted: Vec<String>,
    /// Whether families were detected, i.e. the fast path missed.
    pub detected: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("dx.toml: {0}")]
    Load(#[from] LoadProblem),
    #[error(transparent)]
    Generate(#[from] GenerateError),
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    /// The last sync failed with this message and nothing it read has changed since.
    #[error("{0}")]
    Unchanged(String),
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> SyncError {
    let path = path.to_path_buf();
    move |source| SyncError::Io { path, source }
}

/// Renders `.dx` and the root export block, writes what differs and deletes stale `.dx` files.
/// `detect` returns the families present in the workspace; `Fast` calls it only on a miss.
pub fn sync(
    root: &Path,
    mode: Mode,
    detect: &mut dyn FnMut(&Path) -> BTreeSet<String>,
) -> Result<SyncReport, SyncError> {
    if mode == Mode::Fast
        && let Some(result) = cached(root, &Snapshot::take(root)?)
    {
        return result;
    }
    let lock_path = dx_path::state::sync_lock_path(root);
    let _lock = dx_atomic_fs::lease::acquire_exclusive_at(&lock_path, LOCK_TIMEOUT)
        .map_err(io_error(&lock_path))?;
    let snapshot = Snapshot::take(root)?;
    if mode == Mode::Fast
        && let Some(result) = cached(root, &snapshot)
    {
        return result;
    }
    match render_and_write(root, &snapshot, detect) {
        Ok((report, outputs)) => {
            let mut text = header(&fingerprint(&snapshot, root, &outputs));
            for output in &outputs {
                text.push('\n');
                text.push_str(output);
            }
            text.push('\n');
            record(root, &text)?;
            Ok(report)
        }
        Err(error @ (SyncError::Load(_) | SyncError::Generate(_))) => {
            let text = format!(
                "{}\nerror\n{error}",
                header(&fingerprint(&snapshot, root, &[]))
            );
            record(root, &text)?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

/// The result of the last sync when nothing it read has changed since.
fn cached(root: &Path, snapshot: &Snapshot) -> Option<Result<SyncReport, SyncError>> {
    let text = std::fs::read_to_string(dx_path::state::fingerprint_path(root)).ok()?;
    let (stored, rest) = text.split_once('\n').unwrap_or((&text, ""));
    let (outputs, error) = match rest.strip_prefix("error\n") {
        Some(message) => (Vec::new(), Some(message)),
        None => (rest.lines().map(str::to_owned).collect(), None),
    };
    if stored != header(&fingerprint(snapshot, root, &outputs)) {
        return None;
    }
    Some(match error {
        Some(message) => Err(SyncError::Unchanged(message.to_owned())),
        None => Ok(SyncReport::default()),
    })
}

fn record(root: &Path, text: &str) -> Result<(), SyncError> {
    let path = dx_path::state::fingerprint_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    dx_atomic_fs::write_atomic(&path, text.as_bytes()).map_err(io_error(&path))
}

/// Renders from `snapshot`, writes what differs and removes stale files. Returns the outputs.
fn render_and_write(
    root: &Path,
    snapshot: &Snapshot,
    detect: &mut dyn FnMut(&Path) -> BTreeSet<String>,
) -> Result<(SyncReport, Vec<String>), SyncError> {
    let config = match &snapshot.config {
        None => None,
        Some(bytes) => {
            let text = std::str::from_utf8(bytes).map_err(|error| LoadProblem::Read {
                path: root.join("dx.toml").display().to_string(),
                message: error.to_string(),
            })?;
            Some(dx_config::config::parse(text)?)
        }
    };
    let inputs = Inputs {
        config,
        repo: dx_module_bazel::rules_dx_repo(root),
        families: detect(root),
        root_texts: snapshot.root_texts.clone(),
        files: snapshot.files.clone(),
        root_files: snapshot.root_files.clone(),
        root_build: snapshot.root_build.as_ref().map(|build| RootBuild {
            name: build.name.clone(),
            text: build.text.clone(),
        }),
    };
    let files = render(&inputs)?;

    let mut report = SyncReport {
        detected: true,
        ..SyncReport::default()
    };
    for (rel, bytes) in &files {
        let path = root.join(rel);
        if std::fs::read(&path).is_ok_and(|old| old == *bytes) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_error(parent))?;
        }
        dx_atomic_fs::write_atomic(&path, bytes).map_err(io_error(&path))?;
        report.written.push(rel.clone());
    }
    let dx = dx_path::state::dx_dir(root);
    if dx.is_dir() {
        let keep: BTreeSet<&str> = files.keys().map(String::as_str).collect();
        remove_stale(&dx, dx_path::state::DX_DIR, &keep, &mut report.deleted)?;
    }
    Ok((report, files.into_keys().collect()))
}

fn header(digest: &[u8; 32]) -> String {
    format!("{VERSION} {}", dx_digest::to_hex(digest))
}

fn is_not_found(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}

/// Deletes files under `dir` that are not in `keep`, then `dir` itself if that left it empty.
/// Returns whether `dir` is now gone.
fn remove_stale(
    dir: &Path,
    rel: &str,
    keep: &BTreeSet<&str>,
    deleted: &mut Vec<String>,
) -> Result<bool, SyncError> {
    let top = rel == dx_path::state::DX_DIR;
    let listing = match std::fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(error) if is_not_found(&error) => return Ok(true),
        Err(error) => return Err(io_error(dir)(error)),
    };
    let mut entries: Vec<(String, PathBuf)> = listing
        .map(|entry| {
            entry
                .map(|entry| {
                    (
                        entry.file_name().to_string_lossy().into_owned(),
                        entry.path(),
                    )
                })
                .map_err(io_error(dir))
        })
        .collect::<Result<_, _>>()?;
    entries.sort();
    let mut empty = true;
    for (name, path) in entries {
        if top && KEPT.contains(&name.as_str()) {
            empty = false;
            continue;
        }
        let child = format!("{rel}/{name}");
        let is_dir = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata.is_dir(),
            Err(error) if is_not_found(&error) => continue,
            Err(error) => return Err(io_error(&path)(error)),
        };
        if is_dir {
            empty &= remove_stale(&path, &child, keep, deleted)?;
        } else if keep.contains(child.as_str()) {
            empty = false;
        } else {
            match std::fs::remove_file(&path) {
                Ok(()) => deleted.push(child),
                Err(error) if is_not_found(&error) => {}
                Err(error) => return Err(io_error(&path)(error)),
            }
        }
    }
    if empty && !top {
        return match std::fs::remove_dir(dir) {
            Ok(()) => Ok(true),
            Err(error) if is_not_found(&error) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(false),
            Err(error) => Err(io_error(dir)(error)),
        };
    }
    Ok(false)
}

/// Everything a render reads from the workspace, taken once so the render and the fingerprint agree.
struct Snapshot {
    config: Option<Vec<u8>>,
    pins: Option<Vec<u8>>,
    root_files: BTreeSet<String>,
    root_build: Option<RootBuild>,
    root_texts: BTreeMap<String, String>,
    files: BTreeSet<String>,
    /// (size, mtime) of each of [`SHARED_INPUTS`], taken before anything is read.
    stats: Vec<Option<(u64, u128)>>,
}

impl Snapshot {
    fn take(root: &Path) -> Result<Self, SyncError> {
        let stats = SHARED_INPUTS
            .iter()
            .map(|rel| stat(&root.join(rel)))
            .collect();
        let config = read_optional(&root.join("dx.toml"))?;
        let pins = read_optional(&dx_path::state::dx_dir(root).join("pins"))?;
        let root_files = root_names(root)?;
        let root_build = ["BUILD.bazel", "BUILD"]
            .into_iter()
            .find(|name| root_files.contains(*name))
            .map(|name| {
                let path = root.join(name);
                std::fs::read_to_string(&path)
                    .map(|text| RootBuild {
                        name: name.to_owned(),
                        text,
                    })
                    .map_err(io_error(&path))
            })
            .transpose()?;
        let mut root_texts = BTreeMap::new();
        let mut targets = BTreeSet::new();
        for config in TOOL_CONFIGS.iter().filter(|c| !c.references.is_empty()) {
            let Some(name) = config
                .files
                .iter()
                .filter(|name| config.suffixes.iter().any(|suffix| name.ends_with(suffix)))
                .find(|name| root_files.contains(**name))
            else {
                continue;
            };
            let path = root.join(name);
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    targets.extend(references::targets(config, &text));
                    root_texts.insert((*name).to_owned(), text);
                }
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {}
                Err(error) => return Err(io_error(&path)(error)),
            }
        }
        Ok(Snapshot {
            config,
            pins,
            files: reference_files(root, &targets),
            root_files,
            root_build,
            root_texts,
            stats,
        })
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, SyncError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if is_not_found(&error) => Ok(None),
        Err(error) => Err(io_error(path)(error)),
    }
}

fn stat(path: &Path) -> Option<(u64, u128)> {
    let metadata = std::fs::metadata(path).ok()?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    Some((metadata.len(), mtime))
}

fn root_names(root: &Path) -> Result<BTreeSet<String>, SyncError> {
    std::fs::read_dir(root)
        .map_err(io_error(root))?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(io_error(root))
        })
        .collect()
}

/// The files under each target plus the BUILD files of the directories above it: all that
/// `references::resolve` looks at. Honors `.gitignore` and skips VCS, Bazel output and `.dx`.
fn reference_files(root: &Path, targets: &BTreeSet<String>) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for target in targets {
        let walker = ignore::WalkBuilder::new(root.join(target))
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .require_git(false)
            .parents(true)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                !(name == ".git"
                    || name == ".dx"
                    || name == "node_modules"
                    || name.starts_with("bazel-"))
            })
            .build();
        for entry in walker.flatten() {
            if entry.file_type().is_some_and(|kind| kind.is_dir()) {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let rel: Vec<String> = rel
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.insert(rel.join("/"));
        }
        let mut dir = target.as_str();
        while let Some((parent, _)) = dir.rsplit_once('/') {
            for build in ["BUILD.bazel", "BUILD"] {
                let rel = format!("{parent}/{build}");
                if root.join(&rel).is_file() {
                    files.insert(rel);
                }
            }
            dir = parent;
        }
    }
    files
}

/// Hash of everything in `snapshot` plus the current stats of `outputs`.
fn fingerprint(snapshot: &Snapshot, root: &Path, outputs: &[String]) -> [u8; 32] {
    let mut buf = Vec::new();
    let mut field = |bytes: &[u8]| {
        buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(bytes);
    };
    field(VERSION.as_bytes());
    for bytes in [&snapshot.config, &snapshot.pins] {
        match bytes {
            Some(bytes) => {
                field(b"present");
                field(bytes);
            }
            None => field(b"absent"),
        }
    }
    let is_output = |name: &str| outputs.iter().any(|output| output == name);
    let names: Vec<&String> = snapshot
        .root_files
        .iter()
        .filter(|name| *name != dx_path::state::DX_DIR && !is_output(name))
        .collect();
    field(&(names.len() as u64).to_le_bytes());
    for name in names {
        field(name.as_bytes());
    }
    field(&(snapshot.root_texts.len() as u64).to_le_bytes());
    for (name, text) in &snapshot.root_texts {
        field(name.as_bytes());
        field(text.as_bytes());
    }
    field(&(snapshot.files.len() as u64).to_le_bytes());
    for file in &snapshot.files {
        field(file.as_bytes());
    }
    let shared = SHARED_INPUTS
        .iter()
        .zip(&snapshot.stats)
        .filter(|(rel, _)| !is_output(rel))
        .map(|(rel, stats)| (*rel, *stats));
    let written = outputs
        .iter()
        .map(|rel| (rel.as_str(), stat(&root.join(rel))));
    for (rel, stats) in shared.chain(written) {
        field(rel.as_bytes());
        match stats {
            Some((len, mtime)) => {
                field(&len.to_le_bytes());
                field(&mtime.to_le_bytes());
            }
            None => field(b"absent"),
        }
    }
    for config in TOOL_CONFIGS {
        field(config.id.as_bytes());
        field(config.standard.as_bytes());
        field(config.strict.as_bytes());
    }
    dx_digest::blake3(&buf)
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod sync_tests;
