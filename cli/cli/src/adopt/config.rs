use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use dx_error::DxError;
use dx_diff::{FilePatch, PatchKind, render_patch};
use dx_generate::tools_gen::{TOOL_CONFIGS, ToolConfig};
use dx_output::{
    ConfigFileEvent, FinishedCounts, OutputMode, command_finished, command_started,
    config_file_event,
};
use dx_plan::args::Invocation;

use crate::exec::common::{check_stdout_write, emit_built, report_dx_error};

const CODE_CONFIG_FAILED: &str = "config_failed";
const TIER_BASE: &str = "tier base";

/// One tool: its root file when the workspace has one, the base it reads and whether you
/// edited the root file.
struct Entry {
    tool: &'static str,
    root: Option<String>,
    base: String,
    extends: bool,
    edited: bool,
}

impl Entry {
    fn relation(&self) -> String {
        match (&self.root, self.extends) {
            (None, _) => TIER_BASE.to_owned(),
            (Some(_), true) => format!("extends {}", self.base),
            (Some(_), false) => format!("override of {}", self.base),
        }
    }

    fn text_line(&self) -> String {
        let root = self.root.as_deref().unwrap_or("-");
        let mut line = format!("{}  {}  {}", self.tool, root, self.relation());
        if self.root.is_none() {
            line.push(' ');
            line.push_str(&self.base);
        }
        if self.edited {
            line.push_str("  edited");
        }
        line
    }
}

pub(super) fn root_names(workspace: &Path) -> BTreeSet<String> {
    std::fs::read_dir(workspace)
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn entry(workspace: &Path, config: &ToolConfig, root_files: &BTreeSet<String>) -> Option<Entry> {
    let base = dx_generate::base_path(config)?;
    let root = dx_generate::root_file(config, root_files).map(str::to_owned);
    let edited = root.as_ref().is_some_and(|name| {
        std::fs::read_to_string(workspace.join(name))
            .is_ok_and(|text| dx_adopt::ownership::override_base(&text).is_some())
    });
    Some(Entry {
        tool: config.id,
        root,
        base,
        extends: !config.extends_syntax.is_empty(),
        edited,
    })
}

fn selected(workspace: &Path) -> Result<Vec<Entry>, DxError> {
    let config = dx_config::load(workspace).map_err(|error| DxError::failure(error.to_string()))?;
    let families = dx_adopt::detected_families(workspace);
    let root_files = root_names(workspace);
    let tools = dx_generate::selected_configs(config.as_ref(), &families)
        .map_err(|error| DxError::failure(error.to_string()))?;
    let mut entries: Vec<Entry> = tools
        .into_iter()
        .filter_map(|tool| entry(workspace, tool, &root_files))
        .collect();
    entries.sort_by_key(|entry| entry.tool);
    Ok(entries)
}

fn one(workspace: &Path, tool: &str) -> Result<Entry, DxError> {
    TOOL_CONFIGS
        .iter()
        .filter(|config| config.id == tool)
        .find_map(|config| entry(workspace, config, &root_names(workspace)))
        .ok_or_else(|| {
            DxError::usage(format!(
                "unknown tool {tool:?}: it has no dx-owned base to list"
            ))
        })
}

fn files(entry: &Entry) -> Vec<ConfigFileEvent> {
    let event = |file: &str, role: &str, base: Option<String>, edited| ConfigFileEvent {
        tool: entry.tool.to_owned(),
        file: file.to_owned(),
        role: role.to_owned(),
        base,
        edited: Some(edited),
    };
    let mut files = Vec::new();
    if let Some(root) = &entry.root {
        files.push(event(root, "root", Some(entry.relation()), entry.edited));
    }
    files.push(event(
        &entry.base,
        "base",
        entry.root.is_none().then(|| TIER_BASE.to_owned()),
        false,
    ));
    files
}

fn diff(workspace: &Path, entry: &Entry) -> Result<String, DxError> {
    let Some(root) = &entry.root else {
        return Err(DxError::usage(format!(
            "{} has no root file, so there is no override to diff",
            entry.tool
        )));
    };
    let read = |name: &str| {
        std::fs::read_to_string(workspace.join(name)).map_err(|error| {
            DxError::from_io(format!("cannot read {name}: {error}; run dx sync"), &error)
        })
    };
    let base = read(&entry.base)?;
    let current = read(root)?;
    let (_, body) = dx_adopt::ownership::split_override_header(&current);
    if body == base {
        return Ok(String::new());
    }
    render_patch(&[FilePatch {
        path: root,
        kind: PatchKind::Modify,
        original: &base,
        candidate: body,
    }])
    .map_err(|error| DxError::failure(error.to_string()))
}

fn tool_config(tool: &str) -> Result<&'static ToolConfig, DxError> {
    TOOL_CONFIGS
        .iter()
        .find(|config| config.id == tool && dx_generate::base_path(config).is_some())
        .ok_or_else(|| {
            DxError::usage(format!(
                "unknown tool {tool:?}: it has no dx-owned base to override"
            ))
        })
}

