//! The effective Bazel `build` options from rc files, sections, and configs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{flag_value, import_path, sets_flag, tokenize, unquote, MAX_IMPORT_DEPTH};

const MAX_CONFIG_DEPTH: usize = 32;

/// The command names whose lines apply to `command`, least specific first.
fn levels(command: &str) -> &'static [&'static [&'static str]] {
    match command {
        "test" => &[&["common", "always"], &["build"], &["test"]],
        "coverage" => &[&["common", "always"], &["build"], &["test"], &["coverage"]],
        "run" => &[&["common", "always"], &["build"], &["run"]],
        _ => &[&["common", "always"], &["build"]],
    }
}

/// One rc line: its command, optional `:config`, and flag tokens.
pub(super) struct Line {
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
    let lines = load_rc(env_get, workspace, read_file, read_user_rc, startup);
    let options = effective_options(&lines, "build", flags);
    last_value(&options, "--remote_executor").is_some_and(|value| !value.is_empty())
}

const NESTED_BAZEL_TAG: &str = "resources:nested_bazel";

/// Whether the effective `test` options leave `resources:nested_bazel` tests selected by `--test_tag_filters`.
pub fn nested_bazel_selected(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    read_user_rc: bool,
    startup: &[String],
    flags: &[String],
) -> bool {
    let lines = load_rc(env_get, workspace, read_file, read_user_rc, startup);
    let options = effective_options(&lines, "test", flags);
    let Some(filters) = last_value(&options, "--test_tag_filters") else {
        return true;
    };
    let nested = |tag: &str| tag == NESTED_BAZEL_TAG || tag.starts_with("resources:nested_bazel:");
    let tags = filters
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty());
    let (mut included, mut any_included) = (false, false);
    for tag in tags {
        if let Some(excluded) = tag.strip_prefix('-') {
            if nested(excluded) {
                return false;
            }
        } else {
            any_included = true;
            included |= nested(tag);
        }
    }
    included || !any_included
}

/// Every rc line Bazel would read, imports inlined, in read order.
pub(super) fn load_rc(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    read_user_rc: bool,
    startup: &[String],
) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    for path in rc_files(env_get, workspace, read_user_rc, startup) {
        load(&path, workspace, read_file, &mut seen, 0, &mut lines);
    }
    lines
}

/// The options Bazel would apply to `command`, rc files first and `flags` last.
pub(super) fn effective_options(lines: &[Line], command: &str, flags: &[String]) -> Vec<String> {
    let levels = levels(command);
    let mut out = Vec::new();
    let mut stack = Vec::new();
    expand(lines, levels, None, &mut stack, &mut out);
    if platform_config_enabled(&out) {
        expand_config(lines, levels, std::env::consts::OS, &mut stack, &mut out);
    }
    expand_tokens(lines, levels, flags, &mut stack, &mut out);
    out
}

/// Whether any rc line, in any section or config, names one of `names`.
pub(super) fn any_line_sets(lines: &[Line], names: &[&str]) -> bool {
    lines.iter().any(|line| {
        line.tokens
            .iter()
            .any(|token| names.iter().any(|name| sets_flag(token, name)))
    })
}

/// The last disk cache setting in `options`: `Some(None)` for `--nodisk_cache` or an empty value.
pub(super) fn last_disk_cache(options: &[String]) -> Option<Option<&str>> {
    let mut winner = None;
    for (index, option) in options.iter().enumerate() {
        let next = options.get(index + 1).map(String::as_str);
        if sets_flag(option, "--nodisk_cache") {
            winner = Some(None);
        } else if sets_flag(option, "--disk_cache") {
            let value = flag_value(option, next, "--disk_cache").filter(|value| !value.is_empty());
            winner = Some(value);
        }
    }
    winner
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
        let words = tokenize(line);
        let Some(first) = words.first().map(String::as_str) else {
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
            tokens: words[1..].to_vec(),
        });
    }
}

fn expand(
    lines: &[Line],
    levels: &[&[&str]],
    config: Option<&str>,
    stack: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    for level in levels {
        for line in lines {
            if level.contains(&line.command.as_str()) && line.config.as_deref() == config {
                expand_tokens(lines, levels, &line.tokens, stack, out);
            }
        }
    }
}

fn expand_config(
    lines: &[Line],
    levels: &[&[&str]],
    name: &str,
    stack: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    if stack.len() >= MAX_CONFIG_DEPTH || stack.iter().any(|open| open == name) {
        return;
    }
    stack.push(name.to_owned());
    expand(lines, levels, Some(name), stack, out);
    stack.pop();
}

fn expand_tokens(
    lines: &[Line],
    levels: &[&[&str]],
    tokens: &[String],
    stack: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if let Some(name) = token.strip_prefix("--config=") {
            expand_config(lines, levels, name, stack, out);
        } else if token == "--config" {
            if let Some(name) = tokens.get(index + 1) {
                expand_config(lines, levels, name, stack, out);
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

pub(super) fn last_value<'a>(options: &'a [String], flag: &str) -> Option<&'a str> {
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
