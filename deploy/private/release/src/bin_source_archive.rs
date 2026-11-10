#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::path::PathBuf;

struct SourceArchiveInputs<'a> {
    tag: &'a str,
    commit: &'a str,
    out_dir: PathBuf,
    pins: Option<PathBuf>,
}

fn parse_source_archive_args(argv: &[String]) -> Option<SourceArchiveInputs<'_>> {
    let pins = match argv.len() {
        4 => None,
        6 if argv[4] == "--pins" => Some(PathBuf::from(&argv[5])),
        _ => return None,
    };
    Some(SourceArchiveInputs {
        tag: &argv[1],
        commit: &argv[2],
        out_dir: PathBuf::from(&argv[3]),
        pins,
    })
}

fn workspace_dir_of(value: Option<OsString>) -> Option<PathBuf> {
    value
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.is_absolute())
}

fn write_with(inputs: &SourceArchiveInputs<'_>, repo: Option<PathBuf>) -> Result<String, String> {
    let repo = repo.ok_or_else(|| {
        "source_archive: BUILD_WORKSPACE_DIRECTORY must be an absolute path (run with bazel run)"
            .to_owned()
    })?;
    let candidate = dx_release_tools::release_candidate(inputs.tag, Some(inputs.commit))?;
    let pins =
        match &inputs.pins {
            Some(path) => Some(std::fs::read(path).map_err(|error| {
                format!("source_archive: cannot read {}: {error}", path.display())
            })?),
            None => None,
        };
    let built = dx_release_tools::candidate::write_release_source(
        &repo,
        &candidate,
        &inputs.out_dir,
        pins.as_deref(),
    )?;
    Ok(format!(
        "source_archive: wrote {} sha256 {} to {}\n",
        dx_release_tools::candidate::release_source_name(&candidate),
        built.digest,
        inputs.out_dir.display()
    ))
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let Some(inputs) = parse_source_archive_args(argv) else {
        return dx_release_tools::bin_usage(
            "source_archive",
            "<tag> <commit> <out-dir> [--pins FILE]",
        );
    };
    let repo = workspace_dir_of(std::env::var_os("BUILD_WORKSPACE_DIRECTORY"));
    match write_with(&inputs, repo) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
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
        assert!(parse_source_archive_args(&argv_of(&["source_archive"])).is_none());
        assert!(
            parse_source_archive_args(&argv_of(&["source_archive", "v1.0.0", "abc"])).is_none()
        );
        assert!(parse_source_archive_args(&argv_of(&["s", "v1", "abc", "/o", "x"])).is_none());
        assert!(parse_source_archive_args(&argv_of(&["s", "v1", "abc", "/o", "--pins"])).is_none());
        assert!(
            parse_source_archive_args(&argv_of(&["s", "v1", "abc", "/o", "--x", "f"])).is_none()
        );
        assert!(
            parse_source_archive_args(&argv_of(&["s", "v1", "abc", "/o", "--pins", "f", "g"]))
                .is_none()
        );
    }

    #[test]
    fn parse_reads_tag_commit_and_out_dir() {
        let argv = argv_of(&["source_archive", "v1.0.0", "abc", "/out"]);
        let parsed = parse_source_archive_args(&argv).expect("parsed");
        assert_eq!(parsed.tag, "v1.0.0");
        assert_eq!(parsed.commit, "abc");
        assert_eq!(parsed.out_dir, PathBuf::from("/out"));
        assert_eq!(parsed.pins, None);
    }

    #[test]
    fn parse_reads_the_pins_file() {
        let argv = argv_of(&[
            "source_archive",
            "v1.0.0",
            "abc",
            "/out",
            "--pins",
            "/p.bzl",
        ]);
        let parsed = parse_source_archive_args(&argv).expect("parsed");
        assert_eq!(parsed.pins, Some(PathBuf::from("/p.bzl")));
    }

    #[test]
    fn write_reports_an_unreadable_pins_file() {
        let commit = "a".repeat(40);
        let argv = argv_of(&["s", "v1.0.0", &commit, "/out", "--pins", "/nope/p.bzl"]);
        let parsed = parse_source_archive_args(&argv).expect("parsed");
        let error = write_with(&parsed, Some(PathBuf::from("/ws"))).expect_err("no file");
        assert!(error.contains("cannot read /nope/p.bzl"), "{error}");
    }

    #[test]
    fn workspace_dir_must_be_absolute_and_non_empty() {
        assert_eq!(workspace_dir_of(None), None);
        assert_eq!(workspace_dir_of(Some(OsString::new())), None);
        assert_eq!(workspace_dir_of(Some(OsString::from("rel"))), None);
        assert_eq!(
            workspace_dir_of(Some(OsString::from("/ws"))),
            Some(PathBuf::from("/ws"))
        );
    }

    #[test]
    fn write_needs_workspace_and_valid_candidate() {
        let argv = argv_of(&["source_archive", "v1.0.0", "abc", "/out"]);
        let parsed = parse_source_archive_args(&argv).expect("parsed");
        let error = write_with(&parsed, None).expect_err("no workspace");
        assert!(error.contains("BUILD_WORKSPACE_DIRECTORY"), "{error}");
        let error = write_with(&parsed, Some(PathBuf::from("/ws"))).expect_err("bad commit");
        assert!(error.contains("invalid commit"), "{error}");
        let argv = argv_of(&["source_archive", "1.0.0", &"a".repeat(40), "/out"]);
        let parsed = parse_source_archive_args(&argv).expect("parsed");
        let error = write_with(&parsed, Some(PathBuf::from("/ws"))).expect_err("bad tag");
        assert!(error.contains("must be v<SemVer>"), "{error}");
    }
}
