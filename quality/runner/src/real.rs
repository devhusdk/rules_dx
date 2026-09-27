use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use quality_adapter::commands::{self, Invocation};
use quality_adapter::exec::{self, ChildOutput, MirrorContents, MirrorFile, Scratch};
use quality_adapter::parsers::{self, FileFinding, ParseError};
use quality_adapter::place_finding;

use crate::{
    assemble, max_rounds_for_capability, run_convergence, stage_subset, validate_request,
    FileInput, QualityResult, RunnerError, StageSpec,
};
use quality_result::proto::Diagnostic;

pub const REAL_TOOLS: &[&str] = &[
    "biome",
    "buf",
    "buildifier",
    "checkstyle",
    "clang_format",
    "clang_tidy",
    "clippy",
    "cppcheck",
    "csharpier",
    "cue",
    "djlint",
    "errcheck",
    "eslint",
    "fantomas",
    "flake8",
    "fsharplint",
    "gofumpt",
    "google_java_format",
    "govet",
    "jsonnetfmt",
    "keep_sorted",
    "ktfmt",
    "ktlint",
    "markdown_check",
    "modfmt",
    "pkl",
    "pmd",
    "prettier",
    "psscriptanalyzer",
    "pydoclint",
    "pylint",
    "qmlformat",
    "qmllint",
    "roslyn",
    "rubocop",
    "ruff",
    "rustc",
    "rustfmt",
    "scalafix",
    "scalafmt",
    "shellcheck",
    "shfmt",
    "spotbugs",
    "standardrb",
    "staticcheck",
    "stylelint",
    "taplo",
    "terraform",
    "ty",
    "vale",
    "yamlfmt",
    "yamllint",
];

const RUSTFMT_DEFAULTS_REL: &str = "dx-rustfmt-default.toml";

const BIOME_DEFAULTS_REL: &str = "dx-biome-default/biome.json";
const BIOME_DEFAULTS_BYTES: &[u8] = b"{}";

pub struct RealTool {
    pub binary: PathBuf,
    pub extra_env: Vec<(String, String)>,
    pub config_rel: Option<String>,
    pub edition: Option<String>,
    pub tool_files: Vec<(String, Vec<u8>)>,
    pub upstream_diagnostics: Vec<PathBuf>,
}

pub type SpawnFn = fn(&[OsString], &Path, &[(String, String)]) -> io::Result<ChildOutput>;

type StagedPair = (String, PathBuf);

struct StagedScratch {
    scratch: Scratch,
    pairs: Vec<StagedPair>,
    sibling_pairs: Vec<StagedPair>,
    resolve_pairs: Vec<StagedPair>,
}

pub struct RealBackend {
    tools: BTreeMap<String, RealTool>,
    scratch_parent: PathBuf,
    spawn: SpawnFn,
}

pub fn real_spawn(
    argv: &[OsString],
    cwd: &Path,
    env: &[(String, String)],
) -> io::Result<ChildOutput> {
    exec::spawn(argv, cwd, env)
}

fn execution(tool_id: &str, detail: String) -> RunnerError {
    RunnerError::ToolExecution {
        tool_id: tool_id.to_owned(),
        detail,
    }
}

fn parsed<T>(tool_id: &str, result: Result<T, ParseError>) -> Result<T, RunnerError> {
    result.map_err(|err| RunnerError::ToolOutput {
        tool_id: tool_id.to_owned(),
        detail: err.to_string(),
    })
}

fn reanchor(
    tool_id: &str,
    pairs: &[(String, PathBuf)],
    workspace: &str,
) -> Result<PathBuf, RunnerError> {
    pairs
        .iter()
        .find(|(known, _)| known == workspace)
        .map(|(_, absolute)| absolute.clone())
        .ok_or_else(|| RunnerError::UnplaceableFinding {
            tool_id: tool_id.to_owned(),
            detail: format!("finding names unstaged file: {workspace}"),
        })
}

