use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::grammar::VALUE_OPTIONS;
use super::{parse_with, ArgsError, Command, FileDefaults, Invocation};

const MAX_REDIRECTS: usize = 8;

pub struct ResolvedStartup {
    pub invocation: Invocation,
    pub workspace: PathBuf,
}

pub enum StartupError {
    Help { text: String },
    Usage { message: String },
    Workspace { message: String },
}

fn load_defaults(dir: &Path) -> Result<FileDefaults, StartupError> {
    match dx_adopt::defaults::load_defaults(dir) {
        Ok((defaults, _)) => Ok(defaults),
        Err(error) => Err(StartupError::Usage {
            message: error.to_string(),
        }),
    }
}

fn skip_value<S: AsRef<OsStr>>(args: &[S], index: usize) -> usize {
    match args.get(index + 1) {
        Some(next) => {
            let raw = next.as_ref();
            let is_flag =
                raw == OsStr::new("--") || raw.to_str().is_some_and(|text| text.starts_with('-'));
            if is_flag {
                index + 1
            } else {
                index + 2
            }
        }
        None => index + 1,
    }
}

fn scans_help<S: AsRef<OsStr>>(args: &[S]) -> bool {
    if args
        .first()
        .is_some_and(|first| first.as_ref() == OsStr::new("bazel"))
    {
        return false;
    }
    let mut index = 0;
    let mut first_positional = true;
    while index < args.len() {
        let raw = args[index].as_ref();
        let Some(word) = raw.to_str() else {
            return false;
        };
        if word == "--" {
            return false;
        }
        if word == "-h" || word == "--help" || word == "-V" || word == "--version" {
            return true;
        }
        if word.starts_with('-') {
            let name = word.split_once('=').map_or(word, |(name, _)| name);
            if !word.contains('=') && VALUE_OPTIONS.contains(&name) {
                index = skip_value(args, index);
                continue;
            }
            index += 1;
            continue;
        }
        if first_positional && word == "help" {
            return true;
        }
        first_positional = false;
        index += 1;
    }
    false
}

fn flag_workspace<S: AsRef<OsStr>>(args: &[S]) -> Option<String> {
    if args
        .first()
        .is_some_and(|first| first.as_ref() == OsStr::new("bazel"))
    {
        return None;
    }
    let mut selected: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        let raw = args[index].as_ref();
        let Some(word) = raw.to_str() else {
            index += 1;
            continue;
        };
        if word == "--" {
            break;
        }
        if word.starts_with('-') {
            let (name, inline) = match word.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (word, None),
            };
            if name == "--workspace" {
                match inline {
                    Some(value) => selected = Some(value.to_owned()),
                    None => {
                        if let Some(next) = args.get(index + 1) {
                            if let Some(value) = next.as_ref().to_str() {
                                if value != "--" && !value.starts_with('-') {
                                    selected = Some(value.to_owned());
                                }
                            }
                        }
                    }
                };
                index += 1;
                continue;
            }
            if inline.is_none() && VALUE_OPTIONS.contains(&name) {
                index = skip_value(args, index);
                continue;
            }
            index += 1;
            continue;
        }
        index += 1;
    }
    selected
}

fn explicit_workspace<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if let Some(flag) = flag_workspace(args) {
        return Some(flag);
    }
    dx_adopt::defaults::env_string(env_get, dx_adopt::defaults::DX_WORKSPACE_ENV)
}

fn falls_back(invocation: &Invocation) -> bool {
    match invocation.command {
        Command::Init | Command::New | Command::Completion => true,
        Command::Version => {
            invocation.dry_run
                || !(invocation.pin.is_some() || invocation.rollback || invocation.check)
        }
        _ => false,
    }
}

fn fallback_dir(start: &Path, raw: &Path) -> PathBuf {
    let display = dx_process::resolve_override_display(raw, start);
    dx_process::canonicalize_or_keep(&display)
}

fn under(start: &Path, workspace: &Path) -> bool {
    start == workspace || start.starts_with(workspace)
}

fn effective_selection(start: &Path, selection: &str) -> PathBuf {
    let raw = Path::new(selection);
    let display = dx_process::resolve_override_display(raw, start);
    dx_process::canonicalize_or_keep(&display)
}

fn finalize<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    loaded: &FileDefaults,
    workspace: PathBuf,
) -> Result<ResolvedStartup, StartupError> {
    match parse_with(args, env_get, loaded) {
        Ok(invocation) => Ok(ResolvedStartup {
            invocation,
            workspace,
        }),
        Err(ArgsError::Help { text }) => Err(StartupError::Help { text }),
        Err(error) => Err(StartupError::Usage {
            message: error.to_string(),
        }),
    }
}

/// Resolve the command and the workspace before loading defaults.
pub fn resolve<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    cwd: &Path,
) -> Result<ResolvedStartup, StartupError> {
    let start = dx_process::workspace_start(cwd);
    if scans_help(args) {
        match parse_with(args, env_get, &FileDefaults::default()) {
            Ok(_) => {}
            Err(ArgsError::Help { text }) => return Err(StartupError::Help { text }),
            Err(error) => {
                return Err(StartupError::Usage {
                    message: error.to_string(),
                });
            }
        }
    }
    let provisional = load_defaults(&start)?;
    let early = match parse_with(args, env_get, &provisional) {
        Ok(invocation) => invocation,
        Err(ArgsError::Help { text }) => return Err(StartupError::Help { text }),
        Err(error) => {
            return Err(StartupError::Usage {
                message: error.to_string(),
            });
        }
    };
    let explicit = explicit_workspace(args, env_get);
    let mut selection = early.workspace.clone();
    let mut loaded = provisional;
    let mut visited: Vec<PathBuf> = Vec::new();
    let mut chain: Vec<String> = Vec::new();
    let workspace = loop {
        let Some(selected) = selection.clone() else {
            match dx_process::discover_real(&start, None) {
                Ok(found) => break found,
                Err(error) => {
                    if falls_back(&early) {
                        break start.clone();
                    }
                    return Err(StartupError::Workspace {
                        message: error.to_string(),
                    });
                }
            }
        };
        let raw = Path::new(&selected);
        match dx_process::discover_real(&start, Some(raw)) {
            Err(error) => {
                if falls_back(&early) {
                    break fallback_dir(&start, raw);
                }
                return Err(StartupError::Workspace {
                    message: error.to_string(),
                });
            }
            Ok(found) => {
                let anchor = if under(&start, &found) {
                    start.clone()
                } else {
                    found.clone()
                };
                let next = load_defaults(&anchor)?;
                if explicit.is_some() {
                    loaded = next;
                    break found;
                }
                let next_selection = next.workspace.clone();
                let converged = next_selection
                    .as_deref()
                    .is_none_or(|next| effective_selection(&start, next) == found);
                loaded = next;
                if converged {
                    break found;
                }
                if visited.contains(&found) {
                    chain.push(found.display().to_string());
                    return Err(StartupError::Usage {
                        message: format!("workspace redirect cycle: {}", chain.join(" -> ")),
                    });
                }
                if visited.len() >= MAX_REDIRECTS {
                    chain.push(found.display().to_string());
                    return Err(StartupError::Usage {
                        message: format!(
                            "workspace selection did not converge after {MAX_REDIRECTS} redirects: {}",
                            chain.join(" -> ")
                        ),
                    });
                }
                chain.push(found.display().to_string());
                visited.push(found);
                selection = next_selection;
            }
        }
    };
    finalize(args, env_get, &loaded, workspace)
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod startup_tests;
