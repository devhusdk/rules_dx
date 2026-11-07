//! Keeps the `.dx` tree in step with `dx.toml`, skipping all work when nothing it reads changed.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use dx_config::LoadProblem;

use crate::tools_gen::TOOL_CONFIGS;
use crate::{GenerateError, Inputs, RootBuild, render};

/// Bump when the fingerprint layout changes.
const VERSION: &str = "v1";

/// Root files whose stats the fingerprint covers besides the outputs.
const SHARED_INPUTS: [&str; 2] = ["MODULE.bazel", "pyproject.toml"];

/// Top-level entries under `.dx` that dx does not own.
const KEPT: [&str; 2] = ["local", "pins"];

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
    let fingerprint_path = dx_path::state::fingerprint_path(root);
    if mode == Mode::Fast
        && let Ok(text) = std::fs::read_to_string(&fingerprint_path)
    {
        let mut lines = text.lines();
        let stored = lines.next().unwrap_or_default();
        let outputs: Vec<String> = lines.map(str::to_owned).collect();
        if stored == header(&fingerprint(root, &outputs)?) {
            return Ok(SyncReport::default());
        }
    }

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
    let root_texts = reference_texts(root, &root_files)?;
    let files = if root_texts.is_empty() {
        BTreeSet::new()
    } else {
        workspace_files(root, "")?
    };
    let inputs = Inputs {
        config: dx_config::load(root)?,
        repo: dx_module_bazel::rules_dx_repo(root),
        families: detect(root),
        root_texts,
        files,
        root_files,
        root_build,
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

    let outputs: Vec<String> = files.into_keys().collect();
    let mut text = header(&fingerprint(root, &outputs)?);
    for output in &outputs {
        text.push('\n');
        text.push_str(output);
    }
    text.push('\n');
    if let Some(parent) = fingerprint_path.parent() {
        std::fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    dx_atomic_fs::write_atomic(&fingerprint_path, text.as_bytes())
        .map_err(io_error(&fingerprint_path))?;
    Ok(report)
}

fn header(digest: &[u8; 32]) -> String {
    format!("{VERSION} {}", dx_digest::to_hex(digest))
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
    let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map_err(io_error(dir))?
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
        let is_dir = std::fs::symlink_metadata(&path)
            .map_err(io_error(&path))?
            .is_dir();
        if is_dir {
            empty &= remove_stale(&path, &child, keep, deleted)?;
        } else if keep.contains(child.as_str()) {
            empty = false;
        } else {
            std::fs::remove_file(&path).map_err(io_error(&path))?;
            deleted.push(child);
        }
    }
    if empty && !top {
        std::fs::remove_dir(dir).map_err(io_error(dir))?;
        return Ok(true);
    }
    Ok(false)
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

/// Root config files whose text names other files, keyed by file name.
fn reference_texts(
    root: &Path,
    root_files: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, SyncError> {
    let mut texts = BTreeMap::new();
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
                texts.insert((*name).to_owned(), text);
            }
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {}
            Err(error) => return Err(io_error(&path)(error)),
        }
    }
    Ok(texts)
}

/// Every workspace-relative file path, skipping VCS, Bazel output and `.dx`.
fn workspace_files(root: &Path, rel: &str) -> Result<BTreeSet<String>, SyncError> {
    let dir = root.join(rel);
    let mut files = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).map_err(io_error(&dir))? {
        let entry = entry.map_err(io_error(&dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" || name == ".dx" || name == "node_modules" || name.starts_with("bazel-") {
            continue;
        }
        let path = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        let kind = entry.file_type().map_err(io_error(&entry.path()))?;
        if kind.is_dir() {
            files.extend(workspace_files(root, &path)?);
        } else {
            files.insert(path);
        }
    }
    Ok(files)
}

/// Hash of everything a render reads, plus the stats of the outputs a previous sync wrote.
fn fingerprint(root: &Path, outputs: &[String]) -> Result<[u8; 32], SyncError> {
    let mut buf = Vec::new();
    let mut field = |bytes: &[u8]| {
        buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(bytes);
    };
    field(VERSION.as_bytes());
    let config = root.join("dx.toml");
    let pins = dx_path::state::dx_dir(root).join("pins");
    for path in [config, pins] {
        match std::fs::read(&path) {
            Ok(bytes) => field(&bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => field(b"absent"),
            Err(error) => return Err(io_error(&path)(error)),
        }
    }
    let names = root_names(root)?;
    field(&(names.len() as u64).to_le_bytes());
    for name in &names {
        field(name.as_bytes());
    }
    for (name, text) in reference_texts(root, &names)? {
        field(name.as_bytes());
        field(text.as_bytes());
    }
    for rel in SHARED_INPUTS
        .iter()
        .copied()
        .chain(outputs.iter().map(String::as_str))
    {
        field(rel.as_bytes());
        match std::fs::metadata(root.join(rel)) {
            Ok(metadata) => {
                let mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |since| since.as_nanos());
                field(&metadata.len().to_le_bytes());
                field(&mtime.to_le_bytes());
            }
            Err(_) => field(b"absent"),
        }
    }
    for config in TOOL_CONFIGS {
        field(config.id.as_bytes());
        field(config.standard.as_bytes());
        field(config.strict.as_bytes());
    }
    Ok(dx_digest::blake3(&buf))
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod sync_tests;