fn fresh_scratch(parent: &Path, tool_id: &str) -> Result<Scratch, RunnerError> {
    Scratch::create(parent).map_err(|err| execution(tool_id, format!("scratch: {err}")))
}

fn write_all(scratch: &Scratch, tool_id: &str, mirrors: &[MirrorFile]) -> Result<(), RunnerError> {
    scratch
        .materialize(mirrors)
        .map_err(|err| execution(tool_id, format!("materialize: {err}")))
}

fn cleaned<T>(tool_id: &str, scratch: Scratch, value: T) -> Result<T, RunnerError> {
    scratch
        .close()
        .map_err(|err| execution(tool_id, format!("scratch cleanup: {err}")))?;
    Ok(value)
}

fn hint_dir<'a>(config_rel: Option<&'a str>, cwd_rel: &'a str) -> Option<&'a str> {
    config_rel.map(|_| cwd_rel)
}

fn parent_rel(rel: &str) -> String {
    Path::new(rel)
        .parent()
        .and_then(|parent| parent.to_str())
        .unwrap_or_default()
        .to_owned()
}

impl RealBackend {
    pub fn new(tools: BTreeMap<String, RealTool>, scratch_parent: PathBuf) -> Self {
        Self {
            tools,
            scratch_parent,
            spawn: real_spawn,
        }
    }

    pub fn supports(&self, tool_id: &str) -> bool {
        self.tools.contains_key(tool_id)
    }

    fn tool(&self, tool_id: &str) -> Result<&RealTool, RunnerError> {
        self.tools
            .get(tool_id)
            .ok_or_else(|| RunnerError::UnknownTool {
                tool_id: tool_id.to_owned(),
            })
    }

    fn run(
        &self,
        tool_id: &str,
        tool: &RealTool,
        invocation: &Invocation,
        scratch: &Scratch,
    ) -> Result<ChildOutput, RunnerError> {
        let cwd = scratch.root().join(&invocation.cwd_rel);
        let extra: Vec<(&str, &str)> = tool
            .extra_env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let env = exec::hermetic_env(scratch.root(), &extra);
        (self.spawn)(&invocation.argv, &cwd, &env)
            .map_err(|err| execution(tool_id, format!("spawn: {err}")))
    }

