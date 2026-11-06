use std::path::{Path, PathBuf};

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::*;

/// What a tool module needs to launch one check.
pub(super) struct CheckCall<'a> {
    pub tool_id: &'a str,
    pub tool: &'a RealTool,
    pub capability: &'a str,
    pub staged: &'a StagedScratch,
    pub config: Option<&'a Path>,
}

/// A tool that owns its launch and parse in its own module.
pub(super) struct ToolDef {
    pub id: &'static str,
    pub run: fn(&RealBackend, &CheckCall) -> Result<Vec<FileFinding>, RunnerError>,
}

pub(super) const TOOL_DEFS: &[&ToolDef] = &[
    &super::bicep::TOOL,
    &super::cfn_lint::TOOL,
    &super::cmakelint::TOOL,
    &super::helm::TOOL,
    &super::kubeconform::TOOL,
    &super::periphery::TOOL,
    &super::redocly::TOOL,
    &super::rustfmt::TOOL,
    &super::sqlfluff::TOOL,
    &super::weeder::TOOL,
];

pub(super) fn all_tool_defs() -> impl Iterator<Item = &'static ToolDef> {
    TOOL_DEFS
        .iter()
        .copied()
        .chain(super::config_tools::TOOLS.iter())
        .chain(super::delegated::TOOLS.iter())
        .chain(super::native::TOOLS.iter())
        .chain(super::python::TOOLS.iter())
        .chain(super::jvm::TOOLS.iter())
        .chain(super::schema::TOOLS.iter())
        .chain(super::scripting::TOOLS.iter())
        .chain(super::single_run::TOOLS.iter())
        .chain(super::web::TOOLS.iter())
}

impl RealBackend {
    pub(super) fn run_check(
        &self,
        tool_id: &str,
        tool: &RealTool,
        capability: &str,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let config = self.config_abs(tool_id, tool, scratch)?;
        if let Some(def) = all_tool_defs().find(|def| def.id == tool_id) {
            let call = CheckCall {
                tool_id,
                tool,
                capability,
                staged,
                config: config.as_deref(),
            };
            return (def.run)(self, &call);
        }
        Err(execution(
            tool_id,
            format!("unsupported real tool: {tool_id}"),
        ))
    }