pub(super) fn blob_sha(workspace: &Path, base: &str) -> Result<String, DxError> {
    let output = dx_process::capture::run_checked(
        dx_process::capture::git(workspace).args(["hash-object", "--", base]),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .map_err(|error| DxError::infra(error.to_string()))?;
    Ok(output.stdout_text().trim().to_owned())
}

/// Copies the base to the tool's root file behind a `dx: override of base <sha>` comment and
/// returns the file written.
fn override_tool(workspace: &Path, tool: &str) -> Result<String, DxError> {
    let config = tool_config(tool)?;
    let base = dx_generate::base_path(config).ok_or_else(|| DxError::failure("no base"))?;
    if !config.extends_syntax.is_empty() {
        let line = config.extends_syntax.replace("{base}", &base);
        return Err(DxError::failure(format!(
            "{tool} extends its base, so add this to its root file instead of copying the base:\n{line}"
        )));
    }
    if let Some(root) = dx_generate::root_file(config, &root_names(workspace)) {
        return Err(DxError::failure(format!(
            "{root} already exists and {tool} cannot extend the base; remove it first to override again"
        )));
    }
    let name = config
        .files
        .iter()
        .find(|name| !name.contains('*'))
        .ok_or_else(|| DxError::failure(format!("{tool} has no root file name to write")))?;
    if config.comment.is_empty() {
        return Err(DxError::failure(format!(
            "{tool} files take no comment, so no header can be written"
        )));
    }
    let text = std::fs::read_to_string(workspace.join(&base)).map_err(|error| {
        DxError::from_io(format!("cannot read {base}: {error}; run dx sync"), &error)
    })?;
    let sha = blob_sha(workspace, &base)?;
    let header = format!("{} dx: override of base {sha}\n", config.comment);
    std::fs::write(workspace.join(name), format!("{header}{text}"))
        .map_err(|error| DxError::from_io(format!("cannot write {name}: {error}"), &error))?;
    Ok((*name).to_owned())
}

fn emit_files(
    invocation: &Invocation,
    out: &mut dyn Write,
    events: &[ConfigFileEvent],
    lines: &[String],
) -> Result<(), i32> {
    if invocation.output == OutputMode::Json {
        for event in events {
            emit_built(out, config_file_event(event))?;
        }
        return Ok(());
    }
    for line in lines {
        check_stdout_write(writeln!(out, "{line}"))?;
    }
    Ok(())
}

pub(super) fn execute_config(
    invocation: &Invocation,
    workspace: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let words: Vec<&str> = invocation.targets.iter().map(String::as_str).collect();
    let json = invocation.output == OutputMode::Json;
    let fail = |out: &mut dyn Write, err: &mut dyn Write, error: &DxError| {
        report_dx_error(invocation.output, out, err, CODE_CONFIG_FAILED, error)
    };
    if json && let Err(code) = emit_built(out, command_started("config", "default")) {
        return code;
    }
    let result = match words.as_slice() {
        ["diff", tool] => match one(workspace, tool).and_then(|entry| diff(workspace, &entry)) {
            Ok(patch) => check_stdout_write(write!(out, "{patch}")),
            Err(error) => return fail(out, err, &error),
        },
        ["override", tool] => match override_tool(workspace, tool) {
            Ok(name) => check_stdout_write(writeln!(out, "{name}")),
            Err(error) => return fail(out, err, &error),
        },
        [tool] => match one(workspace, tool) {
            Ok(entry) => {
                let events = files(&entry);
                let lines: Vec<String> = events.iter().map(|event| event.file.clone()).collect();
                emit_files(invocation, out, &events, &lines)
            }
            Err(error) => return fail(out, err, &error),
        },
        _ => match selected(workspace) {
            Ok(entries) => {
                let mut events = Vec::new();
                let mut lines = Vec::new();
                for entry in &entries {
                    let all = files(entry);
                    events.extend(
                        all.into_iter()
                            .filter(|event| event.role == "root" || entry.root.is_none()),
                    );
                    lines.push(entry.text_line());
                }
                emit_files(invocation, out, &events, &lines)
            }
            Err(error) => return fail(out, err, &error),
        },
    };
    if let Err(code) = result {
        return code;
    }
    if json && let Err(code) = emit_built(out, Ok(command_finished(0, &FinishedCounts::default())))
    {
        return code;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{env, events_of_kind, invocation, json_events};

    fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch("config-verb-");
        for (name, text) in files {
            let path = scratch.path().join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            std::fs::write(path, text).expect("write");
        }
        scratch
    }

    fn run(scratch: &Path, words: &[&str]) -> (i32, String) {
        let mut out = Vec::new();
        let code = super::super::execute_adoption(
            &invocation(words),
            env(scratch, &mut out, &mut Vec::new()),
        );
        (code, String::from_utf8(out).expect("utf8"))
    }

    fn run_err(scratch: &Path, words: &[&str]) -> (i32, String) {
        let mut err = Vec::new();
        let code = super::super::execute_adoption(
            &invocation(words),
            env(scratch, &mut Vec::new(), &mut err),
        );
        (code, String::from_utf8(err).expect("utf8"))
    }

    const FILES: &[(&str, &str)] = &[
        ("MODULE.bazel", ""),
        ("a.py", ""),
        ("b.hs", ""),
        ("ruff.toml", "extend = \".dx/config/ruff/base.toml\"\n"),
        (".dx/config/ruff/base.toml", "[lint]\n"),
        (".dx/config/hlint/base.yaml", "- warn: {name: x}\n"),
    ];

    #[test]
    fn lists_one_line_per_selected_tool() {
        let scratch = workspace(FILES);
        let (code, out) = run(scratch.path(), &["config"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("taplo  -  tier base .dx/config/taplo/base.toml\n"),
            "{out}"
        );
        assert!(
            out.contains("ruff  ruff.toml  extends .dx/config/ruff/base.toml\n"),
            "{out}"
        );
        assert!(!out.contains("edited"), "{out}");
    }

    #[test]
    fn marks_a_root_file_with_the_override_header() {
        let mut files = FILES.to_vec();
        files.push((
            "ruff.toml",
            "# dx: override of base abc123\nextend = \".dx/config/ruff/base.toml\"\n",
        ));
        let scratch = workspace(&files);
        let (_, out) = run(scratch.path(), &["config"]);
        assert!(
            out.contains("ruff  ruff.toml  extends .dx/config/ruff/base.toml  edited\n"),
            "{out}"
        );
    }

    #[test]
    fn a_tool_lists_the_files_it_reads_in_order() {
        let mut files = FILES.to_vec();
        files.push((".hlint.yaml", "- warn: {name: y}\n"));
        let scratch = workspace(&files);
        let (code, out) = run(scratch.path(), &["config", "hlint"]);
        assert_eq!(code, 0);
        assert_eq!(out, ".hlint.yaml\n.dx/config/hlint/base.yaml\n");
        let (_, out) = run(scratch.path(), &["config", "ruff"]);
        assert_eq!(out, "ruff.toml\n.dx/config/ruff/base.toml\n");
    }

    #[test]
    fn json_has_one_event_per_tool() {
        let scratch = workspace(FILES);
        let (code, out) = run(scratch.path(), &["config", "--output=json"]);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let files = events_of_kind(&events, "config_file");
        let ruff = files
            .iter()
            .find(|file| file["tool"] == "ruff")
            .expect("ruff");
        assert_eq!(ruff["role"], "root");
        assert_eq!(ruff["file"], "ruff.toml");
        assert_eq!(ruff["edited"], false);
    }

    #[test]
    fn diff_shows_the_override_against_its_base() {
        let mut files = FILES.to_vec();
        files.push((".hlint.yaml", "- warn: {name: y}\n"));
        let scratch = workspace(&files);
        let (code, out) = run(scratch.path(), &["config", "diff", "hlint"]);
        assert_eq!(code, 0);
        assert!(out.contains("-- warn: {name: x}\n"), "{out}");
        assert!(out.contains("+- warn: {name: y}\n"), "{out}");
    }

    #[test]
    fn override_copies_the_base_behind_a_header() {
        let scratch = workspace(FILES);
        let git = |args: &[&str]| {
            dx_process::capture::run_checked(
                dx_process::capture::git(scratch.path()).args(args),
                dx_process::capture::DEFAULT_TIMEOUT,
            )
            .expect("git")
        };
        git(&["init", "-q"]);
        let (code, out) = run(scratch.path(), &["config", "override", "hlint"]);
        assert_eq!(code, 0, "{out}");
        let sha = git(&["hash-object", ".dx/config/hlint/base.yaml"]).stdout_text();
        let text = std::fs::read_to_string(scratch.path().join(".hlint.yaml")).expect("root");
        assert_eq!(
            text,
            format!(
                "# dx: override of base {}\n- warn: {{name: x}}\n",
                sha.trim()
            )
        );
        assert_eq!(dx_adopt::ownership::override_base(&text), Some(sha.trim()));
        assert_ne!(run(scratch.path(), &["config", "override", "hlint"]).0, 0);
    }

    #[test]
    fn diff_of_a_fresh_override_shows_no_header() {
        let scratch = workspace(FILES);
        dx_process::capture::run_checked(
            dx_process::capture::git(scratch.path()).args(["init", "-q"]),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .expect("git init");
        assert_eq!(run(scratch.path(), &["config", "override", "hlint"]).0, 0);
        let (code, out) = run(scratch.path(), &["config", "diff", "hlint"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out, "");
    }

    #[test]
    fn override_refuses_a_tool_that_extends_its_base() {
        let scratch = workspace(FILES);
        let (code, err) = run_err(scratch.path(), &["config", "override", "ruff"]);
        assert_ne!(code, 0);
        assert!(
            err.contains("extend = \".dx/config/ruff/base.toml\""),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("ruff.toml")).expect("root"),
            "extend = \".dx/config/ruff/base.toml\"\n"
        );
    }

    #[test]
    fn unknown_tool_and_missing_override_fail() {
        let scratch = workspace(FILES);
        assert_ne!(run(scratch.path(), &["config", "nope"]).0, 0);
        assert_ne!(run(scratch.path(), &["config", "diff", "hlint"]).0, 0);
    }

    #[test]
    fn errors_carry_their_kind() {
        let scratch = workspace(FILES);
        let unknown = one(scratch.path(), "nope").err().expect("unknown tool");
        assert_eq!(unknown.kind(), dx_error::ErrorKind::Usage);
        let extends = override_tool(scratch.path(), "ruff").expect_err("extends");
        assert_eq!(extends.kind(), dx_error::ErrorKind::Failure);
        let (code, _) = run_err(scratch.path(), &["config", "nope"]);
        assert_eq!(code, 2);
    }
}
