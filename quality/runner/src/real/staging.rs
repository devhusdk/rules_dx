use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use quality_adapter::exec::{MirrorContents, MirrorFile};

use super::*;

const FSHARPLINT_CONTEXT_REL: &str = "dx-fsharplint/context.json";

pub(super) struct FsharplintInputs {
    pub compile: Vec<PathBuf>,
    pub refs: Vec<PathBuf>,
    pub defines: Vec<String>,
    pub langversion: String,
}

impl RealBackend {
    pub(super) fn mirror_tool_files(
        tool_id: &str,
        tool: &RealTool,
        staged: &[MirrorFile],
    ) -> Vec<MirrorFile> {
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
        if tool_id == "ktlint" && tool.config_rel.is_none() {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(KTLINT_DEFAULTS_REL),
                contents: MirrorContents::Bytes(KTLINT_DEFAULTS_BYTES.to_vec()),
            });
        }
        if tool_id == "hadolint" && tool.config_rel.is_none() {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(HADOLINT_DEFAULTS_REL),
                contents: MirrorContents::Bytes(HADOLINT_DEFAULTS_BYTES.to_vec()),
            });
        }
        if tool_id == "stylelint" && tool.config_rel.is_none() {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(STYLELINT_DEFAULTS_REL),
                contents: MirrorContents::Bytes(STYLELINT_DEFAULTS_BYTES.to_vec()),
            });
        }
        mirrors.retain(|mirror| {
            !staged
                .iter()
                .any(|done| done.mirror_rel == mirror.mirror_rel)
        });
        mirrors
    }

    pub(super) fn stage_scratch(
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
        let tools = Self::mirror_tool_files(tool_id, tool, &mirrors);
        mirrors.extend(tools);
        write_all(&scratch, tool_id, &mirrors)?;
        Ok(StagedScratch {
            scratch,
            pairs,
            sibling_pairs,
            resolve_pairs,
        })
    }

    pub(super) fn config_abs(
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
        if tool_id == "ktlint" {
            let rel = tool.config_rel.as_deref().unwrap_or(KTLINT_DEFAULTS_REL);
            return scratch
                .resolve(Path::new(rel))
                .map(Some)
                .map_err(|err| execution(tool_id, format!("scratch config: {err}")));
        }
        if tool_id == "hadolint" {
            let rel = tool.config_rel.as_deref().unwrap_or(HADOLINT_DEFAULTS_REL);
            return scratch
                .resolve(Path::new(rel))
                .map(Some)
                .map_err(|err| execution(tool_id, format!("scratch config: {err}")));
        }
        if tool_id == "stylelint" {
            let rel = tool.config_rel.as_deref().unwrap_or(STYLELINT_DEFAULTS_REL);
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

    pub(super) fn scalafix_classpath(tool: &RealTool, scratch: &Scratch) -> Vec<PathBuf> {
        ["dx-scalafix/owned/", "dx-scalafix/aux/"]
            .iter()
            .flat_map(|prefix| {
                tool.tool_files
                    .iter()
                    .filter(move |(rel, _)| rel.starts_with(prefix) && rel.ends_with(".jar"))
                    .map(|(rel, _)| scratch.root().join(rel))
            })
            .collect()
    }

    pub(super) fn fsharplint_context(
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
    ) -> Result<FsharplintInputs, RunnerError> {
        let bytes = tool
            .tool_files
            .iter()
            .find(|(rel, _)| rel == FSHARPLINT_CONTEXT_REL)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| {
                execution(
                    tool_id,
                    "fsharplint needs the target compile context; the F# aspect did not supply it"
                        .to_owned(),
                )
            })?;
        let doc: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|err| execution(tool_id, format!("fsharplint context: {err}")))?;
        let strings = |key: &str| -> Result<Vec<String>, RunnerError> {
            doc.get(key)
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| execution(tool_id, format!("fsharplint context: missing {key}")))?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        execution(tool_id, format!("fsharplint context: bad {key} entry"))
                    })
                })
                .collect()
        };
        let mut compile = Vec::new();
        for source in strings("sources")? {
            let Some((_, absolute)) = pairs.iter().find(|(workspace, _)| *workspace == source)
            else {
                return Err(execution(
                    tool_id,
                    format!("fsharplint context: compile source {source} was not staged"),
                ));
            };
            compile.push(absolute.clone());
        }
        let refs = strings("refs")?
            .iter()
            .map(|rel| scratch.root().join(rel))
            .collect();
        Ok(FsharplintInputs {
            compile,
            refs,
            defines: strings("defines")?,
            langversion: doc
                .get("langversion")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    pub(super) fn rustfmt_edition(tool: &RealTool) -> Result<&str, RunnerError> {
        const TOOL_ID: &str = "rustfmt";
        tool.edition.as_deref().ok_or_else(|| {
            execution(
                TOOL_ID,
                "missing tool edition: the quality aspect must pass the CrateInfo edition as the request tool edition".to_owned(),
            )
        })
    }

    pub(super) fn gofumpt_lang(tool: &RealTool) -> Result<&str, RunnerError> {
        const TOOL_ID: &str = "gofumpt";
        tool.edition.as_deref().ok_or_else(|| {
            execution(
                TOOL_ID,
                "missing tool edition: the quality aspect must pass the Go language version via --tool-edition".to_owned(),
            )
        })
    }

    pub(super) fn biome_config_dir(
        tool: &RealTool,
        scratch: &Scratch,
    ) -> Result<PathBuf, RunnerError> {
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

    pub(super) fn cwd_rel(tool_id: &str, config_rel: Option<&str>) -> String {
        match tool_id {
            "buildifier" | "vale" | "staticcheck" => config_rel.map(parent_rel).unwrap_or_default(),
            _ => String::new(),
        }
    }
}

pub(super) fn qml_import_roots(
    tool_id: &str,
    context: &[StagedPair],
) -> Result<Vec<PathBuf>, RunnerError> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for (workspace, absolute) in context {
        if absolute.file_name().and_then(|name| name.to_str()) != Some("qmldir") {
            continue;
        }
        let text = std::fs::read_to_string(absolute)
            .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
        let Some(uri) = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("module "))
            .map(str::trim)
            .next()
        else {
            continue;
        };
        let mut root = absolute.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut mirrors = true;
        for segment in uri.rsplit('.') {
            if root.file_name().and_then(|name| name.to_str()) != Some(segment) {
                mirrors = false;
                break;
            }
            root.pop();
        }
        if !mirrors {
            continue;
        }
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    Ok(roots)
}

pub(super) fn place_qmllint_settings(
    tool_id: &str,
    config: &Path,
    sources: &[StagedPair],
) -> Result<(), RunnerError> {
    let mut placed: Vec<PathBuf> = Vec::new();
    for (workspace, absolute) in sources {
        let Some(dir) = absolute.parent() else {
            continue;
        };
        let target = dir.join(".qmllint.ini");
        if target == config || placed.contains(&target) {
            continue;
        }
        dx_atomic_fs::copy_atomic(config, &target)
            .map_err(|err| execution(tool_id, format!("place settings for {workspace}: {err}")))?;
        placed.push(target);
    }
    Ok(())
}
