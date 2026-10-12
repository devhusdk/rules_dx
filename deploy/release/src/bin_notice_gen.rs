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

struct NoticeGenInputs {
    manifest: PathBuf,
    dst: PathBuf,
    root: String,
    texts: Vec<PathBuf>,
}

fn parse_notice_gen_args(argv: &[String]) -> Option<NoticeGenInputs> {
    if argv.len() < 5 {
        return None;
    }
    Some(NoticeGenInputs {
        manifest: PathBuf::from(&argv[1]),
        dst: PathBuf::from(&argv[2]),
        root: argv[3].clone(),
        texts: argv[4..].iter().map(PathBuf::from).collect(),
    })
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let parsed = match parse_notice_gen_args(argv) {
        Some(parsed) => parsed,
        None => {
            return dx_release_tools::bin_usage(
                "notice_gen",
                "<manifest> <out> <root> <text> [<text> ...]",
            );
        }
    };
    match dx_release_tools::write_notice(&parsed.manifest, &parsed.dst, &parsed.root, &parsed.texts)
    {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("notice_gen", &parsed.dst, error),
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
    fn parse_rejects_short_argv() {
        assert!(parse_notice_gen_args(&argv_of(&["notice_gen"])).is_none());
        assert!(
            parse_notice_gen_args(&argv_of(&["notice_gen", "manifest", "out", "root"])).is_none()
        );
    }

    #[test]
    fn parse_keeps_single_text() {
        let parsed = parse_notice_gen_args(&argv_of(&[
            "notice_gen",
            "manifest",
            "out",
            "//demo:root",
            "a.txt",
        ]))
        .expect("full argv parses");
        assert_eq!(parsed.manifest, PathBuf::from("manifest"));
        assert_eq!(parsed.dst, PathBuf::from("out"));
        assert_eq!(parsed.root, "//demo:root");
        assert_eq!(parsed.texts, vec![PathBuf::from("a.txt")]);
    }

    #[test]
    fn parse_keeps_multiple_texts_in_order() {
        let parsed = parse_notice_gen_args(&argv_of(&[
            "notice_gen",
            "manifest",
            "out",
            "//demo:root",
            "a.txt",
            "b.txt",
        ]))
        .expect("full argv parses");
        assert_eq!(
            parsed.texts,
            vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]
        );
    }

    #[test]
    fn run_without_args_exits_nonzero() {
        assert_eq!(run(&argv_of(&["notice_gen"])), 1);
    }

    #[test]
    fn run_with_missing_manifest_exits_nonzero() {
        assert_eq!(
            run(&argv_of(&[
                "notice_gen",
                "/definitely/missing/inventory.txt",
                "/definitely/missing/NOTICE",
                "//demo:root",
                "/definitely/missing/a.txt",
            ])),
            1
        );
    }
}
