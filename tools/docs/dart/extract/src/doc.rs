//! Stages the sources of one Dart library as pub packages and runs `dart doc` over them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use dx_process::capture::run_captured;

const PUBSPEC: &str = "pubspec.yaml";
const DART_DOC_TIMEOUT: Duration = Duration::from_secs(3600);

/// The inputs of one `dart doc` run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocOptions {
    pub dart: String,
    pub packages: String,
    pub srcs: String,
    pub own: String,
    pub label: String,
    pub stage: String,
    pub html: String,
}

/// The staging tree of one run, with paths relative to the stage directory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Staged path and the exec path of the source it points at.
    pub links: Vec<(String, String)>,
    /// Staged path and the text written there.
    pub files: Vec<(String, String)>,
}

/// Runs `dart_doc` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("dart_doc", usage(), parse, run, args)
}

/// The usage line shown for a rejected invocation.
#[must_use]
pub fn usage() -> &'static str {
    "usage: dart_doc --dart FILE --packages FILE --srcs FILE --own NAME --label LABEL --stage DIR --html DIR"
}

/// Parses argv into inputs, or returns the rejection text.
///
/// # Errors
///
/// Returns an error when a flag is unknown, missing or repeated.
pub fn parse(args: &[String]) -> Result<DocOptions, String> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--dart",
            "--packages",
            "--srcs",
            "--own",
            "--label",
            "--stage",
            "--html",
        ],
    )?;
    Ok(DocOptions {
        dart: flags.one("--dart")?,
        packages: flags.one("--packages")?,
        srcs: flags.one("--srcs")?,
        own: flags.one("--own")?,
        label: flags.one("--label")?,
        stage: flags.one("--stage")?,
        html: flags.one("--html")?,
    })
}

/// Stages the packages, runs `dart doc` and removes the staging tree on every path.
///
/// # Errors
///
/// Returns an error when an input cannot be read, two sources stage as one path,
/// staging fails or `dart doc` fails.
pub fn run(options: &DocOptions) -> Result<(), String> {
    let packages = rows(&options.packages)?;
    let srcs = rows(&options.srcs)?;
    let plan = plan(&packages, &srcs, &options.own, &options.label)?;
    let stage = Path::new(&options.stage);
    let result = stage_tree(&plan, stage).and_then(|()| dart_doc(options, stage));
    let cleanup = match fs::remove_dir_all(stage) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot remove {}: {error}", stage.display())),
    };
    result.and(cleanup)
}

fn rows(path: &str) -> Result<Vec<(String, String)>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split_once('\t')
                .map(|(a, b)| (a.to_owned(), b.to_owned()))
                .ok_or_else(|| format!("malformed line '{line}' in {path}"))
        })
        .collect()
}

/// Returns a `File.short_path` relative to the root of its own repository.
fn repo_relative(short_path: &str) -> &str {
    match short_path.strip_prefix("../") {
        Some(rest) => rest.split_once('/').map_or("", |(_, inner)| inner),
        None => short_path,
    }
}

/// Returns `path` relative to the package root `root`, or None when outside it.
fn within<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if root.is_empty() {
        return Some(path);
    }
    path.strip_prefix(root)?.strip_prefix('/')
}

/// Maps the sources onto `<package>/<path>` below the stage and adds each pubspec and the package config.
///
/// `packages` are `(name, lib_root)` rows where a later row of one name replaces
/// the root but keeps the first position; `srcs` are `(exec path, short path)` rows.
///
/// # Errors
///
/// Returns an error when two sources stage as the same path.
pub fn plan(
    packages: &[(String, String)],
    srcs: &[(String, String)],
    own: &str,
    label: &str,
) -> Result<Plan, String> {
    let mut ordered: Vec<(String, String)> = Vec::new();
    for (name, root) in packages {
        match ordered.iter_mut().find(|(seen, _)| seen == name) {
            Some(entry) => entry.1.clone_from(root),
            None => ordered.push((name.clone(), root.clone())),
        }
    }
    let mut roots = ordered.clone();
    roots.sort_by_key(|(_, root)| std::cmp::Reverse(root.len()));
    let mut files: Vec<&(String, String)> = srcs.iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut links: Vec<(String, String)> = Vec::new();
    for (path, short_path) in files {
        let rel = repo_relative(short_path);
        for (name, root) in &roots {
            let Some(inner) = within(short_path, root).or_else(|| within(rel, root)) else {
                continue;
            };
            if !root.is_empty() && !inner.starts_with("lib/") {
                continue;
            }
            let staged = format!("{name}/{inner}");
            if let Some((_, first)) = links.iter().find(|(seen, _)| *seen == staged) {
                return Err(format!(
                    "dart_doc_shard {label}: {first} and {path} both stage as {staged}"
                ));
            }
            links.push((staged, path.clone()));
            break;
        }
    }
    let mut names: Vec<&str> = ordered.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    let mut staged_files: Vec<(String, String)> = names
        .iter()
        .map(|name| {
            (
                format!("{name}/{PUBSPEC}"),
                format!("name: {name}\nenvironment:\n  sdk: ^3.0.0\n"),
            )
        })
        .collect();
    let entries: Vec<String> = names
        .iter()
        .map(|name| {
            let root = if *name == own {
                "../".to_owned()
            } else {
                format!("../../{name}/")
            };
            format!(
                r#"{{"name":"{name}","rootUri":"{root}","packageUri":"lib/","languageVersion":"3.0"}}"#
            )
        })
        .collect();
    staged_files.push((
        format!("{own}/.dart_tool/package_config.json"),
        format!(
            "{{\"configVersion\":2,\"packages\":[{}]}}\n",
            entries.join(",")
        ),
    ));
    Ok(Plan {
        links,
        files: staged_files,
    })
}

fn stage_tree(plan: &Plan, stage: &Path) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|error| format!("cannot read the cwd: {error}"))?;
    for (staged, source) in &plan.links {
        let at = parent_made(&stage.join(staged))?;
        link(&cwd.join(source), &at)
            .map_err(|error| format!("cannot stage {source} at {}: {error}", at.display()))?;
    }
    for (staged, text) in &plan.files {
        let at = parent_made(&stage.join(staged))?;
        fs::write(&at, text).map_err(|error| format!("cannot write {}: {error}", at.display()))?;
    }
    Ok(())
}

fn parent_made(path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    Ok(path.to_path_buf())
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    fs::copy(target, at).map(|_| ())
}

fn dart_doc(options: &DocOptions, stage: &Path) -> Result<(), String> {
    let mut command = Command::new(&options.dart);
    command
        .arg("doc")
        .arg("--output")
        .arg(&options.html)
        .arg(stage.join(&options.own));
    let captured =
        run_captured(&mut command, DART_DOC_TIMEOUT).map_err(|error| error.to_string())?;
    if captured.success() {
        Ok(())
    } else {
        Err(format!(
            "dart doc failed:\n{}{}",
            captured.stdout_text(),
            captured.stderr_text()
        ))
    }
}

#[cfg(test)]
#[path = "doc_tests.rs"]
mod doc_tests;