    fn mirror_tool_files(tool_id: &str, tool: &RealTool) -> Vec<MirrorFile> {
        let mut mirrors = Vec::with_capacity(tool.tool_files.len() + 1);
        for (rel, bytes) in &tool.tool_files {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(rel),
                contents: MirrorContents::Bytes(bytes.clone()),
            });
        }
        if tool_id == "rustfmt" && tool.config_rel.is_none() {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(RUSTFMT_DEFAULTS_REL),
                contents: MirrorContents::Bytes(Vec::new()),
            });
        }
        if tool_id == "biome" && tool.config_rel.is_none() {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(BIOME_DEFAULTS_REL),
                contents: MirrorContents::Bytes(BIOME_DEFAULTS_BYTES.to_vec()),
            });
        }
        mirrors
    }

    fn stage_scratch(
        &self,
        tool_id: &str,
        tool: &RealTool,
        files: &BTreeMap<String, String>,
        siblings: &BTreeMap<String, String>,
        resolve: &BTreeMap<String, String>,
    ) -> Result<StagedScratch, RunnerError> {
        let scratch = fresh_scratch(&self.scratch_parent, tool_id)?;
        let mut mirrors = Vec::with_capacity(
            files.len() + siblings.len() + resolve.len() + tool.tool_files.len() + 1,
        );
        let mut pairs = Vec::with_capacity(files.len());
        for (path, text) in files {
            let absolute = scratch
                .resolve(Path::new(path))
                .map_err(|err| execution(tool_id, format!("scratch file: {err}")))?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
            pairs.push((path.clone(), absolute));
        }
        let mut sibling_pairs = Vec::with_capacity(siblings.len());
        for (path, text) in siblings {
            let absolute = scratch
                .resolve(Path::new(path))
                .map_err(|err| execution(tool_id, format!("scratch sibling: {err}")))?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
            sibling_pairs.push((path.clone(), absolute));
        }
        let mut resolve_pairs = Vec::with_capacity(resolve.len());
        for (path, text) in resolve {
            let absolute = scratch
                .resolve(Path::new(path))
                .map_err(|err| execution(tool_id, format!("scratch resolve: {err}")))?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
            resolve_pairs.push((path.clone(), absolute));
        }
        mirrors.extend(Self::mirror_tool_files(tool_id, tool));
        write_all(&scratch, tool_id, &mirrors)?;
        Ok(StagedScratch {
            scratch,
            pairs,
            sibling_pairs,
            resolve_pairs,
        })
    }

    fn config_abs(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
    ) -> Result<Option<PathBuf>, RunnerError> {
        if tool_id == "rustfmt" {
            let rel = tool.config_rel.as_deref().unwrap_or(RUSTFMT_DEFAULTS_REL);
            return scratch
                .resolve(Path::new(rel))
                .map(Some)
                .map_err(|err| execution(tool_id, format!("scratch config: {err}")));
        }
        tool.config_rel
            .as_deref()
            .map(|rel| {
                scratch
                    .resolve(Path::new(rel))
                    .map_err(|err| execution(tool_id, format!("scratch config: {err}")))
            })
            .transpose()
    }

    fn rustfmt_edition(tool: &RealTool) -> Result<&str, RunnerError> {
        const TOOL_ID: &str = "rustfmt";
        tool.edition.as_deref().ok_or_else(|| {
            execution(
                TOOL_ID,
                "missing tool edition: the quality aspect must pass the CrateInfo edition via --tool-edition".to_owned(),
            )
        })
    }

    fn biome_config_dir(tool: &RealTool, scratch: &Scratch) -> Result<PathBuf, RunnerError> {
        const TOOL_ID: &str = "biome";
        let rel = tool.config_rel.as_deref().unwrap_or(BIOME_DEFAULTS_REL);
        let dir_rel = Path::new(rel)
            .parent()
            .and_then(|parent| parent.to_str())
            .unwrap_or_default();
        let dir = if dir_rel.is_empty() {
            scratch.root().to_path_buf()
        } else {
            scratch
                .resolve(Path::new(dir_rel))
                .map_err(|err| execution(TOOL_ID, format!("scratch config: {err}")))?
        };
        Ok(dir)
    }

    fn cwd_rel(tool_id: &str, config_rel: Option<&str>) -> String {
        match tool_id {
            "buildifier" | "vale" | "staticcheck" => config_rel.map(parent_rel).unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn check_clippy_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_clippy(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_rustc_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_rustc(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_roslyn_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(tool_id, parsers::parse_roslyn(&bytes, &workspaces))?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_scalafix_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_scalafix(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_fsharplint_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_fsharplint(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_buf_lint_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let code = if bytes.iter().all(|b| b.is_ascii_whitespace()) {
                Some(0)
            } else {
                Some(1)
            };
            findings.extend(parsed(
                tool_id,
                parsers::parse_buf_lint(&bytes, code, &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_qmllint_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let text = String::from_utf8_lossy(&bytes);
            let code = if text.contains("\"diagnostics\": []") || text.trim().is_empty() {
                Some(0)
            } else {
                Some(1)
            };
            findings.extend(parsed(
                tool_id,
                parsers::parse_qmllint(&bytes, code, &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_clang_tidy_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_clang_tidy(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_cppcheck_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_cppcheck(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_staticcheck_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_staticcheck(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_govet_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_govet(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_errcheck_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_errcheck(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn check_file_family_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let report = match tool_id {
                "stylelint" => parsers::parse_stylelint(&bytes, Some(0), &workspaces),
                "rubocop" => parsers::parse_rubocop(&bytes, Some(0), &workspaces),
                "psscriptanalyzer" => parsers::parse_psscriptanalyzer(&bytes, Some(0), &workspaces),
                "yamllint" => parsers::parse_yamllint(&bytes, Some(0), &workspaces),
                "shellcheck" => parsers::parse_shellcheck(&bytes, Some(0), &workspaces),
                "keep_sorted" => parsers::parse_keep_sorted(&bytes, Some(0), &workspaces),
                "djlint" => parsers::parse_djlint(&bytes, Some(0), &workspaces),
                _ => {
                    return Err(execution(
                        tool_id,
                        format!("unsupported file-family delegated tool: {tool_id}"),
                    ))
                }
            };
            findings.extend(parsed(tool_id, report)?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    fn run_check(
        &self,
        tool_id: &str,
        tool: &RealTool,
        capability: &str,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let pairs = &staged.pairs;
        let sibling_pairs = &staged.sibling_pairs;
        let resolve_pairs = &staged.resolve_pairs;
        let scratch = &staged.scratch;
        let refs: Vec<&Path> = pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let names: Vec<String> = pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        let config = self.config_abs(tool_id, tool, scratch)?;
        let cwd_rel = Self::cwd_rel(tool_id, tool.config_rel.as_deref());
        match tool_id {
            "buildifier" => {
                let invocation = commands::buildifier_check(
                    &tool.binary,
                    &refs,
                    hint_dir(tool.config_rel.as_deref(), &cwd_rel),
                );
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let reported = parsed(
                    tool_id,
                    parsers::parse_buildifier(&out.stdout, &out.stderr, &strs),
                )?;
                let mut kept = Vec::with_capacity(reported.len());
                for found in reported {
                    if (capability == "format") == found.finding.rule_id.is_empty() {
                        kept.push(found);
                    }
                }
                Ok(kept)
            }
            "clippy" => self.check_clippy_delegated(tool_id, tool, pairs),
            "rustc" => self.check_rustc_delegated(tool_id, tool, pairs),
            "markdown_check" => {
                let specs: Vec<(&str, &Path)> = pairs
                    .iter()
                    .map(|(workspace, absolute)| (workspace.as_str(), absolute.as_path()))
                    .collect();
                let sibling_specs: Vec<(&str, &Path)> = sibling_pairs
                    .iter()
                    .map(|(workspace, absolute)| (workspace.as_str(), absolute.as_path()))
                    .collect();
                let invocation = commands::markdown_check(&tool.binary, &specs, &sibling_specs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let reported = parsed(
                    tool_id,
                    parsers::parse_markdown_findings(&out.stdout, out.code, &workspaces),
                )?;
                let mut rerooted = Vec::with_capacity(reported.len());
                for found in reported {
                    rerooted.push(FileFinding {
                        file: reanchor(tool_id, pairs, &found.file)?
                            .to_string_lossy()
                            .into_owned(),
                        finding: found.finding,
                    });
                }
                Ok(rerooted)
            }
            "rustfmt" => {
                let Some(cfg) = config.as_ref() else {
                    return Err(execution(tool_id, "rustfmt requires a config".to_owned()));
                };
                let invocation =
                    commands::rustfmt(&tool.binary, &refs, cfg, Self::rustfmt_edition(tool)?, true);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_rustfmt(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "ruff" => {
                let config_ref = config.as_deref();
                let invocation = if capability == "format" {
                    commands::ruff_format_check(&tool.binary, &refs, config_ref)
                } else {
                    commands::ruff_check(&tool.binary, &refs, config_ref)
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                if capability == "format" {
                    parsed(
                        tool_id,
                        parsers::parse_ruff_format(&out.stdout, out.code, &strs),
                    )
                } else {
                    parsed(tool_id, parsers::parse_ruff(&out.stdout, out.code, &strs))
                }
            }
            "ty" => {
                let mut dirs: Vec<PathBuf> = Vec::new();
                for (_, absolute) in pairs.iter().chain(resolve_pairs.iter()) {
                    if let Some(parent) = absolute.parent() {
                        if !dirs.contains(&parent.to_path_buf()) {
                            dirs.push(parent.to_path_buf());
                        }
                    }
                }
                let dir_refs: Vec<&Path> = dirs.iter().map(|dir| dir.as_path()).collect();
                let invocation = commands::ty_check(&tool.binary, &refs, &dir_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .chain(resolve_pairs.iter())
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_ty(&out.stdout, out.code, &workspaces),
                )?;
                let checked: std::collections::BTreeSet<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                findings.retain(|found| checked.contains(found.file.as_str()));
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "pydoclint" => {
                let invocation = commands::pydoclint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_pydoclint(&out.stderr, out.code, &strs),
                )
            }
            "flake8" => {
                let invocation = commands::flake8_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_flake8(&out.stdout, out.code, &strs))
            }
            "pylint" => {
                let invocation = commands::pylint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_pylint(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "taplo" => {
                let invocation = if capability == "format" {
                    commands::taplo_format(&tool.binary, &refs, config.as_deref(), true)
                } else {
                    commands::taplo_lint(&tool.binary, &refs, config.as_deref())
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                if capability == "format" {
                    parsed(
                        tool_id,
                        parsers::parse_taplo_format_check(&out.stderr, out.code, &strs),
                    )
                } else {
                    parsed(
                        tool_id,
                        parsers::parse_taplo_lint(&out.stderr, out.code, &strs),
                    )
                }
            }
            "vale" => {
                let invocation = match config.as_ref() {
                    Some(ini) => commands::vale_check(&tool.binary, ini, &refs, &cwd_rel),
                    None => return Err(execution(tool_id, "vale requires a config".to_owned())),
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_vale(&out.stdout, out.code, &strs))
            }
            "biome" => {
                let config_dir = Self::biome_config_dir(tool, scratch)?;
                let invocation = if capability == "format" {
                    commands::biome_format_check(&tool.binary, &refs, &config_dir)
                } else {
                    commands::biome_lint_check(&tool.binary, &refs, &config_dir)
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let report = if capability == "format" {
                    parsers::parse_biome_format(&out.stdout, out.code, &workspaces)
                } else {
                    parsers::parse_biome_lint(&out.stdout, out.code, &workspaces)
                };
                let mut findings = parsed(tool_id, report)?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "eslint" => {
                let invocation = match config.as_ref() {
                    Some(cfg) => commands::eslint_check(&tool.binary, &refs, cfg),
                    None => {
                        return Err(execution(tool_id, "eslint requires a config".to_owned()));
                    }
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_eslint(&out.stdout, out.code, &strs))
            }
            "prettier" => {
                let invocation = commands::prettier_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let report = parsers::parse_prettier_check(&out.stderr, out.code, &workspaces);
                let mut findings = parsed(tool_id, report)?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "scalafmt" => {
                let invocation = commands::scalafmt_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_scalafmt(&out.stdout, out.code, &strs),
                )
            }
            "scalafix" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_scalafix_delegated(tool_id, tool, pairs)
                } else {
                    let invocation =
                        commands::scalafix_check(&tool.binary, &refs, None, None, None);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_scalafix(&out.stdout, out.code, &strs),
                    )
                }
            }
            "csharpier" => {
                let invocation = commands::csharpier_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_csharpier(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "fantomas" => {
                let invocation = commands::fantomas_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_fantomas(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "roslyn" => self.check_roslyn_delegated(tool_id, tool, pairs),
            "fsharplint" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_fsharplint_delegated(tool_id, tool, pairs)
                } else {
                    let invocation =
                        commands::fsharplint_check(&tool.binary, &refs, None, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_fsharplint(&out.stdout, out.code, &strs),
                    )
                }
            }
            "google_java_format" => {
                let invocation = commands::google_java_format_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_google_java_format(&out.stdout, out.code, &strs),
                )
            }
            "ktfmt" => {
                let invocation = commands::ktfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktfmt(&out.stdout, out.code, &strs))
            }
            "checkstyle" => {
                let invocation = match config.as_ref() {
                    Some(cfg) => commands::checkstyle_check(&tool.binary, &refs, cfg),
                    None => {
                        return Err(execution(
                            tool_id,
                            "checkstyle requires a config".to_owned(),
                        ));
                    }
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_checkstyle(&out.stdout, out.code, &strs),
                )
            }
            "pmd" => {
                let invocation = commands::pmd_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_pmd(&out.stdout, out.code, &strs))
            }
            "spotbugs" => {
                let jar_targets: Vec<PathBuf> = tool
                    .tool_files
                    .iter()
                    .filter(|(rel, _)| rel.ends_with(".jar"))
                    .map(|(rel, _)| scratch.root().join(rel))
                    .collect();
                let targets: Vec<&Path> = jar_targets.iter().map(PathBuf::as_path).collect();
                let invocation = commands::spotbugs_check(&tool.binary, &targets);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_spotbugs(&out.stdout, out.code, &strs),
                )
            }
            "ktlint" => {
                let invocation = commands::ktlint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktlint(&out.stdout, out.code, &strs))
            }
            "clang_format" => {
                let invocation =
                    commands::clang_format_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_clang_format(&out.stdout, out.code, &strs),
                )
            }
            "gofumpt" => {
                let invocation = commands::gofumpt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_gofumpt(&out.stdout, out.code, &strs),
                )
            }
            "clang_tidy" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_clang_tidy_delegated(tool_id, tool, pairs)
                } else {
                    let invocation =
                        commands::clang_tidy_check(&tool.binary, &refs, config.as_deref(), None);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_clang_tidy(&out.stderr, out.code, &strs),
                    )
                }
            }
            "cppcheck" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_cppcheck_delegated(tool_id, tool, pairs)
                } else {
                    let invocation =
                        commands::cppcheck_check(&tool.binary, &refs, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_cppcheck(&out.stderr, out.code, &strs),
                    )
                }
            }
            "staticcheck" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_staticcheck_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::staticcheck_check(
                        &tool.binary,
                        &refs,
                        hint_dir(tool.config_rel.as_deref(), &cwd_rel),
                    );
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_staticcheck(&out.stdout, out.code, &strs),
                    )
                }
            }
            "govet" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_govet_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::govet_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(tool_id, parsers::parse_govet(&out.stderr, out.code, &strs))
                }
            }
            "errcheck" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_errcheck_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::errcheck_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_errcheck(&out.stdout, out.code, &strs),
                    )
                }
            }
            "buf" => {
                if capability == "format" {
                    let invocation = commands::buf_format_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_buf_format(&out.stdout, out.code, &strs),
                    )
                } else if !tool.upstream_diagnostics.is_empty() {
                    self.check_buf_lint_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::buf_lint_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_buf_lint(&out.stdout, out.code, &strs),
                    )
                }
            }
            "qmlformat" => {
                let invocation = commands::qmlformat_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_qmlformat(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "qmllint" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_qmllint_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::qmllint_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let workspaces: Vec<&str> = pairs
                        .iter()
                        .map(|(workspace, _)| workspace.as_str())
                        .collect();
                    let mut findings = parsed(
                        tool_id,
                        parsers::parse_qmllint(&out.stdout, out.code, &workspaces),
                    )?;
                    for found in &mut findings {
                        let absolute = reanchor(tool_id, pairs, &found.file)?;
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                    Ok(findings)
                }
            }
            "cue" => {
                let invocation = commands::cue_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_cue(&out.stdout, out.code, &strs))
            }
            "jsonnetfmt" => {
                let invocation = commands::jsonnetfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_jsonnetfmt(&out.stdout, out.code, &strs),
                )
            }
            "pkl" => {
                let invocation = commands::pkl_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_pkl(&out.stdout, out.code, &strs))
            }
            "modfmt" => {
                let invocation = commands::modfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_modfmt(&out.stdout, out.code, &strs))
            }
            "terraform" => {
                let invocation = commands::terraform_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_terraform(&out.stdout, out.code, &strs),
                )
            }
            "yamlfmt" => {
                let invocation = commands::yamlfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_yamlfmt(&out.stdout, out.code, &strs),
                )
            }
            "shfmt" => {
                let invocation = commands::shfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_shfmt(&out.stdout, out.code, &strs))
            }
            "standardrb" => {
                let invocation = commands::standardrb_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_standardrb(&out.stdout, out.code, &strs),
                )
            }
            "djlint" => {
                if capability == "format" {
                    let invocation = commands::djlint_format_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_djlint_format(&out.stdout, out.code, &strs),
                    )
                } else if !tool.upstream_diagnostics.is_empty() {
                    self.check_file_family_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::djlint_check(&tool.binary, &refs, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(tool_id, parsers::parse_djlint(&out.stdout, out.code, &strs))
                }
            }
            "stylelint" | "rubocop" | "psscriptanalyzer" | "yamllint" | "shellcheck"
            | "keep_sorted" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_file_family_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = match tool_id {
                        "stylelint" => {
                            commands::stylelint_check(&tool.binary, &refs, config.as_deref())
                        }
                        "rubocop" => commands::rubocop_check(&tool.binary, &refs),
                        "psscriptanalyzer" => commands::psscriptanalyzer_check(&tool.binary, &refs),
                        "yamllint" => {
                            commands::yamllint_check(&tool.binary, &refs, config.as_deref())
                        }
                        "shellcheck" => commands::shellcheck_check(&tool.binary, &refs),
                        _ => commands::keep_sorted_check(&tool.binary, &refs),
                    };
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let report = match tool_id {
                        "stylelint" => parsers::parse_stylelint(&out.stdout, out.code, &strs),
                        "rubocop" => parsers::parse_rubocop(&out.stdout, out.code, &strs),
                        "psscriptanalyzer" => {
                            parsers::parse_psscriptanalyzer(&out.stdout, out.code, &strs)
                        }
                        "yamllint" => parsers::parse_yamllint(&out.stdout, out.code, &strs),
                        "shellcheck" => parsers::parse_shellcheck(&out.stdout, out.code, &strs),
                        _ => parsers::parse_keep_sorted(&out.stdout, out.code, &strs),
                    };
                    parsed(tool_id, report)
                }
            }
            _ => Err(execution(
                tool_id,
                format!("unsupported real tool: {tool_id}"),
            )),
        }
    }

    pub fn diagnose(
        &self,
        tool_id: &str,
        capability: &str,
        files: &BTreeMap<String, String>,
    ) -> Result<Vec<Diagnostic>, RunnerError> {
        self.diagnose_with_siblings(tool_id, capability, files, &BTreeMap::new())
    }

    pub fn diagnose_with_siblings(
        &self,
        tool_id: &str,
        capability: &str,
        files: &BTreeMap<String, String>,
        siblings: &BTreeMap<String, String>,
    ) -> Result<Vec<Diagnostic>, RunnerError> {
        self.diagnose_with_resolve(tool_id, capability, files, siblings, &BTreeMap::new())
    }

    pub fn diagnose_with_resolve(
        &self,
        tool_id: &str,
        capability: &str,
        files: &BTreeMap<String, String>,
        siblings: &BTreeMap<String, String>,
        resolve: &BTreeMap<String, String>,
    ) -> Result<Vec<Diagnostic>, RunnerError> {
        let tool = self.tool(tool_id)?;
        let staged = self.stage_scratch(tool_id, tool, files, siblings, resolve)?;
        let collected = self.run_check(tool_id, tool, capability, &staged)?;
        let StagedScratch {
            scratch,
            pairs,
            sibling_pairs: _,
            resolve_pairs: _,
        } = staged;
        let mut diagnostics = Vec::with_capacity(collected.len());
        for found in &collected {
            let workspace = pairs
                .iter()
                .find(|(_, absolute)| absolute.as_os_str() == OsStr::new(&found.file))
                .map(|pair| pair.0.clone())
                .ok_or_else(|| RunnerError::UnplaceableFinding {
                    tool_id: tool_id.to_owned(),
                    detail: format!("finding names unstaged file: {}", found.file),
                })?;
            let text = files.get(&workspace).ok_or(RunnerError::MissingFile {
                path: workspace.clone(),
            })?;
            diagnostics.push(
                place_finding(&found.finding, &workspace, text).map_err(|err| {
                    RunnerError::UnplaceableFinding {
                        tool_id: tool_id.to_owned(),
                        detail: err.to_string(),
                    }
                })?,
            );
        }
        cleaned(tool_id, scratch, diagnostics)
    }
}

