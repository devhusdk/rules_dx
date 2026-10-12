#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

struct SpdxGenInputs {
    src: PathBuf,
    dst: PathBuf,
    package: String,
    supplier: String,
}

fn parse_spdx_gen_args(argv: &[String]) -> Option<SpdxGenInputs> {
    if argv.len() != 5 {
        return None;
    }
    Some(SpdxGenInputs {
        src: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
        package: argv[3].clone(),
        supplier: argv[4].clone(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let parsed = match parse_spdx_gen_args(argv) {
        Some(parsed) => parsed,
        None => {
            return dx_release_tools::bin_usage(
                "sbom_spdx_gen",
                "<artifact> <out> <package> <supplier>",
            );
        }
    };
    match dx_release_tools::write_spdx(&parsed.src, &parsed.dst, &parsed.package, &parsed.supplier)
    {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("sbom_spdx_gen", &parsed.dst, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn argv_of(parts: &[&str]) -> Vec<String> {
        parts.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parse_rejects_wrong_arity() {
        assert!(parse_spdx_gen_args(&argv_of(&["sbom_spdx_gen"])).is_none());
        assert!(
            parse_spdx_gen_args(&argv_of(&["sbom_spdx_gen", "artifact", "out", "dx",])).is_none()
        );
        assert!(parse_spdx_gen_args(&argv_of(&[
            "sbom_spdx_gen",
            "artifact",
            "out",
            "dx",
            "supplier",
            "extra",
        ]))
        .is_none());
    }

    #[test]
    fn parse_keeps_all_fields() {
        let parsed = parse_spdx_gen_args(&argv_of(&[
            "sbom_spdx_gen",
            "artifact",
            "out",
            "dx",
            "rules_dx",
        ]))
        .expect("full argv parses");
        assert_eq!(parsed.src, PathBuf::from("artifact"));
        assert_eq!(parsed.dst, PathBuf::from("out"));
        assert_eq!(parsed.package, "dx");
        assert_eq!(parsed.supplier, "rules_dx");
    }

    #[test]
    fn run_without_args_exits_nonzero() {
        assert_eq!(run(&argv_of(&["sbom_spdx_gen"])), 1);
    }

    #[test]
    fn run_with_missing_artifact_exits_nonzero() {
        assert_eq!(
            run(&argv_of(&[
                "sbom_spdx_gen",
                "/definitely/missing/artifact.bin",
                "/definitely/missing/out.spdx.json",
                "dx",
                "rules_dx",
            ])),
            1
        );
    }
}
