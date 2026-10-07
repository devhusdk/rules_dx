//! The checks one matrix case must satisfy after the runner wrote its result.

use std::path::{Path, PathBuf};

use dx_diff::{render_patch, FilePatch, PatchKind};
use quality_result::print_text;
use quality_result::proto::{Capability, Convergence, QualityResult};

use crate::manifest::Manifest;
use crate::Error;

fn capability(value: i32) -> Result<Capability, Error> {
    Capability::try_from(value)
        .ok()
        .filter(|capability| {
            matches!(
                capability,
                Capability::Lint | Capability::Format | Capability::Typecheck
            )
        })
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
        return Err(Error::ResultMismatch("completed_rounds is zero".to_owned()));
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
    Err(Error::PrintedMismatch(format!("no {prefix} header line")))
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
    let edits: usize = result
        .replacements
        .iter()
        .map(|file| file.edits.len())
        .sum();
    for (header, row_prefix, decoded) in [
        ("initial", "initial ", result.initial_diagnostics.len()),
        ("terminal", "terminal ", result.terminal_diagnostics.len()),
        ("replacements", "replacement ", edits),
    ] {
        let header = header_count(&lines, header)?;
        let rows = row_count(&lines, row_prefix) as u64;
        if header != decoded as u64 || rows != header {
            return Err(Error::PrintedMismatch(format!(
                "{header} rows in print but {decoded} entries decoded"
            )));
        }
    }
    if header_count(&lines, "replacements")? != result.replacements.len() as u64 {
        return Err(Error::PrintedMismatch(
            "replacements header is not the decoded file count".to_owned(),
        ));
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
    render_patch(&[FilePatch {
        path: "expected.txt",
        kind: PatchKind::Modify,
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

#[cfg(test)]
mod tests {
    use super::*;
    use quality_result::proto::{Diagnostic, Edit, FileEdits, Stage};

    fn manifest_bytes() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "name": "matrix_case",
            "producer": "//quality/testdata:matrix_case",
            "capability": "lint",
            "stages": ["lint-a;rust;src/lib.rs"],
            "runner": "_main/runner",
            "printer": "_main/printer",
            "expected": "_main/expected",
            "snapshot_dir": "quality/testdata/matrix",
        }))
        .unwrap()
    }

    fn manifest() -> Manifest {
        Manifest::parse(&manifest_bytes()).unwrap()
    }

    fn result() -> QualityResult {
        QualityResult {
            producer: "//quality/testdata:matrix_case".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "lint-a".to_owned(),
                class_ids: vec!["rust".to_owned()],
                source_paths: vec!["src/lib.rs".to_owned()],
                ..Default::default()
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            initial_diagnostics: vec![Diagnostic {
                tool_id: "lint-a".to_owned(),
                path: "src/lib.rs".to_owned(),
                ..Default::default()
            }],
            replacements: vec![FileEdits {
                path: "src/lib.rs".to_owned(),
                edits: vec![Edit {
                    replacement: b"x".to_vec(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn matching_result_verifies() {
        verify_result(&manifest(), &result()).unwrap();
        verify_print(&print_text(&result()), &result()).unwrap();
    }

    #[test]
    fn wrong_producer_fails() {
        let mut decoded = result();
        decoded.producer = "//other:case".to_owned();
        assert!(matches!(
            verify_result(&manifest(), &decoded),
            Err(Error::ResultMismatch(_))
        ));
    }

    #[test]
    fn wrong_capability_fails() {
        let mut decoded = result();
        decoded.capability = Capability::Format as i32;
        assert!(matches!(
            verify_result(&manifest(), &decoded),
            Err(Error::ResultMismatch(_))
        ));
    }

    #[test]
    fn changed_stage_fails() {
        let mut decoded = result();
        decoded.stages[0].tool_id = "lint-b".to_owned();
        assert!(matches!(
            verify_result(&manifest(), &decoded),
            Err(Error::ResultMismatch(_))
        ));
    }

    #[test]
    fn zero_rounds_or_unstable_convergence_fails() {
        let mut decoded = result();
        decoded.completed_rounds = 0;
        assert!(matches!(
            verify_result(&manifest(), &decoded),
            Err(Error::ResultMismatch(_))
        ));
        let mut decoded = result();
        decoded.convergence = Convergence::Oscillation as i32;
        assert!(matches!(
            verify_result(&manifest(), &decoded),
            Err(Error::ResultMismatch(_))
        ));
    }

    #[test]
    fn truncated_print_fails() {
        let decoded = result();
        let printed = print_text(&decoded);
        let short = printed.lines().take(4).collect::<Vec<_>>().join("\n") + "\n";
        assert!(matches!(
            verify_print(&short, &decoded),
            Err(Error::PrintedMismatch(_))
        ));
    }

    #[test]
    fn print_without_trailing_newline_fails() {
        let decoded = result();
        let printed = print_text(&decoded);
        let flat = printed.trim_end_matches('\n').to_owned();
        assert!(matches!(
            verify_print(&flat, &decoded),
            Err(Error::PrintedMismatch(_))
        ));
    }

    #[test]
    fn snapshot_diff_names_the_snapshot() {
        let diff = snapshot_diff("old\n", "new\n").unwrap();
        assert!(diff.contains("expected.txt"), "diff: {diff}");
        assert!(diff.contains("-old"), "diff: {diff}");
        assert!(diff.contains("+new"), "diff: {diff}");
    }

    #[test]
    fn stage_update_writes_the_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let staged = stage_update(dir.path(), "case.expected.update", b"fresh\n").unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), b"fresh\n");
    }
}
