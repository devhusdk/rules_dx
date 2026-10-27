//! The effective Bazel `build` options from rc files, sections, and configs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{import_path, unquote, MAX_IMPORT_DEPTH};

const MAX_CONFIG_DEPTH: usize = 32;

const LEVELS: [&[&str]; 2] = [&["common", "always"], &["build"]];

struct Line {
    command: String,
    config: Option<String>,
    tokens: Vec<String>,
}

/// Whether the effective `build` options set a non-empty `--remote_executor`.
pub fn remote_execution(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    read_user_rc: bool,
    startup: &[String],
    flags: &[String],
) -> bool {
    let options = build_options(env_get, workspace, read_file, read_user_rc, startup, flags);
    last_value(&options, "--remote_executor").is_some_and(|value| !value.is_empty())
}

/// The `build` options Bazel would apply, rc files first and `flags` last.
fn build_options(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    read_user_rc: bool,
    startup: &[String],
    flags: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    for path in rc_files(env_get, workspace, read_user_rc, startup) {
        load(&path, workspace, read_file, &mut seen, 0, &mut lines);
    }
    let mut out = Vec::new();
    let mut stack = Vec::new();
    expand(&lines, None, &mut stack, &mut out);
    if platform_config_enabled(&out) {
        expand_config(&lines, std::env::consts::OS, &mut stack, &mut out);
    }
    expand_tokens(&lines, flags, &mut stack, &mut out);
    out
}

/// The rc files in Bazel's read order: system, workspace, home, `BAZELRC`, then `--bazelrc`.
fn rc_files(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_user_rc: bool,
    startup: &[String],
) -> Vec<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    let mut files = Vec::new();
    if read_user_rc {
        if cfg!(windows) {
            if let Some(data) = non_empty("ProgramData") {
                files.push(PathBuf::from(data).join("bazel.bazelrc"));
            }
        } else {
            files.push(PathBuf::from("/etc/bazel.bazelrc"));
        }
    }
    files.push(workspace.join(".bazelrc"));
    if read_user_rc {
        if let Some(home) = non_empty("HOME").or_else(|| non_empty("USERPROFILE")) {
            files.push(PathBuf::from(home).join(".bazelrc"));
        }
        files.extend(explicit_rc_files(env_get, startup));
    }
    files
}

/// The files named by `BAZELRC` (comma-separated) and then each `--bazelrc`. `/dev/null` ends a list.
fn explicit_rc_files(env_get: &dyn Fn(&str) -> Option<String>, startup: &[String]) -> Vec<PathBuf> {
    let env_paths = env_get("BAZELRC")
        .unwrap_or_default()
        .split(',')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut flag_paths = Vec::new();
    let mut index = 0;
    while index < startup.len() {
        let option = startup[index].as_str();
        if let Some(path) = option.strip_prefix("--bazelrc=") {
            flag_paths.push(path.to_owned());
        } else if option == "--bazelrc" {
            if let Some(path) = startup.get(index + 1) {
                flag_paths.push(path.clone());
                index += 1;
            }
        }
        index += 1;
    }
    env_paths
        .iter()
        .chain(flag_paths.iter())
        .map(String::as_str)
        .take_while(|path| *path != "/dev/null")
        .map(PathBuf::from)
        .collect()
}

fn load(
    path: &Path,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    seen: &mut HashSet<PathBuf>,
    depth: usize,
    lines: &mut Vec<Line>,
) {
    if depth > MAX_IMPORT_DEPTH || !seen.insert(path.to_path_buf()) {
        return;
    }
    let Some(text) = read_file(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace().map(unquote);
        let Some(first) = words.next() else {
            continue;
        };
        if matches!(first, "import" | "try-import") {
            let rest = line[first.len()..].trim();
            let target = import_path(unquote(rest), workspace);
            load(&target, workspace, read_file, seen, depth + 1, lines);
            continue;
        }
        let (command, config) = match first.split_once(':') {
            Some((command, config)) => (command, Some(config.to_owned())),
            None => (first, None),
        };
        lines.push(Line {
            command: command.to_owned(),
            config,
            tokens: words.map(str::to_owned).collect(),
        });
    }
}

fn expand(lines: &[Line], config: Option<&str>, stack: &mut Vec<String>, out: &mut Vec<String>) {
    for level in LEVELS {
        for line in lines {
            if level.contains(&line.command.as_str()) && line.config.as_deref() == config {
                expand_tokens(lines, &line.tokens, stack, out);
            }
        }
    }
}

fn expand_config(lines: &[Line], name: &str, stack: &mut Vec<String>, out: &mut Vec<String>) {
    if stack.len() >= MAX_CONFIG_DEPTH || stack.iter().any(|open| open == name) {
        return;
    }
    stack.push(name.to_owned());
    expand(lines, Some(name), stack, out);
    stack.pop();
}

fn expand_tokens(
    lines: &[Line],
    tokens: &[String],
    stack: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if let Some(name) = token.strip_prefix("--config=") {
            expand_config(lines, name, stack, out);
        } else if token == "--config" {
            if let Some(name) = tokens.get(index + 1) {
                expand_config(lines, name, stack, out);
                index += 1;
            }
        } else {
            out.push(token.to_owned());
        }
        index += 1;
    }
}

fn platform_config_enabled(options: &[String]) -> bool {
    let mut enabled = false;
    for option in options {
        match option.as_str() {
            "--enable_platform_specific_config"
            | "--enable_platform_specific_config=true"
            | "--enable_platform_specific_config=yes"
            | "--enable_platform_specific_config=1" => enabled = true,
            "--noenable_platform_specific_config"
            | "--enable_platform_specific_config=false"
            | "--enable_platform_specific_config=no"
            | "--enable_platform_specific_config=0" => enabled = false,
            _ => {}
        }
    }
    enabled
}

fn last_value<'a>(options: &'a [String], flag: &str) -> Option<&'a str> {
    let mut value = None;
    for (index, option) in options.iter().enumerate() {
        let Some(rest) = option.strip_prefix(flag) else {
            continue;
        };
        if rest.is_empty() {
            value = Some(options.get(index + 1).map_or("", String::as_str));
        } else if let Some(set) = rest.strip_prefix('=') {
            value = Some(set);
        }
    }
    value
}

#[cfg(test)]
#[path = "rc_sections_tests.rs"]
mod rc_sections_tests;
