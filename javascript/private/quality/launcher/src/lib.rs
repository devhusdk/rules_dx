//! Reads a generated JavaScript launcher's own runfiles so a host can run the tool.

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
use std::io;
use std::path::{Path, PathBuf};

/// The program a launcher runs, with the entry point it runs.
#[derive(Debug)]
pub struct Plan {
    pub program: PathBuf,
    pub node_args: Vec<String>,
    pub entry: PathBuf,
    pub args: Vec<String>,
    pub path_prefix: PathBuf,
    pub env_set: Vec<(String, String)>,
    pub env_default: Vec<(String, String)>,
}

impl Plan {
    /// Returns the full child command line this plan runs.
    pub fn argv(&self) -> Vec<OsString> {
        let mut argv = Vec::new();
        argv.push(self.program.as_os_str().to_owned());
        argv.extend(self.node_args.iter().map(OsString::from));
        argv.push(OsString::from("--"));
        argv.push(self.entry.as_os_str().to_owned());
        argv.extend(self.args.iter().map(OsString::from));
        argv
    }
}

/// Names the descriptor beside one launcher.
pub fn descriptor_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_stem().unwrap_or(exe.as_os_str()).to_os_string();
    name.push(".launcher.txt");
    exe.with_file_name(name)
}

/// The launch the Bazel rule wrote for one wrapper.
#[derive(Debug, PartialEq, Eq)]
struct Descriptor {
    entry: String,
    node: String,
    require: String,
    wrapper: String,
    node_options: Vec<String>,
}

/// Returns the launch one descriptor names.
fn descriptor(path: &Path) -> io::Result<Descriptor> {
    let text = read(path)?;
    let mut entry = None;
    let mut node = None;
    let mut require = None;
    let mut wrapper = None;
    let mut node_options = Vec::new();
    for line in text.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().to_owned();
        if value.is_empty() {
            continue;
        }
        match name.trim() {
            "entry" => entry = Some(single(path, entry, value, "entry")?),
            "node" => node = Some(single(path, node, value, "node")?),
            "require" => require = Some(single(path, require, value, "require")?),
            "wrapper" => wrapper = Some(single(path, wrapper, value, "wrapper")?),
            "node_option" => node_options.push(value),
            _ => {}
        }
    }
    Ok(Descriptor {
        entry: need(path, entry, "entry")?,
        node: need(path, node, "node")?,
        require: need(path, require, "require")?,
        wrapper: need(path, wrapper, "wrapper")?,
        node_options,
    })
}

/// Resolves what one launcher invocation runs from its own runfiles.
pub fn plan(exe: &Path, args: &[String]) -> io::Result<Plan> {
    let cwd = std::env::current_dir().map_err(|error| {
        bad(format!(
            "cannot read the working directory for {}: {error}",
            exe.display()
        ))
    })?;
    plan_in(exe, args, &cwd)
}

/// Resolves one launch as if the launcher ran in `cwd`.
fn plan_in(exe: &Path, args: &[String], cwd: &Path) -> io::Result<Plan> {
    let owned = descriptor(&descriptor_path(exe))?;
    let runfiles = dx_path::Resolver::for_binary(exe)?;
    let entry = resolve(&runfiles, &owned.entry)?;
    let program = resolve(&runfiles, &owned.node)?;
    let patches = resolve(&runfiles, &owned.require)?;
    let wrapper = resolve(&runfiles, &owned.wrapper)?;
    let mut node_args = vec!["--require".to_owned(), lossy(&patches)];
    node_args.extend(owned.node_options.iter().cloned());
    let mut script_args = Vec::new();
    for arg in args {
        if let Some(option) = arg.strip_prefix("--node_options=") {
            node_args.push(option.to_owned());
        } else {
            script_args.push(arg.clone());
        }
    }
    let root = runfiles_root(&runfiles);
    let execroot = lossy(cwd);
    let tree = lossy(&root);
    let binary = lossy(&program);
    Ok(Plan {
        program,
        node_args,
        entry,
        args: script_args,
        path_prefix: wrapper
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("")),
        env_set: vec![
            ("JS_BINARY__EXECROOT".to_owned(), execroot.clone()),
            ("JS_BINARY__RUNFILES".to_owned(), tree.clone()),
            ("JS_BINARY__NODE_BINARY".to_owned(), binary),
            ("JS_BINARY__NODE_PATCHES".to_owned(), lossy(&patches)),
            ("JS_BINARY__NODE_WRAPPER".to_owned(), lossy(&wrapper)),
            (
                "JS_BINARY__FS_PATCH_ROOTS".to_owned(),
                format!("{execroot}:{tree}"),
            ),
        ],
        env_default: vec![
            ("JS_BINARY__PATCH_NODE_FS".to_owned(), "1".to_owned()),
            ("NODE_DISABLE_COMPILE_CACHE".to_owned(), "1".to_owned()),
        ],
    })
}

/// Returns the file one runfiles key names.
///
/// Manifests map npm package directories rather than every file inside them,
/// so a key the manifest does not name is retried below each mapped ancestor
/// directory. A resolved file that does not exist fails the same way a key
/// the runfiles never named does.
fn resolve(runfiles: &dx_path::Resolver, key: &str) -> io::Result<PathBuf> {
    if let Ok(path) = runfiles.lookup(key) {
        return Ok(path);
    }
    let mut ancestor = Path::new(key).parent();
    while let Some(dir) = ancestor {
        let prefix = dir.to_string_lossy();
        if let Ok(base) = runfiles.lookup(prefix.as_ref()) {
            let rest = Path::new(key)
                .strip_prefix(dir)
                .map_err(|error| bad(format!("cannot split {key}: {error}")))?;
            let path = base.join(rest);
            if path.is_file() {
                return Ok(path);
            }
            break;
        }
        ancestor = dir.parent();
    }
    runfiles.lookup(key)
}

/// Returns the runfiles tree one resolver read, or the manifest's directory.
fn runfiles_root(runfiles: &dx_path::Resolver) -> PathBuf {
    let source = runfiles.source();
    if source.is_dir() {
        return source.to_path_buf();
    }
    source
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(""))
}

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn single(path: &Path, slot: Option<String>, value: String, name: &str) -> io::Result<String> {
    if slot.is_some() {
        return Err(bad(format!("two {name} lines in {}", path.display())));
    }
    Ok(value)
}

fn need(path: &Path, slot: Option<String>, name: &str) -> io::Result<String> {
    slot.ok_or_else(|| bad(format!("no {name} in {}", path.display())))
}

fn read(path: &Path) -> io::Result<String> {
    std::fs::read_to_string(path)
        .map_err(|error| bad(format!("cannot read {}: {error}", path.display())))
}

fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}

#[cfg(test)]
mod lib_tests;
