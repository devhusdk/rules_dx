#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use dx_error::DxError;

// LCOV_EXCL_START - reason: thin man shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() -> ExitCode {
    dx_output::init_diagnostics(false);
    match run(std::env::args_os().nth(1).map(PathBuf::from)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            error.into()
        }
    }
}
// LCOV_EXCL_STOP - reason: end thin man shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

fn run(out: Option<PathBuf>) -> Result<(), DxError> {
    let out = out.ok_or_else(|| DxError::usage("usage: dx_man <output-file>"))?;
    let buffer = dx_plan::args::man_page()
        .map_err(|error| DxError::failure(format!("dx_man: render failed: {error}")))?;
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            DxError::infra(format!(
                "dx_man: cannot create {}: {error}",
                parent.display()
            ))
        })?;
    }
    let mut file = std::fs::File::create(&out).map_err(|error| {
        DxError::infra(format!("dx_man: cannot write {}: {error}", out.display()))
    })?;
    file.write_all(&buffer)
        .map_err(|error| DxError::infra(format!("dx_man: cannot write {}: {error}", out.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_error::ErrorKind;

    #[test]
    fn missing_output_is_usage() {
        let error = run(None).expect_err("usage");
        assert_eq!(error.kind(), ErrorKind::Usage);
    }

    #[test]
    fn unwritable_output_is_infra() {
        let dir = std::env::temp_dir().join(format!("dx_man_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let error = run(Some(dir.clone())).expect_err("directory is not a file");
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert_eq!(error.kind(), ErrorKind::Infra);
        assert_eq!(error.kind().exit_code(), 3);
    }
}
