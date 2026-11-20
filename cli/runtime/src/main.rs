//! `dx-runtime`, the runtime helpers rules_dx rules run, as one binary.
//!
//! Each rule names its helper in `DX_RUNTIME_TOOL` through `RunEnvironmentInfo`.

use std::ffi::OsStr;
use std::process::ExitCode;

/// The environment variable that names the helper to run.
const TOOL_ENV: &str = "DX_RUNTIME_TOOL";

/// The entry point of one helper.
type Helper = fn() -> ExitCode;

/// The helpers, by the name a rule sets in `DX_RUNTIME_TOOL`.
/// `apple_ops` is left out on Windows, where it does not build.
const TOOLS: &[(&str, Helper)] = &[
    ("android_ops", dx_android_ops::main),
    #[cfg(not(windows))]
    ("apple_ops", dx_apple_ops::main),
    ("bench_report", dx_bench_report::main),
    ("coverlet", dx_coverlet::main),
    ("d_coverage", dx_d_coverage::main),
    ("fuzz_run", dx_fuzz_run::main),
    ("hpc_coverage", dx_hpc_coverage::main),
    ("opa_test", dx_opa_test::main),
    ("php_coverage", dx_php_coverage::main),
    ("shard_runner", dx_shard_runner::main),
];

/// The helper a rule named, if there is one.
fn find(tool: &OsStr) -> Option<Helper> {
    TOOLS
        .iter()
        .find(|(name, _)| tool == *name)
        .map(|(_, run)| *run)
}

fn main() -> ExitCode {
    let names = TOOLS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ");
    let Some(tool) = std::env::var_os(TOOL_ENV) else {
        eprintln!("dx-runtime: {TOOL_ENV} is not set; want one of {names}");
        return ExitCode::from(2);
    };
    match find(&tool) {
        Some(run) => run(),
        None => {
            eprintln!(
                "dx-runtime: unknown {TOOL_ENV}={}; want one of {names}",
                tool.to_string_lossy()
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TOOLS, find};
    use std::ffi::OsStr;

    #[test]
    fn every_helper_is_found_by_its_name() {
        for (name, _) in TOOLS.iter().copied() {
            assert!(find(OsStr::new(name)).is_some(), "{name}");
        }
    }

    #[test]
    fn an_unknown_or_user_name_is_not_a_helper() {
        assert!(find(OsStr::new("my_rego_test")).is_none());
        assert!(find(OsStr::new("")).is_none());
    }
}
