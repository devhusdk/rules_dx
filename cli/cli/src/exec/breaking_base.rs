//! Builds the base image `buf breaking` compares against. A Bazel action cannot
//! see `.git`, so the image is built here and its path replaces the ref in the
//! `--breaking-against` build setting.

use super::pinned_tool::{PinnedTool, PinnedToolError, QueryBazel, resolve_pinned};
use dx_plan::args::BREAKING_AGAINST_OPTION;
use dx_plan::resolve::{QueryRunner, merge_base};
use std::path::{Path, PathBuf};

const BUF: PinnedTool<'static> = PinnedTool {
    var: "DX_BUF_BIN",
    label: "@dx_tools//:buf",
};

pub(crate) struct Context<'a> {
    pub(crate) workspace: &'a Path,
    pub(crate) runner: &'a dyn dx_process::Runner,
    pub(crate) query_runner: &'a dyn QueryRunner,
    pub(crate) startup: &'a [String],
    pub(crate) temp_dir: &'a Path,
}

/// Replaces the ref in the breaking option with the path of an image built at
/// the merge base of `HEAD` and that ref. Other options pass through.
pub(crate) fn resolve_options(
    options: &[String],
    context: &Context<'_>,
) -> Result<Vec<String>, String> {
    let prefix = format!("{BREAKING_AGAINST_OPTION}=");
    let mut resolved = Vec::with_capacity(options.len());
    for option in options {
        match option.strip_prefix(&prefix) {
            Some(reference) => {
                let image = build_image(reference, context)?;
                resolved.push(format!("{prefix}{}", image.display()));
            }
            None => resolved.push(option.clone()),
        }
    }
    Ok(resolved)
}

fn build_image(reference: &str, context: &Context<'_>) -> Result<PathBuf, String> {
    let sha = merge_base(
        Path::new("git"),
        context.workspace,
        context.query_runner,
        "HEAD",
        reference,
    ).map_err(|error| error.to_string())?;
    let bazel = QueryBazel {
        query_runner: context.query_runner,
        workspace: context.workspace,
        startup: context.startup,
    };
    let buf = resolve_pinned(context.runner, &bazel, &BUF, false).map_err(|error| match error {
        PinnedToolError::Invalid => "DX_BUF_BIN must be an absolute path".to_owned(),
        PinnedToolError::Unresolved { detail } => format!("could not resolve buf: {detail}"),
    })?;
    let image = context.temp_dir.join(format!("dx-buf-base-{sha}.binpb"));
    let argv = vec![
        buf.to_string_lossy().into_owned(),
        "build".to_owned(),
        format!(".git#ref={sha}"),
        "-o".to_owned(),
        image.to_string_lossy().into_owned(),
    ];
    let result = context
        .query_runner
        .run_query(&argv, context.workspace)
        .map_err(|error| format!("could not run buf build for {reference}: {error}"))?;
    if result.code != Some(0) {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!(
            "buf build of {reference} ({sha}) failed: {}",
            stderr.trim()
        ));
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_plan::resolve::QueryResult;
    use std::cell::RefCell;
    use std::io;

    struct Fake {
        calls: RefCell<Vec<Vec<String>>>,
        buf_code: i32,
    }

    impl QueryRunner for Fake {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            self.calls.borrow_mut().push(argv.to_vec());
            let (code, stdout) = if argv.get(1).is_some_and(|word| word == "merge-base") {
                (0, b"abc123\n".to_vec())
            } else {
                (self.buf_code, Vec::new())
            };
            Ok(QueryResult {
                code: Some(code),
                stdout,
                stderr: b"no such ref".to_vec(),
            })
        }
    }

    struct BufVar;

    impl dx_process::Runner for BufVar {
        fn run(
            &self,
            _argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            Err(io::Error::other("unexpected run"))
        }

        fn pinned_tool(&self, _var: &str) -> dx_process::PinnedVar {
            dx_process::PinnedVar::Path(PathBuf::from("/pinned/buf"))
        }
    }

    fn run(buf_code: i32) -> (Result<Vec<String>, String>, Vec<Vec<String>>) {
        let fake = Fake {
            calls: RefCell::new(Vec::new()),
            buf_code,
        };
        let options = vec![
            "--keep_going".to_owned(),
            format!("{BREAKING_AGAINST_OPTION}=origin/main"),
        ];
        let context = Context {
            workspace: Path::new("/ws"),
            runner: &BufVar,
            query_runner: &fake,
            startup: &[],
            temp_dir: Path::new("/tmp/dx"),
        };
        let got = resolve_options(&options, &context);
        (got, fake.calls.into_inner())
    }

    #[test]
    fn the_ref_becomes_an_image_built_at_the_merge_base() {
        let (got, calls) = run(0);
        let got = got.expect("resolved");
        assert_eq!(got[0], "--keep_going");
        let image = Path::new("/tmp/dx").join("dx-buf-base-abc123.binpb");
        assert_eq!(
            got[1],
            format!("{BREAKING_AGAINST_OPTION}={}", image.display())
        );
        let build = calls.last().expect("buf call");
        assert_eq!(build[0], "/pinned/buf");
        assert_eq!(build[1..3], ["build", ".git#ref=abc123"]);
    }

    #[test]
    fn a_failed_image_build_names_the_ref() {
        let (got, _) = run(1);
        let error = got.expect_err("failed");
        assert!(
            error.contains("origin/main") && error.contains("no such ref"),
            "{error}"
        );
    }
}
