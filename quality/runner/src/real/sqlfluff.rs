use std::collections::BTreeMap;
use std::path::Path;

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers, sqlfluff_preflight};

use super::*;

/// File names sqlfluff reads as config; only the bound config may be one of these.
const OTHER_CONFIG_NAMES: &[&str] = &["setup.cfg", "tox.ini", "pep8.ini", "pyproject.toml"];
const NESTED_CONFIG_NAME: &str = ".sqlfluff";

fn dir_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn name_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or(rel, |(_, name)| name)
}

fn within(dir: &str, root: &str) -> bool {
    root.is_empty() || dir == root || dir.starts_with(&format!("{root}/"))
}

impl RealBackend {
    /// Preflights the whole staged config closure and the inline directives of `files`.
    ///
    /// The bound config must name a known dialect. Every other staged `.sqlfluff` is a nested
    /// config that must sit at or below the bound config's directory. Any other staged file that
    /// sqlfluff would read as config is refused, because without `--ignore-local-config` it would
    /// silently join the layers. Check and fix share this so they cannot drift apart.
    pub(crate) fn sqlfluff_preflight_closure(
        tool_id: &str,
        tool: &RealTool,
        files: &[&Path],
    ) -> Result<(), RunnerError> {
        let Some(bound) = tool.config_rel.as_deref() else {
            return Err(execution(tool_id, "sqlfluff requires a config".to_owned()));
        };
        let bound_dir = dir_of(bound);
        for (rel, bytes) in &tool.tool_files {
            let is_bound = rel == bound;
            let name = name_of(rel);
            if !is_bound && name != NESTED_CONFIG_NAME && !OTHER_CONFIG_NAMES.contains(&name) {
                continue;
            }
            if !is_bound && name != NESTED_CONFIG_NAME {
                return Err(execution(
                    tool_id,
                    format!(
                        "{rel} would be read as a sqlfluff config; only the bound config and nested `.sqlfluff` files may be staged"
                    ),
                ));
            }
            if !is_bound && !within(dir_of(rel), bound_dir) {
                return Err(execution(
                    tool_id,
                    format!("nested sqlfluff config {rel} must sit at or below {bound_dir:?}"),
                ));
            }
            let text = std::str::from_utf8(bytes)
                .map_err(|err| execution(tool_id, format!("read config {rel}: {err}")))?;
            let checked = if is_bound {
                sqlfluff_preflight::check_config(text)
            } else {
                sqlfluff_preflight::check_nested_config(text)
                    .map_err(|err| format!("{err} (in {rel})"))
            };
            checked.map_err(|err| execution(tool_id, err))?;
        }
        for file in files {
            // Unreadable or non-UTF-8 files are left to sqlfluff to report.
            if let Ok(text) = std::fs::read_to_string(file) {
                sqlfluff_preflight::check_inline_config(&text)
                    .map_err(|err| execution(tool_id, format!("{err} (in {})", file.display())))?;
            }
        }
        Ok(())
    }

    /// The directory sqlfluff runs in for `file_rel`: the deepest staged nested config's
    /// directory that contains it, else the scratch root. sqlfluff takes the templater from the
    /// config at its cwd, so a group must not mix directories.
    pub(crate) fn sqlfluff_config_dir(tool: &RealTool, file_rel: &str) -> String {
        let bound = tool.config_rel.as_deref().unwrap_or("");
        tool.tool_files
            .iter()
            .filter(|(rel, _)| rel != bound && name_of(rel) == NESTED_CONFIG_NAME)
            .map(|(rel, _)| dir_of(rel))
            .filter(|dir| !dir.is_empty() && file_rel.starts_with(&format!("{dir}/")))
            .max_by_key(|dir| dir.len())
            .unwrap_or("")
            .to_owned()
    }

    /// The config directory of `file` inside `root`, or None when it is not inside it.
    pub(crate) fn sqlfluff_config_dir_abs(
        tool: &RealTool,
        root: &Path,
        file: &Path,
    ) -> Option<String> {
        let rel = file
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        Some(Self::sqlfluff_config_dir(tool, &rel))
    }

    /// Preflights the closure, then lints every owned file with one sqlfluff run per config
    /// directory. A format check lints with only the rules `sqlfluff format` applies.
    pub(super) fn check_sqlfluff(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        format: bool,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "sqlfluff has no delegated mode".to_owned(),
            ));
        }
        let files: Vec<&Path> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        Self::sqlfluff_preflight_closure(tool_id, tool, &files)?;
        let mut groups: BTreeMap<String, Vec<&Path>> = BTreeMap::new();
        for (rel, absolute) in &staged.pairs {
            groups
                .entry(Self::sqlfluff_config_dir(tool, rel))
                .or_default()
                .push(absolute.as_path());
        }
        let mut findings = Vec::new();
        for (dir, group) in groups {
            let invocation = if format {
                commands::sqlfluff_format_check(&tool.binary, &group, &dir)
            } else {
                commands::sqlfluff_check(&tool.binary, &group, &dir)
            };
            let out = self.run(tool_id, tool, &invocation, &staged.scratch)?;
            let names: Vec<String> = group
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            findings.extend(parsed(
                tool_id,
                parsers::parse_sqlfluff(&out.stdout, &out.stderr, out.code, &strs),
            )?);
        }
        Ok(findings)
    }
}
