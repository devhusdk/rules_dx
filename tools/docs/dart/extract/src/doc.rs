use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use documentation_ir::tool::{FlagError, PathIoError, path_io};
use dx_process::capture::{RunError, run_captured};

const PUBSPEC: &str = "pubspec.yaml";
const DART_DOC_TIMEOUT: Duration = Duration::from_secs(3600);

#[derive(Debug)]
pub enum DartDocError {
    Io(PathIoError),
    Remove {
        path: PathBuf,
        source: io::Error,
    },
    Cwd(io::Error),
    Stage {
        source_path: String,
        at: PathBuf,
        source: io::Error,
    },
    MalformedRow {
        line: String,
        path: String,
    },
    StageClash {
        label: String,
        first: String,
        path: String,
        staged: String,
    },
    Run(RunError),
    DartDocFailed {
        output: String,
    },
}

impl fmt::Display for DartDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Remove { path, source } => {
                write!(f, "cannot remove {}: {source}", path.display())
            }
            Self::Cwd(error) => write!(f, "cannot read the cwd: {error}"),
            Self::Stage {
                source_path,
                at,
                source,
            } => write!(
                f,
                "cannot stage {source_path} at {}: {source}",
                at.display()
            ),
            Self::MalformedRow { line, path } => write!(f, "malformed line '{line}' in {path}"),
            Self::StageClash {
                label,
                first,
                path,
                staged,
            } => write!(
                f,
                "dart_doc_shard {label}: {first} and {path} both stage as {staged}"
            ),
            Self::Run(error) => error.fmt(f),
            Self::DartDocFailed { output } => write!(f, "dart doc failed:\n{output}"),
        }
    }
}

impl std::error::Error for DartDocError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Remove { source, .. } | Self::Cwd(source) | Self::Stage { source, .. } => {
                Some(source)
            }
            Self::Run(error) => Some(error),
            Self::MalformedRow { .. } | Self::StageClash { .. } | Self::DartDocFailed { .. } => {
                None
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocOptions {
    pub dart: String,
    pub packages: String,
    pub srcs: String,
    pub own: String,
    pub label: String,
    pub scratch: String,
    pub html: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub links: Vec<(String, String)>,
    pub files: Vec<(String, String)>,
}

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    documentation_ir::tool::run_process("dart_doc", usage(), parse, run, args)
}

#[must_use]
pub fn usage() -> &'static str {
    "usage: dart_doc --dart FILE --packages FILE --srcs FILE --own NAME --label LABEL --scratch DIR --html DIR"
}

pub fn parse(args: &[String]) -> Result<DocOptions, FlagError> {
    let mut flags = documentation_ir::tool::flags(
        args,
        &[
            "--dart",
            "--packages",
            "--srcs",
            "--own",
            "--label",
            "--scratch",
            "--html",
        ],
    )?;
    Ok(DocOptions {
        dart: flags.one("--dart")?,
        packages: flags.one("--packages")?,
        srcs: flags.one("--srcs")?,
        own: flags.one("--own")?,
        label: flags.one("--label")?,
        scratch: flags.one("--scratch")?,
        html: flags.one("--html")?,
    })
}

pub fn run(options: &DocOptions) -> Result<(), DartDocError> {
    let packages = rows(&options.packages)?;
    let srcs = rows(&options.srcs)?;
    let plan = plan(&packages, &srcs, &options.own, &options.label)?;
    let scratch = Path::new(&options.scratch);
    let stage = scratch.join("stage");
    let result = stage_tree(&plan, &stage).and_then(|()| dart_doc(options, scratch, &stage));
    result.and(empty_dir(scratch))
}

fn empty_dir(dir: &Path) -> Result<(), DartDocError> {
    let unread = |source| {
        DartDocError::Io(PathIoError::Read {
            path: dir.to_path_buf(),
            source,
        })
    };
    for entry in fs::read_dir(dir).map_err(unread)? {
        let path = entry.map_err(unread)?.path();
        let removed = if path.is_dir() && !path.is_symlink() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        removed.map_err(|source| DartDocError::Remove { path, source })?;
    }
    Ok(())
}

fn rows(path: &str) -> Result<Vec<(String, String)>, DartDocError> {
    let text = path_io::read_to_string(Path::new(path)).map_err(DartDocError::Io)?;
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split_once('\t')
                .map(|(a, b)| (a.to_owned(), b.to_owned()))
                .ok_or_else(|| DartDocError::MalformedRow {
                    line: line.to_owned(),
                    path: path.to_owned(),
                })
        })
        .collect()
}

fn repo_relative(short_path: &str) -> &str {
    match short_path.strip_prefix("../") {
        Some(rest) => rest.split_once('/').map_or("", |(_, inner)| inner),
        None => short_path,
    }
}

fn within<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if root.is_empty() {
        return Some(path);
    }
    path.strip_prefix(root)?.strip_prefix('/')
}

pub fn plan(
    packages: &[(String, String)],
    srcs: &[(String, String)],
    own: &str,
    label: &str,
) -> Result<Plan, DartDocError> {
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
                return Err(DartDocError::StageClash {
                    label: label.to_owned(),
                    first: first.clone(),
                    path: path.clone(),
                    staged,
                });
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

fn stage_tree(plan: &Plan, stage: &Path) -> Result<(), DartDocError> {
    let cwd = std::env::current_dir().map_err(DartDocError::Cwd)?;
    for (staged, source_path) in &plan.links {
        let at = parent_made(&stage.join(staged))?;
        if let Err(source) = link(&cwd.join(source_path), &at) {
            return Err(DartDocError::Stage {
                source_path: source_path.clone(),
                at,
                source,
            });
        }
    }
    for (staged, text) in &plan.files {
        let at = parent_made(&stage.join(staged))?;
        fs::write(&at, text)
            .map_err(|source| DartDocError::Io(PathIoError::Write { path: at, source }))?;
    }
    Ok(())
}

fn parent_made(path: &Path) -> Result<PathBuf, DartDocError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            DartDocError::Io(PathIoError::Create {
                path: parent.to_path_buf(),
                source,
            })
        })?;
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

fn dart_doc(options: &DocOptions, home: &Path, stage: &Path) -> Result<(), DartDocError> {
    let mut command = Command::new(&options.dart);
    command
        .env("HOME", home)
        .env("PUB_CACHE", home.join("pub"))
        .arg("doc")
        .arg("--output")
        .arg(&options.html)
        .arg(stage.join(&options.own));
    let captured = run_captured(&mut command, DART_DOC_TIMEOUT).map_err(DartDocError::Run)?;
    if captured.success() {
        Ok(())
    } else {
        Err(DartDocError::DartDocFailed {
            output: format!("{}{}", captured.stdout_text(), captured.stderr_text()),
        })
    }
}

#[cfg(test)]
#[path = "doc_tests.rs"]
mod doc_tests;
