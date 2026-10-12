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

struct ProvGenInputs {
    src: PathBuf,
    dst: PathBuf,
    builder: String,
}

fn parse_prov_gen_args(argv: &[String]) -> Option<ProvGenInputs> {
    if argv.len() != 4 {
        return None;
    }
    Some(ProvGenInputs {
        src: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
        builder: argv[3].clone(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let parsed = match parse_prov_gen_args(argv) {
        Some(parsed) => parsed,
        None => {
            return dx_release_tools::bin_usage("sbom_prov_gen", "<artifact> <out> <builder_id>");
        }
    };
    match dx_release_tools::write_provenance(&parsed.src, &parsed.dst, &parsed.builder) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("sbom_prov_gen", &parsed.dst, error),
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
        assert!(parse_prov_gen_args(&argv_of(&["sbom_prov_gen"])).is_none());
        assert!(parse_prov_gen_args(&argv_of(&["sbom_prov_gen", "artifact", "out"])).is_none());
        assert!(parse_prov_gen_args(&argv_of(&[
            "sbom_prov_gen",
            "artifact",
            "out",
            "builder",
            "extra",
        ]))
        .is_none());
    }

    #[test]
    fn parse_keeps_all_fields() {
        let parsed =
            parse_prov_gen_args(&argv_of(&["sbom_prov_gen", "artifact", "out", "builder"]))
                .expect("full argv parses");
        assert_eq!(parsed.src, PathBuf::from("artifact"));
        assert_eq!(parsed.dst, PathBuf::from("out"));
        assert_eq!(parsed.builder, "builder");
    }

    #[test]
    fn run_without_args_exits_nonzero() {
        assert_eq!(run(&argv_of(&["sbom_prov_gen"])), 1);
    }

    #[test]
    fn run_with_unwritable_target_exits_nonzero() {
        assert_eq!(
            run(&argv_of(&[
                "sbom_prov_gen",
                "/definitely/missing/artifact.bin",
                "/definitely/missing/out.prov.json",
                "https://invalid.test/builder",
            ])),
            1
        );
    }
}
