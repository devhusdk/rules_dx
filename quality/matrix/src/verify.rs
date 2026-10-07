//! The checks one matrix case must satisfy after the runner wrote its result.

use std::path::{Path, PathBuf};

use quality_result::print_text;
use quality_result::proto::{Capability, Convergence, QualityResult};

use crate::manifest::Manifest;
use crate::Error;

fn capability(value: i32) -> Result<Capability, Error> {
    Capability::try_from(value)
        .ok()
        .filter(|capability| *capability != Capability::Unspecified)
        .ok_or_else(|| Error::ResultMismatch(format!("unknown capability {value}")))
}

/// Checks the decoded result against the manifest that produced it.
pub fn verify_result(manifest: &Manifest, result: &QualityResult) -> Result<(), Error> {
    if result.producer != manifest.producer {
        return Err(Error::ResultMismatch(format!(
            "producer {:?} is not the declared {:?}",
            result.producer, manifest.producer
        )));
    }
    let want = match manifest.capability.as_str() {
        "lint" => Capability::Lint,
        "format" => Capability::Format,
        "typecheck" => Capability::Typecheck,
        other => {
            return Err(Error::Malformed(format!(
                "capability {other:?} is not lint, format or typecheck"
            )))
        }
    };
    let got = capability(result.capability)?;
    if got != want {
        return Err(Error::ResultMismatch(format!(
            "capability {got:?} is not the declared {want:?}"
        )));
    }
    let declared = manifest.stages()?;
    if declared.len() != result.stages.len() {
        return Err(Error::ResultMismatch(format!(
            "{} stages in the manifest, {} in the result",
            declared.len(),
            result.stages.len()
        )));
    }
    for (index, (spec, stage)) in declared.iter().zip(result.stages.iter()).enumerate() {
        if spec.tool_id != stage.tool_id
            || spec.class_ids != stage.class_ids
            || spec.source_paths != stage.source_paths
        {
            return Err(Error::ResultMismatch(format!(
                "stage {index} is {:?}, manifest declares {spec:?}",
                stage.tool_id
            )));
        }
    }
    if result.completed_rounds == 0 {
        return Err(Error::ResultMismatch(
            "completed_rounds is zero".to_owned(),
        ));
    }
    if Convergence::try_from(result.convergence) != Ok(Convergence::Stable) {
        return Err(Error::ResultMismatch(format!(
            "convergence {} is not STABLE",
            result.convergence
        )));
    }
    Ok(())
}

fn header_count(lines: &[&str], prefix: &str) -> Result<u64, Error> {
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == prefix {
            return parts[1]
                .parse::<u64>()
                .map_err(|error| Error::PrintedMismatch(format!("{prefix} {error}")));
        }
    }
    Err(Error::PrintedMismatch(format!(
        "no {prefix} header line"
    )))
}

fn row_count(lines: &[&str], prefix: &str) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with(prefix) && line.split_whitespace().count() > 2)
        .count()
}

/// Checks that the printed result carries every count the decoded result declares.
pub fn verify_print(printed: &str, result: &QualityResult) -> Result<(), Error> {
    if !printed.ends_with('\n') {
        return Err(Error::PrintedMismatch(
            "printed result must end with a newline".to_owned(),
        ));
    }
    if printed != print_text(result) {
        return Err(Error::PrintedMismatch(
            "printed result is not the canonical rendering of the decoded result".to_owned(),
        ));
    }
    let lines: Vec<&str> = printed.lines().collect();
    let replacements: usize = result
        .replacements
        .iter()
        .map(|file| file.edits.len())
        .sum();
    for (header, row_prefix, declared) in [
        ("initial", "initial ", result.initial_diagnostics.len()),
        (
            "terminal",
            "terminal ",
            result.terminal_diagnostics.len(),
        ),
        ("replacements", "replacement ", replacements),
    ] {
        let header = header_count(&lines, header)?;
        let rows = row_count(&lines, row_prefix) as u64;
        if header != declared as u64 || rows != header {
            return Err(Error::PrintedMismatch(format!(
                "{header} header says {header} but {rows} rows and {declared} decoded entries exist"
            )));
        }
    }
    let stages = header_count(&lines, "stages")?;
    if stages != result.stages.len() as u64 || row_count(&lines, "stage ") as u64 != stages {
        return Err(Error::PrintedMismatch(format!(
            "{stages} stages declared but the printed rows disagree"
        )));
    }
    let rounds = header_count(&lines, "completed_rounds")?;
    if rounds != u64::from(result.completed_rounds) {
        return Err(Error::PrintedMismatch(format!(
            "completed_rounds says {rounds}, the result declares {}",
            result.completed_rounds
        )));
    }
    Ok(())
}

/// Renders the difference between the pinned snapshot and the fresh result.
pub fn snapshot_diff(snapshot: &str, actual: &str) -> Result<String, Error> {
    dx_diff::render_patch(&[dx_diff::FilePatch {
        path: "expected.txt",
        kind: dx_diff::PatchKind::Modify,
        original: snapshot,
        candidate: actual,
    }])
    .map_err(|error| Error::Diff(error.to_string()))
}

/// The directory one staged update is written to.
pub fn update_dir() -> PathBuf {
    std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// Writes the fresh result where a reviewer can compare it before pinning.
pub fn stage_update(dir: &Path, name: &str, actual: &[u8]) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(dir).map_err(|error| Error::Stage {
        path: dir.display().to_string(),
        detail: error.to_string(),
    })?;
    let path = dir.join(name);
    std::fs::write(&path, actual).map_err(|error| Error::Stage {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    Ok(path)
}