pub fn run_real_pipeline(
    producer: &str,
    capability: &str,
    stages: &[StageSpec],
    files: &[FileInput],
    backend: &RealBackend,
) -> Result<QualityResult, RunnerError> {
    run_real_pipeline_with_siblings(producer, capability, stages, files, &[], backend)
}

pub fn run_real_pipeline_with_siblings(
    producer: &str,
    capability: &str,
    stages: &[StageSpec],
    files: &[FileInput],
    siblings: &[FileInput],
    backend: &RealBackend,
) -> Result<QualityResult, RunnerError> {
    run_real_pipeline_with_resolve(producer, capability, stages, files, siblings, &[], backend)
}

pub fn run_real_pipeline_with_resolve(
    producer: &str,
    capability: &str,
    stages: &[StageSpec],
    files: &[FileInput],
    siblings: &[FileInput],
    resolve: &[FileInput],
    backend: &RealBackend,
) -> Result<QualityResult, RunnerError> {
    let (capability_value, initial) =
        validate_request(producer, capability, stages, files, |tool| {
            backend.supports(tool)
        })?;
    let mut sibling_texts = BTreeMap::new();
    for sibling in siblings {
        if initial.contains_key(&sibling.path) || sibling_texts.contains_key(&sibling.path) {
            return Err(RunnerError::DuplicateFile {
                path: sibling.path.clone(),
            });
        }
        let text = std::str::from_utf8(&sibling.bytes).map_err(|_| RunnerError::InvalidUtf8 {
            path: sibling.path.clone(),
        })?;
        sibling_texts.insert(sibling.path.clone(), text.to_owned());
    }
    let mut resolve_texts = BTreeMap::new();
    for item in resolve {
        if initial.contains_key(&item.path)
            || sibling_texts.contains_key(&item.path)
            || resolve_texts.contains_key(&item.path)
        {
            return Err(RunnerError::DuplicateFile {
                path: item.path.clone(),
            });
        }
        let text = std::str::from_utf8(&item.bytes).map_err(|_| RunnerError::InvalidUtf8 {
            path: item.path.clone(),
        })?;
        resolve_texts.insert(item.path.clone(), text.to_owned());
    }
    let mut initial_diagnostics = Vec::new();
    for stage in stages {
        let subset = stage_subset(stage, &initial)?;
        initial_diagnostics.extend(backend.diagnose_with_resolve(
            &stage.tool_id,
            capability,
            &subset,
            &sibling_texts,
            &resolve_texts,
        )?);
    }
    let (terminal, completed_rounds, convergence) = run_convergence(
        &initial,
        stages,
        max_rounds_for_capability(capability),
        |tool, path, text| backend.apply_fix(tool, path, text, capability),
    )?;
    let mut terminal_diagnostics = Vec::new();
    if terminal == initial {
        terminal_diagnostics = initial_diagnostics.clone();
    } else {
        for stage in stages {
            let subset = stage_subset(stage, &terminal)?;
            terminal_diagnostics.extend(backend.diagnose_with_resolve(
                &stage.tool_id,
                capability,
                &subset,
                &sibling_texts,
                &resolve_texts,
            )?);
        }
    }
    assemble(
        producer,
        capability_value,
        stages,
        &initial,
        &terminal,
        (initial_diagnostics, terminal_diagnostics),
        (completed_rounds, convergence),
    )
}

#[path = "real_fix.rs"]
mod real_fix;

#[cfg(test)]
#[path = "real_core.rs"]
mod real_core;
#[cfg(test)]
#[path = "real_delegated.rs"]
mod real_delegated;
#[cfg(test)]
#[path = "real_fixtures.rs"]
mod real_fixtures;
#[cfg(test)]
#[path = "real_tools.rs"]
mod real_tools;
