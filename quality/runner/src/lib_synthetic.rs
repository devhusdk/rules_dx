use quality_result::proto::Severity;

use super::*;

pub(crate) const SYNTHETIC_TOOLS: &[&str] = &["fmt-a", "lint-a", "lint-b"];

pub(crate) fn trim_trailing_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        match line.strip_suffix('\n') {
            Some(body) => {
                out.push_str(body.trim_end_matches([' ', '\t']));
                out.push('\n');
            }
            None => out.push_str(line.trim_end_matches([' ', '\t'])),
        }
    }
    out
}

pub(crate) fn apply_synthetic(tool_id: &str, text: &str) -> String {
    match tool_id {
        "lint-a" => text.replace("BAD", "GOOD"),
        "fmt-a" => trim_trailing_whitespace(text),
        "lint-b" => text.to_owned(),
        _ => text.to_owned(),
    }
}

pub(crate) fn collect_diagnostics(tool_id: &str, path: &str, text: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let (needle, severity, message) = match tool_id {
        "lint-a" => ("BAD", Severity::Warning as i32, "synthetic lint-a finding"),
        "lint-b" => ("FAIL", Severity::Error as i32, "synthetic lint-b finding"),
        _ => return out,
    };
    for (offset, _) in text.match_indices(needle) {
        out.push(Diagnostic {
            severity,
            message: message.to_owned(),
            tool_id: tool_id.to_owned(),
            path: path.to_owned(),
            start_byte: Some(offset as u64),
            end_byte: Some((offset + needle.len()) as u64),
            fixable: false,
            related: Vec::new(),
            ..Default::default()
        });
    }
    out
}

/// Lifts a per-file fixer into a stage batch fixer that runs it once per target.
pub(crate) fn per_path(
    mut apply: impl FnMut(&str, &str, &str) -> Result<String, RunnerError>,
) -> impl FnMut(usize, &StageSpec, &Texts, &[String]) -> Result<Texts, RunnerError> {
    move |_, stage, current, targets| {
        let mut out = BTreeMap::new();
        for path in targets {
            let body = current
                .get(path)
                .ok_or(RunnerError::MissingFile { path: path.clone() })?;
            out.insert(path.clone(), apply(&stage.tool_id, path, body)?);
        }
        Ok(out)
    }
}

pub(crate) fn run_pipeline(
    producer: &str,
    capability: &str,
    stages: &[StageSpec],
    files: &[FileInput],
) -> Result<QualityResult, RunnerError> {
    let (capability_value, initial) =
        validate_request(producer, capability, stages, files, |tool| {
            SYNTHETIC_TOOLS.contains(&tool)
        })?;
    let mut initial_diagnostics = Vec::new();
    for stage in stages {
        for path in &stage.source_paths {
            let body = initial
                .get(path)
                .ok_or(RunnerError::MissingFile { path: path.clone() })?;
            initial_diagnostics.extend(collect_diagnostics(&stage.tool_id, path, body));
        }
    }
    let (terminal, completed_rounds, convergence) = run_convergence(
        &initial,
        stages,
        max_rounds_for_capability(capability),
        &full_selection(stages),
        per_path(|tool, _path, text| Ok(apply_synthetic(tool, text))),
    )?;
    let mut terminal_diagnostics = Vec::new();
    if terminal == initial {
        terminal_diagnostics = initial_diagnostics.clone();
    } else {
        for stage in stages {
            for path in &stage.source_paths {
                let body = terminal
                    .get(path)
                    .ok_or(RunnerError::MissingFile { path: path.clone() })?;
                terminal_diagnostics.extend(collect_diagnostics(&stage.tool_id, path, body));
            }
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
