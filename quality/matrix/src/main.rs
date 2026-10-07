use std::process::ExitCode;

use dx_matrix_harness::run;
use dx_output::init_diagnostics;

const MANIFEST_ENV: &str = "DX_MATRIX_MANIFEST";

fn main() -> ExitCode {
    init_diagnostics(false);
    let manifest = match std::env::var(MANIFEST_ENV) {
        Ok(manifest) if !manifest.is_empty() => manifest,
        _ => {
            tracing::error!("matrix: {MANIFEST_ENV} names no manifest");
            return ExitCode::FAILURE;
        }
    };
    match run(&manifest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("matrix: {error}");
            ExitCode::FAILURE
        }
    }
}