    /// Runs one tool over the selection, in argv-budget batches when its transport allows.
    ///
    /// Findings merge in a stable order, and a failing batch names itself when there are
    /// several so the error points at the spawn that broke.
    /// Reads every staged notebook and refuses any that Ruff would skip, so a
    /// non-Python or malformed notebook never reports clean.
    /// `cue vet -c=false` once per package, by import path from the enclosing CUE module root, so
    /// imports and ancestor-directory files of the same package resolve as in the real instance.
    /// zanuda reads the typed trees the compile wrote, not the sources: the aspect stages each
    /// `.cmt`/`.cmti` as `dx-zanuda/{impl,intf}/<source workspace path>.cmt[i]`. This batch lints
    /// the entries whose source is in `pairs` and fails when an owned source has none, so a
    /// target built with `-no-bin-annot` never reads as clean.
    pub(super) fn check_zanuda(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let mut lines = Vec::with_capacity(pairs.len());
        for (workspace, absolute) in pairs {
            let kind = if workspace.ends_with(".mli") {
                ("intf", "cmti")
            } else {
                ("impl", "cmt")
            };
            let rel = format!("dx-zanuda/{}/{workspace}.{}", kind.0, kind.1);
            if !tool.tool_files.iter().any(|(known, _)| *known == rel) {
                return Err(execution(
                    tool_id,
                    format!(
                        "no .{} for {workspace}; was it compiled with -no-bin-annot?",
                        kind.1
                    ),
                ));
            }
            lines.push(format!(
                "{}\t{}\t{}",
                kind.0,
                absolute.display(),
                scratch.root().join(&rel).display()
            ));
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        let dir = scratch.root().join("dx-zanuda");
        std::fs::create_dir_all(&dir)
            .map_err(|err| execution(tool_id, format!("create {}: {err}", dir.display())))?;
        let list = dir.join("list.tsv");
        let report = dir.join("out.jsonl");
        std::fs::write(&list, lines.join("\n") + "\n")
            .map_err(|err| execution(tool_id, format!("write {}: {err}", list.display())))?;
        let invocation = commands::zanuda_check(&tool.binary, &list, &report);
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        if out.code != Some(0) {
            let tail = String::from_utf8_lossy(&out.stderr);
            return Err(execution(
                tool_id,
                format!(
                    "exit {}: {}",
                    out.code
                        .map_or_else(|| "by signal".to_owned(), |c| c.to_string()),
                    tail.trim()
                ),
            ));
        }
        let bytes = std::fs::read(&report)
            .map_err(|err| execution(tool_id, format!("read {}: {err}", report.display())))?;
        let names: Vec<String> = pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        parsed(tool_id, parsers::parse_zanuda(&bytes, out.code, &strs))
    }

    pub(super) fn check_cue_vet(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let modules: Vec<String> = staged
            .pairs
            .iter()
            .chain(staged.resolve_pairs.iter())
            .filter_map(|(ws, _)| {
                if ws == "cue.mod/module.cue" {
                    Some(String::new())
                } else {
                    ws.strip_suffix("/cue.mod/module.cue").map(str::to_owned)
                }
            })
            .collect();
        let mut groups: std::collections::BTreeSet<(String, String, String)> =
            std::collections::BTreeSet::new();
        for (ws, absolute) in &staged.pairs {
            if ws.starts_with("cue.mod/") || ws.contains("/cue.mod/") {
                continue;
            }
            let dir = ws.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
            let root = modules
                .iter()
                .filter(|root| {
                    root.is_empty() || dir == **root || dir.starts_with(&format!("{root}/"))
                })
                .max_by_key(|root| root.len())
                .cloned()
                .unwrap_or_else(|| dir.clone());
            let text = std::fs::read_to_string(absolute)
                .map_err(|err| execution(tool_id, format!("read {ws}: {err}")))?;
            let package = cue_package_clause(&text);
            groups.insert((root, dir, package));
        }
        let owned: Vec<String> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let owned: Vec<&str> = owned.iter().map(String::as_str).collect();
        let mut known_paths: Vec<String> = owned.iter().map(|path| (*path).to_owned()).collect();
        for (_, absolute) in &staged.resolve_pairs {
            let text = absolute.to_string_lossy().into_owned();
            if !known_paths.contains(&text) {
                known_paths.push(text);
            }
        }
        let known: Vec<&str> = known_paths.iter().map(String::as_str).collect();
        let mut findings = Vec::new();
        let mut reported = Vec::new();
        for (root, dir, package) in &groups {
            // Vet by import path so cue builds the real instance, which also unifies the staged
            // same-package files of ancestor directories up to the module root.
            let rel = if root.is_empty() {
                dir.as_str()
            } else {
                dir.strip_prefix(root.as_str())
                    .map_or("", |rest| rest.trim_start_matches('/'))
            };
            let mut target = if rel.is_empty() {
                ".".to_owned()
            } else {
                format!("./{rel}")
            };
            if !package.is_empty() {
                target.push(':');
                target.push_str(package);
            }
            let target = PathBuf::from(target);
            let invocation = commands::cue_vet(&tool.binary, &[target.as_path()], root);
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let cwd = scratch.root().join(root);
            let found = parsed(
                tool_id,
                parsers::parse_cue_vet(&out.stdout, &out.stderr, out.code, &owned, &known, &cwd),
            )?;
            for item in found {
                if owned.contains(&item.file.as_str()) {
                    findings.push(item);
                } else {
                    reported.push(item.file);
                }
            }
        }
        if findings.is_empty() && !reported.is_empty() {
            let mut names: Vec<String> = staged
                .resolve_pairs
                .iter()
                .filter(|(_, absolute)| {
                    reported
                        .iter()
                        .any(|r| r.as_str() == absolute.to_string_lossy())
                })
                .map(|(ws, _)| ws.clone())
                .collect();
            names.sort();
            names.dedup();
            return Err(execution(
                tool_id,
                format!(
                    "cue vet failed in dependencies before the owned files were checked: {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(findings)
    }

    /// `pkl eval` once per owned module with imports staged beside it. Pkl stops at the first
    /// error, so a batched run would lose findings and misattribute them. Abstract modules are
    /// only loaded (`-x 1`), since rendering one reports its by-design unset properties. Values
    /// are lazy: only what the module renders is forced, so hidden or unused members are not
    /// checked. An error whose first frame is a non-owned file is an execution error.
    pub(super) fn check_pkl_eval(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let owned: Vec<String> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let owned: Vec<&str> = owned.iter().map(String::as_str).collect();
        let mut known_paths: Vec<String> = owned.iter().map(|path| (*path).to_owned()).collect();
        for (_, absolute) in &staged.resolve_pairs {
            let text = absolute.to_string_lossy().into_owned();
            if !known_paths.contains(&text) {
                known_paths.push(text);
            }
        }
        let known: Vec<&str> = known_paths.iter().map(String::as_str).collect();
        let mut findings: Vec<FileFinding> = Vec::new();
        let mut reported = Vec::new();
        for (ws, absolute) in &staged.pairs {
            let text = std::fs::read_to_string(absolute)
                .map_err(|err| execution(tool_id, format!("read {ws}: {err}")))?;
            let load_only = text
                .lines()
                .any(|line| line.trim_start().starts_with("abstract module"));
            let invocation = commands::pkl_eval(&tool.binary, absolute, scratch.root(), load_only);
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let found = parsed(
                tool_id,
                parsers::parse_pkl_eval(&out.stdout, &out.stderr, out.code, &known),
            )?;
            for item in found {
                if owned.contains(&item.file.as_str()) {
                    if !findings.contains(&item) {
                        findings.push(item);
                    }
                } else {
                    reported.push(item.file);
                }
            }
        }
        if findings.is_empty() && !reported.is_empty() {
            let mut names: Vec<String> = staged
                .resolve_pairs
                .iter()
                .filter(|(_, absolute)| {
                    reported
                        .iter()
                        .any(|r| r.as_str() == absolute.to_string_lossy())
                })
                .map(|(ws, _)| ws.clone())
                .collect();
            names.sort();
            names.dedup();
            return Err(execution(
                tool_id,
                format!(
                    "pkl eval failed in dependencies before the owned files were checked: {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(findings)
    }

    /// `terraform init` then `terraform validate` once per directory that owns `.tf` files.
    ///
    /// Init is offline: every provider comes from the `-plugin-dir` mirrors the request names
    /// in `DX_TERRAFORM_PLUGIN_DIRS` (an empty directory when there are none, so init never
    /// reaches a registry). Init findings (syntax and configuration errors) skip validate for
    /// that directory. A failure that names no file is an execution error, never clean. The
    /// root run and a child module's own run both see the child's errors, so findings are
    /// de-duplicated. An error only in a non-owned file is an execution error, as for pkl.
    pub(super) fn check_terraform_validate(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let cwd = std::env::current_dir()
            .map_err(|err| execution(tool_id, format!("current dir: {err}")))?;
        let mut mirrors: Vec<PathBuf> = tool
            .extra_env
            .iter()
            .filter(|(key, _)| key == TERRAFORM_PLUGIN_DIRS_ENV)
            .flat_map(|(_, value)| value.split('|'))
            .filter(|entry| !entry.is_empty())
            .map(|entry| cwd.join(entry))
            .collect();
        if mirrors.is_empty() {
            let empty = scratch.root().join(TERRAFORM_EMPTY_MIRROR_REL);
            std::fs::create_dir_all(&empty)
                .map_err(|err| execution(tool_id, format!("empty mirror: {err}")))?;
            mirrors.push(empty);
        }
        let mirror_refs: Vec<&Path> = mirrors.iter().map(PathBuf::as_path).collect();
        let owned: Vec<String> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let mut known_paths = owned.clone();
        for (_, absolute) in &staged.resolve_pairs {
            let text = absolute.to_string_lossy().into_owned();
            if !known_paths.contains(&text) {
                known_paths.push(text);
            }
        }
        let known: Vec<&str> = known_paths.iter().map(String::as_str).collect();
        let sources: Vec<(String, PathBuf)> = staged
            .pairs
            .iter()
            .filter(|(ws, _)| ws.ends_with(".tf") || ws.ends_with(".tf.json"))
            .cloned()
            .collect();
        let mut findings: Vec<FileFinding> = Vec::new();
        let mut reported = Vec::new();
        for (dir, _) in group_by_parent(scratch.root(), &sources) {
            let abs_dir = if dir == "." {
                scratch.root().to_path_buf()
            } else {
                scratch.root().join(&dir)
            };
            let init = commands::terraform_init(&tool.binary, &abs_dir, &mirror_refs);
            let out = self.run(tool_id, tool, &init, scratch)?;
            let mut found = parsed(
                tool_id,
                parsers::parse_terraform_init(&out.stdout, &out.stderr, out.code, &abs_dir, &known),
            )?;
            if found.is_empty() {
                let validate = commands::terraform_validate(&tool.binary, &abs_dir);
                let out = self.run(tool_id, tool, &validate, scratch)?;
                found = parsed(
                    tool_id,
                    parsers::parse_terraform_validate(&out.stdout, out.code, &abs_dir, &known),
                )?;
            }
            for item in found {
                if owned.contains(&item.file) {
                    if !findings.contains(&item) {
                        findings.push(item);
                    }
                } else {
                    reported.push(item.file);
                }
            }
        }
        if findings.is_empty() && !reported.is_empty() {
            let mut names: Vec<String> = staged
                .resolve_pairs
                .iter()
                .filter(|(_, absolute)| {
                    reported
                        .iter()
                        .any(|r| r.as_str() == absolute.to_string_lossy())
                })
                .map(|(ws, _)| ws.clone())
                .collect();
            names.sort();
            names.dedup();
            return Err(execution(
                tool_id,
                format!(
                    "terraform reported errors in dependencies before the owned files were checked: {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(findings)
    }

    pub(super) fn check_files(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
        build: impl Fn(&[&Path]) -> Result<Invocation, RunnerError>,
        parse: impl Fn(&ChildOutput, &[&str]) -> Result<Vec<FileFinding>, RunnerError>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let refs: Vec<&Path> = pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let names: Vec<String> = pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        let batches = match commands::transport_for(tool_id) {
            commands::ArgTransport::Batched => {
                let head = spawn_argv(tool_id, &build(&[])?);
                commands::plan_batches(&head, &refs)
            }
            commands::ArgTransport::Single => vec![refs.clone()],
        };
        let total = batches.len();
        let mut offset = 0usize;
        let mut merged: Vec<FileFinding> = Vec::new();
        for (index, batch) in batches.iter().enumerate() {
            let batch_names = &strs[offset..offset + batch.len()];
            offset += batch.len();
            let report = build(batch.as_slice())
                .and_then(|invocation| self.run(tool_id, tool, &invocation, scratch))
                .and_then(|out| parse(&out, batch_names))
                .map_err(|err| in_batch(err, index + 1, total))?;
            merged.extend(report);
        }
        merged.sort_by(|left, right| {
            (
                left.file.as_str(),
                left.finding.start.line,
                left.finding.start.column,
                left.finding.rule_id.as_str(),
                left.finding.message.as_str(),
            )
                .cmp(&(
                    right.file.as_str(),
                    right.finding.start.line,
                    right.finding.start.column,
                    right.finding.rule_id.as_str(),
                    right.finding.message.as_str(),
                ))
        });
        Ok(merged)
    }
}

/// Drops the scratch directory from `file://` links so messages read the same on every host.
pub(super) fn without_scratch_url(message: &str, root: &Path) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let root = root.trim_start_matches('/');
    message.replace(&format!("file:///{root}/"), "")
}

/// Names the file whose output failed to parse.
pub(super) fn in_file(err: RunnerError, name: &str) -> RunnerError {
    match err {
        RunnerError::ToolExecution { tool_id, detail } => RunnerError::ToolExecution {
            tool_id,
            detail: format!("{name}: {detail}"),
        },
        RunnerError::ToolOutput { tool_id, detail } => RunnerError::ToolOutput {
            tool_id,
            detail: format!("{name}: {detail}"),
        },
        other => other,
    }
}

/// Names the failing batch in an error when a selection ran as several spawns.
fn in_batch(err: RunnerError, index: usize, total: usize) -> RunnerError {
    if total <= 1 {
        return err;
    }
    match err {
        RunnerError::ToolExecution { tool_id, detail } => RunnerError::ToolExecution {
            tool_id,
            detail: format!("batch {index}/{total}: {detail}"),
        },
        RunnerError::ToolOutput { tool_id, detail } => RunnerError::ToolOutput {
            tool_id,
            detail: format!("batch {index}/{total}: {detail}"),
        },
        other => other,
    }
}

/// Groups staged files by their directory under the scratch root, in sorted directory order.
///
/// Each member is the file name inside its directory, its scratch-root-relative path
/// (the spelling a `--chdir` tool reports) and its absolute path. A file at the root
/// belongs to the directory `.`. Reusable by any directory-scoped linter.
pub(super) type DirGroups = std::collections::BTreeMap<String, Vec<(String, String, String)>>;

pub(super) fn group_by_parent(root: &Path, pairs: &[(String, PathBuf)]) -> DirGroups {
    let mut groups = DirGroups::new();
    for (_, absolute) in pairs {
        let rel = absolute.strip_prefix(root).unwrap_or(absolute);
        let dir = rel
            .parent()
            .map(|dir| dir.to_string_lossy().replace('\\', "/"))
            .filter(|dir| !dir.is_empty())
            .unwrap_or_else(|| ".".to_owned());
        let name = rel
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel = if dir == "." {
            name.clone()
        } else {
            format!("{dir}/{name}")
        };
        groups
            .entry(dir)
            .or_default()
            .push((name, rel, absolute.to_string_lossy().into_owned()));
    }
    for members in groups.values_mut() {
        members.sort();
    }
    groups
}

/// The package name of a CUE file: skips blank lines, `//` comments and file-level `@attr(...)`
/// lines before the clause, and drops a trailing `//` comment. Empty when there is no clause.
fn cue_package_clause(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('@'))
        .and_then(|line| line.strip_prefix("package "))
        .map(|rest| {
            let rest = rest.split("//").next().unwrap_or(rest);
            rest.trim().to_owned()
        })
        .unwrap_or_default()
}

/// The JVM finds its modules file beside the launcher it was started from, and a
/// runfiles symlink on Windows does not lead there, so the launcher is named by its real path.
pub(super) fn real_launcher(path: &Path) -> PathBuf {
    let Ok(real) = std::fs::canonicalize(path) else {
        return path.to_path_buf();
    };
    match real.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => real,
    }
}

#[cfg(test)]
mod cue_package_clause_tests {
    use super::cue_package_clause;

    #[test]
    fn clause_ignores_comments_and_attributes() {
        assert_eq!(cue_package_clause("package app\n"), "app");
        assert_eq!(cue_package_clause("package app // main\n"), "app");
        assert_eq!(
            cue_package_clause("// hi\n@if(prod)\n@extern(embed)\n\npackage app\n"),
            "app"
        );
        assert_eq!(cue_package_clause("x: 1\n"), "");
    }
}
