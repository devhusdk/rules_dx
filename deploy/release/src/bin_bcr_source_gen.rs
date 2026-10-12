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

struct BcrGenInputs {
    dst: PathBuf,
    module: String,
    version: String,
}

fn parse_bcr_gen_args(argv: &[String]) -> Option<BcrGenInputs> {
    if argv.len() != 4 {
        return None;
    }
    Some(BcrGenInputs {
        dst: PathBuf::from(&argv[1]),
        module: argv[2].clone(),
        version: argv[3].clone(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let parsed = match parse_bcr_gen_args(argv) {
        Some(parsed) => parsed,
        None => {
            return dx_release_tools::bin_usage("bcr_source_gen", "<out> <module> <version>");
        }
    };
    match dx_release_tools::write_bcr_source(&parsed.dst, &parsed.module, &parsed.version) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("bcr_source_gen", &parsed.dst, error),
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
        assert!(parse_bcr_gen_args(&argv_of(&["bcr_source_gen"])).is_none());
        assert!(parse_bcr_gen_args(&argv_of(&["bcr_source_gen", "out", "rules_dx"])).is_none());
        assert!(parse_bcr_gen_args(&argv_of(&[
            "bcr_source_gen",
            "out",
            "rules_dx",
            "1.2.3",
            "extra",
        ]))
        .is_none());
    }

    #[test]
    fn parse_keeps_all_fields() {
        let parsed = parse_bcr_gen_args(&argv_of(&["bcr_source_gen", "out", "rules_dx", "1.2.3"]))
            .expect("full argv parses");
        assert_eq!(parsed.dst, PathBuf::from("out"));
        assert_eq!(parsed.module, "rules_dx");
        assert_eq!(parsed.version, "1.2.3");
    }

    #[test]
    fn run_without_args_exits_nonzero() {
        assert_eq!(run(&argv_of(&["bcr_source_gen"])), 1);
    }

    #[test]
    fn run_with_unwritable_target_exits_nonzero() {
        assert_eq!(
            run(&argv_of(&[
                "bcr_source_gen",
                "/definitely/missing/dir/out.source.json",
                "rules_dx",
                "1.2.3",
            ])),
            1
        );
    }
}
