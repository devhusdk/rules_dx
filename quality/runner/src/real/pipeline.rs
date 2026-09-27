use std::collections::BTreeMap;
use std::ffi::OsStr;

use quality_adapter::place_finding;
use quality_result::proto::Diagnostic;

use crate::{
    assemble, max_rounds_for_capability, run_convergence, stage_subset, validate_request,
    FileInput, QualityResult, RunnerError, StageSpec,
};

use super::*;

impl RealBackend {
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
